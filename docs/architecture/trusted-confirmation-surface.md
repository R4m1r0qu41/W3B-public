# Trusted Confirmation-Surface Requirements

**Version:** 0.1 · **Tier v1 implemented 2026-07-07** in the wallet repo (ADR-012): `cipherlayer-vaultd` renders these requirements as a macOS system dialog + secret-phrase marker, fail-closed, with the red-team exercise recorded. Requirements 1–4 and 6–10 met; requirement 5 met at the OS-chrome + secret-phrase tiers (SEP/biometric binding and occlusion detection deferred to the next tier). The wallet CLI's terminal prompt remains the acknowledged spoofable fallback (T-12) when the daemon surface is not in use.

The confirmation surface is where a human authorizes a signature. It is **wallet-owned** (Zone D adjacent), never browser content. These requirements bind both the W2 desktop wallet and the post-gate browser integration.

## Hard requirements

1. **No web content inside the final confirmation surface.** Not an iframe, not a webview, not extension HTML, not theme-controlled markup (BS §7.3; BP §4.15).
2. **Not theme-, plugin-, or dapp-controllable.** No API by which skins, plugins, or pages can draw into, occlude reliably, restyle, or script the surface; a plugin must never suppress a warning or alter displayed transaction meaning (BP §4.16, v0.3 guide "Custom UI and Skins").
3. **Process separation.** Rendered by the Vault-side process (post ADR-004 split), not by any renderer-hosting process.
4. **Deterministic content.** What it displays comes only from the deterministic pipeline: decoded intent, simulation evidence (with limitations), risk evidence (with unknowns visible), policy reasons. Advisory/AI text is visually segregated and labeled advisory.
5. **Spoofing resistance**, layered (BS §7.3 options, to be selected at W2 with an ADR). Operator direction (UQ-003, 2026-07-04): on macOS, prefer the strongest tier first — **Secure Enclave Processor (SEP)-backed** approaches on Apple silicon (e.g., LocalAuthentication/biometric gating bound to the confirmation, keys attested via the Secure Enclave), degrading through lower-level techniques for Intel-era Macs:
   - OS-level window treatment (system-owned chrome, macOS first);
   - consistent anti-spoofing visual identifier;
   - user-selected secret phrase/pattern displayed on every genuine surface;
   - optional secure attention sequence;
   - hardware-wallet confirmation as the strongest tier for high-consequence actions.
6. **Visible provenance split.** The user can always distinguish: browser content vs dapp request vs analysis evidence vs Vault confirmation (BS §7.3).
7. **Escalation is explicit.** A policy `escalate` verdict renders differently (stronger affordance, explicit acknowledgement) than a plain allow; a `deny` is never rendered as confirmable.
8. **Unknown stays visible.** Unknown/conflicting evidence is rendered as such — never collapsed into a green check (BP §4.18).
9. **Auditable.** Every render → decision pair emits audit events (UserDecision with surface identifier).
10. **Fail closed.** If the surface cannot attest its own integrity (process handshake failure, unexpected occlusion signals where detectable), it refuses to confirm.

## Acceptance evidence required before "trusted" may appear in any claim

Red-team exercise: a test page + test extension + test theme each attempt to imitate or occlude the surface; the imitation must be distinguishable using the documented markers, and occlusion/spoof attempts must be detected or made non-actionable. Results recorded in the wallet's security-claims register before any user-facing wording uses "trusted surface".
