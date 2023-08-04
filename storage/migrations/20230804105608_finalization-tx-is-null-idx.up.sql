DROP INDEX fd_l2_block_number_ix;
CREATE INDEX fd_l2_block_number_finalization_tx_ix ON finalization_data (l2_block_number, finalization_tx) WHERE finalization_tx IS NULL;
