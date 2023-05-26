#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Finalizer storage operations.

use sqlx::{Connection, PgConnection};

use client::WithdrawalEvent;

mod error;
mod utils;

use utils::u256_to_big_decimal;

pub use error::{Error, Result};

/// A new batch with a given range has been committed, update statuses of withdrawal records.
pub async fn committed_new_batch(
    conn: &mut PgConnection,
    batch_start: u64,
    batch_end: u64,
    l1_block_number: u64,
    l2_batch_number: u64,
) -> Result<()> {
    let mut tx = conn.begin().await?;
    sqlx::query!(
        "
        UPDATE withdrawals
        SET committed_in_block=$1
        WHERE block_number >= $2 AND block_number <= $3
        ",
        l1_block_number as i64,
        batch_start as i64,
        batch_end as i64,
    )
    .execute(&mut tx)
    .await?;

    sqlx::query!(
        "
        INSERT INTO committed_l1_events
        (
            l1_block_number,
            l1_batch_number,
            l2_range_begin,
            l2_range_end
        )
        VALUES ( $1, $2, $3, $4 )
        ON CONFLICT (l1_block_number) DO NOTHING
        ",
        l1_block_number as i64,
        l2_batch_number as i64,
        batch_start as i64,
        batch_end as i64,
    )
    .execute(&mut tx)
    .await?;

    tx.commit().await?;

    Ok(())
}

/// A new batch with a given range has been verified, update statuses of withdrawal records.
pub async fn verified_new_batch(
    conn: &mut PgConnection,
    batch_start: u64,
    batch_end: u64,
    l1_block_number: u64,
    l2_prev_batch_number: u64,
    l2_batch_number: u64,
) -> Result<()> {
    let mut tx = conn.begin().await?;

    sqlx::query!(
        "
        UPDATE withdrawals
        SET verified_in_block=$1
        WHERE block_number >= $2 AND block_number <= $3
        ",
        l1_block_number as i64,
        batch_start as i64,
        batch_end as i64,
    )
    .execute(&mut tx)
    .await?;

    sqlx::query!(
        "
        INSERT INTO verified_l1_events
        (
            l1_block_number,
            l2_previous_last_verified_block,
            l2_current_last_verified_block,
            l2_range_begin,
            l2_range_end
        )
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (l1_block_number) DO NOTHING
        ",
        l1_block_number as i64,
        l2_prev_batch_number as i64,
        l2_batch_number as i64,
        batch_start as i64,
        batch_end as i64,
    )
    .execute(&mut tx)
    .await?;

    tx.commit().await?;

    Ok(())
}

/// A new batch with a given range has been executed, update statuses of withdrawal records.
pub async fn executed_new_batch(
    conn: &mut PgConnection,
    batch_start: u64,
    batch_end: u64,
    l1_block_number: u64,
    l2_batch_number: u64,
) -> Result<()> {
    let mut tx = conn.begin().await?;
    sqlx::query!(
        "
        UPDATE withdrawals
        SET executed_in_block=$1
        WHERE block_number >= $2 AND block_number <= $3
        ",
        l1_block_number as i64,
        batch_start as i64,
        batch_end as i64,
    )
    .execute(&mut tx)
    .await?;

    sqlx::query!(
        "
        INSERT INTO executed_l1_events
        (
            l1_block_number,
            l1_batch_number,
            l2_range_begin,
            l2_range_end
        )
        VALUES ( $1, $2, $3, $4 )
        ON CONFLICT (l1_block_number) DO NOTHING
        ",
        l1_block_number as i64,
        l2_batch_number as i64,
        batch_start as i64,
        batch_end as i64,
    )
    .execute(&mut tx)
    .await?;

    tx.commit().await?;

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
            block_number,
            token,
            amount,
            event_index_in_tx,
            committed_in_block,
            verified_in_block,
            executed_in_block
        )
        VALUES (
            $1, $2, $3, $4, $5,
            (
                SELECT l1_block_number
                FROM committed_l1_events
                WHERE l2_range_begin <= $2 AND l2_range_end >= $2
            ),
            (
                SELECT l1_block_number
                FROM verified_l1_events
                WHERE l2_range_begin <= $2 AND l2_range_end >= $2
            ),
            (
                SELECT l1_block_number
                FROM executed_l1_events
                WHERE l2_range_begin <= $2 AND l2_range_end >= $2
            )
        )
        ON CONFLICT (tx_hash, event_index_in_tx) DO NOTHING
        ",
        event.tx_hash.0.to_vec(),
        event.block_number as i32,
        event.token.0.to_vec(),
        amount,
        event_index_in_tx as i32,
    )
    .execute(conn)
    .await?;

    Ok(())
}

/// Get the block number of the last L2 withdrawal the DB has record of.
pub async fn last_block_processed(conn: &mut PgConnection) -> Result<Option<u64>> {
    let res = sqlx::query!(
        "
        SELECT MAX(block_number)
        FROM withdrawals
        "
    )
    .fetch_one(conn)
    .await?
    .max
    .map(|max| max as u64);

    Ok(res)
}
