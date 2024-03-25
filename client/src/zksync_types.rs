//! This is a copy-paste of necessary types from `zksync-2-dev`
//! repository to avoid having a dependency on it.
//!
//! These are strictly the types that are necessary for
//! interacting with the RPC endpoints.

use chrono::{DateTime, Utc};
use ethers::{
    abi::RawLog,
    types::{Address, Bloom, Bytes, H160, H256, U256, U64},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[allow(missing_docs)]
pub struct BaseSystemContractsHashes {
    pub bootloader: H256,
    pub default_aa: H256,
}

/// A struct with the proof for the L2 to L1 log in a specific block.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct L2ToL1LogProof {
    /// The merkle path for the leaf.
    pub proof: Vec<H256>,
    /// The id of the leaf in a tree.
    pub id: u32,
    /// The root of the tree.
    pub root: H256,
}

/// A transaction receipt in `zksync` network.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransactionReceipt {
    /// Transaction hash.
    #[serde(rename = "transactionHash")]
    pub transaction_hash: H256,
    /// Index within the block.
    #[serde(rename = "transactionIndex")]
    pub transaction_index: Index,
    /// Hash of the block this transaction was included within.
    #[serde(rename = "blockHash")]
    pub block_hash: Option<H256>,
    /// Number of the miniblock this transaction was included within.
    #[serde(rename = "blockNumber")]
    pub block_number: Option<U64>,
    /// Index of transaction in l1 batch
    #[serde(rename = "l1BatchTxIndex")]
    pub l1_batch_tx_index: Option<Index>,
    /// Number of the l1 batch this transaction was included within.
    #[serde(rename = "l1BatchNumber")]
    pub l1_batch_number: Option<U64>,
    /// Sender
    /// Note: default address if the client did not return this value
    /// (maintains backwards compatibility for `<= 0.7.0` when this field was missing)
    #[serde(default)]
    pub from: Address,
    /// Recipient (None when contract creation)
    /// Note: Also `None` if the client did not return this value
    /// (maintains backwards compatibility for `<= 0.7.0` when this field was missing)
    #[serde(default)]
    pub to: Option<Address>,
    /// Cumulative gas used within the block after this was executed.
    #[serde(rename = "cumulativeGasUsed")]
    pub cumulative_gas_used: U256,
    /// Gas used by this transaction alone.
    ///
    /// Gas used is `None` if the the client is running in light client mode.
    #[serde(rename = "gasUsed")]
    pub gas_used: Option<U256>,
    /// Contract address created, or `None` if not a deployment.
    #[serde(rename = "contractAddress")]
    pub contract_address: Option<Address>,
    /// Logs generated within this transaction.
    pub logs: Vec<Log>,
    /// L2 to L1 logs generated within this transaction.
    #[serde(rename = "l2ToL1Logs")]
    pub l2_to_l1_logs: Vec<L2ToL1Log>,
    /// Status: either 1 (success) or 0 (failure).
    pub status: Option<U64>,
    /// State root.
    pub root: Option<H256>,
    /// Logs bloom
    #[serde(rename = "logsBloom")]
    pub logs_bloom: Bloom,
    /// Transaction type, Some(1) for AccessList transaction, None for Legacy
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub transaction_type: Option<U64>,
    /// Effective gas price
    #[serde(rename = "effectiveGasPrice")]
    pub effective_gas_price: Option<U256>,
}

/// Index in block
pub type Index = U64;

/// A log produced by a transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Log {
    /// H160
    pub address: H160,
    /// Topics
    pub topics: Vec<H256>,
    /// Data
    pub data: Bytes,
    /// Block Hash
    #[serde(rename = "blockHash")]
    pub block_hash: Option<H256>,
    /// Block Number
    #[serde(rename = "blockNumber")]
    pub block_number: Option<U64>,
    /// L1 batch number the log is included in.
    #[serde(rename = "l1BatchNumber")]
    pub l1_batch_number: Option<U64>,
    /// Transaction Hash
    #[serde(rename = "transactionHash")]
    pub transaction_hash: Option<H256>,
    /// Transaction Index
    #[serde(rename = "transactionIndex")]
    pub transaction_index: Option<Index>,
    /// Log Index in Block
    #[serde(rename = "logIndex")]
    pub log_index: Option<U256>,
    /// Log Index in Transaction
    #[serde(rename = "transactionLogIndex")]
    pub transaction_log_index: Option<U256>,
    /// Log Type
    #[serde(rename = "logType")]
    pub log_type: Option<String>,
    /// Removed
    pub removed: Option<bool>,
}

impl Log {
    /// Returns true if the log has been removed.
    pub fn is_removed(&self) -> bool {
        if let Some(val_removed) = self.removed {
            return val_removed;
        }

        if let Some(ref val_log_type) = self.log_type {
            if val_log_type == "removed" {
                return true;
            }
        }
        false
    }
}

impl From<Log> for RawLog {
    fn from(val: Log) -> Self {
        (val.topics, val.data.to_vec()).into()
    }
}

/// A log produced by a transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(missing_docs)]
pub struct L2ToL1Log {
    pub block_hash: Option<H256>,
    pub block_number: U64,
    pub l1_batch_number: Option<U64>,
    pub log_index: U256,
    pub transaction_index: Index,
    pub transaction_hash: H256,
    pub transaction_log_index: U256,
    pub shard_id: U64,
    pub is_service: bool,
    pub sender: Address,
    pub key: H256,
    pub value: H256,
}

/// Withdrawal event struct
#[derive(Debug)]
pub struct WithdrawalEvent {
    /// A hash of the transaction of this withdrawal.
    pub tx_hash: H256,

    /// Number of the L2 block this withdrawal happened in.
    pub block_number: u64,

    /// Address of the transferred token
    pub token: Address,

    /// The amount transferred.
    pub amount: U256,

    /// Address on L1 that will receive this withdrawal.
    pub l1_receiver: Option<Address>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(missing_docs)]
pub enum BlockStatus {
    Sealed,
    Verified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(missing_docs)]
pub struct BlockDetails {
    pub number: u32,
    pub l1_batch_number: u32,
    pub timestamp: u64,
    pub l1_tx_count: usize,
    pub l2_tx_count: usize,
    pub root_hash: Option<H256>,
    pub status: BlockStatus,
    pub commit_tx_hash: Option<H256>,
    pub committed_at: Option<DateTime<Utc>>,
    pub prove_tx_hash: Option<H256>,
    pub proven_at: Option<DateTime<Utc>>,
    pub execute_tx_hash: Option<H256>,
    pub executed_at: Option<DateTime<Utc>>,
    pub l1_gas_price: u64,
    pub l2_fair_gas_price: u64,
    pub base_system_contracts_hashes: BaseSystemContractsHashes,
    pub operator_address: Address,
}

/// Token in the zkSync network
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(missing_docs)]
pub struct Token {
    pub l1_address: Address,
    pub l2_address: Address,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}
