use ethers::types::U256;

use client::WithdrawalData;

/// A struct that holds `RequestFinalizeWithdrawal`s and computes
/// when there are enough in a batch to be submitted.
pub struct WithdrawalsAccumulator {
    gas_price: U256,
    tx_fee_limit: U256,
    batch_finalization_gas_limit: U256,
    one_withdrawal_gas_limit: U256,
    withdrawals: Vec<WithdrawalData>,
}

impl WithdrawalsAccumulator {
    /// take withdrawals
    pub fn take_withdrawals(&mut self) -> Vec<WithdrawalData> {
        std::mem::take(&mut self.withdrawals)
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
    pub fn add_withdrawal(&mut self, data: WithdrawalData) {
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
