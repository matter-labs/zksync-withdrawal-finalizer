CREATE TYPE withdrawal_status as ENUM (
    'Seen',
    'Committed',
    'Verified',
    'Executed',
    'Finalized'
);

CREATE TABLE withdrawals
(
    tx_hash BYTEA NOT NULL,
    block_number BIGSERIAL NOT NULL,
    token BYTEA NOT NULL,
    amount NUMERIC(80) NOT NULL,
    event_index_in_tx INT NOT NULL,
    status withdrawal_status NOT NULL,
    UNIQUE (tx_hash, block_number, event_index_in_tx)
);
CREATE INDEX withdrawals_block_number_index ON withdrawals (block_number);
