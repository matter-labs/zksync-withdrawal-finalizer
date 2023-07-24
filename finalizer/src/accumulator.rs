use std::collections::BTreeMap;

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
    // key: (l_2_block_number, l_2_message_index)
    withdrawals: BTreeMap<(u64, u64), WithdrawalParams>,
}

impl WithdrawalsAccumulator {
    /// take withdrawals
    pub fn take_withdrawals(&mut self) -> Vec<WithdrawalParams> {
        std::mem::take(&mut self.withdrawals)
            .into_iter()
            .map(|v| v.1)
            .collect()
    }

    /// Get a reference to a current set of withdrawals
    pub fn withdrawals(&self) -> impl Iterator<Item = &WithdrawalParams> {
        self.withdrawals.values()
    }

    /// Remove unsuccessful withdrawals by returned results.
    pub fn remove_unsuccessful(
        &mut self,
        unsuccessful: &[FinalizeResult],
    ) -> Vec<WithdrawalParams> {
        let mut result = Vec::with_capacity(unsuccessful.len());

        for u in unsuccessful {
            if let Some(wp) = self
                .withdrawals
                .remove(&(u.l_2_block_number.as_u64(), u.l_2_message_index.as_u64()))
            {
                result.push(wp);
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
            withdrawals: BTreeMap::new(),
        }
    }

    /// Add a finalization withdrawals request.
    ///
    /// # Argument
    ///
    /// * `request` A finalization request.
    pub fn add_withdrawal(&mut self, data: WithdrawalParams) {
        self.withdrawals.insert(
            (data.l1_batch_number.as_u64(), data.l2_message_index.into()),
            data,
        );
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
