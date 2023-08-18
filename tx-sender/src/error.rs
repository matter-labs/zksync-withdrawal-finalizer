use std::fmt::Debug;

use ethers::providers::ProviderError;

#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error(transparent)]
    ProviderError(#[from] ProviderError),

    #[error("Middleware error {0}")]
    Middleware(String),
}

/// The crate result type.
pub type Result<T> = std::result::Result<T, Error>;
