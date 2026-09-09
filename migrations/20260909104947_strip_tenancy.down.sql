-- Hand-authored (user-owned). Not regenerated.
--
-- Best-effort restore sketch for the tenancy strip (ADR-0029). This is a breaking module
-- release against dev-stage databases: the down re-adds the company_id column as nullable
-- with its plain index and the company isolation policy shape, but restores NO data —
-- rows written after the strip (or after the decorator re-keyed them) carry org_unit_id
-- only. The composing service's tenancy decorator remains the live fence; treat this
-- down as a schema-shape sketch for archaeology, not a usable rollback.
--
-- The equity.outbox_events fence is deliberately NOT touched: the outbox is relay
-- infrastructure owned by the framework, not a schema model of this module.

ALTER TABLE equity.dividends          ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE equity.share_classes      ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE equity.shareholders       ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE equity.share_transactions ADD COLUMN IF NOT EXISTS company_id uuid;

CREATE INDEX IF NOT EXISTS idx_dividends_company_id_share_class_id
    ON equity.dividends (company_id, share_class_id);
CREATE INDEX IF NOT EXISTS idx_share_classes_company_id_code
    ON equity.share_classes (company_id, code);
CREATE INDEX IF NOT EXISTS idx_shareholders_company_id
    ON equity.shareholders (company_id);
CREATE INDEX IF NOT EXISTS idx_share_transactions_company_id_share_class_id_shareholder_id
    ON equity.share_transactions (company_id, share_class_id, shareholder_id);
