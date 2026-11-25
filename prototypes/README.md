# Prototypes — compatibility experiments (ADR-B001 scope)

Unlocked 2026-07-05 by ADR-B001 (scoped gate opening; criteria 1–3 evidence recorded in the gate checklist). Contents are **experiments, not products**: no shell builds, no engine forks, no confirmation surfaces.

## provider-harness

Drives the full browser-connect contract against a live wallet over the protocol v0.2 Unix-socket transport, as a true external consumer (pinned schemas only, no wallet crates). Run:

```bash
# terminal 1 (wallet repo)
make anvil-up && cargo run -p cipherlayer-cli -- connect serve --yes

# terminal 2 (this repo)
cargo run -p provider-harness
```

Six checks: broker-side deny-by-default (4200) · capability handshake · scoped grant + idempotent reconnect · confirmed native transfer (tx hash + audit head) · unlimited-approval denial (POL-APR reason codes stay browser-side; the page sees a coarse 4001) · revocation effective on the next request.

`provider.js` is the injectable EIP-1193 provider (Zone B artifact) for the upcoming webview injection experiment.

## webview-harness

The provider-injection experiment: one WKWebView window (wry) loads `fixtures/sample-dapp.html` over a custom protocol, injects `provider.js` before page scripts run, and bridges each EIP-1193 request through broker-core to the wallet socket. One window = one capsule; the page is treated as hostile; no confirmation surface is rendered. Run the wallet serve command above, then:

```bash
cargo run -p webview-harness
```

Buttons on the fixture page walk the five paths (detect, connect, confirmed transfer, blocked drain attempt, unsupported method). Findings feed the engine ADR (injection timing, IPC ergonomics, capsule-origin control on WKWebView).

## capsule-isolation

Measures whether the wallet's capsule-keyed permission model actually isolates one dapp capsule from another (the crux `engine-comparison.md` named). Six measurements over the live transport: scoped grant, confused-deputy refusal (T-19), reset-capsule orphaning, immediate revocation, and an honest record of the v0.2 capsule_id-integrity gap. Run the wallet serve command above, then:

```bash
cargo run -p capsule-isolation
```

Its findings, plus the webview-harness and provider-harness data, are the evidence base for **ADR-B002** (engine selection: Tauri/system-WebView + separate Vault process, no Chromium fork).
