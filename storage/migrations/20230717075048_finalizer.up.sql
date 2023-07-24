ALTER TABLE withdrawals ADD id BIGSERIAL NOT NULL UNIQUE;

CREATE TABLE finalization_data (
    tx_hash BYTEA NOT NULL,
    event_index_in_tx INT NOT NULL,

    id BIGINT NOT NULL UNIQUE, 

    l2_block_number BIGINT NOT NULL,
    l1_batch_number BIGINT NOT NULL,
    l2_message_index INT NOT NULL,
    l2_tx_number_in_block SMALLINT NOT NULL,
    message BYTEA NOT NULL,
    sender BYTEA NOT NULL,
    proof BYTEA NOT NULL,

    finalization_tx BYTEA DEFAULT NULL,
    failed_finalization_attempts BIGINT DEFAULT 0,

    PRIMARY KEY (tx_hash, event_index_in_tx),

    FOREIGN KEY (tx_hash, event_index_in_tx) REFERENCES withdrawals (tx_hash, event_index_in_tx)
);

