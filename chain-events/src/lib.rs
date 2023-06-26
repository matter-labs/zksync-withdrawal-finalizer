mod block_events;
mod error;
mod l2_to_l1_events;
mod withdrawal_events;

use std::time::Duration;

pub use error::{Error, Result};

pub(crate) const RECONNECT_BACKOFF: Duration = Duration::from_secs(1);
pub use block_events::BlockEvents;
pub use l2_to_l1_events::L2ToL1Events;
pub use withdrawal_events::WithdrawalEvents;
