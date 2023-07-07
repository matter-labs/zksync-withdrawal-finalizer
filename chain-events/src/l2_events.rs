use std::{collections::HashSet, sync::Arc};

use client::{
    contracts_deployer::codegen::ContractDeployedFilter,
    ethtoken::codegen::WithdrawalFilter,
    l2standard_token::codegen::{
        BridgeBurnFilter, BridgeInitializationFilter, BridgeInitializeFilter,
    },
    zksync_types::Log as ZksyncLog,
    WithdrawalEvent, ZksyncMiddleware, DEPLOYER_ADDRESS, ETH_ADDRESS, ETH_TOKEN_ADDRESS,
};
use ethers::{
    abi::{Address, RawLog},
    contract::EthEvent,
    providers::{Middleware, Provider, PubsubClient, Ws},
    types::{BlockNumber, Filter, Log},
};

use futures::{Sink, SinkExt, StreamExt};

use crate::{Error, L2Event, L2TokenInitEvent, Result, RECONNECT_BACKOFF};

/// A convenience multiplexer for withdrawal-related events.
pub struct L2Events {
    url: String,
    l2_erc20_bridge_addr: Address,
    tokens: HashSet<Address>,
}

impl L2Events {
    /// Create a new `WithdrawalEvents` structure.
    ///
    /// # Arguments
    ///
    /// * `middleware`: THe middleware to perform requests with.
    pub fn new(url: &str, l2_erc20_bridge_addr: Address, mut tokens: HashSet<Address>) -> Self {
        tokens.insert(ETH_TOKEN_ADDRESS);
        tokens.insert(ETH_ADDRESS);
        tokens.insert(DEPLOYER_ADDRESS);

        Self {
            url: url.to_string(),
            l2_erc20_bridge_addr,
            tokens,
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

    async fn query_past_token_init_events<M, B, S>(
        &mut self,
        from_block: B,
        to_block: B,
        sender: &mut S,
        middleware: M,
    ) -> Result<()>
    where
        B: Into<BlockNumber> + Copy,
        M: ZksyncMiddleware,
        <M as Middleware>::Provider: PubsubClient,
        S: Sink<L2Event> + Unpin,
        <S as Sink<L2Event>>::Error: std::fmt::Debug,
    {
        let from_block: BlockNumber = from_block.into();
        let to_block: BlockNumber = to_block.into();
        vlog::debug!("querying past token events {from_block:?} - {to_block:?}");

        // Query all deployment events emitted by Deployer and with address of l2_erc20_bridge_addr
        // as a topic1. This narrows down the query to basically only the needed results so
        // all of them can be returned in one call.
        let filter = Filter::new()
            .from_block(from_block)
            .to_block(to_block)
            .address(DEPLOYER_ADDRESS)
            .topic0(vec![ContractDeployedFilter::signature()])
            .topic1(self.l2_erc20_bridge_addr);

        // `get_logs` are used because there are not that many events
        // expected and `get_logs_paginated` contains a bug that incorrectly
        // calculates the range of the last batch.
        let logs = middleware
            .get_logs(&filter)
            .await
            .map_err(|e| Error::Middleware(e.to_string()))?;

        for log in logs {
            let raw_log: RawLog = log.clone().into();

            if let Some((l2_event, address)) = self
                .try_bridge_initialize_event(&log, &raw_log, &middleware)
                .await?
            {
                if self.tokens.insert(address) {
                    sender.send(l2_event.into()).await.unwrap();
                }
            }
        }

        Ok(())
    }

    // Given a `Log` try to figure out if this is a `BridgeBurnFilter`
    // event.
    //
    // If such event is found, send it to `sender` and return `true`.
    async fn try_bridge_burn_event(log: &Log, raw_log: &RawLog) -> Option<WithdrawalEvent> {
        if let Ok(burn_event) = BridgeBurnFilter::decode_log(raw_log) {
            if let (Some(tx_hash), Some(block_number)) = (log.transaction_hash, log.block_number) {
                metrics::increment_counter!("watcher.chain_events.bridge_burn_events");
                let we = WithdrawalEvent {
                    tx_hash,
                    block_number: block_number.as_u64(),
                    token: log.address,
                    amount: burn_event.amount,
                };
                return Some(we);
            }
        }
        None
    }

    // Given a `Log` try to figure out if this is a `WithdrawalFilter`
    // event.
    //
    // If such event is found, send it to `sender` and return `true`.
    async fn try_withdrawal_event(log: &Log, raw_log: &RawLog) -> Option<WithdrawalEvent> {
        if let Ok(withdrawal_event) = WithdrawalFilter::decode_log(raw_log) {
            if let (Some(tx_hash), Some(block_number)) = (log.transaction_hash, log.block_number) {
                metrics::increment_counter!("watcher.chain_events.withdrawal_events");
                let we = WithdrawalEvent {
                    tx_hash,
                    block_number: block_number.as_u64(),
                    token: log.address,
                    amount: withdrawal_event.amount,
                };
                return Some(we);
            }
        }
        None
    }

    async fn look_for_bridge_initialize_event<M>(
        log: &Log,
        raw_log: &RawLog,
        middleware: M,
    ) -> Result<Option<ZksyncLog>>
    where
        M: ZksyncMiddleware,
        <M as Middleware>::Provider: PubsubClient,
    {
        let bridge_init_topics = vec![
            BridgeInitializeFilter::signature(),
            BridgeInitializationFilter::signature(),
        ];

        // If this is the deployment event get the corresponding transaction
        // and try to find one of bridge initialization events in it.
        if ContractDeployedFilter::decode_log(raw_log).is_ok() {
            let tx = middleware
                .zks_get_transaction_receipt(
                    log.transaction_hash
                        .expect("a log from a transaction always has a tx hash; qed"),
                )
                .await
                .map_err(|e| Error::Middleware(e.to_string()))?;

            for log in tx.logs {
                if bridge_init_topics.contains(&log.topics[0]) {
                    return Ok(Some(log));
                }
            }
        }
        Ok(None)
    }

    // Given a `Log` returned from by `ContractDeployedFilter` query try to figure out if
    // this was the erc20 token deployment event.
    //
    // If such an event is found, send it to `sender.
    //
    // # Returns
    //
    // The address of the token if a bridge init event is found.
    async fn try_bridge_initialize_event<M>(
        &self,
        log: &Log,
        raw_log: &RawLog,
        middleware: M,
    ) -> Result<Option<(L2TokenInitEvent, Address)>>
    where
        M: ZksyncMiddleware,
        <M as Middleware>::Provider: PubsubClient,
    {
        let Some(bridge_init_log) = Self::look_for_bridge_initialize_event(
            log,
            raw_log,
            middleware,
        ).await? else {
            return Ok(None)
        };

        let raw_log: RawLog = bridge_init_log.clone().into();

        let mut bridge_init_event: Option<BridgeInitializeFilter> = None;

        if let Ok(bridge_initialize) = BridgeInitializationFilter::decode_log(&raw_log) {
            bridge_init_event = Some(bridge_initialize.into());
        }
        if let Ok(bridge_initialize) = BridgeInitializeFilter::decode_log(&raw_log) {
            bridge_init_event = Some(bridge_initialize);
        }

        let Some(bridge_init_event) = bridge_init_event else { return Ok(None) };

        if self.tokens.contains(&bridge_init_log.address) {
            return Ok(None);
        }

        let l2_event = L2TokenInitEvent {
            l1_token_address: bridge_init_event.l_1_token,
            l2_token_address: bridge_init_log.address,
            name: bridge_init_event.name,
            symbol: bridge_init_event.symbol,
            decimals: bridge_init_event.decimals,
            l2_block_number: bridge_init_log
                .block_number
                .expect("a mined block always has a block number; qed")
                .as_u64(),
            initialization_transaction: bridge_init_log
                .transaction_hash
                .expect("logs from mined transaction always have a known hash; qed"),
        };

        Ok(Some((l2_event, bridge_init_log.address)))
    }

    /// Run the main loop with re-connecting on websocket disconnects
    //
    // Websocket subscriptions do not work well with reconnections
    // in `ethers-rs`: https://github.com/gakonst/ethers-rs/issues/2418
    // This function is a workaround for that and implements manual re-connecting.
    pub async fn run_with_reconnects<B, S>(
        mut self,
        from_block: B,
        last_seen_l2_token_block: B,
        sender: S,
    ) -> Result<()>
    where
        B: Into<BlockNumber> + Copy,
        S: Sink<L2Event> + Unpin + Clone,
        <S as Sink<L2Event>>::Error: std::fmt::Debug,
    {
        let mut from_block: BlockNumber = from_block.into();
        let mut last_seen_l2_token_block: BlockNumber = last_seen_l2_token_block.into();
        loop {
            let Some(provider_l1) = self.connect().await else {
                tokio::time::sleep(RECONNECT_BACKOFF).await;
                continue
            };

            let middleware = Arc::new(provider_l1);

            match self
                .run(
                    from_block,
                    last_seen_l2_token_block,
                    sender.clone(),
                    middleware,
                )
                .await
            {
                Ok(RunResult::StoppedAtBlock { block }) => {
                    from_block = block;
                    last_seen_l2_token_block = block;
                }
                Err(e) => {
                    vlog::warn!("Withdrawal events worker failed with {e}");
                }
            }
        }
    }
}

enum RunResult {
    StoppedAtBlock { block: BlockNumber },
}

impl L2Events {
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
        &mut self,
        from_block: B,
        last_seen_l2_token_block: B,
        mut sender: S,
        middleware: M,
    ) -> Result<RunResult>
    where
        B: Into<BlockNumber> + Copy,
        M: ZksyncMiddleware,
        <M as Middleware>::Provider: PubsubClient,
        S: Sink<L2Event> + Unpin,
        <S as Sink<L2Event>>::Error: std::fmt::Debug,
    {
        let mut last_seen_block: BlockNumber = from_block.into();
        let last_seen_l2_token_block: BlockNumber = last_seen_l2_token_block.into();
        let from_block: BlockNumber = from_block.into();

        let topic0 = vec![
            ContractDeployedFilter::signature(),
            BridgeBurnFilter::signature(),
            WithdrawalFilter::signature(),
        ];

        vlog::debug!("last_seen_l2_token_block {last_seen_l2_token_block:?}");
        vlog::debug!("from_block {from_block:?}");

        if last_seen_l2_token_block.as_number() <= from_block.as_number() {
            self.query_past_token_init_events(
                last_seen_l2_token_block,
                from_block,
                &mut sender,
                &middleware,
            )
            .await?;
        }
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
            .address(self.tokens.iter().cloned().collect::<Vec<_>>())
            .topic0(topic0.clone());

        let filter = Filter::new()
            .from_block(latest_block)
            .address(self.tokens.iter().cloned().collect::<Vec<_>>())
            .topic0(topic0);

        let past_logs = middleware.get_logs_paginated(&past_filter, 10000);
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

            if let Some(we) = Self::try_withdrawal_event(&log, &raw_log).await {
                sender.send(we.into()).await.unwrap();
                continue;
            }

            if let Some(we) = Self::try_bridge_burn_event(&log, &raw_log).await {
                sender.send(we.into()).await.unwrap();
                continue;
            }

            match self
                .try_bridge_initialize_event(&log, &raw_log, &middleware)
                .await
            {
                Ok(Some((l2_event, address))) => {
                    if self.tokens.insert(address) {
                        sender.send(l2_event.into()).await.unwrap();
                        vlog::info!("Restarting on the token added event {address}");
                        break;
                    }
                }

                Err(_) => break,
                _ => (),
            }
        }
        vlog::info!("withdrawal streams being closed");

        Ok(RunResult::StoppedAtBlock {
            block: last_seen_block,
        })
    }
}
