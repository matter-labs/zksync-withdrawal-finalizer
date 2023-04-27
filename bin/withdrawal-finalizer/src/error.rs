#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    TomlDe(#[from] toml::de::Error),

    #[error(transparent)]
    Hex(#[from] rustc_hex::FromHexError),

    #[error("L1 web3 url was not configured")]
    NoL1Web3Url,

    #[error("zkSync web3 url was not configured")]
    NoZkSyncWeb3Url,
}
