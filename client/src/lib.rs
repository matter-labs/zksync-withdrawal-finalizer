#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Interactions with zkSync on-chain contracts.

mod error;

pub use error::{Error, Result};

use ethers::{
    contract::EthEvent,
    providers::{JsonRpcClient, ProviderError},
    types::{Address, H160, H256},
};

use zksync_types::{L2ToL1LogProof, Log as ZKSLog, TransactionReceipt as ZKSTransactionReceipt};

pub use zksync_types::WithdrawalEvent;
pub mod ethtoken;
pub mod l1bridge;
pub mod l1messenger;
pub mod l2bridge;
pub mod l2standard_token;
pub mod zksync_types;

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

#[allow(missing_docs)]
pub const L1_MESSENGER_ADDRESS: Address = H160([
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x80, 0x08,
]);

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
            entry.address == L1_MESSENGER_ADDRESS
                && entry.topics[0] == l1messenger::L1MessageSentFilter::signature()
        })
        .collect();

    Ok(logs)
}

/// Get the `zksync` withdrawal proof by tx hash
///
/// # Arguments
///
/// * `client`: `JsonRpcClient` instance to perform the request with
/// * `tx_hash`: Hash of the withdrawal transaction
pub async fn get_log_proof<J: JsonRpcClient>(
    client: &J,
    tx_hash: H256,
) -> Result<Option<L2ToL1LogProof>> {
    let proof = client
        .request::<[H256; 1], Option<L2ToL1LogProof>>("zks_getL2ToL1LogProof", [tx_hash])
        .await
        .map_err(Into::<ProviderError>::into)?;

    Ok(proof)
}
