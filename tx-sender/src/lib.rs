#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Wrapper for transaction sending with adjusting a gas price on retries.

use std::time::Duration;

use ethers::{
    providers::Middleware,
    types::{transaction::eip2718::TypedTransaction, TransactionReceipt, U256},
};

mod error;

pub use error::{Error, Result};

/// Send a transaction with specified number of retries.
///
///
/// # Panics
///
/// Panics if tx nonce is not set.
///
/// # Arguments
///
/// * `m`: [`Middleware`] to perform request with
/// * `tx`: Transaction to be sent
/// * `retry_timeout`: A period after which to retry transaction.
/// * `retries`: Amount of retries to perform.
/// * `gas_increase_step`: If present increase gas price by this  number on every step, else ask
///    actual gas price from `m` on every retry.
pub async fn send_tx_adjust_gas<M, T>(
    m: M,
    tx: T,
    retry_timeout: Duration,
    retries: usize,
    gas_increase_step: Option<U256>,
) -> Result<Option<TransactionReceipt>>
where
    M: Middleware,
    T: Into<TypedTransaction> + Send + Sync + Clone,
{
    let mut gas_price = m
        .get_gas_price()
        .await
        .map_err(|e| Error::Middleware(format!("{e}")))?;

    for _ in 0..retries {
        let mut submit_tx = tx.clone().into();

        if submit_tx.nonce().is_none() {
            panic!("tx expected to have a nonce; qed");
        }

        submit_tx.set_gas_price(gas_price);

        println!("sending transaction {submit_tx:?}");

        let sent_tx = m.send_transaction(submit_tx, None).await.unwrap();
        let tx_hash = sent_tx.tx_hash();

        let result = tokio::time::timeout(retry_timeout, sent_tx).await;

        match result {
            Ok(res) => {
                return res.map_err(|e| Error::Middleware(format!("{e}")));
            }
            Err(_e) => {
                vlog::info!("waiting for mined transaction {tx_hash:?} timed out",);

                if let Some(gas_increase_step) = gas_increase_step {
                    gas_price += gas_increase_step;
                } else {
                    gas_price = m
                        .get_gas_price()
                        .await
                        .map_err(|e| Error::Middleware(format!("{e}")))?;
                }
            }
        }
    }

    Err(Error::Timedout)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ethers::{
        prelude::MiddlewareBuilder,
        providers::{Middleware, Provider, ProviderExt},
        types::{TransactionRequest, U256},
        utils::Anvil,
    };
    use pretty_assertions::assert_eq;

    use crate::{send_tx_adjust_gas, Error};

    #[tokio::test(flavor = "multi_thread")]
    async fn retry_sending_single_tx() {
        let anvil = Anvil::new().arg("--no-mining").spawn();

        let provider = Provider::<ethers::providers::Http>::connect(&anvil.endpoint()).await;

        // connect to the network
        let accounts = provider.get_accounts().await.unwrap();
        let from = accounts[0];
        let to = accounts[1];

        let gas_price = provider.get_gas_price().await.unwrap();
        let mut expected_gas_price = gas_price;
        let gas_bump = 10_000_u64.into();

        for _ in 0..2 {
            expected_gas_price += gas_bump;
        }

        let tx = TransactionRequest::new()
            .to(to)
            .value(1000)
            .from(from)
            .gas_price(gas_price);

        send_tx_adjust_gas(
            &provider,
            Into::<TransactionRequest>::into(tx),
            Duration::from_secs(1),
            3,
            Some(10_000_u64.into()),
        )
        .await
        .unwrap_err();

        let mut inspect = provider.txpool_content().await.unwrap();

        assert_eq!(inspect.pending.len(), 1);
        assert_eq!(inspect.queued.len(), 0);

        let (addr, mut txs) = inspect.pending.pop_first().unwrap();
        assert_eq!(addr, from);

        assert_eq!(txs.len(), 1);

        let (nonce_str, tx) = txs.pop_first().unwrap();

        assert_eq!(nonce_str.parse::<usize>().unwrap(), 0);
        assert_eq!(tx.nonce, U256::zero());
        assert_eq!(tx.gas_price.unwrap(), expected_gas_price);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn retry_sending_two_tx() {
        let anvil = Anvil::new().arg("--no-mining").spawn();

        let provider = Provider::<ethers::providers::Http>::connect(&anvil.endpoint()).await;

        // connect to the network
        let accounts = provider.get_accounts().await.unwrap();
        let from = accounts[0];
        let to_1 = accounts[1];
        let to_2 = accounts[2];

        let nonce_manager = provider.nonce_manager(from);

        let gas_price = nonce_manager.get_gas_price().await.unwrap();
        let mut expected_gas_price = gas_price;
        let gas_bump = 10_000_u64.into();

        for _ in 0..2 {
            expected_gas_price += gas_bump;
        }

        let tx_1 = TransactionRequest::new()
            .to(to_1)
            .value(1000)
            .from(from)
            .gas_price(gas_price)
            .nonce(nonce_manager.next());

        let tx_2 = TransactionRequest::new()
            .to(to_2)
            .value(1000)
            .from(from)
            .gas_price(gas_price)
            .nonce(nonce_manager.next());

        let (first, second) = tokio::join!(
            send_tx_adjust_gas(
                &nonce_manager,
                Into::<TransactionRequest>::into(tx_1),
                Duration::from_secs(1),
                3,
                Some(10_000_u64.into()),
            ),
            send_tx_adjust_gas(
                &nonce_manager,
                Into::<TransactionRequest>::into(tx_2),
                Duration::from_secs(1),
                3,
                Some(10_000_u64.into()),
            )
        );

        assert_eq!(first.unwrap_err(), Error::Timedout);
        assert_eq!(second.unwrap_err(), Error::Timedout);

        let mut inspect = nonce_manager.txpool_content().await.unwrap();
        println!("inspect {inspect:?}");

        assert_eq!(inspect.pending.len(), 1);
        assert_eq!(inspect.queued.len(), 0);
        let (addr, mut txs) = inspect.pending.pop_first().unwrap();

        assert_eq!(addr, from);
        assert_eq!(txs.len(), 2);

        let (nonce_str, tx) = txs.pop_first().unwrap();

        assert_eq!(nonce_str.parse::<usize>().unwrap(), 0);
        assert_eq!(tx.nonce, U256::zero());
        assert_eq!(tx.gas_price.unwrap(), expected_gas_price);

        let (nonce_str, tx) = txs.pop_first().unwrap();

        assert_eq!(nonce_str.parse::<usize>().unwrap(), 1);
        assert_eq!(tx.nonce, U256::one());
        assert_eq!(tx.gas_price.unwrap(), expected_gas_price);
    }
}
