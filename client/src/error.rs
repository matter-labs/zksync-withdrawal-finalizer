use ethers::contract::ContractError;
use ethers::prelude::Middleware;

#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error("Contract error {0}")]
    ContractError(String),
}

impl<M: Middleware> From<ContractError<M>> for Error {
    fn from(value: ContractError<M>) -> Self {
        Self::ContractError(value.to_string())
    }
}

/// The client result type.
pub type Result<T> = std::result::Result<T, Error>;
