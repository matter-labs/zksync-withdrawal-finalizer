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

use self::codegen::{CommitBatchesCall, FinalizeEthWithdrawalCall};

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
                .field("block_number", &bc.batch_number)
                .field("block_hash", &H256::from(&bc.batch_hash))
                .field("commitment", &H256::from(&bc.commitment))
                .finish(),
            Self::BlockExecution { event: be, .. } => f
                .debug_struct("BlockExecution")
                .field("block_number", &be.batch_number)
                .field("block_hash", &H256::from(&be.batch_hash))
                .field("commitment", &H256::from(&be.commitment))
                .finish(),
            Self::BlocksVerification { event: bv, .. } => f
                .debug_struct("BlocksVerification")
                .field(
                    "previous_last_verified_batch",
                    &bv.previous_last_verified_batch,
                )
                .field(
                    "current_last_verified_block",
                    &bv.current_last_verified_batch,
                )
                .finish(),
            Self::BlocksRevert { event: br, .. } => f
                .debug_struct("BlocksRevert")
                .field("total_blocks_commited", &br.total_batches_committed)
                .field("total_blocks_verified", &br.total_batches_verified)
                .field("total_blocks_executed", &br.total_batches_executed)
                .finish(),
            Self::L2ToL1Events { events } => f
                .debug_struct("L2ToL1Events")
                .field("events", &events)
                .finish(),
        }
    }
}

/// A wrapper to implement a compressed ABI encoding used in the project.
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
            tx_number_in_batch: u16::from_be_bytes([bytes[2], bytes[3]]),
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

/// Information about withdrawals from [`L2ToL1`] logs.
#[derive(Debug)]
pub struct L2ToL1Event {
    /// L1 address of the token
    pub token: Address,

    /// Recipient of the withdrawal.
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

/// Given a [`CommitBatchesCall`] parse all withdrawal events from [`L2ToL1`] logs.
// TODO: rewrite in `nom`.
pub fn parse_withdrawal_events_l1(
    call: &CommitBatchesCall,
    l1_block_number: u64,
    l2_erc20_bridge_addr: Address,
) -> Vec<L2ToL1Event> {
    let mut withdrawals = vec![];

    for data in &call.new_batches_data {
        let logs_pubdata = &data.total_l2_to_l1_pubdata;
        let mut cursor = 0;
        let length_bytes = match logs_pubdata.get(..4) {
            Some(b) => b,
            None => continue,
        };
        cursor += 4;

        let length = u32::from_be_bytes(
            length_bytes
                .try_into()
                .expect("bytes length checked by .get(); qed"),
        ) as usize;

        let logs = &logs_pubdata[cursor..];

        let mut current_message = 0;

        let mut l2_to_l1_compressed_messages = vec![];
        for i in 0..length {
            let offset = i * L2_TO_L1_LOG_SERIALIZED_SIZE;
            let log_entry =
                L2LogCompresed::decode(&logs[offset..(offset + L2_TO_L1_LOG_SERIALIZED_SIZE)])
                    .unwrap();

            if log_entry.0.sender != L1_MESSENGER_ADDRESS {
                continue;
            }

            l2_to_l1_compressed_messages.push((log_entry, current_message));

            current_message += 1;
        }
        cursor += length * L2_TO_L1_LOG_SERIALIZED_SIZE;

        let messages_length_bytes = &logs_pubdata[cursor..cursor + 4];
        let messages_length = u32::from_be_bytes(
            messages_length_bytes
                .try_into()
                .expect("bytes length checked by .get(); qed"),
        ) as usize;
        cursor += 4;
        let messages_bytes = &logs_pubdata[cursor..];

        // reset cursor, now we are working with messages
        cursor = 0;
        let mut current_message = 0;
        for (log_entry, position) in l2_to_l1_compressed_messages {
            // We are assuming that the messages are sorted by position
            for i in current_message..messages_length {
                let current_message_length = u32::from_be_bytes(
                    messages_bytes[cursor..cursor + 4]
                        .try_into()
                        .expect("bytes length checked by .get(); qed"),
                ) as usize;
                cursor += 4;
                let message = &messages_bytes[cursor..cursor + current_message_length];
                cursor += current_message_length;
                // If the current message is not the one we are looking for, skip it and increase the cursor
                if i < position {
                    continue;
                }
                if i > position {
                    panic!("We should've break before this point")
                }

                let message_sender: Address = H256::from(log_entry.0.key).into();
                let l2_block_number = data.batch_number;

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
                        tx_number_in_block: log_entry.0.tx_number_in_batch,
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
                        tx_number_in_block: log_entry.0.tx_number_in_batch,
                    });
                }
                current_message = i + 1;
                break;
            }
        }
    }

    withdrawals
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::abi::Bytes;
    use hex::FromHex;
    use std::str::FromStr;

    #[test]
    fn parse_l2_to_l1() {
        let input = include_str!("../../test_tx.txt");
        let bytes = Bytes::from_hex(input).unwrap();
        let block = CommitBatchesCall::decode(bytes).unwrap();
        let withdrawals = parse_withdrawal_events_l1(
            &block,
            0,
            Address::from_str("11f943b2c77b743AB90f4A0Ae7d5A4e7FCA3E102").unwrap(),
        );
        assert_eq!(withdrawals.len(), 19);
    }
}
