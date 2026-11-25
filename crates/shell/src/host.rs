//! Shell-side broker host (Zone C plumbing, zero authority).
//!
//! Everything security-relevant about this module is *routing and honesty*,
//! not decision-making: it attaches the host-assigned capsule identity,
//! forwards page requests to the wallet over the pinned v0.3 transport
//! (attesting as the launched host first), and keeps a display-only state
//! snapshot for the shell's evidence panel. Policy, signing, and the
//! confirmation surface all stay wallet-side (`cipherlayer-vaultd`,
//! wallet ADR-012); a compromise here can annoy, not sign (AGENTS #2, #6).
//!
//! Bridge authorization is webview-label keyed: the label comes from the
//! Tauri host (`UriSchemeContext::webview_label`), never from page content,
//! so a hostile capsule page cannot reach the shell-UI state endpoint and
//! the shell UI cannot be impersonated by a capsule (ADR-B003: capsule
//! identity comes from the trusted host, never the page).

use broker_core::eip1193::{
    map_signing_response, map_wallet_error_code, DappResult, ProviderError,
};
use broker_core::transport::{TransportError, WalletClient};
use broker_core::wire::{Envelope, ReviewEvidence, SigningResponse};
use broker_core::{
    build_personal_message_envelope, build_transaction_envelope, build_typed_data_envelope,
    classify_method, CapsuleIdentity, MethodClass,
};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Webview labels, assigned by the host at window construction. Pages cannot
/// choose or change them.
pub const MAIN_LABEL: &str = "main";
pub const CAPSULE_LABEL: &str = "capsule";

/// Host-assigned capsule identity (one capsule in the MVP demo shell).
pub const CAPSULE_ORIGIN: &str = "capsule://dapp";
pub const CAPSULE_ID: &str = "capsule-shell-1";

/// What the capsule webview shows before any host-initiated navigation.
pub const CAPSULE_FIXTURE_URL: &str = "capsule://dapp/index.html";

/// Local Anvil dev chain only (wallet AGENTS: test environments by default).
pub const CHAIN_ID: u64 = 31337;

/// Anvil dev account #0 — a well-known test key's address, never real funds.
const DEFAULT_ACCOUNT: &str = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";

const PERMISSION_TTL_HOURS: i64 = 12;

/// How many activity entries the evidence panel keeps.
const ACTIVITY_CAP: usize = 60;

/// Where a bridge request is allowed to go. Deny-by-default: every
/// (label, method, path) triple not explicitly routed is refused.
#[derive(Debug, PartialEq, Eq)]
pub enum Route {
    /// Capsule page → provider request (EIP-1193 shape, forwarded to wallet).
    Provider,
    /// Shell evidence panel → display-state snapshot.
    State,
    /// Shell evidence panel (address bar) → point the capsule webview at a
    /// new URL. Host-initiated only: the capsule itself has no route to this
    /// endpoint, so hostile page content cannot navigate the capsule out
    /// from under the label-keyed authorization it was loaded under.
    Navigate,
}

/// Authorize a bridge request. The label is host-supplied (per-webview
/// scheme handler context), which is what makes this a real boundary:
/// policy internals and risk evidence go to the shell UI only, never to
/// hostile page content (provider-broker.md #8).
pub fn route(label: &str, method: &str, path: &str) -> Result<Route, (u16, &'static str)> {
    match (label, method, path) {
        (CAPSULE_LABEL, "POST", "/provider") => Ok(Route::Provider),
        (MAIN_LABEL, "GET", "/state") => Ok(Route::State),
        (MAIN_LABEL, "POST", "/navigate") => Ok(Route::Navigate),
        (CAPSULE_LABEL, _, "/state") => Err((403, "state endpoint is shell-UI only")),
        (CAPSULE_LABEL, _, "/navigate") => Err((403, "navigate endpoint is shell-UI only")),
        _ => Err((404, "no such bridge route")),
    }
}

/// Resolve the wallet's v0.3 host-attestation launch token:
/// `CIPHERLAYER_CONNECT_HOST_TOKEN` (env, preferred) or the wallet's
/// `<socket-dir>/connect-host-token` file (0600) — the two channels the
/// protocol doc defines.
pub fn resolve_host_token(socket_path: &Path) -> Result<String, String> {
    if let Ok(t) = std::env::var("CIPHERLAYER_CONNECT_HOST_TOKEN") {
        if !t.trim().is_empty() {
            return Ok(t.trim().to_string());
        }
    }
    let token_path = socket_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("connect-host-token");
    match std::fs::read_to_string(&token_path) {
        Ok(t) if !t.trim().is_empty() => Ok(t.trim().to_string()),
        _ => Err(format!(
            "no host attestation token: set CIPHERLAYER_CONNECT_HOST_TOKEN or start the wallet's \
             `connect serve` (it writes {})",
            token_path.display()
        )),
    }
}

// ---------------------------------------------------------------------------
// Display-only state for the evidence panel (serialized to the shell UI).
// These are *views*: broker-core wire types stay deserialize-only on purpose.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ActivityEntry {
    pub at: chrono::DateTime<chrono::Utc>,
    pub line: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RiskFindingView {
    pub detector: String,
    pub risk_level: String,
    pub reason_codes: Vec<String>,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvidenceView {
    pub decoded_action: String,
    pub intent_confidence: String,
    pub simulation: Option<String>,
    pub risk_level: Option<String>,
    pub risk_findings: Vec<RiskFindingView>,
    pub detectors_unavailable: Vec<String>,
    pub policy_verdict: Option<String>,
    pub policy_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SigningView {
    pub at: chrono::DateTime<chrono::Utc>,
    pub state: String,
    pub tx_hash: Option<String>,
    /// v0.4: the raw signature for typed_data/personal_message requests
    /// (state SIGNED). Display-only, same as tx_hash.
    pub signature_hex: Option<String>,
    pub audit_head_sequence: u64,
    pub refusal_codes: Vec<String>,
    pub evidence: Option<EvidenceView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShellState {
    /// Fixed banner: this panel displays evidence; it never confirms.
    pub role: &'static str,
    pub socket: String,
    pub wallet_protocol: Option<String>,
    pub attested: bool,
    pub capsule_origin: &'static str,
    pub capsule_id: &'static str,
    /// Display-only: what the capsule webview is actually showing right now
    /// (starts at the fixture, changes on host-initiated navigation).
    pub capsule_url: String,
    pub chain_id: u64,
    pub accounts: Vec<String>,
    pub last_signing: Option<SigningView>,
    pub activity: Vec<ActivityEntry>,
}

fn evidence_view(ev: &ReviewEvidence) -> EvidenceView {
    EvidenceView {
        decoded_action: ev.decoded_action.clone(),
        intent_confidence: ev.intent_confidence.clone(),
        simulation: ev.simulation.clone(),
        risk_level: ev.risk_level.clone(),
        risk_findings: ev
            .risk_findings
            .iter()
            .map(|f| RiskFindingView {
                detector: f.detector.clone(),
                risk_level: f.risk_level.clone(),
                reason_codes: f.reason_codes.clone(),
                explanation: f.explanation.clone(),
            })
            .collect(),
        detectors_unavailable: ev.detectors_unavailable.clone(),
        policy_verdict: ev.policy_verdict.clone(),
        policy_reasons: ev.policy_reasons.clone(),
    }
}

pub fn signing_view(resp: &SigningResponse) -> SigningView {
    SigningView {
        at: chrono::Utc::now(),
        state: resp.state.clone(),
        tx_hash: resp.tx_hash.clone(),
        signature_hex: resp.signature_hex.clone(),
        audit_head_sequence: resp.audit_head_sequence,
        refusal_codes: resp
            .refusal_reasons
            .iter()
            .map(|r| r.code.clone())
            .collect(),
        evidence: resp.review_evidence.as_ref().map(evidence_view),
    }
}

// ---------------------------------------------------------------------------
// The host itself
// ---------------------------------------------------------------------------

pub struct ShellHost {
    socket_path: PathBuf,
    identity: CapsuleIdentity,
    /// Held across each wallet round trip: one capsule, one in-flight
    /// request at a time (the wallet serializes the pipeline anyway).
    client: Mutex<Option<WalletClient>>,
    state: Mutex<ShellState>,
}

impl ShellHost {
    pub fn new(socket_path: PathBuf) -> Self {
        let state = ShellState {
            role: "evidence display only — approval happens on the wallet's trusted surface \
                   (cipherlayer-vaultd system dialog, wallet ADR-012)",
            socket: socket_path.display().to_string(),
            wallet_protocol: None,
            attested: false,
            capsule_origin: CAPSULE_ORIGIN,
            capsule_id: CAPSULE_ID,
            capsule_url: CAPSULE_FIXTURE_URL.to_string(),
            chain_id: CHAIN_ID,
            accounts: Vec::new(),
            last_signing: None,
            activity: Vec::new(),
        };
        Self {
            socket_path,
            identity: CapsuleIdentity::new(CAPSULE_ORIGIN, CAPSULE_ID)
                .expect("static capsule identity is non-empty"),
            client: Mutex::new(None),
            state: Mutex::new(state),
        }
    }

    pub fn socket_path_default() -> PathBuf {
        std::env::var("CIPHERLAYER_CONNECT_SOCKET")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("../wallet/data/browser-connect.sock"))
    }

    pub fn log(&self, line: impl Into<String>) {
        let mut st = self.state.lock().expect("state lock");
        st.activity.push(ActivityEntry {
            at: chrono::Utc::now(),
            line: line.into(),
        });
        if st.activity.len() > ACTIVITY_CAP {
            let drop = st.activity.len() - ACTIVITY_CAP;
            st.activity.drain(..drop);
        }
    }

    pub fn state_json(&self) -> String {
        let st = self.state.lock().expect("state lock");
        serde_json::to_string(&*st).expect("state serializes")
    }

    /// Validate a `{"url": "..."}` navigate request from the shell's own
    /// address bar. http(s) only — this is real-web navigation, not a
    /// general-purpose scheme loader, so `javascript:`, `file:`, `data:`,
    /// and `capsule:` itself are all refused rather than handed to the
    /// webview. Authorization (main-only) is enforced by `route()`, not
    /// here; this only validates the target once a request is already
    /// known to come from the trusted shell UI.
    pub fn validate_navigate(&self, raw: &str) -> Result<tauri::Url, String> {
        let parsed: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| format!("bad navigate request: {e}"))?;
        let target = parsed
            .get("url")
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .trim();
        if target.is_empty() {
            return Err("navigate request needs a non-empty `url`".to_string());
        }
        let url = tauri::Url::parse(target).map_err(|e| format!("not a valid URL: {e}"))?;
        if url.scheme() != "http" && url.scheme() != "https" {
            return Err(format!(
                "only http/https navigation is allowed (got scheme `{}`)",
                url.scheme()
            ));
        }
        Ok(url)
    }

    /// Record a navigation the host actually performed (called after
    /// `Webview::navigate` succeeds) — display-only, same discipline as
    /// every other `ShellState` field.
    pub fn record_navigation(&self, url: &tauri::Url) {
        {
            let mut st = self.state.lock().expect("state lock");
            st.capsule_url = url.to_string();
        }
        self.log(format!("host navigated capsule to {url}"));
    }

    /// Connect + capability handshake + v0.3 host attestation. Called lazily
    /// so the shell starts fine before the wallet does.
    fn ensure_wallet<'a>(
        &self,
        guard: &'a mut Option<WalletClient>,
    ) -> Result<&'a mut WalletClient, ProviderError> {
        if guard.is_none() {
            let mut client = WalletClient::connect(&self.socket_path).map_err(|e| {
                ProviderError::disconnected(&format!(
                    "wallet transport unreachable ({e}); start it with \
                     `cargo run -p cipherlayer-cli -- connect serve --vault-confirm` in the wallet repo"
                ))
            })?;
            let (protocol, _capability) = client
                .capability_query()
                .map_err(|e| ProviderError::disconnected(&e.to_string()))?;
            let token = resolve_host_token(&self.socket_path)
                .map_err(|e| ProviderError::disconnected(&e))?;
            client.attest(&token).map_err(|e| match e {
                TransportError::AttestationRejected => ProviderError::disconnected(
                    "host attestation rejected — stale token? restart wallet `connect serve` \
                     and relaunch the shell",
                ),
                other => ProviderError::disconnected(&other.to_string()),
            })?;
            {
                let mut st = self.state.lock().expect("state lock");
                st.wallet_protocol = Some(protocol.clone());
                st.attested = true;
            }
            self.log(format!(
                "wallet transport connected (protocol {protocol}), host attestation accepted (v0.3)"
            ));
            *guard = Some(client);
        }
        Ok(guard.as_mut().expect("just ensured"))
    }

    fn drop_wallet(&self, guard: &mut Option<WalletClient>, why: &str) {
        *guard = None;
        {
            let mut st = self.state.lock().expect("state lock");
            st.attested = false;
            st.wallet_protocol = None;
        }
        self.log(format!("wallet transport dropped: {why}"));
    }

    /// Handle one raw provider message from the capsule page:
    /// `{"id":…,"method":…,"params":…}` → `{"id":…,"result":…,"error":…}`.
    ///
    /// The reply always echoes the page's own id; a garbled message gets a
    /// well-formed error so the page-side promise settles.
    pub fn provider_call(&self, raw: &str) -> serde_json::Value {
        let parsed: serde_json::Value = serde_json::from_str(raw).unwrap_or_default();
        let id = parsed.get("id").cloned().unwrap_or(serde_json::Value::Null);
        let reply = |result: Result<serde_json::Value, ProviderError>| match result {
            Ok(v) => serde_json::json!({"id": id, "result": v, "error": null}),
            Err(e) => serde_json::json!({"id": id, "result": null, "error": e}),
        };
        let Some(method) = parsed.get("method").and_then(|m| m.as_str()) else {
            return reply(Err(ProviderError::invalid_params(
                "provider message needs a string `method`",
            )));
        };
        let params = parsed
            .get("params")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        reply(self.dispatch(method, &params))
    }

    fn dispatch(
        &self,
        method: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, ProviderError> {
        let class = classify_method(method).inspect_err(|e| {
            self.log(format!(
                "page request {method} rejected broker-side: {}",
                e.code
            ));
        })?;
        match class {
            MethodClass::ReadCached => {
                let st = self.state.lock().expect("state lock");
                match method {
                    "eth_accounts" => Ok(serde_json::json!(st.accounts)),
                    "eth_chainId" => Ok(serde_json::json!(format!("0x{CHAIN_ID:x}"))),
                    _ => Err(ProviderError::unsupported_method(method)),
                }
            }
            MethodClass::Connect => {
                self.log("page request eth_requestAccounts → connect_request");
                let account = std::env::var("CIPHERLAYER_SHELL_ACCOUNT")
                    .unwrap_or_else(|_| DEFAULT_ACCOUNT.to_string());
                let mut guard = self.client.lock().expect("client lock");
                let wallet = self.ensure_wallet(&mut guard)?;
                match wallet.connect_request(
                    self.identity.origin(),
                    self.identity.capsule_id(),
                    CHAIN_ID,
                    "signing",
                    std::slice::from_ref(&account),
                    PERMISSION_TTL_HOURS,
                ) {
                    Ok(grant) => {
                        let accounts = grant.accounts.clone();
                        {
                            let mut st = self.state.lock().expect("state lock");
                            st.accounts = accounts.clone();
                        }
                        self.log(format!(
                            "permission granted: {} account(s), expires {}",
                            accounts.len(),
                            grant.expires_at.to_rfc3339()
                        ));
                        Ok(serde_json::json!(accounts))
                    }
                    Err(TransportError::Wallet { code, detail }) => {
                        self.log(format!("connect refused by wallet: {code} ({detail})"));
                        Err(map_wallet_error_code(&code))
                    }
                    Err(e) => {
                        self.drop_wallet(&mut guard, &e.to_string());
                        Err(ProviderError::disconnected(&e.to_string()))
                    }
                }
            }
            MethodClass::Transaction => {
                let envelope = build_transaction_envelope(&self.identity, CHAIN_ID, params)?;
                self.forward_signing_request(envelope, "eth_sendTransaction")
            }
            // v0.4 (ADR-019): both traverse the same validate → decode →
            // risk → policy → USER_REVIEW → vault pipeline as a transaction
            // (design rule 4) — only the envelope shape differs.
            MethodClass::TypedData => {
                let envelope = build_typed_data_envelope(&self.identity, CHAIN_ID, params)?;
                self.forward_signing_request(envelope, "eth_signTypedData_v4")
            }
            MethodClass::PersonalMessage => {
                let envelope = build_personal_message_envelope(&self.identity, CHAIN_ID, params)?;
                self.forward_signing_request(envelope, "personal_sign")
            }
            // v0.4: read-only whitelist, proxied verbatim — never traverses
            // decode/policy/vault (no signing consequence, ADR-019).
            MethodClass::RpcQuery => {
                self.log(format!("page request {method} → rpc_query"));
                let mut guard = self.client.lock().expect("client lock");
                let wallet = self.ensure_wallet(&mut guard)?;
                let query_params = if params.is_null() {
                    serde_json::json!([])
                } else {
                    params.clone()
                };
                match wallet.rpc_query(
                    self.identity.origin(),
                    self.identity.capsule_id(),
                    CHAIN_ID,
                    method,
                    query_params,
                ) {
                    Ok(result) => {
                        self.log(format!("rpc_query {method} → result received"));
                        Ok(result)
                    }
                    Err(TransportError::Wallet { code, detail }) => {
                        self.log(format!(
                            "rpc_query {method} refused by wallet: {code} ({detail})"
                        ));
                        Err(map_wallet_error_code(&code))
                    }
                    Err(e) => {
                        self.drop_wallet(&mut guard, &e.to_string());
                        Err(ProviderError::disconnected(&e.to_string()))
                    }
                }
            }
        }
    }

    /// Shared by `eth_sendTransaction` / `eth_signTypedData_v4` /
    /// `personal_sign`: all three cross TB1 as a `transaction_request`
    /// traversing the wallet's full pipeline (design rule 4) — only
    /// `payload.kind` and how the envelope was built differ. The dapp-facing
    /// result is a tx hash (`evm_transaction`, states CONFIRMED/BROADCAST) or
    /// a raw signature (`typed_data`/`personal_message`, state SIGNED);
    /// every refusal maps to a coarse EIP-1193 error — reason detail and
    /// review evidence stay in the shell's evidence panel only.
    fn forward_signing_request(
        &self,
        envelope: Envelope,
        label: &str,
    ) -> Result<serde_json::Value, ProviderError> {
        self.log(format!(
            "page request {label} → signing request {} \
             (decision on the wallet's trusted surface)",
            envelope.request_id
        ));
        let mut guard = self.client.lock().expect("client lock");
        let wallet = self.ensure_wallet(&mut guard)?;
        match wallet.transaction_request(&envelope) {
            Ok(resp) => {
                let view = signing_view(&resp);
                self.log(format!(
                    "wallet decided: {} (audit head #{}{})",
                    view.state,
                    view.audit_head_sequence,
                    view.tx_hash
                        .as_deref()
                        .map(|h| format!(", tx {h}"))
                        .or_else(|| view.signature_hex.as_deref().map(|s| format!(", sig {s}")))
                        .unwrap_or_default()
                ));
                {
                    let mut st = self.state.lock().expect("state lock");
                    st.last_signing = Some(view);
                }
                match map_signing_response(&resp) {
                    DappResult::TxHash(h) => Ok(serde_json::json!(h)),
                    DappResult::Signature(s) => Ok(serde_json::json!(s)),
                    DappResult::Refused(e) => Err(e),
                }
            }
            Err(TransportError::Wallet { code, detail }) => {
                self.log(format!("signing refused by wallet: {code} ({detail})"));
                Err(map_wallet_error_code(&code))
            }
            Err(e) => {
                self.drop_wallet(&mut guard, &e.to_string());
                Err(ProviderError::disconnected(&e.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bridge is deny-by-default and label-keyed: the capsule can never
    /// read the shell's evidence state (policy internals stay out of hostile
    /// pages, provider-broker.md #8), and nothing but the two defined routes
    /// exists.
    #[test]
    fn bridge_routes_are_label_keyed_and_deny_by_default() {
        assert_eq!(
            route(CAPSULE_LABEL, "POST", "/provider"),
            Ok(Route::Provider)
        );
        assert_eq!(route(MAIN_LABEL, "GET", "/state"), Ok(Route::State));
        assert_eq!(route(MAIN_LABEL, "POST", "/navigate"), Ok(Route::Navigate));

        // Capsule must not reach the state or navigate endpoints by any method.
        assert_eq!(route(CAPSULE_LABEL, "GET", "/state").unwrap_err().0, 403);
        assert_eq!(route(CAPSULE_LABEL, "POST", "/state").unwrap_err().0, 403);
        assert_eq!(
            route(CAPSULE_LABEL, "POST", "/navigate").unwrap_err().0,
            403
        );
        assert_eq!(route(CAPSULE_LABEL, "GET", "/navigate").unwrap_err().0, 403);
        // Shell UI does not get a provider path (it is not a dapp).
        assert!(route(MAIN_LABEL, "POST", "/provider").is_err());
        // Unknown labels (future webviews) get nothing.
        assert!(route("other", "GET", "/state").is_err());
        assert!(route("other", "POST", "/provider").is_err());
        assert!(route("other", "POST", "/navigate").is_err());
        // Unknown paths/methods get nothing.
        assert!(route(CAPSULE_LABEL, "GET", "/provider").is_err());
        assert!(route(MAIN_LABEL, "GET", "/anything").is_err());
    }

    /// Only http(s) targets are accepted — the address bar is real-web
    /// navigation, not a general scheme loader; `javascript:`/`file:` can't
    /// be smuggled through it, and a garbled body fails cleanly.
    #[test]
    fn navigate_validation_is_http_s_only() {
        let host = ShellHost::new(PathBuf::from("/nonexistent/browser-connect.sock"));

        let ok = host.validate_navigate(r#"{"url":"https://opensea.io"}"#);
        assert_eq!(ok.unwrap().as_str(), "https://opensea.io/");

        assert!(host
            .validate_navigate(r#"{"url":"javascript:alert(1)"}"#)
            .is_err());
        assert!(host
            .validate_navigate(r#"{"url":"file:///etc/passwd"}"#)
            .is_err());
        assert!(host
            .validate_navigate(r#"{"url":"capsule://dapp/index.html"}"#)
            .is_err());
        assert!(host.validate_navigate(r#"{"url":""}"#).is_err());
        assert!(host.validate_navigate("not json").is_err());
    }

    /// v1.2 review_evidence maps into the display view verbatim — the shell
    /// must not soften or reinterpret wallet evidence (Unknown stays visible).
    #[test]
    fn signing_view_carries_evidence_verbatim() {
        let resp: SigningResponse = serde_json::from_value(serde_json::json!({
            "schema_version": "1.0",
            "signing_response_id": "018f0000-0000-7000-8000-000000000001",
            "signing_request_id": null,
            "request_id": "018f0000-0000-7000-8000-000000000002",
            "state": "POLICY_DENIED",
            "refusal_reasons": [{"code": "POL-APR-001", "detail": "unlimited approval"}],
            "audit_head_sequence": 9,
            "review_evidence": {
                "decoded_action": "Approve (approve(address,uint256))",
                "intent_confidence": "Exact",
                "simulation": "Success (2 standing limitations)",
                "risk_level": "UNKNOWN — no evidence; never treated as low",
                "risk_findings": [{
                    "detector": "scam-guard-cli",
                    "risk_level": "High",
                    "reason_codes": ["ALW-UNLIMITED"],
                    "explanation": "unlimited allowance requested"
                }],
                "detectors_unavailable": ["fpl"],
                "policy_verdict": "Deny",
                "policy_reasons": ["POL-APR-001: unlimited approval"]
            }
        }))
        .unwrap();
        let view = signing_view(&resp);
        assert_eq!(view.state, "POLICY_DENIED");
        assert_eq!(view.refusal_codes, vec!["POL-APR-001"]);
        let ev = view.evidence.expect("evidence view");
        assert_eq!(
            ev.risk_level.as_deref(),
            Some("UNKNOWN — no evidence; never treated as low")
        );
        assert_eq!(ev.policy_verdict.as_deref(), Some("Deny"));
        assert_eq!(ev.risk_findings[0].detector, "scam-guard-cli");
        assert_eq!(ev.detectors_unavailable, vec!["fpl"]);
    }

    /// A garbled page message still gets a well-formed, id-echoing error —
    /// the page promise settles, nothing is forwarded.
    #[test]
    fn garbled_provider_messages_fail_cleanly() {
        let host = ShellHost::new(PathBuf::from("/nonexistent/browser-connect.sock"));
        let reply = host.provider_call("not json at all");
        assert!(reply["result"].is_null());
        assert_eq!(reply["error"]["code"], -32602);

        let reply = host.provider_call(r#"{"id": 7, "params": []}"#);
        assert_eq!(reply["id"], 7);
        assert_eq!(reply["error"]["code"], -32602);
    }

    /// Unsupported methods are refused broker-side (4200), never forwarded —
    /// and a dead socket surfaces as 4900 for wallet-bound methods.
    #[test]
    fn dispatch_is_deny_by_default_and_fails_disconnected() {
        let host = ShellHost::new(PathBuf::from("/nonexistent/browser-connect.sock"));
        let reply = host.provider_call(r#"{"id": 1, "method": "eth_sign", "params": []}"#);
        assert_eq!(reply["error"]["code"], 4200);

        let reply =
            host.provider_call(r#"{"id": 2, "method": "eth_requestAccounts", "params": []}"#);
        assert_eq!(reply["error"]["code"], 4900);

        // Cached reads work without a wallet: empty accounts, fixed chain.
        let reply = host.provider_call(r#"{"id": 3, "method": "eth_chainId", "params": []}"#);
        assert_eq!(reply["result"], "0x7a69");
    }

    /// v0.4 methods route the same as v0.3's — deny-by-default classification
    /// happens broker-side (never touching the socket) for genuinely
    /// unsupported methods, while methods newly classified as forwardable
    /// (typed data / personal_sign / the read-only whitelist) still fail
    /// disconnected (4900) rather than unsupported (4200) against a dead
    /// socket — proof they reach the wallet-bound path at all.
    #[test]
    fn v04_methods_are_classified_and_reach_the_wallet_bound_path() {
        let host = ShellHost::new(PathBuf::from("/nonexistent/browser-connect.sock"));

        let reply = host.provider_call(
            r#"{"id":1,"method":"eth_signTypedData_v4","params":["0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","{}"]}"#,
        );
        assert_eq!(reply["error"]["code"], 4900);

        let reply = host.provider_call(
            r#"{"id":2,"method":"personal_sign","params":["0x68656c6c6f","0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"]}"#,
        );
        assert_eq!(reply["error"]["code"], 4900);

        let reply = host.provider_call(r#"{"id":3,"method":"eth_blockNumber","params":[]}"#);
        assert_eq!(reply["error"]["code"], 4900);

        // Still deny-by-default: an off-whitelist, rpc-adjacent method never
        // reaches the wallet-bound path at all.
        let reply =
            host.provider_call(r#"{"id":4,"method":"eth_sendRawTransaction","params":["0x"]}"#);
        assert_eq!(reply["error"]["code"], 4200);
    }
}
