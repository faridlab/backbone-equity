// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

pub mod equity_gl;
pub mod equity_events;
pub mod equity_write_service;
// The write surface, chunked out of equity_write_service.rs. Each is an `impl EquityWriteService` block
// over the vocabulary that stays in `equity_write_service` (so the
// `equity_write_service::{IssueShares, ...}` import paths are unchanged).
pub mod equity_register;
pub mod equity_issue;
pub mod equity_transfer;
pub mod equity_buyback;
pub mod equity_dividend;
pub mod equity_read;
// In-tree reference impl of the read contract (gated; not a deployed service).
#[cfg(feature = "unstable-write-service")]
pub mod equity_query_service_impl;
// The hand-authored cap-table write surface — NOT a committed contract. No in-workspace service composes
// it yet (no `impl` of the read contract, no `EquityWriteService::new` outside tests). Reach it via the
// deep path `application::service::equity_write_service::*` (as the tests do) or opt into the curated
// re-exports below with the `unstable-write-service` feature. The crate's default public surface is
// generic CRUD (see `lib.rs`).
#[cfg(feature = "unstable-write-service")]
pub use equity_write_service::{EquityWriteService, EquityError, NewShareClass, NewShareholder, IssueShares, TransferShares, BuybackShares, DeclareDividend, PostOutcome, Holding, Allocation};
#[cfg(feature = "unstable-write-service")]
pub use equity_events::{EquityEvent, EquityEventSink, LoggingSink};
#[cfg(feature = "unstable-write-service")]
pub use equity_gl::{AccountingPostEnvelope, GlPostLine, GlPostAck, GlPostRejected, GlPostSink};
#[cfg(feature = "unstable-write-service")]
pub use equity_query_service_impl::EquityQueryServiceImpl;
