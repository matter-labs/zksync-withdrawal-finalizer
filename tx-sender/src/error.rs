use std::fmt::Debug;

use ethers::{
    prelude::nonce_manager::NonceManagerError,
    providers::{Middleware, ProviderError},
};

#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error<M>
where
    M: Middleware,
{
    #[error(transparent)]
    ProviderError(#[from] ProviderError),

    #[error("Middleware error")]
    Middleware { e: <M as Middleware>::Error },
}

impl<M: Middleware> From<NonceManagerError<M>> for Error<M> {
    fn from(value: NonceManagerError<M>) -> Self {
        match value {
            NonceManagerError::MiddlewareError(e) => Self::Middleware { e },
        }
    }
}

/// The crate result type.
pub type Result<T, M> = std::result::Result<T, Error<M>>;
