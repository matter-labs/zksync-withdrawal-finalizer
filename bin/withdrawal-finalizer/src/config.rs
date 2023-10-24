use std::str::FromStr;

use envconfig::Envconfig;
use ethers::types::Address;
use serde::Deserialize;
use url::Url;

#[derive(Deserialize, Debug)]
pub struct TokenList(pub Vec<Address>);

impl FromStr for TokenList {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let res = serde_json::from_str(s)?;
        Ok(TokenList(res))
    }
}

/// Withdrawal finalizer configuration.
///
/// Can be read from
/// * `env` via [`Self::init_from_env()`]
/// * TOML config file via [`Self::from_file()`]
#[derive(Deserialize, Debug, Envconfig)]
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

    #[envconfig(from = "TOKEN_WHITELIST")]
    pub token_whitelist: Option<TokenList>,

    #[envconfig(from = "TOKEN_BLACKLIST")]
    pub token_blacklist: Option<TokenList>,
}
