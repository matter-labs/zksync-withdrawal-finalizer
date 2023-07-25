ALTER TABLE withdrawals DROP COLUMN is_finalized;
ALTER TABLE withdrawals ADD id BIGSERIAL NOT NULL UNIQUE;

CREATE TABLE finalization_data (
    withdrawal_id BIGINT NOT NULL UNIQUE, 

    l2_block_number BIGINT NOT NULL,
    l1_batch_number BIGINT NOT NULL,
    l2_message_index INT NOT NULL,
    l2_tx_number_in_block SMALLINT NOT NULL,
    message BYTEA NOT NULL,
    sender BYTEA NOT NULL,
    proof BYTEA NOT NULL,

    finalization_tx BYTEA DEFAULT NULL,
    failed_finalization_attempts BIGINT DEFAULT 0,

    FOREIGN KEY (withdrawal_id) REFERENCES withdrawals (id)
);

