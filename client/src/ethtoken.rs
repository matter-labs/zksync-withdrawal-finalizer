//! ABI wrappers for the `IEthToken` contract.

use std::sync::Arc;

use ethers::{
    providers::Middleware,
    types::{Address, BlockNumber},
};

use crate::{Result, WithdrawalEvent};

#[allow(missing_docs)]
mod codegen {
    use ethers::prelude::abigen;

    abigen!(EthToken, "./src/contracts/IEthToken.json",);
}

pub use codegen::WithdrawalFilter;

/// A struct wrapper for interfacing with `L2StandardToken` contract.
pub struct EthToken<M> {
    contract: codegen::EthToken<M>,
}

impl<M: Middleware> EthToken<M> {
    /// Create a new instance of `EthToken` contract.
    ///
    /// # Arguments
    ///
    /// * `address` - An address of the `EthToken`
    /// * `provider` - A middleware to perform calls to the contract
    pub fn new(address: Address, provider: Arc<M>) -> Self {
        let contract = codegen::EthToken::new(address, provider);

        Self { contract }
    }

    /// Withdrawal Events emitted by `EthToken` contract.
    ///
    /// # Arguments
    ///
    /// * `from_block` - beginning of the block interval
    /// * `to_block` = end of the block interval
    pub async fn withdrawal_events(
        &self,
        from_block: BlockNumber,
        to_block: BlockNumber,
    ) -> Result<Vec<WithdrawalEvent>> {
        let events = self
            .contract
            .event::<codegen::WithdrawalFilter>()
            .from_block(from_block)
            .to_block(to_block)
            .query_with_meta()
            .await?
            .into_iter()
            .map(|(event, meta)| WithdrawalEvent {
                tx_hash: meta.transaction_hash,
                block_number: meta.block_number.as_u64(),
                token: meta.address,
                amount: event.amount,
            })
            .collect();

        Ok(events)
    }
}
