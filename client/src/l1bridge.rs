//! ABI wrappers for `L1Bridge` contract.

use std::sync::Arc;

use ethers::prelude::{Address, Middleware};

use crate::Result;

mod codegen {
    use ethers::prelude::abigen;

    abigen!(
        IL1Bridge,
        r#"[
            function l2TokenAddress(address l1Token) external view returns (address)
            function l2Bridge() external view returns (address)
        ]"#,
    );
}

/// A struct wrapper for interacting with the `L1Bridge` contract.
pub struct L1Bridge<M> {
    contract: codegen::IL1Bridge<M>,
}

impl<M: Middleware> L1Bridge<M> {
    /// Create a new instance of `L1Bridge` contract.
    ///
    /// # Arguments
    ///
    /// * `address` - An address of the contract
    /// * `provider` - A middleware to perform calls to the contract
    pub fn new(address: Address, provider: Arc<M>) -> Self {
        let contract = codegen::IL1Bridge::new(address, provider);

        Self { contract }
    }

    /// Call `l2Bridge` method of `L1Bridge` contract.
    pub async fn l2bridge(&self) -> Result<Address> {
        self.contract.l_2_bridge().call().await.map_err(Into::into)
    }

    /// Call `l2TokenAddress` method of `L1Bridge` contract.
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
