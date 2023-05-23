//! ABI wrappers for `L2Bridge` contract.

use std::sync::Arc;

use ethers::prelude::{Address, Middleware};

use crate::Result;

mod codegen {
    use ethers::prelude::abigen;

    abigen!(
        IL2Bridge,
        "$CARGO_MANIFEST_DIR/src/contracts/IL2Bridge.json"
    );
}

/// A struct wrapper for interacting with the `L2Bridge` contract.
pub struct L2Bridge<M> {
    contract: codegen::IL2Bridge<M>,
}

impl<M: Middleware> L2Bridge<M> {
    /// Create a new instance of `L2Bridge` contract.
    ///
    /// # Arguments
    ///
    /// * `address` - An address of the contract
    /// * `provider` - A middleware to perform calls to the contract
    pub fn new(address: Address, provider: Arc<M>) -> Self {
        let contract = codegen::IL2Bridge::new(address, provider);

        Self { contract }
    }

    /// Call `l2TokenAddress` method of `L2Bridge` contract.
    ///
    /// # Arguments
    ///
    /// * `address` - An address of the token
    pub async fn l2_token_address(&self, address: Address) -> Result<Address> {
        self.contract
            .l_2_token_address(address)
            .call()
            .await
            .map_err(Into::into)
    }
}
