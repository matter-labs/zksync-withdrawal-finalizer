#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Interactions with zkSync on-chain contracts.

mod error;

pub use error::{Error, Result};

use ethers::{
    contract::EthEvent,
    providers::{JsonRpcClient, ProviderError},
    types::{Address, Bytes, H160, H256, U64},
};

use zksync_types::{
    L2ToL1Log, L2ToL1LogProof, Log as ZKSLog, TransactionReceipt as ZKSTransactionReceipt,
};

pub use zksync_types::WithdrawalEvent;
pub mod ethtoken;
pub mod l1bridge;
pub mod l1messenger;
pub mod l2bridge;
pub mod l2standard_token;
pub mod withdrawal_finalizer;
pub mod zksync_contract;
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
    index: usize,
) -> Result<(ZKSLog, Option<U64>)> {
    let receipt = get_transaction_receipt(client, tx_hash).await?;

    let log = receipt
        .logs
        .into_iter()
        .filter(|entry| {
            entry.address == L1_MESSENGER_ADDRESS
                && entry.topics[0] == l1messenger::L1MessageSentFilter::signature()
        })
        .nth(index)
        .ok_or(Error::NoSuchIndex(index))?;

    Ok((log, receipt.l1_batch_tx_index))
}

/// Get the `L2ToL1Log` by index.
///
/// # Arguments
///
/// * `client`: A `JsonRpcClient` to perform requests with
/// * `tx_hash`: Hash of the transaction
/// * `index`: Index of the `L2ToL1Log` from the transaction receipt.
pub async fn get_withdrawal_l2_to_l1_log<J: JsonRpcClient>(
    client: &J,
    tx_hash: H256,
    index: usize,
) -> Result<(L2ToL1Log, Option<U64>)> {
    let receipt = get_transaction_receipt(client, tx_hash).await?;

    let log = receipt
        .l2_to_l1_logs
        .into_iter()
        .filter(|entry| entry.sender == L1_MESSENGER_ADDRESS)
        .nth(index)
        .ok_or(Error::NoSuchIndex(index))?;

    Ok((log, Some(U64::from(index))))
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
    index: usize,
) -> Result<Option<L2ToL1LogProof>> {
    let proof = client
        .request::<[H256; 1], Option<Vec<L2ToL1LogProof>>>("zks_getL2ToL1LogProof", [tx_hash])
        .await
        .map_err(Into::<ProviderError>::into)?
        .ok_or(Error::NoSuchIndex(index))?
        .into_iter()
        .nth(index);

    Ok(proof)
}

/// Call `zks_getL1BatchBlockRange` RPC method.
///
/// # Arguments
///
/// * `client`: `JsonRpcCLient` instance to perform the request with
/// * `batch_number`: the number of the batch
pub async fn get_l1_batch_block_range<J: JsonRpcClient>(
    client: &J,
    batch_number: u32,
) -> Result<Option<(U64, U64)>> {
    let range = client
        .request::<[u32; 1], Option<(U64, U64)>>("zks_getL1BatchBlockRange", [batch_number])
        .await
        .map_err(Into::<ProviderError>::into)?;

    Ok(range)
}

/// Withdrawal params
#[allow(unused)]
pub struct WithdrawalParams {
    l1_batch_number: U64,
    l2_message_index: u32,
    l2_tx_number_in_bloc: u16,
    message: Bytes,
    sender: Address,
    proof: Vec<[u8; 32]>,
}

/// Get the parameters necessary to call `finalize_withdrawals`.
pub async fn finalize_withdrawal_params<J: JsonRpcClient>(
    client: &J,
    withdrawal_hash: H256,
    index: usize,
) -> Result<WithdrawalParams> {
    let (log, l1_batch_tx_id) = get_withdrawal_log(client, withdrawal_hash, index).await?;
    let (_, l2_to_l1_log_index) =
        get_withdrawal_l2_to_l1_log(client, withdrawal_hash, index).await?;

    let sender = TryInto::<[u8; 20]>::try_into(&log.topics[1].as_bytes()[..20])
        .expect("H256 always has enough bytes to fill H160. qed")
        .into();

    let proof = get_log_proof(
        client,
        withdrawal_hash,
        l2_to_l1_log_index
            .expect("The L2ToL2LogIndex is always present in the trace. qed")
            .as_usize(),
    )
    .await?
    .expect("Log proof should be present. qed");

    let message = log.data;
    let l2_message_index = proof.id;
    let proof: Vec<_> = proof
        .proof
        .iter()
        .map(|hash| hash.to_fixed_bytes())
        .collect();

    Ok(WithdrawalParams {
        l1_batch_number: log.l1_batch_number.unwrap(),
        l2_message_index,
        l2_tx_number_in_bloc: l1_batch_tx_id.unwrap().as_u32() as u16,
        message: message.0.into(),
        sender,
        proof,
    })
}
