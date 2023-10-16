//! Metrics for chain events

use vise::{Counter, Gauge, Metrics};

/// Chain events metrics.
#[derive(Debug, Metrics)]
#[metrics(prefix = "chain_events")]
pub(super) struct ChainEventsMetrics {
    /// Number of withdrawal events seen
    pub withdrawal_events: Counter,

    /// Number of new tokens added
    pub new_token_added: Counter,

    /// Successful reconnect attempts to RPC API
    pub successful_l2_reconnects: Counter,

    /// Reconnects on error to RPC API
    pub reconnects_on_error: Counter,

    /// Pagination on querying events on L2
    pub query_pagination: Gauge,

    /// Number of L2 logs received
    pub l2_logs_received: Counter,

    /// Number of L2 logs successfully decoded
    pub l2_logs_decoded: Counter,

    /// Number of successful websocket reconnects.
    pub successful_l1_reconnects: Counter,

    /// Number of reconnects errors on L1 WS.
    pub l1_reconnects_on_error: Counter,

    /// Number of received block commit events
    pub block_commit_events: Counter,

    /// Number of received block verification events
    pub block_verification_events: Counter,

    /// Number of received block execution events
    pub block_execution_events: Counter,
}

#[vise::register]
pub(super) static CHAIN_EVENTS_METRICS: vise::Global<ChainEventsMetrics> = vise::Global::new();
