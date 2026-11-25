# ADR-B001 — Operator Opens the Browser Gate for Scoped Pre-MVP Work

**Status:** accepted · **Date:** 2026-07-05 · **Decider:** operator (recorded verbatim intent: "now we pivot to start building the browser before going in with the MetaMask tests")

## Context

The browser MVP gate (`../architecture/browser-mvp-gate.md`, operative copy of wallet ADR-010) blocks browser implementation until six criteria pass. At decision time the honest status was:

| Criterion | Status 2026-07-05 |
|---|---|
| 1. Wallet vertical slice stable | largely — all suites green locally (unit, integration, security-regression, conformance); no remote CI, no external review cycle |
| 2. Vault process boundary real | **not met** — in-process trait boundary, labeled MOCKED (ADR-004 split is W2 work) |
| 3. Provider contract exercised by a consumer | largely — wallet-side conformance harness covers confirmation/denial/expiry and already fixed one contract gap (signing-response v1.1); live transport missing |
| 4. Desktop trusted-confirmation flow | **not met** — no desktop app exists |
| 5. Browser threat-extension reviewed | **not met** — no review cycle yet |
| 6. Operator opens the gate | **this ADR** |

The operator, as gate owner, elects to open the gate **for a limited scope** rather than wait for criteria 2/4/5, sequencing browser groundwork before the MetaMask external-signer work (wallet CL-WAL-010).

## Decision

The gate opens for exactly the work the gate document already names as the first post-gate experiment — and nothing more:

1. **Protocol v0.2 transport** (both repos): a local, same-user-authenticated IPC transport for the browser-connect contract. Wallet side: a serve mode driving the existing pipeline. Browser side: a client. This finishes gate criterion 3 (live transport exercise).
2. **Provider broker library (`broker-core`)**: Zone C plumbing per `provider-broker.md` — schema-conformant envelope construction, mandatory identity attachment, deny-by-default method allowlist, EIP-1193 error mapping. Zero authority; all decisions stay wallet-side.
3. **Compatibility harness in `prototypes/`**: a non-product binary that drives connect → request → decision → response against the real wallet over the real transport, plus the injectable EIP-1193 provider script for the later webview experiment.

**Explicitly still closed:** any product browser shell, engine selection (separate ADR after harness data, per `engine-comparison.md`), Chromium/CEF work of any kind, GUI signing surfaces, and Mode B symbiotic packaging work.

## Compensating controls for the unmet criteria

- Criterion 2 (Vault split): irrelevant to key risk today — the entire family holds only Anvil's publicly known dev keys on chain 31337; the wallet's dev-chain signing gate stays in code. No claim of custody safety is made or implied by this work.
- Criterion 4 (trusted confirmation): the harness never renders a confirmation surface; USER_REVIEW stays on the wallet's terminal prompt (documented spoofable, T-12), and serve-mode auto-approval is an explicit, logged flag for tests only.
- Criterion 5 (threat review): the transport design must disposition the relevant BT rows (hostile-page reachability, peer authentication) in its own design note before the socket ships enabled-by-default anywhere.

## Consequences

- The browser repo gains its first code; AGENTS.md constraints bind unchanged (no wallet-core duplication, no sibling path imports — wire contract only, no keys in renderers, confirmation is wallet-owned).
- Gate criteria 1/4/5 remain open items and still block the **product** browser MVP; this ADR does not check them.
- The engine decision remains un-made; the harness exists to produce the data that decision needs.

## Revisit

When harness data lands (engine ADR), when the Vault process split completes (criterion 2), or if the transport design cannot satisfy peer-authentication requirements.
