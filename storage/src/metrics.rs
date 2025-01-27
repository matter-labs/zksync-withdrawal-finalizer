//! Metrics for storage

use std::time::Duration;

use vise::{Buckets, Histogram, LabeledFamily, Metrics};

/// Storage metrics
#[derive(Debug, Metrics)]
#[metrics(prefix = "storage")]
pub(super) struct StorageMetrics {
    #[metrics(buckets = Buckets::LATENCIES, labels = ["method"])]
    pub call: LabeledFamily<&'static str, Histogram<Duration>>,
}

#[allow(unexpected_cfgs)]
#[vise::register]
pub(super) static STORAGE_METRICS: vise::Global<StorageMetrics> = vise::Global::new();
