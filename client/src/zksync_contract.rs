//! ABI wrappers for the `ZkSync` contract.

use std::{fmt::Debug, sync::Arc};

use ethers::{
    abi::{AbiDecode, AbiError, RawLog},
    prelude::{EthCall, EthEvent, Event},
    providers::{Middleware, PubsubClient},
    types::{Address, BlockNumber, Bytes, Filter, ValueOrArray, H256, U256},
};
use futures::{Sink, SinkExt, StreamExt};

use crate::{
    l1bridge::codegen::FinalizeWithdrawalCall, Result, ETH_TOKEN_ADDRESS, L1_MESSENGER_ADDRESS,
};

#[allow(missing_docs)]
pub mod codegen {
    use ethers::prelude::abigen;

    abigen!(IZkSync, "$CARGO_MANIFEST_DIR/src/contracts/IZkSync.json");
}

pub use codegen::{
    BlockCommitFilter, BlockExecutionFilter, BlocksRevertFilter, BlocksVerificationFilter,
    FinalizeEthWithdrawalCall, IZkSyncEvents,
};

use self::codegen::CommitBlocksCall;

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

/// A struct wrapper for interacting with `ZkSync` contract.
#[derive(Clone)]
pub struct ZkSync<M> {
    /// Cogedenerated contract wrapper.
    pub contract: codegen::IZkSync<M>,
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

/// A wrapper to implement a conpressed ABI encoding used in the project.
#[derive(Default, Debug)]
pub struct L2LogCompresed(pub codegen::L2Log);

impl AbiDecode for L2LogCompresed {
    fn decode(bytes: impl AsRef<[u8]>) -> std::result::Result<Self, AbiError> {
        if bytes.as_ref().len() < 88 {
            return Err(AbiError::DecodingError(ethers::abi::Error::InvalidData));
        }

        let bytes = bytes.as_ref();

        let inner = codegen::L2Log {
            l_2_shard_id: bytes[0],
            is_service: bytes[1] != 0,
            tx_number_in_block: u16::from_be_bytes([bytes[2], bytes[3]]),
            sender: Address::from_slice(&bytes[4..24]),
            key: bytes[24..56]
                .try_into()
                .expect("length has been checked; qed"),
            value: bytes[56..88]
                .try_into()
                .expect("length has been checked; qed"),
        };

        Ok(Self(inner))
    }
}

const L2_TO_L1_LOG_SERIALIZED_SIZE: usize = 88;

/// Information about withdrawals from L2ToL1 logs.
#[derive(Debug)]
pub struct WithdrawalInfo {
    /// L1 address of the token
    pub token: Address,

    /// Recepient of the withdrawal.
    pub to: Address,

    /// Amount of the withdrawal.
    pub amount: U256,

    /// Block number in which the withdrawal was committed on L1.
    pub block_number: u64,
}

/// Given a [`CommitBlocksCall`] parse all withdrawal events from L2ToL1 logs.
// TODO: rewrite in `nom`.
pub fn parse_withdrawal_events_l1(
    call: &CommitBlocksCall,
    block_number: u64,
    l2_erc20_bridge_addr: Address,
) -> Vec<WithdrawalInfo> {
    let mut withdrawals = vec![];

    for data in &call.new_blocks_data {
        let logs = &data.l_2_logs;
        let length_bytes = match logs.get(..4) {
            Some(b) => b,
            None => continue,
        };

        let length = u32::from_be_bytes(
            length_bytes
                .try_into()
                .expect("bytes length checked by .get(); qed"),
        );

        let logs = &logs[4..];

        let mut current_message = 0;

        for i in 0..length as usize {
            let offset = i * L2_TO_L1_LOG_SERIALIZED_SIZE;
            let log_entry =
                L2LogCompresed::decode(&logs[offset..(offset + L2_TO_L1_LOG_SERIALIZED_SIZE)])
                    .unwrap();

            if log_entry.0.sender != L1_MESSENGER_ADDRESS {
                continue;
            }

            let message = &data.l_2_arbitrary_length_messages[current_message];
            let message_sender: Address = H256::from(log_entry.0.key).into();

            if message_sender == ETH_TOKEN_ADDRESS.parse().unwrap()
                && FinalizeEthWithdrawalCall::selector() == message[..4]
                && message.len() >= 56
            {
                let to = Address::from(
                    TryInto::<[u8; 20]>::try_into(&message[4..24])
                        .expect("message length was checked; qed"),
                );
                let amount = U256::from(
                    TryInto::<[u8; 32]>::try_into(&message[24..56])
                        .expect("message length was checked; qed"),
                );

                withdrawals.push(WithdrawalInfo {
                    token: ETH_TOKEN_ADDRESS.parse().unwrap(),
                    to,
                    amount,
                    block_number,
                });
            }

            if message_sender == l2_erc20_bridge_addr
                && FinalizeWithdrawalCall::selector() == message[..4]
                && message.len() >= 68
            {
                let to = Address::from(
                    TryInto::<[u8; 20]>::try_into(&message[4..24])
                        .expect("message length was checked; qed"),
                );
                let token = Address::from(
                    TryInto::<[u8; 20]>::try_into(&message[24..44])
                        .expect("message length was checked; qed"),
                );
                let amount = U256::from(
                    TryInto::<[u8; 32]>::try_into(&message[44..76])
                        .expect("message length was checked; qed"),
                );
                withdrawals.push(WithdrawalInfo {
                    token,
                    to,
                    amount,
                    block_number,
                });
            }
            current_message += 1;
        }
    }

    withdrawals
}
