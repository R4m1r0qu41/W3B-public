# ADR-B002 — Browser Engine Selection

**Status:** proposed (recommendation; awaiting operator sign-off) · **Date:** 2026-07-05 · **Decider:** operator · Informed by the ADR-B001 experiments: `provider-harness`, `webview-harness`, `capsule-isolation`. Supersedes the framing in `engine-comparison.md` with measured data.

## Context

`engine-comparison.md` framed the engine choice but deferred it pending harness data ("decide with an ADR informed by harness data (capsule isolation needs vs WebView API limits are the crux)"). The three sanctioned experiments now provide that data.

### What the experiments measured

**provider-harness** (external consumer over the real v0.2 transport, 7 checks): the pinned browser-connect contract carries a full dapp session — capability, connect, confirmed transfer, policy denial with coarse EIP-1193 mapping, revoke, and FPL origin evidence — with the broker holding zero authority. The provider/broker layer is engine-independent: it is plain Rust over a socket and works under any shell that can host a webview and run a bridge.

**webview-harness** (wry/WKWebView, injection experiment): a system WebView loads a dapp over a custom protocol, the EIP-1193 provider injects before page scripts, and IPC bridges to the wallet. Findings:
- Injection timing (`with_initialization_script`) is reliable and runs before page JS — provider presence is controllable.
- Custom-protocol origin is controllable (`harness://dapp`), which is what lets one window == one capsule origin.
- The system WebView gives **one webview = one isolated storage context** for free on macOS; multiple capsules would be multiple webviews/windows.

**capsule-isolation** (permission-boundary measurement over the transport, 6/6 after a wallet fix): capsule-keyed permission scoping is enforced **wallet-side** — a capsule cannot ride another's grant (T-19), a capsule reset orphans grants, revocation is immediate. Two hard findings for this decision:
1. **capsule_id integrity is not enforced at v0.2**: a local process that forges another capsule's id inherits its grant. Closing this needs *either* per-process/peer attestation on the transport (protocol v0.3) *or* per-capsule OS-process isolation from the engine. **A pure in-page provider cannot close it.**
2. **The v0.2 transport serves connections sequentially** — a real multi-capsule shell needs connection multiplexing or a connection-per-capsule server.

## Decision (recommended)

**Adopt Tauri (system WebView, Rust host) as the Browser MVP shell, with the Vault/wallet as a separate process; do not fork Chromium.** Sequenced:

1. **Now (pre-MVP, unlocked):** continue in `prototypes/` — extend the transport to concurrent capsules (finding 2) and design v0.3 capsule/peer attestation (finding 1). These are contract/transport tasks, not shell tasks, and gate nothing.
2. **Browser MVP:** a Tauri shell, one WebView per capsule, custom-protocol-per-capsule origins, the broker (broker-core) as the Rust-side bridge to the wallet. Storage isolation comes from the WebView-per-capsule model; permission isolation comes from the wallet (already enforced); capsule_id integrity comes from the host process vouching for each WebView's identity to the wallet (the host is trusted; the page is not).
3. **Chromium/CEF:** only if MVP compatibility data shows WebView-class shells cannot deliver required capsule isolation — its own ADR, operator sign-off, funded maintenance plan (unchanged from `engine-comparison.md`).

### Why Tauri over the alternatives, on the measured evidence

- **vs Electron:** the crux was "capsule isolation needs vs WebView API limits." The experiments show the isolation that matters is split between the wallet (permission scoping — already done, engine-independent) and the host process (capsule_id integrity — a Rust host vouching for WebView identity is natural in Tauri, awkward in Electron's Node host). Electron's stronger session-partition APIs buy less here because our permission model does not live in the browser. Tauri keeps the minimal-TCB, Rust-core posture the family already chose (wallet AC-009 lean).
- **vs Extension PoC:** cannot supply capsule_id integrity (finding 1) — extension UI and content scripts are page-adjacent and spoofable; ruled out as the product path, retained only as a compatibility probe.
- **vs Chromium fork:** total isolation control, but the experiments show we do **not** need per-site process isolation to get correct permission scoping — the wallet already provides it. The fork's maintenance treadmill (the known killer, BS §17.8) is unjustified by the current evidence.

## Consequences

- The provider/broker layer already built is reused unchanged; the engine decision does not touch it.
- Two concrete pre-MVP tasks are now well-defined and gate nothing: transport concurrency, and v0.3 capsule/peer attestation. Both are transport-contract work.
- The wallet's capsule-keyed permission fix (this session) is a prerequisite the experiment already drove in.
- `engine-comparison.md`'s "Tauri or Electron, decide with harness data" is now decided; that document remains the historical framing.
- This ADR does **not** open the product-MVP gate (criteria 2/4/5 in `browser-mvp-gate.md` still stand); it selects the engine for when the gate opens.

## Revisit

If transport attestation proves unable to deliver capsule_id integrity without per-site process isolation; if macOS WKWebView policy changes; if compatibility testing shows WebView-class storage isolation is insufficient for real dapps.

## Status of the two pre-MVP tasks (updated 2026-07-08)

Both closed on the wallet side: **finding 1** (capsule_id integrity) by transport v0.3 host attestation (wallet ADR-013 — a non-host peer cannot assert capsule identity; residual is a same-user attacker who reads the launch token, accepted per T-14); **finding 2** (transport concurrency) by the transport-concurrency commit (one task per connection; correctness-critical state serialized, reads lock-free; regression-tested). See ADR-B003 for the resulting gate status.
