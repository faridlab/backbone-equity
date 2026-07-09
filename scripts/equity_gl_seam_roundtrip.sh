#!/usr/bin/env bash
# §5 round-trip: the hand-authored equity write path (register bound + par/premium split + balanced GL) and
# its oracle survive a codegen --force regen byte-identical.
set -euo pipefail
cd "$(dirname "$0")/.."
export DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@localhost:5433/backbone_equity}"
FILES=(src/application/service/equity_gl.rs src/application/service/equity_events.rs src/application/service/equity_write_service.rs \
       tests/equity_golden_cases.rs tests/integrity_probes.rs tests/equity_gl_seam.rs)
before=$(shasum "${FILES[@]}")
echo "== regenerating (--force) =="
metaphor schema schema generate --force >/dev/null
after=$(shasum "${FILES[@]}")
if [[ "$before" != "$after" ]]; then echo "FAIL: user-owned files changed across regen"; diff <(echo "$before") <(echo "$after"); exit 1; fi
echo "OK: equity write path + oracle byte-identical across regen"
echo "== re-running the oracle + seam =="
cargo test --test equity_golden_cases --test integrity_probes --test equity_gl_seam 2>&1 | grep -E "test result"
echo "OK: §5 round-trip holds"
