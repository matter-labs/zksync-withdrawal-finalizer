#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Finalization logic implementation.

use std::time::Duration;

use accumulator::WithdrawalsAccumulator;
use ethers::{
    providers::Middleware,
    types::{H256, U256},
};
use futures::TryFutureExt;
use sqlx::PgPool;

use client::{
    is_eth, withdrawal_finalizer::codegen::withdrawal_finalizer::Result as FinalizeResult,
};
use client::{
    l1bridge::codegen::IL1Bridge, withdrawal_finalizer::codegen::WithdrawalFinalizer,
    zksync_contract::codegen::IZkSync, WithdrawalData, WithdrawalParams, ZksyncMiddleware,
};

use crate::error::{Error, Result};

mod accumulator;
mod error;

/// Finalizer.
pub struct Finalizer<M1, M2> {
    pgpool: PgPool,
    one_withdrawal_gas_limit: U256,
    batch_finalization_gas_limit: U256,
    contract: WithdrawalFinalizer<M1>,
    from_l2_block: u64,
    zksync_contract: IZkSync<M2>,
    l1_bridge: IL1Bridge<M2>,
    unsuccessful: Vec<WithdrawalData>,

    no_new_withdrawals_backoff: Duration,
    query_db_pagination_limit: u64,
    tx_fee_limit: U256,
}

const NO_NEW_WITHDRAWALS_BACKOFF: Duration = Duration::from_secs(5);
const QUERY_DB_PAGINATION_LIMIT: u64 = 50;

impl<M1, M2> Finalizer<M1, M2>
where
    M1: Middleware + 'static,
    M2: Middleware + 'static,
{
    /// Create a new `Finalizer`.
    pub fn new(
        pgpool: PgPool,
        one_withdrawal_gas_limit: U256,
        batch_finalization_gas_limit: U256,
        contract: WithdrawalFinalizer<M1>,
        from_l2_block: u64,
        zksync_contract: IZkSync<M2>,
        l1_bridge: IL1Bridge<M2>,
    ) -> Self {
        let tx_fee_limit =
            ethers::utils::parse_ether("0.8").expect("0.8 ether is a parsable amount; qed");

        Self {
            pgpool,
            one_withdrawal_gas_limit,
            batch_finalization_gas_limit,
            contract,
            from_l2_block,
            zksync_contract,
            l1_bridge,
            unsuccessful: vec![],
            no_new_withdrawals_backoff: NO_NEW_WITHDRAWALS_BACKOFF,
            query_db_pagination_limit: QUERY_DB_PAGINATION_LIMIT,
            tx_fee_limit,
        }
    }

    /// `Finalizer` main loop.
    pub async fn run<M3>(self, middleware: M3) -> Result<()>
    where
        M3: ZksyncMiddleware + 'static,
    {
        let migrator_handle = tokio::spawn(migrator_loop(
            self.pgpool.clone(),
            middleware,
            self.from_l2_block,
        ));

        let finalizer_handle = tokio::spawn(self.finalizer_loop());

        tokio::select! {
            m = migrator_handle => {
                vlog::error!("migrator ended with {m:?}");
            }
            f = finalizer_handle => {
                vlog::error!("finalizer ended with {f:?}");
            }
        }

        Ok(())
    }

    async fn predict_fails(
        &mut self,
        withdrawals: &[WithdrawalData],
    ) -> Result<Vec<FinalizeResult>> {
        let mut results = vec![];

        let w: Vec<_> = withdrawals
            .iter()
            .map(|r| {
                r.clone()
                    .into_request_with_gaslimit(self.one_withdrawal_gas_limit)
            })
            .collect();

        let predictions = self.contract.finalize_withdrawals(w.clone()).call().await?;

        for prediction in predictions {
            if !prediction.success || prediction.gas > self.one_withdrawal_gas_limit {
                results.push(prediction);
            }
        }

        Ok(results)
    }

    async fn finalize_batch(&mut self, withdrawals: Vec<WithdrawalData>) -> Result<()> {
        vlog::debug!("Finalizeing batch {withdrawals:?}");

        let w: Vec<_> = withdrawals
            .iter()
            .map(|r| {
                r.clone()
                    .into_request_with_gaslimit(self.one_withdrawal_gas_limit)
            })
            .collect();

        let tx = self.contract.finalize_withdrawals(w);
        let pending_tx = tx.send().await;

        // Turn actual withdrawals into info to update db with.
        let withdrawals = withdrawals
            .into_iter()
            .map(|w| (w.tx_hash, w.event_index_in_tx))
            .collect::<Vec<_>>();

        let pending_tx = match pending_tx {
            Ok(e) => e,
            Err(e) => {
                vlog::error!("failed to send finalization withdrawal tx: {:?}", e);
                storage::inc_unsuccessful_finalization_attempts(&self.pgpool, &withdrawals).await?;

                return Ok(());
            }
        };

        let mined = pending_tx.await;

        match mined {
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
            }
            // TODO: why would a pending tx resolve to `None`?
            Ok(None) => (),
            Err(e) => {
                vlog::error!("finalizing withdrawals failed with an error {:?}", e);
                storage::inc_unsuccessful_finalization_attempts(&self.pgpool, &withdrawals).await?;
            }
        }

        Ok(())
    }

    async fn new_accumulator(&self) -> Result<WithdrawalsAccumulator> {
        let gas_price = self
            .contract
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

    async fn finalizer_loop(mut self) -> Result<()>
    where
        M1: Middleware,
        M2: Middleware,
    {
        loop {
            let try_finalize_these =
                storage::withdrwals_to_finalize(&self.pgpool, self.query_db_pagination_limit)
                    .await?;

            if try_finalize_these.is_empty() {
                tokio::time::sleep(self.no_new_withdrawals_backoff).await;
                continue;
            }

            let mut accumulator = self.new_accumulator().await?;
            let mut iter = try_finalize_these.iter().peekable();

            while let Some(t) = iter.next() {
                accumulator.add_withdrawal(t.clone());

                if accumulator.ready_to_finalize() || iter.peek().is_none() {
                    let predicted_to_fail = self.predict_fails(accumulator.withdrawals()).await?;

                    vlog::debug!("predicted to fail: {predicted_to_fail:?}");

                    if !predicted_to_fail.is_empty() {
                        let mut removed = accumulator.remove_unsuccessful(&predicted_to_fail);

                        self.unsuccessful.append(&mut removed);
                        continue;
                    } else {
                        let requests = accumulator.take_withdrawals();
                        self.finalize_batch(requests).await?;
                    }
                }
            }

            self.process_unsuccessful().await?;
        }
    }

    async fn process_unsuccessful(&mut self) -> Result<()> {
        if self.unsuccessful.is_empty() {
            return Ok(());
        }

        let predicted = std::mem::take(&mut self.unsuccessful);

        let finalize_query: Vec<_> = predicted
            .iter()
            .map(|d| {
                (
                    d.params.l1_batch_number.as_u64(),
                    d.params.l2_message_index as u64,
                    is_eth(d.params.sender),
                )
            })
            .collect();

        let are_finalized =
            are_withdrawals_finalized(&self.zksync_contract, &self.l1_bridge, &finalize_query)
                .await?;

        let mut already_finalized = vec![];
        let mut unsuccessful = vec![];

        for i in 0..are_finalized.len() {
            if are_finalized[i] {
                already_finalized.push((predicted[i].tx_hash, predicted[i].event_index_in_tx));
            } else {
                unsuccessful.push((predicted[i].tx_hash, predicted[i].event_index_in_tx));
            }
        }

        storage::inc_unsuccessful_finalization_attempts(&self.pgpool, &unsuccessful).await?;
        storage::finalization_data_set_finalized_in_tx(
            &self.pgpool,
            &already_finalized,
            H256::zero(),
        )
        .await?;

        Ok(())
    }
}

async fn request_finalize_params<M2>(
    middleware: M2,
    hash_and_indices: &[(H256, u16)],
) -> Result<Vec<WithdrawalParams>>
where
    M2: ZksyncMiddleware,
{
    let results: Result<Vec<_>> =
        futures::future::join_all(hash_and_indices.iter().map(|(h, i)| {
            middleware
                .finalize_withdrawal_params(*h, *i as usize)
                .map_ok(|r| r.expect("always able to ask withdrawal params; qed"))
                .map_err(|e| e.into())
        }))
        .await
        .into_iter()
        .collect();

    let results = results?;

    Ok(results)
}

async fn migrator_loop<M2>(pool: PgPool, middleware: M2, from_l2_block: u64) -> Result<()>
where
    M2: ZksyncMiddleware,
{
    loop {
        let newly_executed_withdrawals =
            storage::get_withdrawals_with_no_data(&pool, from_l2_block, 50).await?;

        if newly_executed_withdrawals.is_empty() {
            tokio::time::sleep(NO_NEW_WITHDRAWALS_BACKOFF).await;
            continue;
        }

        vlog::info!("newly executed withdrawals {newly_executed_withdrawals:?}");

        let hash_and_index: Vec<_> = newly_executed_withdrawals
            .iter()
            .map(|p| (p.0, p.1))
            .collect();

        let params: Vec<_> = request_finalize_params(&middleware, &hash_and_index)
            .await?
            .into_iter()
            .zip(newly_executed_withdrawals)
            .map(|(params, e)| WithdrawalData {
                tx_hash: e.0,
                event_index_in_tx: e.1 as u32,
                l2_block_number: e.2,
                params,
            })
            .collect();

        storage::add_withdrawals_data(&pool, &params).await?;
    }
}

async fn are_withdrawals_finalized<M>(
    zksync_contract: &IZkSync<M>,
    l1_bridge: &IL1Bridge<M>,
    withdrawals: &[(u64, u64, bool)],
) -> Result<Vec<bool>>
where
    M: Middleware,
{
    let results: Result<Vec<_>> =
        futures::future::join_all(withdrawals.iter().map(|(batch, index, is_eth)| async move {
            if *is_eth {
                zksync_contract
                    .is_eth_withdrawal_finalized(U256::from(*batch), U256::from(*index))
                    .call()
                    .await
                    .map_err(|e| e.into())
            } else {
                l1_bridge
                    .is_withdrawal_finalized(U256::from(*batch), U256::from(*index))
                    .call()
                    .await
                    .map_err(|e| e.into())
            }
        }))
        .await
        .into_iter()
        .collect();

    let results = results?;

    Ok(results)
}
