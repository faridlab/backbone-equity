#![cfg(feature = "unstable-write-service")]
//! Composition proof (R4): the un-composed write surface + the read contract compose end-to-end.
//! A full cap-table cycle — register · issue · transfer · declare+pay dividend — is driven through
//! `EquityWriteService` against the REAL accounting ledger (`GlAdapter`) with a capturing event sink,
//! then `EquityQueryServiceImpl` reads it back under the company scope, and the guarded router composes
//! from the module. Falsifies the council's "un-composed / orphan trait" finding.
//!
//! Requires `--features unstable-write-service` (the surface it proves is gated). Run:
//! `cargo test --test composition_proof --features unstable-write-service`.

mod common;
use common::*;

use backbone_equity::application::service::equity_write_service::*;
use backbone_equity::application::service::EquityQueryServiceImpl;
use backbone_equity::exports::{DividendId, EquityQueryService, ShareClassId, ShareholderId};
use backbone_equity::presentation::http::create_guarded_equity_routes;
use backbone_equity::EquityModule;
use backbone_orm::company_scope;
use uuid::Uuid;

async fn setup(pool: &sqlx::PgPool) -> (Uuid, EquityWriteService, EqAccounts, Uuid, Uuid) {
    let company = Uuid::new_v4();
    let svc = EquityWriteService::new(pool.clone());
    let a = eq_accounts(pool, company).await;
    let class = svc.register_share_class(NewShareClass {
        company_id: company, code: "ORD".into(), name: "Ordinary".into(), par_value: dec("1000"),
        currency: "IDR".into(),
        share_capital_account_id: a.share_capital, share_premium_account_id: a.share_premium,
    }).await.unwrap();
    let holder = svc.register_shareholder(NewShareholder {
        company_id: company, party_id: None, name: "Alice".into(), holder_type: "individual".into(),
    }).await.unwrap();
    (company, svc, a, class, holder)
}

#[tokio::test]
async fn compose_full_cycle_then_read_back_via_query_contract() {
    let pool = pool().await;
    let (company, svc, a, class, holder) = setup(&pool).await;
    let gl = GlAdapter::new(pool.clone()); // the REAL ledger
    let sink = CapturingSink::new();

    let bob = svc.register_shareholder(NewShareholder {
        company_id: company, party_id: None, name: "Bob".into(), holder_type: "individual".into(),
    }).await.unwrap();

    // issue → transfer → declare → pay, through the write surface + real GL + event sink
    svc.issue_shares(IssueShares {
        company_id: company, share_class_id: class, shareholder_id: holder, quantity: dec("100"),
        price_per_share: dec("1000"), txn_date: today(), bank_account_id: a.bank, reference: None,
    }, &gl, &sink).await.unwrap();
    svc.transfer_shares(TransferShares {
        company_id: company, share_class_id: class, from_shareholder_id: holder, to_shareholder_id: bob,
        quantity: dec("40"), txn_date: today(),
    }, &sink).await.unwrap();
    let div = svc.declare_dividend(DeclareDividend {
        company_id: company, share_class_id: class, per_share_amount: dec("50"), declaration_date: today(),
        retained_earnings_account_id: a.retained_earnings, dividend_payable_account_id: a.dividend_payable,
    }, &gl, &sink).await.unwrap();
    svc.pay_dividend(div.id, a.bank, today(), &gl, &sink).await.unwrap();

    // the read contract sees what the write surface produced (RLS → read under the producer's company scope)
    let q = EquityQueryServiceImpl::new(pool.clone());
    company_scope::with_company_scope(Some(company), async {
        assert!(q.share_class_exists(ShareClassId(class)).await.unwrap(), "class readable via contract");
        assert!(q.shareholder_exists(ShareholderId(holder)).await.unwrap(), "holder readable via contract");
        let d = q.get_dividend(DividendId(div.id)).await.unwrap().expect("dividend readable via contract");
        assert_eq!(d.per_share_amount, dec("50"));
    }).await;

    // the guarded router composes from the module (reads + masters; writes driven directly above)
    let module = EquityModule::builder().with_database(pool.clone()).build().unwrap();
    let _router = axum::Router::new().nest("/api/v1", create_guarded_equity_routes(&module));

    // every money movement + the transfer published a lifecycle event (R2 vocabulary, end-to-end)
    assert_eq!(sink.count(), 4, "issue + transfer + declare + pay each published one event");
}
