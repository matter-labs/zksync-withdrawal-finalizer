CREATE TABLE withdrawals
(
    tx_hash BYTEA NOT NULL,
    l2_block_number BIGINT NOT NULL,
    token BYTEA NOT NULL,
    amount NUMERIC(80) NOT NULL,
    event_index_in_tx INT NOT NULL,
    is_finalized BOOLEAN NOT NULL,
    PRIMARY KEY (tx_hash, event_index_in_tx)
);
CREATE INDEX withdrawals_l2_block_number_index ON withdrawals (l2_block_number);

CREATE TABLE l2_blocks
(
    l2_block_number BIGSERIAL NOT NULL,
    commit_l1_block_number BIGINT DEFAULT NULL,
    verify_l1_block_number BIGINT DEFAULT NULL,
    execute_l1_block_number BIGINT DEFAULT NULL,
    PRIMARY KEY (l2_block_number)
);
