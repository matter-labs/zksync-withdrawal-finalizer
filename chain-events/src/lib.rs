#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Crates that listens to events both on L1 and L2.

mod block_events;
mod error;
mod l2_events;

use std::time::Duration;

use client::WithdrawalEvent;
pub use error::{Error, Result};

pub(crate) const RECONNECT_BACKOFF: Duration = Duration::from_secs(1);
pub use block_events::BlockEvents;
use ethers::{
    providers::{LogQueryError, ProviderError},
    types::{Address, H256},
};
pub use l2_events::L2EventsListener;

/// All L2 Events the service is interested in.
#[derive(Debug)]
pub enum L2Event {
    /// There has been a restart from block of given number
    RestartedFromBlock(u64),

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

pub(crate) fn rpc_query_too_large(e: &LogQueryError<ProviderError>) -> bool {
    if let LogQueryError::LoadLogsError(ProviderError::JsonRpcClientError(e)) = e {
        if let Some(e) = e.as_error_response() {
            return e
                .message
                .starts_with("Query returned more than 10000 results. Try with this block range");
        }
    }

    false
}
