use std::sync::Arc;

use chrono::{prelude::*, Datelike, TimeZone, Utc};
use ethers::providers::{Http, Provider};
use pretty_assertions::assert_eq;

#[tokio::test]
async fn request_first_block_unlimited() {
    let provider_l2 = Provider::<Http>::try_from("https://mainnet.era.zksync.io").unwrap();
    let client_l2 = Arc::new(provider_l2);

    let date = "2023-10-5T12:00:00Z".parse::<DateTime<Utc>>().unwrap();

    let previous_midnight = Utc
        .with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0)
        .unwrap();

    let res = client::get_block_number_by_timestamp(previous_midnight, None, client_l2)
        .await
        .unwrap()
        .unwrap();

    // First block on 2023-10-5 https://explorer.zksync.io/block/15560287
    assert_eq!(res, 15560287.into());
}

#[tokio::test]
async fn request_first_block_limited() {
    let provider_l2 = Provider::<Http>::try_from("https://mainnet.era.zksync.io").unwrap();
    let client_l2 = Arc::new(provider_l2);

    let date = "2023-10-5T12:00:00Z".parse::<DateTime<Utc>>().unwrap();

    let previous_midnight = Utc
        .with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0)
        .unwrap();

    let res =
        client::get_block_number_by_timestamp(previous_midnight, Some(15000000.into()), client_l2)
            .await
            .unwrap()
            .unwrap();

    // First block on 2023-10-5 https://explorer.zksync.io/block/15560287
    assert_eq!(res, 15560287.into());
}

#[tokio::test]
async fn request_from_the_future() {
    let provider_l2 = Provider::<Http>::try_from("https://mainnet.era.zksync.io").unwrap();
    let client_l2 = Arc::new(provider_l2);

    let date = Utc::now();

    let previous_midnight = Utc
        .with_ymd_and_hms(date.year(), date.month(), date.day() + 1, 0, 0, 0)
        .unwrap();

    let res =
        client::get_block_number_by_timestamp(previous_midnight, Some(15000000.into()), client_l2)
            .await
            .unwrap();

    // No first block in the next day
    assert_eq!(res, None);
}
