//! ABI wrappers for the `ZkSync` contract.

use std::fmt::Debug;

use ethers::types::H256;

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
