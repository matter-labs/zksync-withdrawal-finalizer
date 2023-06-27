use std::time::{Duration, Instant};

use ethers::{
    abi::{AbiDecode, Address},
    etherscan::Client,
    prelude::account::Sort,
    providers::Middleware,
    types::BlockNumber,
};
use futures::{Sink, SinkExt};

use client::zksync_contract::{codegen::CommitBlocksCall, parse_withdrawal_events_l1, L2ToL1Event};

use crate::{Error, Result};

// etherscan allous up to 10000 results only.
const HISTORY_STEP: u64 = 1024 * 4;

// paginate by pages of this size
const OFFSET_SIZE: u64 = 1024;

// query backoff
const QUERY_BACKOFF: Duration = Duration::from_secs(15);

/// Query L2ToL1Events from etherscan.
pub struct L2ToL1Events {
    client: Client,
    timelock_address: Address,
    l2_erc20_bridge_addr: Address,
    operator_address: Address,
}

impl L2ToL1Events {
    /// Creates a new [`L2ToL1Events`].
    pub fn new(
        client: Client,
        timelock_address: Address,
        l2_erc20_bridge_addr: Address,
        operator_address: Address,
    ) -> Self {
        Self {
            client,
            timelock_address,
            l2_erc20_bridge_addr,
            operator_address,
        }
    }
}

impl L2ToL1Events {
    async fn query_block_range<S>(
        &self,
        start_block: u64,
        end_block: u64,
        sender: &mut S,
    ) -> Result<()>
    where
        S: Sink<Vec<L2ToL1Event>> + Unpin + Clone,
        <S as Sink<Vec<L2ToL1Event>>>::Error: std::fmt::Debug,
    {
        let mut page = 1;
        loop {
            let started_at = Instant::now();

            let transactions = self
                .client
                .get_transactions(
                    &self.operator_address,
                    Some(ethers::prelude::account::TxListParams {
                        start_block,
                        end_block,
                        page,
                        offset: OFFSET_SIZE,
                        sort: Sort::Asc,
                    }),
                )
                .await
                .unwrap();

            metrics::histogram!(
                "watcher.chain_events.l2_to_l1_events.etherscan_get_transactions",
                started_at.elapsed(),
            );

            if transactions.is_empty() {
                break;
            }

            page += 1;

            let mut withdrawals = vec![];
            for tx in &transactions {
                if let Ok(commit_blocks) = CommitBlocksCall::decode(&tx.input) {
                    if tx.to != Some(self.timelock_address) {
                        continue;
                    }
                    let mut res = parse_withdrawal_events_l1(
                        &commit_blocks,
                        tx.block_number
                            .as_number()
                            .expect("only mined transactions are queried; qed")
                            .as_u64(),
                        self.l2_erc20_bridge_addr,
                    );
                    withdrawals.append(&mut res);
                }
            }
            metrics::counter!(
                "watcher.chain_events.l2_to_l1_events.withdrawal_events",
                withdrawals.len() as u64
            );
            sender.send(withdrawals).await.unwrap();
        }
        Ok(())
    }

    /// Run querying Etherscan for l2 to l1 events.
    pub async fn run<B, S, M>(self, client_l1: M, from_block: B, mut sender: S) -> Result<()>
    where
        B: Into<BlockNumber> + Copy,
        M: Middleware,
        S: Sink<Vec<L2ToL1Event>> + Unpin + Clone,
        <S as Sink<Vec<L2ToL1Event>>>::Error: std::fmt::Debug,
    {
        let mut from_block = from_block
            .into()
            .as_number()
            .expect("Should start from a block with a number; qed");

        loop {
            let latest_block = client_l1
                .get_block_number()
                .await
                .map_err(|e| Error::Middleware(format!("{e}")))?;

            let to_block = std::cmp::min(latest_block, from_block + HISTORY_STEP);

            vlog::info!("l2 to l1 events block range {from_block} {to_block}");

            if from_block == latest_block {
                tokio::time::sleep(QUERY_BACKOFF).await;
                continue;
            }

            self.query_block_range(from_block.as_u64(), to_block.as_u64(), &mut sender)
                .await?;
            from_block = to_block;
        }
    }
}
