#!/usr/bin/env bash
# Tier-0 offline invariant checks (CipherLayer review loop, family guide §7).
# Deterministic, no network, no model. Run from CI's gate job and by the root
# family-review.sh orchestrator.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

fail=0
ok()  { echo "  OK   $1"; }
bad() { echo "  FAIL $1"; fail=1; }

echo "== browser family-invariants =="

# 1. No runtime/secret files tracked in git.
tracked_data="$(git ls-files | grep -E '^data/|(^|/)\.env$' || true)"
if [ -z "$tracked_data" ]; then
  ok "no runtime/.env files tracked (data/, .env)"
else
  bad "runtime files tracked in git: $tracked_data"
fi

# 2. Zero exact-64-hex literals anywhere. The browser must NEVER hold key
# material (Zone C, zero authority) — unlike the wallet, there is no
# legitimate reason for a private-key-shaped literal here at all. The
# boundary requires the run not be part of a longer hex blob (e.g. calldata),
# so it doesn't false-positive on fixture/test payloads.
hit="$(git grep -nIE '0x[0-9a-fA-F]{64}([^0-9a-fA-Fx]|$)' -- . 2>/dev/null || true)"
if [ -z "$hit" ]; then
  ok "no exact-64-hex literal anywhere (browser must never hold key material)"
else
  bad "64-hex literal found — verify it is not key material:"
  echo "$hit"
fi

# 3. No cross-repo path dependency (must consume only imports/<sibling>/,
# never link wallet or scam-guard source directly — ADR-002).
hit="$(find . -name Cargo.toml -not -path './target/*' -exec grep -nE 'path[[:space:]]*=[[:space:]]*"[^"]*/(wallet|scam-guard)/' {} + 2>/dev/null || true)"
if [ -z "$hit" ]; then
  ok "no cross-repo path dependency on wallet/scam-guard"
else
  bad "cross-repo path dependency found (forbidden by ADR-002):"
  echo "$hit"
fi

# 4. No product crate defines a signing/authorization function (Zone C, zero
# authority — every decision stays wallet-side). Covers broker-core AND the
# shell (and any future crate under crates/).
hit="$(find crates -name '*.rs' -path '*/src/*' -exec grep -nE '\bfn[[:space:]]+(authorize|sign)\(' {} + 2>/dev/null || true)"
if [ -z "$hit" ]; then
  ok "no product crate defines a sign/authorize function"
else
  bad "a product crate appears to define signing/authorization logic (forbidden):"
  echo "$hit"
fi

# 5. No unsafe Rust anywhere in the workspace.
unsafe_hit="$(find crates prototypes -name '*.rs' -path '*/src/*' -exec grep -lE '\bunsafe\s*(\{|fn|impl|trait)' {} + 2>/dev/null || true)"
if [ -z "$unsafe_hit" ]; then
  ok "no unsafe Rust in the workspace"
else
  bad "unsafe Rust found in: $unsafe_hit"
fi

if [ "$fail" = "0" ]; then
  echo "browser family-invariants: PASS"
else
  echo "browser family-invariants: FAIL" >&2
fi
exit "$fail"
