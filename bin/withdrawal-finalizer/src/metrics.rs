//! Metrics for main binary

use std::time::Duration;

use ethers::types::U256;
use sqlx::PgPool;
use vise::{Gauge, Metrics};

const METRICS_REFRESH_PERIOD: Duration = Duration::from_secs(15);

/// Main finalizer binary metrics
#[derive(Debug, Metrics)]
#[metrics(prefix = "withdrawal_finalizer")]
pub(super) struct FinalizerMainMetrics {
    /// Capacity of the channel sending L1 events.
    pub watcher_l1_channel_capacity: Gauge,

    /// Capacity of the channel sending L2 events.
    pub watcher_l2_channel_capacity: Gauge,

    /// The withdrawals that were not finalized but are executed
    pub executed_eth_withdrawals_not_finalized: Gauge,

    /// The withdrawals that
    pub unexecuted_eth_withdrawals_below_current_threshold: Gauge,
}

#[vise::register]
pub(super) static MAIN_FINALIZER_METRICS: vise::Global<FinalizerMainMetrics> = vise::Global::new();

pub async fn meter_unfinalized_withdrawals(pool: PgPool, eth_threshold: Option<U256>) {
    loop {
        tokio::time::sleep(METRICS_REFRESH_PERIOD).await;

        let Ok(executed_not_finalized) =
            storage::get_executed_and_not_finalized_withdrawals_count(&pool).await
        else {
            continue;
        };
        let Ok(unexecuted) = storage::get_unexecuted_withdrawals_count(&pool, eth_threshold).await
        else {
            continue;
        };

        MAIN_FINALIZER_METRICS
            .executed_eth_withdrawals_not_finalized
            .set(executed_not_finalized);

        MAIN_FINALIZER_METRICS
            .unexecuted_eth_withdrawals_below_current_threshold
            .set(unexecuted);
    }
}
