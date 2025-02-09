//! Metrics for storage

#![allow(unexpected_cfgs)]

use std::time::Duration;

use vise::{Buckets, Histogram, LabeledFamily, Metrics};

/// Storage metrics
#[derive(Debug, Metrics)]
#[metrics(prefix = "storage")]
pub(super) struct StorageMetrics {
    #[metrics(buckets = Buckets::LATENCIES, labels = ["method"])]
    pub call: LabeledFamily<&'static str, Histogram<Duration>>,
}

#[vise::register]
pub(super) static STORAGE_METRICS: vise::Global<StorageMetrics> = vise::Global::new();
