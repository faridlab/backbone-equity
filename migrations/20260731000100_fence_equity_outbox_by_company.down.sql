DROP POLICY IF EXISTS outbox_events_company_isolation ON equity.outbox_events;
ALTER TABLE equity.outbox_events NO FORCE ROW LEVEL SECURITY;
ALTER TABLE equity.outbox_events DISABLE ROW LEVEL SECURITY;
