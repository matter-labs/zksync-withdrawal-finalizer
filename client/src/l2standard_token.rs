//! ABI wrappers for the `L2StandardToken` contract.

use std::sync::Arc;

use ethers::{
    abi::RawLog,
    contract::EthEvent,
    providers::{Middleware, PubsubClient},
    types::{Address, BlockNumber, Filter},
};
use futures::{Sink, SinkExt, StreamExt};

use crate::{ethtoken::WithdrawalFilter, Error, Result, WithdrawalEvent};

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
pub struct WithdrawalEventsStream<M> {
    middleware: Arc<M>,
}

impl<M> WithdrawalEventsStream<M>
where
    M: Middleware,
{
    /// Create a new `WithdrawalEvents` structure.
    ///
    /// # Arguments
    ///
    /// * `middleware`: THe middleware to perform requests with.
    pub async fn new(middleware: Arc<M>) -> Result<Self> {
        Ok(Self { middleware })
    }
}

impl<M> WithdrawalEventsStream<M>
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
        mut addresses: Vec<Address>,
        from_block: B,
        mut sender: S,
    ) -> Result<()>
    where
        B: Into<BlockNumber> + Copy,
        S: Sink<WithdrawalEvent> + Unpin,
        <S as Sink<WithdrawalEvent>>::Error: std::fmt::Debug,
    {
        addresses.push(
            ETH_TOKEN_ADDRESS
                .parse()
                .expect("eth token address constant is correct; qed"),
        );
        let filter = Filter::new()
            .from_block(from_block)
            .address(addresses)
            .topic0(vec![
                BridgeBurnFilter::signature(),
                WithdrawalFilter::signature(),
            ]);

        let logs = self
            .middleware
            .subscribe_logs(&filter)
            .await
            .map_err(|e| Error::Middleware(format!("{e}")))?;

        let mut logs = logs.fuse();
        loop {
            tokio::select! {
                Some(log) = logs.next() => {
                    let raw_log: RawLog = log.clone().into();

                    if let Ok(burn_event) = BridgeBurnFilter::decode_log(&raw_log) {
                        if let (Some(tx_hash), Some(block_number)) = (log.transaction_hash, log.block_number) {
                            let we = WithdrawalEvent {
                                tx_hash,
                                block_number: block_number.as_u64(),
                                token: log.address,
                                amount: burn_event.amount,
                            };
                            sender.send(we).await.map_err(|_| Error::ChannelClosed)?;
                        }
                        continue;
                    }

                    if let Ok(withdrawal_event) = WithdrawalFilter::decode_log(&raw_log) {
                        if let (Some(tx_hash), Some(block_number)) = (log.transaction_hash, log.block_number) {
                            let we = WithdrawalEvent {
                                tx_hash,
                                block_number: block_number.as_u64(),
                                token: log.address,
                                amount: withdrawal_event.amount,
                            };
                            sender.send(we).await.map_err(|_| Error::ChannelClosed)?;
                        }
                    }
                },
                else => {
                    log::info!("withdrawal streams being closed");
                    break
                }
            }
        }

        Ok(())
    }
}
