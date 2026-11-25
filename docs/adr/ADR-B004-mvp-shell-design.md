# ADR-B004 — MVP Shell Design (Tauri host, label-gated scheme bridge)

**Status:** accepted · **Date:** 2026-07-09 · **Context:** first product build
after ADR-B003 opened the gate; engine fixed by ADR-B002 (Tauri / system
WebView, wallet + `cipherlayer-vaultd` as separate processes, no Chromium fork).

## Decision

`crates/shell` (`cipherlayer-shell`) is a Tauri 2 host with two webviews and
zero authority:

- **`main`** — the shell's evidence panel (static `ui/`, trusted Zone C
  chrome). Shows wallet-link status, host attestation, granted accounts, the
  latest signing review (v1.2 `review_evidence`: decoded intent, simulation,
  risk findings incl. detector reason codes, policy verdict/reasons), tx
  hash, audit head, and an activity feed. Display only, and labeled as such.
- **`capsule`** — one dapp capsule (ADR-B002: one WebView = one capsule = one
  custom-protocol origin). Serves the fixture dapp at the **host-assigned**
  origin `capsule://dapp`; the EIP-1193 provider (reused from
  `prototypes/provider-harness/provider.js`) is injected before page scripts.

**Page↔host bridge: a custom URI scheme (`clbridge`), not Tauri command IPC.**
The injected provider `fetch`es `clbridge://localhost/provider`; the shell UI
polls `clbridge://localhost/state`. Authorization is keyed on the
**host-supplied webview label** (`UriSchemeContext::webview_label`) with
deny-by-default routing (`host::route`, regression-tested):

| label | route | why |
|---|---|---|
| `capsule` | `POST /provider` only | the page's channel; zero authority; coarse EIP-1193 errors only |
| `main` | `GET /state` only | policy internals/risk evidence are for the shell's evidence UI, never hostile pages (provider-broker.md #8) |

Rationale for scheme-over-command-IPC: the capsule must have a page-reachable
channel *by design*, while Tauri's command ACL is meant to keep pages away
from app commands — using it for a hostile-page channel would invert its
model. The scheme handler is the same mechanism Tauri's own IPC uses, gives
per-webview provenance (the label cannot be chosen by page content), and
keeps the capsule fully outside the Tauri capability system (it holds no
capabilities at all).

**Wallet coupling** is exactly the pinned contract: `broker-core` over the
protocol v0.3 Unix socket, host attestation first (token from
`CIPHERLAYER_CONNECT_HOST_TOKEN` or the wallet's `connect-host-token` file),
capsule identity attached by the host. Wallet round trips run on worker
threads so a pending USER_REVIEW (vaultd dialog, up to 120 s) never blocks
the UI event loop. USER_REVIEW itself stays on the wallet-owned surface
(`connect serve --vault-confirm` → `cipherlayer-vaultd`, wallet ADR-012);
no browser window renders anything confirmable (AGENTS #6).

**Explicit test hooks** (family discipline, loud and never default):
`CIPHERLAYER_SHELL_SELFTEST=1` drives the provider entry point through the
real shell process and exits pass/fail; `CIPHERLAYER_SHELL_AUTODRIVE=1` makes
the fixture page click itself to exercise injection + the in-page fetch
bridge. `scripts/demo-headless.sh` composes them with the wallet stack and
the vaultd `CIPHERLAYER_CONFIRM_AUTO` hook.

## Conformance with ADR-B002/ADR-B003

No deviations from the decided architecture: Tauri host ✓ · system WebView ✓ ·
wallet/vaultd as separate OS processes ✓ · broker-core reused unchanged ✓ ·
provider injected before page scripts ✓ · host attests, pages cannot choose
capsule ids ✓ · confirmation wallet-owned ✓ · no Chromium/CEF ✓. One point of
interpretation: ADR-B003 sketches the bridge as "broker-core as the Rust-side
bridge" without fixing the page↔host mechanism; this ADR fixes it as the
label-gated scheme above.

## Honest gaps (demo → real product)

- **One capsule, fixture dapp.** No tabs/navigation, no real-site loading, no
  multi-capsule windows yet (the transport already serves capsules
  concurrently; the shell doesn't create them yet). BS §22's fifteen-step
  acceptance is exercised against the fixture dapp, not real dapps.
- **Scope disclosure pre-grant** is minimal: connect is granted by the wallet
  (scoped, expiring, audited) without a shell-side "this site wants X" prompt
  first. `per-site-permissions.md`'s UI surface is future work; the underlying
  per-capsule permission model is active wallet-side.
- **Web2 competency** (AGENTS #7) is untouched by this shell — it browses
  nothing but the fixture. A real shell must not regress ordinary browsing.
- **Packaging/distribution** intentionally out (ADR-B003: BT-11 release
  pipeline first); `bundle.active: false`, dev-run only.
- Carried residuals unchanged: C-2 confirmation tier v1, C-1 same-user token
  reader (wallet T-14), MOCKED custody (ADR-005), criterion-1 external review
  cycle still pending (ADR-B003 compensating control).

## Revisit

When multi-capsule windows land (capsule window factory + per-capsule
identities); if real-dapp compatibility needs storage/session work the
one-webview model cannot give; before any packaged release.
