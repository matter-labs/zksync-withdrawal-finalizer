#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Finalizer storage operations.

use ethers::types::H256;
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
) -> Result<()> {
    let mut tx = conn.begin().await?;
    let range: Vec<_> = (batch_start as i64..=batch_end as i64).collect();

    sqlx::query!(
        "
        INSERT INTO l2_blocks (l2_block_number, commit_l1_block_number)
        SELECT u.l2_block_number,$2
        FROM UNNEST ($1::bigint[])
            AS u(l2_block_number)
        ON CONFLICT (l2_block_number) DO
        UPDATE SET commit_l1_block_number = $2
        ",
        &range,
        l1_block_number as i64,
    )
    .execute(&mut tx)
    .await?;

    tx.commit().await?;

    Ok(())
}

/// Request the number of L1 block this withdrawal was commited in.
pub async fn withdrawal_committed_in_block(
    conn: &mut PgConnection,
    transaction: H256,
) -> Result<Option<i64>> {
    Ok(sqlx::query!(
        "
        SELECT l2_blocks.commit_l1_block_number from
        withdrawals JOIN l2_blocks ON
        l2_blocks.l2_block_number = withdrawals.block_number
        WHERE withdrawals.tx_hash = $1
        ",
        transaction.as_bytes(),
    )
    .fetch_optional(conn)
    .await?
    .and_then(|r| r.commit_l1_block_number))
}

/// Request the number of L1 block this withdrawal was verified in.
pub async fn withdrawal_verified_in_block(
    conn: &mut PgConnection,
    transaction: H256,
) -> Result<Option<i64>> {
    Ok(sqlx::query!(
        "
        SELECT l2_blocks.verify_l1_block_number from
        withdrawals JOIN l2_blocks ON
        l2_blocks.l2_block_number = withdrawals.block_number
        WHERE withdrawals.tx_hash = $1
        ",
        transaction.as_bytes(),
    )
    .fetch_optional(conn)
    .await?
    .and_then(|r| r.verify_l1_block_number))
}

/// Request the number of L1 block this withdrawal was executed in.
pub async fn withdrawal_executed_in_block(
    conn: &mut PgConnection,
    transaction: H256,
) -> Result<Option<i64>> {
    Ok(sqlx::query!(
        "
        SELECT l2_blocks.execute_l1_block_number from
        withdrawals JOIN l2_blocks ON
        l2_blocks.l2_block_number = withdrawals.block_number
        WHERE withdrawals.tx_hash = $1
        ",
        transaction.as_bytes(),
    )
    .fetch_optional(conn)
    .await?
    .and_then(|r| r.execute_l1_block_number))
}
/// A new batch with a given range has been verified, update statuses of withdrawal records.
pub async fn verified_new_batch(
    conn: &mut PgConnection,
    batch_start: u64,
    batch_end: u64,
    l1_block_number: u64,
) -> Result<()> {
    let mut tx = conn.begin().await?;
    let range: Vec<_> = (batch_start as i64..=batch_end as i64).collect();

    sqlx::query!(
        "
        INSERT INTO l2_blocks (l2_block_number, verify_l1_block_number)
        SELECT u.l2_block_number,$2
        FROM UNNEST ($1::bigint[])
            AS u(l2_block_number)
        ON CONFLICT (l2_block_number) DO
        UPDATE SET verify_l1_block_number = $2
        ",
        &range,
        l1_block_number as i64,
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
) -> Result<()> {
    let mut tx = conn.begin().await?;
    let range: Vec<_> = (batch_start as i64..=batch_end as i64).collect();

    sqlx::query!(
        "
        INSERT INTO l2_blocks (l2_block_number, execute_l1_block_number)
        SELECT u.l2_block_number,$2
        FROM UNNEST ($1::bigint[])
            AS u(l2_block_number)
        ON CONFLICT (l2_block_number) DO
        UPDATE SET execute_l1_block_number = $2
        ",
        &range,
        l1_block_number as i64,
    )
    .execute(&mut tx)
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
            block_number,
            token,
            amount,
            event_index_in_tx
        )
        VALUES (
            $1, $2, $3, $4, $5
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
