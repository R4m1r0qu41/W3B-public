# Engine Comparison — Chromium, CEF, Electron, Tauri, Extension

**Version:** 0.1 · Decision status: **no engine is selected** (and none may be until the gate opens). This comparison frames the post-gate decision; the family rule stands: no Chromium fork before product validation (BS §8.1, §13; AGENTS.md #5).

| Criterion | Chromium fork | CEF embed | Electron | Tauri (system WebView) | Extension PoC (in Chrome/Brave) |
|---|---|---|---|---|---|
| Web2 competency | best (it *is* Chrome) | very good | very good (bundled Chromium) | good on macOS (WKWebView), uneven cross-platform | best (host browser) |
| Control over capsule isolation | total (profiles, site isolation, network stack) | high | medium-high (session partitions) | limited (WebView APIs) | low (extension APIs only) |
| Provider injection control | total | high | high (preload scripts) | medium (script injection) | medium (content scripts; MV3 constraints) |
| Trusted-surface separation | full (own process model) | high | medium (must fight its own architecture) | good (Rust host process is native) | poor (extension UI is spoofable-adjacent) |
| Maintenance burden | **very high** (security-patch treadmill; the known killer, BS §2, §17.8) | high | medium (upstream handles Chromium) | low-medium | lowest |
| Trusted computing base size | huge but self-controlled | huge | huge + Node in host | smallest shipped surface; Rust host | host browser (not ours) |
| Rust trust-core fit | possible | possible | awkward (Node host) | **native** | n/a (talks to wallet app) |
| Update security obligations | all ours | mostly ours | shared | shared (OS WebView patched by OS) | host's |
| Cost (BS §13) | very high | high | moderate | moderate | low |
| Credibility as "a browser" | full | full | full-ish | full-ish | none (but proves the signing loop) |

## Framing recommendation (not a decision)

Sequenced per BS §8.1 and the wallet-first roadmap:

1. **Post-gate first experiment:** extension or minimal shell **compatibility harness** — cheapest credible way to exercise the provider contract against real dapps and measure capsule-isolation breakage. Explicitly *not* the product.
2. **Browser MVP candidate:** Tauri or Electron shell consuming the process-split Vault. Tauri leans toward the family's minimal-TCB principle and Rust core (consistent with wallet AC-009 lean); Electron leans toward rendering consistency and mature session partitioning. Decide with an ADR informed by harness data (capsule isolation needs vs WebView API limits are the crux).
3. **Chromium/CEF** only after the MVP proves demand and the compatibility data shows WebView-class shells cannot deliver capsule isolation — with operator sign-off and a funded maintenance plan (this is an organizational commitment, not a code choice).

## Revisit triggers

Gate opening (mandatory ADR) · harness data showing isolation limits · macOS WKWebView policy changes · team capacity changes that alter the Chromium maintenance calculus.
