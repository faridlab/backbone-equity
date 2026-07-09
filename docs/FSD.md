# backbone-equity — FSD

## Entities
ShareClass (`company_id`, `code` (unique per company), `name`, `par_value` `>= 0`, `currency`,
`share_capital_account_id`, `share_premium_account_id` logical FKs to `accounting.Account`, `is_active`)
· Shareholder (`company_id`, `party_id?` logical FK to `party.Party`, `name`, `holder_type`) ·
ShareTransaction (`company_id`, `share_class_id` FK, `shareholder_id` FK, `txn_type`, `quantity` `> 0`,
`price_per_share` `>= 0`, `amount` `>= 0`, `counterparty_shareholder_id?` FK, `transfer_group_id?`,
`posting_reference?`, `txn_date`, `gl_posted`) · Dividend (`company_id`, `share_class_id` FK,
`declaration_date`, `payment_date?`, `per_share_amount` `>= 0`, `shares_outstanding` `>= 0` (snapshotted
at declaration), `total_amount` `>= 0`, `status`, `retained_earnings_account_id`,
`dividend_payable_account_id`). Enums: `HolderType {individual, entity}`, `ShareTxnType {issue,
transfer_in, transfer_out, buyback}`, `DividendStatus {declared, paid}`.

DB CHECKs (migration `20260712000200`, maturity council): `quantity > 0`, `price_per_share >= 0`,
`amount >= 0` on `share_transactions`; `par_value >= 0` on `share_classes`; `per_share_amount >= 0`,
`shares_outstanding >= 0`, `total_amount >= 0` on `dividends`. These stop absurd single-row values; they
do NOT (cannot) express the set-level holding bound — that lives in the write service (§Register).

## Write path (`EquityWriteService`, hand-authored, user-owned)
- `register_share_class(NewShareClass) -> Uuid` — refuses `par_value < 0`.
- `register_shareholder(NewShareholder) -> Uuid`.
- `issue_shares(IssueShares, &dyn GlPostSink, &dyn EquityEventSink) -> PostOutcome` — capital at par,
  excess to premium; refuses `price_per_share < par_value`; posts Dr Bank · Cr Share Capital ·
  [Cr Share Premium]; stages `SharesIssued`.
- `transfer_shares(TransferShares) -> Uuid` (the `transfer_group_id`) — bounds the outgoing quantity
  against the sender's live holding under the position lock; writes the paired `transfer_out`/`transfer_in`
  rows; posts NO GL.
- `buyback_shares(BuybackShares, &dyn GlPostSink, &dyn EquityEventSink) -> PostOutcome` — bounds against
  the holder's live holding under the position lock; posts Dr Share Capital (par) ·
  [Dr/Cr Retained Earnings (excess/discount)] · Cr Bank; stages `SharesBoughtBack` (reuses the
  `SharesIssued` event shape — see ADR-001 parking lot).
- `declare_dividend(DeclareDividend, &dyn GlPostSink, &dyn EquityEventSink) -> PostOutcome` — snapshots
  `class_shares_outstanding`, refuses zero outstanding; posts Dr Retained Earnings · Cr Dividend Payable;
  stages `DividendDeclared`.
- `pay_dividend(dividend_id, bank_account_id, payment_date, &dyn GlPostSink, &dyn EquityEventSink) ->
  PostOutcome` — CAS `declared -> paid`; posts Dr Dividend Payable · Cr Bank under a deterministic derived
  `source_id` (UUIDv5 of the dividend id); refuses a dividend already `paid`; stages `DividendPaid`.

### Register lock (`lock_position`)
`SELECT pg_advisory_xact_lock(hashtextextended('{company}:{class}:{holder}', 0))` inside the write
transaction, held across the holding check AND the insert, so two concurrent removals for the same
(company, class, holder) serialize — the second sees the first's committed effect (EIP-2).

### Public cap-table reads (completeness council 2026-07-12)
- `class_shares_outstanding(company_id, class_id) -> Decimal` — Σ issued − Σ bought back (transfers net
  to zero across holders).
- `holdings(company_id, class_id) -> Vec<Holding{shareholder_id, quantity, ownership_pct}>` — every
  nonzero holder position + its % of shares outstanding.
- `dividend_allocations(dividend_id) -> Vec<Allocation{shareholder_id, quantity, amount}>` — each
  holder's cut = `per_share_amount × their CURRENT holding` (record date = query time, not declaration
  date — see ADR-001 parking lot). `Σ allocations == total_amount` for an unchanged register.

Errors: `EquityError {Db, NotFound, InvalidState, Invalid, InsufficientShares{held, requested},
GlRejected}`.

## Seam (port — zero normal Cargo edge)
- **Post → accounting (`GlPostSink`, proven ESEAM-1/2):** `AccountingPostEnvelope { idempotency_key,
  company_id, source_type: "equity", source_id, posting_date, currency, posting_type: "original", lines }`.
  `is_balanced()` (Σdebit == Σcredit, non-empty) is checked before every `sink.post()` call. The
  idempotency key is `equity:{leg}:{source_id}` — the leg is part of the key because declare and pay share
  one `source_id` (the dividend). Registered as `equity` in accounting's `posting_source_type`.
- **Outbound events**: `SharesIssued`, `DividendDeclared`, `DividendPaid` staged to the outbox (same tx as
  the state change) + published via `EquityEventSink` (default `LoggingSink`, a no-op).

## Register write-path guard (`presentation/http/guarded_routes.rs`, maturity council 2026-07-12)
`create_guarded_equity_routes(&EquityModule) -> Router` — the recommended mount:
- `share_transactions` / `dividends`: **read-only** generic routes. All mutation goes through
  `EquityWriteService`, driven by the composing backend-service (which injects a `GlPostSink`) — these
  verbs cannot be self-contained library HTTP routes because they need the GL port.
- `share_classes` / `shareholders`: full generic CRUD — masters with DB-enforced invariants
  (`(company_id, code)` unique, `par_value >= 0`), no cross-row register invariant, safe to expose.

The generated (ungated) `routes()` still exists but mounts full mutable CRUD on all four entities —
**do not use it in a composing service**; use `create_guarded_equity_routes` instead.

## Test oracle
`equity_golden_cases` (6: EGC-1 par/premium split, EGC-2 issue at par has no premium line, EGC-3 transfer
moves ownership with no GL, EGC-4 buyback retires + pays, EGC-5 declare then pay settles the payable,
EGC-6 holdings + per-holder dividend allocations), `integrity_probes` (5: EIP-1 issue below par refused,
EIP-2 concurrent removal cannot go negative, EIP-3 buyback from an empty position refused, EIP-4 lifecycle
event durable via the outbox, EIP-5 negative-quantity row rejected at the DB), `equity_gl_seam` (2:
ESEAM-1 issue posts a balanced capital journal accepted by REAL accounting as `equity`, ESEAM-2 dividend
declare→pay nets the payable to zero and a second pay is refused), plus 2 guarded-route probes (GRP-1
register read routes are mounted, GRP-2 the generic register CREATE route is NOT mounted). Plus
`scripts/equity_gl_seam_roundtrip.sh` (§5 regen byte-identity on the seam files). **15 focused tests.**
