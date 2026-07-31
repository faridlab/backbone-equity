<!--
council run — date: 2026-07-31 | repo type: module (inferred; no metaphor.toml) | unit: backbone-equity | focus: bounded-context-cleanliness
roster: Steelman, Skeptic, Chair (isolated subagents) · ddd-bounded-context, contract-seat, domain-expert (invited: real domain rules), yagni-business (in-context)
-->

# Council — module:backbone-equity — focus: bounded-context-cleanliness

## Best call

**Demote the un-composed surface to a non-default `unstable-write-service` Cargo feature, retract `lib.rs:29 pub use domain::entity::*`, and rewrite the `exports/services.rs:6` lie ("writes go through events"). Publish the honest contract: unguarded `GenericCrudService` x4 via `all_crud_routes()`, with `EquityWriteService` / `EquityQueryService` / GL-port / outbox / saga explicitly marked "un-composed, not a committed contract."**

This answers the user's "complete and nothing missing?" directly: the module is **not** complete as a cap-table bounded context — its validated, event-sourced write surface is unreachable from any Rust code in the workspace (orchestrator's probe: zero `EquityModule::builder` / `EquityWriteService::new` / `create_guarded_equity_routes` / `impl EquityQueryService` matches; zero `Cargo.toml` depends on `backbone-equity`). The shipped reality is unguarded CRUD that the module itself flags `#[deprecated]` (`lib.rs:99-102`). The least-downside move is to stop publishing a contract nothing realizes, so downstream code can neither couple to raw storage entity structs nor believe the event-sourcing claim.

- **Residual negative value:**
  - Time: ~2-4 hours (feature-gate the three write-service sibling files, retract one re-export, rewrite two doc-comments, regenerate rustdoc).
  - Coupling removed, not added: any future consumer is currently one `use backbone_equity::ShareTransaction;` away from binding to a DB-column-shaped storage struct instead of the `ShareTransactionDto` promise — this move closes that hole.
  - Risk surface reduced: the false "writes go through events" claim (`exports/services.rs:6`) is a correctness trap; the real path on the shipped surface can soft-delete a referenced master and emit nothing. Honesty removes the trap.
  - What stays negative: the cap-table domain stays wrong even after this move (see Disagreement #2 — transfer emits no event, buyback recorded as issuance). That is **not** fixed by the Best call; it lives in Recommendations as R2 because it is bounded to a *future* composer, not the current lens.
- **Reversibility: easy.** Feature flag flips back when a composer is wired; re-adding a re-export is one line; doc-comments are reversible. No code deleted.
- **What would flip this:** a real composing `backend-service` appears in-workspace that constructs `EquityWriteService`, injects `GlPostSink` + `EquityEventSink`, mounts `create_guarded_equity_routes`, and provides `impl EquityQueryService`. Cheap probe (already run by orchestrator): workspace grep for `EquityModule::builder | backbone_equity`. Re-run quarterly or before any `0.4.0` bump.

## Disagreement map

1. **"The rich write surface is the contract" vs "The rich write surface is theatre."**
   - Crux: does reachability matter to a bounded context's published contract?
   - Steelman / ddd-bounded-context (in part): the write service is the design intent; the entities are cohesive; the seams are polish items.
   - Skeptic / yagni-business / contract-seat: an un-composed surface is not a contract, it is internal code with `pub` on it. **Chair sides with Skeptic.** The bounded-context-cleanliness lens is *about* what crosses the module boundary; nothing crosses it because nothing depends on the module. The contract is whatever `EquityModule::all_crud_routes()` mounts — i.e. unguarded CRUD.

2. **"Fix the event vocabulary first" vs "Fix the publication first."**
   - Crux: which correctness axis is load-bearing under the current probe?
   - domain-expert: a cap table must be reconstructable from movement events; today it isn't (`equity_buyback.rs:69-74` mislabels buyback as `SharesIssued`; `transfer_shares` emits no event). Calls this a completeness gap, not a nit.
   - contract-seat / Chair: moot under no-composer. No projection consumes these events, so the wrong vocabulary has zero blast radius *today*. **Chair sides with contract-seat**, but ranks the event-vocab fix as R2 — it is the *next* blocker the instant a composer is proposed, so it should be tracked, not done.

3. **"Build the composer" vs "De-scope the composer."**
   - Crux: is the composer imminent?
   - Steelman (implied): wire the write service into a real service to realize the design.
   - yagni-business: GL seam + outbox + saga + subscriptions + versioning are all built for a payload that is incomplete and for a consumer that does not exist.
   - **Chair sides with yagni-business on the immediate move (do not build the composer under this lens),** but does **not** endorse deleting the plumbing (it is reversible as parked code; deleting is a one-way door that this lens does not require).

## Recommendations (ranked by leverage)

| # | Move | Leverage | Residual negative | Reversibility | Evidence to flip |
|---|------|----------|-------------------|---------------|------------------|
| R1 | **Best call:** feature-gate `EquityWriteService` + siblings + `EquityQueryService` trait behind non-default `unstable-write-service`; retract `lib.rs:29 pub use domain::entity::*`; rewrite `exports/services.rs:6` and `lib.rs:52-58` rustdoc to publish the honest (unguarded-CRUD) contract. | Highest — closes the storage-shape leak and the false event-sourcing claim in one push; bounded to the focus lens. | ~2-4h. Cap-table domain stays *internally* wrong (R2). Zero new coupling. | Easy (feature flag + re-export + comments). | A composer appears (workspace grep for `EquityModule::builder`). |
| R2 | Fix the movement-event vocabulary: add `EquityEvent::SharesTransferred`, add `EquityEvent::SharesBoughtBack` (or a `direction` field), stop encoding buyback as issuance at `src/application/service/equity_buyback.rs:69-74`; emit a transfer event from the transfer path. | Makes the domain reconstructable — required *before* any composer (R4). | ~1 day; touches outbox rows, possibly ADR-0011 fence. Does not change shipped surface today. | Costly (event schema is a contract change). | domain-expert's projection test: a positions projection built only from events must reproduce the cap table. |
| R3 | Delete `lib.rs:100 routes()` (the `#[deprecated]` alias), not just deprecate it. As long as it exists, `equity.routes()` is the path-of-least-resistance mount and it ships unguarded writes. | Stops the default-looking call from being the dangerous one. | Small; one-line API removal; possible downstream breakage = zero (no dependents). | One-way door (API removal) but no blast radius under the probe. | Anyone mounts `routes()` in-tree (grep: zero matches). |
| R4 | If/when a composer is proposed: it must (a) construct `EquityWriteService::new`, (b) provide real `GlPostSink` + `EquityEventSink`, (c) `impl EquityQueryService` against the DTOs, (d) mount `create_guarded_equity_routes`. Until all four exist, R1 stays in force. | Converts the module from CRUD-only to a real cap-table context. | Large effort (days–weeks); exposes R2's event-vocab bugs to real downstreams. | Costly. | R4's own precondition checklist. |
| R5 | Stop asserting "writes go through events" in `exports/services.rs:6` and module rustdoc until R4 lands; reword to "the validated/event-sourced surface is un-composed; see `unstable-write-service` feature." (Subsumed by R1 but listed because it is the single line most likely to mislead a reader who skims.) | Removes the most-cited false claim at zero cost beyond a comment. | Negligible. | Easy. | R4. |

## Parking lot

Out of focus for this council (bounded-context-cleanliness); file separately:

- **ADR-0011 outbox fence** (recent commits `cf35a46`, `78d9dd9` — `company_id` extraction). Correctness of the fence itself is a separate review; not a context-cleanliness issue.
- **Versioning surface** (`presentation/versioning/*`) — built, un-composed, same reachability problem as the write service. Apply the R1 treatment in a dedicated pass if it survives the next regen.
- **Saga executor + subscriptions** (`application/workflows/*`, `example_saga_workflow`) — YAGNI candidate; the example is `pub use`'d from `workflows/mod.rs:3` and leaks into the entity re-export through `lib.rs:41 pub use application::workflows::*`. Tracked under R1's re-export cleanup but the saga subsystem itself is out of lens.
- **gRPC + OpenAPI feature gates** — never reviewed this pass; not load-bearing for bounded-context-cleanliness.
- **Migration safety on the unguarded CRUD surface** (soft-deleting a referenced `Shareholder`) — a data-integrity concern, not a context-boundary concern.

---

**Anchors (absolute paths):**

- `src/lib.rs` — `:29` (`pub use domain::entity::*` storage leak), `:35-38` (only GenericCrudService aliases re-exported; `EquityWriteService` is NOT), `:79-102` (`all_crud_routes` and `routes()` both mount unguarded CRUD; the latter `#[deprecated]`).
- `src/exports/services.rs` — `:6` (false "writes go through events" comment), `:23-60` (orphan `EquityQueryService` trait, never impl'd in-tree).
- `src/application/service/equity_buyback.rs:69-74` — buyback encoded as `EquityEvent::SharesIssued` (the R2 fix point; not load-bearing for the Best call).
