//! ABI wrappers for the `ZkSync` contract.

use std::fmt::Debug;

use ethers::{
    abi::{AbiDecode, AbiError},
    prelude::EthCall,
    types::{Address, H256, U256},
};

use crate::{l1bridge::codegen::FinalizeWithdrawalCall, ETH_TOKEN_ADDRESS, L1_MESSENGER_ADDRESS};

#[allow(missing_docs)]
pub mod codegen {
    use ethers::prelude::abigen;

    abigen!(IZkSync, "$CARGO_MANIFEST_DIR/src/contracts/IZkSync.json");
}

use codegen::{
    BlockCommitFilter, BlockExecutionFilter, BlocksRevertFilter, BlocksVerificationFilter,
};

use self::codegen::{CommitBlocksCall, FinalizeEthWithdrawalCall};

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

    /// `L2ToL1Event`s.
    L2ToL1Events {
        ///events
        events: Vec<L2ToL1Event>,
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
            Self::L2ToL1Events { events } => f
                .debug_struct("L2ToL1Events")
                .field("events", &events)
                .finish(),
        }
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
pub struct L2ToL1Event {
    /// L1 address of the token
    pub token: Address,

    /// Recepient of the withdrawal.
    pub to: Address,

    /// Amount of the withdrawal.
    pub amount: U256,

    /// Block number in which the withdrawal was committed on L1.
    pub l1_block_number: u64,

    /// Block number in which the withdrawal happened on L2.
    pub l2_block_number: u64,

    /// Number of tx in block
    pub tx_number_in_block: u16,
}

/// Given a [`CommitBlocksCall`] parse all withdrawal events from L2ToL1 logs.
// TODO: rewrite in `nom`.
pub fn parse_withdrawal_events_l1(
    call: &CommitBlocksCall,
    l1_block_number: u64,
    l2_erc20_bridge_addr: Address,
) -> Vec<L2ToL1Event> {
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
            let l2_block_number = data.block_number;

            if message_sender == ETH_TOKEN_ADDRESS
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

                withdrawals.push(L2ToL1Event {
                    token: ETH_TOKEN_ADDRESS,
                    to,
                    amount,
                    l1_block_number,
                    l2_block_number,
                    tx_number_in_block: log_entry.0.tx_number_in_block,
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
                withdrawals.push(L2ToL1Event {
                    token,
                    to,
                    amount,
                    l1_block_number,
                    l2_block_number,
                    tx_number_in_block: log_entry.0.tx_number_in_block,
                });
            }
            current_message += 1;
        }
    }

    withdrawals
}
