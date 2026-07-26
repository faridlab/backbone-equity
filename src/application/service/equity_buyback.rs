//! Buy shares back from a holder (hand-authored, user-owned) — Dr Share Capital (par) · Dr Retained
//! Earnings (excess of price over par) · Cr Bank (cash out).
//!
//! An `impl EquityWriteService` chunk over the vocabulary in [`super::equity_write_service`]. Bounds the
//! quantity against the holder's live holding under the position lock.
//!
//! Per the module's 4-layer rule this file holds no SQL — the position lock, the holding read, and the
//! buyback insert live on `ShareTransactionRepository`, which takes this service's transaction so the
//! register move + its journal commit together.

use backbone_orm::company_scope;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::infrastructure::persistence::NewBuybackTxnRow;

use super::equity_events::{EquityEvent, EquityEventSink};
use super::equity_gl::{GlPostLine, GlPostSink};
use super::equity_write_service::{BuybackShares, EquityError, EquityWriteService, PostOutcome, stage};

impl EquityWriteService {
    /// Buy shares back from a holder: Dr Share Capital (par) · Dr Retained Earnings (excess of price over par)
    /// · Cr Bank (cash out). Bounds the quantity against the holder's live holding under the position lock.
    pub async fn buyback_shares(
        &self,
        b: BuybackShares,
        sink: &dyn GlPostSink,
        events: &dyn EquityEventSink,
    ) -> Result<PostOutcome, EquityError> {
        if b.quantity <= Decimal::ZERO {
            return Err(EquityError::Invalid("quantity must be positive".into()));
        }
        let class = self.load_class(b.share_class_id).await?;
        let amount = b.quantity * b.price_per_share;
        let capital = b.quantity * class.par_value;
        let excess = amount - capital; // premium paid on buyback → Retained Earnings (if price > par)
        let txn_id = Uuid::new_v4();

        // RLS scope (ADR-0008): company on the DTO — bind it before the holding read (see `transfer_shares`).
        let mut tx = self.pool.begin().await?;
        company_scope::bind_company_on(&mut tx, b.company_id).await?;
        self.transactions.lock_position(&mut tx, b.company_id, b.share_class_id, b.shareholder_id).await?;
        let held = self.transactions.holding(&mut tx, b.company_id, b.share_class_id, b.shareholder_id).await?;
        if b.quantity > held {
            return Err(EquityError::InsufficientShares { held, requested: b.quantity });
        }
        self.transactions.insert_buyback(&mut tx, &NewBuybackTxnRow {
            id: txn_id,
            company_id: b.company_id,
            share_class_id: b.share_class_id,
            shareholder_id: b.shareholder_id,
            quantity: b.quantity,
            price_per_share: b.price_per_share,
            amount,
            txn_date: b.txn_date,
        }).await?;

        // Post while still holding the lock (the register move + its journal commit together).
        let mut lines = vec![GlPostLine::debit(class.share_capital_account_id, capital).with_description("Share capital retired")];
        if excess > Decimal::ZERO {
            lines.push(GlPostLine::debit(b.retained_earnings_account_id, excess).with_description("Buyback premium"));
        } else if excess < Decimal::ZERO {
            // Bought back below par — the gain credits retained earnings.
            lines.push(GlPostLine::credit(b.retained_earnings_account_id, -excess).with_description("Buyback discount"));
        }
        lines.push(GlPostLine::credit(b.bank_account_id, amount).with_description("Buyback — cash out"));
        let ack = self.post(sink, &b.company_id, "buyback", txn_id, b.txn_date, None, "Share buyback", lines).await?;

        let event = EquityEvent::SharesIssued {
            transaction_id: txn_id, company_id: b.company_id, share_class_id: b.share_class_id,
            shareholder_id: b.shareholder_id, quantity: b.quantity, amount,
        };
        // (buyback reuses the movement event shape; a dedicated SharesBoughtBack can be added when a consumer needs it)
        stage(&mut tx, "SharesBoughtBack", "ShareTransaction", txn_id, &event).await?;
        tx.commit().await?;
        events.publish(&event);
        Ok(PostOutcome { id: txn_id, journal_id: Some(ack.journal_id), amount })
    }
}
