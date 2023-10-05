#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! A utility crate that meters withdrawals amounts.

use std::{collections::HashMap, str::FromStr, sync::Arc};

use chrono::{Datelike, TimeZone, Utc};
use client::{ETH_ADDRESS, ETH_TOKEN_ADDRESS};
use ethers::types::{Address, U64};
use sqlx::PgPool;
use storage::StoredWithdrawal;
use tokio::sync::RwLock;

const HISTORIC_QUERYING_STEP: u64 = 1000;

/// State of withdrawals volumes metering.
pub struct WithdrawalsMeter {
    pool: PgPool,

    /// A mapping from L2 address to L1 address and decimals of token.
    tokens: Arc<RwLock<HashMap<Address, (u32, Address)>>>,
    component_name: &'static str,
}

async fn reset_metrics_at_midnight(
    tokens: Arc<RwLock<HashMap<Address, (u32, Address)>>>,
    component_name: &'static str,
) {
    loop {
        let date = Utc::now();

        let next_midnight =
            match Utc.with_ymd_and_hms(date.year(), date.month(), date.day() + 1, 0, 0, 0) {
                chrono::LocalResult::None => {
                    vlog::error!("Failed to calculate next midnight");
                    continue;
                }
                chrono::LocalResult::Single(s) | chrono::LocalResult::Ambiguous(s, _) => s,
            };

        let duration_to_sleep = next_midnight.signed_duration_since(date).to_std().expect(
            "by calculating just the next day we never overflow i64::MAX milliseconds; qed",
        );

        tokio::time::sleep(duration_to_sleep).await;

        vlog::info!("zeroing metrics for {component_name}");

        let read_guard = tokens.read().await;

        for v in read_guard.values() {
            metrics::increment_gauge!(
                format!("{}_withdrawals", component_name),
                0_f64,
                "token" => format!("{:?}", v.1)
            )
        }
    }
}

impl WithdrawalsMeter {
    /// Create a new [`WithdrawalsMeter`]
    ///
    /// # Arguments
    ///
    /// * `pool`: DB connection pool
    /// * `component_name`: Name of the component that does metering, metric names will be
    ///    derived from it
    pub async fn new(
        pool: PgPool,
        component_name: &'static str,
        historic_interval: Option<(U64, U64)>,
    ) -> Self {
        let mut token_decimals = HashMap::new();

        token_decimals.insert(ETH_TOKEN_ADDRESS, (18_u32, ETH_ADDRESS));

        metrics::increment_gauge!(format!("{component_name}_token_decimals_stored"), 1.0);

        let tokens = Arc::new(RwLock::new(token_decimals));

        tokio::spawn(reset_metrics_at_midnight(tokens.clone(), component_name));

        let mut res = Self {
            pool,
            tokens,
            component_name,
        };

        res.meter_historic_interval(historic_interval).await;

        res
    }

    async fn meter_historic_interval(&mut self, historic_interval: Option<(U64, U64)>) {
        let Some((from, to)) = historic_interval else {
            return;
        };
        let mut from = from.as_u64();
        let to = to.as_u64();

        while from < to {
            vlog::info!("querying historic withdrawal volumes from {from} to {to}");
            let ids = storage::withdrawal_ids(&self.pool, from, to).await.unwrap();

            self.meter_withdrawals_storage(&ids).await.unwrap();
            from += HISTORIC_QUERYING_STEP;
        }
    }

    /// Given a set of withdrawal ids meter all of them to a metric
    /// with a given name.
    pub async fn meter_withdrawals_storage(&mut self, ids: &[i64]) -> Result<(), storage::Error> {
        let withdrawals = storage::get_withdrawals(&self.pool, ids).await?;

        self.meter_withdrawals(&withdrawals).await?;

        Ok(())
    }

    /// Given a set of [`StoredWithdrawal`], meter all of them to a
    /// metric with a given name.
    ///
    /// This function returns only storage error, all formatting, etc
    /// errors will be just logged.
    pub async fn meter_withdrawals(
        &mut self,
        withdrawals: &[StoredWithdrawal],
    ) -> Result<(), storage::Error> {
        for w in withdrawals {
            let guard = self.tokens.read().await;
            let value = guard.get(&w.event.token).cloned();
            drop(guard);

            let (decimals, l1_token_address) = match value {
                None => {
                    let Some((decimals, address)) =
                        storage::token_decimals_and_l1_address(&self.pool, w.event.token).await?
                    else {
                        vlog::error!("Received withdrawal from unknown token {:?}", w.event.token);
                        continue;
                    };

                    let mut guard = self.tokens.write().await;
                    guard.insert(w.event.token, (decimals, address));
                    drop(guard);

                    metrics::increment_gauge!(
                        format!("{}_token_decimals_stored", self.component_name),
                        1.0
                    );

                    (decimals, address)
                }
                Some(d) => d,
            };

            let formatted = match ethers::utils::format_units(w.event.amount, decimals) {
                Ok(f) => f,
                Err(e) => {
                    vlog::error!("failed to format units: {e}");
                    continue;
                }
            };

            let formatted_f64 = match f64::from_str(&formatted) {
                Ok(f) => f,
                Err(e) => {
                    vlog::error!("failed to format units: {e}");
                    continue;
                }
            };

            metrics::increment_gauge!(
                format!("{}_withdrawals", self.component_name),
                formatted_f64,
                "token" => format!("{:?}", l1_token_address)
            )
        }

        Ok(())
    }
}
