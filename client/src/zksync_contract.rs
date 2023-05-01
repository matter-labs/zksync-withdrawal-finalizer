//! ABI wrappers for the `ZkSync` contract.

use std::sync::Arc;

use ethers::{
    providers::Middleware,
    types::{Address, Bytes, U256},
};

use crate::Result;

mod codegen {
    use ethers::prelude::abigen;

    abigen!(IZkSync, "./src/contracts/IZkSync.json");
}

/// A struct wrapper for interacting with `ZkSync` contract.
pub struct ZkSync<M> {
    contract: codegen::IZkSync<M>,
}

impl<M: Middleware> ZkSync<M> {
    /// Create a new instalce of `ZkSync` contract.
    ///
    /// # Arguments
    ///
    /// * `address` - An address of the contract
    /// * `provider` - A middleware to perform calls to the contract
    pub fn new(address: Address, provider: Arc<M>) -> Self {
        let contract = codegen::IZkSync::new(address, provider);

        Self { contract }
    }

    /// Call `finalizeEthWithdrawal` method of `ZkSync` contract.
    pub async fn finalize_eth_withdrawal(
        &self,
        l2_block_number: U256,
        l2_message_index: U256,
        l2_tx_number_in_block: u16,
        message: Bytes,
        merkle_proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        self.contract
            .finalize_eth_withdrawal(
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
