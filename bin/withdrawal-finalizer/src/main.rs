#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! A withdraw-finalizer

use std::sync::Arc;

use clap::Parser;
use color_eyre::eyre::Result;
use ethers::providers::{Http, Provider};
use ethers::types::Address;

use cli::Args;
use client::l1bridge::L1Bridge;
use config::Config;

mod cli;
mod config;
mod error;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Args::parse();

    let config = match args.config_path {
        Some(path) => Config::from_file(path)?,
        None => Config::from_env(true)?,
    };

    let provider = Provider::<Http>::try_from("http://localhost:8545").unwrap();
    let client = Arc::new(provider);

    let address: Address = "0x5fba9bE50d447BF9d77874862e76e7b2fc12ecf9"
        .parse()
        .unwrap();
    let contract = L1Bridge::new(address, client);

    let bridge_addr = contract.l2bridge().await?;

    for token in config.l1_tokens_to_process() {
        let res = contract.l2_token_address(*token).await?;

        println!("Address of token {token} is {res}");
    }

    println!("l2brdige addr is {bridge_addr}");

    Ok(())
}
