#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! A utility crate that meters withdrawals amounts.

use std::{collections::HashMap, str::FromStr};

use client::{ETH_ADDRESS, ETH_TOKEN_ADDRESS};
use ethers::types::Address;
use sqlx::PgPool;
use storage::StoredWithdrawal;

use crate::metrics::WM_METRICS;

mod metrics;
pub use metrics::MeteringComponent;

/// State of withdrawals volumes metering.
pub struct WithdrawalsMeter {
    pool: PgPool,
    /// A mapping from L2 address to L1 address and decimals of token.
    tokens: HashMap<Address, (u32, Address)>,
    metering_component: MeteringComponent,
}

impl WithdrawalsMeter {
    /// Create a new [`WithdrawalsMeter`]
    ///
    /// # Arguments
    ///
    /// * `pool`: DB connection pool
    /// * `component_name`: Name of the component that does metering, metric names will be
    ///    derived from it
    pub fn new(pool: PgPool, metering_component: MeteringComponent) -> Self {
        let mut token_decimals = HashMap::new();

        token_decimals.insert(ETH_TOKEN_ADDRESS, (18_u32, ETH_ADDRESS));

        WM_METRICS.token_decimals_stored[&metering_component].inc_by(1);

        Self {
            pool,
            tokens: token_decimals,
            metering_component,
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
            let (decimals, l1_token_address) = match self.tokens.get(&w.event.token) {
                None => {
                    let Some((decimals, address)) =
                        storage::token_decimals_and_l1_address(&self.pool, w.event.token).await?
                    else {
                        tracing::error!(
                            "Received withdrawal from unknown token {:?}",
                            w.event.token
                        );
                        continue;
                    };

                    self.tokens.insert(w.event.token, (decimals, address));

                    WM_METRICS.token_decimals_stored[&self.metering_component].inc_by(1);

                    (decimals, address)
                }
                Some(d) => *d,
            };

            let formatted = match ethers::utils::format_units(w.event.amount, decimals) {
                Ok(f) => f,
                Err(e) => {
                    tracing::error!("failed to format units: {e}");
                    continue;
                }
            };

            let formatted_f64 = match f64::from_str(&formatted) {
                Ok(f) => f,
                Err(e) => {
                    tracing::error!("failed to format units: {e}");
                    continue;
                }
            };

            WM_METRICS.withdrawals[&(self.metering_component, format!("{:?}", l1_token_address))]
                .inc_by(formatted_f64);
        }

        Ok(())
    }
}
