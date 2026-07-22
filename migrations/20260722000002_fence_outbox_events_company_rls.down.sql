DROP POLICY IF EXISTS outbox_events_company_isolation ON equity.outbox_events;
ALTER TABLE equity.outbox_events NO FORCE ROW LEVEL SECURITY;
ALTER TABLE equity.outbox_events DISABLE ROW LEVEL SECURITY;
DROP INDEX IF EXISTS equity.idx_equity_outbox_company_id;
ALTER TABLE equity.outbox_events DROP COLUMN IF EXISTS company_id;
