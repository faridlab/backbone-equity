# backbone-equity — Extension Guide

## Public surface (stable)
- **GL port** (`application::service::equity_gl`): `GlPostSink` (the seam a composing service implements
  over `backbone-accounting`'s `PostingService`), `AccountingPostEnvelope`, `GlPostLine`, `GlPostAck`,
  `GlPostRejected`. `source_type = "equity"` — register it in accounting's `posting_source_type`.
- **Events** (`application::service::equity_events`): `EquityEvent` union (`SharesIssued`,
  `DividendDeclared`, `DividendPaid`), `EquityEventSink`, the default no-op `LoggingSink`.
- **Write path** (`application::service::equity_write_service::EquityWriteService`):
  `register_share_class`, `register_shareholder`, `issue_shares`, `transfer_shares`, `buyback_shares`,
  `declare_dividend`, `pay_dividend`, plus reads `class_shares_outstanding`, `holdings`,
  `dividend_allocations`. DTOs: `NewShareClass`, `NewShareholder`, `IssueShares`, `TransferShares`,
  `BuybackShares`, `DeclareDividend`, `PostOutcome`, `Holding`, `Allocation`.
- **Guarded routes** (`presentation::http::guarded_routes::create_guarded_equity_routes`): the
  RECOMMENDED mount — full CRUD on `ShareClass`/`Shareholder`, READ-ONLY on `ShareTransaction`/`Dividend`.
- **Durability**: the lifecycle event is staged in `equity.outbox_events` in the same tx as the register
  write / status update.

## How a consuming service uses equity
Mount `create_guarded_equity_routes(&equity_module)`, NOT the generated `routes()` — the generated router
gives full mutable CRUD on the register, which bypasses the holding bound and the GL post entirely.
Implement `GlPostSink::post` over `backbone-accounting`'s `PostingService`, mapping
`AccountingPostEnvelope` into accounting's `PostingRequest` (an ACL adapter — the envelope is the wire
contract, duplicated per producer by design; equity never imports `backbone-accounting`). Drive
`issue_shares`/`transfer_shares`/`buyback_shares`/`declare_dividend`/`pay_dividend` from your own
application/command layer, injecting the `GlPostSink` and an `EquityEventSink`.

To compute WHO gets paid on a dividend, call `dividend_allocations(dividend_id)` — do not re-sum
`share_transactions` yourself; the sign logic (`issue`/`transfer_in` add, `transfer_out`/`buyback`
subtract) lives only inside `EquityWriteService` and is not part of any public contract on the entity
itself.

## Not a contract
- The 12 generated CRUD endpoints per entity are convenience scaffolding, still reachable via the
  generated (ungated) `routes()`. Do **not** wire that router in a composing service — it lets a plain
  `POST /share_transactions {txn_type:"buyback", quantity:-5}` corrupt the cap table (see ADR-001). Use
  `create_guarded_equity_routes` and drive register mutations through `EquityWriteService`.
- `// <<< CUSTOM` blocks preserve local edits only; not a cross-module extension point.

## Invariants a consumer must not break
- Never write `equity.share_transactions` or `equity.dividends` directly, generic CRUD or otherwise —
  `EquityWriteService` is the only door onto the register.
- A holder's position is the signed sum of its movements and must never go negative; don't attempt a
  transfer/buyback larger than `holdings()` reports for that holder.
- Every money-moving call needs a `GlPostSink` — an issue, buyback, or dividend leg cannot commit its
  register row without also posting (or being refused by) the GL.
- `pay_dividend` settles a `declared` dividend exactly once; a second call on a `paid` dividend errors,
  it does not silently no-op.
- Par/issue amounts are in the share class's functional currency; equity does not convert — do not pass
  cross-currency amounts expecting FX handling.
