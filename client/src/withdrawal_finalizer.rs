//! ABI wrappers for `WithdrawalFinalizer` contract.

use std::sync::Arc;

use ethers::{
    providers::Middleware,
    types::{Address, TransactionReceipt},
};

use crate::Result;

#[allow(missing_docs)]
mod codegen {
    use ethers::prelude::abigen;

    abigen!(
        WithdrawalFinalizer,
        "./src/contracts/WithdrawalFinalizer.json"
    );
}

pub use codegen::{RequestFinalizeWithdrawal, Result as FinalizeResult};

/// A struct wrapper for interacting with the `WithdrawalFinalizer` contract
pub struct WithdrawalFinalizer<M> {
    contract: codegen::WithdrawalFinalizer<M>,
}

impl<M: Middleware> WithdrawalFinalizer<M> {
    /// Create a new instance of `WithdrawalFinalizer` contract.
    ///
    /// # Arguments
    ///
    /// * `address` - An address of the contract
    /// * `provider` - A middleware to perform call to the contract
    pub fn new(address: Address, provider: Arc<M>) -> Self {
        let contract = codegen::WithdrawalFinalizer::new(address, provider);

        Self { contract }
    }

    /// Call `finalizeWithdrawals` method
    pub async fn finalize_withdrawals(
        &self,
        requests: Vec<RequestFinalizeWithdrawal>,
    ) -> Result<Vec<FinalizeResult>> {
        self.contract
            .finalize_withdrawals(requests)
            .call()
            .await
            .map_err(Into::into)
    }

    /// Send `finalizeWithdrawals` tranaction
    pub async fn send_finalize_withdrawals(
        &self,
        requests: Vec<RequestFinalizeWithdrawal>,
    ) -> Result<Option<TransactionReceipt>> {
        let pending_tx = self.contract.finalize_withdrawals(requests);

        let x = pending_tx
            .send()
            .await
            .map_err(Into::<crate::Error>::into)?
            .await?;
        Ok(x)
    }
}
