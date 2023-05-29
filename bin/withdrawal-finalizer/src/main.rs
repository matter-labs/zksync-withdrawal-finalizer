#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! A withdraw-finalizer

use std::str::FromStr;
use std::sync::Arc;

use clap::Parser;
use envconfig::Envconfig;
use ethers::{
    providers::{JsonRpcClient, Middleware, Provider, Ws},
    types::BlockNumber,
};
use eyre::{anyhow, Result};
use sqlx::{ConnectOptions, PgConnection};

use cli::Args;
use client::{
    get_block_details, get_confirmed_tokens, l2bridge::L2Bridge,
    l2standard_token::WithdrawalEventsStream, zksync_contract::BlockEvents,
};
use config::Config;

mod cli;
mod config;
mod withdrawal_finalizer;

const CHANNEL_CAPACITY: usize = 1024;

async fn start_from_l1_block<M1, M2>(
    client_l1: Arc<M1>,
    client_l2: Arc<M2>,
    l2_block_number: u32,
) -> Result<u64>
where
    M1: Middleware,
    <M1 as Middleware>::Provider: JsonRpcClient,
    M2: Middleware,
    <M2 as Middleware>::Provider: JsonRpcClient,
{
    let block_details = get_block_details(client_l2.provider().as_ref(), l2_block_number)
        .await?
        .expect("Always start from the block that there is info about; qed");

    let commit_tx_hash = block_details
        .commit_tx_hash
        .expect("Expected to start from already committed block; qed");

    let commit_tx = client_l1
        .get_transaction(commit_tx_hash)
        .await
        .map_err(|e| anyhow!("{e}"))?
        .expect("The corresponding L1 tx exists; qed");

    Ok(commit_tx
        .block_number
        .expect("Already mined TX always has a block number; qed")
        .as_u64())
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
            if let Some(block_number) = storage::last_block_processed(conn).await? {
                // May have stored not the last withdrawal event in `block_number`
                // so to be sure, re-start from the previous block.
                block_number - 1
            } else {
                client
                    .get_block(BlockNumber::Finalized)
                    .await
                    .map_err(|err| anyhow!("{err}"))?
                    .expect("There is also a finalized block; qed")
                    .number
                    .expect("A finalized block number is always known; qed")
                    .as_u64()
            }
        }
    };

    Ok(res)
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    tracing_subscriber::fmt::init();

    let args = Args::parse();

    log::info!("starting withdrawal finalizer");

    let config = match args.config_path {
        Some(path) => Config::from_file(path)?,
        None => {
            dotenvy::dotenv()?;
            Config::init_from_env()?
        }
    };

    let provider_l1 = Provider::<Ws>::connect_with_reconnects(config.l1_ws_url.as_ref(), 0)
        .await
        .unwrap();
    let client_l1 = Arc::new(provider_l1);

    let provider_l2 = Provider::<Ws>::connect_with_reconnects(config.zk_server_ws_url.as_str(), 0)
        .await
        .unwrap();
    let client_l2 = Arc::new(provider_l2);

    let l2_bridge = L2Bridge::new(config.l2_erc20_bridge_addr, client_l2.clone());

    let event_mux = BlockEvents::new(client_l1.clone()).await?;
    let (blocks_rx, blocks_tx) = tokio::sync::mpsc::channel(CHANNEL_CAPACITY);
    let we_mux = WithdrawalEventsStream::new(client_l2.clone()).await?;

    let blocks_rx = tokio_util::sync::PollSender::new(blocks_rx);
    let blocks_tx = tokio_stream::wrappers::ReceiverStream::new(blocks_tx);

    let pgpool_opts = sqlx::postgres::PgConnectOptions::from_str(config.database_url.as_str())?;
    let mut pgpool = pgpool_opts.connect().await?;

    let from_l2_block = start_from_l2_block(client_l2.clone(), &mut pgpool, &config).await?;

    log::info!("Starting from L2 block number {from_l2_block}");

    let (we_rx, we_tx) = tokio::sync::mpsc::channel(CHANNEL_CAPACITY);

    let we_rx = tokio_util::sync::PollSender::new(we_rx);
    let we_tx = tokio_stream::wrappers::ReceiverStream::new(we_tx);

    let from_l1_block =
        start_from_l1_block(client_l1.clone(), client_l2.clone(), from_l2_block as u32).await?;

    log::info!("Starting from L1 block number {from_l1_block}");

    let l1_tokens = get_confirmed_tokens(client_l2.provider().as_ref(), 0, u8::MAX).await?;

    let mut tokens = vec![];

    for l1_token in &l1_tokens {
        let l2_token = l2_bridge.l2_token_address(l1_token.l1_address).await?;

        let l1_token_address = l1_token.l1_address;
        log::info!("l1 token address {l1_token_address} on l2 is {l2_token}");
        tokens.push(l2_token);
    }

    let wf = withdrawal_finalizer::WithdrawalFinalizer::new(client_l2, pgpool);

    let withdrawal_events_handle = tokio::spawn(we_mux.run(tokens, from_l2_block, we_rx));

    let finalizer_handle = tokio::spawn(wf.run(blocks_tx, we_tx, from_l2_block));

    let block_events_handle =
        tokio::spawn(event_mux.run(config.main_zksync_contract, from_l1_block, blocks_rx));

    tokio::select! {
        r = block_events_handle => {
            log::error!("Block Events stream ended with {r:?}");
        }
        r = withdrawal_events_handle => {
            log::error!("Withdrawals Events stream ended with {r:?}");
        }
        r = finalizer_handle => {
            log::error!("Finalizer main loop ended with {r:?}");
        }
    }

    Ok(())
}
