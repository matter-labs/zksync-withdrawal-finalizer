#![allow(unused)]

use std::{collections::HashMap, marker::PhantomData, str::FromStr, sync::Arc, time::Duration};

use ethers::{
    providers::{JsonRpcClient, Middleware},
    types::{Address, BlockNumber, H256, U256, U64},
};
use futures::{stream::StreamExt, Stream};
use tokio::{pin, select, signal, time::timeout};

use client::{
    finalize_withdrawal_params, get_l1_batch_block_range,
    l1bridge::L1Bridge,
    withdrawal_finalizer::{
        RequestFinalizeWithdrawal, WithdrawalFinalizer as WithdrawalFinalizerContract,
    },
    BlockEvent, WithdrawalEvent,
};

use crate::{accumulator::WithdrawalsAccumulator, Result};

const L2_ETH_TOKEN_ADDRESS: &str = "0x000000000000000000000000000000000000800a";

fn is_eth(address: &Address) -> bool {
    address == &Address::zero() || address == &Address::from_str(L2_ETH_TOKEN_ADDRESS).unwrap()
}

pub async fn main_loop<BE, WE>(block_events: BE, withdrawal_events: WE) -> Result<()>
where
    BE: Stream<Item = BlockEvent>,
    WE: Stream<Item = WithdrawalEvent>,
{
    pin!(block_events);
    pin!(withdrawal_events);

    loop {
        select! {
            block_event = block_events.next() => {
                println!("block event");
            }
            withdrawal_event = withdrawal_events.next() => {
                println!("withdrawal_event");
            }
            _ = signal::ctrl_c() => break,
        }
    }

    Ok(())
}

pub struct WithdrawalFinalizer<M> {
    l1_provider: Arc<M>,

    l2_provider: Arc<M>,

    l1_bridge: client::l1bridge::L1Bridge<M>,

    withdrawal_finalizer: WithdrawalFinalizerContract<M>,

    processing_block_offset: U64,

    max_block_range: usize,

    tx_fee_limit: U256,

    batch_finalization_gas_limit: U256,

    one_withdrawal_gas_limit: U256,

    accumulator: WithdrawalsAccumulator,
}

impl<M: Middleware + JsonRpcClient> WithdrawalFinalizer<M> {
    pub async fn process(&mut self) -> Result<()> {
        let provider = self.l2_provider.clone();

        let last_finalized_block = provider
            .get_block(BlockNumber::Finalized)
            .await
            .unwrap()
            .expect("the is always a last finalized block; qed")
            .number
            .expect("the last finalized block always has a number; qed");

        let last_processed_l1_batch = self.fetch_last_processed_l1_batch().await;

        let last_processed_l1_batch_with_offset =
            last_processed_l1_batch.saturating_sub(self.processing_block_offset);

        let last_processed_block =
            get_l1_batch_block_range(&provider, last_processed_l1_batch_with_offset)
                .await
                .unwrap()
                .unwrap()
                .0;

        let mut withdrawal_events: Vec<WithdrawalEvent> = vec![];

        let mut number_of_logs_per_tx: HashMap<H256, usize> = HashMap::new();

        for _starting_block in (last_processed_block.as_u64()..last_finalized_block.as_u64())
            .step_by(self.max_block_range)
        {
            // TODO: fetch withdrawal events
        }

        let gas_price = provider.get_gas_price().await.unwrap();
        let tx_fee_limit = self.tx_fee_limit;

        let accounts = provider.get_accounts().await.unwrap();
        let nonce = provider
            .get_transaction_count(accounts[0], None)
            .await
            .unwrap();

        for event in &withdrawal_events {
            *number_of_logs_per_tx.entry(event.tx_hash).or_default() += 1;

            let params = finalize_withdrawal_params(
                &provider,
                event.tx_hash,
                *number_of_logs_per_tx
                    .get(&event.tx_hash)
                    .expect("the entry has been inserted on the previous step; qed"),
            )
            .await
            .unwrap();

            if self
                .l1_bridge
                .is_withdrawal_finalized(event.block_number.into(), 0.into())
                .await
                .unwrap()
            {
                continue;
            }

            self.accumulator.add_withdrawal(
                client::withdrawal_finalizer::RequestFinalizeWithdrawal {
                    l_2_block_number: params.l1_batch_number.as_u64().into(),
                    l_2_message_index: params.l2_message_index.into(),
                    l_2_tx_number_in_block: params.l2_tx_number_in_block,
                    message: params.message,
                    merkle_proof: params.proof,
                    is_eth: is_eth(&params.sender),
                    gas: self.one_withdrawal_gas_limit,
                },
            );

            if self.accumulator.ready_to_finalize() {
                self.finalize().await.unwrap();
            }
        }

        Ok(())
    }

    async fn fetch_last_processed_l1_batch(&self) -> U64 {
        0.into()
    }

    async fn finalize(&mut self) -> Result<FinalizeResult> {
        let withdrawals = self.accumulator.take_withdrawals();

        let withdrawals_predictions = self
            .withdrawal_finalizer
            .finalize_withdrawals(withdrawals.clone())
            .await
            .unwrap();

        let mut predicted_ok = vec![];
        let mut predicted_to_fail = vec![];

        for (w, p) in withdrawals
            .into_iter()
            .zip(withdrawals_predictions.into_iter())
        {
            if p.success && p.gas <= self.one_withdrawal_gas_limit {
                predicted_ok.push(w);
            } else {
                predicted_to_fail.push(w);
            }
        }

        let mut res = FinalizeResult {
            failed: predicted_to_fail,
        };

        match timeout(
            Duration::from_secs(10),
            self.withdrawal_finalizer
                .send_finalize_withdrawals(predicted_ok.clone()),
        )
        .await
        {
            Ok(res) => {
                let res = res?;
                if let Some(receipt) = res {
                    println!(
                        "Eth tx {} with {} withdrawals has been mined",
                        receipt.transaction_hash,
                        predicted_ok.len()
                    );
                }
            }
            Err(_) => {
                // TX was not mined after a timeout so the withdrawals
                // in it also considered failed.
                res.failed.extend_from_slice(predicted_ok.as_slice());
            }
        }

        Ok(res)
    }
}

struct FinalizeResult {
    failed: Vec<RequestFinalizeWithdrawal>,
}
