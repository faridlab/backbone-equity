-- Down: drop the declared foreign keys for equity module
ALTER TABLE IF EXISTS equity.share_transactions DROP CONSTRAINT IF EXISTS fk_share_transactions_counterparty_shareholder_id;
ALTER TABLE IF EXISTS equity.share_transactions DROP CONSTRAINT IF EXISTS fk_share_transactions_shareholder_id;
ALTER TABLE IF EXISTS equity.share_transactions DROP CONSTRAINT IF EXISTS fk_share_transactions_share_class_id;
ALTER TABLE IF EXISTS equity.dividends DROP CONSTRAINT IF EXISTS fk_dividends_share_class_id;
