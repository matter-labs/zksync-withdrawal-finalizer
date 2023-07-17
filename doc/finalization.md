Finalizer
=========

Note:

> All previously implemented components names have to change their names from `finalizer`-related
> ones to `watcher`-related counterparts.

Storage
-------

The finalizer keeps information about its operation in a separate table. This table
references the information about withdrawals in the `withdrawals` table by
`(tx_hash, event_index_in_tx)` key. To this key this table adds the fields specific
to finalization process:
```sql
CREATE TABLE finalized_withdrawals (
    tx_hash BYTEA NOT NULL,
    event_index_in_tx INT NOT NULL,
    l2_block_number BIGINT NOT NULL,

    --- The tx of successful tx call to finalizer contract.
    finalization_tx BYTEA DEFAULT NULL,

    -- If the tx to finalize has failed, this number is bumped.
    failed_finalization_attempts BIGINT DEFAULT 0,

    PRIMARY KEY (tx_hash, event_index_in_tx)
);
```

Operation
---------

New finalizer needs to pick up operation at some point from the
old one. Since at this point `finalized_withdrawals` table is empty
the finalizer needs to be configured with some block number to pick
up execution from.

Then in a loop finalizer performs the following steps:

1. Query all info about all newly executed events from `withdrawals` table that
    have not been yet updated into `finalized_withdrawals` as such:

    ```sql
    WITH maxExecutedBlock (max) AS (SELECT MAX(l2_block_number) FROM l2_blocks WHERE execute_l1_block_number is not NULL),
    maxSeenBlock (max) AS (SELECT MAX(l2_block_number) FROM finalized_withdrawals)

    SELECT *
    FROM withdrawals,maxExecutedBlock,maxSeenBlock
    WHERE l2_block_number < maxExecutedBlock.max AND l2_block_number > maxSeenBlock.max;
    ```

2. All newly received information is inserted into `finalized_withdrawals` with `finalization_tx`
   set to `NULL`. and `failed_finalization_attempts` set to `0`.

3. Start querying all events in `finalized_withdrawals` that have never been attempted to be
   finalized before:

   ```sql
   SELECT (tx_hash, event_index_in_tx) FROM finalized_withdrawals
   WHERE failed_finalization_attempts = 0 AND finalization_tx = NULL;
   ```

4. For each of the returned events start building a finalization batch in `WithdrawalsAccumulator`
   by calling `finalize_withdrawal_params` and adding returned parameters to accumulator as
   done in the previous version of the finalizer.

5. As before at each newly added withdrawal to accumulator check if the batch is ready to be finalized
   and if it is:

   * Consume the ready-to-be-finalized batch
   * Spawn off an `async` task to handle finalization of this batch
     * Get predictions for this batch from a call to `finalizeWithdrawals`
     * Do the same checks for success and gas as in the previous finalizer.
     * If all checks pass create a `finalizeWithdrawals` call, `.send().await` it to get the pending tx future
     * `.await` this future
     * if the submitted transaction future has failed, inc `failed_finalization_attempts` field
       for in this batch.
     * if the submitted transaction future has succeeded, update the `finalization_tx` field from
       `NULL` to the hash of the said tx for the batch.


Possible pitfalls
=================

Step `5` may fail at any step and create inconsistencies such as "a transaction has been submitted"
but neither successful or failed result has ever been recorded into the database.

Not clear what to do with tx that have failed once and `failed_finalization_attempts` has been
incremented for them from `0` to `1`.

In the process of switching from the old finalizer to the new one there still may
be an overlap of history and the new finalizer may see withdrawals that have been
finalizer by the old one. As such likely `is_withdrawal_finalized` call is needed.

Transition to the new finalizer makes `withdrawals.is_finalized` field obsolete.
