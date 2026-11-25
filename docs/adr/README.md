# Browser ADRs

- **ADR-B001** — Operator opens the browser gate for scoped pre-MVP work (transport, broker library, compatibility harnesses). *Accepted 2026-07-05.*
- **ADR-B002** — Browser engine selection: Tauri / system-WebView + separate Vault process; no Chromium fork. Informed by the three sanctioned experiments. *Proposed (recommendation).*
- **ADR-B003** — Operator opens the gate for the MVP shell build (full opening, supersedes ADR-B001's scope). *Accepted 2026-07-07 — satisfies gate criterion 6; the gate is open.*
- **ADR-B004** — MVP shell design: Tauri host, two webviews, label-gated `clbridge` scheme bridge. *Accepted 2026-07-09.*
- **ADR-B005** — Dapp discovery: native in-shell identity (`window.ethereum` + EIP-6963) is the primary mechanism; WalletConnect v2 compatibility deferred to a future ADR as an external-reach path. *Accepted 2026-07-16 (operator directive).*

Cross-repo: the wallet-side decisions these depend on live in `../../../wallet/docs/adr/` — notably ADR-004 (Vault process boundary), ADR-012 (trusted confirmation surface), and ADR-013 (transport v0.3 host attestation).
