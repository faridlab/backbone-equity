//! Cap-table reads (hand-authored, user-owned) — the public aggregation surface.
//!
//! An `impl EquityWriteService` chunk over the vocabulary in [`super::equity_write_service`]. The register
//! applies TWO different aggregation rules — a holder position (issue/transfer_in +, buyback/transfer_out −)
//! and shares-outstanding (issue +, buyback −, transfers net out). Both live ONLY here; exposing them keeps
//! a consumer (a registrar, a dividend disburser, an ownership report) from re-implementing equity's sign
//! logic across the boundary and drifting when a txn_type is added.

use backbone_orm::company_scope;
use rust_decimal::Decimal;
use uuid::Uuid;

use super::equity_write_service::{Allocation, EquityError, EquityWriteService, Holding, shares_outstanding};

impl EquityWriteService {
    /// Shares outstanding for a class = Σ issued − Σ bought back.
    pub async fn class_shares_outstanding(&self, company_id: Uuid, class_id: Uuid) -> Result<Decimal, EquityError> {
        shares_outstanding(&self.transactions, &self.pool, company_id, class_id).await
    }

    /// Every holder's position in a class + its ownership percentage of shares outstanding.
    pub async fn holdings(&self, company_id: Uuid, class_id: Uuid) -> Result<Vec<Holding>, EquityError> {
        // RLS scope (ADR-0008): read-only, company on the parameter — bind it so the cap-table read is
        // fenced (and returns rows) for request and non-request callers alike.
        let rows = company_scope::with_company_scope(
            Some(company_id),
            self.transactions.holdings(&self.pool, company_id, class_id),
        )
        .await?;
        let outstanding = shares_outstanding(&self.transactions, &self.pool, company_id, class_id).await?;
        Ok(rows.into_iter().map(|r| {
            let quantity = r.quantity;
            let pct = if outstanding > Decimal::ZERO { quantity / outstanding * Decimal::from(100) } else { Decimal::ZERO };
            Holding { shareholder_id: r.shareholder_id, quantity, ownership_pct: pct }
        }).collect())
    }

    /// The per-holder split of a dividend — each holder's cut = per_share × their CURRENT holding (record
    /// date = query time). This is what makes `pay_dividend` an exit a disburser can actually act on: it
    /// tells the payout system WHOM to pay and HOW MUCH. Σ allocations == total for an unchanged register.
    pub async fn dividend_allocations(&self, dividend_id: Uuid) -> Result<Vec<Allocation>, EquityError> {
        // RLS scope (ADR-0008), ID-only pattern — see `pay_dividend`. The company read off this row then
        // scopes the `holdings` call below.
        let d = self.dividends.fetch_allocation_basis(&self.pool, dividend_id).await?
            .ok_or(EquityError::NotFound("dividend"))?;
        let company_id = d.company_id;
        let class_id = d.share_class_id;
        let per_share = d.per_share_amount;
        let holdings = self.holdings(company_id, class_id).await?;
        Ok(holdings.into_iter()
            .filter(|h| h.quantity > Decimal::ZERO)
            .map(|h| Allocation { shareholder_id: h.shareholder_id, quantity: h.quantity, amount: per_share * h.quantity })
            .collect())
    }
}
