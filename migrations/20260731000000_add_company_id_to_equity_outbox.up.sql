-- The equity outbox table (20260712000100) was authored in the pre-multi-tenant shape: it has no
-- `company_id` column. But `backbone-outbox` is pinned to v2.7.4 with the `multi_tenant` feature, whose
-- `stage()` INSERTs `company_id` (and whose canonical `migrate()` declares it `NOT NULL`). Without this
-- column every equity write path fails at the outbox stage. Add it + the tenant lookup index so a fresh
-- `sqlx migrate run` matches the crate.
--
-- Idempotent (IF NOT EXISTS) so it is safe on a DB already band-aided by hand. NOT NULL matches the
-- crate's canonical schema; safe because the table holds no rows until `stage()` populates it (and
-- `stage` always supplies the tenant). Deliberately does NOT add the v2.7.4 RLS fence — that fence would
-- hide outbox rows from readers that don't set `app.company_id` (e.g. integrity probe EIP-4); adopting it
-- is a separate decision that requires those readers to bind company scope first.
ALTER TABLE equity.outbox_events ADD COLUMN IF NOT EXISTS company_id uuid NOT NULL;
CREATE INDEX IF NOT EXISTS idx_equity_outbox_company_id ON equity.outbox_events (company_id);
