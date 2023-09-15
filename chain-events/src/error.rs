use std::fmt::{Debug, Display};

use ethers::providers::LogQueryError;

#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error("Middleware error {0}")]
    Middleware(String),

    #[error("LogQuery error {0}")]
    LogQuery(String),

    #[error("Channel closing")]
    ChannelClosing,
}

impl<E: Debug + Display> From<LogQueryError<E>> for Error {
    fn from(value: LogQueryError<E>) -> Self {
        Self::LogQuery(value.to_string())
    }
}

/// The crate result type.
pub type Result<T> = std::result::Result<T, Error>;
