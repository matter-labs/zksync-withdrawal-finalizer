//! ABI wrappers for the `L2StandardToken` contract.

use std::sync::Arc;

use ethers::{
    abi::RawLog,
    contract::EthEvent,
    providers::{Middleware, PubsubClient},
    types::{Address, BlockNumber, Filter},
};
use futures::{Sink, SinkExt, StreamExt};

use crate::{
    ethtoken::codegen::WithdrawalFilter, Error, Result, WithdrawalEvent, ETH_TOKEN_ADDRESS,
};

use self::codegen::BridgeBurnFilter;

#[allow(missing_docs)]
pub mod codegen {
    use ethers::prelude::abigen;

    abigen!(
        L2StandardToken,
        "$CARGO_MANIFEST_DIR/src/contracts/IL2StandardToken.json",
    );
}

/// A convenience multiplexer for withdrawal-related events.
pub struct WithdrawalEventsStream<M> {
    middleware: Arc<M>,
}

impl<M> WithdrawalEventsStream<M>
where
    M: Middleware,
{
    /// Create a new `WithdrawalEvents` structure.
    ///
    /// # Arguments
    ///
    /// * `middleware`: THe middleware to perform requests with.
    pub async fn new(middleware: Arc<M>) -> Result<Self> {
        Ok(Self { middleware })
    }
}

impl<M> WithdrawalEventsStream<M>
where
    M: Middleware,
    <M as Middleware>::Provider: PubsubClient,
{
    /// A convenience function that listens for all withdrawal events on L2
    ///
    /// For more reasoning about the necessity of this function
    /// check the similar [`BlockEvents::run()`].
    ///
    /// # Arguments
    ///
    /// * `addresses`: The address of the ERC20 tokens on L1 to monitor
    /// * `from_block`: Query the chain from this particular block
    /// * `sender`: The `Sink` to send received events into.
    pub async fn run<B, S>(
        self,
        mut addresses: Vec<Address>,
        from_block: B,
        mut sender: S,
    ) -> Result<()>
    where
        B: Into<BlockNumber> + Copy,
        S: Sink<WithdrawalEvent> + Unpin,
        <S as Sink<WithdrawalEvent>>::Error: std::fmt::Debug,
    {
        addresses.push(ETH_TOKEN_ADDRESS);

        let filter = Filter::new()
            .from_block(from_block)
            .address(addresses)
            .topic0(vec![
                BridgeBurnFilter::signature(),
                WithdrawalFilter::signature(),
            ]);

        let mut logs = self
            .middleware
            .subscribe_logs(&filter)
            .await
            .map_err(|e| Error::Middleware(format!("{e}")))?;

        while let Some(log) = logs.next().await {
            let raw_log: RawLog = log.clone().into();

            if let Ok(burn_event) = BridgeBurnFilter::decode_log(&raw_log) {
                if let (Some(tx_hash), Some(block_number)) =
                    (log.transaction_hash, log.block_number)
                {
                    let we = WithdrawalEvent {
                        tx_hash,
                        block_number: block_number.as_u64(),
                        token: log.address,
                        amount: burn_event.amount,
                    };
                    sender.send(we).await.map_err(|_| Error::ChannelClosed)?;
                }
                continue;
            }

            if let Ok(withdrawal_event) = WithdrawalFilter::decode_log(&raw_log) {
                if let (Some(tx_hash), Some(block_number)) =
                    (log.transaction_hash, log.block_number)
                {
                    let we = WithdrawalEvent {
                        tx_hash,
                        block_number: block_number.as_u64(),
                        token: log.address,
                        amount: withdrawal_event.amount,
                    };
                    sender.send(we).await.map_err(|_| Error::ChannelClosed)?;
                }
            }
        }
        log::info!("withdrawal streams being closed");

        Ok(())
    }
}
