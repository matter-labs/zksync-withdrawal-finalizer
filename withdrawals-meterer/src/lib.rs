#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! A utility crate that meters withdrawals amounts.

use std::{collections::HashMap, str::FromStr};

use client::ETH_TOKEN_ADDRESS;
use ethers::types::Address;
use sqlx::PgPool;
use storage::StoredWithdrawal;

/// State of withdrawls volumes metering.
pub struct WithdrawalsMeter {
    pool: PgPool,
    token_decimals: HashMap<Address, u32>,
    metric_name: &'static str,
}

impl WithdrawalsMeter {
    /// Create a new [`WithdrawalsMeter`]
    ///
    /// # Arguments
    ///
    /// * `pool`: DB connection pool
    /// * `metric_name: Name of the metric to meter to
    pub fn new(pool: PgPool, metric_name: &'static str) -> Self {
        let mut token_decimals = HashMap::new();
        token_decimals.insert(ETH_TOKEN_ADDRESS, 18_u32);

        Self {
            pool,
            token_decimals,
            metric_name,
        }
    }
    /// Given a set of withdrawal ids meter all of them to a metric
    /// with a given name.
    pub async fn meter_finalized_withdrawals_storage(
        &mut self,
        ids: &[i64],
    ) -> Result<(), storage::Error> {
        let withdrawals = storage::get_withdrawals(&self.pool, ids).await?;

        self.meter_finalized_withdrawals(&withdrawals).await?;

        Ok(())
    }

    /// Given a set of [`StoredWithdrawal`], meter all of them to a
    /// metric with a given name.
    ///
    /// This function returns only storage error, all formatting, etc
    /// errors will be just logged.
    pub async fn meter_finalized_withdrawals(
        &mut self,
        withdrawals: &[StoredWithdrawal],
    ) -> Result<(), storage::Error> {
        for w in withdrawals {
            let decimals = match self.token_decimals.get(&w.event.token) {
                None => {
                    let Some(decimals) = storage::token_decimals(&self.pool, w.event.token).await?
                    else {
                        vlog::error!("Received withdrawal from unknown token {:?}", w.event.token);
                        continue;
                    };

                    self.token_decimals.insert(w.event.token, decimals);
                    decimals
                }
                Some(decimals) => *decimals,
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
                self.metric_name,
                formatted_f64,
                "token" => format!("{:?}", w.event.token)
            )
        }

        Ok(())
    }
}
