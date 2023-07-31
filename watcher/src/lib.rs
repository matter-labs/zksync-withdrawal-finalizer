use std::sync::Arc;

use chain_events::L2Event;
use ethers::providers::{JsonRpcClient, Middleware};
use futures::{stream::StreamExt, Stream};
use sqlx::PgPool;
use storage::StoredWithdrawal;
use tokio::pin;

use client::{zksync_contract::L2ToL1Event, BlockEvent, WithdrawalEvent, ZksyncMiddleware};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    PgError(#[from] sqlx::Error),

    #[error(transparent)]
    StorageError(#[from] storage::Error),

    #[error(transparent)]
    ClientError(#[from] client::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub struct Watcher<M2> {
    l2_provider: Arc<M2>,
    pgpool: PgPool,
}

impl<M2> Watcher<M2>
where
    M2: ZksyncMiddleware + 'static,
    <M2 as Middleware>::Provider: JsonRpcClient,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(l2_provider: Arc<M2>, pgpool: PgPool) -> Self {
        Self {
            l2_provider,
            pgpool,
        }
    }

    pub async fn run<BE, WE>(
        self,
        block_events: BE,
        withdrawal_events: WE,
        from_l2_block: u64,
    ) -> Result<()>
    where
        BE: Stream<Item = BlockEvent> + Send + 'static,
        WE: Stream<Item = L2Event> + Send + 'static,
    {
        let Watcher {
            l2_provider,
            pgpool,
        } = self;

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

        let l1_loop_handler = tokio::spawn(run_l1_events_loop(
            pgpool.clone(),
            block_events,
            l2_provider,
        ));
        let l2_loop_handler =
            tokio::spawn(run_l2_events_loop(pgpool, withdrawal_events, from_l2_block));

        pin!(l1_loop_handler);
        pin!(l2_loop_handler);
        tokio::select! {
            l1 = l1_loop_handler => {
                vlog::error!("watcher l1 loop ended with {l1:?}");
                l1.unwrap()?;
            }
            l2 = l2_loop_handler => {
                vlog::error!("watcher l2 loop ended with {l2:?}");
                l2.unwrap()?;
            }
        }

        Ok(())
    }
}

async fn process_l2_to_l1_events(pool: &PgPool, events: Vec<L2ToL1Event>) -> Result<()> {
    storage::l2_to_l1_events(pool, &events).await?;

    Ok(())
}

async fn process_block_event<M2>(pool: &PgPool, event: BlockEvent, l2_middleware: M2) -> Result<()>
where
    M2: ZksyncMiddleware,
{
    match event {
        BlockEvent::BlockCommit {
            block_number,
            event,
        } => {
            if let Some((range_begin, range_end)) = l2_middleware
                .get_l1_batch_block_range(event.block_number.as_u64() as u32)
                .await?
            {
                metrics::gauge!("watcher.l2_last_committed_block", range_end.as_u64() as f64);

                storage::committed_new_batch(
                    pool,
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
            let range_begin = l2_middleware
                .get_l1_batch_block_range(current_first_verified_batch as u32)
                .await?
                .map(|range| range.0.as_u64());

            let range_end = l2_middleware
                .get_l1_batch_block_range(current_last_verified_batch as u32)
                .await?
                .map(|range| range.1.as_u64());

            if let (Some(range_begin), Some(range_end)) = (range_begin, range_end) {
                metrics::gauge!("watcher.l2_last_verified_block", range_end as f64);
                storage::verified_new_batch(pool, range_begin, range_end, block_number).await?;
                vlog::info!(
                    "Changed withdrawals status to verified for range {range_begin}-{range_end}"
                );
            } else {
                vlog::warn!("One of the verified ranges not found: {range_begin:?}, {range_end:?}");
            }
        }
        BlockEvent::BlockExecution {
            block_number,
            event,
        } => {
            if let Some((range_begin, range_end)) = l2_middleware
                .get_l1_batch_block_range(event.block_number.as_u64() as u32)
                .await?
            {
                metrics::gauge!("watcher.l2_last_executed_block", range_end.as_u64() as f64);

                storage::executed_new_batch(
                    pool,
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
        BlockEvent::L2ToL1Events { events } => process_l2_to_l1_events(pool, events).await?,
    }

    Ok(())
}

async fn process_withdrawals_in_block(pool: &PgPool, events: Vec<WithdrawalEvent>) -> Result<()> {
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
        stored_withdrawals.push(StoredWithdrawal {
            event,
            index_in_tx: index,
        });
    }

    storage::add_withdrawals(pool, &stored_withdrawals).await?;
    Ok(())
}

async fn run_l1_events_loop<BE, M2>(pool: PgPool, be: BE, l2_middleware: M2) -> Result<()>
where
    BE: Stream<Item = BlockEvent>,
    M2: ZksyncMiddleware,
{
    pin!(be);

    while let Some(event) = be.next().await {
        vlog::debug!("block event {event}");
        process_block_event(&pool, event, &l2_middleware).await?;
    }

    Ok(())
}

async fn run_l2_events_loop<WE>(pool: PgPool, we: WE, from_l2_block: u64) -> Result<()>
where
    WE: Stream<Item = L2Event>,
{
    pin!(we);

    let mut curr_l2_block_number = from_l2_block;
    let mut in_block_events = vec![];
    while let Some(event) = we.next().await {
        match event {
            L2Event::Withdrawal(event) => {
                vlog::debug!("withdrawal event {event:?}");
                if event.block_number > curr_l2_block_number {
                    process_withdrawals_in_block(&pool, std::mem::take(&mut in_block_events))
                        .await?;
                    curr_l2_block_number = event.block_number;
                }
                in_block_events.push(event);
            }
            L2Event::L2TokenInitEvent(event) => {
                vlog::debug!("l2 token init event {event:?}");
                storage::add_token(&pool, &event).await?;
            }
            L2Event::RestartedFromBlock(block_number) => {
                // The event producer has been restarted at a given
                // block height. It is going to re-send all events
                // from that block number up. To avoid duplications
                // already received events of that height and above
                // have to be removed from the accumulator.
                in_block_events.retain(|event| event.block_number < block_number)
            }
        }
    }

    Ok(())
}
