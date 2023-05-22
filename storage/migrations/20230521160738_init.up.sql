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
    blocknumber BIGSERIAL NOT NULL,
    token BYTEA NOT NULL,
    amount NUMERIC(80) NOT NULL,
    event_index_in_tx INT NOT NULL,
    status withdrawal_status NOT NULL,
    UNIQUE (tx_hash, blocknumber,event_index_in_tx)
);
