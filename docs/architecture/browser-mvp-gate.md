# Browser MVP Gate — Operative Checklist

**Version:** 0.4 · **GATE OPEN as of 2026-07-07** (ADR-B003 accepted): the MVP shell build is unlocked, under all standing prohibitions below; criterion 1's external-review sub-item is carried as a tracked compensating control. Transport concurrency (ADR-B002 finding 2), the one sequencing prerequisite ahead of shell work, closed 2026-07-08. **2026-07-09: the Tauri MVP shell landed** (`crates/shell`, ADR-B004; runbook `DEMO.md`): fixture-dapp connect → pipeline review with v1.2 `review_evidence` displayed → approve/reject on the vaultd trusted surface → broadcast on Anvil → hash-chained audit entry, verified live and by `scripts/demo-headless.sh`. The BS §22 acceptance walkthrough below is exercised against the fixture dapp; real-dapp compatibility, multi-capsule windows, and the pre-grant scope-disclosure UI remain open (ADR-B004 "honest gaps"). History and per-criterion evidence retained below.

This is the operative copy of the gate defined in wallet ADR-010. Browser implementation was blocked until **every** box was checked and the operator explicitly opened the gate. "Implementation" = any shell code, provider injection, rendering integration, or engine work. Docs, contracts, and threat modeling are always allowed; narrow compatibility experiments unlock at criteria 1–3.

## Gate criteria

- [ ] **1. Wallet vertical slice stable.** validate → decode → simulate → risk → policy → authorize → sign → broadcast → audit passes its unit, integration, and security-regression suites in CI (not just locally), and has survived at least one review cycle.
  - Status note 2026-07-04: slice works end-to-end locally (demo + 8 integration tests green); no CI, no review cycle yet.
  - Status 2026-07-07: **remote CI now green.** GitHub Actions on `R4m1r0qu41/W4113T-public` runs `make ci` (fmt · clippy -D warnings · unit + security-regression · docs-check) and a Foundry/Anvil job for the integration suite, on every push/PR. Sibling repos `W3B` and `ScamGuard` also have green CI. Remaining sub-item: at least one external **review cycle** (not yet done).
- [ ] **2. Vault process boundary is real.** ADR-004 process split implemented: Vault in its own OS process, schema-validated IPC, no GUI signing before this.
  - Status note: MOCKED — in-process trait boundary only.
- [x] **3. Provider contract exercised by a consumer.** browser-connect schemas + protocol doc versioned, and a test harness (not this repo's product code) has driven connect → request → decision → response end-to-end against them, including denial/expiry paths.
  - Status 2026-07-05: **satisfied.** Three layers of evidence: (a) wallet-side conformance harness (in-process, denial/expiry/confirmation paths; surfaced + fixed the nullable signing_request_id contract gap, v1.1); (b) wallet cross-process transport test spawning the real binary over the protocol v0.2 Unix socket; (c) this repo's `prototypes/provider-harness` — a separate consumer built only against the pinned contract — passing all six contract paths live (capability, connect + idempotent regrant, confirmed transfer with tx hash + audit ref, unlimited-approval denial with coarse EIP-1193 mapping, revoke-takes-effect).
- [x] **4. Desktop trusted-confirmation flow demonstrated.** W2 desktop wallet shows the confirmation flow outside the CLI, meeting `trusted-confirmation-surface.md` requirements including the red-team exercise.
  - Status 2026-07-07: **satisfied (tier v1).** Wallet ADR-012: `cipherlayer-vaultd` (the ADR-004 Vault process, criterion 2) renders USER_REVIEW in a macOS **system dialog** — system-owned chrome, no web content, deterministic pipeline evidence only, a user-selected secret phrase (0600, fail-closed), escalation requiring a typed `ESCALATE`, deny never rendered. Demonstrated live end-to-end (`cipherlayer tx send --vault-confirm` → dialog → confirmed on Anvil; audit records `surface: desktop_trusted_surface`). Red-team RT-1…RT-6 recorded in ADR-012 with the honest residuals (SEP/biometric binding and occlusion detection deferred to the next tier; the phrase resists spoofers who cannot read the file, not an attacker who already has same-user read). Wallet security-claims register row SC-11 added; "trusted surface" remains a qualified claim until the SEP tier lands.
- [x] **5. Browser threat-model extension reviewed.** `../threat-model/browser-threat-extension.md` reviewed against the then-current wallet threat model; open criticals dispositioned.
  - Status 2026-07-07: **satisfied.** `browser-threat-extension.md` re-reviewed (now v0.2) against wallet threat model T-01…T-26. Every BT-01…BT-12 row carries a disposition (mitigate-MVP / accept-label / defer-trigger). No new criticals surfaced; two browser-specific residuals dispositioned: **C-1 (capsule_id integrity)** — **since closed** for non-host peers by wallet transport v0.3 host attestation (ADR-013): a peer that is not the launched host is refused before it can assert a capsule id; residual is a same-user attacker who reads the launch token (= T-14 local malware, accepted). **C-2 (confirmation-surface tiers)** — tier v1 accept-label, SEP/occlusion deferred (ADR-012 / SC-11). The engine-choice tension (BT-01 vs BT-11) is recorded as resolved by ADR-B002. The capsule_id-integrity prerequisite that blocked a multi-capsule shell is now met at the transport level.
- [x] **6. Operator explicitly opens the gate.** Recorded as an ADR in this repository naming the first post-gate build.
  - Status 2026-07-05: **scoped opening recorded** — ADR-B001 opened the gate for the transport, the broker library, and the compatibility harness only, with compensating controls for the then-unmet criteria 2/4/5.
  - Status 2026-07-07: **gate OPEN.** Criteria 2/3/4/5 met and C-1 closed; **ADR-B003 accepted** (operator sign-off) opens the gate for the Tauri MVP shell per ADR-B002. Criterion 1's remaining sub-item (one external review cycle) is carried as ADR-B003's compensating control. The "all criteria + operator sign-off → Browser MVP build" milestone is unlocked.
  - Status 2026-07-08: transport concurrency (ADR-B002 finding 2), the sequencing prerequisite ADR-B003 placed ahead of shell work, is **closed** (see wallet's transport-concurrency commit). The Tauri shell is now the only remaining item before the BS §22 MVP acceptance walkthrough.

## What unlocks when

| Milestone | Unlocked work |
|---|---|
| Criteria 1–3 checked | Narrow compatibility experiments in `prototypes/` (provider-injection harness against real dapps; capsule-isolation breakage measurements). Still no shell product. |
| All criteria + operator sign-off | Browser MVP build per `engine-comparison.md` sequencing (cheapest credible shell; **no Chromium fork** without its own ADR). |

## Standing prohibitions regardless of gate state

No Vault/wallet-core duplication here · no key material in any renderer, ever · no confirmation surfaces rendered as web content · dependency direction stays scam-guard → wallet → browser.

## Browser MVP acceptance (post-gate target, BS §22)

User opens shell → visits sample dapp → dapp detects provider → requests connection → scope disclosed → permission granted → dapp requests transaction → intercepted → simulated → decoded consequences displayed → policy evaluates (high-risk approval blocked/escalated) → user confirms allowed tx → mock/testnet signing through the Vault → result + audit record displayed. All fifteen steps of BS §22, with the per-site permission model of `per-site-permissions.md` active.
