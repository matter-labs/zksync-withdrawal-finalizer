#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Crates that listens to events both on L1 and L2.

mod block_events;
mod error;
mod withdrawal_events;

use std::time::Duration;

use client::WithdrawalEvent;
pub use error::{Error, Result};

pub(crate) const RECONNECT_BACKOFF: Duration = Duration::from_secs(1);
pub use block_events::BlockEvents;
use ethers::types::{Address, H256};
pub use withdrawal_events::WithdrawalEvents;

/// All L2 Events the service is interested in.
#[derive(Debug)]
pub enum L2Event {
    /// A withdrawal event.
    Withdrawal(WithdrawalEvent),

    /// Token initialization event.
    L2TokenInitEvent(L2TokenInitEvent),
}

impl From<WithdrawalEvent> for L2Event {
    fn from(value: WithdrawalEvent) -> Self {
        Self::Withdrawal(value)
    }
}

impl From<L2TokenInitEvent> for L2Event {
    fn from(value: L2TokenInitEvent) -> Self {
        Self::L2TokenInitEvent(value)
    }
}

/// Information on the deployment of a token to L2 from
/// `BridgeInitialize` event.
#[derive(Debug)]
pub struct L2TokenInitEvent {
    /// Address of the token on l1
    pub l1_token_address: Address,

    /// Address of the token on l2
    pub l2_token_address: Address,

    /// Name of the token
    pub name: String,

    /// Symbol of the token
    pub symbol: String,

    /// Decimals
    pub decimals: u8,

    /// Number of miniblock on l2 where this deployment happened
    pub l2_block_number: u64,

    /// Transaction on l2 in which the event happened
    pub initialization_transaction: H256,
}
