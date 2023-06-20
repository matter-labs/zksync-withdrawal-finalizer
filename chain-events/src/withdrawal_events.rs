use std::sync::Arc;

use client::{
    ethtoken::codegen::WithdrawalFilter, l2standard_token::codegen::BridgeBurnFilter,
    WithdrawalEvent, ETH_TOKEN_ADDRESS,
};
use ethers::{
    abi::{Address, RawLog},
    contract::EthEvent,
    providers::{Middleware, Provider, PubsubClient, Ws},
    types::{BlockNumber, Filter},
};

use futures::{Sink, SinkExt, StreamExt};

use crate::{Error, Result, RECONNECT_BACKOFF};

/// A convenience multiplexer for withdrawal-related events.
pub struct WithdrawalEvents {
    url: String,
}

impl WithdrawalEvents {
    /// Create a new `WithdrawalEvents` structure.
    ///
    /// # Arguments
    ///
    /// * `middleware`: THe middleware to perform requests with.
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
        }
    }

    async fn connect(&self) -> Option<Provider<Ws>> {
        match Provider::<Ws>::connect_with_reconnects(&self.url, 0).await {
            Ok(p) => {
                metrics::increment_counter!(
                    "watcher.chain_events.withdrawal_events.successful_reconnects"
                );
                Some(p)
            }
            Err(e) => {
                vlog::warn!("Withdrawal events stream reconnect attempt failed: {e}");
                metrics::increment_counter!(
                    "watcher.chain_events.withdrawal_events.reconnects_on_error"
                );
                None
            }
        }
    }

    /// Run the main loop with re-connecting on websocket disconnects
    //
    // Websocket subscriptions do not work well with reconnections
    // in `ethers-rs`: https://github.com/gakonst/ethers-rs/issues/2418
    // This function is a workaround for that and implements manual re-connecting.
    pub async fn run_with_reconnects<B, S>(
        self,
        addresses: Vec<Address>,
        from_block: B,
        sender: S,
    ) -> Result<()>
    where
        B: Into<BlockNumber> + Copy,
        S: Sink<WithdrawalEvent> + Unpin + Clone,
        <S as Sink<WithdrawalEvent>>::Error: std::fmt::Debug,
    {
        let mut from_block: BlockNumber = from_block.into();

        loop {
            let Some(provider_l1) = self.connect().await else { continue };

            let middleware = Arc::new(provider_l1);

            match Self::run(addresses.clone(), from_block, sender.clone(), middleware).await {
                Ok(block) => from_block = block,
                Err(e) => {
                    vlog::warn!("Withdrawal events worker failed with {e}");
                }
            }
            tokio::time::sleep(RECONNECT_BACKOFF).await;
        }
    }
}

impl WithdrawalEvents {
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
    async fn run<B, S, M>(
        mut addresses: Vec<Address>,
        from_block: B,
        mut sender: S,
        middleware: M,
    ) -> Result<BlockNumber>
    where
        B: Into<BlockNumber> + Copy,
        M: Middleware,
        <M as Middleware>::Provider: PubsubClient,
        S: Sink<WithdrawalEvent> + Unpin,
        <S as Sink<WithdrawalEvent>>::Error: std::fmt::Debug,
    {
        let mut last_seen_block: BlockNumber = from_block.into();

        addresses.push(ETH_TOKEN_ADDRESS);

        let latest_block = middleware
            .get_block(BlockNumber::Latest)
            .await
            .map_err(|e| Error::Middleware(e.to_string()))?
            .expect("last block number always exists in a live network; qed")
            .number
            .expect("last block always has a number; qed");

        let past_filter = Filter::new()
            .from_block(from_block)
            .to_block(latest_block)
            .address(addresses.clone())
            .topic0(vec![
                BridgeBurnFilter::signature(),
                WithdrawalFilter::signature(),
            ]);

        let filter = Filter::new()
            .from_block(latest_block)
            .address(addresses)
            .topic0(vec![
                BridgeBurnFilter::signature(),
                WithdrawalFilter::signature(),
            ]);

        let past_logs = middleware.get_logs_paginated(&past_filter, 256);
        let current_logs = middleware
            .subscribe_logs(&filter)
            .await
            .map_err(|e| Error::Middleware(e.to_string()))?;

        let mut logs = past_logs.chain(current_logs.map(Ok));

        while let Some(log) = logs.next().await {
            let log = match log {
                Err(e) => {
                    vlog::warn!("L2 withdrawal events stream ended with {e}");
                    break;
                }
                Ok(log) => log,
            };
            let raw_log: RawLog = log.clone().into();
            metrics::increment_counter!("watcher.chain_events.l2_logs_received");

            if let Some(block_number) = log.block_number {
                last_seen_block = block_number.into();
            }

            if let Ok(burn_event) = BridgeBurnFilter::decode_log(&raw_log) {
                if let (Some(tx_hash), Some(block_number)) =
                    (log.transaction_hash, log.block_number)
                {
                    metrics::increment_counter!("watcher.chain_events.bridge_burn_events");
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
                    metrics::increment_counter!("watcher.chain_events.withdrawal_events");
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
        vlog::info!("withdrawal streams being closed");

        Ok(last_seen_block)
    }
}
