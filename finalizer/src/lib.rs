#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Finalization logic implementation.

use std::time::Duration;

use ethers::types::H256;
use futures::TryFutureExt;
use sqlx::PgPool;

use client::{WithdrawalData, WithdrawalParams, ZksyncMiddleware};

mod error;

/// Finalizer.
pub struct Finalizer {
    pgpool: PgPool,
}

use crate::error::Result;

const NO_NEW_WITHDRAWALS_BACKOFF: Duration = Duration::from_secs(5);

impl Finalizer {
    /// Create a new `Finalizer`.
    pub fn new(pgpool: PgPool) -> Self {
        Self { pgpool }
    }

    /// `Finalizer` main loop.
    pub async fn run<M>(self, middleware: M) -> Result<()>
    where
        M: ZksyncMiddleware,
    {
        loop {
            let newly_executed_withdrawals =
                storage::get_withdrawals_with_no_data(&self.pgpool, 0, 50).await?;

            if newly_executed_withdrawals.is_empty() {
                tokio::time::sleep(NO_NEW_WITHDRAWALS_BACKOFF).await;
                continue;
            }

            vlog::info!("newly executed withdrawals {newly_executed_withdrawals:?}");

            let hash_and_index: Vec<_> = newly_executed_withdrawals
                .iter()
                .map(|p| (p.0, p.1))
                .collect();

            let params: Vec<_> = self
                .request_finalize_params(&middleware, &hash_and_index)
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

            vlog::info!("params for withdrawals {params:?}");

            storage::add_withdrawals_data(&self.pgpool, &params).await?;
        }
    }

    async fn request_finalize_params<M>(
        &self,
        middleware: M,
        hash_and_indices: &[(H256, u16)],
    ) -> Result<Vec<WithdrawalParams>>
    where
        M: ZksyncMiddleware,
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
}
