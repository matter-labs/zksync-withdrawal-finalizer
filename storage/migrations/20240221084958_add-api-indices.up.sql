CREATE INDEX l2_to_l1_events_t1 ON l2_to_l1_events (to_address,l2_block_number);
CREATE INDEX finalization_data_t1 on finalization_data (l1_batch_number,l2_tx_number_in_block);
