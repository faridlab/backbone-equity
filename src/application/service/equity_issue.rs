//! Issue new shares to a holder (hand-authored, user-owned) — the cap table grows + a capital journal posts.
//!
//! An `impl EquityWriteService` chunk over the vocabulary in [`super::equity_write_service`]. Capital is
//! booked AT PAR, the excess to share premium; an issue below par is refused. Posts
//! Dr Bank · Cr Share Capital · Cr Share Premium.
//!
//! Per the module's 4-layer rule this file holds no SQL — the issue insert lives on
//! `ShareTransactionRepository`, which takes this service's transaction so the register move + its
//! outbox stage commit together.

use backbone_orm::company_scope;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::infrastructure::persistence::NewIssueTxnRow;

use super::equity_events::{EquityEvent, EquityEventSink};
use super::equity_gl::{GlPostLine, GlPostSink};
use super::equity_write_service::{EquityError, EquityWriteService, IssueShares, PostOutcome, stage};

impl EquityWriteService {
    /// Issue new shares to a holder: capital booked at par, the excess to share premium. Refuses an issue
    /// below par. Posts Dr Bank · Cr Share Capital · Cr Share Premium.
    pub async fn issue_shares(
        &self,
        i: IssueShares,
        sink: &dyn GlPostSink,
        events: &dyn EquityEventSink,
    ) -> Result<PostOutcome, EquityError> {
        if i.quantity <= Decimal::ZERO {
            return Err(EquityError::Invalid("quantity must be positive".into()));
        }
        let class = self.load_class(i.share_class_id).await?;
        if i.price_per_share < class.par_value {
            return Err(EquityError::Invalid("issue price is below par value".into()));
        }
        let amount = i.quantity * i.price_per_share;
        let capital = i.quantity * class.par_value;
        let premium = amount - capital;
        let txn_id = Uuid::new_v4();

        // Post the balanced capital journal first (the external effect), then record the movement in a tx.
        let mut lines = vec![
            GlPostLine::debit(i.bank_account_id, amount).with_description("Share issue — cash in"),
            GlPostLine::credit(class.share_capital_account_id, capital).with_description("Share capital at par"),
        ];
        if premium > Decimal::ZERO {
            lines.push(GlPostLine::credit(class.share_premium_account_id, premium).with_description("Share premium"));
        }
        let ack = self.post(sink, &i.company_id, "issue", txn_id, i.txn_date, i.reference.clone(), "Share issue", lines).await?;

        // RLS scope (ADR-0008): the company is on the DTO — bind it explicitly onto our own tx, so the
        // register movement and its outbox stage are fenced for request and job callers alike.
        let mut tx = self.pool.begin().await?;
        company_scope::bind_company_on(&mut tx, i.company_id).await?;
        self.transactions.insert_issue(&mut tx, &NewIssueTxnRow {
            id: txn_id,
            company_id: i.company_id,
            share_class_id: i.share_class_id,
            shareholder_id: i.shareholder_id,
            quantity: i.quantity,
            price_per_share: i.price_per_share,
            amount,
            posting_reference: i.reference.as_deref(),
            txn_date: i.txn_date,
        }).await?;

        let event = EquityEvent::SharesIssued {
            transaction_id: txn_id, company_id: i.company_id, share_class_id: i.share_class_id,
            shareholder_id: i.shareholder_id, quantity: i.quantity, amount,
        };
        stage(&mut tx, "SharesIssued", "ShareTransaction", txn_id, &event).await?;
        tx.commit().await?;
        events.publish(&event);
        Ok(PostOutcome { id: txn_id, journal_id: Some(ack.journal_id), amount })
    }
}
