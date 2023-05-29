use std::sync::Arc;

use ethers::providers::{JsonRpcClient, Middleware, Provider};
use futures::{stream::StreamExt, Stream};
use sqlx::PgConnection;
use tokio::pin;

use client::{get_l1_batch_block_range, BlockEvent, WithdrawalEvent};

use crate::Result;

pub struct WithdrawalFinalizer<M> {
    l2_provider: Arc<Provider<M>>,
    pgpool: PgConnection,
}

impl<M> WithdrawalFinalizer<M>
where
    M: JsonRpcClient,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(l2_provider: Arc<Provider<M>>, pgpool: PgConnection) -> Self {
        Self {
            l2_provider,
            pgpool,
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
        WE: Stream<Item = WithdrawalEvent>,
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
                    log::info!("block event {event}");
                    self.process_block_event(event).await?;
                }
                Some(event) = withdrawal_events.next() => {
                    log::info!("withdrawal event {event:?}");
                    if event.block_number > curr_l2_block_number {
                        self.process_withdrawals_in_block(std::mem::take(&mut in_block_events)).await?;
                        curr_l2_block_number = event.block_number;
                    }
                    in_block_events.push(event);
                }
                else => {
                    log::info!("terminating finalizer");
                    break
                }
            }
        }

        Ok(())
    }

    async fn process_block_event(&mut self, event: BlockEvent) -> Result<()> {
        match event {
            BlockEvent::BlockCommit {
                block_number,
                event,
            } => {
                if let Some((range_begin, range_end)) = get_l1_batch_block_range(
                    &self.l2_provider.provider().as_ref(),
                    event.block_number.as_u64() as u32,
                )
                .await?
                {
                    storage::committed_new_batch(
                        &mut self.pgpool,
                        range_begin.as_u64(),
                        range_end.as_u64(),
                        block_number,
                    )
                    .await?;

                    log::info!(
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

                let range_begin = get_l1_batch_block_range(
                    &self.l2_provider.provider().as_ref(),
                    current_first_verified_batch as u32,
                )
                .await?
                .map(|range| range.0.as_u64());

                let range_end = get_l1_batch_block_range(
                    &self.l2_provider.provider().as_ref(),
                    current_last_verified_batch as u32,
                )
                .await?
                .map(|range| range.1.as_u64());

                if let (Some(range_begin), Some(range_end)) = (range_begin, range_end) {
                    storage::verified_new_batch(
                        &mut self.pgpool,
                        range_begin,
                        range_end,
                        block_number,
                    )
                    .await?;
                    log::info!(
                        "Changed withdrawals status to verified for range {range_begin}-{range_end}"
                    );
                } else {
                    log::warn!(
                        "One of the verified ranges not found: {range_begin:?}, {range_end:?}"
                    );
                }
            }
            BlockEvent::BlockExecution {
                block_number,
                event,
            } => {
                if let Some((range_begin, range_end)) = get_l1_batch_block_range(
                    &self.l2_provider.provider().as_ref(),
                    event.block_number.as_u64() as u32,
                )
                .await?
                {
                    storage::executed_new_batch(
                        &mut self.pgpool,
                        range_begin.as_u64(),
                        range_end.as_u64(),
                        block_number,
                    )
                    .await?;

                    log::info!(
                        "Changed withdrawals status to executed for range {range_begin}-{range_end}"
                    );
                }
            }
            BlockEvent::BlocksRevert { .. } => todo!(),
        }

        Ok(())
    }

    async fn process_withdrawals_in_block(&mut self, events: Vec<WithdrawalEvent>) -> Result<()> {
        use itertools::Itertools;

        let group_by = events.into_iter().group_by(|event| event.tx_hash);

        let mut withdrawals_vec = vec![];
        for (_tx_hash, group) in group_by.into_iter() {
            for (index, event) in group.into_iter().enumerate() {
                log::info!("withdrawal {event:?} index in transaction is {index}");
                withdrawals_vec.push((event, index));
            }
        }
        storage::add_withdrawals(&mut self.pgpool, &withdrawals_vec).await?;
        Ok(())
    }
}
