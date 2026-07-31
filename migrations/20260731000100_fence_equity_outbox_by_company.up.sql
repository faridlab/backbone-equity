-- Adopt the backbone-outbox v2.7.4 `multi_tenant` RLS fence on equity.outbox_events, in parity with
-- the crate's canonical fence (cargo checkout 10829dc / backbone-outbox/src/outbox.rs). The
-- `company_id` column + tenant index were added by 20260731000000; this adds the tenant-isolation
-- fence itself so a tenant's event stream is isolated (ADR-0011). The cross-tenant outbox relay logs
-- in as `metaphor_relay` and is admitted by the policy's OR — a surgical per-table bypass, NOT a
-- BYPASSRLS attribute, so every other table's fence still holds. An unset `app.company_id` session
-- var sees zero rows (NULLIF → NULL), the standard fail-closed posture.
ALTER TABLE equity.outbox_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE equity.outbox_events FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS outbox_events_company_isolation ON equity.outbox_events;

CREATE POLICY outbox_events_company_isolation ON equity.outbox_events
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid
                OR current_user = 'metaphor_relay')
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid
                OR current_user = 'metaphor_relay');
