#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Finalization logic implementation.

use std::{collections::HashSet, str::FromStr, time::Duration};

use accumulator::WithdrawalsAccumulator;
use ethers::{
    abi::Address,
    providers::{Middleware, MiddlewareError},
    types::{H256, U256},
};
use futures::TryFutureExt;
use serde::Deserialize;
use sqlx::PgPool;

use client::{
    is_eth, withdrawal_finalizer::codegen::withdrawal_finalizer::Result as FinalizeResult,
    WithdrawalKey,
};
use client::{
    l1bridge::codegen::IL1Bridge, withdrawal_finalizer::codegen::WithdrawalFinalizer,
    zksync_contract::codegen::IZkSync, WithdrawalParams, ZksyncMiddleware,
};
use withdrawals_meterer::{MeteringComponent, WithdrawalsMeter};

use crate::{
    error::{Error, Result},
    metrics::FINALIZER_METRICS,
};

mod accumulator;
mod error;
mod metrics;

/// A limit to cap a transaction fee (in ether) for safety reasons.
const TX_FEE_LIMIT: f64 = 0.8;

/// When finalizer runs out of money back off this amount of time.
const OUT_OF_FUNDS_BACKOFF: Duration = Duration::from_secs(10);

/// Backoff period if one of the loop iterations has failed.
const LOOP_ITERATION_ERROR_BACKOFF: Duration = Duration::from_secs(5);

/// An `enum` that defines a set of tokens that Finalizer finalizes.
#[derive(Deserialize, Debug, Eq, PartialEq)]
pub enum TokenList {
    /// Finalize all known tokens
    All,
    /// Finalize nothing
    None,
    /// Finalize everything but these tokens, this is a blacklist.
    BlackList(Vec<Address>),
    /// Finalize nothing but these tokens, this is a whitelist.
    WhiteList(Vec<Address>),
}

impl Default for TokenList {
    fn default() -> Self {
        Self::WhiteList(vec![client::ETH_TOKEN_ADDRESS])
    }
}

impl FromStr for TokenList {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let res = serde_json::from_str(s)?;
        Ok(res)
    }
}

/// Finalizer.
pub struct Finalizer<M1, M2> {
    pgpool: PgPool,
    one_withdrawal_gas_limit: U256,
    batch_finalization_gas_limit: U256,
    finalizer_contract: WithdrawalFinalizer<M1>,
    zksync_contract: IZkSync<M2>,
    l1_bridge: IL1Bridge<M2>,
    unsuccessful: Vec<WithdrawalParams>,

    no_new_withdrawals_backoff: Duration,
    query_db_pagination_limit: u64,
    tx_fee_limit: U256,
    tx_retry_timeout: Duration,
    account_address: Address,
    withdrawals_meterer: WithdrawalsMeter,
    token_list: TokenList,
}

const NO_NEW_WITHDRAWALS_BACKOFF: Duration = Duration::from_secs(5);
const QUERY_DB_PAGINATION_LIMIT: u64 = 50;

impl<S, M> Finalizer<S, M>
where
    S: Middleware + 'static,
    M: Middleware + 'static,
{
    /// Create a new [`Finalizer`].
    ///
    /// * `S` is expected to be a [`Middleware`] instance equipped with [`SignerMiddleware`]
    /// * `M` is expected to be an ordinary read-only middleware to read information from L1.
    ///
    /// [`SignerMiddleware`]: https://docs.rs/ethers/latest/ethers/middleware/struct.SignerMiddleware.html
    /// [`Middleware`]: https://docs.rs/ethers/latest/ethers/providers/trait.Middleware.html
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pgpool: PgPool,
        one_withdrawal_gas_limit: U256,
        batch_finalization_gas_limit: U256,
        finalizer_contract: WithdrawalFinalizer<S>,
        zksync_contract: IZkSync<M>,
        l1_bridge: IL1Bridge<M>,
        tx_retry_timeout: usize,
        account_address: Address,
        token_list: TokenList,
    ) -> Self {
        let withdrawals_meterer =
            WithdrawalsMeter::new(pgpool.clone(), MeteringComponent::FinalizedWithdrawals);
        let tx_fee_limit = ethers::utils::parse_ether(TX_FEE_LIMIT)
            .expect("{TX_FEE_LIMIT} ether is a parsable amount; qed");

        tracing::info!("finalizing tokens {token_list:?}");

        Self {
            pgpool,
            one_withdrawal_gas_limit,
            batch_finalization_gas_limit,
            finalizer_contract,
            zksync_contract,
            l1_bridge,
            unsuccessful: vec![],
            no_new_withdrawals_backoff: NO_NEW_WITHDRAWALS_BACKOFF,
            query_db_pagination_limit: QUERY_DB_PAGINATION_LIMIT,
            tx_fee_limit,
            tx_retry_timeout: Duration::from_secs(tx_retry_timeout as u64),
            account_address,
            withdrawals_meterer,
            token_list,
        }
    }

    /// [`Finalizer`] main loop.
    ///
    /// `M2` is expected to be an [`ZksyncMiddleware`] to connect to L2.
    pub async fn run<M2>(self, middleware: M2) -> Result<()>
    where
        M2: ZksyncMiddleware + 'static,
    {
        let params_fetcher_handle = tokio::spawn(params_fetcher_loop(
            self.pgpool.clone(),
            middleware,
            self.zksync_contract.clone(),
            self.l1_bridge.clone(),
        ));

        let finalizer_handle = tokio::spawn(self.finalizer_loop());

        tokio::select! {
            m = params_fetcher_handle => {
                tracing::error!("migrator ended with {m:?}");
            }
            f = finalizer_handle => {
                tracing::error!("finalizer ended with {f:?}");
            }
        }

        Ok(())
    }

    async fn predict_fails<'a, W: Iterator<Item = &'a WithdrawalParams>>(
        &mut self,
        withdrawals: W,
    ) -> Result<Vec<FinalizeResult>> {
        let w: Vec<_> = withdrawals
            .cloned()
            .map(|r| r.into_request_with_gaslimit(self.one_withdrawal_gas_limit))
            .collect();

        let results = self
            .finalizer_contract
            .finalize_withdrawals(w)
            .call()
            .await?;
        tracing::info!("predicted results for withdrawals: {results:?}");

        Ok(results
            .into_iter()
            .filter(|p| !p.success || p.gas > self.one_withdrawal_gas_limit)
            .collect())
    }

    async fn finalize_batch(
        &mut self,
        withdrawals: Vec<WithdrawalParams>,
        one_withdrawal_gas_limit: U256,
    ) -> Result<()> {
        let Some(highest_batch_number) = withdrawals.iter().map(|w| w.l1_batch_number).max() else {
            return Ok(());
        };

        tracing::info!(
            "finalizing batch {:?}",
            withdrawals.iter().map(|w| w.id).collect::<Vec<_>>()
        );

        let w: Vec<_> = withdrawals
            .iter()
            .cloned()
            .map(|r| r.into_request_with_gaslimit(one_withdrawal_gas_limit))
            .collect();

        let tx = self.finalizer_contract.finalize_withdrawals(w);
        let nonce = self
            .finalizer_contract
            .client()
            .get_transaction_count(self.account_address, None)
            .await
            .map_err(|e| Error::Middleware(format!("{e}")))?;

        let tx = tx_sender::send_tx_adjust_gas(
            self.finalizer_contract.client(),
            tx.tx.clone(),
            self.tx_retry_timeout,
            nonce,
        )
        .await;

        let ids: Vec<_> = withdrawals.iter().map(|w| w.id as i64).collect();

        // Turn actual withdrawals into info to update db with.
        let withdrawals = withdrawals.into_iter().map(|w| w.key()).collect::<Vec<_>>();

        match tx {
            Ok(Some(tx)) if tx.status.expect("EIP-658 is enabled; qed").is_zero() => {
                tracing::error!(
                    "withdrawal transaction {:?} was reverted",
                    tx.transaction_hash
                );

                FINALIZER_METRICS.reverted_withdrawal_transactions.inc();
                storage::inc_unsuccessful_finalization_attempts(&self.pgpool, &withdrawals).await?;

                return Err(Error::WithdrawalTransactionReverted);
            }
            Ok(Some(tx)) => {
                tracing::info!(
                    "withdrawal transaction {:?} successfully mined",
                    tx.transaction_hash
                );

                storage::finalization_data_set_finalized_in_tx(
                    &self.pgpool,
                    &withdrawals,
                    tx.transaction_hash,
                )
                .await?;

                FINALIZER_METRICS
                    .highest_finalized_batch_number
                    .set(highest_batch_number.as_u64() as i64);

                if let Err(e) = self
                    .withdrawals_meterer
                    .meter_withdrawals_storage(&ids)
                    .await
                {
                    tracing::error!("Failed to meter the withdrawals: {e}");
                }
            }
            // TODO: why would a pending tx resolve to `None`?
            Ok(None) => {
                tracing::warn!("sent transaction resolved with none result",);
            }
            Err(e) => {
                tracing::error!(
                    "waiting for transaction status withdrawals failed with an error {:?}",
                    e
                );

                if let Some(provider_error) = e.as_provider_error() {
                    tracing::error!("failed to send finalization transaction: {provider_error}");
                } else if !is_gas_required_exceeds_allowance::<S>(&e) {
                    storage::inc_unsuccessful_finalization_attempts(&self.pgpool, &withdrawals)
                        .await?;
                } else {
                    tracing::error!("failed to send finalization withdrawal tx: {e}");
                    FINALIZER_METRICS
                        .failed_to_finalize_low_gas
                        .inc_by(withdrawals.len() as u64);

                    tokio::time::sleep(OUT_OF_FUNDS_BACKOFF).await;
                }
                // no need to bump the counter here, waiting for tx
                // has failed becuase of networking or smth, but at
                // this point tx has already been accepted into tx pool
            }
        }

        Ok(())
    }

    // Create a new withdrawal accumulator given the current gas price.
    async fn new_accumulator(&self) -> Result<WithdrawalsAccumulator> {
        let gas_price = self
            .finalizer_contract
            .client()
            .get_gas_price()
            .await
            .map_err(|e| Error::Middleware(format!("{e}")))?;

        Ok(WithdrawalsAccumulator::new(
            gas_price,
            self.tx_fee_limit,
            self.batch_finalization_gas_limit,
            self.one_withdrawal_gas_limit,
        ))
    }

    async fn finalizer_loop(mut self)
    where
        S: Middleware,
        M: Middleware,
    {
        loop {
            if let Err(e) = self.loop_iteration().await {
                tracing::error!("iteration of finalizer loop has ended with {e}");
                tokio::time::sleep(LOOP_ITERATION_ERROR_BACKOFF).await;
            }
        }
    }

    async fn loop_iteration(&mut self) -> Result<()> {
        tracing::debug!("begin iteration of the finalizer loop");

        let try_finalize_these = match &self.token_list {
            TokenList::All => {
                storage::withdrawals_to_finalize(&self.pgpool, self.query_db_pagination_limit)
                    .await?
            }
            TokenList::WhiteList(w) => {
                storage::withdrawals_to_finalize_with_whitelist(
                    &self.pgpool,
                    self.query_db_pagination_limit,
                    w,
                )
                .await?
            }
            TokenList::BlackList(b) => {
                storage::withdrawals_to_finalize_with_blacklist(
                    &self.pgpool,
                    self.query_db_pagination_limit,
                    b,
                )
                .await?
            }
            TokenList::None => return Ok(()),
        };

        tracing::debug!("trying to finalize these {try_finalize_these:?}");

        if try_finalize_these.is_empty() {
            tokio::time::sleep(self.no_new_withdrawals_backoff).await;
            return Ok(());
        }

        let mut accumulator = self.new_accumulator().await?;
        let mut iter = try_finalize_these.into_iter().peekable();

        while let Some(t) = iter.next() {
            accumulator.add_withdrawal(t);

            if accumulator.ready_to_finalize() || iter.peek().is_none() {
                tracing::info!(
                    "predicting results for withdrawals: {:?}",
                    accumulator.withdrawals().map(|w| w.id).collect::<Vec<_>>()
                );

                let predicted_to_fail = self.predict_fails(accumulator.withdrawals()).await?;

                FINALIZER_METRICS
                    .predicted_to_fail_withdrawals
                    .inc_by(predicted_to_fail.len() as u64);

                tracing::debug!("predicted to fail: {predicted_to_fail:?}");

                if !predicted_to_fail.is_empty() {
                    let mut removed = accumulator.remove_unsuccessful(&predicted_to_fail);

                    self.unsuccessful.append(&mut removed);
                }
            }

            if accumulator.ready_to_finalize() || iter.peek().is_none() {
                let requests = accumulator.take_withdrawals();
                const RETRY_REVERTED_TX: usize = 3;
                let one_withdrawal_gas_limit = self.one_withdrawal_gas_limit;

                for i in 0..RETRY_REVERTED_TX {
                    match self
                        .finalize_batch(requests.clone(), one_withdrawal_gas_limit + 500 * i)
                        .await
                    {
                        Err(Error::WithdrawalTransactionReverted) => {}
                        Err(e) => return Err(e),
                        Ok(()) => break,
                    }
                }

                accumulator = self.new_accumulator().await?;
            }
        }

        self.process_unsuccessful().await
    }

    // process withdrawals that have been predicted as unsuccessful.
    //
    // there may be many reasons for such predictions for instance the following:
    // * a withdrawal is already finalized
    // * a gas limit on request was too low
    // * erc20 has denied a tx for some internal reasons.
    async fn process_unsuccessful(&mut self) -> Result<()> {
        if self.unsuccessful.is_empty() {
            tracing::debug!("no unsuccessful withdrawals");
            return Ok(());
        }

        let predicted = std::mem::take(&mut self.unsuccessful);
        tracing::debug!("requesting finalization status of withdrawals");
        let are_finalized =
            get_finalized_withdrawals(&predicted, &self.zksync_contract, &self.l1_bridge).await?;

        let mut already_finalized = vec![];
        let mut unsuccessful = vec![];

        for p in predicted {
            let key = p.key();

            if are_finalized.contains(&key) {
                already_finalized.push(key);
            } else {
                unsuccessful.push(key);
            }
        }

        tracing::debug!(
            "setting unsuccessful finalization attempts to {} withdrawals",
            unsuccessful.len()
        );

        // Either finalization tx has failed for these, or they were
        // predicted to fail.
        storage::inc_unsuccessful_finalization_attempts(&self.pgpool, &unsuccessful).await?;

        tracing::debug!(
            "setting already finalized status to {} withdrawals",
            already_finalized.len()
        );

        // if the withdrawal has already been finalized set its
        // finalization transaction to zero which is signals exactly this
        // it is known that withdrawal has been fianlized but not known
        // in which exact transaction.
        //
        // this may happen in two cases:
        // 1. someone else has finalized it
        // 2. finalizer has finalized it however its course of
        // execution has been interrupted somewhere between the
        // submission of transaction and updating the db with the
        // result of said transaction success.
        storage::finalization_data_set_finalized_in_tx(
            &self.pgpool,
            &already_finalized,
            H256::zero(),
        )
        .await?;

        Ok(())
    }
}

async fn get_finalized_withdrawals<M>(
    withdrawals: &[WithdrawalParams],
    zksync_contract: &IZkSync<M>,
    l1_bridge: &IL1Bridge<M>,
) -> Result<HashSet<WithdrawalKey>>
where
    M: Middleware,
{
    let results: Result<Vec<_>> =
        futures::future::join_all(withdrawals.iter().map(|wd| async move {
            let l1_batch_number = U256::from(wd.l1_batch_number.as_u64());
            let l2_message_index = U256::from(wd.l2_message_index);

            if is_eth(wd.sender) {
                zksync_contract
                    .is_eth_withdrawal_finalized(l1_batch_number, l2_message_index)
                    .call()
                    .await
                    .map_err(|e| e.into())
            } else {
                l1_bridge
                    .is_withdrawal_finalized(l1_batch_number, l2_message_index)
                    .call()
                    .await
                    .map_err(|e| e.into())
            }
        }))
        .await
        .into_iter()
        .collect();

    let results = results?;

    let mut set = HashSet::new();
    for i in 0..results.len() {
        if results[i] {
            set.insert(withdrawals[i].key());
        }
    }

    Ok(set)
}

fn is_gas_required_exceeds_allowance<M: Middleware>(e: &<M as Middleware>::Error) -> bool {
    if let Some(e) = e.as_error_response() {
        return e.code == -32000 && e.message.starts_with("gas required exceeds allowance ");
    }

    false
}

// Request finalization parameters for a set of withdrawals in parallel.
async fn request_finalize_params<M2>(
    pgpool: &PgPool,
    middleware: M2,
    hash_and_indices: &[(H256, u16, u64)],
) -> Vec<WithdrawalParams>
where
    M2: ZksyncMiddleware,
{
    let mut ok_results = Vec::with_capacity(hash_and_indices.len());

    // Run all parametere fetching in parallel.
    // Filter out errors and log them and increment a metric counter.
    // Return successful fetches.
    for (i, result) in futures::future::join_all(hash_and_indices.iter().map(|(h, i, id)| {
        middleware
            .finalize_withdrawal_params(*h, *i as usize)
            .map_ok(|r| {
                let mut r = r.expect("always able to ask withdrawal params; qed");
                r.id = *id;
                r
            })
            .map_err(crate::Error::from)
    }))
    .await
    .into_iter()
    .enumerate()
    {
        match result {
            Ok(r) => ok_results.push(r),
            Err(e) => {
                FINALIZER_METRICS.failed_to_fetch_withdrawal_params.inc();
                if let Error::Client(client::Error::WithdrawalLogNotFound(index, tx_hash)) = e {
                    storage::set_withdrawal_unfinalizable(pgpool, tx_hash, index)
                        .await
                        .ok();
                }
                tracing::error!(
                    "failed to fetch withdrawal parameters: {e} {:?}",
                    hash_and_indices[i]
                );
            }
        }
    }

    ok_results
}

// Continiously query the new withdrawals that have been seen by watcher
// request finalizing params for them and store this information into
// finalizer db table.
async fn params_fetcher_loop<M1, M2>(
    pool: PgPool,
    middleware: M2,
    zksync_contract: IZkSync<M1>,
    l1_bridge: IL1Bridge<M1>,
) where
    M1: Middleware,
    M2: ZksyncMiddleware,
{
    loop {
        if let Err(e) =
            params_fetcher_loop_iteration(&pool, &middleware, &zksync_contract, &l1_bridge).await
        {
            tracing::error!("params fetcher iteration ended with {e}");
            tokio::time::sleep(LOOP_ITERATION_ERROR_BACKOFF).await;
        }
    }
}

async fn params_fetcher_loop_iteration<M1, M2>(
    pool: &PgPool,
    middleware: &M2,
    zksync_contract: &IZkSync<M1>,
    l1_bridge: &IL1Bridge<M1>,
) -> Result<()>
where
    M1: Middleware,
    M2: ZksyncMiddleware,
{
    let newly_executed_withdrawals = storage::get_withdrawals_with_no_data(pool, 1000).await?;

    if newly_executed_withdrawals.is_empty() {
        tokio::time::sleep(NO_NEW_WITHDRAWALS_BACKOFF).await;
        return Ok(());
    }

    tracing::debug!("newly committed withdrawals {newly_executed_withdrawals:?}");

    let hash_and_index_and_id: Vec<_> = newly_executed_withdrawals
        .iter()
        .map(|p| (p.key.tx_hash, p.key.event_index_in_tx as u16, p.id))
        .collect();

    let params = request_finalize_params(pool, &middleware, &hash_and_index_and_id).await;

    let already_finalized: Vec<_> = get_finalized_withdrawals(&params, zksync_contract, l1_bridge)
        .await?
        .into_iter()
        .collect();

    storage::add_withdrawals_data(pool, &params).await?;
    storage::finalization_data_set_finalized_in_tx(pool, &already_finalized, H256::zero()).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::TokenList;
    use ethers::abi::Address;
    use pretty_assertions::assert_eq;

    #[test]
    fn tokens_list_de() {
        let all = "\"All\"";

        let none = "\"None\"";

        let all: TokenList = serde_json::from_str(all).unwrap();
        assert_eq!(all, TokenList::All);

        let none: TokenList = serde_json::from_str(none).unwrap();
        assert_eq!(none, TokenList::None);

        let black = r#"
            {
                "BlackList":[
                    "0x3355df6D4c9C3035724Fd0e3914dE96A5a83aaf4"
                ]
            }
        "#;

        let usdc_addr: Address = "0x3355df6D4c9C3035724Fd0e3914dE96A5a83aaf4"
            .parse()
            .unwrap();

        let blocked_usdc: TokenList = serde_json::from_str(black).unwrap();
        assert_eq!(blocked_usdc, TokenList::BlackList(vec![usdc_addr]));

        let white = r#"
            {
                "WhiteList":[
                    "0x3355df6D4c9C3035724Fd0e3914dE96A5a83aaf4"
                ]
            }
        "#;

        let allowed_usdc: TokenList = serde_json::from_str(white).unwrap();
        assert_eq!(allowed_usdc, TokenList::WhiteList(vec![usdc_addr]));
    }
}
