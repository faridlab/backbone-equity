DROP INDEX IF EXISTS equity.idx_equity_outbox_company_id;
ALTER TABLE equity.outbox_events DROP COLUMN IF EXISTS company_id;
