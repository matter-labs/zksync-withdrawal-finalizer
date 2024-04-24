#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! A withdraw-finalizer

use std::{str::FromStr, sync::Arc, time::Duration};

use envconfig::Envconfig;
use ethers::{
    prelude::SignerMiddleware,
    providers::{Http, JsonRpcClient, Middleware, Provider},
    signers::LocalWallet,
    types::U256,
};
use eyre::{anyhow, Result};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    ConnectOptions, PgConnection,
};

use chain_events::{BlockEvents, L2EventsListener};
use client::{l1bridge::codegen::IL1Bridge, zksync_contract::codegen::IZkSync, ZksyncMiddleware};
use config::Config;
use tokio::sync::watch;
use vise_exporter::MetricsExporter;
use watcher::Watcher;

use crate::metrics::MAIN_FINALIZER_METRICS;

mod config;
mod metrics;

const CHANNEL_CAPACITY: usize = 1024 * 16;

fn run_vise_exporter() -> Result<watch::Sender<()>> {
    let (shutdown_sender, mut shutdown_receiver) = watch::channel(());
    let exporter = MetricsExporter::default().with_graceful_shutdown(async move {
        shutdown_receiver.changed().await.ok();
    });
    let bind_address = "0.0.0.0:3312".parse().unwrap();
    tokio::spawn(exporter.start(bind_address));

    Ok(shutdown_sender)
}

async fn start_from_l1_block<M1, M2>(
    client_l1: Arc<M1>,
    client_l2: Arc<M2>,
    conn: &mut PgConnection,
) -> Result<u64>
where
    M1: Middleware,
    <M1 as Middleware>::Provider: JsonRpcClient,
    M2: Middleware,
    <M2 as Middleware>::Provider: JsonRpcClient,
{
    match storage::last_l1_block_seen(conn).await? {
        Some(b2) => Ok(b2),
        None => {
            tracing::info!(concat!(
                "information about last block seen is missing, ",
                "starting from L1 block corresponding to L2 block 1"
            ));

            let block_details = client_l2
                .provider()
                .get_block_details(1)
                .await?
                .expect("Always start from the block that there is info about; qed");

            let commit_tx_hash = block_details
                .commit_tx_hash
                .expect("A first block on L2 is always committed; qed");

            let commit_tx = client_l1
                .get_transaction(commit_tx_hash)
                .await
                .map_err(|e| anyhow!("{e}"))?
                .expect("The corresponding L1 tx exists; qed");

            let commit_tx_block_number = commit_tx
                .block_number
                .expect("Already mined TX always has a block number; qed")
                .as_u64();

            Ok(commit_tx_block_number)
        }
    }
}

// Determine an L2 block to start processing withdrawals from.
//
// The priority is:
// 1. Config variable `start_from_l2_block`. If not present:
// 2. The block of last seen withdrawal event decremented by one. If not present:
// 3. Last finalized block on L2.
async fn start_from_l2_block<M: Middleware>(
    client: Arc<M>,
    conn: &mut PgConnection,
    config: &Config,
) -> Result<u64> {
    let res = match config.start_from_l2_block {
        Some(l2_block) => l2_block,
        None => {
            if let Some(block_number) = storage::last_l2_block_seen(conn).await? {
                // May have stored not the last withdrawal event in `block_number`
                // so to be sure, re-start from the previous block.
                block_number - 1
            } else {
                client
                    .get_block(1)
                    .await
                    .map_err(|err| anyhow!("{err}"))?
                    .expect("The genesis block always exists; qed")
                    .number
                    .expect("The genesis block number is always known; qed")
                    .as_u64()
            }
        }
    };

    Ok(res)
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    dotenvy::dotenv().ok();
    let config = Config::init_from_env()?;

    let sentry_guard = vlog::init();

    if sentry_guard.is_some() {
        tracing::info!(
            "Starting Sentry url: {}, l1_network: {}, l2_network {}",
            std::env::var("MISC_SENTRY_URL").unwrap(),
            std::env::var("CHAIN_ETH_NETWORK").unwrap(),
            std::env::var("CHAIN_ETH_ZKSYNC_NETWORK").unwrap(),
        );
    } else {
        tracing::info!("No sentry url configured");
    }

    client::add_predefined_token_addrs(config.token_mappings().as_ref()).await;
    let stop_vise_exporter = run_vise_exporter()?;

    // Successful reconnections do not reset the reconnection count trackers in the
    // `ethers-rs`. In the logic of reconnections have to happen as long
    // as the application exists; below code configures that number to
    // be `usize::MAX` as such.
    let provider_l1 = Provider::<Http>::try_from(config.eth_client_http_url.as_ref()).unwrap();
    let client_l1 = Arc::new(provider_l1);

    let provider_l2 =
        Provider::<Http>::try_from(config.api_web3_json_rpc_http_url.as_str()).unwrap();

    let client_l2 = Arc::new(provider_l2);

    let event_mux = BlockEvents::new(config.eth_client_ws_url.as_ref());
    let (blocks_tx, blocks_rx) = tokio::sync::mpsc::channel(CHANNEL_CAPACITY);

    let blocks_tx_wrapped = tokio_util::sync::PollSender::new(blocks_tx.clone());
    let blocks_rx = tokio_stream::wrappers::ReceiverStream::new(blocks_rx);

    let options =
        PgConnectOptions::from_str(config.database_url.as_str())?.disable_statement_logging();

    let pgpool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(2))
        .connect_with(options)
        .await?;

    let from_l2_block = start_from_l2_block(
        client_l2.clone(),
        &mut pgpool.acquire().await?.detach(),
        &config,
    )
    .await?;

    tracing::info!("Starting from L2 block number {from_l2_block}");

    let (we_tx, we_rx) = tokio::sync::mpsc::channel(CHANNEL_CAPACITY);

    let we_tx_wrapped = tokio_util::sync::PollSender::new(we_tx.clone());
    let we_rx = tokio_stream::wrappers::ReceiverStream::new(we_rx);

    let from_l1_block = start_from_l1_block(
        client_l1.clone(),
        client_l2.clone(),
        &mut pgpool.acquire().await?.detach(),
    )
    .await?;

    tracing::info!("Starting from L1 block number {from_l1_block}");

    let (mut tokens, last_token_seen_at_block) = storage::get_tokens(&pgpool.clone()).await?;

    if let Some(ref custom_tokens) = config.custom_token_addresses {
        tokens.extend_from_slice(custom_tokens.0.as_slice());
    }

    tracing::info!("tokens {tokens:?}");
    if let Some(ref custom_tokens) = config.custom_token_deployer_addresses {
        tokens.extend_from_slice(custom_tokens.0.as_slice());
    }

    let l2_events = L2EventsListener::new(
        config.api_web3_json_rpc_ws_url.as_str(),
        config
            .custom_token_deployer_addresses
            .map(|list| list.0)
            .unwrap_or(vec![config.l2_erc20_bridge_addr]),
        tokens.into_iter().collect(),
        config.finalize_eth_token.unwrap_or(true),
    );

    let l1_bridge = IL1Bridge::new(config.l1_erc20_bridge_proxy_addr, client_l1.clone());

    let zksync_contract = IZkSync::new(config.diamond_proxy_addr, client_l1.clone());

    // by default meter withdrawals
    let meter_withdrawals = config.enable_withdrawal_metering.unwrap_or(true);

    let watcher = Watcher::new(client_l2.clone(), pgpool.clone(), meter_withdrawals);

    let withdrawal_events_handle = tokio::spawn(l2_events.run_with_reconnects(
        from_l2_block,
        last_token_seen_at_block,
        we_tx_wrapped,
    ));

    let watcher_handle = tokio::spawn(watcher.run(blocks_rx, we_rx, from_l2_block));

    let block_events_handle = tokio::spawn(event_mux.run_with_reconnects(
        config.diamond_proxy_addr,
        config.l2_erc20_bridge_addr,
        from_l1_block,
        blocks_tx_wrapped,
    ));

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            MAIN_FINALIZER_METRICS
                .watcher_l1_channel_capacity
                .set(blocks_tx.capacity() as i64);
            MAIN_FINALIZER_METRICS
                .watcher_l2_channel_capacity
                .set(we_tx.capacity() as i64);
        }
    });

    let wallet = config.account_private_key.parse::<LocalWallet>()?;

    let client_l1_with_signer = Arc::new(
        SignerMiddleware::new_with_provider_chain(client_l1, wallet)
            .await
            .unwrap(),
    );

    let finalizer_account_address = client_l1_with_signer.address();

    let contract = client::withdrawal_finalizer::codegen::WithdrawalFinalizer::new(
        config.withdrawal_finalizer_addr,
        client_l1_with_signer,
    );
    let batch_finalization_gas_limit = U256::from_dec_str(&config.batch_finalization_gas_limit)?;
    let one_withdrawal_gas_limit = U256::from_dec_str(&config.one_withdrawal_gas_limit)?;

    tracing::info!(
        "finalization gas limits one: {}, batch: {}",
        config.one_withdrawal_gas_limit,
        config.batch_finalization_gas_limit,
    );

    let eth_finalization_threshold = match config.eth_finalization_threshold {
        Some(eth_finalization_threshold) => {
            Some(ethers::utils::parse_ether(eth_finalization_threshold)?)
        }
        None => None,
    };
    let finalizer = finalizer::Finalizer::new(
        pgpool.clone(),
        one_withdrawal_gas_limit,
        batch_finalization_gas_limit,
        contract,
        zksync_contract,
        l1_bridge,
        config.tx_retry_timeout,
        finalizer_account_address,
        meter_withdrawals,
        eth_finalization_threshold,
        config.only_l1_recipients.map(|v| v.0.into_iter().collect()),
    );
    let finalizer_handle = tokio::spawn(finalizer.run(client_l2));

    let metrics_handle = tokio::spawn(metrics::meter_unfinalized_withdrawals(
        pgpool.clone(),
        eth_finalization_threshold,
    ));

    tokio::select! {
        r = block_events_handle => {
            tracing::error!("Block Events stream ended with {r:?}");
        }
        r = withdrawal_events_handle => {
            tracing::error!("Withdrawals Events stream ended with {r:?}");
        }
        r = watcher_handle => {
            tracing::error!("Finalizer main loop ended with {r:?}");
        }
        r = finalizer_handle => {
            tracing::error!("Finalizer ended with {r:?}");
        }
        _ = metrics_handle => {
            tracing::error!("Metrics loop has ended");
        }
    }

    stop_vise_exporter.send_replace(());

    Ok(())
}
