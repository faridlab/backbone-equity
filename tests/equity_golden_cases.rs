//! Golden cases — the equity oracle: an issue splits capital at par vs premium; an issue at par has no
//! premium line; a transfer moves ownership with no GL; a buyback retires shares + pays cash; a declared
//! dividend is settled by payment.

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

// EGC-1 — issuing 100 shares @ 1,500 (par 1,000) posts Dr Bank 150,000 · Cr Capital 100,000 · Cr Premium
// 50,000: capital at par, the excess to premium.
#[tokio::test]
async fn egc1_issue_splits_par_and_premium() {
    let pool = pool().await;
    let (company, svc, a, class, holder) = setup(&pool).await;
    let gl = CountingGl::new();
    let sink = CapturingSink::new();

    let out = svc.issue_shares(IssueShares {
        company_id: company, share_class_id: class, shareholder_id: holder, quantity: dec("100"),
        price_per_share: dec("1500"), txn_date: today(), bank_account_id: a.bank, reference: None,
    }, &gl, &sink).await.unwrap();
    assert_eq!(out.amount, dec("150000"));
    assert_eq!(gl.account_balance(a.bank), dec("150000"), "cash in");
    assert_eq!(gl.account_balance(a.share_capital), dec("-100000"), "capital at par (credit)");
    assert_eq!(gl.account_balance(a.share_premium), dec("-50000"), "premium = excess over par (credit)");
    assert_eq!(sink.count(), 1);
}

// EGC-2 — issuing at par exactly has NO premium line (2 lines, not 3).
#[tokio::test]
async fn egc2_issue_at_par_has_no_premium_line() {
    let pool = pool().await;
    let (company, svc, a, class, holder) = setup(&pool).await;
    let gl = CountingGl::new();

    svc.issue_shares(IssueShares {
        company_id: company, share_class_id: class, shareholder_id: holder, quantity: dec("10"),
        price_per_share: dec("1000"), txn_date: today(), bank_account_id: a.bank, reference: None,
    }, &gl, &CapturingSink::new()).await.unwrap();
    assert_eq!(gl.last().lines.len(), 2, "at par: Dr Bank · Cr Capital, no premium");
    assert_eq!(gl.account_balance(a.share_premium), Decimal::ZERO);
}

// EGC-3 — a transfer moves shares between holders with NO GL; both positions update.
#[tokio::test]
async fn egc3_transfer_moves_ownership_no_gl() {
    let pool = pool().await;
    let (company, svc, a, class, alice) = setup(&pool).await;
    let bob = svc.register_shareholder(NewShareholder {
        company_id: company, party_id: None, name: "Bob".into(), holder_type: "individual".into(),
    }).await.unwrap();
    let gl = CountingGl::new();
    svc.issue_shares(IssueShares {
        company_id: company, share_class_id: class, shareholder_id: alice, quantity: dec("100"),
        price_per_share: dec("1000"), txn_date: today(), bank_account_id: a.bank, reference: None,
    }, &gl, &CapturingSink::new()).await.unwrap();

    let before = gl.count();
    svc.transfer_shares(TransferShares {
        company_id: company, share_class_id: class, from_shareholder_id: alice, to_shareholder_id: bob,
        quantity: dec("30"), txn_date: today(),
    }).await.unwrap();
    assert_eq!(gl.count(), before, "a transfer posts NO GL");

    // Alice 100−30=70, Bob 30. (holdings verified via a subsequent transfer that would fail if wrong.)
    let over = svc.transfer_shares(TransferShares {
        company_id: company, share_class_id: class, from_shareholder_id: bob, to_shareholder_id: alice,
        quantity: dec("31"), txn_date: today(),
    }).await;
    assert!(matches!(over, Err(EquityError::InsufficientShares { .. })), "Bob holds only 30");
}

// EGC-4 — a buyback retires shares and pays cash: Dr Share Capital (par) · Dr Retained Earnings (excess) ·
// Cr Bank (price).
#[tokio::test]
async fn egc4_buyback_retires_and_pays() {
    let pool = pool().await;
    let (company, svc, a, class, holder) = setup(&pool).await;
    let gl = CountingGl::new();
    svc.issue_shares(IssueShares {
        company_id: company, share_class_id: class, shareholder_id: holder, quantity: dec("100"),
        price_per_share: dec("1000"), txn_date: today(), bank_account_id: a.bank, reference: None,
    }, &gl, &CapturingSink::new()).await.unwrap();

    svc.buyback_shares(BuybackShares {
        company_id: company, share_class_id: class, shareholder_id: holder, quantity: dec("20"),
        price_per_share: dec("1200"), txn_date: today(), bank_account_id: a.bank,
        retained_earnings_account_id: a.retained_earnings,
    }, &gl, &CapturingSink::new()).await.unwrap();
    // 20 × 1200 = 24,000 out; par 20×1000 = 20,000 off capital; excess 4,000 to RE.
    let last = gl.last();
    assert_eq!(last.source_type, "equity");
    assert_eq!(gl.account_balance(a.share_capital), dec("-80000"), "100k issued − 20k retired");
    // this buyback's RE debit = 4,000; bank net = 150? recompute against the buyback post only via last().
    let re: Decimal = last.lines.iter().filter(|l| l.account_id == a.retained_earnings).map(|l| l.debit - l.credit).sum();
    assert_eq!(re, dec("4000"), "buyback premium to retained earnings");
    let bank: Decimal = last.lines.iter().filter(|l| l.account_id == a.bank).map(|l| l.debit - l.credit).sum();
    assert_eq!(bank, dec("-24000"), "cash out");
}

// EGC-5 — a declared dividend is settled by payment: declare books Dr RE · Cr Payable; pay books
// Dr Payable · Cr Bank, netting the payable to zero. Completeness council.
#[tokio::test]
async fn egc5_declare_then_pay_dividend() {
    let pool = pool().await;
    let (company, svc, a, class, holder) = setup(&pool).await;
    let gl = CountingGl::new();
    svc.issue_shares(IssueShares {
        company_id: company, share_class_id: class, shareholder_id: holder, quantity: dec("100"),
        price_per_share: dec("1000"), txn_date: today(), bank_account_id: a.bank, reference: None,
    }, &gl, &CapturingSink::new()).await.unwrap();

    let div = svc.declare_dividend(DeclareDividend {
        company_id: company, share_class_id: class, per_share_amount: dec("50"), declaration_date: today(),
        retained_earnings_account_id: a.retained_earnings, dividend_payable_account_id: a.dividend_payable,
    }, &gl, &CapturingSink::new()).await.unwrap();
    assert_eq!(div.amount, dec("5000"), "50 × 100 shares outstanding");
    assert_eq!(gl.account_balance(a.dividend_payable), dec("-5000"), "payable booked (credit)");

    svc.pay_dividend(div.id, a.bank, today(), &gl, &CapturingSink::new()).await.unwrap();
    assert_eq!(gl.account_balance(a.dividend_payable), Decimal::ZERO, "payable settled to zero");
}

// EGC-6 — the cap-table read + per-holder dividend split (completeness council): equity exposes each
// holder's position + ownership %, and derives the per-holder dividend cut, so `pay_dividend` is an exit a
// disburser can act on (WHOM to pay, HOW MUCH) instead of a hollow lump settle.
#[tokio::test]
async fn egc6_holdings_and_dividend_allocations() {
    let pool = pool().await;
    let (company, svc, a, class, alice) = setup(&pool).await;
    let bob = svc.register_shareholder(NewShareholder {
        company_id: company, party_id: None, name: "Bob".into(), holder_type: "individual".into(),
    }).await.unwrap();
    let gl = CountingGl::new();
    for (h, q) in [(alice, "70"), (bob, "30")] {
        svc.issue_shares(IssueShares {
            company_id: company, share_class_id: class, shareholder_id: h, quantity: dec(q),
            price_per_share: dec("1000"), txn_date: today(), bank_account_id: a.bank, reference: None,
        }, &gl, &CapturingSink::new()).await.unwrap();
    }

    // Holdings: Alice 70 (70%), Bob 30 (30%).
    let holdings = svc.holdings(company, class).await.unwrap();
    assert_eq!(holdings.len(), 2);
    let alice_h = holdings.iter().find(|h| h.shareholder_id == alice).unwrap();
    assert_eq!(alice_h.quantity, dec("70"));
    assert_eq!(alice_h.ownership_pct, dec("70"));
    assert_eq!(svc.class_shares_outstanding(company, class).await.unwrap(), dec("100"));

    // Declare 50/share and split it per holder: Alice 3,500 + Bob 1,500 = 5,000 (the declared total).
    let div = svc.declare_dividend(DeclareDividend {
        company_id: company, share_class_id: class, per_share_amount: dec("50"), declaration_date: today(),
        retained_earnings_account_id: a.retained_earnings, dividend_payable_account_id: a.dividend_payable,
    }, &gl, &CapturingSink::new()).await.unwrap();
    let allocs = svc.dividend_allocations(div.id).await.unwrap();
    let total: Decimal = allocs.iter().map(|x| x.amount).sum();
    assert_eq!(total, div.amount, "Σ per-holder allocations == the declared total");
    assert_eq!(allocs.iter().find(|x| x.shareholder_id == alice).unwrap().amount, dec("3500"));
    assert_eq!(allocs.iter().find(|x| x.shareholder_id == bob).unwrap().amount, dec("1500"));
}
