mod block_events;
mod error;
mod withdrawal_events;

use std::time::Duration;

pub use error::{Error, Result};

pub(crate) const RECONNECT_BACKOFF: Duration = Duration::from_secs(1);
pub use block_events::BlockEvents;
pub use withdrawal_events::WithdrawalEvents;
