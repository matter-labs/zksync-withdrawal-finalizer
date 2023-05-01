#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Interactions with zkSync on-chain contracts.

mod error;

pub use error::{Error, Result};

use ethers::{
    contract::EthEvent,
    providers::{JsonRpcClient, ProviderError},
    types::{Address, H256, U256},
};

pub use zksync_types::api::{Log as ZKSLog, TransactionReceipt as ZKSTransactionReceipt};

pub mod ethtoken;
pub mod l1bridge;
pub mod l1messenger;
pub mod l2bridge;
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

/// Get the `zksync` transaction receipt by transaction hash
///
/// # Arguments
///
/// * `client`: `JsonRpcClient` instance to perform the request with
/// * `tx_hash`: Hash of the transaction
pub async fn get_transaction_receipt<J: JsonRpcClient>(
    client: &J,
    tx_hash: H256,
) -> Result<ZKSTransactionReceipt> {
    let receipt = client
        .request::<[H256; 1], ZKSTransactionReceipt>("eth_getTransactionReceipt", [tx_hash])
        .await
        .map_err(Into::<ProviderError>::into)?;

    Ok(receipt)
}

/// Get the `zksync` withdrawal logs by tx hash.
///
/// # Arguments
///
/// * `client`: `JsonRpcClient` instance to perform the request with
/// * `tx_hash`: Hash of the transaction
pub async fn get_withdrawal_log<J: JsonRpcClient>(
    client: &J,
    tx_hash: H256,
) -> Result<Vec<ZKSLog>> {
    let logs = get_transaction_receipt(client, tx_hash)
        .await?
        .logs
        .into_iter()
        .filter(|entry| {
            entry.address == zksync_config::constants::contracts::L1_MESSENGER_ADDRESS
                // Hash types from zksync on ZKSTransactionReceipt
                // do not match hash type H256 from ethers since
                // primitive-types crate version mismatch.
                && entry.topics[0].as_bytes()
                    == l1messenger::L1MessageSentFilter::signature().as_bytes()
        })
        .collect();

    Ok(logs)
}
