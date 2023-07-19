use std::fmt::Debug;

#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error(transparent)]
    Storage(#[from] storage::Error),

    #[error(transparent)]
    Client(#[from] client::Error),
}

/// The crate result type.
pub type Result<T> = std::result::Result<T, Error>;
