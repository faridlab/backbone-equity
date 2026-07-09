# backbone-equity — PRD

Financials-park (Tier 5) · the **cap table + equity accounting** · the 10th GL producer.

## Why
Once a company issues shares, buys some back, or declares a dividend, someone owns a slice of the
business and the ledger owes it money. `backbone-equity` is the cap table: who holds how many shares of
which class, and the accounting that follows a movement — capital booked at par with the excess to share
premium on issue, a balanced retirement journal on buyback, a payable booked on dividend declaration and
settled on payment. It is the tenth module to post its own balanced journal through the shared GL-posting
port — zero normal Cargo edge to `backbone-accounting`.

## Scope (KEEP)
- **ShareClass** — a category of shares (ordinary/preferred) with a par value and the two equity accounts
  (share capital, share premium) capital splits across on issue.
- **Shareholder** — a holder of shares; a logical ref to `backbone-party`, region-neutral.
- **ShareTransaction** — the register: one movement (issue / transfer_in / transfer_out / buyback). A
  holder's position is the SIGNED sum of its movements and can never go negative.
- **Dividend** — declared → paid. Declaration books the liability at the class's current shares
  outstanding; payment settles it.
- **The write path** (`EquityWriteService`) — `register_share_class`, `register_shareholder`,
  `issue_shares`, `transfer_shares`, `buyback_shares`, `declare_dividend`, `pay_dividend` — the only door
  onto the register (guarded routes; §BRD-2).
- **Cap-table reads** — `holdings`, `class_shares_outstanding`, `dividend_allocations` — equity's own
  sign logic, promoted to the public surface so a consumer never re-implements it.
- **The GL seam** (`GlPostSink`) — one balanced posting per money-moving event, `source_type = "equity"`.

## Non-goals (CUT / DEFER)
- Multi-book / parallel ledgers.
- Share options, warrants, convertibles, vesting schedules.
- Multi-currency par value — par/issue amounts are in the class's functional currency; FX is corporate's job.
- A record-date snapshot for dividend allocations — allocations use the CURRENT register (record date =
  query time), not the declaration date.
- Issue idempotency-on-retry — a client retry after a lost ack re-posts the capital journal under a new id.
- An exact issue-reversal verb — a buyback partially unwinds an over-issue today.

## Success criteria
- A holder's position never goes negative, even under concurrent removals from the same (company, class,
  holder) — proven under real concurrency (EIP-2).
- Capital is always booked at par with the excess to premium; an issue below par is refused.
- Every money-moving event posts ONE balanced journal accepted by the REAL `backbone-accounting` as
  `source_type='equity'` (ESEAM-1/2). Zero normal Cargo edge; survives a full codegen regen (§5).
- The register is mutated ONLY through `EquityWriteService` — the generic CRUD door is mounted read-only
  for `share_transactions`/`dividends` (GRP-1/GRP-2).
- `pay_dividend` drives a reconcilable per-holder disbursement, not a hollow lump settle (EGC-6).
