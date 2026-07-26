//! The hand-authored equity write path (user-owned; survives regen).
//!
//! The cap table + its GL. A holder's position in a class is the SIGNED sum of its register movements. The
//! load-bearing invariants:
//!   - a position NEVER goes negative — you cannot transfer or buy back more shares than a holder holds
//!     (bounded under a per-(class,holder) advisory lock so two concurrent removals can't both pass the
//!     check — the maturity invariant);
//!   - capital is booked AT PAR, the excess to share premium (an issue below par is refused);
//!   - every money-moving event posts ONE balanced journal via the `GlPostSink` (the 10th GL producer).
//! Equity reaches accounting only through the port — zero normal Cargo edge.
//!
//! **This file is the hub:** it holds the module's vocabulary (input structs, outcomes, errors, the
//! service struct + ctor) and the shared helpers (`load_class`, `post`, the `shares_outstanding` and
//! `stage` scope wrappers). The rest of the write surface is chunked into focused siblings, each an
//! `impl EquityWriteService` block over these same types:
//!
//! - [`super::equity_register`] — register a share class / shareholder.
//! - [`super::equity_issue`] — issue new shares (`issue_shares`).
//! - [`super::equity_transfer`] — transfer shares between holders (`transfer_shares`).
//! - [`super::equity_buyback`] — buy shares back (`buyback_shares`).
//! - [`super::equity_dividend`] — declare + pay a dividend.
//! - [`super::equity_read`] — cap-table reads (shares outstanding, holdings, dividend allocations).

use backbone_orm::company_scope;
use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::infrastructure::persistence::{
    DividendRepository, ShareClassRepository, ShareTransactionRepository, ShareholderRepository,
};

use super::equity_events::*;
use super::equity_gl::*;

#[derive(Debug, thiserror::Error)]
pub enum EquityError {
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("not found: {0}")]
    NotFound(&'static str),
    #[error("invalid state: {0}")]
    InvalidState(&'static str),
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("insufficient shares: holder holds {held}, tried to remove {requested}")]
    InsufficientShares { held: Decimal, requested: Decimal },
    #[error("gl rejected: {0}")]
    GlRejected(String),
}

pub struct NewShareClass {
    pub company_id: Uuid,
    pub code: String,
    pub name: String,
    pub par_value: Decimal,
    pub currency: String,
    pub share_capital_account_id: Uuid,
    pub share_premium_account_id: Uuid,
}

pub struct NewShareholder {
    pub company_id: Uuid,
    pub party_id: Option<Uuid>,
    pub name: String,
    pub holder_type: String, // individual | entity
}

pub struct IssueShares {
    pub company_id: Uuid,
    pub share_class_id: Uuid,
    pub shareholder_id: Uuid,
    pub quantity: Decimal,
    pub price_per_share: Decimal,
    pub txn_date: NaiveDate,
    pub bank_account_id: Uuid,
    pub reference: Option<String>,
}

pub struct TransferShares {
    pub company_id: Uuid,
    pub share_class_id: Uuid,
    pub from_shareholder_id: Uuid,
    pub to_shareholder_id: Uuid,
    pub quantity: Decimal,
    pub txn_date: NaiveDate,
}

pub struct BuybackShares {
    pub company_id: Uuid,
    pub share_class_id: Uuid,
    pub shareholder_id: Uuid,
    pub quantity: Decimal,
    pub price_per_share: Decimal,
    pub txn_date: NaiveDate,
    pub bank_account_id: Uuid,
    pub retained_earnings_account_id: Uuid,
}

pub struct DeclareDividend {
    pub company_id: Uuid,
    pub share_class_id: Uuid,
    pub per_share_amount: Decimal,
    pub declaration_date: NaiveDate,
    pub retained_earnings_account_id: Uuid,
    pub dividend_payable_account_id: Uuid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PostOutcome {
    pub id: Uuid,
    pub journal_id: Option<Uuid>,
    pub amount: Decimal,
}

/// A holder's position in a class + its ownership percentage (the cap-table read).
#[derive(Debug, Clone, PartialEq)]
pub struct Holding {
    pub shareholder_id: Uuid,
    pub quantity: Decimal,
    pub ownership_pct: Decimal,
}

/// A holder's slice of a declared dividend (the per-holder payout a disburser acts on).
#[derive(Debug, Clone, PartialEq)]
pub struct Allocation {
    pub shareholder_id: Uuid,
    pub quantity: Decimal,
    pub amount: Decimal,
}

pub struct EquityWriteService {
    pub(super) pool: PgPool,
    pub(super) share_classes: ShareClassRepository,
    pub(super) shareholders: ShareholderRepository,
    pub(super) transactions: ShareTransactionRepository,
    pub(super) dividends: DividendRepository,
}

impl EquityWriteService {
    pub fn new(pool: PgPool) -> Self {
        let share_classes = ShareClassRepository::new(pool.clone());
        let shareholders = ShareholderRepository::new(pool.clone());
        let transactions = ShareTransactionRepository::new(pool.clone());
        let dividends = DividendRepository::new(pool.clone());
        Self { pool, share_classes, shareholders, transactions, dividends }
    }

    // ---- shared helpers (used by the sibling impl blocks) -----------------------------------------

    /// Look up a share class's par value + capital/premium accounts, refusing an inactive class.
    ///
    /// RLS scope (ADR-0008), ID-only pattern: the class is identified by id alone — no company argument to
    /// scope from, and this read runs BEFORE the caller's transaction is bound. It therefore rides the
    /// ambient scope: under HTTP the request-dedicated connection carries the caller's `app.company_id`,
    /// so another company's class is simply not found. A non-request caller (job, event subscriber) MUST
    /// wrap in `with_company_scope(Some(company_id))`.
    pub(super) async fn load_class(&self, id: Uuid) -> Result<ShareClassRow, EquityError> {
        let r = self.share_classes.fetch_class(&self.pool, id).await?
            .ok_or(EquityError::NotFound("share_class"))?;
        if !r.is_active {
            return Err(EquityError::InvalidState("share class is inactive"));
        }
        Ok(ShareClassRow {
            par_value: r.par_value,
            share_capital_account_id: r.share_capital_account_id,
            share_premium_account_id: r.share_premium_account_id,
        })
    }

    /// Post a balanced journal through the GL port.
    ///
    /// The idempotency key includes the LEG — a dividend's declare and pay share the same source_id
    /// (the dividend), so keying on source_id alone would make accounting dedup the pay as a replay of
    /// the declare and silently skip it (payable never settles).
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn post(
        &self,
        sink: &dyn GlPostSink,
        company_id: &Uuid,
        leg: &str,
        source_id: Uuid,
        posting_date: NaiveDate,
        reference: Option<String>,
        description: &str,
        lines: Vec<GlPostLine>,
    ) -> Result<GlPostAck, EquityError> {
        let env = AccountingPostEnvelope {
            idempotency_key: format!("equity:{leg}:{source_id}"),
            company_id: *company_id, branch_id: None, source_type: "equity".into(), source_id,
            source_reference: reference, posting_date, currency: "IDR".into(),
            posting_type: "original".into(), description: Some(description.into()), lines,
        };
        if !env.is_balanced() {
            return Err(EquityError::Invalid("emitted posting is not balanced".into()));
        }
        sink.post(&env).await.map_err(|r| EquityError::GlRejected(r.code))
    }
}

pub(super) struct ShareClassRow {
    pub(super) par_value: Decimal,
    pub(super) share_capital_account_id: Uuid,
    pub(super) share_premium_account_id: Uuid,
}

/// Shares outstanding for a class = Σ issued − Σ bought back (transfers net to zero across holders).
///
/// Stays a free function (the SQL now lives in `ShareTransactionRepository`, but the SCOPE WRAPPER
/// belongs to this layer): a bare-pool read here returns 0 through the fence — which would read as a
/// real "no shares outstanding" and wrongly refuse every dividend. The company is on the parameter,
/// so bind it explicitly: correct for request and non-request (job) callers alike.
pub(super) async fn shares_outstanding(
    transactions: &ShareTransactionRepository,
    pool: &PgPool,
    company_id: Uuid,
    class_id: Uuid,
) -> Result<Decimal, EquityError> {
    let s = company_scope::with_company_scope(
        Some(company_id),
        transactions.shares_outstanding(pool, company_id, class_id),
    )
    .await?;
    Ok(s)
}

pub(super) async fn stage(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_type: &str,
    aggregate_type: &str,
    aggregate_id: Uuid,
    event: &EquityEvent,
) -> Result<(), EquityError> {
    let payload = serde_json::to_value(event).map_err(|e| EquityError::Invalid(e.to_string()))?;
    // Every EquityEvent carries company_id; extract it for the ADR-0011 outbox fence.
    let company_id: Uuid = payload
        .get("company_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| EquityError::Invalid("equity event missing company_id".into()))?
        .parse()
        .map_err(|e| EquityError::Invalid(format!("equity event company_id parse: {e}")))?;
    let record = backbone_outbox::OutboxRecord::new(
        event_type, aggregate_type, aggregate_id.to_string(), company_id, payload, Utc::now(),
    );
    backbone_outbox::outbox::stage(&mut **tx, "equity", &record)
        .await.map_err(|e| EquityError::Invalid(format!("outbox stage: {e}")))?;
    Ok(())
}
