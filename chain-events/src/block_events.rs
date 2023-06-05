use std::sync::Arc;

use ethers::{
    abi::{Address, RawLog},
    contract::EthEvent,
    providers::{Middleware, PubsubClient},
    types::{BlockNumber, Filter, ValueOrArray},
};
use futures::{Sink, SinkExt, StreamExt};

use client::{
    zksync_contract::codegen::{BlockCommitFilter, BlockExecutionFilter, BlocksVerificationFilter},
    BlockEvent,
};

use crate::Result;

// A convenience multiplexer for `Block`-related events.
//
// The only purose of this structure is multpliexing together
// the `Block` events from the middleware as currently `ethers` events
// api relies on lifetimes and borrowing is hard to use otherwise
// in the async context.
pub struct BlockEvents<M: Middleware> {
    middleware: Arc<M>,
}

impl<M> BlockEvents<M>
where
    M: Middleware,
{
    /// Creates a new `BlockEvents` structure
    ///
    /// # Arguments
    ///
    /// * `middleware`: The middleware to perform requests with.
    pub fn new(middleware: Arc<M>) -> Result<BlockEvents<M>> {
        Ok(Self { middleware })
    }
}

impl<M> BlockEvents<M>
where
    M: Middleware,
    <M as Middleware>::Provider: PubsubClient,
{
    /// A cunvenience function that listens for all `Block`-related and sends them to the user.
    ///
    /// `ethers` apis have two approaches to querying events from chain:
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
    pub async fn run<B, S>(self, address: Address, from_block: B, mut sender: S) -> Result<()>
    where
        B: Into<BlockNumber> + Copy,
        S: Sink<BlockEvent> + Unpin,
        <S as Sink<BlockEvent>>::Error: std::fmt::Debug,
    {
        let filter = Filter::new()
            .from_block(from_block)
            .address(Into::<ValueOrArray<Address>>::into(address))
            .topic0(vec![
                BlockCommitFilter::signature(),
                BlocksVerificationFilter::signature(),
                BlockExecutionFilter::signature(),
            ]);

        let mut logs = self.middleware.subscribe_logs(&filter).await?;

        while let Some(log) = logs.next().await {
            let block_number = match log.block_number {
                Some(b) => b.as_u64(),
                None => {
                    continue;
                }
            };
            let raw_log: RawLog = log.clone().into();

            if let Ok(event) = BlockCommitFilter::decode_log(&raw_log) {
                sender
                    .send(BlockEvent::BlockCommit {
                        block_number,
                        event,
                    })
                    .await
                    .unwrap();
                continue;
            }

            if let Ok(event) = BlocksVerificationFilter::decode_log(&raw_log) {
                sender
                    .send(BlockEvent::BlocksVerification {
                        block_number,
                        event,
                    })
                    .await
                    .unwrap();
                continue;
            }

            if let Ok(event) = BlockExecutionFilter::decode_log(&raw_log) {
                sender
                    .send(BlockEvent::BlockExecution {
                        block_number,
                        event,
                    })
                    .await
                    .unwrap();
                continue;
            }
        }

        log::info!("all event streams have terminated, exiting...");

        Ok(())
    }
}