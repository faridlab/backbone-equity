//! Integrity probes — the equity invariants: an issue below par is refused; a position can NEVER go
//! negative (even under concurrent removals — the maturity invariant); a paid dividend can't be re-paid;
//! the lifecycle event is durable.

mod common;
use common::*;

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

// EIP-1 — an issue below par value is refused (capital cannot be booked below its nominal).
#[tokio::test]
async fn eip1_issue_below_par_refused() {
    let pool = pool().await;
    let (company, svc, a, class, holder) = setup(&pool).await;
    let r = svc.issue_shares(IssueShares {
        company_id: company, share_class_id: class, shareholder_id: holder, quantity: dec("10"),
        price_per_share: dec("900"), txn_date: today(), bank_account_id: a.bank, reference: None,
    }, &CountingGl::new(), &CapturingSink::new()).await;
    assert!(matches!(r, Err(EquityError::Invalid(_))), "below-par issue refused");
}

// EIP-2 — MATURITY: two CONCURRENT buybacks of 80 each from a holding of 100 cannot both succeed — the
// per-(class,holder) advisory lock serializes them, so exactly one wins (holding 20) and the register never
// goes negative. Proven-by-revert: removing the lock+bound lets both read 100, both remove → holding −60.
#[tokio::test]
async fn eip2_concurrent_removal_cannot_go_negative() {
    let pool = pool().await;
    let (company, svc, a, class, holder) = setup(&pool).await;
    svc.issue_shares(IssueShares {
        company_id: company, share_class_id: class, shareholder_id: holder, quantity: dec("100"),
        price_per_share: dec("1000"), txn_date: today(), bank_account_id: a.bank, reference: None,
    }, &CountingGl::new(), &CapturingSink::new()).await.unwrap();

    let svc = std::sync::Arc::new(svc);
    let (s1, s2) = (svc.clone(), svc.clone());
    let bb = move |s: std::sync::Arc<EquityWriteService>| async move {
        s.buyback_shares(BuybackShares {
            company_id: company, share_class_id: class, shareholder_id: holder, quantity: dec("80"),
            price_per_share: dec("1000"), txn_date: today(), bank_account_id: a.bank,
            retained_earnings_account_id: a.retained_earnings,
        }, &CountingGl::new(), &CapturingSink::new()).await
    };
    let (r1, r2) = tokio::join!(bb(s1), bb(s2));
    let wins = [r1.is_ok(), r2.is_ok()].iter().filter(|x| **x).count();
    assert_eq!(wins, 1, "exactly one buyback of 80 from 100 succeeds");

    // The register is non-negative — the surviving holding is 20.
    let held: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(CASE WHEN txn_type IN ('issue','transfer_in') THEN quantity ELSE -quantity END),0)
           FROM equity.share_transactions WHERE share_class_id=$1 AND shareholder_id=$2"#,
    )
    .bind(class).bind(holder).fetch_one(&pool).await.unwrap();
    assert_eq!(held, dec("20"), "holding never went negative");
}

// EIP-3 — a buyback from a holder with a zero position is refused.
#[tokio::test]
async fn eip3_buyback_with_no_holding_refused() {
    let pool = pool().await;
    let (company, svc, a, class, holder) = setup(&pool).await;
    let r = svc.buyback_shares(BuybackShares {
        company_id: company, share_class_id: class, shareholder_id: holder, quantity: dec("1"),
        price_per_share: dec("1000"), txn_date: today(), bank_account_id: a.bank,
        retained_earnings_account_id: a.retained_earnings,
    }, &CountingGl::new(), &CapturingSink::new()).await;
    assert!(matches!(r, Err(EquityError::InsufficientShares { .. })), "cannot buy back from an empty position");
}

// EIP-4 — the lifecycle event is durable: with the in-proc publish dropped, DividendDeclared is still staged
// in the outbox for the relay.
#[tokio::test]
async fn eip4_lifecycle_event_durable() {
    let pool = pool().await;
    let (company, svc, a, class, holder) = setup(&pool).await;
    let gl = CountingGl::new();
    svc.issue_shares(IssueShares {
        company_id: company, share_class_id: class, shareholder_id: holder, quantity: dec("100"),
        price_per_share: dec("1000"), txn_date: today(), bank_account_id: a.bank, reference: None,
    }, &gl, &DroppingSink).await.unwrap();
    let div = svc.declare_dividend(DeclareDividend {
        company_id: company, share_class_id: class, per_share_amount: dec("50"), declaration_date: today(),
        retained_earnings_account_id: a.retained_earnings, dividend_payable_account_id: a.dividend_payable,
    }, &gl, &DroppingSink).await.unwrap();

    let staged: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM equity.outbox_events WHERE aggregate_id=$1 AND event_type='DividendDeclared'")
        .bind(div.id.to_string()).fetch_one(&pool).await.unwrap();
    assert_eq!(staged, 1, "DividendDeclared durably staged despite the dropped publish");
}

// EIP-5 — MATURITY: the register's @non_negative was never emitted to DDL. A NEGATIVE quantity inserted
// through ANY writer that bypasses the write service (the auto-wired generic CRUD stack, a raw INSERT)
// would poison the signed holding SUM — a `buyback` of −5 ADDS 5 shares — corrupting the cap table AND the
// bound the concurrency guard reads. The `quantity > 0` DB CHECK closes that door. Proven-by-revert:
// dropping the CHECK lets the row land and `holdings` returns a fabricated position.
#[tokio::test]
async fn eip5_negative_quantity_rejected_at_db() {
    let pool = pool().await;
    let (company, _svc, _a, class, holder) = setup(&pool).await;
    // A raw INSERT — the trust boundary the generic CRUD create/update endpoints sit on.
    let bad = sqlx::query(
        r#"INSERT INTO equity.share_transactions
             (id, company_id, share_class_id, shareholder_id, txn_type, quantity, price_per_share, amount, txn_date, gl_posted)
           VALUES (gen_random_uuid(), $1, $2, $3, 'buyback'::share_txn_type, -5, 0, 0, DATE '2026-01-01', false)"#,
    )
    .bind(company).bind(class).bind(holder)
    .execute(&pool).await;
    assert!(bad.is_err(), "a negative-quantity register row cannot be inserted through any writer");
}
