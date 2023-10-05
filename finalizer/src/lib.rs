#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Finalization logic implementation.

use std::{collections::HashSet, time::Duration};

use accumulator::WithdrawalsAccumulator;
use ethers::{
    abi::Address,
    providers::{Middleware, MiddlewareError},
    types::{H256, U256, U64},
};
use futures::TryFutureExt;
use sqlx::PgPool;

use client::{
    is_eth, withdrawal_finalizer::codegen::withdrawal_finalizer::Result as FinalizeResult,
    WithdrawalKey,
};
use client::{
    l1bridge::codegen::IL1Bridge, withdrawal_finalizer::codegen::WithdrawalFinalizer,
    zksync_contract::codegen::IZkSync, WithdrawalParams, ZksyncMiddleware,
};
use withdrawals_meterer::WithdrawalsMeter;

use crate::error::{Error, Result};

mod accumulator;
mod error;

/// A limit to cap a transaction fee (in ether) for safety reasons.
const TX_FEE_LIMIT: f64 = 0.8;

/// When finalizer runs out of money back off this amount of time.
const OUT_OF_FUNDS_BACKOFF: Duration = Duration::from_secs(10);

/// Backoff period if one of the loop iterations has failed.
const LOOP_ITERATION_ERROR_BACKOFF: Duration = Duration::from_secs(5);

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
    pub async fn new(
        pgpool: PgPool,
        one_withdrawal_gas_limit: U256,
        batch_finalization_gas_limit: U256,
        finalizer_contract: WithdrawalFinalizer<S>,
        zksync_contract: IZkSync<M>,
        l1_bridge: IL1Bridge<M>,
        tx_retry_timeout: usize,
        account_address: Address,
        first_block_today: Option<U64>,
    ) -> Result<Self> {
        let max_finalized_today = match first_block_today {
            Some(first_block_today) => {
                storage::max_finalized_l2_miniblock_since_block(&pgpool, first_block_today.as_u64())
                    .await?
            }
            None => None,
        };

        let historic_interval = match (first_block_today, max_finalized_today) {
            (Some(from), Some(to)) => Some((from, to.into())),
            _ => None,
        };

        vlog::info!("historic interval {historic_interval:?}");

        // we need to tell the meter to meter only finalized withdrawls
        let withdrawals_meterer = withdrawals_meterer::WithdrawalsMeter::new(
            pgpool.clone(),
            "era_withdrawal_finalizer_meter",
            historic_interval,
        )
        .await;
        let tx_fee_limit = ethers::utils::parse_ether(TX_FEE_LIMIT)
            .expect("{TX_FEE_LIMIT} ether is a parsable amount; qed");

        Ok(Self {
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
        })
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
                vlog::error!("migrator ended with {m:?}");
            }
            f = finalizer_handle => {
                vlog::error!("finalizer ended with {f:?}");
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
        vlog::info!("predicted results for withdrawals: {results:?}");

        Ok(results
            .into_iter()
            .filter(|p| !p.success || p.gas > self.one_withdrawal_gas_limit)
            .collect())
    }

    async fn finalize_batch(&mut self, withdrawals: Vec<WithdrawalParams>) -> Result<()> {
        let Some(highest_batch_number) = withdrawals.iter().map(|w| w.l1_batch_number).max() else {
            return Ok(());
        };

        vlog::info!(
            "finalizing batch {:?}",
            withdrawals.iter().map(|w| w.id).collect::<Vec<_>>()
        );

        let w: Vec<_> = withdrawals
            .iter()
            .cloned()
            .map(|r| r.into_request_with_gaslimit(self.one_withdrawal_gas_limit))
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
            Ok(Some(tx)) => {
                vlog::info!(
                    "withdrawal transaction {:?} successfully mined",
                    tx.transaction_hash
                );

                storage::finalization_data_set_finalized_in_tx(
                    &self.pgpool,
                    &withdrawals,
                    tx.transaction_hash,
                )
                .await?;

                metrics::gauge!(
                    "finalizer.highest_finalized_batch_number",
                    highest_batch_number.as_u64() as f64,
                );

                if let Err(e) = self
                    .withdrawals_meterer
                    .meter_withdrawals_storage(&ids)
                    .await
                {
                    vlog::error!("Failed to meter the withdrawals: {e}");
                }
            }
            // TODO: why would a pending tx resolve to `None`?
            Ok(None) => {
                vlog::warn!("sent transaction resolved with none result",);
            }
            Err(e) => {
                vlog::error!(
                    "waiting for transaction status withdrawals failed with an error {:?}",
                    e
                );

                if let Some(provider_error) = e.as_provider_error() {
                    vlog::error!("failed to send finalization transaction: {provider_error}");
                } else if !is_gas_required_exceeds_allowance::<S>(&e) {
                    storage::inc_unsuccessful_finalization_attempts(&self.pgpool, &withdrawals)
                        .await?;
                } else {
                    vlog::error!("failed to send finalization withdrawal tx: {e}");
                    metrics::counter!(
                        "finalizer.finalization_events.failed_to_finalize_low_gas",
                        withdrawals.len() as u64
                    );

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
                vlog::error!("iteration of finalizer loop has ended with {e}");
                tokio::time::sleep(LOOP_ITERATION_ERROR_BACKOFF).await;
            }
        }
    }

    async fn loop_iteration(&mut self) -> Result<()> {
        vlog::debug!("begin iteration of the finalizer loop");

        let try_finalize_these =
            storage::withdrwals_to_finalize(&self.pgpool, self.query_db_pagination_limit).await?;

        vlog::debug!("trying to finalize these {try_finalize_these:?}");

        if try_finalize_these.is_empty() {
            tokio::time::sleep(self.no_new_withdrawals_backoff).await;
            return Ok(());
        }

        let mut accumulator = self.new_accumulator().await?;
        let mut iter = try_finalize_these.into_iter().peekable();

        while let Some(t) = iter.next() {
            accumulator.add_withdrawal(t);

            if accumulator.ready_to_finalize() || iter.peek().is_none() {
                vlog::info!(
                    "predicting results for withdrawals: {:?}",
                    accumulator.withdrawals().map(|w| w.id).collect::<Vec<_>>()
                );

                let predicted_to_fail = self.predict_fails(accumulator.withdrawals()).await?;

                metrics::counter!(
                    "finalizer.predicted_to_fail_withdrawals",
                    predicted_to_fail.len() as u64
                );

                vlog::debug!("predicted to fail: {predicted_to_fail:?}");

                if !predicted_to_fail.is_empty() {
                    let mut removed = accumulator.remove_unsuccessful(&predicted_to_fail);

                    self.unsuccessful.append(&mut removed);
                }
            }

            if accumulator.ready_to_finalize() || iter.peek().is_none() {
                let requests = accumulator.take_withdrawals();
                self.finalize_batch(requests).await?;
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
            vlog::debug!("no unsuccessful withdrawals");
            return Ok(());
        }

        let predicted = std::mem::take(&mut self.unsuccessful);
        vlog::debug!("requesting finalization status of withdrawals");
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

        vlog::debug!(
            "setting unsuccessful finalization attempts to {} withdrawals",
            unsuccessful.len()
        );

        // Either finalization tx has failed for these, or they were
        // predicted to fail.
        storage::inc_unsuccessful_finalization_attempts(&self.pgpool, &unsuccessful).await?;

        vlog::debug!(
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
                metrics::increment_counter!(
                    "finalizer.params_fetcher.failed_to_fetch_withdrawal_params"
                );
                vlog::error!(
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
            vlog::error!("params fetcher iteration ended with {e}");
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

    vlog::debug!("newly committed withdrawals {newly_executed_withdrawals:?}");

    let hash_and_index_and_id: Vec<_> = newly_executed_withdrawals
        .iter()
        .map(|p| (p.key.tx_hash, p.key.event_index_in_tx as u16, p.id))
        .collect();

    let params = request_finalize_params(&middleware, &hash_and_index_and_id).await;

    let already_finalized: Vec<_> = get_finalized_withdrawals(&params, zksync_contract, l1_bridge)
        .await?
        .into_iter()
        .collect();

    storage::add_withdrawals_data(pool, &params).await?;
    storage::finalization_data_set_finalized_in_tx(pool, &already_finalized, H256::zero()).await?;

    Ok(())
}
