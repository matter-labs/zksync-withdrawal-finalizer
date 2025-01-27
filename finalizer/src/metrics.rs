//! Metrics for finalizer

use vise::{Counter, Gauge, Metrics};

/// Finalizer metrics
#[derive(Debug, Metrics)]
#[metrics(prefix = "finalizer")]
pub(super) struct FinalizerMetrics {
    /// Highest finalized batch number
    pub highest_finalized_batch_number: Gauge,

    /// Number of withdrawals failed to finalize because of insufficient funds.
    pub failed_to_finalize_low_gas: Counter,

    /// Number of withdrawals predicted to fail by the smart contract.
    pub predicted_to_fail_withdrawals: Counter,

    /// Number of withdrawals failed to fetch withdrawal parameters for.
    pub failed_to_fetch_withdrawal_params: Counter,

    /// Number of withdrawal transactions that were reverted.
    pub reverted_withdrawal_transactions: Counter,
}

#[allow(unexpected_cfgs)]
#[vise::register]
pub(super) static FINALIZER_METRICS: vise::Global<FinalizerMetrics> = vise::Global::new();
