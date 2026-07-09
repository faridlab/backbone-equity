//! Guarded route composition — the RECOMMENDED way to mount the equity module (hand-authored, user-owned).
//!
//! Closes the register's SECOND door (maturity council 2026-07-12). The generated `routes()` exposes full
//! mutable generic CRUD on `share_transactions` and `dividends`, backed by generic services with NO domain
//! logic. That path never enters `EquityWriteService`, so it bypasses the signed-sum holding bound (a
//! `buyback` of 1000 from a holder of 100 has a positive quantity and passes every column CHECK) and the GL
//! post — a caller could POST a raw register row and corrupt the cap table + the ledger's agreement with it.
//! A column CHECK can stop absurd VALUES; it cannot express the set-level holding invariant. So the register
//! must be mutated ONLY through the write service.
//!
//! Guarded surface:
//!   - **ShareTransaction / Dividend**: READ ONLY. All mutation flows through `EquityWriteService`
//!     (`issue_shares` / `transfer_shares` / `buyback_shares` / `declare_dividend` / `pay_dividend`), which
//!     the composing backend-service drives with an injected `GlPostSink` (equity posts GL through the port,
//!     so these verbs cannot be self-contained HTTP routes in the library).
//!   - **ShareClass / Shareholder**: full generic CRUD — masters with a DB-enforced unique code and a
//!     `par_value >= 0` CHECK, no cross-entity register invariant, safe to expose directly.

use axum::Router;

use super::{
    create_dividend_read_routes, create_share_class_routes, create_share_transaction_read_routes,
    create_shareholder_routes,
};
use crate::EquityModule;

/// The recommended mount: read-only register/dividends + full master CRUD. The register-mutating verbs are
/// driven through `EquityWriteService` by the composing service, never via generic CRUD.
pub fn create_guarded_equity_routes(m: &EquityModule) -> Router {
    Router::new()
        .merge(create_share_transaction_read_routes(m.share_transaction_service.clone()))
        .merge(create_dividend_read_routes(m.dividend_service.clone()))
        .merge(create_share_class_routes(m.share_class_service.clone()))
        .merge(create_shareholder_routes(m.shareholder_service.clone()))
}
