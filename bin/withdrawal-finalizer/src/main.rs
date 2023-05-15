#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! A withdraw-finalizer

use std::sync::Arc;

use clap::Parser;
use color_eyre::eyre::Result;
use envconfig::Envconfig;
use ethers::providers::{Http, Provider, StreamExt, Ws};
use log::LevelFilter;

use cli::Args;
use client::{l2bridge::L2Bridge, zksync_contract::BlockEvents};
use config::Config;

mod accumulator;
mod cli;
mod config;
mod error;
mod withdrawal_finalizer;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info)
        .init();

    let args = Args::parse();

    log::info!("starting withdrawal finalizer");

    let mut config = match args.config_path {
        Some(path) => Config::from_file(path)?,
        None => Config::init_from_env()?,
    };

    config.get_tokens("localhost")?;

    let provider = Provider::<Http>::try_from(config.zksync_web3_url.as_str())?;
    let client = Arc::new(provider);

    let provider_l1 = Provider::<Ws>::connect("ws://127.0.0.1:8546")
        .await
        .unwrap();
    let client_l1 = Arc::new(provider_l1);

    let _contract = L2Bridge::new(config.l2_erc20_bridge_addr, client.clone());

    let event_mux = BlockEvents::new(client_l1).await?;
    let (blocks_rx, blocks_tx) = tokio::sync::mpsc::channel(1024);

    let blocks_rx = tokio_util::sync::PollSender::new(blocks_rx);
    let mut blocks_tx = tokio_stream::wrappers::ReceiverStream::new(blocks_tx);

    tokio::spawn(event_mux.run(config.main_contract, 0, blocks_rx));

    loop {
        tokio::select! {
            event = blocks_tx.next() => {
                if let Some(event) = event {
                    log::info!("event {event}");
                }
            }
            _ = tokio::signal::ctrl_c() => {
                break
            }
        }
    }

    Ok(())
}
