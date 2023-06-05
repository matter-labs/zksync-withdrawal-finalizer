use ethers::prelude::MiddlewareError;

#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error("Middleware error {0}")]
    Middleware(String),
}

impl<M: MiddlewareError> From<M> for Error {
    fn from(value: M) -> Self {
        Self::Middleware(value.to_string())
    }
}

/// The crate result type.
pub type Result<T> = std::result::Result<T, Error>;
