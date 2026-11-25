#!/usr/bin/env bash
# Headless end-to-end verification of the browser MVP demo flow (DEMO.md):
# Anvil + cipherlayer-vaultd + wallet `connect serve --vault-confirm`, then
# the provider-harness contract paths and the shell's selftest through the
# real Tauri process. USER_REVIEW is auto-approved via the vaultd test hook
# (CIPHERLAYER_CONFIRM_AUTO=approve — loud, logged, tests/harness only);
# the interactive demo in DEMO.md uses the real trusted dialog instead.
#
# Usage:  ./scripts/demo-headless.sh            # harness + shell selftest
#         SKIP_SHELL=1 ./scripts/demo-headless.sh   # no GUI windows
#
# Requires the wallet sibling checkout (../wallet) and Foundry's anvil.
set -euo pipefail
cd "$(dirname "$0")/.."

WALLET_DIR="${WALLET_DIR:-../wallet}"
[ -d "$WALLET_DIR" ] || { echo "wallet checkout not found at $WALLET_DIR (set WALLET_DIR)"; exit 2; }
WALLET_DIR="$(cd "$WALLET_DIR" && pwd)"

echo "== 1/5 Anvil dev chain =="
(cd "$WALLET_DIR" && make anvil-up)

echo "== 2/5 wallet processes (vaultd + connect serve --vault-confirm) =="
(cd "$WALLET_DIR" && cargo build -q -p cipherlayer-vaultd -p cipherlayer-cli)

VAULT_SOCKET="$WALLET_DIR/data/vaultd/vault.sock"
CONNECT_SOCKET="$WALLET_DIR/data/browser-connect.sock"

cleanup() {
  [ -n "${SERVE_PID:-}" ] && kill "$SERVE_PID" 2>/dev/null || true
  [ -n "${VAULTD_PID:-}" ] && kill "$VAULTD_PID" 2>/dev/null || true
}
trap cleanup EXIT

# `exec` so $! is the daemon itself and the EXIT trap really kills it.
(cd "$WALLET_DIR" && CIPHERLAYER_CONFIRM_AUTO=approve \
  CIPHERLAYER_VAULT_SOCKET="$VAULT_SOCKET" exec ./target/debug/cipherlayer-vaultd) &
VAULTD_PID=$!

(cd "$WALLET_DIR" && CIPHERLAYER_VAULT_SOCKET="$VAULT_SOCKET" \
  exec ./target/debug/cipherlayer-cli connect serve --vault-confirm) &
SERVE_PID=$!

for _ in $(seq 1 50); do
  [ -S "$CONNECT_SOCKET" ] && [ -S "$VAULT_SOCKET" ] && break
  sleep 0.2
done
[ -S "$CONNECT_SOCKET" ] || { echo "connect serve socket never appeared"; exit 1; }

export CIPHERLAYER_CONNECT_SOCKET="$CONNECT_SOCKET"
export CIPHERLAYER_CONNECT_HOST_TOKEN="$(cat "$WALLET_DIR/data/connect-host-token")"

echo "== 3/5 provider-harness: pinned-contract paths against the live stack =="
cargo run -q -p provider-harness

if [ "${SKIP_SHELL:-0}" != "1" ]; then
  echo "== 4/5 shell selftest: same flow through the real Tauri shell process =="
  CIPHERLAYER_SHELL_SELFTEST=1 cargo run -q -p cipherlayer-shell
else
  echo "== 4/5 shell selftest skipped (SKIP_SHELL=1) =="
fi

echo "== 5/5 wallet audit chain =="
(cd "$WALLET_DIR" && ./target/debug/cipherlayer-cli audit verify)

echo "== demo-headless: PASS =="
