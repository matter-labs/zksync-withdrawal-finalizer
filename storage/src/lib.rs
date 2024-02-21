#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![warn(unused_imports)]

//! Finalizer watcher.storage.operations.

use ethers::types::{Address, H160, H256, U256};
use sqlx::{PgConnection, PgPool};

use chain_events::L2TokenInitEvent;
use client::{
    is_eth, withdrawal_finalizer::codegen::RequestFinalizeWithdrawal, zksync_contract::L2ToL1Event,
    WithdrawalEvent, WithdrawalKey, WithdrawalParams,
};

mod error;
mod metrics;
mod utils;

use utils::u256_to_big_decimal;

pub use error::{Error, Result};

use crate::metrics::STORAGE_METRICS;

/// A convenience struct that couples together [`WithdrawalEvent`]
/// with index in tx and boolean `is_finalized` value
#[derive(Debug)]
pub struct StoredWithdrawal {
    /// Withdrawal event
    pub event: WithdrawalEvent,

    /// Index of this event within the transaction
    pub index_in_tx: usize,
}

/// A new batch with a given range has been committed, update statuses of withdrawal records.
pub async fn committed_new_batch(
    pool: &PgPool,
    batch_start: u64,
    batch_end: u64,
    l1_block_number: u64,
) -> Result<()> {
    let mut tx = pool.begin().await?;

    let latency = STORAGE_METRICS.call[&"committed_new_batch"].start();

    let range: Vec<_> = (batch_start as i64..=batch_end as i64).collect();

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

    latency.observe();

    Ok(())
}

/// Request the number of L1 block this withdrawal was committed in.
pub async fn withdrawal_committed_in_block(
    conn: &mut PgConnection,
    tx_hash: H256,
) -> Result<Option<i64>> {
    let latency = STORAGE_METRICS.call[&"withdrawal_committed_in_block"].start();

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

    latency.observe();

    Ok(res)
}

/// Request the number of L1 block this withdrawal was verified in.
pub async fn withdrawal_verified_in_block(
    conn: &mut PgConnection,
    tx_hash: H256,
) -> Result<Option<i64>> {
    let latency = STORAGE_METRICS.call[&"withdrawal_verified_in_block"].start();

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

    latency.observe();

    Ok(res)
}

/// Request the number of L1 block this withdrawal was executed in.
pub async fn withdrawal_executed_in_block(
    conn: &mut PgConnection,
    tx_hash: H256,
) -> Result<Option<i64>> {
    let latency = STORAGE_METRICS.call[&"withdrawal_executed_in_block"].start();
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

    latency.observe();

    Ok(res)
}
/// A new batch with a given range has been verified, update statuses of withdrawal records.
pub async fn verified_new_batch(
    pool: &PgPool,
    batch_start: u64,
    batch_end: u64,
    l1_block_number: u64,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    let range: Vec<_> = (batch_start as i64..=batch_end as i64).collect();

    let latency = STORAGE_METRICS.call[&"verified_new_batch"].start();
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
    latency.observe();

    Ok(())
}

/// A new batch with a given range has been executed, update statuses of withdrawal records.
pub async fn executed_new_batch(
    pool: &PgPool,
    batch_start: u64,
    batch_end: u64,
    l1_block_number: u64,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    let range: Vec<_> = (batch_start as i64..=batch_end as i64).collect();
    let latency = STORAGE_METRICS.call[&"executed_new_batch"].start();

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
    latency.observe();

    Ok(())
}

/// Gets withdrawal events from the db by a set of IDs.
///
/// # Arguments
///
/// * `conn`: Connection to the Postgres DB
/// * `ids`: ID fields of the withdrawals to be returned.
pub async fn get_withdrawals(pool: &PgPool, ids: &[i64]) -> Result<Vec<StoredWithdrawal>> {
    let latency = STORAGE_METRICS.call[&"get_withdrawals"].start();

    let events = sqlx::query!(
        "
        SELECT * FROM
            withdrawals
        WHERE id in (SELECT * FROM unnest( $1 :: bigint[] ))
        ",
        ids
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| StoredWithdrawal {
        event: WithdrawalEvent {
            tx_hash: H256::from_slice(&r.tx_hash),
            block_number: r.l2_block_number as u64,
            token: Address::from_slice(&r.token),
            amount: utils::bigdecimal_to_u256(r.amount),
        },
        index_in_tx: r.event_index_in_tx as usize,
    })
    .collect();

    latency.observe();

    Ok(events)
}

/// Adds a withdrawal event to the DB.
///
/// # Arguments
///
/// * `conn`: Connection to the Postgres DB
/// * `events`: Withdrawal events grouped with their indices in transaction.
pub async fn add_withdrawals(pool: &PgPool, events: &[StoredWithdrawal]) -> Result<()> {
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

    let latency = STORAGE_METRICS.call[&"add_withdrawals"].start();

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
        amounts.as_slice(),
        &indices_in_tx,
    )
    .execute(pool)
    .await?;

    latency.observe();

    Ok(())
}

/// Get the block number of the last L2 withdrawal the DB has record of.
pub async fn last_l2_block_seen(conn: &mut PgConnection) -> Result<Option<u64>> {
    let latency = STORAGE_METRICS.call[&"last_l2_block_seen"].start();

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

    latency.observe();

    Ok(res)
}

/// Get the block number of the last L1 block seen.
pub async fn last_l1_block_seen(conn: &mut PgConnection) -> Result<Option<u64>> {
    let latency = STORAGE_METRICS.call[&"last_l1_block_seen"].start();

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

    latency.observe();

    Ok(res)
}

/// Get the last block seen for the `l2_to_l1_events` set
pub async fn last_l2_to_l1_events_block_seen(conn: &mut PgConnection) -> Result<Option<u64>> {
    let latency = STORAGE_METRICS.call[&"last_l2_to_l1_events_block_seen"].start();

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

    latency.observe();

    Ok(res)
}

/// Adds a `L2ToL1Event` set to the DB.
///
/// # Arguments
///
/// * `conn`: Connection to the Postgres DB
/// * `events`: The `L2ToL1Event`s
pub async fn l2_to_l1_events(pool: &PgPool, events: &[L2ToL1Event]) -> Result<()> {
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

    let latency = STORAGE_METRICS.call[&"l2_to_l1_events"].start();

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
    .execute(pool)
    .await?;

    latency.observe();

    Ok(())
}

/// Get addresses of known tokens on L2 and the last seen block.
pub async fn get_tokens(pool: &PgPool) -> Result<(Vec<Address>, u64)> {
    let latency = STORAGE_METRICS.call[&"get_tokens"].start();
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

    latency.observe();
    Ok((tokens, last_l2_block_seen as u64))
}

/// Insert a token initialization event into the DB.
pub async fn add_token(pool: &PgPool, token: &L2TokenInitEvent) -> Result<()> {
    let latency = STORAGE_METRICS.call[&"add_token"].start();

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

    latency.observe();

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

    let latency = STORAGE_METRICS.call[&"add_withdrawals_data"].start();

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

    latency.observe();

    Ok(())
}

/// Returns all previously unseen executed events after a given block
pub async fn get_withdrawals_with_no_data(
    pool: &PgPool,
    limit_by: u64,
) -> Result<Vec<WithdrawalWithBlock>> {
    let latency = STORAGE_METRICS.call[&"get_withdrawals_no_data"].start();

    let withdrawals = sqlx::query!(
        "
        SELECT
          tx_hash,
          event_index_in_tx,
          id,
          l2_block_number
        FROM
          withdrawals
        WHERE
          l2_block_number <= COALESCE(
            (
              SELECT
                MAX(l2_block_number)
              FROM
                l2_blocks
              WHERE
                commit_l1_block_number IS NOT NULL
            ),
            1
          )
          AND id > COALESCE(
            (
              SELECT
                MAX(withdrawal_id)
              FROM
                finalization_data
            ),
            1
          )
          AND finalizable = TRUE
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

    latency.observe();

    Ok(withdrawals)
}

/// Set a withdrawals as unfinalizable since we have failed to request parameters
pub async fn set_withdrawal_unfinalizable(
    pool: &PgPool,
    tx_hash: H256,
    event_index_in_tx: usize,
) -> Result<()> {
    let latency = STORAGE_METRICS.call[&"set_withdrawal_unfinalizable"].start();

    sqlx::query!(
        "
            UPDATE withdrawals
            SET finalizable = false 
            WHERE
              tx_hash = $1
              AND
              event_index_in_tx = $2
        ",
        tx_hash.as_bytes(),
        event_index_in_tx as i32,
    )
    .execute(pool)
    .await?;

    latency.observe();

    Ok(())
}

/// Get the earliest withdrawals never attempted to be finalized before
pub async fn withdrawals_to_finalize_with_blacklist(
    pool: &PgPool,
    limit_by: u64,
    token_blacklist: &[Address],
    eth_threshold: Option<U256>,
) -> Result<Vec<WithdrawalParams>> {
    let blacklist: Vec<_> = token_blacklist.iter().map(|a| a.0.to_vec()).collect();
    // if no threshold, query _all_ ethereum withdrawals since all of them are >= 0.
    let eth_threshold = eth_threshold.unwrap_or(U256::zero());

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
          AND finalization_data.l2_block_number <= COALESCE(
            (
              SELECT
                MAX(l2_block_number)
              FROM
                l2_blocks
              WHERE
                execute_l1_block_number IS NOT NULL
            ),
            1
          )
          AND w.token NOT IN (SELECT * FROM UNNEST (
            $2 :: BYTEA []
          ))
          AND (
            CASE WHEN token = decode('000000000000000000000000000000000000800A', 'hex') THEN amount >= $3
            ELSE TRUE
            END
          )
        LIMIT
          $1
        ",
        limit_by as i64,
        &blacklist,
        u256_to_big_decimal(eth_threshold),
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

/// Get the earliest withdrawals never attempted to be finalized before
pub async fn withdrawals_to_finalize_with_whitelist(
    pool: &PgPool,
    limit_by: u64,
    token_whitelist: &[Address],
    eth_threshold: Option<U256>,
) -> Result<Vec<WithdrawalParams>> {
    let whitelist: Vec<_> = token_whitelist.iter().map(|a| a.0.to_vec()).collect();
    // if no threshold, query _all_ ethereum withdrawals since all of them are >= 0.
    let eth_threshold = eth_threshold.unwrap_or(U256::zero());

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
          AND finalization_data.l2_block_number <= COALESCE(
            (
              SELECT
                MAX(l2_block_number)
              FROM
                l2_blocks
              WHERE
                execute_l1_block_number IS NOT NULL
            ),
            1
          )
          AND w.token IN (SELECT * FROM UNNEST (
            $2 :: BYTEA []
          ))
          AND (
            CASE WHEN token = decode('000000000000000000000000000000000000800A', 'hex') THEN amount >= $3
            ELSE TRUE
            END
          )
        LIMIT
          $1
        ",
        limit_by as i64,
        &whitelist,
        u256_to_big_decimal(eth_threshold),
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

/// Get the earliest withdrawals never attempted to be finalized before
pub async fn withdrawals_to_finalize(
    pool: &PgPool,
    limit_by: u64,
    eth_threshold: Option<U256>,
) -> Result<Vec<WithdrawalParams>> {
    let latency = STORAGE_METRICS.call[&"withdrawals_to_finalize"].start();
    // if no threshold, query _all_ ethereum withdrawals since all of them are >= 0.
    let eth_threshold = eth_threshold.unwrap_or(U256::zero());

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
          AND finalization_data.l2_block_number <= COALESCE(
            (
              SELECT
                MAX(l2_block_number)
              FROM
                l2_blocks
              WHERE
                execute_l1_block_number IS NOT NULL
            ),
            1
          )
          AND (
            last_finalization_attempt IS NULL
          OR
            last_finalization_attempt < NOW() - INTERVAL '1 minutes'
          )
          AND (
            CASE WHEN token = decode('000000000000000000000000000000000000800A', 'hex') THEN amount >= $2
            ELSE TRUE
            END
          )
        LIMIT
          $1
        ",
        limit_by as i64,
        u256_to_big_decimal(eth_threshold),
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

    latency.observe();

    Ok(data)
}

/// Get the number of ETH withdrawals not yet executed and finalized and above some threshold
pub async fn get_unexecuted_withdrawals_count(
    pool: &PgPool,
    eth_threshold: Option<U256>,
) -> Result<i64> {
    // if no threshold, query _all_ ethereum withdrawals since all of them are >= 0.
    let eth_threshold = eth_threshold.unwrap_or(U256::zero());

    let count = sqlx::query!(
        "
        SELECT
            COUNT(*)
        FROM
          finalization_data
          JOIN withdrawals w ON finalization_data.withdrawal_id = w.id
        WHERE
          finalization_tx IS NULL
          AND finalization_data.l2_block_number > COALESCE(
            (
              SELECT
                MAX(l2_block_number)
              FROM
                l2_blocks
              WHERE
                execute_l1_block_number IS NOT NULL
            ),
            1
          )
          AND token = decode('000000000000000000000000000000000000800A', 'hex') 
          AND amount >= $1
        ",
        u256_to_big_decimal(eth_threshold),
    )
    .fetch_one(pool)
    .await?;

    Ok(count.count.unwrap_or(0))
}

/// Get the number of ETH withdrawals executed but not finalized
pub async fn get_executed_and_not_finalized_withdrawals_count(pool: &PgPool) -> Result<i64> {
    let count = sqlx::query!(
        "
        SELECT
            COUNT(*)
        FROM
          finalization_data
          JOIN withdrawals w ON finalization_data.withdrawal_id = w.id
        WHERE
          finalization_tx IS NULL
          AND failed_finalization_attempts = 0
          AND finalization_data.l2_block_number <= COALESCE(
            (
              SELECT
                MAX(l2_block_number)
              FROM
                l2_blocks
              WHERE
                execute_l1_block_number IS NOT NULL
            ),
            1
          )
          AND token = decode('000000000000000000000000000000000000800A', 'hex') 
        ",
    )
    .fetch_one(pool)
    .await?;

    Ok(count.count.unwrap_or(0))
}

/// Fetch finalization parameters for some withdrawal
pub async fn get_finalize_withdrawal_params(
    pool: &PgPool,
    id: u64,
    gas: u64,
) -> Result<Option<RequestFinalizeWithdrawal>> {
    let res = sqlx::query!(
        "
        SELECT
            finalization_data.l2_block_number,
            l1_batch_number,
            l2_message_index,
            l2_tx_number_in_block,
            message,
            proof,
            withdrawals.token
        FROM
            finalization_data
        JOIN withdrawals ON withdrawals.id = finalization_data.withdrawal_id
        WHERE
            withdrawal_id = $1
        ",
        id as i64
    )
    .fetch_optional(pool)
    .await?
    .map(|r| RequestFinalizeWithdrawal {
        l_2_block_number: r.l1_batch_number.into(),
        l_2_message_index: r.l2_message_index.into(),
        l_2_tx_number_in_block: r.l2_tx_number_in_block as u16,
        message: r.message.into(),
        merkle_proof: bincode::deserialize(&r.proof).unwrap(),
        is_eth: is_eth(Address::from_slice(&r.token)),
        gas: gas.into(),
    });

    Ok(res)
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

    let latency = STORAGE_METRICS.call[&"finalization_data_set_finalized_in_tx"].start();

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

    latency.observe();

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

    let latency = STORAGE_METRICS.call[&"inc_unsuccessful_finalization_attempts"].start();

    sqlx::query!(
        "
        UPDATE
          finalization_data
        SET
          last_finalization_attempt = NOW(),
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

    latency.observe();

    Ok(())
}

/// Fetch decimals and L1 address for a token.
///
/// # Arguments
///
/// * `pool` - `PgPool`
/// * `token` - L2 token address.
pub async fn token_decimals_and_l1_address(
    pool: &PgPool,
    token: Address,
) -> Result<Option<(u32, Address)>> {
    let latency = STORAGE_METRICS.call[&"token_decimals_and_l1_address"].start();

    let result = sqlx::query!(
        "
        SELECT
            decimals,
            l1_token_address
        FROM
            tokens
        WHERE
            l2_token_address = $1
        ",
        token.as_bytes(),
    )
    .fetch_optional(pool)
    .await?
    .map(|r| (r.decimals as u32, Address::from_slice(&r.l1_token_address)));

    latency.observe();
    Ok(result)
}

async fn wipe_finalization_data(pool: &PgPool, delete_batch_size: usize) -> Result<()> {
    loop {
        let deleted_ids = sqlx::query!(
            "
            DELETE FROM
              finalization_data
            WHERE
              withdrawal_id in (
                SELECT
                  withdrawal_id
                from
                  finalization_data
                LIMIT
                  $1
              )
            RETURNING withdrawal_id
            ",
            delete_batch_size as i64,
        )
        .fetch_all(pool)
        .await?;

        if deleted_ids.is_empty() {
            return Ok(());
        }
    }
}

async fn wipe_l2_blocks(pool: &PgPool, delete_batch_size: usize) -> Result<()> {
    loop {
        let deleted_ids = sqlx::query!(
            "
            DELETE FROM
              l2_blocks
            WHERE
              l2_block_number in (
                SELECT
                  l2_block_number
                from
                  l2_blocks
                LIMIT
                  $1
              )
            RETURNING l2_block_number
            ",
            delete_batch_size as i64,
        )
        .fetch_all(pool)
        .await?;

        if deleted_ids.is_empty() {
            return Ok(());
        }
    }
}

async fn wipe_l2_to_l1_events(pool: &PgPool, delete_batch_size: usize) -> Result<()> {
    loop {
        let deleted_ids = sqlx::query!(
            "
            DELETE FROM
              l2_to_l1_events
            WHERE
              l1_block_number in (
                SELECT
                  l1_block_number
                from
                  l2_to_l1_events
                LIMIT
                  $1
              )
            RETURNING l1_block_number
            ",
            delete_batch_size as i64,
        )
        .fetch_all(pool)
        .await?;

        if deleted_ids.is_empty() {
            return Ok(());
        }
    }
}

async fn wipe_tokens(pool: &PgPool) -> Result<()> {
    sqlx::query!("DELETE FROM tokens").execute(pool).await?;

    Ok(())
}

async fn wipe_withdrawals(pool: &PgPool, delete_batch_size: usize) -> Result<()> {
    loop {
        let deleted_ids = sqlx::query!(
            "
            DELETE FROM
              withdrawals
            WHERE
              id in (
                SELECT
                  id
                from
                  withdrawals
                LIMIT
                  $1
              ) RETURNING id
            ",
            delete_batch_size as i64,
        )
        .fetch_all(pool)
        .await?;

        if deleted_ids.is_empty() {
            return Ok(());
        }
    }
}

/// Delete all content from finalizer db tables
pub async fn delete_db_content(pool: &PgPool, delete_batch_size: usize) -> Result<()> {
    wipe_finalization_data(pool, delete_batch_size).await?;

    wipe_l2_blocks(pool, delete_batch_size).await?;

    wipe_l2_to_l1_events(pool, delete_batch_size).await?;

    wipe_tokens(pool).await?;

    wipe_withdrawals(pool, delete_batch_size).await?;

    Ok(())
}

/// Delete all content from `finalization_data` table.
pub async fn delete_finalization_data_content(
    pool: &PgPool,
    delete_batch_size: usize,
) -> Result<()> {
    wipe_finalization_data(pool, delete_batch_size).await?;

    Ok(())
}

/// Finalization status of a withdrawal
#[derive(Debug, Clone)]
pub enum FinalizationStatus {
    /// Withdrawal has been finalized
    Finalized,
    /// Withdrawal has not been finalized
    NotFinalized,
}

/// Withdrawal event requested for address
pub struct UserWithdrawal {
    /// Transaction hash
    pub tx_hash: H256,
    /// Token address
    pub token: Address,
    /// Amount
    pub amount: U256,
    /// Status
    pub status: FinalizationStatus,
}

/// Request withdrawals for a given address.
pub async fn withdrawals_for_address(
    pool: &PgPool,
    address: Address,
    limit: u64,
) -> Result<Vec<UserWithdrawal>> {
    let latency = STORAGE_METRICS.call[&"withdrawals_for_address"].start();

    let events = sqlx::query!(
        "
         SELECT
             l2_to_l1_events.l1_token_addr,
             l2_to_l1_events.amount,
             withdrawals.tx_hash,
             finalization_data.finalization_tx
         FROM
             l2_to_l1_events
         JOIN finalization_data ON
             finalization_data.l1_batch_number = l2_to_l1_events.l2_block_number
         AND finalization_data.l2_tx_number_in_block = l2_to_l1_events.tx_number_in_block
         JOIN withdrawals ON
             withdrawals.id = finalization_data.withdrawal_id
         WHERE l2_to_l1_events.to_address = $1
         ORDER BY l2_to_l1_events.l2_block_number DESC
         LIMIT $2
        ",
        address.as_bytes(),
        limit as i64,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| {
        let status = if r.finalization_tx.is_some() {
            FinalizationStatus::Finalized
        } else {
            FinalizationStatus::NotFinalized
        };
        UserWithdrawal {
            tx_hash: H256::from_slice(&r.tx_hash),
            token: Address::from_slice(&r.l1_token_addr),
            amount: utils::bigdecimal_to_u256(r.amount),
            status,
        }
    })
    .collect();

    latency.observe();

    Ok(events)
}
