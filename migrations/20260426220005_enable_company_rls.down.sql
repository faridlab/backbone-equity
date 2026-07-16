-- Down: remove the company RLS fence for equity module

-- Reverse the company RLS fence for equity.dividends
DROP POLICY IF EXISTS dividends_company_isolation ON equity.dividends;
ALTER TABLE equity.dividends NO FORCE ROW LEVEL SECURITY;
ALTER TABLE equity.dividends DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for equity.share_classes
DROP POLICY IF EXISTS share_classes_company_isolation ON equity.share_classes;
ALTER TABLE equity.share_classes NO FORCE ROW LEVEL SECURITY;
ALTER TABLE equity.share_classes DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for equity.shareholders
DROP POLICY IF EXISTS shareholders_company_isolation ON equity.shareholders;
ALTER TABLE equity.shareholders NO FORCE ROW LEVEL SECURITY;
ALTER TABLE equity.shareholders DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for equity.share_transactions
DROP POLICY IF EXISTS share_transactions_company_isolation ON equity.share_transactions;
ALTER TABLE equity.share_transactions NO FORCE ROW LEVEL SECURITY;
ALTER TABLE equity.share_transactions DISABLE ROW LEVEL SECURITY;

