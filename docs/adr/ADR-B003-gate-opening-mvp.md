# ADR-B003 — Operator Opens the Browser Gate for the MVP Shell Build

**Status:** **Accepted** (operator sign-off 2026-07-07 — this satisfies gate criterion 6 and opens the gate for the MVP shell build) · **Date:** 2026-07-07 · **Decider:** operator · Supersedes the scoped opening of ADR-B001 with the full opening the gate reserved for "all criteria + operator sign-off." Builds on ADR-B002 (engine selection) and the wallet work that closed criteria 2/4/5.

## Context

The browser MVP gate (`../architecture/browser-mvp-gate.md`, operative copy of wallet ADR-010) blocks a product browser build until six criteria pass. ADR-B001 opened the gate only for the transport, the broker library, and the compatibility harnesses, with compensating controls for the then-unmet criteria 2/4/5. Those three are now met, and criterion 3 was already satisfied:

| Criterion | Status 2026-07-07 | Evidence |
|---|---|---|
| 1. Wallet vertical slice stable | **substantially — one sub-item open** | All wallet suites green in **remote CI** now (GitHub Actions `R4m1r0qu41/W4113T-public`: `make ci` + a Foundry/Anvil integration + security-regression job, green on push/PR; siblings `W3B`/`ScamGuard` CI green too). Remaining: at least one external **review cycle**. |
| 2. Vault process boundary real | **met** | Wallet ADR-004 addendum: `cipherlayer-vaultd` hosts the Vault in its own OS process behind the pinned signing-request IPC; refusal invariants re-proven across the boundary. |
| 3. Provider contract exercised by a consumer | **met** | v0.2/v0.3 transport + `broker-core` + `provider-harness` drive connect→request→decision→response live; capsule-isolation 6/6. |
| 4. Desktop trusted-confirmation flow | **met** | Wallet ADR-012 tier v1: vault-process system dialog, secret-phrase marker, fail-closed, red-teamed (RT-1…RT-6); demonstrated live. |
| 5. Browser threat-extension reviewed | **met** | `browser-threat-extension.md` v0.3 reviewed against wallet T-01…T-26; every BT row dispositioned; C-1 closed (ADR-013), C-2 accept-labeled. |
| 6. Operator opens the gate | **this ADR** | — |

The operator, as gate owner, elects to open the gate for the **product MVP shell build** on the engine selected in ADR-B002.

## Decision

Open the gate for the Browser MVP shell — and only per the sequencing below, honoring every standing prohibition.

**Engine (from ADR-B002, unchanged):** Tauri / system-WebView with the wallet + `cipherlayer-vaultd` as separate processes. **No Chromium/CEF fork** — that still requires its own ADR, operator sign-off, and a funded maintenance plan.

**Sequencing:**

1. ~~**Transport concurrency first (ADR-B002 finding 2).**~~ **Closed 2026-07-08.** The wallet's browser-connect transport now serves one task per accepted connection (correctness-critical state — permissions, audit, the single vault/nonce space — stays serialized behind one lock; capability queries and chain lookups are lock-free). Proven with a regression test that fails against the pre-fix code and passes after. The hard prerequisite for a multi-capsule shell is met — **shell work (step 2) is now the only remaining blocker.**
2. **MVP shell (per `../architecture/browser-mvp-gate.md` "Browser MVP acceptance", BS §22, fifteen steps).** A Tauri host: one WebView per capsule (storage isolation), custom-protocol-per-capsule origins, the EIP-1193 provider injected before page scripts, `broker-core` as the Rust-side bridge. The host **attests to the wallet** (transport v0.3 host token) and assigns capsule ids pages cannot choose — capsule identity comes from the trusted host, never the page. USER_REVIEW is never rendered as web content: it stays on the wallet's trusted surface (`cipherlayer-vaultd`, ADR-012). Per-site permissions per `per-site-permissions.md` active.
3. **Acceptance:** the fifteen BS §22 steps demonstrated end-to-end against a testnet/dev chain, with the per-capsule permission model and the trusted confirmation surface in the loop; results recorded, and any user-facing security wording checked against the wallet's claims register.

**Explicitly still closed** (each needs its own decision): a Chromium/CEF fork; any real-key / real-fund custody (test keys and dev/testnets only, ADR-005 unchanged); Mode B symbiotic packaging; an extension/plugin runtime (browser threat BT-04 remains defer-trigger); shipping a signed, distributed release (browser threat BT-11 — needs the release-security pipeline first).

## Compensating control for the one open criterion

- **Criterion 1 (remote CI + review cycle).** Remote CI is now **in place and green** (GitHub Actions on all three repos, 2026-07-07): the wallet runs `make ci` plus a Foundry/Anvil integration + security-regression job on every push/PR. The one remaining sub-item is an external **review cycle**. **Control:** until that review lands, the MVP shell stays a local, dev-chain, test-key demonstration (which the standing prohibitions already require); no custody or safety claim rests on it. If the operator wants criterion 1 fully closed before any shell work, hold for the review cycle first (see alternatives).

## Honest residuals carried into the MVP (not blockers, but bounded)

- **C-2 — confirmation surface tier.** ADR-012 tier v1 (system dialog + secret phrase) resists spoofers who cannot read the phrase file, not a same-user attacker who can; no occlusion detection. SEP/biometric + occlusion are the next tier. "Trusted surface" stays a qualified claim (wallet SC-11).
- **C-1 residual — attestation token.** Host attestation (ADR-013) closes capsule forgery for any non-host peer; a same-user attacker who reads the launch token is local malware (wallet T-14, accepted for the prototype).
- **Per-capsule OS isolation** remains the engine's defence-in-depth job; the wallet already enforces permission scoping.

## Alternatives considered

1. **Hold the gate until criterion 1 is fully closed (remote CI + review).** Defensible; rejected as the default because the shell's first form is a local dev-chain demonstration that makes no custody claim, and CI can land in parallel. The operator may choose this instead.
2. **Another compatibility harness before a shell.** Rejected: the three sanctioned experiments already produced the data (ADR-B002); the next honest step is the shell itself.
3. **Keep the gate closed.** Rejected: criteria 2/3/4/5 are met and C-1 is closed; the reason the gate stood is largely resolved.

## Consequences

- The browser repo may begin the Tauri MVP shell now that transport concurrency has landed, under all standing prohibitions (no wallet-core duplication, no keys in any renderer, confirmation is wallet-owned, dependency direction scam-guard → wallet → browser).
- Gate checklist criterion 6 is satisfied when this ADR is accepted; the checklist's "all criteria + operator sign-off" milestone then unlocks the MVP build.
- Criterion 1's remote-CI item and residuals C-1/C-2 travel with the work as tracked, labeled limitations.

## Revisit

If transport concurrency proves unable to preserve capsule isolation; if WebView storage isolation is insufficient for real dapps (could reopen the Chromium-fork question, its own ADR); before any distributed/signed release (release-security pipeline, BT-11); before any real key or fund (all custody residuals become blocking).
