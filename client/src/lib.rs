#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Interactions with zkSync on-chain contracts.

mod error;

pub use error::{Error, Result};

use async_trait::async_trait;
use auto_impl::auto_impl;
use ethers::{
    contract::EthEvent,
    providers::{JsonRpcClient, Middleware, Provider},
    types::{Address, Bytes, H160, H256, U64},
};

use zksync_types::{
    BlockDetails, L2ToL1Log, L2ToL1LogProof, Log as ZKSLog, Token,
    TransactionReceipt as ZksyncTransactionReceipt,
};

pub use zksync_contract::BlockEvent;
pub use zksync_types::WithdrawalEvent;

/// Eth token address
pub const ETH_TOKEN_ADDRESS: Address = H160([
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x80, 0x0a,
]);

/// Eth address
pub const ETH_ADDRESS: Address = Address::zero();

/// Address of ethereum L1 messenger
pub const L1_MESSENGER_ADDRESS: Address = H160([
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x80, 0x08,
]);

pub mod ethtoken;
pub mod l1bridge;
pub mod l1messenger;
pub mod l2bridge;
pub mod l2standard_token;
pub mod withdrawal_finalizer;
pub mod zksync_contract;
pub mod zksync_types;

/// is this eth?
pub fn is_eth(address: Address) -> bool {
    address == ETH_TOKEN_ADDRESS || address == ETH_ADDRESS
}

/// Withdrawal params
pub struct WithdrawalParams {
    /// The number of batch on L1
    pub l1_batch_number: U64,

    /// Index of the message on L2
    pub l2_message_index: u32,

    /// Index of tx number in L2 block
    pub l2_tx_number_in_block: u16,

    /// Message
    pub message: Bytes,

    /// Sender of the transaction
    pub sender: Address,

    /// Proof
    pub proof: Vec<[u8; 32]>,
}

/// A middleware for interacting with Zksync node.
#[async_trait]
#[auto_impl(&, Arc, Box)]
pub trait ZksyncMiddleware: Middleware {
    /// Call `zks_getBlockDetails` RPC method.
    ///
    /// # Arguments
    ///
    /// * `client` - RPC client to make request with
    /// * `block_number` - Number of the block
    async fn get_block_details(&self, block_number: u32) -> Result<Option<BlockDetails>>;

    /// Get the `zksync` withdrawal proof by tx hash
    ///
    /// # Arguments
    ///
    /// * `client`: `JsonRpcClient` instance to perform the request with
    /// * `tx_hash`: Hash of the withdrawal transaction
    async fn get_log_proof(
        &self,
        tx_hash: H256,
        l2_to_l1_index: Option<u64>,
    ) -> Result<Option<L2ToL1LogProof>>;

    /// Call `zks_getL1BatchBlockRange` RPC method.
    ///
    /// # Arguments
    ///
    /// * `client`: `JsonRpcCLient` instance to perform the request with
    /// * `batch_number`: the number of the batch
    async fn get_l1_batch_block_range(&self, batch_number: u32) -> Result<Option<(U64, U64)>>;

    /// Call `zks_getConfirmedTokens` RPC method.
    ///
    /// # Arguments
    ///
    /// * `client`: `JsonRpcClient` instance to perform the request with
    /// * `from`: beginning of the requested token interval
    /// * `limit: length of the requested token interval
    async fn get_confirmed_tokens(&self, from: u32, limit: u8) -> Result<Vec<Token>>;

    /// Get the `zksync` transaction receipt by transaction hash
    ///
    /// # Arguments
    ///
    /// * `client`: `JsonRpcClient` instance to perform the request with
    /// * `tx_hash`: Hash of the transaction
    async fn zks_get_transaction_receipt(&self, tx_hash: H256) -> Result<ZksyncTransactionReceipt>;

    /// Get the parameters necessary to call `finalize_withdrawals`.
    async fn finalize_withdrawal_params(
        &self,
        withdrawal_hash: H256,
        index: usize,
    ) -> Result<WithdrawalParams>;

    /// Get the `zksync` withdrawal logs by tx hash.
    ///
    /// # Arguments
    ///
    /// * `client`: `JsonRpcClient` instance to perform the request with
    /// * `tx_hash`: Hash of the transaction
    async fn get_withdrawal_log(
        &self,
        tx_hash: H256,
        index: usize,
    ) -> Result<(ZKSLog, Option<U64>)>;

    /// Get the `L2ToL1Log` by index.
    ///
    /// # Arguments
    ///
    /// * `client`: A `JsonRpcClient` to perform requests with
    /// * `tx_hash`: Hash of the transaction
    /// * `index`: Index of the `L2ToL1Log` from the transaction receipt.
    async fn get_withdrawal_l2_to_l1_log(
        &self,
        tx_hash: H256,
        index: usize,
    ) -> Result<(L2ToL1Log, Option<U64>)>;
}

#[async_trait]
impl<P: JsonRpcClient> ZksyncMiddleware for Provider<P> {
    async fn get_block_details(&self, block_number: u32) -> Result<Option<BlockDetails>> {
        let res = self
            .request::<[u32; 1], Option<BlockDetails>>("zks_getBlockDetails", [block_number])
            .await?;

        Ok(res)
    }

    async fn get_log_proof(
        &self,
        tx_hash: H256,
        l2_to_l1_index: Option<u64>,
    ) -> Result<Option<L2ToL1LogProof>> {
        let params = match l2_to_l1_index {
            Some(idx) => vec![
                ethers::utils::serialize(&tx_hash),
                ethers::utils::serialize(&idx),
            ],
            None => vec![ethers::utils::serialize(&tx_hash)],
        };
        let res = self.request("zks_getL2ToL1LogProof", params).await?;

        Ok(res)
    }

    async fn get_l1_batch_block_range(&self, batch_number: u32) -> Result<Option<(U64, U64)>> {
        let res = self
            .request::<[u32; 1], Option<(U64, U64)>>("zks_getL1BatchBlockRange", [batch_number])
            .await?;
        Ok(res)
    }

    async fn get_confirmed_tokens(&self, from: u32, limit: u8) -> Result<Vec<Token>> {
        let res = self
            .request::<[u32; 2], Vec<Token>>("zks_getConfirmedTokens", [from, limit as u32])
            .await?;

        Ok(res)
    }

    async fn zks_get_transaction_receipt(&self, tx_hash: H256) -> Result<ZksyncTransactionReceipt> {
        let res = self
            .request::<[H256; 1], ZksyncTransactionReceipt>("eth_getTransactionReceipt", [tx_hash])
            .await?;

        Ok(res)
    }

    async fn finalize_withdrawal_params(
        &self,
        withdrawal_hash: H256,
        index: usize,
    ) -> Result<WithdrawalParams> {
        let (log, l1_batch_tx_id) = self.get_withdrawal_log(withdrawal_hash, index).await?;

        let (_, l2_to_l1_log_index) = self
            .get_withdrawal_l2_to_l1_log(withdrawal_hash, index)
            .await?;

        let sender = TryInto::<[u8; 20]>::try_into(&log.topics[1].as_bytes()[..20])
            .expect("H256 always has enough bytes to fill H160. qed")
            .into();

        let proof = self
            .get_log_proof(withdrawal_hash, l2_to_l1_log_index.map(|idx| idx.as_u64()))
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
            l2_tx_number_in_block: l1_batch_tx_id.unwrap().as_u32() as u16,
            message: message.0.into(),
            sender,
            proof,
        })
    }

    async fn get_withdrawal_log(
        &self,
        tx_hash: H256,
        index: usize,
    ) -> Result<(ZKSLog, Option<U64>)> {
        let receipt = self.zks_get_transaction_receipt(tx_hash).await?;

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

    async fn get_withdrawal_l2_to_l1_log(
        &self,
        tx_hash: H256,
        index: usize,
    ) -> Result<(L2ToL1Log, Option<U64>)> {
        let receipt = self.zks_get_transaction_receipt(tx_hash).await?;

        let log = receipt
            .l2_to_l1_logs
            .into_iter()
            .filter(|entry| entry.sender == L1_MESSENGER_ADDRESS)
            .nth(index)
            .ok_or(Error::NoSuchIndex(index))?;

        Ok((log, Some(U64::from(index))))
    }
}
