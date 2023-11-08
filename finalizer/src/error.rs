use std::fmt::Debug;

use ethers::{prelude::ContractError, providers::Middleware};

#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error(transparent)]
    Storage(#[from] storage::Error),

    #[error(transparent)]
    Client(#[from] client::Error),

    #[error("contract all error {0}")]
    Contract(String),

    #[error("middleware error {0}")]
    Middleware(String),

    #[error("withdrawal transaction was reverted")]
    WithdrawalTransactionReverted,
}

impl<M: Middleware> From<ContractError<M>> for Error {
    fn from(value: ContractError<M>) -> Self {
        Self::Contract(format!("{value}"))
    }
}

/// The crate result type.
pub type Result<T> = std::result::Result<T, Error>;
