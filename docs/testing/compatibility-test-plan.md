# Compatibility Test Plan

**Version:** 0.1 · Written pre-implementation so the post-gate harness has defined success criteria before any engine is chosen. Two competencies are measured separately: **Web2 competency** (ordinary sites) and **dapp compatibility under isolation** (the predicted #1 breakage source, BS §17.1).

## Phase 0 (allowed now, no gate needed): corpus + criteria

- Assemble the Web2 corpus: news, long-form docs, video (short/long, live), webmail, office suites, OAuth/passkey logins, banking-style flows, social feeds with heavy JS, WebRTC call test pages.
- Assemble the dapp corpus: a spread of provider-usage styles — classic `window.ethereum` dapps, EIP-6963 multi-provider discovery, WalletConnect-first dapps, wagmi/viem apps, popup-window flows, multi-tab session dapps, testnet faucets, one deliberately hostile test page (provider shadowing, fake modal).
- Define per-item pass criteria (loads / renders / core flow completes / no console-level breakage / no crash).

## Phase 1 (gate criteria 1–3 met): provider harness

Runs in `prototypes/` against the vendored wallet contract:

1. **Injection matrix:** provider present before first script; EIP-6963 announcement; shadowing resistance (BT-02 race tests).
2. **Method coverage:** every method in `wallet-capability.v1` exercised; unknown methods produce correct EIP-1193 errors; denial/timeout/expiry paths verified end-to-end.
3. **Envelope fidelity:** origin + capsule identity always attached; identity-less injection attempts rejected wallet-side (mirrors wallet regression tests).
4. **Per-site permission semantics:** read-only mode, tier filtering, network allowlist, chain-switch prompts, testnet-mode refusals.

## Phase 2 (engine candidates): isolation breakage measurement

For each candidate shell (per `engine-comparison.md` sequencing), run the dapp corpus under full capsule isolation and record, per dapp:

| Field | Values |
|---|---|
| result | works / degraded / broken |
| breakage cause | storage isolation / popup flow / window.opener / third-party cookies / service worker / provider quirk / other |
| workaround class | none needed / per-site relaxation (user-visible) / protocol change / engine limitation |

Exit data: percentage working unmodified; breakage-cause histogram; the list of isolation relaxations that would be required — this is the objective input to the engine ADR and to how honest the "dapps just work" claim can be.

## Phase 3 (browser MVP): acceptance runs

- Web2 corpus pass rate against the bar in `../architecture/web2-competency.md` (incl. performance factor vs reference browser, crash-free sessions).
- BS §22 fifteen-step acceptance flow on the sample dapp.
- Red-team fixtures: hostile test page + test extension + test theme against the confirmation surface (required before any "trusted surface" claim).

## Reporting

Every run produces a dated report under `docs/research/compat/` with environment (engine, version, OS), corpus version, raw per-item results, and deltas vs the previous run. No summary may omit the broken list.
