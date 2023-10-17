use clap::Parser;
use client::withdrawal_finalizer::codegen::FinalizeWithdrawalsCall;
use ethers::abi::AbiEncode;
use sqlx::postgres::PgPool;

#[derive(Parser, Debug)]
struct Args {
    /// id of withdrawal
    #[arg(short, long)]
    withdrawal_id: u64,

    /// database url
    #[arg(short, long)]
    database_url: String,

    /// gas
    #[arg(short, long)]
    gas: u64,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    println!(
        "building finalization calldata for withdrawal with id {}",
        args.withdrawal_id
    );

    let pool = PgPool::connect(&args.database_url).await.unwrap();

    let request_finalize_withdrawal =
        storage::get_finalize_withdrawal_params(&pool, args.withdrawal_id, args.gas)
            .await
            .unwrap()
            .unwrap();

    let finalize_withdrawal_call = FinalizeWithdrawalsCall {
        requests: vec![request_finalize_withdrawal],
    };

    let encoded = finalize_withdrawal_call.encode();

    println!("hex payload is\n{}", hex::encode(&encoded));
}
