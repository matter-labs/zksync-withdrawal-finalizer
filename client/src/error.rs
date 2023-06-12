use ethers::{contract::ContractError, prelude::Middleware, providers::ProviderError};

#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error(transparent)]
    ProviderError(#[from] ProviderError),

    #[error("Contract error {0}")]
    ContractError(String),

    #[error("Middleware error {0}")]
    Middleware(String),

    #[error("Channel is closed")]
    ChannelClosed,
}

impl<M: Middleware> From<ContractError<M>> for Error {
    fn from(value: ContractError<M>) -> Self {
        Self::ContractError(value.to_string())
    }
}

/// The client result type.
pub type Result<T> = std::result::Result<T, Error>;
