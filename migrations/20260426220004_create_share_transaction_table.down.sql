-- Down: drop equity.share_transactions table
DROP TABLE IF EXISTS equity.share_transactions CASCADE;
DROP FUNCTION IF EXISTS equity.share_transactions_audit_timestamp() CASCADE;
