-- Revert the ADR-0014 strict fence re-statement for equity module.
-- The fence predates this migration (ADR-0008-era), so the honest reverse is to
-- re-state the same live policy, not to disarm the tables: a down that disabled RLS
-- would leave company data unfenced — a posture this module never had.

-- Re-state the pre-existing fence for equity.dividends (identical policy; see header).
DROP POLICY IF EXISTS dividends_company_isolation ON equity.dividends;
CREATE POLICY dividends_company_isolation ON equity.dividends
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Re-state the pre-existing fence for equity.share_classes (identical policy; see header).
DROP POLICY IF EXISTS share_classes_company_isolation ON equity.share_classes;
CREATE POLICY share_classes_company_isolation ON equity.share_classes
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Re-state the pre-existing fence for equity.share_transactions (identical policy; see header).
DROP POLICY IF EXISTS share_transactions_company_isolation ON equity.share_transactions;
CREATE POLICY share_transactions_company_isolation ON equity.share_transactions
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Re-state the pre-existing fence for equity.shareholders (identical policy; see header).
DROP POLICY IF EXISTS shareholders_company_isolation ON equity.shareholders;
CREATE POLICY shareholders_company_isolation ON equity.shareholders
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

