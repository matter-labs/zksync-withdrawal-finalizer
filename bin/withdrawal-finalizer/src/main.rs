#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! A withdraw-finalizer

use std::{str::FromStr, sync::Arc};

use clap::Parser;
use envconfig::Envconfig;
use ethers::providers::{JsonRpcClient, Middleware, Provider, Ws};
use eyre::{anyhow, Result};
use sqlx::{postgres::PgConnectOptions, ConnectOptions, PgConnection, PgPool};

use chain_events::{BlockEvents, WithdrawalEvents};
use cli::Args;
use client::{
    l1bridge::codegen::IL1Bridge, l2bridge::codegen::IL2Bridge, zksync_contract::codegen::IZkSync,
    ZksyncMiddleware,
};
use config::Config;
use metrics_exporter_prometheus::PrometheusBuilder;
use tokio::task::JoinHandle;

mod cli;
mod config;
mod withdrawal_finalizer;
mod withdrawal_status_updater;

const CHANNEL_CAPACITY: usize = 1024;

fn run_prometheus_exporter() -> Result<JoinHandle<()>> {
    let builder = {
        let addr = ([0, 0, 0, 0], 3312);
        PrometheusBuilder::new().with_http_listener(addr)
    };

    let (recorder, exporter) = builder.build()?;

    metrics::set_boxed_recorder(Box::new(recorder)).expect("failed to set the metrics recorder");

    Ok(tokio::spawn(async move {
        tokio::pin!(exporter);
        loop {
            tokio::select! {
                _ = &mut exporter => {}
            }
        }
    }))
}

async fn start_from_l1_block<M1, M2>(
    client_l1: Arc<M1>,
    client_l2: Arc<M2>,
    l2_block_number: u32,
    conn: &mut PgConnection,
) -> Result<u64>
where
    M1: Middleware,
    <M1 as Middleware>::Provider: JsonRpcClient,
    M2: Middleware,
    <M2 as Middleware>::Provider: JsonRpcClient,
{
    let block_details = client_l2
        .provider()
        .get_block_details(l2_block_number)
        .await?
        .expect("Always start from the block that there is info about; qed");

    match block_details.commit_tx_hash {
        Some(commit_tx_hash) => {
            let commit_tx = client_l1
                .get_transaction(commit_tx_hash)
                .await
                .map_err(|e| anyhow!("{e}"))?
                .expect("The corresponding L1 tx exists; qed");
            let commit_tx_block_number = commit_tx
                .block_number
                .expect("Already mined TX always has a block number; qed")
                .as_u64();

            let last_seen_l1_block = storage::last_l1_block_seen(conn)
                .await?
                .map(|b| b.saturating_sub(1));

            // If some blocks from l1 have already been seen the minumum value
            // of the last seen block and the l1 block that corresponds to `l2_block_number`
            // have to be taken since syncing l1 and l2 events is not synchronous
            // and simply relying on `commit_tx_block_number` may lead to gaps in
            // the l1 block history.
            match last_seen_l1_block {
                Some(l1_block) => Ok(std::cmp::min(l1_block, commit_tx_block_number)),
                None => Ok(commit_tx_block_number),
            }
        }
        None => Ok(storage::last_l1_block_seen(conn).await?.unwrap()),
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

    let args = Args::parse();

    let config = match args.config_path {
        Some(path) => Config::from_file(path)?,
        None => {
            dotenvy::dotenv().ok();
            Config::init_from_env()?
        }
    };

    let sentry_guard = vlog::init();

    if sentry_guard.is_some() {
        vlog::info!(
            "Starting Sentry url: {}, l1_network: {}, l2_network {}",
            std::env::var("MISC_SENTRY_URL").unwrap(),
            std::env::var("CHAIN_ETH_NETWORK").unwrap(),
            std::env::var("CHAIN_ETH_ZKSYNC_NETWORK").unwrap(),
        );
    } else {
        vlog::info!("No sentry url configured");
    }

    let prometheus_exporter_handle = run_prometheus_exporter()?;

    // reconnections do not reset the reconnection count trackers in the
    // `ethers-rs`. In the logic of reconnections have to happen as long
    // as the application exists; below code configures that number to
    // be `usize::MAX` as such.
    let provider_l1 =
        Provider::<Ws>::connect_with_reconnects(config.eth_client_ws_url.as_ref(), usize::MAX)
            .await
            .unwrap();
    let client_l1 = Arc::new(provider_l1);

    let provider_l2 = Provider::<Ws>::connect_with_reconnects(
        config.api_web3_json_rpc_ws_url.as_str(),
        usize::MAX,
    )
    .await
    .unwrap();

    let client_l2 = Arc::new(provider_l2);

    let l2_bridge = IL2Bridge::new(config.l2_erc20_bridge_addr, client_l2.clone());

    let event_mux = BlockEvents::new(config.eth_client_ws_url.as_ref());
    let (blocks_tx, blocks_rx) = tokio::sync::mpsc::channel(CHANNEL_CAPACITY);
    let we_mux = WithdrawalEvents::new(config.api_web3_json_rpc_ws_url.as_str());

    let blocks_tx = tokio_util::sync::PollSender::new(blocks_tx);
    let blocks_rx = tokio_stream::wrappers::ReceiverStream::new(blocks_rx);

    let mut options = PgConnectOptions::from_str(config.database_url.as_str())?;
    options.disable_statement_logging();

    let pgpool = PgPool::connect_with(options).await?;

    let from_l2_block = start_from_l2_block(
        client_l2.clone(),
        &mut pgpool.acquire().await?.detach(),
        &config,
    )
    .await?;

    vlog::info!("Starting from L2 block number {from_l2_block}");

    let (we_tx, we_rx) = tokio::sync::mpsc::channel(CHANNEL_CAPACITY);

    let we_tx = tokio_util::sync::PollSender::new(we_tx);
    let we_rx = tokio_stream::wrappers::ReceiverStream::new(we_rx);

    let from_l1_block = start_from_l1_block(
        client_l1.clone(),
        client_l2.clone(),
        from_l2_block as u32,
        &mut pgpool.acquire().await?.detach(),
    )
    .await?;

    vlog::info!("Starting from L1 block number {from_l1_block}");

    let l1_tokens = client_l2.get_confirmed_tokens(0, u8::MAX).await?;

    let mut tokens = vec![];

    for l1_token in &l1_tokens {
        let l2_token = l2_bridge.l_2_token_address(l1_token.l1_address).await?;

        let l1_token_address = l1_token.l1_address;
        vlog::info!("l1 token address {l1_token_address} on l2 is {l2_token}");
        tokens.push(l2_token);
    }
    let l1_bridge = IL1Bridge::new(config.l1_erc20_bridge_proxy_addr, client_l1.clone());

    let zksync_contract = IZkSync::new(config.diamond_proxy_addr, client_l1.clone());

    let updater_handle = tokio::spawn(withdrawal_status_updater::run(
        pgpool.clone(),
        zksync_contract.clone(),
        l1_bridge.clone(),
        client_l2.clone(),
        config.updater_backoff,
    ));

    let wf = withdrawal_finalizer::WithdrawalFinalizer::new(
        client_l2,
        pgpool,
        zksync_contract,
        l1_bridge,
    );

    let withdrawal_events_handle =
        tokio::spawn(we_mux.run_with_reconnects(tokens, from_l2_block, we_tx));

    let finalizer_handle = tokio::spawn(wf.run(blocks_rx, we_rx, from_l2_block));

    let block_events_handle = tokio::spawn(event_mux.run_with_reconnects(
        config.diamond_proxy_addr,
        from_l1_block,
        blocks_tx,
    ));

    tokio::select! {
        r = block_events_handle => {
            vlog::error!("Block Events stream ended with {r:?}");
        }
        r = withdrawal_events_handle => {
            vlog::error!("Withdrawals Events stream ended with {r:?}");
        }
        r = finalizer_handle => {
            vlog::error!("Finalizer main loop ended with {r:?}");
        }
        r = updater_handle => {
            vlog::error!("Withdrawals updater ended with {r:?}");
        }
        r = prometheus_exporter_handle => {
            vlog::error!("Prometheus exporter ended with {r:?}");
        }
    }

    Ok(())
}
