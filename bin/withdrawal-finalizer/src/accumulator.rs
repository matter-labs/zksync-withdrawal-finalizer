#![allow(unused)]
use std::collections::HashMap;

use ethers::types::U256;

use client::withdrawal_finalizer::RequestFinalizeWithdrawal;

/// A struct that holds `RequestFinalizeWithdrawal`s and computes
/// when there are enough in a batch to be submitted.
pub struct WithdrawalsAccumulator {
    gas_price: U256,
    tx_fee_limit: U256,
    batch_finalization_gas_limit: U256,
    one_withdrawal_gas_limit: U256,
    withdrawals: HashMap<(U256, U256), RequestFinalizeWithdrawal>,
}

impl WithdrawalsAccumulator {
    /// Create a new `WithdrawalsAccumulator`.
    pub fn new(
        gas_price: U256,
        tx_fee_limit: U256,
        batch_finalization_gas_limit: U256,
        one_withdrawal_gas_limit: U256,
    ) -> Self {
        Self {
            gas_price,
            tx_fee_limit,
            batch_finalization_gas_limit,
            one_withdrawal_gas_limit,
            withdrawals: HashMap::new(),
        }
    }

    /// Add a finalization withdrawals request.
    ///
    /// # Argument
    ///
    /// * `request` A finalization request.
    pub fn add_withdrawal(&mut self, request: RequestFinalizeWithdrawal) {
        self.withdrawals.insert(
            (request.l_2_block_number, request.l_2_message_index),
            request,
        );
    }

    /// Get the current number of withdrawals in this accumulator.
    pub fn len(&self) -> usize {
        self.withdrawals.len()
    }

    /// Get estimated gas consumption of the current set.
    pub fn current_gas_usage(&self) -> U256 {
        self.one_withdrawal_gas_limit * self.withdrawals.len()
    }

    /// Is this batch of withdrawals ready to be finalized.
    pub fn ready_to_finalize(&self) -> bool {
        let current_gas_usage = self.current_gas_usage();
        current_gas_usage >= self.batch_finalization_gas_limit
            || current_gas_usage * self.gas_price >= self.tx_fee_limit
    }

    /// Remove all withdrawals from this accumulator.
    pub fn clear(&mut self) {
        self.withdrawals.clear()
    }

    /// Remove a single withdrawal request by key
    ///
    /// # Arguments
    ///
    /// * `block_number`: the number of the block
    /// * `message_index`: the index of the message in the block
    pub fn remove_withdrawal(&mut self, block_number: U256, message_index: U256) {
        self.withdrawals.remove(&(block_number, message_index));
    }
}
