use axum::extract::{Path, Query, State};
use axum::{http::StatusCode, routing::get, Json, Router};
use ethers::abi::Address;
use ethers::types::{H256, U256};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use storage::UserWithdrawal;
use tower_http::cors::CorsLayer;

#[derive(Deserialize, Serialize, Clone)]
struct WithdrawalRequest {
    pub limit: u64,
}
#[derive(Deserialize, Serialize, Clone)]
struct WithdrawalResponse {
    pub tx_hash: H256,
    pub token: Address,
    pub amount: U256,
    pub status: String,
}

impl From<UserWithdrawal> for WithdrawalResponse {
    fn from(withdrawal: UserWithdrawal) -> Self {
        Self {
            tx_hash: withdrawal.tx_hash,
            token: withdrawal.token,
            amount: withdrawal.amount,
            status: format!("{:?}", withdrawal.status),
        }
    }
}

pub async fn run_server(pool: PgPool) {
    let cors_layer = CorsLayer::permissive();
    let app = Router::new()
        .route("/withdrawals/:from", get(get_withdrawals))
        .route("/health", get(health))
        .layer(cors_layer)
        .with_state(pool);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health(State(pool): State<PgPool>) -> Result<&'static str, StatusCode> {
    pool.acquire()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok("ok")
}

async fn get_withdrawals(
    Path(from): Path<Address>,
    State(pool): State<PgPool>,
    Query(payload): Query<WithdrawalRequest>,
) -> Result<Json<Vec<WithdrawalResponse>>, StatusCode> {
    let result: Vec<_> = storage::withdrawals_for_address(&pool, from, payload.limit)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .map(WithdrawalResponse::from)
        .collect();
    Ok(Json(result))
}
