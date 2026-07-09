-- Down: drop equity.shareholders table
DROP TABLE IF EXISTS equity.shareholders CASCADE;
DROP FUNCTION IF EXISTS equity.shareholders_audit_timestamp() CASCADE;
