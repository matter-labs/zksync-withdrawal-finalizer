#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Interactions with zkSync on-chain contracts.

mod error;
pub use error::{Error, Result};
pub mod l1bridge;
