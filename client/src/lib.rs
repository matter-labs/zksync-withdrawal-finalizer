#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Interactions with zkSync on-chain contracts.

mod error;

pub use error::{Error, Result};
use ethers::types::{Address, H256, U256};

pub mod ethtoken;
pub mod l1bridge;
pub mod l2standard_token;

/// Withdrawal event struct
#[derive(Debug)]
pub struct WithdrawalEvent {
    /// A hash of the transaction of this withdrawal.
    pub tx_hash: H256,

    /// Number of the block this withdrawal happened in.
    pub block_number: u64,

    /// Address of the transfered token
    pub token: Address,

    /// The amount transfered.
    pub amount: U256,
}
