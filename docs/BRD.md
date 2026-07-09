# backbone-equity — BRD

## Documents
ShareClass (par value + share capital/premium accounts) · Shareholder (a holder, logical FK to
`party.Party`) · ShareTransaction (one register movement) · Dividend (declared → paid). Own Postgres
schema `equity`. The 10th GL producer — reaches `backbone-accounting` only through the `GlPostSink` port.

## Business rules

**BR-1 (the register — signed sum, never negative).** A holder's position in a class is the SIGNED sum of
its `share_transactions` (`issue`/`transfer_in` add, `transfer_out`/`buyback` subtract). A removal
(transfer-out or buyback) is bounded against the holder's LIVE position under a per-(company, class,
holder) `pg_advisory_xact_lock`, so two concurrent removals from the same holding cannot both pass the
check (maturity council 2026-07-12; EIP-2, proven-by-revert).

**BR-2 (the register is write-service-only — the second door).** The generated 12-endpoint generic CRUD
exposes full mutable CRUD on `share_transactions`/`dividends` with no domain logic — no lock, no holding
bound, no GL. A DB CHECK stops absurd values (`quantity > 0`) but cannot express the set-level holding
invariant (a buyback of 1000 from a holder of 100 has a positive quantity and passes every CHECK). Fixed
two ways: DB CHECKs (`quantity > 0`, all money columns `>= 0`, `par_value >= 0`) close the raw-INSERT
door (EIP-5), and `create_guarded_equity_routes` mounts `share_transactions`/`dividends` READ-ONLY so
mutation flows only through `EquityWriteService` (GRP-1/GRP-2). Masters (`ShareClass`/`Shareholder`) keep
full generic CRUD — a unique code and `par_value >= 0` are the only invariants, both DB-enforced.

**BR-3 (par/premium split on issue).** `issue_shares` books capital AT the class's par value
(`quantity × par_value` → Share Capital) and the excess (`quantity × price_per_share − capital`) → Share
Premium. An issue priced below par is refused — capital cannot be booked below its nominal value. Pricing
exactly at par posts no premium line (2 lines, not 3).

**BR-4 (transfers move ownership, not cash).** `transfer_shares` writes a paired `transfer_out` +
`transfer_in` row (sharing a `transfer_group_id`) and posts NO GL — it is an ownership change between two
holders, not a cash event. Bounded by the same live-holding lock as a buyback.

**BR-5 (buyback retires shares and pays cash).** `buyback_shares` debits Share Capital at par, debits (or
credits, if bought back below par) Retained Earnings for the excess, and credits Bank for the cash paid.
Bounded against the holder's live position under the same advisory lock.

**BR-6 (dividend: declare then pay — two legs, one payable).** `declare_dividend` snapshots the class's
current shares outstanding, computes `total = per_share_amount × outstanding`, and books
Dr Retained Earnings · Cr Dividend Payable. `pay_dividend` settles the SAME payable — Dr Dividend Payable ·
Cr Bank — CAS-gated on `status='declared'` so it settles at most once (a second `pay_dividend` on a paid
dividend is refused).

**BR-7 (one balanced journal per money event, one producer identity).** Every money-moving event posts
through `GlPostSink` as `source_type = "equity"`, idempotency-keyed on `equity:{leg}:{source_id}`. The
dividend's pay leg needs its OWN identity distinct from the declare leg — accounting dedups on
`(source_type, source_id, posting_type)`, and both legs share `source_id = dividend_id`, so the pay leg
derives a deterministic UUIDv5 (`equity-dividend-pay` namespace) — stable across a retry, distinct from
the declaration, so accounting never treats the payment as a replay of the declaration and silently skips
it (which would leave the payable unsettled forever).

**BR-8 (cap-table reads are equity's own math, not a consumer's).** `holdings`/`class_shares_outstanding`
apply the SAME sign rules the write path enforces; `dividend_allocations` derives each holder's cut as
`per_share_amount × their_current_holding`. A consumer must never re-sum raw `share_transactions` rows —
that re-implements equity's sign logic across the boundary and drifts the moment a fifth `txn_type` is
added (completeness council 2026-07-12).

## Events
`SharesIssued` (transaction_id, company/class/shareholder, quantity, amount) — also emitted (reused) for a
buyback under event_type `SharesBoughtBack`; `DividendDeclared` (dividend_id, company/class, total_amount);
`DividendPaid` (dividend_id, company_id, total_amount). Staged in the transactional outbox in the same tx
as the state change.

## Deferred (with reason)
Multi-book ledgers, options/warrants/convertibles/vesting, multi-currency par, a record-date snapshot for
allocations, issue idempotency-on-retry, an exact issue-reversal verb (PRD; council parking lots).
