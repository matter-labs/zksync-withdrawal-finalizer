#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Wrapper for transaction sending with adjusting a gas price on retries.

use std::{sync::Arc, time::Duration, u8};

use ethers::{
    prelude::NonceManagerMiddleware,
    providers::Middleware,
    types::{
        transaction::eip2718::TypedTransaction, Eip2930TransactionRequest, TransactionReceipt, U256,
    },
};

mod error;

pub use error::{Error, Result};

const RETRY_BUMP_FEES_PERCENT: u8 = 15;

fn bump_predicted_fees(tx: &mut TypedTransaction, percent: u8) {
    match tx {
        TypedTransaction::Legacy(ref mut tx)
        | TypedTransaction::Eip2930(Eip2930TransactionRequest { ref mut tx, .. }) => {
            tx.gas_price
                .as_mut()
                .map(|gas_price| *gas_price = *gas_price + inc_u256_percent(*gas_price, percent));
        }
        TypedTransaction::Eip1559(ref mut tx) => {
            let mut bump = U256::zero();
            tx.max_priority_fee_per_gas.as_mut().map(|gas_price| {
                bump = inc_u256_percent(*gas_price, percent);
                *gas_price = *gas_price + bump;
            });

            tx.max_fee_per_gas.as_mut().map(|gas_price| {
                *gas_price += bump;
            });
        }
    }
}

fn inc_u256_percent(num: U256, percent: u8) -> U256 {
    num.saturating_mul(percent.into()) / 100
}

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
) -> Result<Option<TransactionReceipt>, M>
where
    M: Middleware,
    T: Into<TypedTransaction> + Send + Sync + Clone,
{
    let nonce = m.next();

    for retry_num in 0..retries {
        let mut submit_tx = tx.clone().into();

        m.fill_transaction(&mut submit_tx, None).await?;

        submit_tx.set_nonce(nonce);

        if retry_num > 0 {
            bump_predicted_fees(&mut submit_tx, retry_num as u8 * RETRY_BUMP_FEES_PERCENT);
        }

        let sent_tx = m.send_transaction(submit_tx, None).await?;

        let tx_hash = sent_tx.tx_hash();

        let result = tokio::time::timeout(retry_timeout, sent_tx).await;

        match result {
            Ok(res) => {
                return Ok(res?);
            }
            Err(_e) => {
                vlog::info!("waiting for mined transaction {tx_hash:?} timed out",);
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
        types::{
            transaction::eip2718::TypedTransaction, Eip1559TransactionRequest, TransactionRequest,
            U256,
        },
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

        for _ in 0..2 {
            expected_gas_price += gas_price * 15 / 100;
        }

        let nonce_manager = Arc::new(provider.nonce_manager(from));

        let tx = TransactionRequest::new()
            .to(to)
            .value(1000)
            .from(from)
            .gas_price(gas_price);

        send_tx_adjust_gas(nonce_manager.clone(), tx, Duration::from_secs(1), 3)
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

        assert_eq!(tx.max_fee_per_gas, None);
        assert_eq!(tx.max_priority_fee_per_gas, None);
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

        for _ in 0..2 {
            expected_gas_price += gas_price * 15 / 100;
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
            send_tx_adjust_gas(nonce_manager.clone(), tx_1, Duration::from_secs(1), 3),
            send_tx_adjust_gas(nonce_manager.clone(), tx_2, Duration::from_secs(1), 3)
        );

        match first.unwrap_err() {
            Error::Timedout => (),
            a => panic!("Expected Timeout error {a:?}"),
        };
        match second.unwrap_err() {
            Error::Timedout => (),
            a => panic!("Expected Timeout error {a:?}"),
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

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tx_price_bumps() {
        let anvil = Anvil::new().arg("--no-mining").spawn();

        let provider = Provider::<ethers::providers::Http>::connect(&anvil.endpoint()).await;

        let accounts = provider.get_accounts().await.unwrap();
        let from = accounts[0];
        let to = accounts[1];

        let mut eip_1559_tx: TypedTransaction = Eip1559TransactionRequest::new()
            .to(to)
            .value(1000)
            .from(from)
            .into();

        provider
            .fill_transaction(&mut eip_1559_tx, None)
            .await
            .unwrap();

        let (estimated_max_priority_fee_per_gas, estimated_max_fee_per_gas) = match eip_1559_tx {
            TypedTransaction::Eip1559(ref tx) => (
                tx.max_priority_fee_per_gas.unwrap(),
                tx.max_fee_per_gas.unwrap(),
            ),
            _ => panic!("expected eip1559 tx"),
        };

        super::bump_predicted_fees(&mut eip_1559_tx, 10);

        let (bumped_max_priority_fee_per_gas, bumped_max_fee_per_gas) = match eip_1559_tx {
            TypedTransaction::Eip1559(ref tx) => (
                tx.max_priority_fee_per_gas.unwrap(),
                tx.max_fee_per_gas.unwrap(),
            ),
            _ => panic!("expected eip1559 tx"),
        };

        assert_eq!(
            estimated_max_priority_fee_per_gas + estimated_max_priority_fee_per_gas / 10,
            bumped_max_priority_fee_per_gas
        );
        assert_eq!(
            estimated_max_fee_per_gas + estimated_max_priority_fee_per_gas / 10,
            bumped_max_fee_per_gas
        );

        let mut legacy_tx: TypedTransaction = TransactionRequest::new()
            .to(to)
            .value(1000)
            .from(from)
            .into();

        provider
            .fill_transaction(&mut legacy_tx, None)
            .await
            .unwrap();

        let estimated_gas_price = match legacy_tx {
            TypedTransaction::Legacy(ref tx) => tx.gas_price.unwrap(),
            _ => panic!("expecged legacy tx"),
        };

        super::bump_predicted_fees(&mut legacy_tx, 10);

        let bumped_gas_price = match legacy_tx {
            TypedTransaction::Legacy(ref tx) => tx.gas_price.unwrap(),
            _ => panic!("expecged legacy tx"),
        };

        assert_eq!(
            estimated_gas_price + estimated_gas_price / 10,
            bumped_gas_price
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn retry_sending_single_eip1559_tx() {
        let anvil = Anvil::new().arg("--no-mining").spawn();

        let provider = Provider::<ethers::providers::Http>::connect(&anvil.endpoint()).await;

        let accounts = provider.get_accounts().await.unwrap();
        let from = accounts[0];
        let to = accounts[1];

        let (mut max_fee, mut priority_fee) = provider.estimate_eip1559_fees(None).await.unwrap();

        max_fee += priority_fee * 30 / 100;
        priority_fee += priority_fee * 30 / 100;

        let nonce_manager = Arc::new(provider.nonce_manager(from));

        let tx = Eip1559TransactionRequest::new()
            .to(to)
            .value(1000)
            .from(from);

        send_tx_adjust_gas(nonce_manager.clone(), tx, Duration::from_secs(1), 3)
            .await
            .unwrap_err();

        let mut inspect = nonce_manager.txpool_content().await.unwrap();
        println!("inspect {inspect:?}");

        assert_eq!(inspect.pending.len(), 1);
        assert_eq!(inspect.queued.len(), 0);

        let (addr, mut txs) = inspect.pending.pop_first().unwrap();
        assert_eq!(addr, from);

        assert_eq!(txs.len(), 1);

        let (nonce_str, tx) = txs.pop_first().unwrap();

        assert_eq!(nonce_str.parse::<usize>().unwrap(), 0);
        assert_eq!(tx.nonce, U256::zero());
        assert_eq!(tx.max_fee_per_gas.unwrap(), max_fee);
        assert_eq!(tx.max_priority_fee_per_gas.unwrap(), priority_fee);
    }
}
