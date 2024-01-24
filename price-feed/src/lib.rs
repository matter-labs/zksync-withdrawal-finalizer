use ethers::abi::Address;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client, ClientBuilder, Url,
};

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
        let res = match self
            .client
            .get(format!(
                "{}/coins/ethereum/contract/{:?}?x_cg_demo_api_key={}",
                self.url, address, self.token
            ))
            .send()
            .await
            .unwrap()
            .text()
            .await
        {
            Ok(res) => res,
            Err(e) => {
                tracing::warn!("Failed to query token {address:?} price: {e}");
                return None;
            }
        };

        let json: serde_json::Value = serde_json::from_str(&res).unwrap();
        let token_price = &json["market_data"]["current_price"]["usd"];

        tracing::error!("res {token_price:?}");

        None
    }

    pub async fn historical_token_price(&self, address: Address, timestamp: u64) -> Option<f64> {
        let res = match self.client
            .get(
                format!("{}/coins/ethereum/contract/{address:?}/market_chart/range?vs_currency=usd&from={timestamp}&to={}", self.url, timestamp + 10000))
            .send()
            .await
            .unwrap()
            .text()
            .await {
                Ok(res) => res,
                Err(e) => {
                    tracing::warn!("Failed to query token {address:?} price: {e}");
                    return None;
                }};

        let response: serde_json::Value = serde_json::from_str(&res).unwrap();

        let price = &response["prices"][0][1];

        tracing::error!("past timestamp is {price:?} {:?}", price.as_f64());

        price.as_f64()
    }
}
