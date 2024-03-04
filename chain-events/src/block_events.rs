use std::sync::Arc;

use ethers::{
    abi::{Address, RawLog},
    contract::EthEvent,
    prelude::EthLogDecode,
    providers::{Middleware, Provider, PubsubClient, Ws},
    types::{BlockNumber, Filter, Log, ValueOrArray},
};
use futures::{Sink, SinkExt, StreamExt};

use client::{
    zksync_contract::codegen::{BlockCommitFilter, BlockExecutionFilter, BlocksVerificationFilter},
    BlockEvent,
};
use ethers_log_decode::EthLogDecode;

use crate::{metrics::CHAIN_EVENTS_METRICS, Error, Result, RECONNECT_BACKOFF};

#[derive(EthLogDecode)]
enum L1Events {
    BlockCommit(BlockCommitFilter),
    BlocksVerification(BlocksVerificationFilter),
    BlocksExecution(BlockExecutionFilter),
}

// A convenience multiplexer for `Block`-related events.
//
// The only purose of this structure is multpliexing together
// the `Block` events from the middleware as currently `ethers` events
// api relies on lifetimes and borrowing is hard to use otherwise
// in the async context.
/// Listener of block events on L1.
pub struct BlockEvents {
    url: String,
}

impl BlockEvents {
    /// Creates a new `BlockEvents` structure
    ///
    /// # Arguments
    ///
    /// * `middleware`: The middleware to perform requests with.
    pub fn new(url: &str) -> BlockEvents {
        Self {
            url: url.to_string(),
        }
    }

    async fn connect(&self) -> Option<Provider<Ws>> {
        match Provider::<Ws>::connect_with_reconnects(&self.url, 0).await {
            Ok(p) => {
                CHAIN_EVENTS_METRICS.successful_l1_reconnects.inc();
                Some(p)
            }
            Err(e) => {
                tracing::warn!("Block events stream reconnect attempt failed: {e}");
                CHAIN_EVENTS_METRICS.l1_reconnects_on_error.inc();
                None
            }
        }
    }

    /// Run the main loop with re-connecting on websocket disconnects
    //
    // Websocket subscriptions do not work well with reconnections
    // in `ethers-rs`: https://github.com/gakonst/ethers-rs/issues/2418
    // This function is a workaround for that and implements manual re-connecting.
    pub async fn run_with_reconnects<B, S>(
        self,
        diamond_proxy_addr: Address,
        l2_erc20_bridge_addr: Address,
        from_block: B,
        sender: S,
    ) -> Result<()>
    where
        B: Into<BlockNumber> + Copy,
        S: Sink<BlockEvent> + Unpin + Clone,
        <S as Sink<BlockEvent>>::Error: std::fmt::Debug,
    {
        let mut from_block: BlockNumber = from_block.into();

        loop {
            let Some(provider_l1) = self.connect().await else {
                tokio::time::sleep(RECONNECT_BACKOFF).await;
                continue;
            };

            let middleware = Arc::new(provider_l1);

            match Self::run(
                diamond_proxy_addr,
                l2_erc20_bridge_addr,
                from_block,
                sender.clone(),
                middleware,
            )
            .await
            {
                Err(e) => {
                    tracing::warn!("Block events worker failed with {e}");
                }
                Ok(block) => from_block = block,
            }
        }
    }
}

impl BlockEvents {
    /// A convenience function that listens for all `Block`-related and sends them to the user.
    ///
    /// `ethers` APIs have two approaches to querying events from chain:
    ///   1. Listen to *all* types of events (will generate a less-performant code)
    ///   2. Listen to a *single* type of event
    ///
    /// The purpose of this function is two wrap the second approach
    /// and conveniently run a background loop for it sending all
    /// needed events to the user.
    ///
    /// This implementation is the only possible since `ethers` async
    /// APIs heavily rely on `&self` and what is worse on `&self`
    /// lifetimes making it practically impossible to decouple
    /// `Event` and `EventStream` types from each other.
    async fn run<B, S, M>(
        diamond_proxy_addr: Address,
        l2_erc20_bridge_addr: Address,
        from_block: B,
        mut sender: S,
        middleware: M,
    ) -> Result<BlockNumber>
    where
        B: Into<BlockNumber> + Copy,
        M: Middleware,
        <M as Middleware>::Provider: PubsubClient,
        S: Sink<BlockEvent> + Unpin,
        <S as Sink<BlockEvent>>::Error: std::fmt::Debug,
    {
        let mut last_seen_block: BlockNumber = from_block.into();
        let latest_block = middleware
            .get_block(BlockNumber::Latest)
            .await
            .map_err(|e| Error::Middleware(e.to_string()))?
            .expect("last block number always exists in a live network; qed")
            .number
            .expect("last block always has a number; qed");

        tracing::info!(
            "Filtering logs from {} to {}",
            from_block
                .into()
                .as_number()
                .expect("always starting from a numbered block; qed")
                .as_u64(),
            latest_block.as_u64(),
        );

        let past_filter = Filter::new()
            .from_block(from_block)
            .to_block(latest_block)
            .address(diamond_proxy_addr)
            .topic0(vec![
                BlockCommitFilter::signature(),
                BlocksVerificationFilter::signature(),
                BlockExecutionFilter::signature(),
            ]);

        let filter = Filter::new()
            .from_block(latest_block)
            .address(Into::<ValueOrArray<Address>>::into(diamond_proxy_addr))
            .topic0(vec![
                BlockCommitFilter::signature(),
                BlocksVerificationFilter::signature(),
                BlockExecutionFilter::signature(),
            ]);

        let past_logs = middleware.get_logs_paginated(&past_filter, 256);
        let current_logs = middleware
            .subscribe_logs(&filter)
            .await
            .map_err(|e| Error::Middleware(e.to_string()))?;

        let mut logs = past_logs.chain(current_logs.map(Ok));

        while let Some(log) = logs.next().await {
            let log = match log {
                Err(e) => {
                    tracing::warn!("L1 block events stream ended with {e}");
                    break;
                }
                Ok(log) => log,
            };
            let Some(block_number) = log.block_number.map(|bn| bn.as_u64()) else {
                continue;
            };

            last_seen_block = block_number.into();
            let raw_log: RawLog = log.clone().into();

            if let Ok(l1_event) = L1Events::decode_log(&raw_log) {
                process_l1_event(
                    l2_erc20_bridge_addr,
                    &log,
                    &l1_event,
                    &middleware,
                    &mut sender,
                )
                .await?;
            }
        }

        tracing::info!("all event streams have terminated, exiting...");

        Ok(last_seen_block)
    }
}

async fn process_l1_event<M, S>(
    _l2_erc20_bridge_addr: Address,
    log: &Log,
    l1_event: &L1Events,
    _middleware: M,
    sender: &mut S,
) -> Result<()>
where
    M: Middleware,
    <M as Middleware>::Provider: PubsubClient,
    S: Sink<BlockEvent> + Unpin,
    <S as Sink<BlockEvent>>::Error: std::fmt::Debug,
{
    let Some(block_number) = log.block_number.map(|bn| bn.as_u64()) else {
        return Ok(());
    };

    match l1_event {
        L1Events::BlockCommit(bc) => {
            CHAIN_EVENTS_METRICS.block_commit_events.inc();
            sender
                .send(BlockEvent::BlockCommit {
                    block_number,
                    event: bc.clone(),
                })
                .await
                .map_err(|_| Error::ChannelClosing)?;
        }
        L1Events::BlocksVerification(event) => {
            CHAIN_EVENTS_METRICS.block_verification_events.inc();
            sender
                .send(BlockEvent::BlocksVerification {
                    block_number,
                    event: event.clone(),
                })
                .await
                .map_err(|_| Error::ChannelClosing)?;
        }
        L1Events::BlocksExecution(event) => {
            tracing::info!(
                "Received a block execution event {event:?} {:?}",
                log.transaction_hash
            );

            CHAIN_EVENTS_METRICS.block_execution_events.inc();
            sender
                .send(BlockEvent::BlockExecution {
                    block_number,
                    event: event.clone(),
                })
                .await
                .map_err(|_| Error::ChannelClosing)?;
        }
    }
    Ok(())
}
