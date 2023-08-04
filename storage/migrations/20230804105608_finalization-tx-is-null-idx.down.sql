DROP INDEX fd_l2_block_number_finalization_tx_ix;
CREATE INDEX IF NOT EXISTS fd_l2_block_number_ix ON finalization_data (l2_block_number);
