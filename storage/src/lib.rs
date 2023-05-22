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
    Committed,
    Verified,
    Executed,
    Finalized,
}

/// A new batch with a given range has been committed, update statuses of withdrawal records.
pub async fn commited_new_batch(
    conn: &mut PgConnection,
    batch_start: u64,
    batch_end: u64,
) -> Result<()> {
    update_status_for_block_range(
        conn,
        batch_start,
        batch_end,
        WithdrawalStatus::Seen,
        WithdrawalStatus::Committed,
    )
    .await
}

/// A new batch with a given range has been verified, update statuses of withdrawal records.
pub async fn verified_new_batch(
    conn: &mut PgConnection,
    batch_start: u64,
    batch_end: u64,
) -> Result<()> {
    update_status_for_block_range(
        conn,
        batch_start,
        batch_end,
        WithdrawalStatus::Committed,
        WithdrawalStatus::Verified,
    )
    .await
}

/// A new batch with a given range has been executed, update statuses of withdrawal records.
pub async fn executed_new_batch(
    conn: &mut PgConnection,
    batch_start: u64,
    batch_end: u64,
) -> Result<()> {
    update_status_for_block_range(
        conn,
        batch_start,
        batch_end,
        WithdrawalStatus::Verified,
        WithdrawalStatus::Executed,
    )
    .await
}

async fn update_status_for_block_range(
    conn: &mut PgConnection,
    _batch_start: u64,
    batch_end: u64,
    prev_status: WithdrawalStatus,
    status: WithdrawalStatus,
) -> Result<()> {
    sqlx::query!(
        "
        UPDATE withdrawals
        SET status=$1
        WHERE blocknumber <= $2
        AND status = $3
        ",
        status as WithdrawalStatus,
        batch_end as i64,
        prev_status as WithdrawalStatus,
    )
    .fetch_optional(conn)
    .await?;

    Ok(())
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
        ON CONFLICT (tx_hash, blocknumber, event_index_in_tx) DO NOTHING
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
