CREATE TABLE withdrawals
(
    tx_hash BYTEA NOT NULL,
    block_number BIGSERIAL NOT NULL,
    token BYTEA NOT NULL,
    amount NUMERIC(80) NOT NULL,
    event_index_in_tx INT NOT NULL,
    committed_in_block BIGINT DEFAULT NULL,
    verified_in_block BIGINT DEFAULT NULL,
    executed_in_block BIGINT DEFAULT NULL,
    is_finalized BOOLEAN DEFAULT FALSE,
    PRIMARY KEY (tx_hash, event_index_in_tx)
);
CREATE INDEX withdrawals_block_number_index ON withdrawals (block_number);

CREATE TABLE committed_l1_events
(
    l1_block_number BIGINT NOT NULL,
    l1_batch_number BIGINT NOT NULL,
    l2_range_begin BIGINT NOT NULL,
    l2_range_end BIGINT NOT NULL
);
CREATE INDEX committed_l1_events_batch_range_index ON committed_l1_events (l2_range_begin, l2_range_end);

CREATE TABLE verified_l1_events
(
    l1_block_number BIGINT NOT NULL,
    l2_previous_last_verified_block BIGINT NOT NULL,
    l2_current_last_verified_block BIGINT NOT NULL,
    l2_range_begin BIGINT NOT NULL,
    l2_range_end BIGINT NOT NULL
);
CREATE INDEX verified_l1_events_batch_range_index ON verified_l1_events (l2_range_begin, l2_range_end);

CREATE TABLE executed_l1_events
(
    l1_block_number BIGINT NOT NULL,
    l1_batch_number BIGINT NOT NULL,
    l2_range_begin BIGINT NOT NULL,
    l2_range_end BIGINT NOT NULL
);
CREATE INDEX executed_l1_events_batch_range_index ON executed_l1_events (l2_range_begin, l2_range_end);
