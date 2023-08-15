use std::fmt::Debug;

#[derive(PartialEq, Eq, Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error("Middleware error {0}")]
    Middleware(String),

    #[error("sending a gas adjusted transaction timed out")]
    Timedout,
}

/// The crate result type.
pub type Result<T> = std::result::Result<T, Error>;
