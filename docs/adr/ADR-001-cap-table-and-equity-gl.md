# ADR-001 — The cap table, the register's two doors, and equity's GL seam

Status: accepted · 2026-07-12 · Financials-park (Tier 5; the 10th GL producer)

## Context
Once a company issues shares, a shareholder exists; once it buys shares back or declares a dividend, the
ledger owes money against a specific slice of ownership. `backbone-equity` owns the cap table
(ShareClass/Shareholder/ShareTransaction/Dividend) and posts its own balanced journal — like the other
nine GL producers — through the `GlPostSink` port, zero normal Cargo edge to `backbone-accounting`.

## Decision
1. **A holder's position is the SIGNED sum of register movements, never negative.** `issue`/`transfer_in`
   add, `transfer_out`/`buyback` subtract. A removal is bounded against the LIVE holding under a
   per-(company, class, holder) `pg_advisory_xact_lock`, held across the check AND the insert, so two
   concurrent removals from the same holding can't both pass (EIP-2, proven-by-revert).
2. **Capital is booked AT PAR; the excess goes to Share Premium.** `issue_shares` refuses a price below
   par — capital cannot be booked below its nominal value. Pricing exactly at par omits the premium line.
3. **The register has TWO write doors, and the guarantees must live at BOTH (maturity council
   2026-07-12).** The write service is one door (locked, bounded, GL-posting). The auto-wired 12-endpoint
   generic CRUD stack is the other — it writes `share_transactions` directly with no lock, no holding
   bound, no sign logic, no GL. Two facts made this load-bearing:
   - The schema's `@non_negative` never reached DDL — every numeric column was a bare `NUMERIC NOT NULL`.
   - `holding()`/`shares_outstanding()` are signed SUMs; a single ordinary `POST
     /share_transactions {txn_type:"buyback", quantity:-5}` through the generic door **adds** 5 shares —
     silently, in one benign request — and the next `declare_dividend` books a payable on a fabricated
     share count. The journal is internally balanced, so nothing complains.
   - A column CHECK stops absurd single-row *values* but cannot express the set-level holding bound (a
     buyback of 1000 from a holder of 100 has a positive quantity and passes every CHECK).
   **Fix, two-part:** (a) DB CHECKs (`quantity > 0`, all money columns `>= 0`, `par_value >= 0`, migration
   `20260712000200`) close the raw-INSERT / any-writer door (EIP-5, proven-by-revert: dropping the
   `quantity > 0` CHECK lets a `-5` buyback land and `holdings` returns a fabricated position). (b)
   `create_guarded_equity_routes` mounts `share_transactions`/`dividends` READ-ONLY — all mutation flows
   through `EquityWriteService` (GRP-1/GRP-2, proven-by-revert: merging the full generic route makes a
   `POST /share_transactions` succeed and the probe goes red). Masters (`ShareClass`/`Shareholder`) keep
   full CRUD — their only invariants (unique code, `par_value >= 0`) are DB-enforced and safe to expose.
4. **One balanced journal per money event, one producer identity.** `source_type = "equity"`; the
   idempotency key includes the LEG (`equity:{leg}:{source_id}`) because a dividend's declare and pay
   share one `source_id` (the dividend id) — keying on `source_id` alone would make accounting dedup the
   pay as a replay of the declare and silently skip it, leaving the payable unsettled forever. The pay leg
   posts under a deterministic UUIDv5 derived from the dividend id (stable across a retry, distinct from
   the declaration).
5. **The cap table's own math is promoted to public reads (completeness council 2026-07-12).**
   `holdings`/`class_shares_outstanding`/`dividend_allocations` expose the SAME sign logic the write
   service enforces internally, so `pay_dividend` — declared on a lump total, disbursed pro-rata per
   holder — drives a reconcilable per-holder payout instead of a hollow lump settle (EGC-6,
   proven-by-revert: without the reads a consumer must re-sum raw rows and cannot answer whom to pay).

## Consequences
- The register cannot be corrupted from either door: the DB refuses absurd single-row values, and the
  generic mutation path for the register entities is simply not mounted.
- A consumer never re-implements equity's sign logic; it calls `holdings`/`dividend_allocations` and gets
  equity's own authoritative answer, which stays correct when a fifth `txn_type` is added.
- Proven vs REAL `backbone-accounting` (ESEAM-1/2); durable across a lost publish (EIP-4); survives regen
  (§5, `equity_gl_seam_roundtrip.sh`).

## Parking lot (each with a gate)
- **Issue idempotency on retry** — `issue_shares` mints a fresh `txn_id` per call; a client retry after a
  lost ack re-posts the capital journal under a new `source_id` (a double-issue). Gate: a caller-supplied
  idempotency key with a partial-unique index (the dividend pay leg already derives a stable id — the
  same treatment fits issue/buyback).
- **GL-before-tx crash window** — `issue`/`declare` post the journal before the DB tx that records the
  movement; a crash between strands a balanced journal with no register row. Gate: a reconciling reaper on
  `gl_posted` (the same pattern other GL producers park).
- **Record-date snapshot for allocations** — `dividend_allocations` uses the CURRENT register; a transfer
  between declaration and payment shifts the split. Correct for a "record date = now" policy. Gate: a
  `record_date` on the dividend + a point-in-time holdings read if a declaration-date record date is
  required.
- **Buyback event fidelity** — `buyback_shares` reuses the `SharesIssued` event shape, so a consumer
  can't distinguish grow-vs-shrink from the event alone. Gate: a dedicated `SharesBoughtBack` event.
- **Precision to DDL** — restore the declared `NUMERIC(20,4)` scale (the CHECKs cover sign, not scale).
- **Exact issue reversal** — a correction verb; a buyback partially unwinds an over-issue today. Gate: a
  consumer that needs an exact reversal.
