//! Metrics for withdrawal meterer

#![allow(unexpected_cfgs)]

use vise::{EncodeLabelSet, EncodeLabelValue, Family, Gauge, LabeledFamily, Metrics};

/// Kinds of withdrawal volumes currently being metered by application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EncodeLabelValue, EncodeLabelSet)]
#[metrics(label = "component", rename_all = "snake_case")]
pub enum MeteringComponent {
    /// Volumes of requested withdrawals, that is, volumes metered at times
    /// as seen in received withdrawal events.
    RequestedWithdrawals,

    /// Volumes of finalized withdrawals. Metered as these withdrawals are
    /// actually finalized.
    FinalizedWithdrawals,
}

const LABELS: [&str; 2] = ["component", "token"];
type Labels = (MeteringComponent, String);

#[derive(Debug, Metrics)]
#[metrics(prefix = "withdrawals_meterer")]
pub(super) struct WithdrawalsMetererMetrics {
    /// Token decimals stored in each metering component
    pub token_decimals_stored: Family<MeteringComponent, Gauge>,

    /// Volumes of withdrawals
    #[metrics(labels = LABELS)]
    pub withdrawals: LabeledFamily<Labels, Gauge<f64>, 2>,
}

#[vise::register]
pub(super) static WM_METRICS: vise::Global<WithdrawalsMetererMetrics> = vise::Global::new();
