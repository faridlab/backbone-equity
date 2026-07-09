-- Down: drop equity.share_classes table
DROP TABLE IF EXISTS equity.share_classes CASCADE;
DROP FUNCTION IF EXISTS equity.share_classes_audit_timestamp() CASCADE;
