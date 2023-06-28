CREATE TABLE l2_to_l1_events
(
    l1_token_addr BYTEA NOT NULL,
    to_address BYTEA NOT NULL,
    amount NUMERIC(80) NOT NULL,
    l1_block_number BIGINT NOT NULL,
    l2_block_number BIGINT NOT NULL,
    tx_number_in_block INT NOT NULL,
    PRIMARY KEY (l1_block_number, l2_block_number, tx_number_in_block)
);
