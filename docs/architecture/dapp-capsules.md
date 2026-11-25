# Dapp Capsule Model

**Version:** 0.1 (design document; gate closed)

A **capsule** is the per-dapp isolation unit (Zone B): one dapp origin → one capsule → one identity the broker attaches to every request. Capsules make "isolated dapp sessions" (US §5, v0.3 guide) concrete.

## Isolation targets (BS §7.1 Zone B)

Per capsule, isolated from every other capsule and from the shell:

- cookies, localStorage, sessionStorage, IndexedDB, cache
- service workers and their registrations
- wallet permissions (accounts, chains, access class) — held by the wallet, keyed by capsule identity
- chain/network permissions (per-site network allowlist)
- clipboard access (explicit, prompted, logged)
- file access (none by default)
- extension access (extensions cannot reach into capsules by default)
- network rules (per-capsule request policies; testnet/devnet dapp mode forces dev endpoints)
- cross-dapp communication (none, except explicit user-mediated flows)

## Capsule identity

`capsule_id` = stable identifier bound to (origin, profile, capsule generation). It is attached by the broker to every envelope (`transaction-request-envelope.v1` → `caller.capsule_id`) so that:

- permissions are scoped to the capsule, not ambient (confused-deputy defense, wallet threat T-19);
- the audit trail reconstructs which surface asked for what;
- "reset this dapp" = new capsule generation = all prior grants orphaned.

Identity attestation to the wallet (so a rogue local process cannot claim a capsule ID) is a protocol **v0.3** open item, **measured** by the `capsule-isolation` experiment: at v0.2 a process forging a capsule_id inherits its grant. The experiment also drove a wallet fix — dapp permissions are now keyed by `capsule_id`, so capsules of one origin never share a grant and a reset orphans grants (previously they were keyed by origin alone). What remains is *integrity* of the id, which the host process must vouch for (ADR-B002).

## Capsule modes (from the v0.3 guide)

Per-site, user-selectable, enforced by broker + wallet permission scopes:

| Mode | Meaning |
|---|---|
| **read-only** | capsule may query accounts/chain state; signing requests are refused at the broker with a visible notice |
| **hot-wallet-only** | only accounts labeled *hot* are offered/connectable |
| **cold-wallet blocking** | accounts labeled *cold* are never disclosed to this capsule (default: always on for all capsules) |
| **testnet/devnet dapp mode** | capsule's network scope restricted to testnet/dev chains; mainnet requests refused |
| **isolated session** | default; described above |

## Compatibility consequence (honest)

Strict isolation is the number-one predicted breakage source (BS §17.1): dapps assuming shared storage, multi-tab sessions, popup OAuth flows, or window.opener tricks will misbehave. The compatibility test plan (`../testing/compatibility-test-plan.md`) exists to measure exactly this before any public claim of Web2/dapp competency.
