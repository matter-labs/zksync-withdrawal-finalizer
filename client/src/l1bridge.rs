//! ABI wrappers for `L1Bridge` contract.

use std::sync::Arc;

use ethers::{
    prelude::{Address, Middleware},
    types::{Bytes, U256},
};

use crate::Result;

mod codegen {
    use ethers::prelude::abigen;

    abigen!(IL1Bridge, "./src/contracts/IL1bridge.json");
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

    /// Call `finalizeWithdrawal` function of the `L1Bridge` contract.
    pub async fn finalize_withdrawal(
        &self,
        l2_block_number: U256,
        l2_message_index: U256,
        l2_tx_number_in_block: u16,
        message: Bytes,
        merkle_proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        self.contract
            .finalize_withdrawal(
                l2_block_number,
                l2_message_index,
                l2_tx_number_in_block,
                message,
                merkle_proof,
            )
            .call()
            .await
            .map_err(Into::into)
    }
}
