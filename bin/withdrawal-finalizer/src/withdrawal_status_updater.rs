use std::time::Duration;

use ethers::providers::{JsonRpcClient, Middleware};
use sqlx::PgPool;
use tokio::time::sleep;

use client::{l1bridge::codegen::IL1Bridge, zksync_contract::codegen::IZkSync, ZksyncMiddleware};
use storage::update_withdrawals_to_finalized;

use crate::Result;

const DEFAULT_UPDATER_BACKOFF: u64 = 5;

pub async fn run<M1, M2>(
    pool: PgPool,
    zksync_contract: IZkSync<M1>,
    l1_bridge: IL1Bridge<M1>,
    l2_middleware: M2,
    backoff: Option<u64>,
) -> Result<()>
where
    M1: Clone + Middleware,
    <M1 as Middleware>::Provider: JsonRpcClient,
    M2: ZksyncMiddleware,
    <M2 as Middleware>::Provider: JsonRpcClient,
{
    let mut conn = pool.acquire().await?;
    let backoff = Duration::from_secs(backoff.unwrap_or(DEFAULT_UPDATER_BACKOFF));

    loop {
        sleep(backoff).await;

        let unfinalized_withdrawals = storage::unfinalized_withdrawals(&mut conn).await?;

        let mut tx_hashes_and_indices_in_tx = Vec::with_capacity(unfinalized_withdrawals.len());
        for withdrawal in unfinalized_withdrawals.into_iter() {
            if client::is_withdrawal_finalized(
                withdrawal.event.tx_hash,
                withdrawal.index_in_tx,
                withdrawal.event.token,
                &zksync_contract,
                &l1_bridge,
                &l2_middleware,
            )
            .await?
            {
                vlog::debug!(
                    "withdrawal {} with index in tx {} became finalized",
                    withdrawal.event.tx_hash,
                    withdrawal.index_in_tx
                );
                tx_hashes_and_indices_in_tx
                    .push((withdrawal.event.tx_hash, withdrawal.index_in_tx));
            }
        }
        if !tx_hashes_and_indices_in_tx.is_empty() {
            update_withdrawals_to_finalized(&mut conn, &tx_hashes_and_indices_in_tx).await?;
        }
    }
}
