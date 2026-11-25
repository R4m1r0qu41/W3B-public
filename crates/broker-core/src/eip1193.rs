//! EIP-1193 provider error shapes and the mapping from wallet refusals.
//!
//! Denial semantics per `provider-broker.md` #8: dapp-facing errors are
//! coarse — the code plus a short message. Policy internals (reason-code
//! details, risk evidence) are for the browser's own evidence UI, never for
//! hostile pages.

use crate::wire::SigningResponse;
use serde::Serialize;

/// EIP-1193 / EIP-1474 error codes the broker emits.
pub const USER_REJECTED: i32 = 4001;
pub const UNAUTHORIZED: i32 = 4100;
pub const UNSUPPORTED_METHOD: i32 = 4200;
pub const DISCONNECTED: i32 = 4900;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL: i32 = -32603;

#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[error("provider error {code}: {message}")]
pub struct ProviderError {
    pub code: i32,
    pub message: String,
}

impl ProviderError {
    pub fn unsupported_method(method: &str) -> Self {
        Self {
            code: UNSUPPORTED_METHOD,
            message: format!("method `{method}` is not supported"),
        }
    }

    pub fn invalid_params(detail: &str) -> Self {
        Self {
            code: INVALID_PARAMS,
            message: detail.to_string(),
        }
    }

    pub fn disconnected(detail: &str) -> Self {
        Self {
            code: DISCONNECTED,
            message: detail.to_string(),
        }
    }

    pub fn internal(detail: &str) -> Self {
        Self {
            code: INTERNAL,
            message: detail.to_string(),
        }
    }
}

/// What the dapp receives for a signing-response terminal state.
#[derive(Debug)]
pub enum DappResult {
    /// CONFIRMED (or BROADCAST): tx hash goes back to the page.
    TxHash(String),
    /// SIGNED (protocol v0.4): the raw signature for a `typed_data`/
    /// `personal_message` request — no tx hash, no broadcast; this is these
    /// kinds' legitimate terminal success state.
    Signature(String),
    /// Every refusal: a coarse EIP-1193 error. The full refusal reasons stay
    /// browser-side for the evidence UI.
    Refused(ProviderError),
}

/// Map a wallet signing-response to the dapp-facing result. Coarse on
/// purpose: states that mean "the wallet or user said no" are 4001; states
/// that mean "you never had (or no longer have) permission" are 4100;
/// everything unexpected is a generic internal error.
pub fn map_signing_response(resp: &SigningResponse) -> DappResult {
    match resp.state.as_str() {
        "CONFIRMED" | "BROADCAST" => match &resp.tx_hash {
            Some(h) => DappResult::TxHash(h.clone()),
            None => DappResult::Refused(ProviderError::internal(
                "confirmed without transaction hash",
            )),
        },
        "SIGNED" => match &resp.signature_hex {
            Some(s) => DappResult::Signature(s.clone()),
            None => DappResult::Refused(ProviderError::internal("signed without a signature")),
        },
        "POLICY_DENIED" | "REJECTED" | "BLOCKED" | "CANCELLED" => {
            DappResult::Refused(ProviderError {
                code: USER_REJECTED,
                message: "request refused".into(),
            })
        }
        "EXPIRED" => DappResult::Refused(ProviderError {
            code: UNAUTHORIZED,
            message: "permission expired; reconnect".into(),
        }),
        "SIMULATION_FAILED" | "SIGNING_FAILED" | "BROADCAST_FAILED" => {
            DappResult::Refused(ProviderError {
                code: USER_REJECTED,
                message: "request could not be completed".into(),
            })
        }
        _ => DappResult::Refused(ProviderError::internal("unexpected wallet state")),
    }
}

/// Map a wallet **transport-level** error code — a `{"message":"error",...}`
/// reply to `rpc_query`/`connect_request`/`transaction_request` that never
/// reached a `signing-response` state at all (chain mismatch, permission
/// gate, transport unavailable, malformed/off-whitelist request) — to a
/// coarse EIP-1193 code. Same discipline as `map_signing_response`: only the
/// code crosses to the dapp; `detail` (and the raw wallet `code` string) stay
/// browser-side for the evidence panel / activity log.
pub fn map_wallet_error_code(code: &str) -> ProviderError {
    let mapped = match code {
        // Off-whitelist / unparseable request shape.
        "malformed_or_unsupported" => UNSUPPORTED_METHOD,
        // The wallet's RPC endpoint proved it serves a different chain, or
        // is unreachable — either way, the dapp isn't getting an answer.
        "chain_mismatch" | "rpc_unavailable" => DISCONNECTED,
        "attestation_required" => DISCONNECTED,
        // Caller never had (or no longer has) the grant this needs.
        "permission_required" | "identity_required" => UNAUTHORIZED,
        // Bad input the broker should not have sent (defensive; broker-core
        // validates before forwarding).
        "size_cap_exceeded" | "unknown_chain" | "invalid_account" | "invalid_ttl"
        | "invalid_envelope" => INVALID_PARAMS,
        _ => INTERNAL,
    };
    ProviderError {
        code: mapped,
        message: "request refused".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resp(state: &str, tx_hash: Option<&str>, detail_code: &str) -> SigningResponse {
        serde_json::from_value(serde_json::json!({
            "schema_version": "1.1",
            "signing_response_id": "0193b5a4-0000-7000-8000-00000000000a",
            "signing_request_id": null,
            "request_id": "0193b5a4-0000-7000-8000-00000000000b",
            "state": state,
            "tx_hash": tx_hash,
            "refusal_reasons": [{"code": detail_code, "detail": "internal detail text"}],
            "audit_head_sequence": 7
        }))
        .unwrap()
    }

    #[test]
    fn refusals_are_coarse_no_policy_internals_leak() {
        let r = resp("POLICY_DENIED", None, "POL-APR-001");
        match map_signing_response(&r) {
            DappResult::Refused(e) => {
                assert_eq!(e.code, USER_REJECTED);
                assert!(!e.message.contains("POL-APR"), "reason codes must not leak");
                assert!(!e.message.contains("internal detail"));
            }
            _ => panic!("denial must refuse"),
        }
    }

    #[test]
    fn expiry_maps_to_unauthorized() {
        match map_signing_response(&resp("EXPIRED", None, "STATE-EXPIRED")) {
            DappResult::Refused(e) => assert_eq!(e.code, UNAUTHORIZED),
            _ => panic!(),
        }
    }

    #[test]
    fn confirmation_returns_the_hash() {
        match map_signing_response(&resp("CONFIRMED", Some("0xabc"), "")) {
            DappResult::TxHash(h) => assert_eq!(h, "0xabc"),
            _ => panic!(),
        }
    }

    /// v0.4: `SIGNED` (typed_data/personal_message) is a legitimate terminal
    /// success state carrying `signature_hex`, not `tx_hash` — distinct from
    /// CONFIRMED/BROADCAST, and must not be treated as a refusal.
    #[test]
    fn signed_returns_the_signature() {
        let r: SigningResponse = serde_json::from_value(serde_json::json!({
            "schema_version": "1.3",
            "signing_response_id": "0193b5a4-0000-7000-8000-00000000000c",
            "signing_request_id": "0193b5a4-0000-7000-8000-00000000000d",
            "request_id": "0193b5a4-0000-7000-8000-00000000000e",
            "state": "SIGNED",
            "signature_hex": "0xdeadbeef",
            "refusal_reasons": [],
            "audit_head_sequence": 3
        }))
        .unwrap();
        match map_signing_response(&r) {
            DappResult::Signature(s) => assert_eq!(s, "0xdeadbeef"),
            _ => panic!("SIGNED must return the signature, not a refusal"),
        }
    }

    #[test]
    fn wallet_error_codes_map_coarse_and_do_not_leak_the_raw_code() {
        for (code, expected) in [
            ("chain_mismatch", DISCONNECTED),
            ("rpc_unavailable", DISCONNECTED),
            ("malformed_or_unsupported", UNSUPPORTED_METHOD),
            ("permission_required", UNAUTHORIZED),
            ("unknown_chain", INVALID_PARAMS),
            ("something_new_and_unmapped", INTERNAL),
        ] {
            let e = map_wallet_error_code(code);
            assert_eq!(e.code, expected, "{code} mapped unexpectedly");
            assert!(
                !e.message.contains(code),
                "raw wallet error code `{code}` must not leak into the dapp-facing message"
            );
        }
    }
}
