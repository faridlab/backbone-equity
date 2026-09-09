//! Declare a dividend on a class + pay a declared dividend (hand-authored, user-owned).
//!
//! An `impl EquityWriteService` chunk over the vocabulary in [`super::equity_write_service`]:
//!   - `declare_dividend` snapshots shares outstanding and books the liability
//!     (Dr Retained Earnings · Cr Dividend Payable).
//!   - `pay_dividend` settles the liability (Dr Dividend Payable · Cr Bank), `declared → paid`. The
//!     exit that keeps a declared dividend from sitting as a payable forever.
//!
//! Per the module's 4-layer rule this file holds no SQL — the dividend insert, the fetch-for-payment,
//! and the CAS claim live on `DividendRepository`, which takes this service's transaction so the
//! state flip + its journal + its outbox stage commit together.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::infrastructure::persistence::NewDividendRow;

use super::equity_events::{EquityEvent, EquityEventSink};
use super::equity_gl::{GlPostLine, GlPostSink};
use super::equity_write_service::{
    shares_outstanding, stage, DeclareDividend, EquityError, EquityWriteService, PostOutcome,
};

impl EquityWriteService {
    /// Declare a dividend on a class: snapshot shares outstanding, book the liability
    /// (Dr Retained Earnings · Cr Dividend Payable). The cash goes out later via `pay_dividend`.
    pub async fn declare_dividend(
        &self,
        d: DeclareDividend,
        sink: &dyn GlPostSink,
        events: &dyn EquityEventSink,
    ) -> Result<PostOutcome, EquityError> {
        if d.per_share_amount <= Decimal::ZERO {
            return Err(EquityError::Invalid(
                "per-share amount must be positive".into(),
            ));
        }
        let outstanding =
            shares_outstanding(&self.transactions, &self.pool, d.share_class_id).await?;
        if outstanding <= Decimal::ZERO {
            return Err(EquityError::InvalidState(
                "no shares outstanding to pay a dividend on",
            ));
        }
        let total = d.per_share_amount * outstanding;
        let div_id = Uuid::new_v4();

        let lines = vec![
            GlPostLine::debit(d.retained_earnings_account_id, total)
                .with_description("Dividend declared"),
            GlPostLine::credit(d.dividend_payable_account_id, total)
                .with_description("Dividend payable"),
        ];
        let ack = self
            .post(
                sink,
                "declare",
                div_id,
                d.declaration_date,
                None,
                "Dividend declaration",
                lines,
            )
            .await?;

        // Propagate the ambient request scope onto our own tx (relay-only): the declaration insert and
        // its outbox stage run on a fresh connection the composing service's fence does not decorate.
        // Unfenced deployments have no ambient scope and skip this entirely.
        let mut tx = self.pool.begin().await?;
        if let Some(scope) = backbone_orm::org_scope::current_org_scope() {
            backbone_orm::org_scope::bind_org_scope_on(&mut *tx, &scope).await?;
        }
        self.dividends
            .insert_dividend(
                &mut tx,
                &NewDividendRow {
                    id: div_id,
                    share_class_id: d.share_class_id,
                    declaration_date: d.declaration_date,
                    per_share_amount: d.per_share_amount,
                    shares_outstanding: outstanding,
                    total_amount: total,
                    retained_earnings_account_id: d.retained_earnings_account_id,
                    dividend_payable_account_id: d.dividend_payable_account_id,
                },
            )
            .await?;

        let event = EquityEvent::DividendDeclared {
            dividend_id: div_id,
            share_class_id: d.share_class_id,
            total_amount: total,
        };
        stage(&mut tx, "DividendDeclared", "Dividend", div_id, &event).await?;
        tx.commit().await?;
        events.publish(&event);
        Ok(PostOutcome {
            id: div_id,
            journal_id: Some(ack.journal_id),
            amount: total,
        })
    }

    /// Pay a declared dividend: settle the liability (Dr Dividend Payable · Cr Bank), `declared → paid`. The
    /// exit that keeps a declared dividend from sitting as a payable forever (completeness council). Gated on
    /// the `declared` status so it settles at most once.
    pub async fn pay_dividend(
        &self,
        dividend_id: Uuid,
        bank_account_id: Uuid,
        payment_date: NaiveDate,
        sink: &dyn GlPostSink,
        events: &dyn EquityEventSink,
    ) -> Result<PostOutcome, EquityError> {
        // The dividend is identified by id alone (id is globally unique — the module carries no
        // tenancy, ADR-0029). The read rides the request-dedicated connection when the composing
        // service bound one, so a row the caller's fence excludes is simply not found.
        let row = self
            .dividends
            .fetch_for_payment(&self.pool, dividend_id)
            .await?
            .ok_or(EquityError::NotFound("dividend"))?;
        if row.status == "paid" {
            return Err(EquityError::InvalidState("dividend already paid"));
        }
        let total = row.total_amount;
        let payable_acct = row.dividend_payable_account_id;

        // Claim the settlement first (CAS declared→paid), so a concurrent pay can't double-remit.
        let mut tx = self.pool.begin().await?;
        if let Some(scope) = backbone_orm::org_scope::current_org_scope() {
            backbone_orm::org_scope::bind_org_scope_on(&mut *tx, &scope).await?;
        }
        let claimed = self
            .dividends
            .claim_payment(&mut tx, dividend_id, payment_date)
            .await?;
        if claimed != 1 {
            tx.rollback().await?;
            return Err(EquityError::InvalidState("dividend already paid"));
        }

        let lines = vec![
            GlPostLine::debit(payable_acct, total)
                .with_description("Dividend paid — settle payable"),
            GlPostLine::credit(bank_account_id, total).with_description("Dividend paid — cash out"),
        ];
        // The pay posting needs its OWN source identity: accounting dedups on (source_type, source_id,
        // posting_type), and the declaration already booked source_id=dividend_id — so the pay reuses a
        // deterministic derived id (stable across a retry, distinct from the declaration).
        let pay_source_id = Uuid::new_v5(&dividend_id, b"equity-dividend-pay");
        let ack = self
            .post(
                sink,
                "pay",
                pay_source_id,
                payment_date,
                None,
                "Dividend payment",
                lines,
            )
            .await
            .inspect_err(
                |_| { /* on GL reject the tx below rolls back, leaving the dividend declared */ },
            )?;

        let event = EquityEvent::DividendPaid {
            dividend_id,
            total_amount: total,
        };
        stage(&mut tx, "DividendPaid", "Dividend", dividend_id, &event).await?;
        tx.commit().await?;
        events.publish(&event);
        Ok(PostOutcome {
            id: dividend_id,
            journal_id: Some(ack.journal_id),
            amount: total,
        })
    }
}
