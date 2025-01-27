//! Metrics for withdrawal watcher

#![allow(unexpected_cfgs)]

use vise::{Gauge, Metrics};

/// Watcher metrics
#[derive(Debug, Metrics)]
#[metrics(prefix = "watcher")]
pub(super) struct WatcherMetrics {
    /// Block number of last seen block commit event.
    pub l2_last_committed_block: Gauge,

    /// Block number of last seen block verify event.
    pub l2_last_verified_block: Gauge,

    /// Block number of last seen block execute event.
    pub l2_last_executed_block: Gauge,

    /// Last seen L2 block number.
    pub l2_last_seen_block: Gauge,
}

#[vise::register]
pub(super) static WATCHER_METRICS: vise::Global<WatcherMetrics> = vise::Global::new();
