use std::sync::Arc;

use ethers::{
    abi::{AbiDecode, Address},
    providers::{Middleware, PubsubClient},
};
use eyre::{anyhow, Result};
use futures::StreamExt;

use client::zksync_contract::{codegen::CommitBlocksCall, parse_withdrawal_events_l1};

pub struct L2ToL1Events<M: Middleware> {
    middleware: Arc<M>,
}

impl<M: Middleware> L2ToL1Events<M> {
    pub fn new(middleware: Arc<M>) -> Self {
        Self { middleware }
    }
}

impl<M> L2ToL1Events<M>
where
    M: Middleware,
    <M as Middleware>::Provider: PubsubClient,
{
    // TODO: The `subscribe_blocks` is probably not the best subscription strategy,
    // consider polling blocks in from some point in the past until [`BlockNumber::Safe`].
    pub async fn run(self, timelock_address: Address, l2_erc20_bridge_addr: Address) -> Result<()>
where {
        let mut blocks = self
            .middleware
            .subscribe_blocks()
            .await
            .map_err(|e| anyhow!("{e}"))?;

        while let Some(block) = blocks.next().await {
            let block_number = block
                .number
                .expect("mined blocks always have a number; qed");

            // re-request a block by block number, subscriptions always return blocks with
            // no transactions.
            let block = self
                .middleware
                .get_block(block_number)
                .await
                .map_err(|e| anyhow!("{e}"))?
                .expect("subscription always sends an existing block; qed");

            for tx_hash in &block.transactions {
                let tx = self
                    .middleware
                    .get_transaction(*tx_hash)
                    .await
                    .map_err(|e| anyhow!("{e}"))?
                    .expect("transaction in a mined block always exists; qed");

                if tx.to != Some(timelock_address) {
                    continue;
                }

                if let Ok(commit_blocks) = CommitBlocksCall::decode(tx.input) {
                    let withdrawals = parse_withdrawal_events_l1(
                        &commit_blocks,
                        block_number.as_u64(),
                        l2_erc20_bridge_addr,
                    );

                    // TODO: write to storage happens here.
                    log::info!("withdrawals {withdrawals:#?}");
                }
            }
        }
        Ok(())
    }
}
