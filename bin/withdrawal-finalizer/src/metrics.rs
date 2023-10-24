//! Metrics for main binary

use vise::{Gauge, Metrics};

/// Main finalizer binary metrics
#[derive(Debug, Metrics)]
#[metrics(prefix = "withdrawal_finalizer")]
pub(super) struct FinalizerMainMetrics {
    /// Capacity of the channel sending L1 events.
    pub watcher_l1_channel_capacity: Gauge,

    /// Capacity of the channel sending L2 events.
    pub watcher_l2_channel_capacity: Gauge,
}

#[vise::register]
pub(super) static MAIN_FINALIZER_METRICS: vise::Global<FinalizerMainMetrics> = vise::Global::new();
