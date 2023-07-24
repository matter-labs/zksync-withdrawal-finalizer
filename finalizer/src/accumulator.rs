use std::collections::HashSet;

use ethers::types::U256;

use client::{
    withdrawal_finalizer::codegen::withdrawal_finalizer::Result as FinalizeResult, WithdrawalParams,
};

/// A struct that holds `RequestFinalizeWithdrawal`s and computes
/// when there are enough in a batch to be submitted.
pub struct WithdrawalsAccumulator {
    gas_price: U256,
    tx_fee_limit: U256,
    batch_finalization_gas_limit: U256,
    one_withdrawal_gas_limit: U256,
    withdrawals: Vec<WithdrawalParams>,
}

impl WithdrawalsAccumulator {
    /// take withdrawals
    pub fn take_withdrawals(&mut self) -> Vec<WithdrawalParams> {
        std::mem::take(&mut self.withdrawals)
    }

    /// Get a reference to a current set of withdrawals
    pub fn withdrawals(&self) -> &[WithdrawalParams] {
        &self.withdrawals
    }

    /// Remove unsuccessful withdrawals by returned results.
    pub fn remove_unsuccessful(
        &mut self,
        unsuccessful: &[FinalizeResult],
    ) -> Vec<WithdrawalParams> {
        let mut result = Vec::with_capacity(unsuccessful.len());
        let unsuccessful_set: HashSet<_> = unsuccessful
            .iter()
            .map(|r| (r.l_2_block_number, r.l_2_message_index))
            .collect();

        let mut i = 0;

        while i < self.withdrawals.len() {
            if unsuccessful_set.contains(&(
                self.withdrawals[i].l1_batch_number.as_u64().into(),
                self.withdrawals[i].l2_message_index.into(),
            )) {
                result.push(self.withdrawals.remove(i));
            } else {
                i += 1;
            }
        }

        result
    }

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
            withdrawals: Vec::new(),
        }
    }

    /// Add a finalization withdrawals request.
    ///
    /// # Argument
    ///
    /// * `request` A finalization request.
    pub fn add_withdrawal(&mut self, data: WithdrawalParams) {
        self.withdrawals.push(data);
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
}
