//! Wire types mirroring the pinned browser-connect schemas
//! (`imports/wallet/`, PIN.json at wallet commit `87284e5`).
//!
//! Mirrored by hand, on purpose: linking wallet crates is forbidden
//! (AGENTS.md #4) precisely so this repo can only couple to the versioned
//! contract. Deserialization is tolerant of unknown fields (additive minor
//! versions must not break the broker); serialization emits only v1 fields.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Caller {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capsule_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvelopePayload {
    pub from: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_wei: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_limit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<u64>,
    /// v0.4 (protocol v0.4, additive): absent = `evm_transaction` (unchanged
    /// v0.3 wire shape — old envelopes stay byte-for-byte identical); set to
    /// `"typed_data"`/`"personal_message"` for `eth_signTypedData_v4`/
    /// `personal_sign` requests (ADR-019).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Required iff `kind == "typed_data"`: the full `eth_signTypedData_v4`
    /// JSON payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed_data_json: Option<String>,
    /// Required iff `kind == "personal_message"`: the raw message bytes,
    /// `0x`-hex (the bytes need not be valid UTF-8).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_hex: Option<String>,
}

/// transaction-request-envelope.v1
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub schema_version: String,
    pub request_id: uuid::Uuid,
    pub received_at: chrono::DateTime<chrono::Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<String>,
    pub caller: Caller,
    pub chain_id: u64,
    pub access: String,
    pub payload: EnvelopePayload,
}

/// signing-response.v1 (v1.1: signing_request_id nullable for pre-Vault refusals)
#[derive(Debug, Clone, Deserialize)]
pub struct SigningResponse {
    pub schema_version: String,
    pub signing_response_id: uuid::Uuid,
    pub signing_request_id: Option<uuid::Uuid>,
    pub request_id: uuid::Uuid,
    pub state: String,
    #[serde(default)]
    pub tx_hash: Option<String>,
    /// v1.3 (protocol v0.4, additive): the raw ECDSA signature for
    /// `typed_data`/`personal_message` requests (65-byte r/s/v hex). Absent
    /// for `evm_transaction` requests and for any refusal — those terminal
    /// states are `SIGNED` with no broadcast step (there is nothing to
    /// broadcast for a signature-only request).
    #[serde(default)]
    pub signature_hex: Option<String>,
    #[serde(default)]
    pub refusal_reasons: Vec<RefusalReason>,
    pub audit_head_sequence: u64,
    /// v1.2 (optional, additive): display-only pipeline evidence — decoded
    /// intent, simulation outcome, risk findings, policy verdict/reasons.
    /// Evidence for the shell to *display* (AGENTS #6: the browser shows
    /// evidence; the wallet's trusted surface decides). Never authoritative.
    #[serde(default)]
    pub review_evidence: Option<ReviewEvidence>,
}

/// signing-response.v1.2 review_evidence (display-only; tolerant of additions)
#[derive(Debug, Clone, Deserialize)]
pub struct ReviewEvidence {
    pub decoded_action: String,
    pub intent_confidence: String,
    #[serde(default)]
    pub simulation: Option<String>,
    #[serde(default)]
    pub risk_level: Option<String>,
    #[serde(default)]
    pub risk_findings: Vec<RiskFinding>,
    #[serde(default)]
    pub detectors_unavailable: Vec<String>,
    #[serde(default)]
    pub policy_verdict: Option<String>,
    #[serde(default)]
    pub policy_reasons: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RiskFinding {
    pub detector: String,
    pub risk_level: String,
    #[serde(default)]
    pub reason_codes: Vec<String>,
    pub explanation: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RefusalReason {
    pub code: String,
    pub detail: String,
}

/// wallet-capability.v1 (fields the broker consumes; tolerant of additions)
#[derive(Debug, Clone, Deserialize)]
pub struct WalletCapability {
    pub schema_version: String,
    pub protocol_versions: Vec<String>,
    pub chains: Vec<u64>,
    pub methods: Vec<String>,
}

/// dapp-permission.v1 (granted-permission metadata; the protocol doc's
/// "may cache" list covers exactly this)
#[derive(Debug, Clone, Deserialize)]
pub struct Permission {
    pub schema_version: String,
    pub permission_id: uuid::Uuid,
    pub caller_key: String,
    pub chain_id: u64,
    pub access: String,
    pub accounts: Vec<String>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pre-v1.2 responses (no review_evidence) still parse; v1.2 responses
    /// deliver the typed display evidence. Additive-minor tolerance is a
    /// contract requirement (protocol design rule 6).
    #[test]
    fn signing_response_review_evidence_optional_and_parsed() {
        let old = serde_json::json!({
            "schema_version": "1.0",
            "signing_response_id": "018f0000-0000-7000-8000-000000000001",
            "signing_request_id": null,
            "request_id": "018f0000-0000-7000-8000-000000000002",
            "state": "POLICY_DENIED",
            "refusal_reasons": [],
            "audit_head_sequence": 1
        });
        let parsed: SigningResponse = serde_json::from_value(old).unwrap();
        assert!(parsed.review_evidence.is_none());

        let new = serde_json::json!({
            "schema_version": "1.0",
            "signing_response_id": "018f0000-0000-7000-8000-000000000003",
            "signing_request_id": null,
            "request_id": "018f0000-0000-7000-8000-000000000004",
            "state": "POLICY_DENIED",
            "refusal_reasons": [{"code": "POL-APR-001", "detail": "unlimited approval"}],
            "audit_head_sequence": 5,
            "review_evidence": {
                "decoded_action": "Approve (approve(address,uint256))",
                "intent_confidence": "Exact",
                "simulation": "Success (2 standing limitations)",
                "risk_level": "High",
                "risk_findings": [{
                    "detector": "scam-guard-cli",
                    "risk_level": "High",
                    "reason_codes": ["ALW-UNLIMITED"],
                    "explanation": "unlimited allowance requested",
                    "future_field": "ignored"
                }],
                "detectors_unavailable": [],
                "policy_verdict": "Deny",
                "policy_reasons": ["POL-APR-001: unlimited approval"],
                "future_field": "ignored"
            }
        });
        let parsed: SigningResponse = serde_json::from_value(new).unwrap();
        let ev = parsed.review_evidence.expect("evidence");
        assert_eq!(ev.policy_verdict.as_deref(), Some("Deny"));
        assert_eq!(ev.risk_findings[0].detector, "scam-guard-cli");
        assert_eq!(ev.risk_findings[0].reason_codes, vec!["ALW-UNLIMITED"]);
    }
}
