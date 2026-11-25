# Browser–Wallet Integration Architecture

**Version:** 0.1 · The authoritative protocol document is published by the wallet repo and pinned at [`../../imports/wallet/browser-wallet-protocol.md`](../../imports/wallet/browser-wallet-protocol.md) (commit + checksum in `../../imports/wallet/PIN.json`). This document describes the browser's side of the integration.

## Division of authority

| Concern | Browser (this repo) | Wallet (sibling) |
|---|---|---|
| Dapp detection, capsule creation | ✔ | — |
| Provider endpoint (EIP-1193 shape) | ✔ broker | — |
| Origin/capsule identity attachment | ✔ broker (mandatory) | verifies non-empty, keys permissions by it |
| Per-site permission **UI** | ✔ | — |
| Permission **grant/enforcement** | — | ✔ (`dapp-permission.v1`, mandatory expiry) |
| Decode / simulate / risk / policy | — | ✔ deterministic pipeline |
| Confirmation surface | never | ✔ wallet-owned trusted surface |
| Signing, broadcast | never | ✔ Vault |
| Audit trail | adds capsule context | ✔ hash-chained log |
| Risk display (reason codes, unknowns visible) | ✔ renders | produces (via Scam Guard contract) |

## Message flow (browser perspective)

1. Capsule loads a page; quiet dapp detection (provider access attempt, known request shapes) — no UI unless the page calls.
2. `eth_requestAccounts` → broker builds `connect_request` (origin, capsule, requested chains, scope, requested expiry) → wallet discloses → user grants on wallet surface → browser caches grant *metadata* for UI only.
3. Read-class calls → broker checks scope → wallet read path.
4. Signing-class calls → broker checks per-site mode (read-only? tier allowed? network allowed?) → `transaction_request` envelope → wallet pipeline → wallet trusted surface → `signing_response` (terminal state + refusal reasons + audit head reference) → broker maps to EIP-1193 response/error.
5. Revocation: from browser UI (requests wallet-side revoke) or wallet-side directly; capsule reset rotates identity.

## Versioning discipline

The browser tracks the pinned contract version (`protocol_version` 0.1). It must: refuse to speak versions it doesn't know; surface a clear incompatibility message rather than degrade silently; and update only via the PIN procedure. Contract change requests flow to the wallet repo as issues/ADRs — never by editing pinned files.

## Open items owned jointly (tracked for protocol v0.2)

Transport selection + browser-process authentication to the wallet · capsule identity attestation · session continuity across restarts · batch requests · EIP-712 typed-data preview payloads.
