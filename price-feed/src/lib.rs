use std::time::Duration;

use coingecko::CoinGeckoClient;
use ethers::abi::{AbiEncode, Address};
use sqlx::PgPool;

const PRICE_UPDATER_LOOP: Duration = Duration::from_secs(60 * 60);

pub async fn current_token_price(client: &CoinGeckoClient, address: Address) -> Option<f64> {
    let res = client
        .contract("ethereum", address.encode_hex().as_str())
        .await
        .unwrap();

    res.market_data.current_price.usd
}

pub async fn price_updater_loop(client: CoinGeckoClient, pool: PgPool) {
    loop {
        tokio::time::sleep(PRICE_UPDATER_LOOP).await;

        let tokens = storage::get_all_tokens(&pool).await.unwrap();
        let prices = futures::future::join_all(tokens.iter().map(|token| async {
            (
                token.l1_token_address,
                current_token_price(&client, token.l1_token_address).await,
            )
        }))
        .await;
    }
}
