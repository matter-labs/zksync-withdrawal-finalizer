use std::time::Duration;

use ethers::providers::{JsonRpcClient, Middleware};
use sqlx::PgPool;
use tokio::time::sleep;

use client::{l1bridge::L1Bridge, zksync_contract::ZkSync};
use storage::update_withdrawals_to_finalized;

use crate::Result;

const UPDATER_BACKOFF: Duration = Duration::from_secs(5);

pub async fn run<M1, M2>(
    pool: PgPool,
    zksync_contract: ZkSync<M1>,
    l1_bridge: L1Bridge<M1>,
    l2_middleware: M2,
) -> Result<()>
where
    M1: Clone + Middleware,
    <M1 as Middleware>::Provider: JsonRpcClient,
    M2: Middleware,
    <M2 as Middleware>::Provider: JsonRpcClient,
{
    let mut conn = pool.acquire().await?;

    log::warn!("running updater");
    loop {
        sleep(UPDATER_BACKOFF).await;

        let unfinalized_withdrawals = storage::unfinalized_withdrawals(&mut conn).await?;
        log::warn!("tick");

        let mut tx_hashes_and_indices_in_tx = Vec::with_capacity(unfinalized_withdrawals.len());
        for withdrawal in unfinalized_withdrawals.into_iter() {
            if crate::withdrawal_finalizer::is_withdrawal_finalized(
                withdrawal.event.tx_hash,
                withdrawal.index_in_tx,
                withdrawal.event.token,
                &zksync_contract,
                &l1_bridge,
                &l2_middleware,
            )
            .await?
            {
                log::warn!(
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
