//! ABI wrappers for the `L2StandardToken` contract.

use std::sync::Arc;

use ethers::{
    providers::Middleware,
    types::{Address, BlockNumber},
};

use crate::{Result, WithdrawalEvent};

mod codegen {
    use ethers::prelude::abigen;

    abigen!(L2StandardToken, "./src/contracts/IL2StandardToken.json",);
}

/// A struct wrapper for interfacing with `L2StandardToken` contract.
pub struct L2StandardToken<M> {
    contract: codegen::L2StandardToken<M>,
}

impl<M: Middleware> L2StandardToken<M> {
    /// Create a new instance of `L2StandardToken` contract.
    ///
    /// # Arguments
    ///
    /// * `address` - An address of the contract
    /// * `provider` - A middleware to perform calls to the contract.
    pub fn new(address: Address, provider: Arc<M>) -> Self {
        let contract = codegen::L2StandardToken::new(address, provider);

        Self { contract }
    }

    /// Withdrawal Events emitted in a block interval
    ///
    /// # Arguments
    ///
    /// * `from_block` - beginning of the block interval
    /// * `to_block` - end of the block interval
    pub async fn withdrawal_events(
        &self,
        from_block: BlockNumber,
        to_block: BlockNumber,
    ) -> Result<Vec<WithdrawalEvent>> {
        let events = self
            .contract
            .event::<codegen::BridgeBurnFilter>()
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
