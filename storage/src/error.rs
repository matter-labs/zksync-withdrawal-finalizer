#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error(transparent)]
    PgError(#[from] sqlx::Error),
}

/// Crate result type.
pub type Result<T> = std::result::Result<T, Error>;
