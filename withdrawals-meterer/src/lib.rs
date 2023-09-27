#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! A utility crate that meters withdrawals amounts.

use std::{collections::HashMap, str::FromStr, sync::Arc};

use client::ETH_TOKEN_ADDRESS;
use ethers::types::Address;
use lazy_static::lazy_static;
use sqlx::PgPool;
use storage::StoredWithdrawal;
use tokio::sync::RwLock;

lazy_static! {
    static ref TOKEN_DECIMALS: Arc<RwLock<HashMap<Address, u32>>> = {
        let mut map = HashMap::new();
        map.insert(ETH_TOKEN_ADDRESS, 18_u32);

        Arc::new(RwLock::new(map))
    };
}

/// Given a set of withdrawal ids meter all of them to a metric
/// with a given name.
pub async fn meter_finalized_withdrawals_storage(
    pool: &PgPool,
    ids: Vec<i64>,
    metric_name: &'static str,
) -> Result<(), storage::Error> {
    let withdrawals = storage::get_withdrawals(pool, &ids).await?;

    meter_finalized_withdrawals(pool, &withdrawals, metric_name).await?;

    Ok(())
}

/// Given a set of [`StoredWithdrawal`], meter all of them to a
/// metric with a given name.
///
/// This function returns only storage error, all formatting, etc
/// errors will be just logged.
pub async fn meter_finalized_withdrawals(
    pool: &PgPool,
    withdrawals: &[StoredWithdrawal],
    metric_name: &'static str,
) -> Result<(), storage::Error> {
    for w in withdrawals {
        let guard = TOKEN_DECIMALS.read().await;
        let decimals = guard.get(&w.event.token).copied();
        drop(guard);

        let decimals = match decimals {
            None => {
                let Some(decimals) = storage::token_decimals(pool, w.event.token).await? else {
                    vlog::error!("Received withdrawal from unknown token {:?}", w.event.token);
                    continue;
                };

                TOKEN_DECIMALS.write().await.insert(w.event.token, decimals);
                decimals
            }
            Some(decimals) => decimals,
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
            metric_name,
            formatted_f64,
            "token" => format!("{:?}", w.event.token)
        )
    }

    Ok(())
}
