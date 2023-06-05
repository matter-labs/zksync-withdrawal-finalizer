#![allow(unused)]
use std::{env, fs, path::Path, str::FromStr};

use envconfig::Envconfig;
use ethers::types::{Address, H160, U256};
use serde::Deserialize;
use url::Url;

/// Withdrawal finalizer configuration.
///
/// Can be read from
/// * `env` via [`Self::init_from_env()`]
/// * TOML config file via [`Self::from_file()`]
#[derive(Deserialize, Debug, Envconfig)]
pub struct Config {
    /// L1 WS url.
    pub l1_ws_url: Url,

    /// Sentry url.
    #[envconfig(from = "SENTRY_URL")]
    pub sentry_url: Option<Url>,

    /// Address of the `L1Bridge` contract.
    // TODO: #[envconfig(from = "CONTRACTS_L1_ETH_BRIDGE_ADDR")]
    #[envconfig(from = "CONTRACTS_L1_ERC20_BRIDGE_PROXY_ADDR")]
    pub l1_eth_bridge_addr: Address,

    /// Address of the
    #[envconfig(from = "CONTRACTS_L1_ERC20_BRIDGE_IMPL_ADDR")]
    pub l1_erc20_bridge_addr: Address,

    /// Address of the `L2ERC20Bridge` contract.
    #[envconfig(from = "CONTRACTS_L2_ERC20_BRIDGE_ADDR")]
    pub l2_erc20_bridge_addr: Address,

    /// Main contract
    #[envconfig(from = "CONTRACTS_DIAMOND_PROXY_ADDR")]
    pub main_zksync_contract: Address,

    /// The timelock address
    #[envconfig(from = "VALIDATOR_TIMELOCK_ADDR")]
    pub timelock_address: Address,

    /// L2 WS Endpoint
    #[envconfig(from = "API_WEB3_JSON_RPC_WS_URL")]
    pub zk_server_ws_url: Url,

    #[envconfig(from = "API_WEB3_JSON_RPC_HTTP_URL")]
    pub zk_server_http_url: Option<Url>,

    #[envconfig(from = "GAS_LIMIT")]
    pub one_withdrawal_gas_limit: U256,

    #[envconfig(from = "BATCH_FINALIZATION_GAS_LIMIT")]
    pub batch_finalization_gas_limit: U256,

    #[envconfig(from = "DATABASE_URL")]
    pub database_url: Url,

    pub start_from_l2_block: Option<u64>,

    #[envconfig(from = "UPDATER_BACKOFF")]
    pub updater_backoff: Option<u64>,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(config_path: P) -> eyre::Result<Self> {
        let contents = fs::read_to_string(config_path)?;

        let config: Config = toml::from_str(&contents)?;

        Ok(config)
    }
}
