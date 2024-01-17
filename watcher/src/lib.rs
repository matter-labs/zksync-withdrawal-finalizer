use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use chain_events::L2Event;
use ethers::providers::{JsonRpcClient, Middleware};
use futures::{stream::StreamExt, Stream};
use sqlx::PgPool;
use storage::StoredWithdrawal;
use tokio::pin;

use client::{zksync_contract::L2ToL1Event, BlockEvent, WithdrawalEvent, ZksyncMiddleware};
use withdrawals_meterer::{MeteringComponent, WithdrawalsMeter};

use crate::metrics::WATCHER_METRICS;

mod metrics;

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
    withdrawals_meterer: Option<WithdrawalsMeter>,
}

impl<M2> Watcher<M2>
where
    M2: ZksyncMiddleware + 'static,
    <M2 as Middleware>::Provider: JsonRpcClient,
{
    pub fn new(l2_provider: Arc<M2>, pgpool: PgPool, meter_withdrawals: bool) -> Self {
        let withdrawals_meterer = meter_withdrawals.then_some(WithdrawalsMeter::new(
            pgpool.clone(),
            MeteringComponent::RequestedWithdrawals,
        ));

        Self {
            l2_provider,
            pgpool,
            withdrawals_meterer,
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
            withdrawals_meterer,
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
        let l2_loop_handler = tokio::spawn(async move {
            run_l2_events_loop(
                pgpool,
                withdrawal_events,
                from_l2_block,
                withdrawals_meterer,
            )
            .await
        });

        pin!(l1_loop_handler);
        pin!(l2_loop_handler);
        tokio::select! {
            l1 = l1_loop_handler => {
                tracing::error!("watcher l1 loop ended with {l1:?}");
                l1.unwrap()?;
            }
            l2 = l2_loop_handler => {
                tracing::error!("watcher l2 loop ended with {l2:?}");
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

enum BlockRangesParams {
    Commit {
        range_begin: u64,
        range_end: u64,
        block_number: u64,
    },
    Verify {
        range_begin: u64,
        range_end: u64,
        block_number: u64,
    },
    Execute {
        range_begin: u64,
        range_end: u64,
        block_number: u64,
    },
    L2ToL1Events {
        events: Vec<L2ToL1Event>,
    },
}

impl BlockRangesParams {
    async fn write_to_storage(self, pool: &PgPool) -> Result<()> {
        match self {
            BlockRangesParams::Commit {
                range_begin,
                range_end,
                block_number,
            } => {
                storage::committed_new_batch(pool, range_begin, range_end, block_number).await?;

                tracing::info!(
                    "Changed withdrawals status to committed for range {range_begin}-{range_end}"
                );
            }
            BlockRangesParams::Verify {
                range_begin,
                range_end,
                block_number,
            } => {
                storage::verified_new_batch(pool, range_begin, range_end, block_number).await?;
                tracing::info!(
                    "Changed withdrawals status to verified for range {range_begin}-{range_end}"
                );
            }
            BlockRangesParams::Execute {
                range_begin,
                range_end,
                block_number,
            } => {
                storage::executed_new_batch(pool, range_begin, range_end, block_number).await?;

                tracing::info!(
                    "Changed withdrawals status to executed for range {range_begin}-{range_end}"
                );
            }
            BlockRangesParams::L2ToL1Events { events } => {
                process_l2_to_l1_events(pool, events).await?;
            }
        }
        Ok(())
    }
}

async fn request_block_ranges<M2>(
    event: BlockEvent,
    l2_middleware: M2,
) -> Result<Option<BlockRangesParams>>
where
    M2: ZksyncMiddleware,
{
    match event {
        BlockEvent::BlockCommit {
            block_number,
            event,
        } => {
            if let Some((range_begin, range_end)) = l2_middleware
                .get_l1_batch_block_range(event.batch_number.as_u64() as u32)
                .await?
            {
                WATCHER_METRICS
                    .l2_last_committed_block
                    .set(range_end.as_u64() as i64);
                Ok(Some(BlockRangesParams::Commit {
                    range_begin: range_begin.as_u64(),
                    range_end: range_end.as_u64(),
                    block_number,
                }))
            } else {
                Ok(None)
            }
        }
        BlockEvent::BlocksVerification {
            block_number,
            event,
        } => {
            let current_first_verified_batch = event.previous_last_verified_batch.as_u64() + 1;
            let current_last_verified_batch = event.current_last_verified_batch.as_u64();
            let range_begin = l2_middleware
                .get_l1_batch_block_range(current_first_verified_batch as u32)
                .await?
                .map(|range| range.0.as_u64());

            let range_end = l2_middleware
                .get_l1_batch_block_range(current_last_verified_batch as u32)
                .await?
                .map(|range| range.1.as_u64());

            if let (Some(range_begin), Some(range_end)) = (range_begin, range_end) {
                WATCHER_METRICS.l2_last_verified_block.set(range_end as i64);
                Ok(Some(BlockRangesParams::Verify {
                    range_begin,
                    range_end,
                    block_number,
                }))
            } else {
                tracing::warn!(
                    "One of the verified ranges not found: {range_begin:?}, {range_end:?}"
                );
                Ok(None)
            }
        }
        BlockEvent::BlockExecution {
            block_number,
            event,
        } => {
            if let Some((range_begin, range_end)) = l2_middleware
                .get_l1_batch_block_range(event.batch_number.as_u64() as u32)
                .await?
            {
                WATCHER_METRICS
                    .l2_last_executed_block
                    .set(range_end.as_u64() as i64);
                Ok(Some(BlockRangesParams::Execute {
                    range_begin: range_begin.as_u64(),
                    range_end: range_end.as_u64(),
                    block_number,
                }))
            } else {
                Ok(None)
            }
        }
        BlockEvent::BlocksRevert { .. } => {
            tracing::error!("Received a blocks revert event: {event:?}");
            Ok(None)
        }
        BlockEvent::L2ToL1Events { events } => Ok(Some(BlockRangesParams::L2ToL1Events { events })),
    }
}

async fn process_block_events<M2>(
    pool: &PgPool,
    events: Vec<BlockEvent>,
    l2_middleware: M2,
) -> Result<()>
where
    M2: ZksyncMiddleware,
{
    let results: Result<Vec<_>> = futures::future::join_all(
        events
            .into_iter()
            .map(|event| request_block_ranges(event, &l2_middleware)),
    )
    .await
    .into_iter()
    .collect();

    let results = results?;

    for result in results.into_iter().flatten() {
        result.write_to_storage(pool).await?;
    }

    Ok(())
}

async fn process_withdrawals_in_block(
    pool: &PgPool,
    events: Vec<WithdrawalEvent>,
    withdrawals_meterer: &mut Option<WithdrawalsMeter>,
) -> Result<()> {
    use itertools::Itertools;
    let group_by = events.into_iter().group_by(|event| event.tx_hash);
    let mut withdrawals_vec = vec![];

    for (_tx_hash, group) in group_by.into_iter() {
        for (index, event) in group.into_iter().enumerate() {
            WATCHER_METRICS
                .l2_last_seen_block
                .set(event.block_number as i64);
            tracing::info!("withdrawal {event:?} index in transaction is {index}");

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

    if let Some(ref mut withdrawals_meterer) = withdrawals_meterer {
        if let Err(e) = withdrawals_meterer
            .meter_withdrawals(&stored_withdrawals)
            .await
        {
            tracing::error!("Failed to meter requested withdrawals: {e}");
        }
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

    let mut block_event_batch = vec![];
    let mut batch_begin = Instant::now();
    let batch_backoff = Duration::from_secs(5);
    let batch_size = 1024;

    while let Some(event) = be.next().await {
        tracing::debug!("block event {event}");
        block_event_batch.push(event);

        if block_event_batch.len() >= batch_size || batch_begin.elapsed() > batch_backoff {
            tracing::debug!("processing batch of l1 events {}", block_event_batch.len());

            process_block_events(
                &pool,
                std::mem::take(&mut block_event_batch),
                &l2_middleware,
            )
            .await?;

            batch_begin = Instant::now();
        }
    }

    Ok(())
}

async fn run_l2_events_loop<WE>(
    pool: PgPool,
    we: WE,
    from_l2_block: u64,
    mut withdrawals_meterer: Option<WithdrawalsMeter>,
) -> Result<()>
where
    WE: Stream<Item = L2Event>,
{
    pin!(we);

    let mut curr_l2_block_number = from_l2_block;
    let mut in_block_events = vec![];
    while let Some(event) = we.next().await {
        match event {
            L2Event::Withdrawal(event) => {
                tracing::info!("received withdrawal event {event:?}");
                if event.block_number > curr_l2_block_number {
                    process_withdrawals_in_block(
                        &pool,
                        std::mem::take(&mut in_block_events),
                        &mut withdrawals_meterer,
                    )
                    .await?;
                    curr_l2_block_number = event.block_number;
                }
                in_block_events.push(event);
            }
            L2Event::L2TokenInitEvent(event) => {
                tracing::debug!("l2 token init event {event:?}");
                storage::add_token(&pool, &event).await?;
            }
            L2Event::RestartedFromBlock(_block_number) => {
                // The event producer has been restarted at a given
                // block height. It is going to re-send all events
                // from that block number up. However the already received
                // events need to be processed because they may never be sent again.
                //
                // Consider the situation where events at following block already sit in the
                // accumulator:
                // `[1042]`.
                //
                // The producer is restarted from block `1045`, and as such event at `1042`
                // will never be re-sent.
                process_withdrawals_in_block(
                    &pool,
                    std::mem::take(&mut in_block_events),
                    &mut withdrawals_meterer,
                )
                .await?;
            }
        }
    }

    Ok(())
}
