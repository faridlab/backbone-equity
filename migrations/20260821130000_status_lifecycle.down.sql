-- Down: restore the is_active boolean exactly as it was.
-- Only 'inactive' rows are written back as FALSE; rows at the column default
-- map to the boolean default TRUE without an UPDATE.

ALTER TABLE equity.share_classes ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT TRUE;
UPDATE equity.share_classes SET is_active = FALSE WHERE status = 'inactive';
ALTER TABLE equity.share_classes DROP COLUMN status;

DROP TYPE IF EXISTS share_class_status;
