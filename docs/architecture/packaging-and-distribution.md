# Packaging and Distribution — Own Binary vs Embedded

**Version:** 0.1 · **Status:** decided at the product level (operator decisions UQ-005 2026-07-04 and suite-governance choice 2026-07-05); implementation post-gate. This document records the decided model and the technical analysis behind it.

## The decision

The browser ships **as its own binary**, and the codebase is structured as modular packages so two binaries share one browser implementation:

| Package | Kind | Contents | Built in |
|---|---|---|---|
| `browser-core` | library | rendering setup, security settings, ad-blocking logic, privacy configurations, UI design tokens, capsule + broker implementation | **browser repo** |
| `standalone-browser` | app binary | `browser-core` wrapped in a traditional browser frame: tabs management, bookmarks, address bar | **browser repo** |
| `wallet-app` (suite build, "browser mode") | app binary | wallet UI embedding `browser-core` as a dedicated web-view mode, styled to match the desktop layout, with the wallet provider injected natively | **browser repo** (see governance below) |

**Governance (dependency direction preserved):** the family rule is `scam-guard → wallet → browser` — the wallet must never depend on browser internals. Therefore `browser-core` and *both* suite binaries are built from the **browser repository**, which already imports the wallet's exported contract (`imports/wallet/PIN.json`) and, at suite-build time, the wallet's released UI/application artifacts. The **pure wallet app** (no browser mode) remains the wallet repo's own product. Branding is a product decision; build home is a governance decision — the "wallet suite" is a browser-repo distribution.

## Distribution modes

**Mode A — Embedded (baseline default).** The suite binary compiles/bundles the browser engine directly into its dependencies. One download, self-contained, works out of the box for a user who only installs one thing. This is always the fallback and always shippable.

**Mode B — Symbiotic (opportunistic optimization).** When the standalone browser is already installed, the suite links against / delegates to that installation (shared framework or daemon) instead of carrying its own copy — depressurizing bundle size for the most engaged users while keeping Mode A as the zero-friction fallback.

### Mode B security guardrails (mandatory, from the operator's design)

Piggybacking on an external binary is a **local supply-chain attack vector**: if malware replaces or patches the standalone browser on disk, the wallet must not load its code. Before the suite ever activates Mode B:

1. **Code-signature enforcement.** Programmatically verify the standalone binary is validly signed by *our* developer certificate — `codesign -v` (plus `spctl`/notarization checks) on macOS, `WinVerifyTrust` on Windows. Verification happens at every activation, not just install time; any failure = silent fallback to Mode A + an audit event.
2. **Version-matrix matching.** The suite carries a compatibility matrix; if the installed standalone browser is older than the suite requires (or unknown), Mode B is refused and Mode A used — never "best effort" linking across mismatched API contracts or unpatched engine versions.

Additional guardrails this document adds to the operator's two:

3. **Same-team requirement:** signature must chain to our Team ID specifically, not merely "validly signed by someone" (blocks a validly-signed impostor).
4. **No privilege inheritance:** Mode B reuses the *engine*, never the standalone browser's profile, extensions, or storage — capsule and profile state stay suite-owned (prevents cross-product state bleed and extension-borne code entering the suite).
5. **Vault indifference:** the Vault process and trusted confirmation surface are identical in Mode A and Mode B — packaging mode must be invisible to the trust core, and switching modes must never change any security behavior, only footprint.

## Technical analysis: own binary vs embedded web-view (why both, structured this way)

- **Trust-surface:** the security-relevant code (capsules, broker) lives in `browser-core` either way — one implementation to review, test, and patch, regardless of which binary hosts it. This is the main argument for the library split over two divergent codebases.
- **Engine interplay:** if the W2 evaluation confirms Tauri (wallet ADR-003, operator-confirmed default), the *wallet app's* own UI uses the system WebView, while `browser-core`'s engine choice (post-gate, see `engine-comparison.md`) may differ. Mode A "embedding browser-core" therefore ranges from cheap (browser-core is also system-WebView-based) to heavy (browser-core carries Chromium/CEF). The engine ADR must price both binaries, not just the standalone browser.
- **Update surface:** Mode A means engine security patches ship through the suite's update channel too — two update artifacts per engine patch (standalone + suite). Mode B collapses that to one for co-installed users, which is its real security benefit beyond footprint: fewer stale engines in the field. The version matrix is what prevents the inverse (suite trusting a stale standalone).
- **macOS specifics:** both binaries notarized; Mode B verification uses the hardened-runtime + notarization chain; SEP-backed confirmation (operator UQ-003) is unaffected by mode since it lives wallet-side.
- **Failure honesty:** Mode B activation/refusal is user-visible in diagnostics and audited — silent mode differences are how "works on my machine" security bugs hide.

## Sequencing

Nothing here unblocks early: packaging work starts only after the browser gate opens and an engine is selected. Pre-gate, this document plus the engine comparison are the inputs; the first post-gate packaging deliverable is Mode A only — Mode B lands after the standalone browser has a stable release + signing identity to verify against.
