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

use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

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
    pool: PgPool,
}

impl EquityWriteService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn register_share_class(&self, c: NewShareClass) -> Result<Uuid, EquityError> {
        if c.par_value < Decimal::ZERO {
            return Err(EquityError::Invalid("par value must be non-negative".into()));
        }
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO equity.share_classes
                 (id, company_id, code, name, par_value, currency, share_capital_account_id,
                  share_premium_account_id, is_active)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,true)"#,
        )
        .bind(id).bind(c.company_id).bind(&c.code).bind(&c.name).bind(c.par_value).bind(&c.currency)
        .bind(c.share_capital_account_id).bind(c.share_premium_account_id)
        .execute(&self.pool).await?;
        Ok(id)
    }

    pub async fn register_shareholder(&self, s: NewShareholder) -> Result<Uuid, EquityError> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO equity.shareholders (id, company_id, party_id, name, holder_type)
               VALUES ($1,$2,$3,$4,$5::holder_type)"#,
        )
        .bind(id).bind(s.company_id).bind(s.party_id).bind(&s.name).bind(&s.holder_type)
        .execute(&self.pool).await?;
        Ok(id)
    }

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

        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"INSERT INTO equity.share_transactions
                 (id, company_id, share_class_id, shareholder_id, txn_type, quantity, price_per_share, amount,
                  posting_reference, txn_date, gl_posted)
               VALUES ($1,$2,$3,$4,'issue'::share_txn_type,$5,$6,$7,$8,$9,true)"#,
        )
        .bind(txn_id).bind(i.company_id).bind(i.share_class_id).bind(i.shareholder_id)
        .bind(i.quantity).bind(i.price_per_share).bind(amount).bind(&i.reference).bind(i.txn_date)
        .execute(&mut *tx).await?;

        let event = EquityEvent::SharesIssued {
            transaction_id: txn_id, company_id: i.company_id, share_class_id: i.share_class_id,
            shareholder_id: i.shareholder_id, quantity: i.quantity, amount,
        };
        stage(&mut tx, "SharesIssued", "ShareTransaction", txn_id, &event).await?;
        tx.commit().await?;
        events.publish(&event);
        Ok(PostOutcome { id: txn_id, journal_id: Some(ack.journal_id), amount })
    }

    /// Transfer shares between two holders — an ownership change, NO GL. Bounds the outgoing quantity against
    /// the sender's live holding under a per-(class,holder) advisory lock, so the register can't go negative.
    pub async fn transfer_shares(&self, t: TransferShares) -> Result<Uuid, EquityError> {
        if t.quantity <= Decimal::ZERO {
            return Err(EquityError::Invalid("quantity must be positive".into()));
        }
        if t.from_shareholder_id == t.to_shareholder_id {
            return Err(EquityError::Invalid("cannot transfer to the same holder".into()));
        }
        let mut tx = self.pool.begin().await?;
        // Serialize concurrent removals from this (class, holder): the holding is a SUM with no single row to
        // lock, so an advisory xact lock is the guard.
        lock_position(&mut tx, t.company_id, t.share_class_id, t.from_shareholder_id).await?;
        let held = holding(&mut tx, t.company_id, t.share_class_id, t.from_shareholder_id).await?;
        if t.quantity > held {
            return Err(EquityError::InsufficientShares { held, requested: t.quantity });
        }
        let group = Uuid::new_v4();
        for (holder, ttype, cp) in [
            (t.from_shareholder_id, "transfer_out", t.to_shareholder_id),
            (t.to_shareholder_id, "transfer_in", t.from_shareholder_id),
        ] {
            sqlx::query(
                r#"INSERT INTO equity.share_transactions
                     (id, company_id, share_class_id, shareholder_id, txn_type, quantity, price_per_share,
                      amount, counterparty_shareholder_id, transfer_group_id, txn_date, gl_posted)
                   VALUES ($1,$2,$3,$4,$5::share_txn_type,$6,0,0,$7,$8,$9,false)"#,
            )
            .bind(Uuid::new_v4()).bind(t.company_id).bind(t.share_class_id).bind(holder).bind(ttype)
            .bind(t.quantity).bind(cp).bind(group).bind(t.txn_date)
            .execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(group)
    }

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

        let mut tx = self.pool.begin().await?;
        lock_position(&mut tx, b.company_id, b.share_class_id, b.shareholder_id).await?;
        let held = holding(&mut tx, b.company_id, b.share_class_id, b.shareholder_id).await?;
        if b.quantity > held {
            return Err(EquityError::InsufficientShares { held, requested: b.quantity });
        }
        sqlx::query(
            r#"INSERT INTO equity.share_transactions
                 (id, company_id, share_class_id, shareholder_id, txn_type, quantity, price_per_share, amount,
                  txn_date, gl_posted)
               VALUES ($1,$2,$3,$4,'buyback'::share_txn_type,$5,$6,$7,$8,true)"#,
        )
        .bind(txn_id).bind(b.company_id).bind(b.share_class_id).bind(b.shareholder_id)
        .bind(b.quantity).bind(b.price_per_share).bind(amount).bind(b.txn_date)
        .execute(&mut *tx).await?;

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

    /// Declare a dividend on a class: snapshot shares outstanding, book the liability
    /// (Dr Retained Earnings · Cr Dividend Payable). The cash goes out later via `pay_dividend`.
    pub async fn declare_dividend(
        &self,
        d: DeclareDividend,
        sink: &dyn GlPostSink,
        events: &dyn EquityEventSink,
    ) -> Result<PostOutcome, EquityError> {
        if d.per_share_amount <= Decimal::ZERO {
            return Err(EquityError::Invalid("per-share amount must be positive".into()));
        }
        let outstanding = shares_outstanding(&self.pool, d.company_id, d.share_class_id).await?;
        if outstanding <= Decimal::ZERO {
            return Err(EquityError::InvalidState("no shares outstanding to pay a dividend on"));
        }
        let total = d.per_share_amount * outstanding;
        let div_id = Uuid::new_v4();

        let lines = vec![
            GlPostLine::debit(d.retained_earnings_account_id, total).with_description("Dividend declared"),
            GlPostLine::credit(d.dividend_payable_account_id, total).with_description("Dividend payable"),
        ];
        let ack = self.post(sink, &d.company_id, "declare", div_id, d.declaration_date, None, "Dividend declaration", lines).await?;

        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"INSERT INTO equity.dividends
                 (id, company_id, share_class_id, declaration_date, per_share_amount, shares_outstanding,
                  total_amount, status, retained_earnings_account_id, dividend_payable_account_id)
               VALUES ($1,$2,$3,$4,$5,$6,$7,'declared'::dividend_status,$8,$9)"#,
        )
        .bind(div_id).bind(d.company_id).bind(d.share_class_id).bind(d.declaration_date)
        .bind(d.per_share_amount).bind(outstanding).bind(total)
        .bind(d.retained_earnings_account_id).bind(d.dividend_payable_account_id)
        .execute(&mut *tx).await?;

        let event = EquityEvent::DividendDeclared {
            dividend_id: div_id, company_id: d.company_id, share_class_id: d.share_class_id, total_amount: total,
        };
        stage(&mut tx, "DividendDeclared", "Dividend", div_id, &event).await?;
        tx.commit().await?;
        events.publish(&event);
        Ok(PostOutcome { id: div_id, journal_id: Some(ack.journal_id), amount: total })
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
        let row = sqlx::query(
            r#"SELECT company_id, total_amount, status::text AS status, dividend_payable_account_id
               FROM equity.dividends WHERE id=$1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(dividend_id).fetch_optional(&self.pool).await?
        .ok_or(EquityError::NotFound("dividend"))?;
        let status: String = row.get("status");
        if status == "paid" {
            return Err(EquityError::InvalidState("dividend already paid"));
        }
        let company_id: Uuid = row.get("company_id");
        let total: Decimal = row.get("total_amount");
        let payable_acct: Uuid = row.get("dividend_payable_account_id");

        // Claim the settlement first (CAS declared→paid), so a concurrent pay can't double-remit.
        let mut tx = self.pool.begin().await?;
        let claimed = sqlx::query(
            r#"UPDATE equity.dividends SET status='paid'::dividend_status, payment_date=$2
               WHERE id=$1 AND status='declared'::dividend_status"#,
        )
        .bind(dividend_id).bind(payment_date).execute(&mut *tx).await?;
        if claimed.rows_affected() != 1 {
            tx.rollback().await?;
            return Err(EquityError::InvalidState("dividend already paid"));
        }

        let lines = vec![
            GlPostLine::debit(payable_acct, total).with_description("Dividend paid — settle payable"),
            GlPostLine::credit(bank_account_id, total).with_description("Dividend paid — cash out"),
        ];
        // The pay posting needs its OWN source identity: accounting dedups on (source_type, source_id,
        // posting_type), and the declaration already booked source_id=dividend_id — so the pay reuses a
        // deterministic derived id (stable across a retry, distinct from the declaration).
        let pay_source_id = Uuid::new_v5(&dividend_id, b"equity-dividend-pay");
        let ack = self.post(sink, &company_id, "pay", pay_source_id, payment_date, None, "Dividend payment", lines).await
            .inspect_err(|_| { /* on GL reject the tx below rolls back, leaving the dividend declared */ })?;

        let event = EquityEvent::DividendPaid { dividend_id, company_id, total_amount: total };
        stage(&mut tx, "DividendPaid", "Dividend", dividend_id, &event).await?;
        tx.commit().await?;
        events.publish(&event);
        Ok(PostOutcome { id: dividend_id, journal_id: Some(ack.journal_id), amount: total })
    }

    // ---- public cap-table reads (completeness council) -----------------------------------------------
    // The register applies TWO different aggregation rules — a holder position (issue/transfer_in +,
    // buyback/transfer_out −) and shares-outstanding (issue +, buyback −, transfers net out). Both live
    // ONLY here; exposing them keeps a consumer (a registrar, a dividend disburser, an ownership report)
    // from re-implementing equity's sign logic across the boundary and drifting when a txn_type is added.

    /// Shares outstanding for a class = Σ issued − Σ bought back.
    pub async fn class_shares_outstanding(&self, company_id: Uuid, class_id: Uuid) -> Result<Decimal, EquityError> {
        shares_outstanding(&self.pool, company_id, class_id).await
    }

    /// Every holder's position in a class + its ownership percentage of shares outstanding.
    pub async fn holdings(&self, company_id: Uuid, class_id: Uuid) -> Result<Vec<Holding>, EquityError> {
        let rows = sqlx::query(
            r#"SELECT shareholder_id,
                      COALESCE(SUM(CASE WHEN txn_type IN ('issue','transfer_in') THEN quantity ELSE -quantity END),0) AS qty
               FROM equity.share_transactions
               WHERE company_id=$1 AND share_class_id=$2 AND (metadata->>'deleted_at') IS NULL
               GROUP BY shareholder_id HAVING
                 COALESCE(SUM(CASE WHEN txn_type IN ('issue','transfer_in') THEN quantity ELSE -quantity END),0) <> 0
               ORDER BY qty DESC"#,
        )
        .bind(company_id).bind(class_id).fetch_all(&self.pool).await?;
        let outstanding = shares_outstanding(&self.pool, company_id, class_id).await?;
        Ok(rows.iter().map(|r| {
            let quantity: Decimal = r.get("qty");
            let pct = if outstanding > Decimal::ZERO { quantity / outstanding * Decimal::from(100) } else { Decimal::ZERO };
            Holding { shareholder_id: r.get("shareholder_id"), quantity, ownership_pct: pct }
        }).collect())
    }

    /// The per-holder split of a dividend — each holder's cut = per_share × their CURRENT holding (record
    /// date = query time). This is what makes `pay_dividend` an exit a disburser can actually act on: it
    /// tells the payout system WHOM to pay and HOW MUCH. Σ allocations == total for an unchanged register.
    pub async fn dividend_allocations(&self, dividend_id: Uuid) -> Result<Vec<Allocation>, EquityError> {
        let d = sqlx::query(
            r#"SELECT company_id, share_class_id, per_share_amount
               FROM equity.dividends WHERE id=$1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(dividend_id).fetch_optional(&self.pool).await?
        .ok_or(EquityError::NotFound("dividend"))?;
        let company_id: Uuid = d.get("company_id");
        let class_id: Uuid = d.get("share_class_id");
        let per_share: Decimal = d.get("per_share_amount");
        let holdings = self.holdings(company_id, class_id).await?;
        Ok(holdings.into_iter()
            .filter(|h| h.quantity > Decimal::ZERO)
            .map(|h| Allocation { shareholder_id: h.shareholder_id, quantity: h.quantity, amount: per_share * h.quantity })
            .collect())
    }

    // ---- helpers -------------------------------------------------------------------------------------

    async fn load_class(&self, id: Uuid) -> Result<ShareClassRow, EquityError> {
        let r = sqlx::query(
            r#"SELECT par_value, currency, share_capital_account_id, share_premium_account_id, is_active
               FROM equity.share_classes WHERE id=$1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(id).fetch_optional(&self.pool).await?
        .ok_or(EquityError::NotFound("share_class"))?;
        if !r.get::<bool, _>("is_active") {
            return Err(EquityError::InvalidState("share class is inactive"));
        }
        Ok(ShareClassRow {
            par_value: r.get("par_value"),
            share_capital_account_id: r.get("share_capital_account_id"),
            share_premium_account_id: r.get("share_premium_account_id"),
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn post(
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
        // The idempotency key includes the LEG — a dividend's declare and pay share the same source_id
        // (the dividend), so keying on source_id alone would make accounting dedup the pay as a replay of
        // the declare and silently skip it (payable never settles).
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

struct ShareClassRow {
    par_value: Decimal,
    share_capital_account_id: Uuid,
    share_premium_account_id: Uuid,
}

/// A per-(company, class, holder) advisory xact lock — serializes concurrent removals so the holding check
/// and the insert are atomic against another remover.
async fn lock_position(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    company_id: Uuid,
    class_id: Uuid,
    holder_id: Uuid,
) -> Result<(), EquityError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("{company_id}:{class_id}:{holder_id}"))
        .execute(&mut **tx).await?;
    Ok(())
}

/// The holder's current position in a class = Σ (issue/transfer_in) − Σ (buyback/transfer_out).
async fn holding(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    company_id: Uuid,
    class_id: Uuid,
    holder_id: Uuid,
) -> Result<Decimal, EquityError> {
    let h: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(CASE WHEN txn_type IN ('issue','transfer_in') THEN quantity ELSE -quantity END),0)
           FROM equity.share_transactions
           WHERE company_id=$1 AND share_class_id=$2 AND shareholder_id=$3 AND (metadata->>'deleted_at') IS NULL"#,
    )
    .bind(company_id).bind(class_id).bind(holder_id).fetch_one(&mut **tx).await?;
    Ok(h)
}

/// Shares outstanding for a class = Σ issued − Σ bought back (transfers net to zero across holders).
async fn shares_outstanding(pool: &PgPool, company_id: Uuid, class_id: Uuid) -> Result<Decimal, EquityError> {
    let s: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(CASE WHEN txn_type='issue' THEN quantity
                                    WHEN txn_type='buyback' THEN -quantity ELSE 0 END),0)
           FROM equity.share_transactions
           WHERE company_id=$1 AND share_class_id=$2 AND (metadata->>'deleted_at') IS NULL"#,
    )
    .bind(company_id).bind(class_id).fetch_one(pool).await?;
    Ok(s)
}

async fn stage(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_type: &str,
    aggregate_type: &str,
    aggregate_id: Uuid,
    event: &EquityEvent,
) -> Result<(), EquityError> {
    let record = backbone_outbox::OutboxRecord::new(
        event_type, aggregate_type, aggregate_id.to_string(),
        serde_json::to_value(event).map_err(|e| EquityError::Invalid(e.to_string()))?,
        Utc::now(),
    );
    backbone_outbox::outbox::stage(&mut **tx, "equity", &record)
        .await.map_err(|e| EquityError::Invalid(format!("outbox stage: {e}")))?;
    Ok(())
}
