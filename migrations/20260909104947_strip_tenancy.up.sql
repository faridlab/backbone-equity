-- Hand-authored (user-owned). Not regenerated.
--
-- Strip every company-fence artifact from the equity tables (ADR-0029): the module is
-- tenant-agnostic; org scoping is installed by the COMPOSING service's tenancy decorator,
-- never by the module. Dropped here, per table: the company-leading indexes, the
-- <table>_company_isolation RLS policy, and the company_id column itself.
--
-- Ordering guard (the decorator must run FIRST on any database with data): the module
-- never moves tenancy data. A table is safe to strip when EITHER
--   a) it carries org_unit_id with no NULLs — the decorator backfilled it from company_id —
--      or b) it is empty (a fresh database: the earlier chain files created it empty).
-- Otherwise the strip RAISEs, naming the decorator step, rather than dropping a column
-- that still holds the only tenancy key. The file is re-runnable (every drop is IF EXISTS
-- and the tracker has no checksums), so a failed run retries cleanly after the decorator
-- lands.
--
-- The equity.outbox_events fence is deliberately NOT touched: the outbox is relay
-- infrastructure owned by the framework, not a schema model of this module.
--
-- RLS enable/force flags are deliberately NOT touched: the decorator owns those now.

DO $$
DECLARE
    t text;
    has_org boolean;
    org_nulls bigint;
    total bigint;
    offenders text := '';
BEGIN
    FOREACH t IN ARRAY ARRAY['dividends', 'share_classes', 'shareholders', 'share_transactions']
    LOOP
        IF to_regclass(format('equity.%I', t)) IS NULL THEN
            CONTINUE; -- chain not fully applied on this database; nothing to strip
        END IF;

        SELECT EXISTS (
                   SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'equity' AND table_name = t AND column_name = 'org_unit_id'
               )
        INTO has_org;

        EXECUTE format('SELECT count(*) FROM equity.%I', t) INTO total;

        IF has_org THEN
            EXECUTE format(
                'SELECT count(*) FROM equity.%I WHERE org_unit_id IS NULL', t)
            INTO org_nulls;
        ELSE
            org_nulls := total; -- no org column: every row's only tenancy key is company_id
        END IF;

        IF has_org AND org_nulls = 0 THEN
            CONTINUE; -- decorator backfilled: safe
        END IF;
        IF total = 0 THEN
            CONTINUE; -- empty table (fresh database): safe
        END IF;
        offenders := offenders || format(' equity.%s (%s rows, %s rows not covered by org_unit_id);', t, total, org_nulls);
    END LOOP;

    IF offenders <> '' THEN
        RAISE EXCEPTION 'refusing to strip company_id — these tables are not yet covered by the tenancy decorator:%. Apply the composing service''s tenancy decorator (it backfills org_unit_id from company_id) and re-run; it is the only step that moves tenancy data.', offenders;
    END IF;
END $$;

-- ── dividends ──────────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS equity.idx_dividends_company_id_share_class_id;
DROP POLICY IF EXISTS dividends_company_isolation ON equity.dividends;
ALTER TABLE equity.dividends DROP COLUMN IF EXISTS company_id;

-- ── share_classes ──────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS equity.idx_share_classes_company_id_code;
DROP POLICY IF EXISTS share_classes_company_isolation ON equity.share_classes;
ALTER TABLE equity.share_classes DROP COLUMN IF EXISTS company_id;

-- ── shareholders ───────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS equity.idx_shareholders_company_id;
DROP POLICY IF EXISTS shareholders_company_isolation ON equity.shareholders;
ALTER TABLE equity.shareholders DROP COLUMN IF EXISTS company_id;

-- ── share_transactions ─────────────────────────────────────────────────────────
DROP INDEX IF EXISTS equity.idx_share_transactions_company_id_share_class_id_shareholder_id;
DROP POLICY IF EXISTS share_transactions_company_isolation ON equity.share_transactions;
ALTER TABLE equity.share_transactions DROP COLUMN IF EXISTS company_id;

-- No domain constraints to restore: the register and the dividend ledger carry no
-- tenant-free unique (the per-unit share-class code unique was POSTURE — the composing
-- service's tenancy decorator owns its org-scoped form, as with party_code in the party
-- module). The tenant-free indexes that predate the strip (transfer_group_id, status,
-- the metadata GIN family) are untouched.
