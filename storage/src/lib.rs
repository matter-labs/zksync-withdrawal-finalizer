#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Finalizer storage operations.

use sqlx::PgConnection;

use client::WithdrawalEvent;

mod error;
mod utils;

use utils::u256_to_big_decimal;

pub use error::{Error, Result};

#[derive(Debug, sqlx::Type)]
#[sqlx(type_name = "withdrawal_status")]
enum WithdrawalStatus {
    Seen,
    Finalized,
}

/// Adds a withdrawal event to the DB.
///
/// # Arguments
///
/// * `conn`: Connection to the Postgres DB
/// * `event`: Withdrawal event itself
/// * `event_index_in_tx`: Index of the given event in the transaction
pub async fn add_withdrawal(
    conn: &mut PgConnection,
    event: &WithdrawalEvent,
    event_index_in_tx: usize,
) -> Result<()> {
    let amount = u256_to_big_decimal(event.amount);

    sqlx::query!(
        "
        INSERT INTO withdrawals
        (
            tx_hash,
            blocknumber,
            token,
            amount,
            event_index_in_tx,
            status
        )
        VALUES (
            $1, $2, $3, $4, $5, $6
        )
        ",
        event.tx_hash.0.to_vec(),
        event.block_number as i32,
        event.token.0.to_vec(),
        amount,
        event_index_in_tx as i32,
        WithdrawalStatus::Seen as WithdrawalStatus,
    )
    .fetch_optional(conn)
    .await?;

    Ok(())
}
