ALTER TABLE withdrawals ADD COLUMN l1_receiver BYTEA;
CREATE INDEX IF NOT EXISTS ix_withdrawals_l1_receiver ON withdrawals (l1_receiver);
