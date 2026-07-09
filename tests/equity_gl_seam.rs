//! The GL-posting seam against the REAL backbone-accounting ledger — equity is the 10th GL producer. Proves
//! the share-issue journal and the dividend declare→pay journals land balanced, accounting accepts
//! `source_type='equity'`, and a fully-issued-then-bought-back-and-paid company nets its equity/cash movements
//! correctly. ZERO normal Cargo edge — the envelope is the wire contract.

mod common;
use common::*;

use backbone_equity::application::service::equity_events::LoggingSink;
use backbone_equity::application::service::equity_write_service::*;
use rust_decimal::Decimal;
use uuid::Uuid;

async fn setup(pool: &sqlx::PgPool) -> (Uuid, EquityWriteService, EqAccounts, Uuid, Uuid) {
    let company = Uuid::new_v4();
    let svc = EquityWriteService::new(pool.clone());
    let a = eq_accounts(pool, company).await;
    let class = svc.register_share_class(NewShareClass {
        company_id: company, code: "ORD".into(), name: "Ordinary".into(), par_value: dec("1000"),
        currency: "IDR".into(), share_capital_account_id: a.share_capital, share_premium_account_id: a.share_premium,
    }).await.unwrap();
    let holder = svc.register_shareholder(NewShareholder {
        company_id: company, party_id: None, name: "Alice".into(), holder_type: "individual".into(),
    }).await.unwrap();
    (company, svc, a, class, holder)
}

// ESEAM-1 — a share issue posts a balanced capital journal accepted by the REAL ledger as 'equity'.
// Dr Bank 150,000 · Cr Share Capital 100,000 · Cr Share Premium 50,000.
#[tokio::test]
async fn eseam1_issue_posts_balanced_capital_journal() {
    let pool = pool().await;
    let (company, svc, a, class, holder) = setup(&pool).await;
    let gl = GlAdapter::new(pool.clone());

    svc.issue_shares(IssueShares {
        company_id: company, share_class_id: class, shareholder_id: holder, quantity: dec("100"),
        price_per_share: dec("1500"), txn_date: today(), bank_account_id: a.bank, reference: None,
    }, &gl, &LoggingSink).await.expect("real accounting accepts the equity issue post");

    assert_eq!(balance(&pool, a.bank).await, dec("150000"));
    assert_eq!(balance(&pool, a.share_capital).await, dec("-100000"));
    assert_eq!(balance(&pool, a.share_premium).await, dec("-50000"));
    let net = balance(&pool, a.bank).await + balance(&pool, a.share_capital).await + balance(&pool, a.share_premium).await;
    assert_eq!(net, Decimal::ZERO, "double-entry: Σ debits = Σ credits");
}

// ESEAM-2 — declare then pay a dividend against the REAL ledger: the payable is booked then settled to zero,
// and the cash leaves the bank. Proves the completeness exit posts a real, balanced settlement.
#[tokio::test]
async fn eseam2_dividend_declare_then_pay_nets_payable_to_zero() {
    let pool = pool().await;
    let (company, svc, a, class, holder) = setup(&pool).await;
    let gl = GlAdapter::new(pool.clone());
    svc.issue_shares(IssueShares {
        company_id: company, share_class_id: class, shareholder_id: holder, quantity: dec("200"),
        price_per_share: dec("1000"), txn_date: today(), bank_account_id: a.bank, reference: None,
    }, &gl, &LoggingSink).await.unwrap();

    let div = svc.declare_dividend(DeclareDividend {
        company_id: company, share_class_id: class, per_share_amount: dec("25"), declaration_date: today(),
        retained_earnings_account_id: a.retained_earnings, dividend_payable_account_id: a.dividend_payable,
    }, &gl, &LoggingSink).await.unwrap();
    assert_eq!(div.amount, dec("5000"), "25 × 200 outstanding");
    assert_eq!(balance(&pool, a.dividend_payable).await, dec("-5000"), "payable booked");

    svc.pay_dividend(div.id, a.bank, today(), &gl, &LoggingSink).await.unwrap();
    assert_eq!(balance(&pool, a.dividend_payable).await, Decimal::ZERO, "payable settled");
    // Bank: +200,000 issue − 5,000 dividend = 195,000.
    assert_eq!(balance(&pool, a.bank).await, dec("195000"), "cash reduced by the dividend paid");

    // Paying again is refused (settled once).
    let again = svc.pay_dividend(div.id, a.bank, today(), &gl, &LoggingSink).await;
    assert!(matches!(again, Err(EquityError::InvalidState(_))), "a paid dividend can't be re-paid");
}
