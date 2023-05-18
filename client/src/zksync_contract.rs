//! ABI wrappers for the `ZkSync` contract.

use std::{fmt::Debug, sync::Arc};

use ethers::{
    prelude::{Contract, EthEvent, Event},
    providers::{Middleware, PubsubClient},
    types::{Address, BlockNumber, Bytes, H256, U256},
};
use futures::{Sink, SinkExt, StreamExt};

use crate::Result;

#[allow(missing_docs)]
mod codegen {
    use ethers::prelude::abigen;

    abigen!(IZkSync, "./src/contracts/IZkSync.json");
}

pub use codegen::{
    BlockCommitFilter, BlockExecutionFilter, BlocksRevertFilter, BlocksVerificationFilter,
    FinalizeEthWithdrawalCall, IZkSyncEvents,
};

/// An `enum` wrapping different block `event`s
#[derive(Debug)]
pub enum BlockEvent {
    /// A `BlockCommit` event
    BlockCommit(BlockCommitFilter),

    /// A `BlockExecution` event
    BlockExecution(BlockExecutionFilter),

    /// A `BlocksVerification` event
    BlocksVerification(BlocksVerificationFilter),

    /// A `BlocksRevert` event.
    BlocksRevert(BlocksRevertFilter),
}

// This custom impl sole purpose is pretty hash display instead of [u8; 32]
impl std::fmt::Display for BlockEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BlockCommit(bc) => f
                .debug_struct("BlockCommitFilter")
                .field("block_number", &bc.block_number)
                .field("block_hash", &H256::from(&bc.block_hash))
                .field("commitment", &H256::from(&bc.commitment))
                .finish(),
            Self::BlockExecution(be) => f
                .debug_struct("BlockExecution")
                .field("block_number", &be.block_number)
                .field("block_hash", &H256::from(&be.block_hash))
                .field("commitment", &H256::from(&be.commitment))
                .finish(),
            Self::BlocksVerification(bv) => f
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
            Self::BlocksRevert(br) => f
                .debug_struct("BlocksRevert")
                .field("total_blocks_commited", &br.total_blocks_committed)
                .field("total_blocks_verified", &br.total_blocks_verified)
                .field("total_blocks_executed", &br.total_blocks_executed)
                .finish(),
        }
    }
}

impl From<BlockCommitFilter> for BlockEvent {
    fn from(value: BlockCommitFilter) -> Self {
        Self::BlockCommit(value)
    }
}

impl From<BlockExecutionFilter> for BlockEvent {
    fn from(value: BlockExecutionFilter) -> Self {
        Self::BlockExecution(value)
    }
}

impl From<BlocksVerificationFilter> for BlockEvent {
    fn from(value: BlocksVerificationFilter) -> Self {
        Self::BlocksVerification(value)
    }
}

impl From<BlocksRevertFilter> for BlockEvent {
    fn from(value: BlocksRevertFilter) -> Self {
        Self::BlocksRevert(value)
    }
}

/// A struct wrapper for interacting with `ZkSync` contract.
pub struct ZkSync<M> {
    contract: codegen::IZkSync<M>,
}

impl<M: Middleware> ZkSync<M> {
    /// Create a new instalce of `ZkSync` contract.
    ///
    /// # Arguments
    ///
    /// * `address` - An address of the contract
    /// * `provider` - A middleware to perform calls to the contract
    pub fn new(address: Address, provider: Arc<M>) -> Self {
        let contract = codegen::IZkSync::new(address, provider);

        Self { contract }
    }

    /// Call `finalizeEthWithdrawal` method of `ZkSync` contract.
    pub async fn finalize_eth_withdrawal(
        &self,
        l2_block_number: U256,
        l2_message_index: U256,
        l2_tx_number_in_block: u16,
        message: Bytes,
        merkle_proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        self.contract
            .finalize_eth_withdrawal(
                l2_block_number,
                l2_message_index,
                l2_tx_number_in_block,
                message,
                merkle_proof,
            )
            .call()
            .await
            .map_err(Into::into)
    }

    /// Get the [`ethers::contract::Event`] from the contract.
    pub fn event_of_type<D: EthEvent>(&self) -> Event<Arc<M>, M, D> {
        self.contract.event::<D>()
    }

    /// Call `isEthWithdrawalFinalized` method from the contract.
    pub async fn is_eth_withdrawal_finalized(
        &self,
        l2_block_number: U256,
        l2_message_index: U256,
    ) -> Result<bool> {
        self.contract
            .is_eth_withdrawal_finalized(l2_block_number, l2_message_index)
            .call()
            .await
            .map_err(Into::into)
    }
}

/// A convenience multiplexer for `Block`-related events.
//
// The only purose of this structure is multpliexing together
// the `Block` events from the middleware as currently `ethers` events
// api relies on lifetimes and borrowing is hard to use otherwise
// in the async context.
pub struct BlockEvents<M: Middleware> {
    commit_event: Event<Arc<M>, M, BlockCommitFilter>,
    execution_event: Event<Arc<M>, M, BlockExecutionFilter>,
    verification_event: Event<Arc<M>, M, BlocksVerificationFilter>,
    revert_event: Event<Arc<M>, M, BlocksRevertFilter>,
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
        let commit_event = Contract::event_of_type::<BlockCommitFilter>(middleware.clone());
        let execution_event = Contract::event_of_type::<BlockExecutionFilter>(middleware.clone());
        let verification_event =
            Contract::event_of_type::<BlocksVerificationFilter>(middleware.clone());
        let revert_event = Contract::event_of_type::<BlocksRevertFilter>(middleware);

        Ok(Self {
            commit_event,
            execution_event,
            verification_event,
            revert_event,
        })
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
        let sub_commit = self
            .commit_event
            .from_block(from_block.into())
            .address(address.into());
        let mut sub_commit_stream = sub_commit.subscribe().await?.fuse();

        let sub_execute = self
            .execution_event
            .from_block(from_block.into())
            .address(address.into());

        let mut sub_commit_execute = sub_execute.subscribe().await?.fuse();

        let sub_verification = self
            .verification_event
            .from_block(from_block.into())
            .address(address.into());

        let mut sub_commit_verify = sub_verification.subscribe().await?.fuse();

        let sub_revert = self
            .revert_event
            .from_block(from_block.into())
            .address(address.into());

        let mut sub_revert = sub_revert.subscribe().await?.fuse();
        loop {
            futures::select! {
                commit_event = sub_commit_stream.next() => {
                    if let Some(event) = commit_event {
                        let commit_event = event?;
                        sender.send(commit_event.into()).await.unwrap();
                    }
                }
                execute_event = sub_commit_execute.next() => {
                    if let Some(event) = execute_event {
                        let execute_event = event?;
                        sender.send(execute_event.into()).await.unwrap();
                    }
                }
                verify_event = sub_commit_verify.next() => {
                    if let Some(event) = verify_event {
                        let verify_event = event?;
                        sender.send(verify_event.into()).await.unwrap();
                    }
                }
                revert_event = sub_revert.next() => {
                    if let Some(event) = revert_event {
                        let revert_event = event?;
                        sender.send(revert_event.into()).await.unwrap();
                    }
                }
                complete => break,
            }
        }

        Ok(())
    }
}
