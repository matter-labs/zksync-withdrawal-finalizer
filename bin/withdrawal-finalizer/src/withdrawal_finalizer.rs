use std::{collections::HashMap, sync::Arc};

use ethers::{
    providers::{JsonRpcClient, Middleware, Provider},
    types::H256,
};
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

        let mut curr_block_number = from_l2_block;

        // While reading the stream of withdrawal events asyncronously
        // we may never be sure that we are currenly looking at the last
        // event from the given block.
        let mut in_block_events = vec![];

        loop {
            tokio::select! {
                Some(event) = block_events.next() => {
                    log::info!("block event {event}");
                    self.process_block_event(event).await?;
                }
                Some(event) = withdrawal_events.next() => {
                    log::info!("withdrawal event {event:?}");
                    if event.block_number > curr_block_number {
                        self.process_withdrawals_in_block(&mut in_block_events).await;
                        curr_block_number = event.block_number;
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
                        event.block_number.as_u64(),
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
                if let Some((range_begin, range_end)) = get_l1_batch_block_range(
                    &self.l2_provider.provider().as_ref(),
                    event.current_last_verified_block.as_u64() as u32,
                )
                .await?
                {
                    storage::verified_new_batch(
                        &mut self.pgpool,
                        range_begin.as_u64(),
                        range_end.as_u64(),
                        block_number,
                        event.previous_last_verified_block.as_u64(),
                        event.current_last_verified_block.as_u64(),
                    )
                    .await?;
                    log::info!(
                        "Changed withdrawals status to verified for range {range_begin}-{range_end}"
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
                        event.block_number.as_u64(),
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

    async fn process_withdrawals_in_block(&mut self, events: &mut Vec<WithdrawalEvent>) {
        let mut numbers_in_blocks: HashMap<H256, usize> = HashMap::new();

        for event in events.drain(..) {
            let r = numbers_in_blocks.entry(event.tx_hash).or_default();

            let index = *r;

            *r += 1;

            log::info!("withdrawal {event:?} index in transaction is {index}");
            storage::add_withdrawal(&mut self.pgpool, &event, index)
                .await
                .unwrap();
        }
    }
}
