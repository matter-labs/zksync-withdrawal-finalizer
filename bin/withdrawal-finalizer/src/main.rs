#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! A withdraw-finalizer

use std::str::FromStr;
use std::sync::Arc;

use clap::Parser;
use color_eyre::eyre::Result;
use envconfig::Envconfig;
use ethers::{
    providers::{Middleware, Provider, Ws},
    types::{BlockNumber, Chain},
};
use log::LevelFilter;
use sqlx::ConnectOptions;

use cli::Args;
use client::{
    l2bridge::L2Bridge, l2standard_token::WithdrawalEventsStream, zksync_contract::BlockEvents,
};
use config::Config;

mod accumulator;
mod cli;
mod config;
mod error;
mod withdrawal_finalizer;

const CHANNEL_CAPACITY: usize = 1024;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    env_logger::Builder::new()
        .filter_level(LevelFilter::Info)
        .init();

    let args = Args::parse();

    log::info!("starting withdrawal finalizer");

    let config = match args.config_path {
        Some(path) => Config::from_file(path)?,
        None => Config::init_from_env()?,
    };

    let provider_l1 = Provider::<Ws>::connect(config.l1_ws_url.as_ref())
        .await
        .unwrap();
    let client_l1 = Arc::new(provider_l1);

    let provider_l2 = Provider::<Ws>::connect(config.zk_server_ws_url.as_str())
        .await
        .unwrap();
    let client_l2 = Arc::new(provider_l2);

    let l2_bridge = L2Bridge::new(config.l2_erc20_bridge_addr, client_l2.clone());

    let event_mux = BlockEvents::new(client_l1.clone()).await?;
    let (blocks_rx, blocks_tx) = tokio::sync::mpsc::channel(CHANNEL_CAPACITY);
    let we_mux = WithdrawalEventsStream::new(client_l2.clone()).await?;

    let blocks_rx = tokio_util::sync::PollSender::new(blocks_rx);
    let blocks_tx = tokio_stream::wrappers::ReceiverStream::new(blocks_tx);

    let from_l2_block = match config.start_from_l2_block {
        Some(l2_block) => l2_block,
        None => client_l2
            .get_block(BlockNumber::Latest)
            .await?
            .expect("There is also a finalized block; qed")
            .number
            .expect("A finalized block number is always known; qed")
            .as_u64(),
    };

    let (we_rx, we_tx) = tokio::sync::mpsc::channel(CHANNEL_CAPACITY);

    let we_rx = tokio_util::sync::PollSender::new(we_rx);
    let we_tx = tokio_stream::wrappers::ReceiverStream::new(we_tx);

    let from_l1_block = match config.start_from_l1_block {
        Some(l1_block) => l1_block,
        None => client_l1
            .get_block(BlockNumber::Safe)
            .await?
            .expect("There is also a finalized block; qed")
            .number
            .expect("A finalized block number is always known; qed")
            .as_u64(),
    };

    tokio::spawn(event_mux.run(config.main_zksync_contract, from_l1_block, blocks_rx));

    let l1_tokens = config.l1_tokens_to_process.as_ref().unwrap().0.clone();

    log::info!("l1_tokens {l1_tokens:#?}");

    let mut tokens = vec![];

    for l1_token in &l1_tokens {
        let l2_token = l2_bridge.l2_token_address(*l1_token).await?;

        log::info!("l1 token address {l1_token} on l2 is {l2_token}");
        tokens.push(l2_token);
    }

    tokio::spawn(we_mux.run(tokens, from_l2_block, we_rx));

    let mut pgpool_opts = sqlx::postgres::PgConnectOptions::from_str(config.database_url.as_str())?;
    let pgpool = pgpool_opts
        .log_statements(LevelFilter::Debug)
        .connect()
        .await?;

    let wf = withdrawal_finalizer::WithdrawalFinalizer::new(
        client_l1,
        client_l2,
        config.l1_eth_bridge_addr,
        config.withdrawal_finalizer_contract,
        config.main_zksync_contract,
        config.one_withdrawal_gas_limit,
        config.batch_finalization_gas_limit,
        pgpool,
    );

    let last_batch = client::etherscan::last_processed_l1_batch(
        Chain::Goerli,
        config.withdrawal_finalizer_contract,
        config.withdrawal_finalizer_eth_address,
        config.main_zksync_contract,
        config.l1_erc20_bridge_addr,
        config.etherscan_token.as_ref().unwrap().clone(),
    )
    .await;

    if let Ok(last_batch) = last_batch {
        log::info!("last_batch: {last_batch:?}");
    }
    wf.run(blocks_tx, we_tx, from_l2_block).await?;

    Ok(())
}
