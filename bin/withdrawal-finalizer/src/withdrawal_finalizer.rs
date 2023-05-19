#![allow(unused)]

use std::{collections::HashMap, marker::PhantomData, str::FromStr, sync::Arc, time::Duration};

use ethers::{
    providers::{JsonRpcClient, Middleware, Provider},
    types::{Address, BlockNumber, H256, U256, U64},
};
use futures::{stream::StreamExt, Stream};
use tokio::{pin, select, signal, time::timeout};

use client::{
    finalize_withdrawal_params, get_l1_batch_block_range, get_log_proof,
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

pub struct WithdrawalFinalizer<M1, M2> {
    l1_provider: Arc<Provider<M1>>,

    l2_provider: Arc<Provider<M2>>,

    l1_bridge: client::l1bridge::L1Bridge<Provider<M1>>,

    l1_main_contract: client::zksync_contract::ZkSync<Provider<M1>>,

    withdrawal_finalizer: WithdrawalFinalizerContract<Provider<M1>>,

    processing_block_offset: usize,

    max_block_range: usize,

    tx_fee_limit: U256,

    batch_finalization_gas_limit: U256,

    one_withdrawal_gas_limit: U256,
}

impl<M1, M2> WithdrawalFinalizer<M1, M2>
where
    M1: JsonRpcClient,
    M2: JsonRpcClient,
{
    pub fn new(
        l1_provider: Arc<Provider<M1>>,
        l2_provider: Arc<Provider<M2>>,
        l1_bridge_address: Address,
        withdrawal_finalizer_address: Address,
        main_contract_address: Address,
        one_withdrawal_gas_limit: U256,
        batch_finalization_gas_limit: U256,
    ) -> Self {
        let l1_bridge = client::l1bridge::L1Bridge::new(l1_bridge_address, l1_provider.clone());

        let l1_main_contract =
            client::zksync_contract::ZkSync::new(main_contract_address, l1_provider.clone());

        let withdrawal_finalizer =
            WithdrawalFinalizerContract::new(withdrawal_finalizer_address, l1_provider.clone());

        let processing_block_offset = 0;

        let tx_fee_limit =
            ethers::utils::parse_ether("0.8").expect("0.8 ether is a parsable amount; qed");

        let max_block_range = 1000usize;

        Self {
            l1_provider,
            l2_provider,
            l1_main_contract,
            l1_bridge,
            withdrawal_finalizer,
            processing_block_offset,
            max_block_range,
            tx_fee_limit,
            batch_finalization_gas_limit,
            one_withdrawal_gas_limit,
        }
    }

    pub async fn run<BE, WE>(
        self,
        block_events: BE,
        withdrawal_events: WE,
        from_l2_block: u64,
    ) -> Result<()>
    where
        BE: Stream<Item = BlockEvent>,
        WE: Stream<Item = WithdrawalEvent>,
    {
        pin!(block_events);
        pin!(withdrawal_events);

        let mut curr_block_number = from_l2_block;

        let mut accumulator = WithdrawalsAccumulator::new(
            0.into(),
            self.tx_fee_limit,
            self.batch_finalization_gas_limit,
            self.one_withdrawal_gas_limit,
        );

        // While reading the stream of withdrawal events asyncronously
        // we may never be sure that we are currenly looking at the last
        // event from the given block.
        let mut in_block_events = vec![];

        loop {
            tokio::select! {
                block_event = block_events.next() => {
                    if let Some(event) = block_event {
                        log::info!("event {event}");
                    }
                }
                withdrawal_event = withdrawal_events.next() => {
                    if let Some(event) = withdrawal_event {
                        log::info!("withdrawal event {event:?}");
                        if event.block_number > curr_block_number {
                            self.process_withdrawals_in_block(&mut in_block_events, &mut accumulator).await;
                            curr_block_number = event.block_number;
                        }
                        in_block_events.push(event);
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    log::info!("terminating finalizer loop on ctrl-c");
                    break;
                }
            }
        }

        Ok(())
    }

    async fn process_withdrawals_in_block(
        &self,
        events: &mut Vec<WithdrawalEvent>,
        accumulator: &mut WithdrawalsAccumulator,
    ) {
        let mut numbers_in_blocks: HashMap<H256, usize> = HashMap::new();

        for event in events.drain(..) {
            let r = numbers_in_blocks.entry(event.tx_hash).or_default();

            let index = *r;

            *r += 1;

            log::info!("withdrawal {event:?} index in transaction is {index}");
        }
    }

    async fn is_withdrawal_finalized(&self, tx_hash: H256, index: usize) -> Result<bool> {
        let log = client::get_withdrawal_log(self.l2_provider.provider().as_ref(), tx_hash, index)
            .await
            .unwrap();

        let logindex = client::get_withdrawal_l2_to_l1_log(
            self.l2_provider.provider().as_ref(),
            tx_hash,
            index,
        )
        .await
        .unwrap();

        let sender: Address = log.0.topics[1].into();

        let proof = get_log_proof(self.l2_provider.provider().as_ref(), tx_hash, index)
            .await
            .unwrap()
            .unwrap();

        if is_eth(&sender) {
            self.l1_main_contract
                .is_eth_withdrawal_finalized(
                    log.0.l1_batch_number.unwrap().as_u64().into(),
                    proof.id.into(),
                )
                .await
                .map_err(Into::into)
        } else {
            self.l1_bridge
                .is_withdrawal_finalized(
                    log.0.l1_batch_number.unwrap().as_u64().into(),
                    proof.id.into(),
                )
                .await
                .map_err(Into::into)
        }
    }
}

struct FinalizeResult {
    failed: Vec<RequestFinalizeWithdrawal>,
}
