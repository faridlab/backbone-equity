-- Down: drop equity.dividends table
DROP TABLE IF EXISTS equity.dividends CASCADE;
DROP FUNCTION IF EXISTS equity.dividends_audit_timestamp() CASCADE;
