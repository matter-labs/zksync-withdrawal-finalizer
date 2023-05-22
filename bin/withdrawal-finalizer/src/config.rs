#![allow(unused)]
use std::{env, fs, path::Path, str::FromStr};

use envconfig::Envconfig;
use ethers::types::{Address, H160, U256};
use serde::Deserialize;
use url::Url;

use crate::error::Error;

/// A list of tokens to process.
///
/// The sole purpose of this newtype is `FromStr` implementation that
/// reads from a string of comma-separated addresses.
#[derive(serde::Deserialize, Default, Debug)]
pub struct TokensToProcess(pub Vec<Address>);

impl FromStr for TokensToProcess {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut res = vec![];
        for token in s.split(',') {
            let address = Address::from_str(token).map_err(|_| ())?;
            res.push(address);
        }

        Ok(TokensToProcess(res))
    }
}

/// Withdrawal finalizer configuration.
///
/// Can be read from
/// * `env` via [`Self::init_from_env()`]
/// * TOML config file via [`Self::from_file()`]
#[derive(Deserialize, Debug, Envconfig)]
pub struct Config {
    /// A list of L1 tokens to process.
    #[envconfig(from = "WITHDRAWAL_FINALIZER_L1_TOKENS")]
    pub l1_tokens_to_process: Option<TokensToProcess>,

    /// Private key of the finalizer account.
    #[envconfig(from = "WITHDRAWAL_FINALIZER_ACCOUNT_PRIVATE_KEY")]
    pub account_private_key: String,

    /// L1 RPC url.
    #[envconfig(from = "ETH_CLIENT_WEB3_URL")]
    pub l1_web3_url: Url,

    /// L1 WS url.
    pub l1_ws_url: Option<Url>,

    /// L2 RPC url.
    #[envconfig(from = "API_WEB3_JSON_RPC_HTTP_URL")]
    pub zksync_web3_url: Url,

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
    pub main_contract: Address,

    /// L2 WS Endpoint
    #[envconfig(from = "API_WEB3_JSON_RPC_WS_URL")]
    pub zk_server_ws_url: Url,

    #[envconfig(from = "API_WEB3_JSON_RPC_HTTP_URL")]
    pub zk_server_http_url: Option<Url>,

    #[envconfig(from = "ETHERSCAN_TOKEN")]
    pub etherscan_token: Option<String>,

    #[envconfig(from = "CONTRACTS_WITHDRAWAL_FINALIZER_ADDRESS")]
    pub withdrawal_finalizer_contract: Address,

    #[envconfig(from = "WITHDRAWAL_FINALIZER_ETH_ADDRESS")]
    pub withdrawal_finalizer_eth_address: Address,

    #[envconfig(from = "GAS_LIMIT")]
    pub one_withdrawal_gas_limit: U256,

    #[envconfig(from = "BATCH_FINALIZATION_GAS_LIMIT")]
    pub batch_finalization_gas_limit: U256,

    #[envconfig(from = "DATABASE_URL")]
    pub database_url: Url,
}

impl Config {
    pub fn get_tokens(&mut self, network: &str) -> Result<(), Error> {
        let zksync_home = env::var("ZKSYNC_HOME").map_err(|_| Error::NoZkSyncHome)?;

        let tokens = std::fs::read_to_string(format!("{zksync_home}/etc/tokens/{network}.json"))?;

        let mut l1_tokens_to_process = self.l1_tokens_to_process.take().unwrap_or_default();

        for addr in serde_json::from_str::<Vec<TokenConfig>>(&tokens)?
            .into_iter()
            .map(|t| t.address)
        {
            l1_tokens_to_process.0.push(addr);
        }

        self.l1_tokens_to_process = Some(l1_tokens_to_process);

        Ok(())
    }

    pub fn from_file<P: AsRef<Path>>(config_path: P) -> Result<Self, Error> {
        let contents = fs::read_to_string(config_path)?;

        let config: Config = toml::from_str(&contents)?;

        Ok(config)
    }

    pub fn l1_tokens_to_process(&self) -> Option<&[H160]> {
        self.l1_tokens_to_process.as_ref().map(|f| f.0.as_ref())
    }
}

#[derive(Debug, Deserialize)]
struct TokenConfig {
    name: String,
    symbol: String,
    decimals: usize,
    address: Address,
}
