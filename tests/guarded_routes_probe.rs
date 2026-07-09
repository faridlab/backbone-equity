//! Router probe (maturity council 2026-07-12): the RECOMMENDED guarded composition mounts the register
//! (`share_transactions`) READ-only — a caller cannot POST/PATCH a raw register row that would bypass the
//! signed-sum holding bound in `EquityWriteService`. The generic `routes()` DOES expose that mutation; the
//! guarded router must not. Proven-by-revert: merging the full `create_share_transaction_routes` into the
//! guarded router (instead of the read-only variant) makes the POST succeed and this probe red.

mod common;
use common::*;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use backbone_equity::presentation::http::create_guarded_equity_routes;
use backbone_equity::EquityModule;
use tower::ServiceExt; // oneshot

async fn guarded() -> axum::Router {
    let module = EquityModule::builder().with_database(pool().await).build().expect("build module");
    axum::Router::new().nest("/api/v1", create_guarded_equity_routes(&module))
}

// GRP-1 — the register list (GET) IS mounted on the guarded router.
#[tokio::test]
async fn grp1_register_read_is_mounted() {
    let app = guarded().await;
    let res = app.oneshot(
        Request::builder().method("GET").uri("/api/v1/share_transactions").body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_ne!(res.status(), StatusCode::NOT_FOUND, "GET /share_transactions is mounted (read allowed)");
    assert_ne!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
}

// GRP-2 — a generic register CREATE (POST) is NOT mounted on the guarded router: the register is mutated
// only through EquityWriteService. The path exists (read is mounted) so an absent POST is 405.
#[tokio::test]
async fn grp2_register_generic_create_is_not_mounted() {
    let app = guarded().await;
    let res = app.oneshot(
        Request::builder().method("POST").uri("/api/v1/share_transactions")
            .header("content-type", "application/json").body(Body::from("{}")).unwrap()
    ).await.unwrap();
    assert!(
        res.status() == StatusCode::METHOD_NOT_ALLOWED || res.status() == StatusCode::NOT_FOUND,
        "the generic register mutation must NOT be reachable on the guarded router (got {})", res.status()
    );
}
