#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! A withdraw-finalizer

use std::{sync::Arc, time::Duration};

use clap::Parser;
use color_eyre::eyre::Result;
use envconfig::Envconfig;
use ethers::{
    providers::{Provider, StreamExt, Ws},
    types::Chain,
};
use log::LevelFilter;

use cli::Args;
use client::{
    l2bridge::L2Bridge, l2standard_token::WithdrawalEvents, zksync_contract::BlockEvents,
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

    let provider_l1 = Provider::<Ws>::connect(config.l1_ws_url.as_ref().unwrap())
        .await
        .unwrap();
    let client_l1 = Arc::new(provider_l1);

    let provider_l2 = Provider::<Ws>::connect(config.zk_server_ws_url.as_str())
        .await
        .unwrap();
    let client_l2 = Arc::new(provider_l2);

    let l2_bridge = L2Bridge::new(config.l2_erc20_bridge_addr, client_l2.clone());

    let event_mux = BlockEvents::new(client_l1).await?;
    let (blocks_rx, blocks_tx) = tokio::sync::mpsc::channel(CHANNEL_CAPACITY);
    let we_mux = WithdrawalEvents::new(client_l2).await?;

    let blocks_rx = tokio_util::sync::PollSender::new(blocks_rx);
    let mut blocks_tx = tokio_stream::wrappers::ReceiverStream::new(blocks_tx);

    let (we_rx, we_tx) = tokio::sync::mpsc::channel(CHANNEL_CAPACITY);

    let we_rx = tokio_util::sync::PollSender::new(we_rx);
    let mut we_tx = tokio_stream::wrappers::ReceiverStream::new(we_tx);

    tokio::spawn(event_mux.run(config.main_contract, 9015215, blocks_rx));

    let l1_tokens = config.l1_tokens_to_process.as_ref().unwrap().0.clone();

    log::info!("l1_tokens {l1_tokens:#?}");

    let mut tokens = vec![];

    for l1_token in &l1_tokens {
        let l2_token = l2_bridge.l2_token_address(*l1_token).await?;

        log::info!("l1 token address {l1_token} on l2 is {l2_token}");
        tokens.push(l2_token);
    }

    tokio::spawn(we_mux.run(tokens, 5723175, we_rx));

    let mut query_last_batch = tokio::time::interval(Duration::from_secs(10));

    loop {
        tokio::select! {
            event = blocks_tx.next() => {
                if let Some(event) = event {
                    log::info!("event {event}");
                }
            }
            event = we_tx.next() => {
                if let Some(event) = event {
                    log::info!("withdrawal event {event:?}");
                }
            }
            _ = query_last_batch.tick() => {
                let last_batch = client::etherscan::last_processed_l1_batch(
                    Chain::Goerli,
                    config.withdrawal_finalizer_contract,
                    config.withdrawal_finalizer_eth_address,
                    config.main_contract,
                    config.l1_erc20_bridge_addr,
                    config.etherscan_token.as_ref().unwrap().clone(),
                ).await;

                if let Ok(last_batch) = last_batch {
                    log::info!("last_batch: {last_batch:?}");
                }
            }
            _ = tokio::signal::ctrl_c() => {
                break
            }
        }
    }

    Ok(())
}
