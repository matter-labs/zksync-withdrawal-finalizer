//! ABI wrappers for the `L2StandardToken` contract.

use std::sync::Arc;

use ethers::{
    prelude::{Contract, Event},
    providers::{Middleware, PubsubClient},
    types::{Address, BlockNumber},
};
use futures::{Sink, SinkExt, StreamExt};

use crate::{ethtoken::WithdrawalFilter, Result, WithdrawalEvent};

use self::codegen::BridgeBurnFilter;

mod codegen {
    use ethers::prelude::abigen;

    abigen!(L2StandardToken, "./src/contracts/IL2StandardToken.json",);
}

/// A struct wrapper for interfacing with `L2StandardToken` contract.
pub struct L2StandardToken<M> {
    contract: codegen::L2StandardToken<M>,
}

const ETH_TOKEN_ADDRESS: &str = "0x000000000000000000000000000000000000800a";

impl<M: Middleware> L2StandardToken<M> {
    /// Create a new instance of `L2StandardToken` contract.
    ///
    /// # Arguments
    ///
    /// * `address` - An address of the contract
    /// * `provider` - A middleware to perform calls to the contract.
    pub fn new(address: Address, provider: Arc<M>) -> Self {
        let contract = codegen::L2StandardToken::new(address, provider);

        Self { contract }
    }

    /// Withdrawal Events emitted in a block interval
    ///
    /// # Arguments
    ///
    /// * `from_block` - beginning of the block interval
    /// * `to_block` - end of the block interval
    pub async fn withdrawal_events(
        &self,
        from_block: BlockNumber,
        to_block: BlockNumber,
    ) -> Result<Vec<WithdrawalEvent>> {
        let events = self
            .contract
            .event::<codegen::BridgeBurnFilter>()
            .from_block(from_block)
            .to_block(to_block)
            .query_with_meta()
            .await?
            .into_iter()
            .map(|(event, meta)| WithdrawalEvent {
                tx_hash: meta.transaction_hash,
                block_number: meta.block_number.as_u64(),
                token: meta.address,
                amount: event.amount,
            })
            .collect();

        Ok(events)
    }
}

/// A convenience multiplexer for withdrawal-related events.
pub struct WithdrawalEvents<M> {
    withdrawal_events: Event<Arc<M>, M, BridgeBurnFilter>,
    token_withdrawals: Event<Arc<M>, M, WithdrawalFilter>,
}

impl<M> WithdrawalEvents<M>
where
    M: Middleware,
{
    /// Create a new `WithdrawalEvents` structure.
    ///
    /// # Arguments
    ///
    /// * `middleware`: THe middleware to perform requests with.
    pub async fn new(middleware: Arc<M>) -> Result<Self> {
        let withdrawal_events = Contract::event_of_type::<BridgeBurnFilter>(middleware.clone());

        let token_withdrawals = Contract::event_of_type::<WithdrawalFilter>(middleware);

        Ok(Self {
            withdrawal_events,
            token_withdrawals,
        })
    }
}

impl<M> WithdrawalEvents<M>
where
    M: Middleware,
    <M as Middleware>::Provider: PubsubClient,
{
    /// A convenience function that listens for all withdrawal events on L2
    ///
    /// For more reasoning about the necessity of this function
    /// check the similar [`BlockEvents::run()`].
    ///
    /// # Arguments
    ///
    /// * `addresses`: The addrs of the ERC20 tokens on L1 to monitor
    /// * `from_block`: Query the chain from this particular block
    /// * `sender`: The `Sink` to send received events into.
    pub async fn run<B, S>(
        self,
        addresses: Vec<Address>,
        from_block: B,
        mut sender: S,
    ) -> Result<()>
    where
        B: Into<BlockNumber> + Copy,
        S: Sink<WithdrawalEvent> + Unpin,
        <S as Sink<WithdrawalEvent>>::Error: std::fmt::Debug,
    {
        let sub_withdrawals = self
            .withdrawal_events
            .from_block(from_block.into())
            .address(addresses.clone().into());

        let mut sub_withdrawals_s = sub_withdrawals.subscribe_with_meta().await?.fuse();

        let eth_token_address: Address = ETH_TOKEN_ADDRESS
            .parse()
            .expect("eth token address constant is correct; qed");

        let eth_withdrawals = self
            .token_withdrawals
            .from_block(from_block.into())
            .address(eth_token_address.into());

        let mut eth_withdrawals_s = eth_withdrawals.subscribe_with_meta().await?.fuse();

        loop {
            futures::select! {
                we = sub_withdrawals_s.next() => {
                    if let Some(event) = we {
                        let (event, meta) = event?;

                        let we = WithdrawalEvent {
                            tx_hash: meta.transaction_hash,
                            block_number: meta.block_number.as_u64(),
                            token: meta.address,
                            amount: event.amount,
                        };

                        sender.send(we).await.unwrap();
                    }
                },
                eth = eth_withdrawals_s.next() => {
                    if let Some(event) = eth {
                        let (event, meta) = event?;

                        let we = WithdrawalEvent {
                            tx_hash: meta.transaction_hash,
                            block_number: meta.block_number.as_u64(),
                            token: meta.address,
                            amount: event.amount,
                        };

                        sender.send(we).await.unwrap();
                    }
                }
                complete => break,
            }
        }

        Ok(())
    }
}
