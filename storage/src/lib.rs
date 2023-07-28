#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Finalizer watcher.storage.operations.

use std::time::Instant;

use ethers::types::{Address, H160, H256};
use sqlx::{Connection, PgConnection, PgPool};

use chain_events::L2TokenInitEvent;
use client::{zksync_contract::L2ToL1Event, WithdrawalEvent, WithdrawalKey, WithdrawalParams};

mod error;
mod utils;

use utils::u256_to_big_decimal;

pub use error::{Error, Result};

/// A convenience struct that couples together [`WithdrawalEvent`]
/// with index in tx and boolean is_finalized value
#[derive(Debug)]
pub struct StoredWithdrawal {
    /// Withdrawal event
    pub event: WithdrawalEvent,

    /// Index of this event within the transaction
    pub index_in_tx: usize,
}

/// A new batch with a given range has been committed, update statuses of withdrawal records.
pub async fn committed_new_batch(
    conn: &mut PgConnection,
    batch_start: u64,
    batch_end: u64,
    l1_block_number: u64,
) -> Result<()> {
    let mut tx = conn.begin().await?;
    let range: Vec<_> = (batch_start as i64..=batch_end as i64).collect();
    let started_at = Instant::now();

    sqlx::query!(
        "
        INSERT INTO
          l2_blocks (
            l2_block_number,
            commit_l1_block_number
          )
        SELECT
          u.l2_block_number,
          $2
        FROM
          UNNEST ($1 :: bigint []) AS u(l2_block_number) ON CONFLICT (l2_block_number) DO
        UPDATE
        SET
          commit_l1_block_number = $2
        ",
        &range,
        l1_block_number as i64,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    metrics::histogram!(
        "watcher.storage.transactions.commited_new_batch",
        started_at.elapsed()
    );

    Ok(())
}

/// Request the number of L1 block this withdrawal was commited in.
pub async fn withdrawal_committed_in_block(
    conn: &mut PgConnection,
    tx_hash: H256,
) -> Result<Option<i64>> {
    let started_at = Instant::now();

    let res = sqlx::query!(
        "
        SELECT
          l2_blocks.commit_l1_block_number
        FROM
          withdrawals
          JOIN l2_blocks ON l2_blocks.l2_block_number = withdrawals.l2_block_number
        WHERE
          withdrawals.tx_hash = $1
        ",
        tx_hash.as_bytes(),
    )
    .fetch_optional(conn)
    .await?
    .and_then(|r| r.commit_l1_block_number);

    metrics::histogram!("watcher.storage.request", started_at.elapsed(), "method" => "withdrawal_committed_in_block");

    Ok(res)
}

/// Request the number of L1 block this withdrawal was verified in.
pub async fn withdrawal_verified_in_block(
    conn: &mut PgConnection,
    tx_hash: H256,
) -> Result<Option<i64>> {
    let started_at = Instant::now();

    let res = sqlx::query!(
        "
        SELECT
          l2_blocks.verify_l1_block_number
        FROM
          withdrawals
          JOIN l2_blocks ON l2_blocks.l2_block_number = withdrawals.l2_block_number
        WHERE
          withdrawals.tx_hash = $1
        ",
        tx_hash.as_bytes(),
    )
    .fetch_optional(conn)
    .await?
    .and_then(|r| r.verify_l1_block_number);

    metrics::histogram!("watcher.storage.request", started_at.elapsed(), "method" => "withdrawal_verified_in_block");

    Ok(res)
}

/// Request the number of L1 block this withdrawal was executed in.
pub async fn withdrawal_executed_in_block(
    conn: &mut PgConnection,
    tx_hash: H256,
) -> Result<Option<i64>> {
    let started_at = Instant::now();

    let res = sqlx::query!(
        "
        SELECT
          l2_blocks.execute_l1_block_number
        FROM
          withdrawals
          JOIN l2_blocks ON l2_blocks.l2_block_number = withdrawals.l2_block_number
        WHERE
          withdrawals.tx_hash = $1
        ",
        tx_hash.as_bytes(),
    )
    .fetch_optional(conn)
    .await?
    .and_then(|r| r.execute_l1_block_number);

    metrics::histogram!("watcher.storage.request", started_at.elapsed(), "method" => "withdrawal_executed_in_block");

    Ok(res)
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

    let started_at = Instant::now();

    sqlx::query!(
        "
        INSERT INTO
          l2_blocks (
            l2_block_number,
            verify_l1_block_number
          )
        SELECT
          u.l2_block_number,
          $2
        FROM
          UNNEST ($1 :: bigint []) AS u(l2_block_number) ON CONFLICT (l2_block_number) DO
        UPDATE
        SET
          verify_l1_block_number = $2
        ",
        &range,
        l1_block_number as i64,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    metrics::histogram!(
        "watcher.storage.transactions.verified_new_batch",
        started_at.elapsed()
    );

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
    let started_at = Instant::now();

    sqlx::query!(
        "
        INSERT INTO
          l2_blocks (
            l2_block_number,
            execute_l1_block_number
          )
        SELECT
          u.l2_block_number,
          $2
        FROM
          UNNEST ($1 :: bigint []) AS u(l2_block_number) ON CONFLICT (l2_block_number) DO
        UPDATE
        SET
          execute_l1_block_number = $2
        ",
        &range,
        l1_block_number as i64,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    metrics::histogram!(
        "watcher.storage.transactions.executed_new_batch",
        started_at.elapsed(),
    );

    Ok(())
}

/// Adds a withdrawal event to the DB.
///
/// # Arguments
///
/// * `conn`: Connection to the Postgres DB
/// * `events`: Withdrawal events grouped with their indices in transcation.
pub async fn add_withdrawals(conn: &mut PgConnection, events: &[StoredWithdrawal]) -> Result<()> {
    let mut tx_hashes = Vec::with_capacity(events.len());
    let mut block_numbers = Vec::with_capacity(events.len());
    let mut tokens = Vec::with_capacity(events.len());
    let mut amounts = Vec::with_capacity(events.len());
    let mut indices_in_tx = Vec::with_capacity(events.len());

    events.iter().for_each(|sw| {
        tx_hashes.push(sw.event.tx_hash.0.to_vec());
        block_numbers.push(sw.event.block_number as i64);
        tokens.push(sw.event.token.0.to_vec());
        amounts.push(u256_to_big_decimal(sw.event.amount));
        indices_in_tx.push(sw.index_in_tx as i32);
    });

    let started_at = Instant::now();

    sqlx::query!(
        "
        INSERT INTO
          withdrawals (
            tx_hash,
            l2_block_number,
            token,
            amount,
            event_index_in_tx
          )
        SELECT
          u.tx_hash,
          u.l2_block_number,
          u.token,
          u.amount,
          u.index_in_tx
        FROM
          unnest(
            $1 :: BYTEA [],
            $2 :: bigint [],
            $3 :: BYTEA [],
            $4 :: numeric [],
            $5 :: integer []
          ) AS u(
            tx_hash,
            l2_block_number,
            token,
            amount,
            index_in_tx
          ) ON CONFLICT (
            tx_hash,
            event_index_in_tx
          ) DO NOTHING
        ",
        &tx_hashes,
        &block_numbers,
        &tokens,
        &amounts,
        &indices_in_tx,
    )
    .execute(conn)
    .await?;

    metrics::histogram!(
        "watcher.storage.transactions.add_withdrawals",
        started_at.elapsed()
    );

    Ok(())
}

/// Get the block number of the last L2 withdrawal the DB has record of.
pub async fn last_l2_block_seen(conn: &mut PgConnection) -> Result<Option<u64>> {
    let started_at = Instant::now();

    let res = sqlx::query!(
        "
        SELECT
          max(l2_block_number)
        FROM
          withdrawals
        "
    )
    .fetch_one(conn)
    .await?
    .max
    .map(|max| max as u64);

    metrics::histogram!("watcher.storage.request", started_at.elapsed(), "method" => "last_l2_block_seen");

    Ok(res)
}

/// Get the block number of the last L1 block seen.
pub async fn last_l1_block_seen(conn: &mut PgConnection) -> Result<Option<u64>> {
    let started_at = Instant::now();

    let res = sqlx::query!(
        "
        SELECT
          max(commit_l1_block_number)
        FROM
          l2_blocks
        "
    )
    .fetch_one(conn)
    .await?
    .max
    .map(|max| max as u64);

    metrics::histogram!("watcher.storage.request", started_at.elapsed(), "method" => "last_l1_block_seen");

    Ok(res)
}

/// Get the last block seen for the l2_to_l1_events set
pub async fn last_l2_to_l1_events_block_seen(conn: &mut PgConnection) -> Result<Option<u64>> {
    let started_at = Instant::now();

    let res = sqlx::query!(
        "
        SELECT
          max(l1_block_number)
        FROM
          l2_to_l1_events
        "
    )
    .fetch_one(conn)
    .await?
    .max
    .map(|max| max as u64);

    metrics::histogram!(
        "watcher.storage.request",
        started_at.elapsed(),
        "method" => "last_l2_to_l1_events_block_seen",
    );

    Ok(res)
}

/// Adds a `L2ToL1Event` set to the DB.
///
/// # Arguments
///
/// * `conn`: Connection to the Postgres DB
/// * `events`: The `L2ToL1Event`s
pub async fn l2_to_l1_events(conn: &mut PgConnection, events: &[L2ToL1Event]) -> Result<()> {
    let mut l1_token_addrs = Vec::with_capacity(events.len());
    let mut to_addrs = Vec::with_capacity(events.len());
    let mut amounts = Vec::with_capacity(events.len());
    let mut l1_block_numbers = Vec::with_capacity(events.len());
    let mut l2_block_numbers = Vec::with_capacity(events.len());
    let mut tx_numbers_in_block = Vec::with_capacity(events.len());

    events.iter().for_each(|e| {
        l1_token_addrs.push(e.token.0.to_vec());
        to_addrs.push(e.to.0.to_vec());
        amounts.push(u256_to_big_decimal(e.amount));
        l1_block_numbers.push(e.l1_block_number as i64);
        l2_block_numbers.push(e.l2_block_number as i64);
        tx_numbers_in_block.push(e.tx_number_in_block as i32);
    });

    let started_at = Instant::now();

    sqlx::query!(
        "
        INSERT INTO
          l2_to_l1_events (
            l1_token_addr,
            to_address,
            amount,
            l1_block_number,
            l2_block_number,
            tx_number_in_block
          )
        SELECT
          u.l1_token_addr,
          u.to_address,
          u.amount,
          u.l1_block_number,
          u.l2_block_number,
          u.tx_number_in_block
        FROM
          unnest(
            $1 :: BYTEA [],
            $2 :: BYTEA [],
            $3 :: numeric [],
            $4 :: bigint [],
            $5 :: bigint [],
            $6 :: integer []
          ) AS u(
            l1_token_addr,
            to_address,
            amount,
            l1_block_number,
            l2_block_number,
            tx_number_in_block
          ) ON CONFLICT (
            l1_block_number,
            l2_block_number,
            tx_number_in_block
          ) DO NOTHING
        ",
        &l1_token_addrs,
        &to_addrs,
        &amounts,
        &l1_block_numbers,
        &l2_block_numbers,
        &tx_numbers_in_block,
    )
    .execute(conn)
    .await?;

    metrics::histogram!(
        "watcher.storage.transactions.add_l2_to_l1_event",
        started_at.elapsed()
    );
    Ok(())
}

/// Get addresses of known tokens on L2 + last seen block.
pub async fn get_tokens(pool: &PgPool) -> Result<(Vec<Address>, u64)> {
    let last_l2_block_seen = sqlx::query!(
        "
        SELECT
          max(l2_block_number)
        FROM
          tokens
        ",
    )
    .fetch_one(pool)
    .await?
    .max
    .unwrap_or(1);

    let tokens = sqlx::query!(
        "
        SELECT
          l2_token_address
        FROM
          tokens
        "
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| H160::from_slice(&r.l2_token_address))
    .collect();

    Ok((tokens, last_l2_block_seen as u64))
}

/// Insert a token initalization event into the DB.
pub async fn add_token(pool: &PgPool, token: &L2TokenInitEvent) -> Result<()> {
    sqlx::query!(
        "
        INSERT INTO
          tokens (
            l1_token_address,
            l2_token_address,
            name,
            symbol,
            decimals,
            l2_block_number,
            initialization_transaction
          )
        VALUES
          ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (l1_token_address, l2_token_address) DO NOTHING
        ",
        token.l1_token_address.0.to_vec(),
        token.l2_token_address.0.to_vec(),
        token.name,
        token.symbol,
        token.decimals as i64,
        token.l2_block_number as i64,
        token.initialization_transaction.0.to_vec(),
    )
    .execute(pool)
    .await?;

    Ok(())
}

#[allow(missing_docs)]
#[derive(Debug)]
pub struct WithdrawalWithBlock {
    pub key: WithdrawalKey,
    pub id: u64,
    pub l2_block_number: u64,
}

/// Adds withdrawal information to the `finalization_data` table.
pub async fn add_withdrawals_data(pool: &PgPool, wd: &[WithdrawalParams]) -> Result<()> {
    let mut ids = Vec::with_capacity(wd.len());
    let mut l2_block_number = Vec::with_capacity(wd.len());
    let mut l1_batch_number = Vec::with_capacity(wd.len());
    let mut l2_message_index = Vec::with_capacity(wd.len());
    let mut l2_tx_number_in_block = Vec::with_capacity(wd.len());
    let mut message = Vec::with_capacity(wd.len());
    let mut sender = Vec::with_capacity(wd.len());
    let mut proof = Vec::with_capacity(wd.len());

    wd.iter().for_each(|d| {
        ids.push(d.id as i64);
        l2_block_number.push(d.l2_block_number as i64);
        l1_batch_number.push(d.l1_batch_number.as_u64() as i64);
        l2_message_index.push(d.l2_message_index as i32);
        l2_tx_number_in_block.push(d.l2_tx_number_in_block as i32);
        message.push(d.message.to_vec());
        sender.push(d.sender.0.to_vec());
        proof.push(bincode::serialize(&d.proof).unwrap());
    });

    let started_at = Instant::now();

    sqlx::query!(
        "
        INSERT INTO
          finalization_data (
            withdrawal_id,
            l2_block_number,
            l1_batch_number,
            l2_message_index,
            l2_tx_number_in_block,
            message,
            sender,
            proof
          )
        SELECT
          u.id,
          u.l2_block_number,
          u.l1_batch_number,
          u.l2_message_index,
          u.l2_tx_number_in_block,
          u.message,
          u.sender,
          u.proof
        FROM
          UNNEST (
            $1 :: bigint [],
            $2 :: bigint [],
            $3 :: bigint [],
            $4 :: integer [],
            $5 :: integer [],
            $6 :: BYTEA [],
            $7 :: BYTEA [],
            $8 :: BYTEA []
          ) AS u(
            id,
            l2_block_number,
            l1_batch_number,
            l2_message_index,
            l2_tx_number_in_block,
            message,
            sender,
            proof
          ) ON CONFLICT (withdrawal_id) DO NOTHING
        ",
        &ids,
        &l2_block_number,
        &l1_batch_number,
        &l2_message_index,
        &l2_tx_number_in_block,
        &message,
        &sender,
        &proof
    )
    .execute(pool)
    .await?;

    metrics::histogram!(
        "watcher.storage.transactions.add_withdrawal_data",
        started_at.elapsed(),
    );

    Ok(())
}

/// Returns all previously unseen executed events after a given block
pub async fn get_withdrawals_with_no_data(
    pool: &PgPool,
    limit_by: u64,
) -> Result<Vec<WithdrawalWithBlock>> {
    let withdrawals = sqlx::query!(
        "
        WITH max_committed AS (
          SELECT
            max(l2_block_number)
          FROM
            l2_blocks
          WHERE
            commit_l1_block_number IS NOT NULL
        ),
        max_seen AS (
          SELECT
            max(withdrawal_id)
          FROM
            finalization_data
        )
        SELECT
          tx_hash,
          event_index_in_tx,
          id,
          l2_block_number
        FROM
          withdrawals,
          max_committed,
          max_seen
        WHERE
          id >= coalesce(max_seen.max, 1)
          AND l2_block_number <= max_committed.max
        ORDER BY
          l2_block_number
        LIMIT
          $1
        ",
        limit_by as i64,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| WithdrawalWithBlock {
        key: WithdrawalKey {
            tx_hash: H256::from_slice(&r.tx_hash),
            event_index_in_tx: r.event_index_in_tx as u32,
        },
        id: r.id as u64,
        l2_block_number: r.l2_block_number as u64,
    })
    .collect();

    Ok(withdrawals)
}

/// Get the earliest withdrawals never attempted to be finalized before
pub async fn withdrwals_to_finalize(pool: &PgPool, limit_by: u64) -> Result<Vec<WithdrawalParams>> {
    let data = sqlx::query!(
        "
        SELECT
          w.tx_hash,
          w.event_index_in_tx,
          withdrawal_id,
          finalization_data.l2_block_number,
          l1_batch_number,
          l2_message_index,
          l2_tx_number_in_block,
          message,
          sender,
          proof
        FROM
          finalization_data
          JOIN withdrawals w ON finalization_data.withdrawal_id = w.id
        WHERE
          finalization_tx IS NULL
          AND failed_finalization_attempts < 3
        ORDER BY
          finalization_data.l2_block_number
        LIMIT
          $1
        ",
        limit_by as i64,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|record| WithdrawalParams {
        tx_hash: H256::from_slice(&record.tx_hash),
        event_index_in_tx: record.event_index_in_tx as u32,
        id: record.withdrawal_id as u64,
        l2_block_number: record.l2_block_number as u64,
        l1_batch_number: record.l1_batch_number.into(),
        l2_message_index: record.l2_message_index as u32,
        l2_tx_number_in_block: record.l2_tx_number_in_block as u16,
        message: record.message.into(),
        sender: Address::from_slice(&record.sender),
        proof: bincode::deserialize(&record.proof)
            .expect("storage contains data correctly serialized by bincode; qed"),
    })
    .collect();

    Ok(data)
}

/// Set status of a set of withdrawals in `finalization_data` to finalized
pub async fn finalization_data_set_finalized_in_tx(
    pool: &PgPool,
    withdrawals: &[WithdrawalKey],
    tx_hash: H256,
) -> Result<()> {
    let mut tx_hashes = Vec::with_capacity(withdrawals.len());
    let mut event_index_in_tx = Vec::with_capacity(withdrawals.len());

    withdrawals.iter().for_each(|w| {
        tx_hashes.push(w.tx_hash.0.to_vec());
        event_index_in_tx.push(w.event_index_in_tx as i32);
    });

    sqlx::query!(
        "
        UPDATE
          finalization_data
        SET
          finalization_tx = $1
        FROM
          (
            SELECT
              UNNEST ($2 :: BYTEA []) AS tx_hash,
              UNNEST ($3 :: integer []) AS event_index_in_tx
          ) AS u
        WHERE
          finalization_data.withdrawal_id = (
            SELECT
              id
            FROM
              withdrawals
            WHERE
              tx_hash = u.tx_hash
              AND event_index_in_tx = u.event_index_in_tx
          )
        ",
        &tx_hash.0.as_ref(),
        &tx_hashes,
        &event_index_in_tx,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Increment unsuccessful transaction attempt count for a set
/// of withdrawals
pub async fn inc_unsuccessful_finalization_attempts(
    pool: &PgPool,
    withdrawals: &[WithdrawalKey],
) -> Result<()> {
    let mut tx_hashes = Vec::with_capacity(withdrawals.len());
    let mut event_index_in_tx = Vec::with_capacity(withdrawals.len());

    withdrawals.iter().for_each(|w| {
        tx_hashes.push(w.tx_hash.0.to_vec());
        event_index_in_tx.push(w.event_index_in_tx as i32);
    });

    sqlx::query!(
        "
        UPDATE
          finalization_data
        SET
          failed_finalization_attempts = failed_finalization_attempts + 1
        FROM
          (
            SELECT
              UNNEST ($1 :: BYTEA []) AS tx_hash,
              UNNEST ($2 :: integer []) AS event_index_in_tx
          ) AS u
        WHERE
          finalization_data.withdrawal_id = (
            SELECT
              id
            FROM
              withdrawals
            WHERE
              tx_hash = u.tx_hash
              AND event_index_in_tx = u.event_index_in_tx
          )
        ",
        &tx_hashes,
        &event_index_in_tx,
    )
    .execute(pool)
    .await?;

    Ok(())
}
