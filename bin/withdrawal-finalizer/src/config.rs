#![allow(dead_code)]
use std::{env, fs, path::Path, str::FromStr};

use crate::error::Error;
use ethers::types::{Address, H160};
use serde::Deserialize;
use url::Url;

const ACCOUNT_PRIVATE_KEY: &str = "WITHDRAWAL_FINALIZER_ACCOUNT_PRIVATE_KEY";
const L1_TOKENS: &str = "WITHDRAWAL_FINALIZER_L1_TOKENS";
const CHAIN_ETH_NETWORK: &str = "CHAIN_ETH_NETWORK";
const API_WEB3_JSON_RPC_HTTP_URL: &str = "API_WEB3_JSON_RPC_HTTP_URL";
const ETHERSCAN_TOKEN: &str = "ETHERSCAN_TOKEN";
const ZKSYNC_NETWORK: &str = "ZKSYNC_NETWORK";
const SENTRY_URL: &str = "WITHDRAWAL_FINALIZER_SENTRY_URL";
const CONTRACTS_WITHDRAWAL_FINALIZER_ADDRESS: &str = "CONTRACTS_WITHDRAWAL_FINALIZER_ADDRESS";
const BATCH_FINALIZATION_GAS_LIMIT: &str = "BATCH_FINALIZATION_GAS_LIMIT";
const ONE_WITHDRAWAL_GAS_LIMIT: &str = "GAS_LIMIT";
const FETCH_DATA_ATTEMPTS: &str = "FETCH_DATA_ATTEMPTS";
const SUBMIT_REQUEST_ATTEMPTS: &str = "SUBMIT_REQUEST_ATTEMPTS";
const FINALIZE_WITHDRAWAL_FLOW_ATTEMPTS: &str = "FINALIZE_WITHDRAWAL_FLOW_ATTEMPTS";
const SLEEP_TIME: &str = "SLEEP_TIME";
const SENT_ETH_TX_MINING_TIMEOUT_IN_MS: &str = "SENT_ETH_TX_MINING_TIMEOUT_IN_MS";
const START_FROM_BLOCK: &str = "WITHDRAWAL_FINALIZER_START_FROM_BLOCK";
const PROCESSING_BLOCK_OFFSET: &str = "WITHDRAWAL_FINALIZER_PROCESSING_BLOCK_OFFSET";
const ETH_CLIENT_WEB3_URL: &str = "ETH_CLIENT_WEB3_URL";

#[derive(Deserialize, Debug)]
pub(crate) struct Config {
    l1_tokens_to_process: Vec<Address>,
    account_private_key: String,
    l1_web3_url: Url,
    zksync_web3_url: Url,
    sentry_url: Option<Url>,
}

impl Config {
    fn get_tokens(network: &str) -> Result<Vec<Address>, Error> {
        let zksync_home = env::var("ZKSYNC_HOME").map_err(|_| Error::NoZkSyncHome)?;

        let tokens = std::fs::read_to_string(format!("{zksync_home}/etc/tokens/{network}.json"))?;

        Ok(serde_json::from_str::<Vec<TokenConfig>>(&tokens)?
            .into_iter()
            .map(|t| t.address)
            .collect())
    }

    pub(crate) fn from_file<P: AsRef<Path>>(config_path: P) -> Result<Self, Error> {
        let contents = fs::read_to_string(config_path)?;

        let config: Config = toml::from_str(&contents)?;

        Ok(config)
    }

    pub(crate) fn from_env(is_localhost: bool) -> Result<Self, Error> {
        let mut l1_tokens_to_process: Vec<_> = match env::var(L1_TOKENS) {
            Ok(tokens) => {
                let mut res = vec![];
                for token in tokens.split(',') {
                    let address = Address::from_str(token)?;
                    res.push(address);
                }
                res
            }
            Err(_) => vec![],
        };

        let account_private_key = env::var(ACCOUNT_PRIVATE_KEY).unwrap_or_default();

        if is_localhost {
            l1_tokens_to_process.extend(Self::get_tokens("localhost")?.into_iter());
        }
        let l1_web3_url = env::var(ETH_CLIENT_WEB3_URL).map_err(|_| Error::NoL1Web3Url)?;

        let l1_web3_url = Url::parse(&l1_web3_url)?;

        let zksync_web3_url =
            env::var(API_WEB3_JSON_RPC_HTTP_URL).map_err(|_| Error::NoZkSyncWeb3Url)?;

        let zksync_web3_url = Url::parse(&zksync_web3_url)?;

        Ok(Self {
            l1_tokens_to_process,
            account_private_key,
            l1_web3_url,
            zksync_web3_url,
            sentry_url: None,
        })
    }

    pub(crate) fn l1_tokens_to_process(&self) -> &[H160] {
        self.l1_tokens_to_process.as_ref()
    }
}

#[derive(Debug, Deserialize)]
struct TokenConfig {
    name: String,
    symbol: String,
    decimals: usize,
    address: Address,
}
