# Real-Dapp Compatibility Matrix

**Version:** 0.3 (first live real-dapp run recorded, 2026-07-17) · Evidence
document, not a design doc.

## 0-pre. First live run against a production dapp (opensea.io, 2026-07-17)

Operator-driven session in the MVP shell (host-navigated capsule → real
`https://opensea.io`), wallet `connect serve --yes` on Anvil 31337. Results:

| Step | Result | Evidence |
|---|---|---|
| EIP-6963 discovery | **WORKS** | OpenSea's wallet modal listed "CipherLayer — Installed" above MetaMask/WalletConnect, zero rendezvous UI |
| `eth_requestAccounts` | **WORKS** (after fix, see below) | account granted, OpenSea session established, avatar rendered |
| `personal_sign` (SIWE) | **WORKS** | full SIWE message decoded in the evidence panel (`PersonalSign`, confidence High, POL-RSK-002 escalate), vault signed (state `SIGNED`, audit #568), OpenSea accepted the signature and proceeded to signup |
| Account creation | server-side refusal | "Failed to claim username" — OpenSea's backend rejects `Chain ID: 31337` (its SIWE message echoed our chain honestly); not a protocol failure |

**Finding 1 — remote-page CSP breaks fetch-based bridges (fixed 2026-07-17).**
The original capsule→host bridge was `fetch("clbridge://…")`; production
https pages (WebKit CSP/mixed-content rules) block fetches to custom
schemes, so every provider call died in-page — discovery worked (pure
injected JS) but the first click did nothing, silently. Fix: the bridge now
rides Tauri command IPC (`provider_msg`, native postMessage — page CSP
cannot touch it), granted to the capsule label only
(`capabilities/capsule-provider.json`), same zero-authority broker entry
point, fetch kept as local-page fallback. Any wallet-in-webview design that
bridges via custom-scheme fetch will hit this on real dapps.

**Finding 2 — chain 31337 is a dead end for real dapp backends.** OpenSea
accepted discovery, connect, and a valid SIWE signature, then refused
account creation server-side because the chain is one it doesn't support.
The frontier is now exactly the deliberate ADR-019 deferral: chain
identity/switching (plus a mainnet-fork dev profile so real backends see a
chain id they recognize). This finding should drive the chain-switch design
pass (BUILD-STATE next #3) with a concrete acceptance test: complete an
OpenSea login. Written per ADR-B004's "honest gaps" (everything proven against the MVP
shell so far is proven against **one fixture dapp**,
`fixtures/sample-dapp.html`). This matrix asks a narrower question: if a real
production dapp were pointed at the shell today, which of its actual
`window.ethereum`/EIP-1193 calls would work, and why. It is kept in sync with
`crates/broker-core`'s actual classification — not a compatibility claim
about real dapps, which still needs the run against real dapp code v0.1
called for.

**What changed since v0.1:** v0.1 found four blocking gaps — no
`window.ethereum`/EIP-6963 discovery, no forwardable message-signing methods,
no browser-connect wire shape to carry them, and no read-only RPC proxy. All
four are closed in this rev (ADR-019 wallet-side, ADR-B005 + this browser
repo's v0.4 broker-core/shell work). What is **not** closed: chain-switching
(`wallet_switchEthereumChain`/`wallet_addEthereumChain`, deliberately
deferred by ADR-019 — a per-site-permissions design question, not a
mechanical wire addition), `wallet_watchAsset`, EIP-2255 permissions
methods, and multi-capsule/real-site browsing generally (ADR-B004's honest
gaps, unchanged). See §5 for the updated counts.

## 0. What the shell actually supports today

`crates/broker-core/src/lib.rs::classify_method` (v0.4):

```rust
"eth_requestAccounts"    => Connect
"eth_sendTransaction"    => Transaction        (payload.kind = evm_transaction)
"eth_signTypedData_v4"   => TypedData          (payload.kind = typed_data)
"personal_sign"          => PersonalMessage    (payload.kind = personal_message)
"eth_accounts" | "eth_chainId" => ReadCached
"eth_call" | "eth_estimateGas" | "eth_blockNumber" | "eth_getBalance"
  | "eth_gasPrice" | "eth_getTransactionByHash" | "eth_getTransactionReceipt"
  | "net_version"        => RpcQuery           (proxied verbatim, no pipeline)
other                     => Err(4200 UNSUPPORTED_METHOD)
```

Fourteen methods now reach the wallet in some form (up from four in v0.1).
Everything else — chain-switching, `wallet_watchAsset`, EIP-2255
permissions, `eth_sign`, and anything off the strict `RpcQuery` whitelist
(including rpc-adjacent but consequence-bearing methods like
`eth_sendRawTransaction`) — is still denied with EIP-1193 code `4200` before
it ever reaches the wallet. This remains correct deny-by-default behavior
(`provider-broker.md` #2, `AGENTS.md`) and should not change further without
a deliberate classification decision.

`TypedData`/`PersonalMessage` are **not** a bypass: both build a
`transaction-request-envelope.v1` with `payload.kind` set (v1.1, additive)
and traverse `wallet.transaction_request` — the exact same full pipeline
(validate → decode → risk → policy → USER_REVIEW → vault) as
`eth_sendTransaction`. The wallet's own EIP-712 classifier
(`cipherlayer_evm::typed_data`, wallet requirements-matrix CL-EVM-003) is now
closed end-to-end: EIP-2612/DAI-style permits project into the same `Permit`
action calldata `approve`/`permit` produces, so an unlimited-value typed-data
permit hits the identical default-deny policy rule (`POL-APR-001`) as an
unlimited `approve`. Anything that doesn't match a recognized permit shape
classifies `unknown` and takes policy's restrictive unknown-action path —
never a quiet "Sign" prompt. CL-EVM-004 (distinguishing login/ownership/
delegation/governance signature *classes* specifically, as opposed to
permit-vs-unknown) is still **partial** per the wallet's own
requirements-matrix — those classes fall to `unknown` today, which is safe
(escalate-or-deny) but not yet semantically labeled.

`RpcQuery` is a pure proxy: none of its eight methods can move funds, change
permissions, or produce a signature, so the wallet forwards them straight to
its own RPC endpoint (never a caller-supplied one) after verifying the
endpoint's chain identity — no decode/policy/vault stage, no audit event per
query (ADR-019: high-volume, no forensic value; connection-level enforcement
still applies to every call).

`window.ethereum`/EIP-6963 discovery (§4, formerly the structural
prerequisite blocking this whole matrix) is now implemented — see ADR-B005.

## 1. Method matrix

Legend — **Classification**: *forwarded* (reaches the wallet), *denied 4200*
(broker-core rejects before the wallet sees it), *N/A* (not something an
EIP-1193 injected provider is expected to serve). **Consequence**: *works*,
*degraded* (dapp continues with reduced functionality or a caught error),
*hard-broken* (the flow the method exists for cannot complete).
**Disposition**: *must-forward read-only* (candidate to proxy to the
wallet's read path, no signing consequence), *needs wallet pipeline support*
(consequence-bearing; must traverse decode→simulate→risk→policy, not a
bypass), *correctly denied* (should stay denied), *defer* (real but not
next).

| Method | Who calls it, and when | Classification today | Consequence | Disposition |
|---|---|---|---|---|
| `eth_requestAccounts` | Every dapp, on Connect click (Uniswap, OpenSea, Aave, ENS, all bridges) | forwarded → `Connect` | works | — |
| `eth_accounts` | Every dapp, silent reconnect on page load | forwarded → `ReadCached` | works | — |
| `eth_chainId` | Every dapp, connect-time + on every `chainChanged` check (wagmi/viem, ethers, web3.js all call this first) | forwarded → `ReadCached` | works | — |
| `net_version` | Legacy path: web3.js and ethers v5-era dapps call this alongside/instead of `eth_chainId` | **forwarded → `RpcQuery`** | works | — |
| `eth_call` | Balance/allowance/price reads (ERC-20 `balanceOf`, `allowance`; Uniswap quoters; Aave reserve data; ENS resolution) whenever a dapp routes reads through the injected provider instead of its own RPC key | **forwarded → `RpcQuery`** | works — proxied to the wallet's own RPC endpoint after chain-identity verification | — |
| `eth_estimateGas` | Called before `eth_sendTransaction` to size gas limits and show fee estimates (Uniswap swap preview, most send flows) | **forwarded → `RpcQuery`** | works | — |
| `eth_blockNumber` | Countdown/confirmation-tracking UI, some price-staleness checks | **forwarded → `RpcQuery`** | works | — |
| `eth_getBalance` | Native balance display for the connected account | **forwarded → `RpcQuery`** | works | — |
| `eth_gasPrice` | Fee display/estimation alongside `eth_estimateGas` | **forwarded → `RpcQuery`** | works | — |
| `eth_getTransactionReceipt` / `eth_getTransactionByHash` | Post-send confirmation polling | **forwarded → `RpcQuery`** | works | — |
| `eth_sendTransaction` | Every state-changing action: swaps, approvals, NFT purchases, bridge deposits | forwarded → `Transaction`, full pipeline (decode→simulate→risk→policy→USER_REVIEW) | works, by design the one flow this shell was built and demoed against | — |
| `personal_sign` | Sign-In with Ethereum (SIWE) login (OpenSea and many dapps use this to authenticate before showing the main app, no on-chain tx involved); also some off-chain order signing | **forwarded → `PersonalMessage`**, full pipeline, `payload.kind=personal_message`, terminal state `SIGNED`/`signature_hex` | works — the dapp's login step completes (subject to policy: the message is decoded and reviewed, not rubber-stamped) | — |
| `eth_signTypedData_v4` | EIP-712 structured signing: Uniswap Permit2 gasless approvals, OpenSea/Seaport listing and offer signatures, Aave delegated-signature flows, most modern "gasless" UX | **forwarded → `TypedData`**, full pipeline, `payload.kind=typed_data` | works for recognized permit shapes (policy-reviewed, unlimited-value permits denied same as calldata `approve`); unrecognized typed-data shapes escalate rather than silently sign — see §3 | — |
| `wallet_switchEthereumChain` | Any multi-chain dapp or bridge prompting a network switch (virtually all bridges; most dapps supporting L2s) | denied 4200 (**deliberately deferred, ADR-019**: chain-switching is a per-site-permissions design question, not forwarded even though v0.4 shipped other signing methods) | **hard-broken** — the dapp's chain-switch prompt fails outright, and most bridge/multi-chain UIs treat that as fatal rather than falling back | needs wallet pipeline support — `per-site-permissions.md` already describes "chain-switch requests outside the allowlist prompt" as intended per-site behavior; the design exists on paper, not in broker-core |
| `wallet_addEthereumChain` | Dapps onboarding an unfamiliar chain (L2s, new testnets) | denied 4200 (same deliberate deferral as above) | hard-broken for the onboarding flow; no-op if the chain is already known to the wallet | needs wallet pipeline support, same design gap as `wallet_switchEthereumChain` |
| `wallet_watchAsset` | "Add token to wallet" buttons (ERC-20/721 add-to-wallet prompts) | denied 4200 | degraded — cosmetic feature, dapp continues without it | defer |
| `wallet_getPermissions` / `wallet_requestPermissions` (EIP-2255) | Rare; permission-scoping UIs, a handful of dapps that request scoped account access | denied 4200 | degraded — low real-world incidence | defer |
| `eth_subscribe` / `eth_unsubscribe` | Event-log/new-block subscriptions over the injected provider | denied 4200 | N/A in practice — mainstream injected wallets (MetaMask et al.) don't support these over EIP-1193 either; dapps needing subscriptions use a separate WS RPC | N/A / correctly denied (no compat regression versus the wallets dapps are actually built against) |
| `eth_sign` | Legacy raw-hash signing (dangerous — no message context shown to a user) | denied 4200 | correctly degrades a dapp still calling it (vanishingly rare today; deprecated for phishing risk) | correctly denied — should never be forwarded regardless of dapp compatibility pressure |
| `eth_sendRawTransaction` | Not called by dapps through an injected provider in practice (it's the "already signed" broadcast primitive) — included here because it sounds read-adjacent and must not be mistaken for one | denied 4200 (not on the `RpcQuery` whitelist — consequence-bearing, broadcasts a signature) | correctly denied | correctly denied — off-whitelist by design, regression-tested |
| `eth_exportPrivateKey` (non-standard) | Not a real EIP-1193 method; included in broker-core's own test as a hypothetical hostile probe | denied 4200 | n/a | correctly denied |

## 2. Read-only vs. consequence-bearing, restated

The matrix above still sorts into two groups, now both closed:

- **Chain-state reads with no signing consequence** — `net_version`,
  `eth_call`, `eth_estimateGas`, `eth_blockNumber`, `eth_getBalance`,
  `eth_gasPrice`, `eth_getTransactionReceipt`/`eth_getTransactionByHash`.
  None of these can move funds, change permissions, or produce a signature.
  **Closed in v0.4**: `MethodClass::RpcQuery` proxies all eight verbatim to
  the wallet's own RPC endpoint (`rpc_query`, ADR-019) — no new pipeline
  stage, no new confirmation surface, exactly the "pure proxy candidate"
  disposition v0.1 called for.
- **Methods that produce a signature or a state change** —
  `eth_sendTransaction` (already worked), `personal_sign`,
  `eth_signTypedData_v4` — **closed in v0.4** via `payload.kind`
  (`transaction-request-envelope.v1` v1.1) traversing the identical
  decode→risk→policy→USER_REVIEW→vault pipeline as a transaction — and
  `wallet_switchEthereumChain`/`wallet_addEthereumChain` — **still open**,
  deliberately deferred by ADR-019 pending the per-site-permissions design
  work, not a mechanical wire gap like the other three were.

## 3. `eth_signTypedData_v4` — status update

v0.1 identified three independent blocking layers. All three are now closed:

1. **broker-core classification** — closed: `classify_method` maps
   `eth_signTypedData_v4` → `MethodClass::TypedData`,
   `build_typed_data_envelope` normalizes EIP-1193 params `[address,
   typedDataJson]` (accepting either a JSON string or an already-parsed
   object) into the wire shape.
2. **browser-connect contract shape** — closed: `transaction-request-
   envelope.v1` v1.1 (additive) carries `payload.kind=typed_data` +
   `payload.typed_data_json`, mirroring the wallet's internal
   `signing-request.v1` shape across TB1.
3. **Wallet-side EIP-712 parsing** — closed end-to-end per the wallet's own
   `requirements-matrix.md`: CL-EVM-003 is now **done end-to-end**
   (`cipherlayer_evm::typed_data` — domainSeparator/hashStruct/encodeType/
   encodeData per spec, bounded against cyclic/deep type graphs; EIP-2612
   and DAI-style permits classify unlimited-value grants as unlimited-
   approval authority, feeding the same default-deny policy path as calldata
   `approve`). CL-EVM-004 (distinguishing signature *classes* — login vs.
   ownership-transfer vs. delegation vs. governance, as opposed to
   permit-vs-unknown) is still **partial**: those classes fall to `unknown`
   today, which is the *safe* path (escalate-or-deny, per
   `docs/threat-model/abuse-cases.md`'s requirement that undecodable typed
   data never gets a quiet "Sign" prompt) but not yet semantically labeled
   for the user.

Net: Uniswap Permit2, OpenSea/Seaport, and Aave delegated-signature flows now
reach the wallet's real policy pipeline. An unlimited-value permit is denied
exactly like an unlimited calldata `approve`; a typed-data shape the
classifier doesn't recognize as a known permit escalates rather than
silently signs. What is *not* yet true: fine-grained signature-class
labeling (CL-EVM-004) for shapes outside the permit family — those still
present to policy/the user as "unknown," which is honest but not maximally
informative.

## 4. EIP-6963 / provider discovery status in the shell

**Closed in this rev — ADR-B005.** v0.1 found this was the structural
prerequisite blocking the entire method matrix (a real dapp's own "detect
wallet" logic failed before any request was ever sent, since the shell only
exposed `window.cipherlayer`, not `window.ethereum`, and fired no EIP-6963
event). Current state:

- `prototypes/provider-harness/provider.js` (reused unmodified by
  `crates/shell/src/main.rs`) injects `window.cipherlayer` (back-compat with
  the fixture) **and** `window.ethereum` (claimed only if unclaimed — never
  displaces another injected provider).
- `eip6963:announceProvider` fires once at injection and again on every
  `eip6963:requestProvider` the page dispatches, carrying `{info: {uuid,
  name: "CipherLayer", icon: <inline SVG data URI>, rdns:
  "org.cipherlayer.browser"}, provider}` — `uuid` fresh per page load,
  `rdns` the stable product identity.
- This covers both real-world detection patterns: legacy direct
  `window.ethereum` access (ethers' `BrowserProvider`, wagmi/viem's
  "injected" connector without EIP-6963) and EIP-6963 `announceProvider`
  listeners (wagmi's multi-injected-provider discovery, RainbowKit, most
  2024+ wallet-connection UIs).
- `docs/testing/compatibility-test-plan.md` Phase 1's "EIP-6963 announcement"
  injection-matrix item is now implemented, not just planned.

**Honest bound**: this closes discovery *inside CipherLayer's own shell*
only. A dapp running in a third-party browser still cannot discover
CipherLayer at all — ADR-B005 explicitly defers that "external-reach"
problem (WalletConnect or a successor) to a future ADR; it is a different
problem (rendezvous across two processes/devices) from the one this section
closes (a page and its own injected provider sharing one webview).

## 5. Summary disposition counts

| Disposition | Methods |
|---|---|
| Works today | `eth_requestAccounts`, `eth_accounts`, `eth_chainId`, `eth_sendTransaction`, `personal_sign`, `eth_signTypedData_v4`, `net_version`, `eth_call`, `eth_estimateGas`, `eth_blockNumber`, `eth_getBalance`, `eth_gasPrice`, `eth_getTransactionReceipt`/`eth_getTransactionByHash` (13, treating the receipt/hash pair as one) |
| Needs wallet pipeline support (consequence-bearing, deliberately deferred — ADR-019) | `wallet_switchEthereumChain`, `wallet_addEthereumChain` (2) |
| Correctly denied, keep denied | `eth_sign`, `eth_sendRawTransaction`, `eth_exportPrivateKey` (3) |
| Defer (low incidence / cosmetic) | `wallet_watchAsset`, `wallet_getPermissions`/`wallet_requestPermissions` (3) |
| N/A vs. real wallets | `eth_subscribe`/`eth_unsubscribe` (2, treated as one row) |
| Structural prerequisite — CLOSED this rev (ADR-B005) | `window.ethereum` shim + EIP-6963 `announceProvider` |
| Structural prerequisite — still open | third-party-browser discovery (WalletConnect/successor, deferred by ADR-B005); one capsule / fixture-dapp-only browsing (ADR-B004 honest gaps, unchanged) |

## Appendix — factual notes found while reading (not corrected here)

Per the task instructions, these are flagged for the owning docs, not edited:

- `docs/architecture/provider-broker.md` still headers itself "**Version:**
  0.1 (design document; gate closed)" — the gate opened per ADR-B003
  (2026-07-07, `docs/architecture/browser-mvp-gate.md`) and the MVP shell
  landed per ADR-B004 (2026-07-09), so this header appears stale relative to
  the gate's current state. Left as-is; flagging for whoever owns that doc.
- `docs/architecture/per-site-permissions.md` is also still headered
  "(design document; gate closed)" for the same reason — its content (e.g.
  "chain-switch requests outside the allowlist prompt") is written as
  already-active wallet-side behavior, but nothing in broker-core forwards
  `wallet_switchEthereumChain` today (§1), so the design intent and the
  current implementation state are worth reconciling explicitly rather than
  left to be inferred from two documents that don't cross-reference each
  other.
