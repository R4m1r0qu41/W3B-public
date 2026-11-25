# CipherLayer Browser MVP — Demo Runbook

End-to-end demo of the trust spine through the Tauri shell: a fixture dapp
connects, requests a transaction, the wallet pipeline decodes/simulates/
risk-scores it, **you** approve or reject on the wallet-owned trusted surface
(a macOS system dialog rendered by `cipherlayer-vaultd` — never by the
browser), and the result lands on local Anvil with a hash-chained audit entry.

Everything runs locally: Anvil dev chain, well-known Anvil test keys (MOCKED
custody, wallet ADR-005), no real funds, no remote RPC.

## Prerequisites

- macOS (the trusted confirmation surface tier v1 is macOS-only, wallet ADR-012).
- Rust stable toolchain.
- [Foundry](https://getfoundry.sh) (`anvil` on PATH).
- Sibling checkouts: this repo and `../wallet` (`Dev/CipherLayer/{browser,wallet}`).

## One-time setup: the anti-spoof phrase

The genuine confirmation dialog always displays a secret phrase only the
Vault side can read. Create it once (owner-only):

```sh
cd ../wallet
mkdir -p data/vaultd
echo "your personal phrase here" > data/vaultd/confirm-phrase
chmod 600 data/vaultd/confirm-phrase
```

## Run the demo (three terminals)

**Terminal 1 — dev chain + Vault process:**

```sh
cd ../wallet
make anvil-up
CIPHERLAYER_VAULT_SOCKET=./data/vaultd/vault.sock cargo run -p cipherlayer-vaultd
```

**Terminal 2 — wallet browser-connect transport (protocol v0.3):**

```sh
cd ../wallet
CIPHERLAYER_VAULT_SOCKET=./data/vaultd/vault.sock \
  cargo run -p cipherlayer-cli -- connect serve --vault-confirm
```

`--vault-confirm` routes USER_REVIEW to the vaultd system dialog. The serve
process prints where the v0.3 host-attestation token lives
(`../wallet/data/connect-host-token`); the shell picks it up automatically.

**Terminal 3 — the shell (this repo):**

```sh
cargo run -p cipherlayer-shell
```

Two windows open:

- **CipherLayer Shell — evidence panel** (left): wallet link status, host
  attestation, granted accounts, the latest signing review evidence, and an
  activity feed. Display only — it cannot approve anything.
- **Dapp capsule** (right): the fixture dapp, hostile-by-assumption Zone B
  content at the host-assigned origin `capsule://dapp`, with the EIP-1193
  provider injected before page scripts.

## What to click

1. **`1 · detect provider`** — the page finds `window.cipherlayer`.
2. **`2 · eth_requestAccounts`** — connect. The wallet grants a scoped,
   expiring permission for the capsule; the evidence panel shows the granted
   account and "attested as launched host".
3. **`3 · eth_sendTransaction` (0-wei transfer)** — the request crosses the
   Unix-socket transport, runs validate → decode → simulate → risk → policy,
   then **the vaultd system dialog appears** with the decoded intent,
   simulation, risk and policy evidence plus your secret phrase. Click
   **Approve**: the Vault signs, the tx broadcasts to Anvil, the page gets the
   tx hash, and the evidence panel shows `CONFIRMED` with decoded intent,
   simulation/risk provenance, tx hash, and the audit head sequence.
   (Click **Reject** instead to watch the fail-closed path: the page gets a
   coarse 4001 and the audit log records the refusal.)
4. **`4 · unlimited approve (drain attempt)`** — the deterministic policy
   denies it *before* any confirmation surface (a deny is never rendered as
   confirmable). The page sees only a coarse `4001 request refused`; the
   evidence panel shows the full story: `POLICY_DENIED`, risk findings with
   detector reason codes, and the POL-APR-001 policy reasons.
5. **`5 · eth_sign`** — refused broker-side (4200, deny-by-default), never
   forwarded to the wallet.

Verify the audit chain any time:

```sh
cd ../wallet && cargo run -p cipherlayer-cli -- audit verify
```

## Headless verification (no clicking)

```sh
./scripts/demo-headless.sh
```

Starts the same stack with the vaultd auto-approve **test hook**
(`CIPHERLAYER_CONFIRM_AUTO=approve` — loud and logged, harness only), runs
the provider-harness contract paths (9 checks, including the v1.2
`review_evidence` assertions) and the shell's built-in selftest
(`CIPHERLAYER_SHELL_SELFTEST=1`, 5 checks through the real Tauri process),
then verifies the audit hash chain. `SKIP_SHELL=1` skips the GUI windows.

## Trust posture (what this demo does and does not claim)

- The shell is **presentation only**: no keys, no policy, no signing state —
  a shell compromise can annoy, not sign. Approval happens exclusively on the
  wallet-owned surface (browser AGENTS #6, wallet ADR-012 tier v1 with its
  recorded residuals).
- The dapp page is hostile by assumption; it talks to the host only through
  the label-gated `clbridge` scheme and sees only coarse EIP-1193 errors.
  Policy internals and risk evidence go to the shell's evidence panel, never
  to the page.
- Custody is MOCKED (Anvil dev keys); local dev chain only. This is a demo
  of the trust pipeline, not a custody product.

See `docs/adr/ADR-B004-mvp-shell-design.md` for the shell's design and its
honest gaps against the full BS §22 acceptance walkthrough.
