use std::sync::Arc;

use client::{
    ethtoken::codegen::WithdrawalFilter, l2standard_token::codegen::BridgeBurnFilter,
    WithdrawalEvent, ETH_TOKEN_ADDRESS,
};
use ethers::{
    abi::{Address, RawLog},
    contract::EthEvent,
    providers::{Middleware, PubsubClient},
    types::{BlockNumber, Filter},
};

use futures::{Sink, SinkExt, StreamExt};

use crate::Result;

/// A convenience multiplexer for withdrawal-related events.
pub struct WithdrawalEvents<M> {
    middleware: Arc<M>,
}

impl<M> WithdrawalEvents<M>
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

impl<M> WithdrawalEvents<M>
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

        let mut logs = self.middleware.subscribe_logs(&filter).await?;

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
                    sender.send(we).await.unwrap();
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
                    sender.send(we).await.unwrap();
                }
            }
        }
        log::info!("withdrawal streams being closed");

        Ok(())
    }
}
