#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Interactions with zkSync on-chain contracts.

mod error;

use std::time::Instant;

pub use error::{Error, Result};

use async_trait::async_trait;
use auto_impl::auto_impl;
use ethers::{
    abi::{ParamType, Token},
    contract::EthEvent,
    providers::{JsonRpcClient, Middleware, Provider},
    types::{Address, Bytes, H160, H256, U256, U64},
};

use l1bridge::codegen::IL1Bridge;
use l1messenger::codegen::L1MessageSentFilter;
use withdrawal_finalizer::codegen::RequestFinalizeWithdrawal;
use zksync_contract::codegen::IZkSync;
use zksync_types::{
    BlockDetails, L2ToL1Log, L2ToL1LogProof, Log as ZKSLog,
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

/// deployer
pub const DEPLOYER_ADDRESS: Address = H160([
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x80, 0x06,
]);
pub mod contracts_deployer;
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

impl WithdrawalParams {
    /// Convert `WithdrawalData` into a `RequestFinalizeWithdrawal` given a gas limit.
    pub fn into_request_with_gaslimit(
        self,
        withdrawal_gas_limit: U256,
    ) -> RequestFinalizeWithdrawal {
        RequestFinalizeWithdrawal {
            l_2_block_number: self.l1_batch_number.as_u64().into(),
            l_2_message_index: self.l2_message_index.into(),
            l_2_tx_number_in_block: self.l2_tx_number_in_block,
            message: self.message,
            merkle_proof: self.proof,
            is_eth: is_eth(self.sender),
            gas: withdrawal_gas_limit,
        }
    }
}

/// Withdrawal params
#[derive(Debug, Clone)]
pub struct WithdrawalParams {
    /// Hash of the withdrawal transaction.
    pub tx_hash: H256,

    /// Event index in the transaction.
    pub event_index_in_tx: u32,

    /// Block number on l2 withdrawal transaction happened in.
    pub l2_block_number: u64,

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
    ) -> Result<Option<WithdrawalParams>>;

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
    ) -> Result<Option<(ZKSLog, Option<U64>)>>;

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
    ) -> Result<Option<(L2ToL1Log, Option<U64>)>>;
}

#[async_trait]
impl<P: JsonRpcClient> ZksyncMiddleware for Provider<P> {
    async fn get_block_details(&self, block_number: u32) -> Result<Option<BlockDetails>> {
        let start = Instant::now();
        let res = self
            .request::<[u32; 1], Option<BlockDetails>>("zks_getBlockDetails", [block_number])
            .await?;

        metrics::histogram!("watcher.zks_client.get_block_details", start.elapsed());

        Ok(res)
    }

    async fn get_log_proof(
        &self,
        tx_hash: H256,
        l2_to_l1_index: Option<u64>,
    ) -> Result<Option<L2ToL1LogProof>> {
        let start = Instant::now();
        let params = match l2_to_l1_index {
            Some(idx) => vec![
                ethers::utils::serialize(&tx_hash),
                ethers::utils::serialize(&idx),
            ],
            None => vec![ethers::utils::serialize(&tx_hash)],
        };
        let res = self.request("zks_getL2ToL1LogProof", params).await?;

        metrics::histogram!("watcher.zks_client.get_log_proof", start.elapsed());

        Ok(res)
    }

    async fn get_l1_batch_block_range(&self, batch_number: u32) -> Result<Option<(U64, U64)>> {
        let start = Instant::now();
        let res = self
            .request::<[u32; 1], Option<(U64, U64)>>("zks_getL1BatchBlockRange", [batch_number])
            .await?;

        metrics::histogram!(
            "watcher.zks_client.get_l1_batch_block_range",
            start.elapsed()
        );

        Ok(res)
    }

    async fn get_confirmed_tokens(&self, from: u32, limit: u8) -> Result<Vec<Token>> {
        let start = Instant::now();
        let res = self
            .request::<[u32; 2], Vec<Token>>("zks_getConfirmedTokens", [from, limit as u32])
            .await?;

        metrics::histogram!("watcher.zks_client.get_confirmed_tokens", start.elapsed());

        Ok(res)
    }

    async fn zks_get_transaction_receipt(&self, tx_hash: H256) -> Result<ZksyncTransactionReceipt> {
        let start = Instant::now();
        let res = self
            .request::<[H256; 1], ZksyncTransactionReceipt>("eth_getTransactionReceipt", [tx_hash])
            .await?;

        metrics::histogram!(
            "watcher.zks_client.get_transaction_receipt",
            start.elapsed()
        );

        Ok(res)
    }

    async fn finalize_withdrawal_params(
        &self,
        withdrawal_hash: H256,
        index: usize,
    ) -> Result<Option<WithdrawalParams>> {
        let (log, l1_batch_tx_id) = match self.get_withdrawal_log(withdrawal_hash, index).await? {
            Some(l) => l,
            None => return Ok(None),
        };

        let (_, l2_to_l1_log_index) = match self
            .get_withdrawal_l2_to_l1_log(withdrawal_hash, index)
            .await?
        {
            Some(l) => l,
            None => return Ok(None),
        };

        let sender = log.topics[1].into();

        let proof = self
            .get_log_proof(withdrawal_hash, l2_to_l1_log_index.map(|idx| idx.as_u64()))
            .await?
            .expect("Log proof should be present. qed");

        let message: Bytes = match ethers::abi::decode(&[ParamType::Bytes], &log.data)
            .expect("log data is valid rlp data; qed")
            .swap_remove(0)
        {
            Token::Bytes(b) => b.into(),
            b => panic!("log data is expected to be rlp bytes, got {b:?}"),
        };

        let l2_message_index = proof.id;
        let proof: Vec<_> = proof
            .proof
            .iter()
            .map(|hash| hash.to_fixed_bytes())
            .collect();

        Ok(Some(WithdrawalParams {
            tx_hash: withdrawal_hash,
            event_index_in_tx: index as u32,
            l2_block_number: log
                .block_number
                .expect("log always has a block number; qed")
                .as_u64(),
            l1_batch_number: log.l1_batch_number.unwrap(),
            l2_message_index,
            l2_tx_number_in_block: l1_batch_tx_id.unwrap().as_u32() as u16,
            message,
            sender,
            proof,
        }))
    }

    async fn get_withdrawal_log(
        &self,
        tx_hash: H256,
        index: usize,
    ) -> Result<Option<(ZKSLog, Option<U64>)>> {
        let receipt = self.zks_get_transaction_receipt(tx_hash).await?;

        let log = receipt
            .logs
            .into_iter()
            .filter(|entry| {
                entry.address == L1_MESSENGER_ADDRESS
                    && entry.topics[0] == L1MessageSentFilter::signature()
            })
            .nth(index);

        let log = match log {
            Some(log) => log,
            None => return Ok(None),
        };

        Ok(Some((log, receipt.l1_batch_tx_index)))
    }

    async fn get_withdrawal_l2_to_l1_log(
        &self,
        tx_hash: H256,
        index: usize,
    ) -> Result<Option<(L2ToL1Log, Option<U64>)>> {
        let receipt = self.zks_get_transaction_receipt(tx_hash).await?;

        let log = receipt
            .l2_to_l1_logs
            .into_iter()
            .filter(|entry| entry.sender == L1_MESSENGER_ADDRESS)
            .nth(index);

        let log = match log {
            Some(log) => log,
            None => return Ok(None),
        };

        Ok(Some((log, Some(U64::from(index)))))
    }
}

/// Check if the withdrawal is finalized on L1.
pub async fn is_withdrawal_finalized<'a, M1, M2>(
    withdrawal_hash: H256,
    index: usize,
    sender: Address,
    zksync_contract: &'a IZkSync<M1>,
    l1_bridge: &'a IL1Bridge<M1>,
    l2_middleware: &'a M2,
) -> Result<bool>
where
    M1: Middleware + 'a,
    <M1 as Middleware>::Provider: JsonRpcClient,
    M2: ZksyncMiddleware,
    <M2 as Middleware>::Provider: JsonRpcClient,
{
    let log = match l2_middleware
        .get_withdrawal_log(withdrawal_hash, index)
        .await?
    {
        Some(log) => log,
        None => return Ok(false),
    };

    let (_, l2_to_l1_log_index) = match l2_middleware
        .get_withdrawal_l2_to_l1_log(withdrawal_hash, index)
        .await?
    {
        Some(log) => log,
        None => return Ok(false),
    };

    let proof = match l2_middleware
        .get_log_proof(withdrawal_hash, l2_to_l1_log_index.map(|idx| idx.as_u64()))
        .await?
    {
        Some(proof) => proof,
        None => return Ok(false),
    };

    let l2_message_index = proof.id;
    let l1_batch_number = match log.0.l1_batch_number {
        Some(b) => b.as_u64().into(),
        None => return Ok(false),
    };

    if is_eth(sender) {
        let is_finalized = zksync_contract
            .is_eth_withdrawal_finalized(l1_batch_number, l2_message_index.into())
            .call()
            .await?;

        Ok(is_finalized)
    } else {
        let is_finalized = l1_bridge
            .is_withdrawal_finalized(l1_batch_number, l2_message_index.into())
            .call()
            .await?;

        Ok(is_finalized)
    }
}
