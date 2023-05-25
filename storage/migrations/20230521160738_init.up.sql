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

CREATE TABLE last_committed_block
(
    onerow_id BOOL PRIMARY KEY DEFAULT TRUE,
    block_number BIGSERIAL NOT NULL,
    CONSTRAINT onerow_uni CHECK(onerow_id)
);

CREATE TABLE last_verified_block
(
    onerow_id BOOL PRIMARY KEY DEFAULT TRUE,
    block_number BIGSERIAL NOT NULL,
    CONSTRAINT onerow_uni CHECK(onerow_id)
);

CREATE TABLE last_executed_block
(
    onerow_id BOOL PRIMARY KEY DEFAULT TRUE,
    block_number BIGSERIAL NOT NULL,
    CONSTRAINT onerow_uni CHECK(onerow_id)
);
