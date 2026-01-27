use std::{collections::HashSet, sync::Arc};

use futures::{Sink, SinkExt, StreamExt};

use client::{
    contracts_deployer::codegen::ContractDeployedFilter,
    ethtoken::codegen::WithdrawalFilter,
    l2standard_token::codegen::{
        BridgeBurnFilter, BridgeInitializationFilter, BridgeInitializeFilter,
    },
    zksync_types::{Log as ZksyncLog, TransactionReceipt as ZksyncTransactionReceipt},
    WithdrawalEvent, ZksyncMiddleware, DEPLOYER_ADDRESS, ETH_ADDRESS, ETH_TOKEN_ADDRESS,
};
use ethers::{
    abi::{Address, RawLog},
    contract::EthEvent,
    prelude::EthLogDecode,
    providers::{Middleware, Provider, PubsubClient, Ws},
    types::{BlockNumber, Filter, Log},
};

use crate::{
    metrics::CHAIN_EVENTS_METRICS, rpc_query_too_large, Error, L2Event, L2TokenInitEvent, Result,
    RECONNECT_BACKOFF,
};
use ethers_log_decode::EthLogDecode;

struct NewTokenAdded;

/// A convenience multiplexer for withdrawal-related events.
pub struct L2EventsListener {
    url: String,
    token_deployer_addrs: Vec<Address>,
    tokens: HashSet<Address>,
}

#[derive(EthLogDecode)]
enum L2Events {
    BridgeBurn(BridgeBurnFilter),
    Withdrawal(WithdrawalFilter),
    #[allow(unused)]
    ContractDeployed(ContractDeployedFilter),
}

#[derive(EthLogDecode)]
enum BridgeInitEvents {
    BridgeInitializeFilter(BridgeInitializeFilter),
    BridgeInitializationFilter(BridgeInitializationFilter),
}

const PAGINATION_STEP: u64 = 10_000;
const PAGINATION_INCREASE_STEP: u64 = 256;
// usually queries break at 10k return results
// if restarted on that limit make sure it is passed.
const SUCCESSFUL_LOGS_TO_UPSCALE: i32 = 11_000;

impl L2EventsListener {
    /// Create a new `WithdrawalEvents` structure.
    ///
    /// # Arguments
    ///
    /// * `middleware`: The middleware to perform requests with.
    pub fn new(
        url: &str,
        token_deployer_addrs: Vec<Address>,
        mut tokens: HashSet<Address>,
        finalize_eth_token: bool,
    ) -> Self {
        if finalize_eth_token {
            tokens.insert(ETH_TOKEN_ADDRESS);
            tokens.insert(ETH_ADDRESS);
        }
        tokens.insert(DEPLOYER_ADDRESS);

        Self {
            url: url.to_string(),
            token_deployer_addrs,
            tokens,
        }
    }

    async fn connect(&self) -> Option<Provider<Ws>> {
        match Provider::<Ws>::connect_with_reconnects(&self.url, 0).await {
            Ok(p) => {
                CHAIN_EVENTS_METRICS.successful_l2_reconnects.inc();
                Some(p)
            }
            Err(e) => {
                tracing::warn!("Withdrawal events stream reconnect attempt failed: {e}");
                CHAIN_EVENTS_METRICS.reconnects_on_error.inc();
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
        tracing::debug!("querying past token events {from_block:?} - {to_block:?}");

        // Query all deployment events emitted by Deployer and with address of l2_erc20_bridge_addr
        // as a topic1. This narrows down the query to basically only the needed results so
        // all of them can be returned in one call.
        let filter = Filter::new()
            .from_block(from_block)
            .to_block(to_block)
            .address(DEPLOYER_ADDRESS)
            .topic0(vec![ContractDeployedFilter::signature()])
            .topic1(self.token_deployer_addrs.clone());

        // `get_logs` are used because there are not that many events
        // expected and `get_logs_paginated` contains a bug that incorrectly
        // calculates the range of the last batch.
        let logs = middleware
            .get_logs(&filter)
            .await
            .map_err(|e| Error::Middleware(e.to_string()))?;

        for log in logs {
            let tx = middleware
                .zks_get_transaction_receipt(log.transaction_hash.unwrap_or_else(|| {
                    panic!(
                        "a log from a transaction always has a tx hash {:?}; qed",
                        log
                    )
                }))
                .await
                .map_err(|e| Error::Middleware(e.to_string()))?;

            let Some(bridge_init_log) = look_for_bridge_initialize_event(tx) else {
                continue;
            };

            if let Some((l2_event, address)) = self.bridge_initialize_event(bridge_init_log)? {
                if self.tokens.insert(address) {
                    let event = l2_event.into();

                    tracing::info!("sending token event {event:?}");
                    sender
                        .send(event)
                        .await
                        .map_err(|_| Error::ChannelClosing)?;
                }
            }
        }

        Ok(())
    }

    // Given a `Log` returned from by `ContractDeployedFilter` query try to figure out if
    // this was the erc20 token deployment event.
    //
    // If such an event is found, send it to `sender.
    //
    // # Returns
    //
    // The address of the token if a bridge init event is found.
    fn bridge_initialize_event(
        &self,
        bridge_init_log: ZksyncLog,
    ) -> Result<Option<(L2TokenInitEvent, Address)>> {
        let raw_log: RawLog = bridge_init_log.clone().into();

        let Ok(bridge_initialize) = BridgeInitEvents::decode_log(&raw_log) else {
            return Ok(None);
        };

        if self.tokens.contains(&bridge_init_log.address) {
            return Ok(None);
        }

        let (l1_token_address, name, symbol, decimals) = match bridge_initialize {
            BridgeInitEvents::BridgeInitializeFilter(bi) => {
                (bi.l_1_token, bi.name, bi.symbol, bi.decimals)
            }
            BridgeInitEvents::BridgeInitializationFilter(bi) => {
                (bi.l_1_token, bi.name, bi.symbol, bi.decimals)
            }
        };

        let l2_event = L2TokenInitEvent {
            l1_token_address,
            l2_token_address: bridge_init_log.address,
            name,
            symbol,
            decimals,
            l2_block_number: bridge_init_log
                .block_number
                .unwrap_or_else(|| {
                    panic!(
                        "a mined block always has a block number {:?}; qed",
                        bridge_init_log
                    )
                })
                .as_u64(),
            initialization_transaction: bridge_init_log.transaction_hash.unwrap_or_else(|| {
                panic!(
                    "logs from mined transaction always have a known hash {:?}; qed",
                    bridge_init_log
                )
            }),
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
        mut sender: S,
    ) -> Result<()>
    where
        B: Into<BlockNumber> + Copy,
        S: Sink<L2Event> + Unpin + Clone,
        <S as Sink<L2Event>>::Error: std::fmt::Debug,
    {
        let mut pagination = PAGINATION_STEP;
        let mut from_block: BlockNumber = from_block.into();
        let mut last_seen_l2_token_block: BlockNumber = last_seen_l2_token_block.into();
        loop {
            let Some(provider_l1) = self.connect().await else {
                tokio::time::sleep(RECONNECT_BACKOFF).await;
                continue;
            };

            let middleware = Arc::new(provider_l1);
            CHAIN_EVENTS_METRICS.query_pagination.set(pagination as i64);

            match self
                .run(
                    from_block,
                    last_seen_l2_token_block,
                    sender.clone(),
                    pagination,
                    middleware,
                )
                .await
            {
                Ok((last_seen_block, reason)) => {
                    from_block = last_seen_block;
                    last_seen_l2_token_block = last_seen_block;

                    match reason {
                        RunResult::PaginationTooLarge => {
                            let pagination_old = pagination;
                            if pagination > 2 {
                                pagination /= 2;
                                tracing::debug!(
                                    "Decreasing pagination from {pagination_old} to {pagination}",
                                );
                            }
                        }
                        RunResult::AttemptPaginationIncrease => {
                            let pagination_old = pagination;
                            if pagination + PAGINATION_INCREASE_STEP < PAGINATION_STEP {
                                pagination += PAGINATION_INCREASE_STEP;
                                tracing::debug!(
                                    "Increasing pagination from {pagination_old} to {pagination}",
                                );
                            }
                        }
                        _ => (),
                    }
                }
                Err(e) => {
                    tracing::warn!("Withdrawal events worker failed with {e}");
                }
            }

            sender
                .send(L2Event::RestartedFromBlock(
                    from_block
                        .as_number()
                        .expect("block number always exists; qed")
                        .as_u64(),
                ))
                .await
                .map_err(|_| Error::ChannelClosing)?;
        }
    }
}

#[derive(PartialEq)]
enum RunResult {
    PaginationTooLarge,
    AttemptPaginationIncrease,
    OtherError,
}

impl L2EventsListener {
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
        pagination_step: u64,
        middleware: M,
    ) -> Result<(BlockNumber, RunResult)>
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

        let past_topic0 = vec![BridgeBurnFilter::signature(), WithdrawalFilter::signature()];

        let topic0 = vec![
            ContractDeployedFilter::signature(),
            BridgeBurnFilter::signature(),
            WithdrawalFilter::signature(),
        ];

        tracing::info!("topic0 {topic0:?}");

        tracing::debug!("last_seen_l2_token_block {last_seen_l2_token_block:?}");
        tracing::debug!("from_block {from_block:?}");

        let latest_block = middleware
            .get_block(BlockNumber::Latest)
            .await
            .map_err(|e| Error::Middleware(e.to_string()))?
            .expect("last block number always exists in a live network; qed")
            .number
            .expect("last block always has a number; qed");

        if last_seen_l2_token_block.as_number() <= from_block.as_number() {
            self.query_past_token_init_events(
                last_seen_l2_token_block,
                BlockNumber::Number(latest_block),
                &mut sender,
                &middleware,
            )
            .await?;
        }

        let mut tokens = self.tokens.iter().cloned().collect::<Vec<_>>();
        tokens.extend_from_slice(self.token_deployer_addrs.as_slice());

        tracing::info!("Listeing to events from tokens {tokens:?}");

        let past_filter = Filter::new()
            .from_block(from_block)
            .to_block(latest_block)
            .address(tokens.clone())
            .topic0(past_topic0.clone());

        let filter = Filter::new()
            .from_block(latest_block + 1)
            .address(tokens)
            .topic0(topic0);

        tracing::info!("filter past {past_filter:#?}");
        tracing::info!("filter {filter:#?}");

        let past_logs = middleware.get_logs_paginated(&past_filter, pagination_step);
        let current_logs = middleware
            .subscribe_logs(&filter)
            .await
            .map_err(|e| Error::Middleware(e.to_string()))?;

        let mut logs = past_logs.chain(current_logs.map(Ok));
        let mut successful_logs = 0;

        while let Some(log) = logs.next().await {
            let log = match log {
                Err(e) => {
                    tracing::info!("L2 withdrawal events stream ended with {e:?}");
                    if rpc_query_too_large(&e) {
                        return Ok((last_seen_block, RunResult::PaginationTooLarge));
                    }

                    break;
                }
                Ok(log) => log,
            };
            let raw_log: RawLog = log.clone().into();
            CHAIN_EVENTS_METRICS.l2_logs_received.inc();
            successful_logs += 1;

            if should_attempt_pagination_increase(pagination_step, successful_logs) {
                return Ok((last_seen_block, RunResult::AttemptPaginationIncrease));
            }

            if let Some(block_number) = log.block_number {
                last_seen_block = block_number.into();
            }

            if let Ok(l2_event) = L2Events::decode_log(&raw_log) {
                if let L2Events::ContractDeployed(_) = l2_event {
                    if log.topics.get(1) != Some(&DEPLOYER_ADDRESS.into()) {
                        continue;
                    };
                }
                CHAIN_EVENTS_METRICS.l2_logs_decoded.inc();

                match self
                    .process_l2_event(&log, &l2_event, &mut sender, &middleware)
                    .await
                {
                    Ok(Some(_new_token_added)) => {
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("Stopping event loop with an error {e}");
                        break;
                    }
                    _ => (),
                };
            }
        }

        tracing::info!("withdrawal streams being closed");

        Ok((last_seen_block, RunResult::OtherError))
    }

    async fn process_l2_event<M, S>(
        &mut self,
        log: &Log,
        l2_event: &L2Events,
        sender: &mut S,
        middleware: M,
    ) -> Result<Option<NewTokenAdded>>
    where
        M: ZksyncMiddleware,
        <M as Middleware>::Provider: PubsubClient,
        S: Sink<L2Event> + Unpin,
        <S as Sink<L2Event>>::Error: std::fmt::Debug,
    {
        if let (Some(tx_hash), Some(block_number)) = (log.transaction_hash, log.block_number) {
            match l2_event {
                L2Events::BridgeBurn(BridgeBurnFilter { amount, .. }) => {
                    CHAIN_EVENTS_METRICS.withdrawal_events.inc();

                    let we = WithdrawalEvent {
                        tx_hash,
                        block_number: block_number.as_u64(),
                        token: log.address,
                        amount: *amount,
                        l1_receiver: None,
                    };
                    let event = we.into();
                    tracing::info!("sending withdrawal event {event:?}");
                    sender
                        .send(event)
                        .await
                        .map_err(|_| Error::ChannelClosing)?;
                }
                L2Events::Withdrawal(WithdrawalFilter {
                    amount,
                    l_1_receiver,
                    ..
                }) => {
                    CHAIN_EVENTS_METRICS.withdrawal_events.inc();

                    let we = WithdrawalEvent {
                        tx_hash,
                        block_number: block_number.as_u64(),
                        token: log.address,
                        amount: *amount,
                        l1_receiver: Some(*l_1_receiver),
                    };
                    let event = we.into();
                    tracing::info!("sending withdrawal event {event:?}");
                    sender
                        .send(event)
                        .await
                        .map_err(|_| Error::ChannelClosing)?;
                }
                L2Events::ContractDeployed(_) => {
                    let tx = middleware
                        .zks_get_transaction_receipt(log.transaction_hash.unwrap_or_else(|| {
                            panic!(
                                "a log from a transaction always has a tx hash {:?}; qed",
                                log
                            )
                        }))
                        .await
                        .map_err(|e| Error::Middleware(e.to_string()))?;

                    let Some(bridge_init_log) = look_for_bridge_initialize_event(tx) else {
                        return Ok(None);
                    };

                    if let Some((l2_event, address)) =
                        self.bridge_initialize_event(bridge_init_log)?
                    {
                        if self.tokens.insert(address) {
                            CHAIN_EVENTS_METRICS.new_token_added.inc();

                            let event = l2_event.into();
                            tracing::info!("sending new token event {event:?}");
                            sender
                                .send(event)
                                .await
                                .map_err(|_| Error::ChannelClosing)?;
                            tracing::info!("Restarting on the token added event {address}");
                            return Ok(Some(NewTokenAdded));
                        }
                    }
                }
            }
        }

        Ok(None)
    }
}

fn should_attempt_pagination_increase(pagination_step: u64, successful_logs: i32) -> bool {
    pagination_step < PAGINATION_STEP && successful_logs > SUCCESSFUL_LOGS_TO_UPSCALE
}

fn look_for_bridge_initialize_event(tx: ZksyncTransactionReceipt) -> Option<ZksyncLog> {
    let bridge_init_topics = [
        BridgeInitializeFilter::signature(),
        BridgeInitializationFilter::signature(),
    ];

    tx.logs
        .into_iter()
        .find(|log| bridge_init_topics.contains(&log.topics[0]))
}
