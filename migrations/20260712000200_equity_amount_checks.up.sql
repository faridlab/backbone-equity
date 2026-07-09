-- Equity maturity backstop (money/register correctness): the schema's @non_negative on quantity/amount/
-- price/par_value was never emitted to DDL (bare NUMERIC), and the register/GL logic trusts the sign of
-- `quantity` (holding = Σ CASE WHEN issue/transfer_in THEN +qty ELSE -qty). A NEGATIVE quantity inserted
-- through the generic CRUD stack (which never calls the write service) would make a `buyback` of -80 ADD 80
-- shares to a holding — corrupting the cap table and the very sum the concurrency bound reads — and a
-- negative amount/price would post a nonsensical capital journal. These CHECKs close every writer at the DB.
ALTER TABLE equity.share_transactions
  ADD CONSTRAINT share_transactions_quantity_positive CHECK (quantity > 0),
  ADD CONSTRAINT share_transactions_price_non_negative CHECK (price_per_share >= 0),
  ADD CONSTRAINT share_transactions_amount_non_negative CHECK (amount >= 0);
ALTER TABLE equity.share_classes
  ADD CONSTRAINT share_classes_par_non_negative CHECK (par_value >= 0);
ALTER TABLE equity.dividends
  ADD CONSTRAINT dividends_amounts_non_negative CHECK (per_share_amount >= 0 AND shares_outstanding >= 0 AND total_amount >= 0);
