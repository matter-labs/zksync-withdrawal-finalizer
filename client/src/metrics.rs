//! Metrics for client

#![allow(unexpected_cfgs)]

use std::time::Duration;

use vise::{Buckets, Histogram, LabeledFamily, Metrics};

/// Client metrics.
#[derive(Debug, Metrics)]
#[metrics(prefix = "client")]
pub(super) struct ClientMetrics {
    #[metrics(buckets = Buckets::LATENCIES, labels = ["method"])]
    pub call: LabeledFamily<&'static str, Histogram<Duration>>,
}

#[vise::register]
pub(super) static CLIENT_METRICS: vise::Global<ClientMetrics> = vise::Global::new();
