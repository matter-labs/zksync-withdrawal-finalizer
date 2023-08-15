#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Wrapper for transaction sending with adjusting a gas price on retries.

use std::{sync::Arc, time::Duration};

use ethers::{
    prelude::NonceManagerMiddleware,
    providers::Middleware,
    types::{transaction::eip2718::TypedTransaction, TransactionReceipt, U256},
};

mod error;

pub use error::{Error, Result};

/// Send a transaction with specified number of retries.
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
    m: Arc<NonceManagerMiddleware<M>>,
    tx: T,
    retry_timeout: Duration,
    retries: usize,
    gas_increase_step: Option<U256>,
) -> Result<Option<TransactionReceipt>, M>
where
    M: Middleware,
    T: Into<TypedTransaction> + Send + Sync + Clone,
{
    let mut gas_price = m.get_gas_price().await?;

    let nonce = m.next();
    for _ in 0..retries {
        let mut submit_tx = tx.clone().into();

        submit_tx.set_gas_price(gas_price);
        submit_tx.set_nonce(nonce);

        let sent_tx = m.send_transaction(submit_tx, None).await?;

        let tx_hash = sent_tx.tx_hash();

        let result = tokio::time::timeout(retry_timeout, sent_tx).await;

        match result {
            Ok(res) => {
                return Ok(res?);
            }
            Err(_e) => {
                vlog::info!("waiting for mined transaction {tx_hash:?} timed out",);

                if let Some(gas_increase_step) = gas_increase_step {
                    gas_price += gas_increase_step;
                } else {
                    gas_price = m.get_gas_price().await?;
                }
            }
        }
    }

    Err(Error::Timedout)
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use ethers::{
        middleware::MiddlewareBuilder,
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

        let nonce_manager = Arc::new(provider.nonce_manager(from));

        let tx = TransactionRequest::new()
            .to(to)
            .value(1000)
            .from(from)
            .gas_price(gas_price);

        send_tx_adjust_gas(
            nonce_manager.clone(),
            Into::<TransactionRequest>::into(tx),
            Duration::from_secs(1),
            3,
            Some(10_000_u64.into()),
        )
        .await
        .unwrap_err();

        let mut inspect = nonce_manager.txpool_content().await.unwrap();

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

        let nonce_manager = Arc::new(provider.nonce_manager(from));

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
            .gas_price(gas_price);

        let tx_2 = TransactionRequest::new()
            .to(to_2)
            .value(1000)
            .from(from)
            .gas_price(gas_price);

        let (first, second) = tokio::join!(
            send_tx_adjust_gas(
                nonce_manager.clone(),
                Into::<TransactionRequest>::into(tx_1),
                Duration::from_secs(1),
                3,
                Some(10_000_u64.into()),
            ),
            send_tx_adjust_gas(
                nonce_manager.clone(),
                Into::<TransactionRequest>::into(tx_2),
                Duration::from_secs(1),
                3,
                Some(10_000_u64.into()),
            )
        );

        match first.unwrap_err() {
            Error::Timedout => (),
            _ => panic!("Expected Timeout error"),
        };
        match second.unwrap_err() {
            Error::Timedout => (),
            _ => panic!("Expected Timeout error"),
        };

        let mut inspect = nonce_manager.txpool_content().await.unwrap();

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
