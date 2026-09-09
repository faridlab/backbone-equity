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
//! service struct + ctor) and the shared helpers (`load_class`, `post`, `shares_outstanding`,
//! `stage`). The rest of the write surface is chunked into focused siblings, each an
//! `impl EquityWriteService` block over these same types:
//!
//! - [`super::equity_register`] — register a share class / shareholder.
//! - [`super::equity_issue`] — issue new shares (`issue_shares`).
//! - [`super::equity_transfer`] — transfer shares between holders (`transfer_shares`).
//! - [`super::equity_buyback`] — buy shares back (`buyback_shares`).
//! - [`super::equity_dividend`] — declare + pay a dividend.
//! - [`super::equity_read`] — cap-table reads (shares outstanding, holdings, dividend allocations).

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
    /// A write that must hand a company id to a still-company-keyed sibling seam (the GL
    /// posting envelope, the outbox record) found no ambient org scope carrying one.
    /// Fail-closed by design (ADR-0029): the module is tenant-agnostic and never guesses a
    /// company — the caller binds one, via the composing service's org request scope
    /// (middleware, a job wrapper, or an equivalent test harness).
    #[error("no org scope bound: this operation must run under a request scope that carries a company (the GL relay and outbox record are company-keyed)")]
    NoCompanyScope,
}

pub struct NewShareClass {
    pub code: String,
    pub name: String,
    pub par_value: Decimal,
    pub currency: String,
    pub share_capital_account_id: Uuid,
    pub share_premium_account_id: Uuid,
}

pub struct NewShareholder {
    pub party_id: Option<Uuid>,
    pub name: String,
    pub holder_type: String, // individual | entity
}

pub struct IssueShares {
    pub share_class_id: Uuid,
    pub shareholder_id: Uuid,
    pub quantity: Decimal,
    pub price_per_share: Decimal,
    pub txn_date: NaiveDate,
    pub bank_account_id: Uuid,
    pub reference: Option<String>,
}

pub struct TransferShares {
    pub share_class_id: Uuid,
    pub from_shareholder_id: Uuid,
    pub to_shareholder_id: Uuid,
    pub quantity: Decimal,
    pub txn_date: NaiveDate,
}

pub struct BuybackShares {
    pub share_class_id: Uuid,
    pub shareholder_id: Uuid,
    pub quantity: Decimal,
    pub price_per_share: Decimal,
    pub txn_date: NaiveDate,
    pub bank_account_id: Uuid,
    pub retained_earnings_account_id: Uuid,
}

pub struct DeclareDividend {
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
        Self {
            pool,
            share_classes,
            shareholders,
            transactions,
            dividends,
        }
    }

    // ---- shared helpers (used by the sibling impl blocks) -----------------------------------------

    /// Look up a share class's par value + capital/premium accounts, refusing an inactive class.
    ///
    /// The class is identified by id alone — id is globally unique, so no tenant argument exists
    /// (ADR-0029: the module carries no tenancy). The read rides the request-dedicated connection
    /// when the composing service bound one, so under a decorated deployment a row the caller's
    /// fence excludes is simply not found; with no scope bound this is a plain lookup.
    pub(super) async fn load_class(&self, id: Uuid) -> Result<ShareClassRow, EquityError> {
        let r = self
            .share_classes
            .fetch_class(&self.pool, id)
            .await?
            .ok_or(EquityError::NotFound("share_class"))?;
        if r.status != "active" {
            return Err(EquityError::InvalidState("share class is inactive"));
        }
        Ok(ShareClassRow {
            par_value: r.par_value,
            share_capital_account_id: r.share_capital_account_id,
            share_premium_account_id: r.share_premium_account_id,
        })
    }

    /// The company id for the seams that still key on one (the GL posting envelope into
    /// backbone-accounting, the outbox record). Sourced from the ambient org scope the COMPOSING
    /// service binds; absent → fail-closed. The module never guesses a company.
    pub(super) fn legacy_company_id() -> Result<Uuid, EquityError> {
        backbone_orm::org_scope::current_org_scope()
            .and_then(|s| s.legacy_company_id())
            .ok_or(EquityError::NoCompanyScope)
    }

    /// Post a balanced journal through the GL port.
    ///
    /// The idempotency key includes the LEG — a dividend's declare and pay share the same source_id
    /// (the dividend), so keying on source_id alone would make accounting dedup the pay as a replay of
    /// the declare and silently skip it (payable never settles).
    ///
    /// The envelope's company id (accounting's books owner — accounting is not stripped) is read off
    /// the ambient org scope, fail-closed when absent; see [`Self::legacy_company_id`].
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn post(
        &self,
        sink: &dyn GlPostSink,
        leg: &str,
        source_id: Uuid,
        posting_date: NaiveDate,
        reference: Option<String>,
        description: &str,
        lines: Vec<GlPostLine>,
    ) -> Result<GlPostAck, EquityError> {
        let env = AccountingPostEnvelope {
            idempotency_key: format!("equity:{leg}:{source_id}"),
            company_id: Self::legacy_company_id()?,
            branch_id: None,
            source_type: "equity".into(),
            source_id,
            source_reference: reference,
            posting_date,
            currency: "IDR".into(),
            posting_type: "original".into(),
            description: Some(description.into()),
            lines,
        };
        if !env.is_balanced() {
            return Err(EquityError::Invalid(
                "emitted posting is not balanced".into(),
            ));
        }
        sink.post(&env)
            .await
            .map_err(|r| EquityError::GlRejected(r.code))
    }
}

pub(super) struct ShareClassRow {
    pub(super) par_value: Decimal,
    pub(super) share_capital_account_id: Uuid,
    pub(super) share_premium_account_id: Uuid,
}

/// Shares outstanding for a class = Σ issued − Σ bought back (transfers net to zero across holders).
///
/// Stays a free function (the SQL lives in `ShareTransactionRepository`): the module is
/// tenant-agnostic (ADR-0029), so the read carries no company argument — under a composing
/// service's row fence the read rides the request-dedicated connection and the fence decides
/// what is visible; with no fence mounted it is a plain aggregation.
pub(super) async fn shares_outstanding(
    transactions: &ShareTransactionRepository,
    pool: &PgPool,
    class_id: Uuid,
) -> Result<Decimal, EquityError> {
    Ok(transactions.shares_outstanding(pool, class_id).await?)
}

pub(super) async fn stage(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_type: &str,
    aggregate_type: &str,
    aggregate_id: Uuid,
    event: &EquityEvent,
) -> Result<(), EquityError> {
    // The outbox RECORD is relay infrastructure and stays company-keyed (its column carries the
    // tenant key, ADR-0011): source it from the ambient org scope, fail-closed when absent. The
    // event PAYLOAD itself carries no tenant key — the module is tenant-agnostic (ADR-0029).
    let company_id = EquityWriteService::legacy_company_id()?;
    let payload = serde_json::to_value(event).map_err(|e| EquityError::Invalid(e.to_string()))?;
    let record = backbone_outbox::OutboxRecord::new(
        event_type,
        aggregate_type,
        aggregate_id.to_string(),
        company_id,
        payload,
        Utc::now(),
    );
    backbone_outbox::outbox::stage(&mut **tx, "equity", &record)
        .await
        .map_err(|e| EquityError::Invalid(format!("outbox stage: {e}")))?;
    Ok(())
}
