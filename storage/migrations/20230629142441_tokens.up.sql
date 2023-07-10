CREATE TABLE tokens
(
    l1_token_address BYTEA NOT NULL,
    l2_token_address BYTEA NOT NULL,
    name VARCHAR NOT NULL,
    symbol VARCHAR NOT NULL,
    decimals INT NOT NULL,
    l2_block_number BIGINT NOT NULL,
    initialization_transaction BYTEA NOT NULL,
    PRIMARY KEY (l1_token_address, l2_token_address)
);
