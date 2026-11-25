# CipherLayer Browser (W3B)

**Never browse blind.** A competent general-purpose browser that integrates the CipherLayer Wallet and activates Web3 security controls only when consequence increases.

**Status: MVP shell (demo) stage.** The gate ([`docs/architecture/browser-mvp-gate.md`](docs/architecture/browser-mvp-gate.md)) opened 2026-07-07 (ADR-B003) on the engine selected by ADR-B002 (Tauri / system WebView, wallet + Vault as separate processes, no Chromium fork). The first product build is here: `crates/shell` (`cipherlayer-shell`), a two-webview Tauri host — evidence panel + one dapp capsule — over `crates/broker-core` and the pinned wallet contract (protocol v0.3, host-attested). **Run it: [`DEMO.md`](DEMO.md).** Design and honest gaps: [`docs/adr/ADR-B004-mvp-shell-design.md`](docs/adr/ADR-B004-mvp-shell-design.md). Approval never happens in this repo's UI — the wallet's trusted surface decides (AGENTS #6).

## Product definition

The browser is the wallet's most powerful integration surface — not the wallet's owner. Ordinary browsing stays ordinary (tabs, bookmarks, search, downloads, history, extensions where safe); Web3 behavior escalates with consequence: quiet dapp detection → connection disclosure → human-readable signing → simulation preview → policy-controlled confirmation on a wallet-owned trusted surface. A Web3-only browser that renders ordinary sites poorly is dead on arrival (US §1).

## Source specifications (`docs/source/`)

| Document | Role |
|---|---|
| `CipherLayer_Web3_Stack_Unified_Scope_v1_0.md` | Controlling product scope |
| `Cipherlayer_Web3_Browser_Full_Project_Scope_v0.1.md` | Browser architecture & requirements source |
| `Modular_Web3_Browser_Wallet_Project_Scope_Fable5_v0.3.md` | **Supplementary detailed guide** for the browser leg (per operator instruction 2026-07-04): per-site wallet-tier permissions, cold-wallet blocking, read-only / hot-only modes, dapp session isolation, network profile modes. Where it conflicts with the build prompt or Unified Scope, they win. |

## This repository owns

Browser shell (future) · Web2 browsing competency requirements · dapp detection · dapp capsules · provider & capability broker · browser-to-wallet integration · trusted confirmation surface requirements · browser threat model · compatibility experiments · the eventual dedicated browser.

It must **never** duplicate the Vault or wallet trust core — it consumes the versioned wallet contract pinned under [`imports/wallet/`](imports/wallet/) and the Scam Guard detector contract under [`imports/scam-guard/`](imports/scam-guard/) (PIN.json provenance, family convention per wallet register AC-001).

## Document map

| Document | Content |
|---|---|
| [`docs/architecture/trust-zones.md`](docs/architecture/trust-zones.md) | Zone A–G browser trust-zone architecture |
| [`docs/architecture/dapp-capsules.md`](docs/architecture/dapp-capsules.md) | Per-dapp isolation model |
| [`docs/architecture/provider-broker.md`](docs/architecture/provider-broker.md) | Provider & capability broker (Zone C) |
| [`docs/architecture/trusted-confirmation-surface.md`](docs/architecture/trusted-confirmation-surface.md) | Non-spoofable confirmation requirements |
| [`docs/architecture/per-site-permissions.md`](docs/architecture/per-site-permissions.md) | Per-site wallet-tier & network permissions (v0.3 guide) |
| [`docs/architecture/engine-comparison.md`](docs/architecture/engine-comparison.md) | Chromium / CEF / Electron / Tauri / extension comparison |
| [`docs/architecture/web2-competency.md`](docs/architecture/web2-competency.md) | Normal-browsing competency requirements |
| [`docs/architecture/browser-mvp-gate.md`](docs/architecture/browser-mvp-gate.md) | **The gate checklist** (operative) |
| [`docs/architecture/packaging-and-distribution.md`](docs/architecture/packaging-and-distribution.md) | Own binary vs embedded: browser-core/standalone/suite packages, Mode A/B distribution + guardrails (UQ-005) |
| [`docs/integration/browser-wallet-integration.md`](docs/integration/browser-wallet-integration.md) | How the browser consumes the wallet contract |
| [`docs/threat-model/browser-threat-extension.md`](docs/threat-model/browser-threat-extension.md) | Browser-specific threats extending the wallet threat model |
| [`docs/testing/compatibility-test-plan.md`](docs/testing/compatibility-test-plan.md) | Dapp/site compatibility test plan (pre-implementation) |

`crates/` holds product code (`broker-core`, `shell`); `prototypes/` holds the three sanctioned pre-gate experiments (`provider-harness`, `webview-harness`, `capsule-isolation`) that produced the ADR-B002 engine evidence; `fixtures/` holds the sample dapp they share.

## License

Apache-2.0 — see [`LICENSE`](LICENSE). Contributions require a DCO `Signed-off-by` line; see [`CONTRIBUTING.md`](CONTRIBUTING.md).
