#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Interactions with zkSync on-chain contracts.

mod error;

use std::sync::Arc;
use std::{num::NonZeroUsize, time::Instant};

use chrono::{Datelike, LocalResult, TimeZone, Utc};
pub use error::{Error, Result};

use async_trait::async_trait;
use auto_impl::auto_impl;
use ethers::types::BlockNumber;
use ethers::{
    abi::{AbiDecode, AbiEncode, ParamType, RawLog, Token},
    contract::{EthCall, EthEvent, EthLogDecode},
    providers::{JsonRpcClient, Middleware, Provider},
    types::{transaction::eip2718::TypedTransaction, Address, Bytes, H160, H256, U256, U64},
};

use ethers_log_decode::EthLogDecode;
use ethtoken::codegen::WithdrawalFilter;
use l1bridge::codegen::{FinalizeWithdrawalCall, IL1Bridge};
use l1messenger::codegen::L1MessageSentFilter;
use l2standard_token::codegen::{BridgeBurnFilter, L1AddressCall};
use lazy_static::lazy_static;
use lru::LruCache;
use tokio::sync::Mutex;
use withdrawal_finalizer::codegen::RequestFinalizeWithdrawal;
use zksync_contract::codegen::{FinalizeEthWithdrawalCall, IZkSync};
use zksync_types::{
    BlockDetails, L2ToL1Log, L2ToL1LogProof, Log as ZKSLog,
    TransactionReceipt as ZksyncTransactionReceipt,
};

pub use zksync_contract::BlockEvent;
pub use zksync_types::WithdrawalEvent;

use crate::l2bridge::codegen::WithdrawalInitiatedFilter;

/// Eth token address
pub const ETH_TOKEN_ADDRESS: Address = H160([
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x80, 0x0a,
]);

/// Eth address
pub const ETH_ADDRESS: Address = Address::zero();

/// Address of Ethereum L1 messenger
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

#[derive(EthLogDecode)]
enum WithdrawalEvents {
    BridgeBurn(BridgeBurnFilter),
    Withdrawal(WithdrawalFilter),
}

lazy_static! {
    static ref TOKEN_ADDRS: Arc<Mutex<LruCache<Address, Address>>> =
        Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(64).unwrap())));
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

/// A key that uniquely identifies each withdrawal
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct WithdrawalKey {
    /// A transaction in which the withdrawal has happened
    pub tx_hash: H256,

    /// Event index of withdrawal within the transaction
    pub event_index_in_tx: u32,
}

/// Withdrawal parameters
#[derive(Debug, Clone)]
pub struct WithdrawalParams {
    /// Hash of the withdrawal transaction.
    pub tx_hash: H256,

    /// Event index in the transaction.
    pub event_index_in_tx: u32,

    /// ID serial number.
    ///
    /// A monotonically increasing counter for every withdrawal.
    pub id: u64,

    /// Block number on L2 withdrawal transaction happened in.
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

impl WithdrawalParams {
    /// Get the key of this withdrawal.
    pub fn key(&self) -> WithdrawalKey {
        WithdrawalKey {
            tx_hash: self.tx_hash,
            event_index_in_tx: self.event_index_in_tx,
        }
    }
}

/// A middleware for interacting with zkSync node.
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
    ///
    /// # Arguments
    ///
    /// * `withdrawal_hash`: Hash of the TX in which withdrawal event was emitted
    /// * `index`: Index of the withdrawal event in transaction.
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
        let receipt = self.zks_get_transaction_receipt(withdrawal_hash).await?;

        let withdrawal_log = receipt
            .logs
            .iter()
            .filter(|log| {
                log.topics[0] == BridgeBurnFilter::signature()
                    || log.topics[0] == WithdrawalFilter::signature()
            })
            .nth(index)
            .ok_or(Error::WithdrawalLogNotFound(index, withdrawal_hash))?;

        let raw_log: RawLog = withdrawal_log.clone().into();
        let withdrawal_event = WithdrawalEvents::decode_log(&raw_log)?;

        let l2_to_l1_message_hash = match withdrawal_event {
            WithdrawalEvents::BridgeBurn(b) => {
                let mut addr_lock = TOKEN_ADDRS.lock().await;

                let l1_address =
                    if let Some(l1_address) = addr_lock.get(&withdrawal_log.address).cloned() {
                        l1_address
                    } else {
                        // Send manually the call to the erc20 token address to call `l1Address`.
                        // Manual call has to be done instead of `abigen`-generated typesafe one
                        // since it is impossible to wrap a reference to `self` into the `Arc`.
                        let l1_address_call = L1AddressCall;
                        let mut call = TypedTransaction::default();

                        call.set_to(withdrawal_log.address);
                        call.set_data(l1_address_call.encode().into());

                        let l1_address = Address::decode(self.call(&call, None).await?)?;

                        addr_lock.put(withdrawal_log.address, l1_address);

                        l1_address
                    };
                drop(addr_lock);

                // Get the `l1_receiver` address that receives the withdrawal on L1;
                // it is available only in the `WithdrawalInitiatedFilter` event, look for it.
                let withdrawal_initiated_event = receipt
                    .logs
                    .iter()
                    .filter_map(|log| {
                        let raw_log: RawLog = log.clone().into();
                        <WithdrawalInitiatedFilter as EthEvent>::decode_log(&raw_log).ok()
                    })
                    .nth(index)
                    .ok_or(Error::WithdrawalInitiatedFilterNotFound(
                        withdrawal_hash,
                        index,
                    ))?;

                let l1_receiver = withdrawal_initiated_event.l_1_receiver;

                get_l1_bridge_burn_message_keccak(&b, l1_receiver, l1_address)?
            }
            WithdrawalEvents::Withdrawal(w) => get_l1_withdraw_message_keccak(&w)?,
        };

        let l2_to_l1_log_index = receipt
            .l2_to_l1_logs
            .iter()
            .position(|l| l.value == l2_to_l1_message_hash)
            .ok_or(Error::L2ToL1WithValueNotFound(
                withdrawal_hash,
                l2_to_l1_message_hash,
            ))?;

        let l1_batch_tx_id = receipt.l1_batch_tx_index;
        let log = receipt
            .logs
            .into_iter()
            .filter(|entry| {
                entry.address == L1_MESSENGER_ADDRESS
                    && entry.topics[0] == L1MessageSentFilter::signature()
            })
            .nth(index)
            .ok_or(Error::L1MessageSentNotFound(withdrawal_hash, index))?;

        let sender = log.topics[1].into();

        let proof = self
            .get_log_proof(withdrawal_hash, Some(l2_to_l1_log_index as u64))
            .await?
            .expect("Log proof should be present. qed");

        let message: Bytes = match ethers::abi::decode(&[ParamType::Bytes], &log.data)
            .expect("log data is valid rlp data; qed")
            .swap_remove(0)
        {
            Token::Bytes(b) => b.into(),
            b => return Err(Error::MessageNotRlpBytes(format!("{b:?}"))),
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
            id: 0,
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

/// Get the first block mined today by UTC time
///
/// # Arguments
/// * `at_date_time`: Look for the block at this date-time
/// * `start_from_block`: Will perform search starting from this block,
///    if `None` is specified then searches from block 1.
pub async fn get_first_block_today<M: Middleware>(
    start_from_block: Option<U64>,
    middleware: M,
) -> Result<Option<U64>> {
    let now = Utc::now();

    let todays_midnight = match Utc.with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0) {
        LocalResult::None => {
            // vlog::error!("could not compute `with_ymd_and_hms` from todays date");
            return Err(Error::TimeConversion);
        }
        LocalResult::Single(s) | LocalResult::Ambiguous(s, _) => s,
    };

    get_block_number_by_timestamp(todays_midnight, start_from_block, middleware).await
}

/// Get the block number by timestamp
///
/// # Arguments
/// * `at_date_time`: Look for the block at this date-time
/// * `start_from_block`: Will perform search starting from this block,
///    if `None` is specified then searches from block 1.
/// * `middleware`: The client to perform requests to RPC with.
pub async fn get_block_number_by_timestamp<M: Middleware>(
    at_date: chrono::DateTime<chrono::offset::Utc>,
    start_from_block: Option<U64>,
    middleware: M,
) -> Result<Option<U64>> {
    let start_from_block = start_from_block.unwrap_or(1_u64.into());

    let right_block = match middleware
        .get_block(BlockNumber::Latest)
        .await
        .map_err(|e| Error::Middleware(format!("{e}")))?
    {
        Some(r) => r,
        None => return Ok(None),
    };

    let mut right = right_block
        .number
        .ok_or(Error::BlockHasNoNumber(right_block.parent_hash))?;

    let mut left = start_from_block;

    if at_date > right_block.time()? {
        return Ok(None);
    }

    let mut middle = left + (right - left) / 2;

    while left < right {
        middle = left + (right - left) / 2;

        let middle_block = middleware
            .get_block(BlockNumber::Number(middle))
            .await
            .map_err(|e| Error::Middleware(format!("{e}")))?
            .ok_or(Error::BlockHasNoNumber(right_block.parent_hash))?;

        let middle_block_timestamp = middle_block.time()?;

        let signed_duration_since_requested_timestamp =
            middle_block_timestamp.signed_duration_since(at_date);

        let num_milliseconds = signed_duration_since_requested_timestamp.num_milliseconds();

        // look within the 500ms margin to the right of the given date.
        if (0..500).contains(&num_milliseconds) {
            return Ok(Some(middle));
        } else if num_milliseconds.is_positive() {
            right = middle;
        } else {
            left = middle;
        }
    }

    Ok(Some(middle))
}

fn get_l1_bridge_burn_message_keccak(
    burn: &BridgeBurnFilter,
    l1_receiver: Address,
    l1_token: Address,
) -> Result<H256> {
    let message = ethers::abi::encode_packed(&[
        Token::FixedBytes(FinalizeWithdrawalCall::selector().to_vec()),
        Token::Address(l1_receiver),
        Token::Address(l1_token),
        Token::Bytes(burn.amount.encode()),
    ])?;
    Ok(ethers::utils::keccak256(message).into())
}

fn get_l1_withdraw_message_keccak(withdraw: &WithdrawalFilter) -> Result<H256> {
    let message = ethers::abi::encode_packed(&[
        Token::FixedBytes(FinalizeEthWithdrawalCall::selector().to_vec()),
        Token::Address(withdraw.l_1_receiver),
        Token::Bytes(withdraw.amount.encode()),
    ])?;

    Ok(ethers::utils::keccak256(message).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // https://goerli.explorer.zksync.io/tx/0x4E322DB1BE846FB046CBDEC53FE0D1D09ADDE726990AE776B1EE4043F2DBF79F
    #[test]
    fn bridge_burn_correctly_encodes_to_message() {
        let dai_l1_addr: Address = "0x5C221E77624690FFF6DD741493D735A17716C26B"
            .parse()
            .unwrap();

        let bridge_burn = BridgeBurnFilter {
            account: "f1e7d54cd9cc2a4aea139305addcd36bb7d45ddf".parse().unwrap(),
            amount: "0x00000000000000000000000000000000000000000000001043561a8829300000"
                .parse()
                .unwrap(),
        };

        let a = super::get_l1_bridge_burn_message_keccak(
            &bridge_burn,
            // withdraws to the same account on L1, so can take this value.
            bridge_burn.account,
            dai_l1_addr,
        )
        .unwrap();

        assert_eq!(
            hex::encode(a),
            "2c634ea4538cf0fe4e8a9ccde494271fc484c117c60cf11694ea0c610dc9257c"
        );
    }

    // https://goerli.explorer.zksync.io/tx/0x0089EA26FC0DDA7016D893F669E18299EF56055B3BB0418B2C4DD241301B513A
    #[test]
    fn withdrawal_correctly_encodes_to_message() {
        let addr = "0x3827c65A7F9D0dB023Ac10E0fA81D8D2cd992A81"
            .parse()
            .unwrap();

        let withdrawal = WithdrawalFilter {
            l_2_sender: addr,
            l_1_receiver: addr,
            amount: "0x000000000000000000000000000000000000000000000000000003a352944000"
                .parse()
                .unwrap(),
        };

        let a = super::get_l1_withdraw_message_keccak(&withdrawal).unwrap();

        assert_eq!(
            hex::encode(a),
            "4a4c388f10244d8c96b8723aa654231ae43eed5bc382d46a803937421923414e"
        );
    }

    // https://goerli.explorer.zksync.io/tx/0xe423e38d66b8ad79c963a6855488f6f3e9eae907ce30d09fd1fb39a0c9631420
    #[test]
    fn bridge_burn_correctly_encodes_to_message_with_different_l1_address() {
        let dai_l1_addr: Address = "0x5C221E77624690FFF6DD741493D735A17716C26B"
            .parse()
            .unwrap();

        let bridge_burn = BridgeBurnFilter {
            account: "769F2B14f36E248F3D9A7151a7F0e8A3D0903dF5".parse().unwrap(),
            amount: "0x0000000000000000000000000000000000000000000000d8d726b7177a800000"
                .parse()
                .unwrap(),
        };

        let a = super::get_l1_bridge_burn_message_keccak(
            &bridge_burn,
            "d1ced2fBDFa24daa9920D37237ca2D4d5616a6e2".parse().unwrap(),
            dai_l1_addr,
        )
        .unwrap();

        assert_eq!(
            hex::encode(a),
            "70fb2e243d0ec70adf97f4941ca257a34f01d39f5ee20b4b1795304d656a9751"
        );
    }
}
