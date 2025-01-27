//! Metrics for tx sender

use vise::{Counter, Metrics};

/// TX Sender metrics
#[derive(Debug, Metrics)]
#[metrics(prefix = "txsender")]
pub(super) struct TxSenderMetrics {
    /// Timedout transactions count.
    pub timedout_transactions: Counter,
}

#[allow(unexpected_cfgs)]
#[vise::register]
pub(super) static TX_SENDER_METRICS: vise::Global<TxSenderMetrics> = vise::Global::new();
