//! Shared test helpers: a live pool, a real-accounting GL adapter, equity account seeding + ledger
//! balances, a counting GL sink, and capturing/dropping event sinks.

#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use backbone_accounting::application::service::posting_service::{PostingLine, PostingRequest, PostingService};
use backbone_equity::application::service::equity_events::{EquityEvent, EquityEventSink};
use backbone_equity::application::service::equity_gl::{
    AccountingPostEnvelope, GlPostAck, GlPostRejected, GlPostSink,
};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

pub fn dburl() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5433/backbone_equity".into())
}
pub async fn pool() -> PgPool {
    PgPool::connect(&dburl()).await.expect("connect")
}
pub fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}
pub fn today() -> chrono::NaiveDate {
    chrono::Utc::now().date_naive()
}

pub async fn account(pool: &PgPool, company: Uuid, code: &str, atype: &str, subtype: &str, normal: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO accounting.accounts
             (id, company_id, account_number, account_code, name, account_type, account_subtype,
              normal_balance, is_header, is_detail, status)
           VALUES ($1,$2,$3,$4,$5,$6::account_type,$7::account_subtype,$8::normal_balance,
                   false,true,'active'::account_status)"#,
    )
    .bind(id).bind(company).bind(code).bind(code).bind(code).bind(atype).bind(subtype).bind(normal)
    .execute(pool).await.expect("seed account");
    id
}

pub async fn balance(pool: &PgPool, acct: Uuid) -> Decimal {
    sqlx::query_scalar(
        "SELECT COALESCE(SUM(debit_amount),0) - COALESCE(SUM(credit_amount),0) FROM accounting.ledgers WHERE account_id=$1")
        .bind(acct).fetch_one(pool).await.expect("balance")
}

pub struct EqAccounts {
    pub bank: Uuid,
    pub share_capital: Uuid,
    pub share_premium: Uuid,
    pub retained_earnings: Uuid,
    pub dividend_payable: Uuid,
}
pub async fn eq_accounts(pool: &PgPool, company: Uuid) -> EqAccounts {
    EqAccounts {
        bank: account(pool, company, "1000-BANK", "asset", "bank", "debit").await,
        share_capital: account(pool, company, "3000-CAP", "equity", "paid_in_capital", "credit").await,
        share_premium: account(pool, company, "3100-PREM", "equity", "paid_in_capital", "credit").await,
        retained_earnings: account(pool, company, "3900-RE", "equity", "retained_earnings", "credit").await,
        dividend_payable: account(pool, company, "2300-DIV", "liability", "current_liability", "credit").await,
    }
}

/// ACL: equity's envelope → accounting's PostingRequest against the REAL ledger.
pub struct GlAdapter {
    pub svc: PostingService,
}
impl GlAdapter {
    pub fn new(pool: PgPool) -> Self {
        Self { svc: PostingService::new(pool) }
    }
}
#[async_trait::async_trait]
impl GlPostSink for GlAdapter {
    async fn post(&self, e: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        let mut r = PostingRequest::original(e.company_id, &e.source_type, e.source_id, e.posting_date);
        r.source_reference = e.source_reference.clone();
        r.posting_type = e.posting_type.clone();
        r.lines = e.lines.iter().map(|l| PostingLine {
            account_id: l.account_id, debit: l.debit, credit: l.credit,
            party_type: l.party_type.clone(), party_id: l.party_id,
            cost_center_id: None, project_id: None, department_id: None, description: l.description.clone(),
        }).collect();
        match self.svc.post(r, None).await {
            Ok(x) => Ok(GlPostAck { post_id: x.post_id, journal_id: x.journal_id, idempotent_reuse: x.idempotent_reuse }),
            Err(x) => Err(GlPostRejected { code: x.code().to_string(), message: x.to_string() }),
        }
    }
}

/// A counting GL sink — records each post envelope so tests can assert count + shape without a real ledger.
#[derive(Clone, Default)]
pub struct CountingGl {
    pub posts: Arc<Mutex<Vec<AccountingPostEnvelope>>>,
}
impl CountingGl {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn count(&self) -> usize {
        self.posts.lock().unwrap().len()
    }
    pub fn last(&self) -> AccountingPostEnvelope {
        self.posts.lock().unwrap().last().cloned().expect("a post")
    }
    /// The signed balance recorded against an account across all captured posts (debit +, credit −).
    pub fn account_balance(&self, acct: Uuid) -> Decimal {
        self.posts.lock().unwrap().iter().flat_map(|p| p.lines.iter())
            .filter(|l| l.account_id == acct)
            .map(|l| l.debit - l.credit).sum()
    }
}
#[async_trait::async_trait]
impl GlPostSink for CountingGl {
    async fn post(&self, e: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        assert!(e.is_balanced(), "equity emitted an unbalanced posting: {e:?}");
        self.posts.lock().unwrap().push(e.clone());
        Ok(GlPostAck { post_id: Uuid::new_v4(), journal_id: Uuid::new_v4(), idempotent_reuse: false })
    }
}

#[derive(Clone, Default)]
pub struct CapturingSink {
    pub events: Arc<Mutex<Vec<EquityEvent>>>,
}
impl CapturingSink {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn count(&self) -> usize {
        self.events.lock().unwrap().len()
    }
}
impl EquityEventSink for CapturingSink {
    fn publish(&self, event: &EquityEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

pub struct DroppingSink;
impl EquityEventSink for DroppingSink {
    fn publish(&self, _e: &EquityEvent) {}
}
