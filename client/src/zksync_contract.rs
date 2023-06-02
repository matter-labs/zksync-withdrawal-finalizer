//! ABI wrappers for the `ZkSync` contract.

use std::{fmt::Debug, sync::Arc};

use ethers::{
    abi::RawLog,
    prelude::EthEvent,
    providers::{Middleware, PubsubClient},
    types::{Address, BlockNumber, Filter, ValueOrArray, H256},
};
use futures::{Sink, SinkExt, StreamExt};

use crate::Result;

#[allow(missing_docs)]
pub mod codegen {
    use ethers::prelude::abigen;

    abigen!(IZkSync, "$CARGO_MANIFEST_DIR/src/contracts/IZkSync.json");
}

use codegen::{
    BlockCommitFilter, BlockExecutionFilter, BlocksRevertFilter, BlocksVerificationFilter,
};

/// An `enum` wrapping different block `event`s
#[derive(Debug)]
pub enum BlockEvent {
    /// A `BlockCommit` event
    BlockCommit {
        /// Number of the block in which the event happened.
        block_number: u64,

        /// Event itself.
        event: BlockCommitFilter,
    },

    /// A `BlockExecution` event
    BlockExecution {
        /// Number of the block in which the event happened.
        block_number: u64,

        /// Event itself.
        event: BlockExecutionFilter,
    },

    /// A `BlocksVerification` event
    BlocksVerification {
        /// Number of the block in which the event happened.
        block_number: u64,

        /// Event itself.
        event: BlocksVerificationFilter,
    },

    /// A `BlocksRevert` event.
    BlocksRevert {
        /// Number of the block in which the event happened.
        block_number: u64,

        /// Event itself.
        event: BlocksRevertFilter,
    },
}

// This custom impl sole purpose is pretty hash display instead of [u8; 32]
impl std::fmt::Display for BlockEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BlockCommit { event: bc, .. } => f
                .debug_struct("BlockCommitFilter")
                .field("block_number", &bc.block_number)
                .field("block_hash", &H256::from(&bc.block_hash))
                .field("commitment", &H256::from(&bc.commitment))
                .finish(),
            Self::BlockExecution { event: be, .. } => f
                .debug_struct("BlockExecution")
                .field("block_number", &be.block_number)
                .field("block_hash", &H256::from(&be.block_hash))
                .field("commitment", &H256::from(&be.commitment))
                .finish(),
            Self::BlocksVerification { event: bv, .. } => f
                .debug_struct("BlocksVerification")
                .field(
                    "previous_last_verified_block",
                    &bv.previous_last_verified_block,
                )
                .field(
                    "current_last_verified_block",
                    &bv.current_last_verified_block,
                )
                .finish(),
            Self::BlocksRevert { event: br, .. } => f
                .debug_struct("BlocksRevert")
                .field("total_blocks_commited", &br.total_blocks_committed)
                .field("total_blocks_verified", &br.total_blocks_verified)
                .field("total_blocks_executed", &br.total_blocks_executed)
                .finish(),
        }
    }
}

/// A convenience multiplexer for `Block`-related events.
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
    pub async fn new(middleware: Arc<M>) -> Result<BlockEvents<M>> {
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

        let mut logs = self
            .middleware
            .subscribe_logs(&filter)
            .await
            .map_err(|e| crate::Error::Middleware(format!("{e}")))?;

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
