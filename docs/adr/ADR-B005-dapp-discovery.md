# ADR-B005 — Dapp Discovery: Native In-Shell Identity, WalletConnect Deferred

**Status:** **Accepted** (operator directive 2026-07-16) · **Date:** 2026-07-16 ·
**Decider:** operator · **Context:** protocol v0.4 re-pin (ADR-019, `imports/wallet/`
commit `f64682e`) adds forwardable signing methods and a read-only proxy; none
of it reaches a real dapp if the dapp never finds the provider first
(`docs/testing/dapp-compat-matrix.md` §4 flagged this as the structural
prerequisite blocking the whole method matrix).

## Context

Two discovery mechanisms exist in the wild:

1. **Native injection** — a wallet exposes `window.ethereum` (legacy,
   majority pattern) and/or announces itself via **EIP-6963**
   (`eip6963:requestProvider` / `eip6963:announceProvider`), letting a page
   enumerate every injected wallet in the same browsing context without a
   naming collision on `window.ethereum`.
2. **WalletConnect** (v2) — a remote-pairing protocol: the dapp shows a QR
   code / deep link, the wallet (a *separate* app, often on a different
   device) scans or opens it, and a session is negotiated over relay
   infrastructure. Designed for the case where the wallet and the dapp are
   not in the same browsing context at all (mobile wallet ↔ desktop
   browser).

The operator evaluated WalletConnect directly against real dapps during this
cycle and recorded the evidence: **2026-07-16 — WalletConnect QR modal
friction/glitch loops on real dapps** (pairing modals stalling, re-render
loops, session drops requiring a fresh QR scan). This is consistent with
WalletConnect's own purpose: it solves a rendezvous problem between two
processes that don't share a page context.

## Decision

**Native in-shell discovery — `window.ethereum` plus EIP-6963 identity — is
the primary and, for now, only dapp-discovery mechanism.** Implemented in
`prototypes/provider-harness/provider.js` (reused unmodified by
`crates/shell/src/main.rs`'s capsule injection):

- `window.cipherlayer` (back-compat with `fixtures/sample-dapp.html`) **and**
  `window.ethereum` (claimed only if unclaimed) are injected before page
  scripts run.
- `eip6963:announceProvider` fires once at injection and again on every
  `eip6963:requestProvider` the page dispatches (per spec — a discovery UI
  that starts listening after injection must still find us), carrying
  `{ info: { uuid, name: "CipherLayer", icon: <inline SVG data URI>, rdns:
  "org.cipherlayer.browser" }, provider }`. `uuid` is a fresh v4 per page
  load; `rdns` is the stable product identity.

**Rationale:** inside our own shell, the wallet and the dapp capsule already
share a process tree — the shell launches the wallet transport, injects the
provider into the same webview the dapp renders in. There is no rendezvous
problem to solve; the only requirement is that a real dapp's own "detect
wallet" logic (legacy `window.ethereum` access, or an EIP-6963 listener)
finds CipherLayer the instant the page runs. Discovery must be
**zero-interaction** — no QR code, no modal, no second device — because none
of those exist between two things running in the same window.

**WalletConnect v2 compatibility is explicitly DEFERRED**, tracked as a
future ADR, and scoped only as an **external-reach path**: letting
CipherLayer be *paired to* from a dapp running in a **third-party** browser
(one CipherLayer doesn't own the webview of), not as the in-shell discovery
mechanism. The operator's evidence above is about WalletConnect's UX
reliability on real dapps generally — it is a reason to keep it out of the
critical path for our own shell's discovery, not a verdict on whether it is
ever useful for the separate, later, external-reach problem.

## Non-decision

This ADR does not decide anything about the WalletConnect external-reach
path itself (protocol version, relay infrastructure, session lifecycle, how
it would interact with per-capsule identity) — that is left entirely to the
future ADR referenced above. It also does not change method classification
or the signing pipeline; see `docs/testing/dapp-compat-matrix.md` for the
v0.4 method-support picture that this ADR's discovery fix makes reachable.

## Honest gaps

- No third-party-browser reach today: a dapp not running inside a
  CipherLayer shell capsule cannot discover CipherLayer at all yet (that is
  exactly the deferred WalletConnect path).
- EIP-6963 `uuid` freshness (per page load, not persisted) means a dapp that
  caches a provider by `uuid` across reloads will not recognize CipherLayer
  as "the same" wallet between sessions; `rdns` is the stable identifier for
  that case, per spec.
- One capsule, one shell process today (ADR-B004) — multi-capsule discovery
  (each capsule announcing independently) is untested, though the injection
  script itself is capsule-agnostic (host-injected per webview).

## Revisit

When a WalletConnect (or successor) external-reach ADR is proposed; if a real
dapp's discovery logic is found to require something this mechanism doesn't
provide (e.g. `eip6963:announceProvider` timing relative to other injected
scripts); before any packaged release.
