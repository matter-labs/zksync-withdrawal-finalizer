use ethers::{
    abi::EncodePackedError, contract::ContractError, prelude::Middleware, providers::ProviderError,
    types::H256,
};

#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error(transparent)]
    AbiError(#[from] ethers::abi::Error),

    #[error(transparent)]
    ContractAbiError(#[from] ethers::contract::AbiError),

    #[error(transparent)]
    ProviderError(#[from] ProviderError),

    #[error(transparent)]
    EncodePackedError(#[from] EncodePackedError),

    #[error("Contract error {0}")]
    ContractError(String),

    #[error("Middleware error {0}")]
    Middleware(String),

    #[error("Channel is closed")]
    ChannelClosed,

    #[error("Withdrawal event with index {0} not found in transaction {1:?}")]
    WithdrawalLogNotFound(usize, H256),

    #[error("Failed to decode withdrawal event from log")]
    FailedToDecodeLog,
}

impl<M: Middleware> From<ContractError<M>> for Error {
    fn from(value: ContractError<M>) -> Self {
        Self::ContractError(value.to_string())
    }
}

/// The client result type.
pub type Result<T> = std::result::Result<T, Error>;
