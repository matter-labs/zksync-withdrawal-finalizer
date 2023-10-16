#![deny(unused_crate_dependencies)]

//! A set of logging macros that print not only timestamp and log level,
//! but also file name, line and column.
//!
//! They behave just like usual tracing::warn, tracing::info, etc.
//! For warn and error macros we are adding file line and column to tracing variables
//!
//! The format of the logs in `stdout` can be `plain` or `json` and is set by the `MISC_LOG_FORMAT` env variable.
//!
//! Full documentation for the `tracing` crate here <https://docs.rs/tracing/>
//!
//! Integration with sentry for catching errors and react on them immediately
//! <https://docs.sentry.io/platforms/rust/>
//!

use std::{borrow::Cow, str::FromStr};

use sentry::{types::Dsn, ClientInitGuard};
use std::backtrace::Backtrace;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub use chrono as __chrono;
pub use sentry as __sentry;
pub use tracing as __tracing;
pub use tracing::{debug, info, log, trace};

fn get_sentry_url() -> Option<Dsn> {
    if let Ok(sentry_url) = std::env::var("MISC_SENTRY_URL") {
        if let Ok(sentry_url) = Dsn::from_str(sentry_url.as_str()) {
            return Some(sentry_url);
        }
    }
    None
}

pub const DEFAULT_SAMPLING_RATIO: f64 = 0.1;

/// Initialize logging with tracing and set up log format
///
/// If the sentry URL is provided via an environment variable, this function will also initialize sentry.
/// Returns a sentry client guard. The full description can be found in the official documentation:
/// <https://docs.sentry.io/platforms/rust/#configure>
#[must_use]
pub fn init() -> Option<ClientInitGuard> {
    let log_format = std::env::var("MISC_LOG_FORMAT").unwrap_or_else(|_| "plain".to_string());

    match log_format.as_str() {
        "plain" => {
            tracing_subscriber::registry()
                .with(fmt::Layer::default())
                .with(tracing_subscriber::EnvFilter::from_default_env())
                .init();
        }
        "json" => {
            let timer = tracing_subscriber::fmt::time::UtcTime::rfc_3339();
            // must be set before sentry hook for sentry to function
            install_pretty_panic_hook();

            tracing_subscriber::registry()
                .with(
                    fmt::Layer::default()
                        .with_file(true)
                        .with_line_number(true)
                        .with_timer(timer)
                        .json(),
                )
                .with(tracing_subscriber::EnvFilter::from_default_env())
                .init();
        }
        _ => panic!("MISC_LOG_FORMAT has an unexpected value {}", log_format),
    };

    get_sentry_url().map(|sentry_url| {
        let l1_network = std::env::var("CHAIN_ETH_NETWORK").expect("Must be set");
        let l2_network = std::env::var("CHAIN_ETH_ZKSYNC_NETWORK").expect("Must be set");

        let options = sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: Some(Cow::from(format!("{} - {}", l1_network, l2_network))),
            attach_stacktrace: true,
            ..Default::default()
        };

        sentry::init((sentry_url, options))
    })
}

/// Format panics like tracing::error
fn install_pretty_panic_hook() {
    // This hook does not use the previous one set because it leads to 2 logs:
    // the first is the default panic log and the second is from this code. To avoid this situation,
    // hook must be installed first
    std::panic::set_hook(Box::new(move |panic_info| {
        let backtrace = Backtrace::capture();
        let timestamp = chrono::Utc::now();
        let panic_message = if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.as_str()
        } else if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s
        } else {
            "Panic occurred without additional info"
        };

        let panic_location = panic_info
            .location()
            .map(|val| val.to_string())
            .unwrap_or_else(|| "Unknown location".to_owned());

        let backtrace_str = format!("{}", backtrace);
        let timestamp_str = format!("{}", timestamp.format("%Y-%m-%dT%H:%M:%S%.fZ"));

        println!(
            "{}",
            serde_json::json!({
                "timestamp": timestamp_str,
                "level": "CRITICAL",
                "fields": {
                    "message": panic_message,
                    "location": panic_location,
                    "backtrace": backtrace_str,
                }
            })
        );
    }));
}
