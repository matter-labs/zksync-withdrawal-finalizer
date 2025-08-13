CREATE INDEX IF NOT EXISTS l2_blocks_chain_id ON l2_blocks (l2_block_number DESC) INCLUDE (commit_chain_id);
