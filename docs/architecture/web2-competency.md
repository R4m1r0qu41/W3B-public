# Normal Web2 Browsing Competency Requirements

**Version:** 0.1 · Source: US §1/§5 ("A Web3-only browser that renders ordinary sites poorly is dead on arrival"), BS §6.1, v0.3 guide. These are **requirements the eventual browser must meet**, stated now so the engine decision and test plan are honest about them.

## Baseline (BS §6.1 — all required for browser MVP)

- multi-tab browsing (restore, drag, pin)
- address bar with sane URL/search disambiguation
- bookmarks (add, organize, import)
- history (search, clear, ranges)
- downloads manager
- standard search-engine integration (user-choosable)
- profiles (and the privacy rule: ordinary browsing separated from wallet identity, BS §15)
- private browsing
- password-manager integration or native password management
- extension support **subject to the security model** (extensions never reach capsules or confirmation surfaces by default)
- developer tools
- standard web compatibility (see measurable bar below)
- cross-platform design path: desktop first, macOS first for prototyping, Windows/Linux later (BS §6.1)

## The measurable bar

"Renders ordinary sites well" becomes testable requirements:

1. **Compat corpus:** top-style everyday sites (news, video, docs/office web apps, webmail, social, banking-grade login flows, OAuth flows) render and function without site-specific fixes — corpus and pass criteria in `../testing/compatibility-test-plan.md`.
2. **Media:** HTML5 video/audio incl. common DRM-free codecs plays; WebRTC basic calls work; (EME/DRM support is an explicit decision to record, not an accident).
3. **Auth flows:** multi-tab OAuth, passkeys/WebAuthn, and popup-based logins work in ordinary (non-capsule) browsing.
4. **Performance:** page-load and scroll jank within an agreed factor of the host engine's reference browser on the same hardware (target set when the engine is chosen).
5. **Stability:** the crash rule from the v0.3 guide's pain-point list — UI flows (profile setup, tab churn, large media pages) must not crash; stability is a product feature.
6. **No Web3 tax:** with no dapp present, no Web3 UI intrudes and no measurable overhead is added to ordinary browsing (quiet detection only, US §5 escalation table).

## Non-goals at browser MVP

Full extension-store parity · mobile browsing · sync services (privacy architecture first) · being anyone's default browser (the adoption target is the *intentional* Web3 browser, BS §3 — but it must still be a competent ordinary browser when used as one).
