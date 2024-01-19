use std::time::Duration;

use ethers::abi::{AbiEncode, Address};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client, ClientBuilder, Url,
};
use sqlx::PgPool;

const PRICE_UPDATER_LOOP: Duration = Duration::from_secs(60 * 60);

#[derive(Clone)]
pub struct CoinGeckoClient {
    client: Client,
    url: Url,
    token: String,
}

impl CoinGeckoClient {
    pub fn new(url: Url, coingecko_token: &str) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-cg-demo-api-key",
            HeaderValue::from_str(coingecko_token)
                .expect("coingecko token is a valid http header value; qed"),
        );

        println!("token {coingecko_token}");

        let client = ClientBuilder::new()
            .default_headers(headers)
            .build()
            .expect("can always build a coingecko client; qed");

        Self {
            client,
            url,
            token: coingecko_token.to_string(),
        }
    }

    pub async fn current_token_price(&self, address: Address) -> Option<f64> {
        let res = self
            .client
            .get(format!(
                "{}/coins/ethereum/contract/{}&x_cg_demo_api_key={}",
                self.url,
                address.encode_hex(),
                self.token
            ))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();

        panic!("res {res:?}");

        None
    }
}

pub async fn price_updater_loop(pool: PgPool, client: CoinGeckoClient) {
    loop {
        tokio::time::sleep(PRICE_UPDATER_LOOP).await;

        let tokens = storage::get_all_tokens(&pool).await.unwrap();
        let prices = futures::future::join_all(tokens.iter().map(|token| async {
            (
                token.l1_token_address,
                client.current_token_price(token.l1_token_address).await,
            )
        }))
        .await;
    }
}
