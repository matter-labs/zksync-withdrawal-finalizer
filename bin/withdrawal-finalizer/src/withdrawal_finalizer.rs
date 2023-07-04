use std::sync::Arc;

use chain_events::L2Event;
use ethers::{
    providers::{JsonRpcClient, Middleware},
    types::{Address, H256},
};
use futures::{stream::StreamExt, Stream};
use sqlx::PgPool;
use storage::StoredWithdrawal;
use tokio::pin;

use client::{
    l1bridge::codegen::IL1Bridge,
    zksync_contract::{codegen::IZkSync, L2ToL1Event},
    BlockEvent, WithdrawalEvent, ZksyncMiddleware,
};

use crate::Result;

pub struct WithdrawalFinalizer<M1, M2> {
    l2_provider: Arc<M2>,
    pgpool: PgPool,
    l1_bridge: IL1Bridge<M1>,
    zksync_contract: IZkSync<M1>,
}

impl<M1, M2> WithdrawalFinalizer<M1, M2>
where
    M1: Middleware,
    <M1 as Middleware>::Provider: JsonRpcClient,
    M2: ZksyncMiddleware,
    <M2 as Middleware>::Provider: JsonRpcClient,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        l2_provider: Arc<M2>,
        pgpool: PgPool,
        zksync_contract: IZkSync<M1>,
        l1_bridge: IL1Bridge<M1>,
    ) -> Self {
        Self {
            l2_provider,
            pgpool,
            zksync_contract,
            l1_bridge,
        }
    }

    pub async fn run<BE, WE>(
        mut self,
        block_events: BE,
        withdrawal_events: WE,
        from_l2_block: u64,
    ) -> Result<()>
    where
        BE: Stream<Item = BlockEvent>,
        WE: Stream<Item = L2Event>,
    {
        pin!(block_events);
        pin!(withdrawal_events);

        let mut curr_l2_block_number = from_l2_block;

        // While reading the stream of withdrawal events asyncronously
        // we may never be sure that we are currenly looking at the last
        // event from the given block.
        //
        // The following code asyncronously reads and accumulates events
        // that happened within the single block (the exact number is tracked in
        // `curr_block_number`) until it sees an event with a higher block number.
        // Then the following vector is drained and all events within it are written
        // into storage.
        //
        // TODO: investigate instead subscribing to whole blocks via `subcscribe_blocks()`
        // method and pasring and sending all events at once so that this function WE type
        // would change to `Stream<Vec<WithdrawalEvent>>` to handle a vector of all withdrawal
        // events that have happened within a single block.
        let mut in_block_events = vec![];

        loop {
            tokio::select! {
                Some(event) = block_events.next() => {
                    vlog::info!("block event {event}");
                    self.process_block_event(event).await?;
                }
                Some(event) = withdrawal_events.next() => {
                    match event {
                        L2Event::Withdrawal(event) => {
                            vlog::info!("withdrawal event {event:?}");
                            if event.block_number > curr_l2_block_number {
                                self.process_withdrawals_in_block(std::mem::take(&mut in_block_events)).await?;
                                curr_l2_block_number = event.block_number;
                            }
                            in_block_events.push(event);

                        }
                        L2Event::L2TokenInitEvent(event) => storage::add_token(&self.pgpool, &event).await?,
                    }
                }
                else => {
                    vlog::info!("terminating finalizer");
                    break
                }
            }
        }

        Ok(())
    }

    async fn process_l2_to_l1_events(&mut self, events: Vec<L2ToL1Event>) -> Result<()> {
        let mut pgconn = self.pgpool.acquire().await?;

        storage::l2_to_l1_events(&mut pgconn, &events).await?;

        Ok(())
    }

    async fn process_block_event(&mut self, event: BlockEvent) -> Result<()> {
        let mut pgconn = self.pgpool.acquire().await?;
        match event {
            BlockEvent::BlockCommit {
                block_number,
                event,
            } => {
                if let Some((range_begin, range_end)) = self
                    .l2_provider
                    .get_l1_batch_block_range(event.block_number.as_u64() as u32)
                    .await?
                {
                    metrics::gauge!("watcher.l2_last_committed_block", range_end.as_u64() as f64);

                    storage::committed_new_batch(
                        &mut pgconn,
                        range_begin.as_u64(),
                        range_end.as_u64(),
                        block_number,
                    )
                    .await?;

                    vlog::info!(
                        "Changed withdrawals status to committed for range {range_begin}-{range_end}"
                    );
                }
            }
            BlockEvent::BlocksVerification {
                block_number,
                event,
            } => {
                let current_first_verified_batch = event.previous_last_verified_block.as_u64() + 1;
                let current_last_verified_batch = event.current_last_verified_block.as_u64();
                let range_begin = self
                    .l2_provider
                    .get_l1_batch_block_range(current_first_verified_batch as u32)
                    .await?
                    .map(|range| range.0.as_u64());

                let range_end = self
                    .l2_provider
                    .get_l1_batch_block_range(current_last_verified_batch as u32)
                    .await?
                    .map(|range| range.1.as_u64());

                if let (Some(range_begin), Some(range_end)) = (range_begin, range_end) {
                    metrics::gauge!("watcher.l2_last_verified_block", range_end as f64);
                    storage::verified_new_batch(&mut pgconn, range_begin, range_end, block_number)
                        .await?;
                    vlog::info!(
                        "Changed withdrawals status to verified for range {range_begin}-{range_end}"
                    );
                } else {
                    vlog::warn!(
                        "One of the verified ranges not found: {range_begin:?}, {range_end:?}"
                    );
                }
            }
            BlockEvent::BlockExecution {
                block_number,
                event,
            } => {
                if let Some((range_begin, range_end)) = self
                    .l2_provider
                    .get_l1_batch_block_range(event.block_number.as_u64() as u32)
                    .await?
                {
                    metrics::gauge!("watcher.l2_last_executed_block", range_end.as_u64() as f64);

                    storage::executed_new_batch(
                        &mut pgconn,
                        range_begin.as_u64(),
                        range_end.as_u64(),
                        block_number,
                    )
                    .await?;

                    vlog::info!(
                        "Changed withdrawals status to executed for range {range_begin}-{range_end}"
                    );
                }
            }
            BlockEvent::BlocksRevert { .. } => todo!(),
            BlockEvent::L2ToL1Events { events } => self.process_l2_to_l1_events(events).await?,
        }

        Ok(())
    }

    async fn process_withdrawals_in_block(&mut self, events: Vec<WithdrawalEvent>) -> Result<()> {
        use itertools::Itertools;

        let group_by = events.into_iter().group_by(|event| event.tx_hash);

        let mut withdrawals_vec = vec![];
        for (_tx_hash, group) in group_by.into_iter() {
            for (index, event) in group.into_iter().enumerate() {
                metrics::gauge!("watcher.l2_last_seen_block", event.block_number as f64);
                vlog::info!("withdrawal {event:?} index in transaction is {index}");

                withdrawals_vec.push((event, index));
            }
        }

        let mut stored_withdrawals = vec![];

        for (event, index) in withdrawals_vec.into_iter() {
            let is_finalized = self
                .is_withdrawal_finalized(event.tx_hash, index, event.token)
                .await?;

            stored_withdrawals.push(StoredWithdrawal {
                event,
                index_in_tx: index,
                is_finalized,
            });
        }

        let mut pgconn = self.pgpool.acquire().await?;
        storage::add_withdrawals(&mut pgconn, &stored_withdrawals).await?;
        Ok(())
    }

    async fn is_withdrawal_finalized(
        &self,
        withdrawal_hash: H256,
        index: usize,
        sender: Address,
    ) -> Result<bool> {
        Ok(client::is_withdrawal_finalized(
            withdrawal_hash,
            index,
            sender,
            &self.zksync_contract,
            &self.l1_bridge,
            &self.l2_provider,
        )
        .await?)
    }
}
