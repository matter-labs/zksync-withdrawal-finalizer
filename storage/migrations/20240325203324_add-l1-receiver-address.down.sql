DROP INDEX IF EXISTS ix_withdrawals_l1_receiver;
ALTER TABLE withdrawals DROP COLUMN l1_receiver;
