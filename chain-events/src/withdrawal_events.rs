use std::{collections::HashSet, sync::Arc};

use client::{
    contracts_deployer::codegen::ContractDeployedFilter,
    ethtoken::codegen::WithdrawalFilter,
    l2standard_token::codegen::{
        BridgeBurnFilter, BridgeInitializationFilter, BridgeInitializeFilter,
    },
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
pub struct WithdrawalEvents {
    url: String,
    l2_erc20_bridge_addr: Address,
}

impl WithdrawalEvents {
    /// Create a new `WithdrawalEvents` structure.
    ///
    /// # Arguments
    ///
    /// * `middleware`: THe middleware to perform requests with.
    pub fn new(url: &str, l2_erc20_bridge_addr: Address) -> Self {
        Self {
            url: url.to_string(),
            l2_erc20_bridge_addr,
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
        from_block: B,
        to_block: B,
        l2_erc20_bridge_addr: Address,
        sender: &mut S,
        middleware: M,
    ) -> Result<HashSet<Address>>
    where
        B: Into<BlockNumber> + Copy,
        M: ZksyncMiddleware,
        <M as Middleware>::Provider: PubsubClient,
        S: Sink<L2Event> + Unpin,
        <S as Sink<L2Event>>::Error: std::fmt::Debug,
    {
        let from_block: BlockNumber = from_block.into();
        let to_block: BlockNumber = to_block.into();
        vlog::info!("querying past token events {from_block:?} - {to_block:?}");

        let filter = Filter::new()
            .from_block(from_block)
            .to_block(to_block)
            .address(DEPLOYER_ADDRESS)
            .topic0(vec![ContractDeployedFilter::signature()])
            .topic1(l2_erc20_bridge_addr);

        let logs = middleware
            .get_logs(&filter)
            .await
            .map_err(|e| Error::Middleware(e.to_string()))?;

        let mut res = HashSet::new();
        for log in logs {
            let raw_log: RawLog = log.clone().into();

            if let Some(address) =
                Self::bridge_initialize_event(&log, &raw_log, &res, sender, &middleware).await?
            {
                res.insert(address);
            }
        }

        Ok(res)
    }

    async fn bridge_initialize_event<S, M>(
        log: &Log,
        raw_log: &RawLog,
        known_tokens: &HashSet<Address>,
        sender: &mut S,
        middleware: M,
    ) -> Result<Option<Address>>
    where
        M: ZksyncMiddleware,
        <M as Middleware>::Provider: PubsubClient,
        S: Sink<L2Event> + Unpin,
        <S as Sink<L2Event>>::Error: std::fmt::Debug,
    {
        let mut bridge_init_log = None;
        if ContractDeployedFilter::decode_log(raw_log).is_ok() {
            let tx = middleware
                .zks_get_transaction_receipt(
                    log.transaction_hash
                        .expect("a log from a transaction always has a tx hash; qed"),
                )
                .await
                .map_err(|e| Error::Middleware(e.to_string()))?;
            for log in tx.logs {
                if log.topics[0] == BridgeInitializeFilter::signature()
                    || log.topics[0] == BridgeInitializationFilter::signature()
                {
                    bridge_init_log = Some(log);
                    break;
                }
            }
        }
        let Some(bridge_init_log) = bridge_init_log else { return Ok(None) };
        let log = bridge_init_log;

        let raw_log: RawLog = log.clone().into();

        let mut bridge_init_event: Option<BridgeInitializeFilter> = None;
        if let Ok(bridge_initialize) = BridgeInitializationFilter::decode_log(&raw_log) {
            bridge_init_event = Some(bridge_initialize.into());
        }
        if let Ok(bridge_initialize) = BridgeInitializeFilter::decode_log(&raw_log) {
            bridge_init_event = Some(bridge_initialize);
        }

        if let Some(bridge_initialize) = bridge_init_event {
            if known_tokens.contains(&log.address) {
                return Ok(None);
            }
            let l2_event = L2TokenInitEvent {
                l1_token_address: bridge_initialize.l_1_token,
                l2_token_address: log.address,
                name: bridge_initialize.name,
                symbol: bridge_initialize.symbol,
                decimals: bridge_initialize.decimals,
                l2_block_number: log
                    .block_number
                    .expect("a mined block always has a block number; qed")
                    .as_u64(),
                initialization_transaction: log
                    .transaction_hash
                    .expect("logs from mined transaction always have a known hash; qed"),
            };
            sender.send(l2_event.into()).await.unwrap();
            Ok(Some(log.address))
        } else {
            Ok(None)
        }
    }

    /// Run the main loop with re-connecting on websocket disconnects
    //
    // Websocket subscriptions do not work well with reconnections
    // in `ethers-rs`: https://github.com/gakonst/ethers-rs/issues/2418
    // This function is a workaround for that and implements manual re-connecting.
    pub async fn run_with_reconnects<B, S>(
        self,
        mut addresses: HashSet<Address>,
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

        addresses.insert(ETH_TOKEN_ADDRESS);
        addresses.insert(ETH_ADDRESS);
        addresses.insert(DEPLOYER_ADDRESS);

        loop {
            let Some(provider_l1) = self.connect().await else {
                tokio::time::sleep(RECONNECT_BACKOFF).await;
                continue
            };

            let middleware = Arc::new(provider_l1);

            match Self::run(
                addresses.clone(),
                from_block,
                last_seen_l2_token_block.into(),
                self.l2_erc20_bridge_addr,
                sender.clone(),
                middleware,
            )
            .await
            {
                Ok(RunResult::StoppedAtBlock { block }) => from_block = block,
                Ok(RunResult::NewTokenInitialized { token, block }) => {
                    from_block = block;
                    last_seen_l2_token_block = block;
                    addresses.insert(token);
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

    NewTokenInitialized { token: Address, block: BlockNumber },
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
        mut addresses: HashSet<Address>,
        from_block: B,
        last_seen_l2_token_block: B,
        l2_erc20_bridge_addr: Address,
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

        vlog::info!("last_seen_l2_token_block {last_seen_l2_token_block:?}");
        vlog::info!("from_block {from_block:?}");
        if last_seen_l2_token_block.as_number() <= from_block.as_number() {
            let init_tokens = Self::query_past_token_init_events(
                last_seen_l2_token_block,
                from_block,
                l2_erc20_bridge_addr,
                &mut sender,
                &middleware,
            )
            .await?;
            vlog::info!("Init tokens {init_tokens:?}");
            addresses.extend(&init_tokens);
        }
        vlog::info!("{addresses:?}");
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
            .address(addresses.iter().cloned().collect::<Vec<_>>())
            .topic0(topic0.clone());

        let filter = Filter::new()
            .from_block(latest_block)
            .address(addresses.iter().cloned().collect::<Vec<_>>())
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
                    sender.send(we.into()).await.unwrap();
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
                    sender.send(we.into()).await.unwrap();
                }
                continue;
            }
            match Self::bridge_initialize_event(
                &log,
                &raw_log,
                &addresses,
                &mut sender,
                &middleware,
            )
            .await
            {
                Ok(Some(address)) => {
                    if !addresses.contains(&address) {
                        vlog::info!("Restarting on the token added event {address}",);

                        return Ok(RunResult::NewTokenInitialized {
                            token: address,
                            block: log
                                .block_number
                                .expect("a mined block always has a block number; qed")
                                .into(),
                        });
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
