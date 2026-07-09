# backbone-equity — business flows & golden cases

## Flow: issue → the register grows and capital splits at par
```
issue_shares (quantity, price_per_share)
   │
   ▼  price_per_share < class.par_value? → refused (capital cannot be booked below par)
   │
   ▼  capital = quantity × par_value; premium = (quantity × price_per_share) − capital
   │
   ▼  POST via GlPostSink: Dr Bank (amount) · Cr Share Capital (capital) · [Cr Share Premium (premium)]
   │        (no premium line when price == par)
   │
   ▼  INSERT share_transactions(txn_type='issue', gl_posted=true) + STAGE SharesIssued (same tx) → commit
   │
   └▶ SharesIssued → the holder's position grew; a downstream notification/reporting consumer subscribes
```

## Flow: transfer → ownership moves, no GL
```
transfer_shares (from, to, quantity)
   │
   ▼  pg_advisory_xact_lock(company, class, from)  — serializes concurrent removals from `from`
   │
   ▼  live holding of `from` < quantity? → InsufficientShares
   │
   ▼  INSERT paired rows sharing a transfer_group_id: from → transfer_out, to → transfer_in
   │        (gl_posted=false — no journal; this is an ownership change, not a cash event)
   │
   └▶ commit — `from`'s position shrinks, `to`'s grows; shares_outstanding is UNCHANGED
```

## Flow: buyback → shares retire and cash goes out
```
buyback_shares (quantity, price_per_share)
   │
   ▼  pg_advisory_xact_lock(company, class, holder); live holding < quantity? → InsufficientShares
   │
   ▼  capital = quantity × par_value; excess = (quantity × price_per_share) − capital
   │
   ▼  INSERT share_transactions(txn_type='buyback') [same tx, still holding the lock]
   │
   ▼  POST via GlPostSink: Dr Share Capital (capital) · [Dr/Cr Retained Earnings (excess/discount)]
   │        · Cr Bank (amount)
   │
   └▶ commit → STAGE SharesBoughtBack → publish
```

## Flow: dividend — declare books the payable, pay settles it
```
declare_dividend (per_share_amount)
   │
   ▼  outstanding = class_shares_outstanding(company, class); 0? → InvalidState
   │
   ▼  total = per_share_amount × outstanding
   │
   ▼  POST: Dr Retained Earnings (total) · Cr Dividend Payable (total)
   │
   ▼  INSERT dividends(status='declared') + STAGE DividendDeclared → commit
   │
   ▼  … time passes …
   │
pay_dividend (dividend_id, bank_account_id)
   │
   ▼  CAS declared → paid (a concurrent/second pay is refused: "dividend already paid")
   │
   ▼  POST under source_id = uuid_v5(dividend_id, "equity-dividend-pay")  — a DISTINCT identity from the
   │        declare leg, so accounting's (source_type, source_id, posting_type) dedup never treats the
   │        pay as a replay of the declare and silently skips it
   │
   ▼  Dr Dividend Payable (total) · Cr Bank (total)
   │
   └▶ STAGE DividendPaid → commit → publish. Payable nets to zero.
```

## Golden cases (`tests/equity_golden_cases.rs`)
- **EGC-1 — issue splits par and premium.** 100 shares @ 1,500 (par 1,000) → Dr Bank 150,000 · Cr Capital
  100,000 · Cr Premium 50,000.
- **EGC-2 — issue at par has no premium line.** Pricing exactly at par posts 2 lines, not 3.
- **EGC-3 — transfer moves ownership, no GL.** A 30-share transfer posts zero new GL entries; the
  register correctly bounds a subsequent over-transfer (`InsufficientShares`).
- **EGC-4 — buyback retires and pays.** 20 shares @ 1,200 (par 1,000) → Cr Bank 24,000 · Dr Capital
  20,000 · Dr Retained Earnings 4,000 (the premium paid on buyback).
- **EGC-5 — declare then pay settles the payable.** Declare 50/share on 100 outstanding → payable
  5,000; pay → payable nets to zero.
- **EGC-6 — holdings + per-holder dividend allocations.** Issue 70 to Alice + 30 to Bob →
  `holdings` returns 70%/30%; a 50/share dividend's `dividend_allocations` splits Alice 3,500 + Bob
  1,500 = the declared 5,000 (`Σ allocations == total`). Completeness council 2026-07-12.

## Integrity probes (`tests/integrity_probes.rs`)
- **EIP-1 — issue below par refused.**
- **EIP-2 — MATURITY: concurrent removal cannot go negative.** Two concurrent buybacks of 80 each from a
  holding of 100 — exactly one wins (holding 20); the advisory lock serializes them. Proven-by-revert:
  removing the lock+bound lets both read 100 and both succeed, holding goes to −60.
- **EIP-3 — buyback from an empty position refused.**
- **EIP-4 — lifecycle event durable.** With the in-proc publish dropped, `DividendDeclared` is still
  staged in `equity.outbox_events`.
- **EIP-5 — MATURITY: negative quantity rejected at the DB.** A raw `INSERT ... quantity=-5` (the trust
  boundary the generic CRUD create/update endpoints sit on) is refused by the `quantity > 0` CHECK.
  Proven-by-revert: dropping the CHECK lets the row land and `holdings` returns a fabricated position.

## Seam (`tests/equity_gl_seam.rs`)
- **ESEAM-1 — issue posts a balanced capital journal accepted by REAL accounting as `equity`.** Bank
  150,000 · Share Capital −100,000 · Share Premium −50,000; net zero (double-entry holds).
- **ESEAM-2 — dividend declare→pay nets the payable to zero against the REAL ledger.** Bank ends at
  195,000 (200,000 issued − 5,000 dividend); a second `pay_dividend` on the same dividend is refused.

## Guarded-route probes (maturity council 2026-07-12)
- **GRP-1 — the register's READ routes are mounted** (`share_transactions`/`dividends` list/get work).
- **GRP-2 — the register's generic CREATE route is NOT mounted.** `POST /share_transactions` through
  `create_guarded_equity_routes` fails — the register is write-service-only.

## §5 round-trip (`scripts/equity_gl_seam_roundtrip.sh`)
Regen (`--force`) leaves the seam files byte-identical; the oracle + seam re-run green.
