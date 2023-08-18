#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Wrapper for transaction sending with adjusting a gas price on retries.

use std::{time::Duration, u8};

use ethers::{
    providers::{Middleware, MiddlewareError, ProviderError},
    types::{
        transaction::eip2718::TypedTransaction, BlockNumber, Eip2930TransactionRequest,
        TransactionReceipt, U256,
    },
};

const RETRY_BUMP_FEES_PERCENT: u8 = 15;

/// Bump prices of a `TypedTransaction` depending on its type.
///
/// For non-`eip1559` txs the `gas_price` is bumped by the given percentage.
///
/// For `eip1559` txs:
///  * the `max_priority_fee_per_gas` is bumped by the given percentage
///  * the `max_fee_per_gas` is bumped by the flat value
///    `max_priority_fee_per_gas` was bumped with.
async fn bump_predicted_fees<M: Middleware>(
    tx: &mut TypedTransaction,
    percent: u8,
    m: M,
) -> Result<(), M::Error> {
    match tx {
        TypedTransaction::Legacy(ref mut tx)
        | TypedTransaction::Eip2930(Eip2930TransactionRequest { ref mut tx, .. }) => {
            if let Some(gas_price) = tx.gas_price.as_mut() {
                *gas_price = *gas_price + inc_u256_percent(*gas_price, percent);
            }
        }
        TypedTransaction::Eip1559(ref mut tx) => {
            let base_fee_per_gas = m
                .get_block(BlockNumber::Latest)
                .await?
                .ok_or_else(|| {
                    <M::Error>::from_provider_err(ProviderError::CustomError(
                        "Latest block not found".into(),
                    ))
                })?
                .base_fee_per_gas
                .ok_or_else(|| {
                    <M::Error>::from_provider_err(ProviderError::CustomError(
                        "EIP-1559 not activated".into(),
                    ))
                })?;

            let mut bump = U256::zero();

            if let Some(max_priority_fee_per_gas) = tx.max_priority_fee_per_gas.as_mut() {
                bump = inc_u256_percent(*max_priority_fee_per_gas, percent);
                *max_priority_fee_per_gas += bump;
            }

            if let Some(max_fee_per_gas) = tx.max_fee_per_gas.as_mut() {
                *max_fee_per_gas = std::cmp::max(
                    *max_fee_per_gas + bump,
                    base_fee_per_gas + tx.max_priority_fee_per_gas.unwrap_or(U256::zero()),
                );
            }
        }
    }

    Ok(())
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
    m: M,
    tx: T,
    retry_timeout: Duration,
    nonce: U256,
) -> Result<Option<TransactionReceipt>, <M as Middleware>::Error>
where
    M: Middleware,
    T: Into<TypedTransaction> + Send + Sync + Clone,
{
    let mut submit_tx = tx.into();
    m.fill_transaction(&mut submit_tx, None).await?;
    submit_tx.set_nonce(nonce);

    for retry_num in 0..usize::MAX {
        if retry_num > 0 {
            bump_predicted_fees(&mut submit_tx, RETRY_BUMP_FEES_PERCENT, &m).await?;
            submit_tx.set_nonce(nonce);
        }

        let sent_tx = m.send_transaction(submit_tx.clone(), None).await?;

        let tx_hash = sent_tx.tx_hash();

        let result = tokio::time::timeout(retry_timeout, sent_tx).await;

        match result {
            Ok(res) => {
                let res = res.map_err(MiddlewareError::from_provider_err)?;
                return Ok(res);
            }
            Err(_e) => {
                vlog::info!("waiting for mined transaction {tx_hash:?} timed out",);
                metrics::increment_counter!("finalizer.tx_sender.timedout_transactions");
            }
        }
    }

    panic!("going to usize::MAX with delays is too far away in the future; qed");
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use ethers::{
        providers::{Middleware, Provider, ProviderExt},
        types::{
            transaction::eip2718::TypedTransaction, Eip1559TransactionRequest, TransactionRequest,
            U256,
        },
        utils::Anvil,
    };
    use pretty_assertions::assert_eq;

    use crate::{inc_u256_percent, send_tx_adjust_gas, RETRY_BUMP_FEES_PERCENT};

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
            expected_gas_price += inc_u256_percent(expected_gas_price, RETRY_BUMP_FEES_PERCENT);
        }

        let provider = Arc::new(provider);

        let tx = TransactionRequest::new()
            .to(to)
            .value(1000)
            .from(from)
            .gas_price(gas_price);

        tokio::time::timeout(Duration::from_secs(3), async {
            send_tx_adjust_gas(provider.clone(), tx, Duration::from_secs(1), 0.into())
                .await
                .unwrap_err()
        })
        .await
        .ok();

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

        let provider = Arc::new(provider);

        let gas_price = provider.get_gas_price().await.unwrap();
        let mut expected_gas_price = gas_price;

        for _ in 0..2 {
            expected_gas_price += inc_u256_percent(expected_gas_price, RETRY_BUMP_FEES_PERCENT);
        }

        let tx_1 = TransactionRequest::new()
            .to(to_1)
            .value(1000)
            .from(from)
            .gas_price(gas_price)
            .nonce(0);

        let tx_2 = TransactionRequest::new()
            .to(to_2)
            .value(1000)
            .from(from)
            .gas_price(gas_price)
            .nonce(1);

        tokio::time::timeout(Duration::from_secs(3), async {
            let (first, second) = tokio::join!(
                send_tx_adjust_gas(provider.clone(), tx_1, Duration::from_secs(1), 0.into()),
                send_tx_adjust_gas(provider.clone(), tx_2, Duration::from_secs(1), 1.into())
            );
            first.unwrap();
            second.unwrap();
        })
        .await
        .ok();

        let mut inspect = provider.txpool_content().await.unwrap();

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

        super::bump_predicted_fees(&mut eip_1559_tx, 10, &provider)
            .await
            .unwrap();

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

        super::bump_predicted_fees(&mut legacy_tx, 10, &provider)
            .await
            .unwrap();

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

        for _ in 0..2 {
            let bump = inc_u256_percent(priority_fee, RETRY_BUMP_FEES_PERCENT);

            priority_fee += bump;
            max_fee += bump;
        }

        let provider = Arc::new(provider);

        let tx = Eip1559TransactionRequest::new()
            .to(to)
            .value(1000)
            .from(from);

        tokio::time::timeout(Duration::from_secs(3), async {
            send_tx_adjust_gas(provider.clone(), tx, Duration::from_secs(1), 0.into())
                .await
                .unwrap()
        })
        .await
        .ok();

        let mut inspect = provider.txpool_content().await.unwrap();
        println!("inspect {inspect:?}");

        assert_eq!(inspect.pending.len(), 1);
        assert_eq!(inspect.queued.len(), 0);

        let (addr, mut txs) = inspect.pending.pop_first().unwrap();
        assert_eq!(addr, from);

        assert_eq!(txs.len(), 1);

        let (nonce_str, tx) = txs.pop_first().unwrap();

        assert_eq!(nonce_str.parse::<usize>().unwrap(), 0);
        assert_eq!(tx.nonce, U256::zero());
        assert_eq!(tx.max_priority_fee_per_gas.unwrap(), priority_fee);
        assert_eq!(tx.max_fee_per_gas.unwrap(), max_fee);
    }
}
