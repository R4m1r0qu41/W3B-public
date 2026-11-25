# Per-Site Wallet and Network Permissions

**Version:** 0.2 (design document; gate OPEN since ADR-B003, MVP shell landed per ADR-B004 — implementation status: broker-core partial, see docs/testing/dapp-compat-matrix.md) · Primary source: v0.3 modular-stack guide (§Browser Integration), aligned with US §5 per-site controls. This is a headline safety feature, not an option buried in settings.

## Model

Each capsule (site) has a permission profile the user can see and edit in one place:

```text
Site: experimental-defi.example        [capsule: a3f2…, profile: default]
Mode: ⦿ isolated  ○ read-only  ○ testnet/devnet only

Allowed wallets (by tier):
☑ Hot Wallet 1        (hot — fast signing, small balances)
☐ Warm Vault          (warm — spending limits, extra confirmation)
☐ Cold Vault          (cold — NEVER connectable from sites; hardware only)

Allowed networks:
☑ Anvil local (31337)      [development]
☑ Sepolia (11155111)       [testnet]
☐ Ethereum (1)             [mainnet]

Session: expires 2026-07-04 18:00 · [revoke all] [reset capsule]
```

## Rules

1. **Cold-wallet blocking is absolute by default.** Accounts labeled *cold* are never disclosed to any capsule; enabling them for a site requires an explicit, escalated, wallet-side flow — and the default UI does not offer it (v0.3: "Cold Vault: no direct dapp connections").
2. **Tier filtering happens at disclosure time.** A site never learns that undisclosed accounts exist (privacy: no enumeration).
3. **Network allowlists are per-site.** A dapp allowed on Sepolia does not get mainnet by default; chain-switch requests outside the allowlist prompt, and in testnet/devnet mode they are refused.
4. **Read-only mode** serves account/chain queries but refuses all signing-class methods at the broker with a clear dapp-visible error.
5. **Expiry is mandatory** (wallet contract: `dapp-permission.v1` requires `expires_at`). Dormant grants surface for cleanup (approval-manager overlap, BS §6.8).
6. **Revocation is one click** and audited; "reset capsule" additionally rotates the capsule identity so nothing ambient survives.
7. **Permission changes are wallet-side state.** The browser renders and requests; the wallet grants and enforces. The browser cannot mint itself a permission.

## Why this is the wedge

Per-site wallet/network isolation with cold-wallet blocking is ranked wedge #3 in the unified scope's competitive positioning (US §14) — mainstream wallets expose one global connection surface. This document, plus `dapp-capsules.md`, defines the behavior; enforcement mechanics live in the wallet permission model the browser consumes.
