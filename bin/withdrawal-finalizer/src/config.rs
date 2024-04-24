use std::str::FromStr;

use envconfig::Envconfig;
use ethers::types::Address;
use finalizer::AddrList;
use serde::{Deserialize, Serialize};
use url::Url;

/// Withdrawal finalizer configuration.
///
/// Can be read from
/// * `env` via [`Self::init_from_env()`]
/// * TOML config file via [`Self::from_file()`]
#[derive(Debug, Envconfig)]
pub struct Config {
    /// L1 WS url.
    #[envconfig(from = "ETH_CLIENT_WS_URL")]
    pub eth_client_ws_url: Url,

    /// L1 HTTP url.
    #[envconfig(from = "ETH_CLIENT_HTTP_URL")]
    pub eth_client_http_url: Url,

    /// Address of the `L1Bridge` contract.
    #[envconfig(from = "CONTRACTS_L1_ERC20_BRIDGE_PROXY_ADDR")]
    pub l1_erc20_bridge_proxy_addr: Address,

    /// Address of the `L2ERC20Bridge` contract.
    #[envconfig(from = "CONTRACTS_L2_ERC20_BRIDGE_ADDR")]
    pub l2_erc20_bridge_addr: Address,

    /// Main contract
    #[envconfig(from = "CONTRACTS_DIAMOND_PROXY_ADDR")]
    pub diamond_proxy_addr: Address,

    /// Finalizer contract
    #[envconfig(from = "CONTRACTS_WITHDRAWAL_FINALIZER_CONTRACT")]
    pub withdrawal_finalizer_addr: Address,

    /// L2 WS Endpoint
    #[envconfig(from = "API_WEB3_JSON_RPC_WS_URL")]
    pub api_web3_json_rpc_ws_url: Url,

    /// L2 HTTP Endpoint
    #[envconfig(from = "API_WEB3_JSON_RPC_HTTP_URL")]
    pub api_web3_json_rpc_http_url: Url,

    #[envconfig(from = "DATABASE_URL")]
    pub database_url: Url,

    #[envconfig(from = "START_FROM_L2_BLOCK")]
    pub start_from_l2_block: Option<u64>,

    #[envconfig(from = "UPDATER_BACKOFF")]
    pub updater_backoff: Option<u64>,

    #[envconfig(from = "GAS_LIMIT")]
    pub one_withdrawal_gas_limit: String,

    #[envconfig(from = "BATCH_FINALIZATION_GAS_LIMIT")]
    pub batch_finalization_gas_limit: String,

    #[envconfig(from = "WITHDRAWAL_FINALIZER_ACCOUNT_PRIVATE_KEY")]
    pub account_private_key: String,

    #[envconfig(from = "TX_RETRY_TIMEOUT_SECS")]
    pub tx_retry_timeout: usize,

    #[envconfig(from = "FINALIZE_ETH_TOKEN")]
    pub finalize_eth_token: Option<bool>,

    #[envconfig(from = "CUSTOM_TOKEN_DEPLOYER_ADDRESSES")]
    pub custom_token_deployer_addresses: Option<AddrList>,

    #[envconfig(from = "CUSTOM_TOKEN_ADDRESSES")]
    pub custom_token_addresses: Option<AddrList>,

    #[envconfig(from = "ENABLE_WITHDRAWAL_METERING")]
    pub enable_withdrawal_metering: Option<bool>,

    #[envconfig(from = "CUSTOM_TOKEN_ADDRESS_MAPPINGS")]
    pub custom_token_address_mappings: Option<CustomTokenAddressMappings>,

    #[envconfig(from = "ETH_FINALIZATION_THRESHOLD")]
    pub eth_finalization_threshold: Option<String>,

    #[envconfig(from = "ONLY_L1_RECIPIENTS")]
    pub only_l1_recipients: Option<AddrList>,

    /// Only finalize these tokens specified by their L2 addresses
    #[envconfig(from = "ONLY_FINALIZE_THESE_TOKENS")]
    pub only_finalize_these_tokens: Option<AddrList>,
}

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
pub struct CustomTokenAddressMapping {
    pub l_1_addr: Address,
    pub l_2_addr: Address,
}

impl FromStr for CustomTokenAddressMapping {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CustomTokenAddressMappings(pub Vec<CustomTokenAddressMapping>);

impl FromStr for CustomTokenAddressMappings {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let res: CustomTokenAddressMappings = serde_json::from_str(s)?;

        Ok(res)
    }
}

impl Config {
    /// Returns a mapping of tokens (L1, L2) addresses.
    pub fn token_mappings(&self) -> Vec<(Address, Address)> {
        self.custom_token_address_mappings
            .as_ref()
            .map(|f| &f.0)
            .unwrap_or(&vec![])
            .iter()
            .map(|m| (m.l_1_addr, m.l_2_addr))
            .collect()
    }
}
