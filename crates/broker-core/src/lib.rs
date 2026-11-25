//! Provider and capability broker (Zone C) — browser-side plumbing with
//! **zero authority** (see `docs/architecture/provider-broker.md`).
//!
//! This crate never links wallet code (AGENTS.md #4): it speaks to the wallet
//! only through the pinned browser-connect wire contract (`imports/wallet/`),
//! over the protocol v0.2 same-user Unix-socket transport. It validates,
//! scopes, attaches identity, forwards, and maps results into EIP-1193
//! shapes. Every decision — policy, signing, confirmation — happens
//! wallet-side; a broker compromise can annoy, not sign.

pub mod eip1193;
pub mod transport;
pub mod wire;

use eip1193::ProviderError;
use wire::{Caller, Envelope, EnvelopePayload};

/// Wallet-bound method classes the broker recognizes. Deny-by-default:
/// anything not classified here is rejected with 4200, never forwarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodClass {
    /// eth_requestAccounts — becomes a connect_request (permission grant).
    Connect,
    /// eth_sendTransaction — becomes a transaction_request, payload.kind
    /// `evm_transaction` (full pipeline).
    Transaction,
    /// eth_signTypedData_v4 — becomes a transaction_request, payload.kind
    /// `typed_data` (protocol v0.4, ADR-019). Same full pipeline as a
    /// transaction: decode → risk → policy → USER_REVIEW → vault. Not a
    /// bypass — the wallet decides everything.
    TypedData,
    /// personal_sign — becomes a transaction_request, payload.kind
    /// `personal_message` (protocol v0.4, ADR-019). Same full pipeline.
    PersonalMessage,
    /// eth_accounts / eth_chainId — served from granted-permission metadata
    /// the broker may cache (protocol doc "may cache" list).
    ReadCached,
    /// The v0.4 read-only RPC whitelist (`rpc_query`, ADR-019): pure
    /// chain-state reads with no signing consequence, proxied to the
    /// wallet's own RPC endpoint. Never traverses decode/policy/vault.
    RpcQuery,
}

/// The v0.4 `rpc_query` whitelist — STRICT and not caller-extensible, mirrors
/// `RPC_QUERY_WHITELIST` in the pinned wallet contract (`rpc-query.v1.json`,
/// `browser-wallet-protocol.md` §"Read-only RPC proxy"). Anything off this
/// list is refused broker-side (4200), never forwarded — including methods
/// that sound read-only-adjacent, like `eth_sendRawTransaction`, which is
/// consequence-bearing and not on the wallet's whitelist either.
const RPC_QUERY_WHITELIST: &[&str] = &[
    "eth_call",
    "eth_estimateGas",
    "eth_blockNumber",
    "eth_getBalance",
    "eth_gasPrice",
    "eth_getTransactionByHash",
    "eth_getTransactionReceipt",
    "net_version",
];

pub fn classify_method(method: &str) -> Result<MethodClass, ProviderError> {
    match method {
        "eth_requestAccounts" => Ok(MethodClass::Connect),
        "eth_sendTransaction" => Ok(MethodClass::Transaction),
        "eth_signTypedData_v4" => Ok(MethodClass::TypedData),
        "personal_sign" => Ok(MethodClass::PersonalMessage),
        "eth_accounts" | "eth_chainId" => Ok(MethodClass::ReadCached),
        m if RPC_QUERY_WHITELIST.contains(&m) => Ok(MethodClass::RpcQuery),
        other => Err(ProviderError::unsupported_method(other)),
    }
}

/// The identity the broker attaches to every wallet-bound message.
/// Constructing one with empty fields is impossible by design — the wallet
/// rejects identity-less envelopes and the broker must never fabricate
/// or generalize identity (provider-broker.md #3).
#[derive(Debug, Clone)]
pub struct CapsuleIdentity {
    origin: String,
    capsule_id: String,
}

#[derive(Debug, thiserror::Error)]
#[error("capsule identity requires non-empty origin and capsule_id")]
pub struct IdentityError;

impl CapsuleIdentity {
    pub fn new(origin: &str, capsule_id: &str) -> Result<Self, IdentityError> {
        if origin.trim().is_empty() || capsule_id.trim().is_empty() {
            return Err(IdentityError);
        }
        Ok(Self {
            origin: origin.to_string(),
            capsule_id: capsule_id.to_string(),
        })
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub fn capsule_id(&self) -> &str {
        &self.capsule_id
    }

    fn as_caller(&self) -> Caller {
        Caller {
            kind: "dapp".into(),
            origin: Some(self.origin.clone()),
            app_id: None,
            capsule_id: Some(self.capsule_id.clone()),
        }
    }
}

/// Build a schema-conformant transaction-request-envelope.v1 from EIP-1193
/// eth_sendTransaction params, with identity attached.
pub fn build_transaction_envelope(
    identity: &CapsuleIdentity,
    chain_id: u64,
    params: &serde_json::Value,
) -> Result<Envelope, ProviderError> {
    let tx = params
        .get(0)
        .and_then(|v| v.as_object())
        .ok_or_else(|| ProviderError::invalid_params("params[0] must be a transaction object"))?;
    let from = tx
        .get("from")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ProviderError::invalid_params("`from` is required"))?;
    // EIP-1193 carries hex quantities; the wallet envelope takes decimal or
    // hex strings — pass value through as decimal to stay on the schema's
    // canonical form.
    let value_wei = match tx.get("value").and_then(|v| v.as_str()) {
        Some(h) => Some(hex_quantity_to_decimal(h)?),
        None => None,
    };
    let gas_limit = match tx.get("gas").and_then(|v| v.as_str()) {
        Some(h) => Some(hex_quantity_to_decimal(h)?),
        None => None,
    };
    Ok(Envelope {
        schema_version: "1.0".into(),
        request_id: uuid::Uuid::now_v7(),
        received_at: chrono::Utc::now(),
        protocol_version: Some(transport::PROTOCOL_VERSION.into()),
        caller: identity.as_caller(),
        chain_id,
        access: "signing".into(),
        payload: EnvelopePayload {
            from: from.to_string(),
            to: tx.get("to").and_then(|v| v.as_str()).map(str::to_string),
            value_wei,
            data: tx.get("data").and_then(|v| v.as_str()).map(str::to_string),
            gas_limit,
            nonce: None,
            kind: None,
            typed_data_json: None,
            message_hex: None,
        },
    })
}

/// Build a `transaction-request-envelope.v1` (`payload.kind = "typed_data"`,
/// protocol v0.4) from `eth_signTypedData_v4` params: `[address,
/// typedDataJson]` (EIP-712 convention — the JSON payload is a string on the
/// wire; some dapps pass an already-parsed object instead, so both are
/// accepted and normalized to a string here). This traverses the same full
/// pipeline as a transaction (design rule 4) — the wallet decodes,
/// classifies (EIP-2612/DAI-style permits hit the same unlimited-approval
/// policy rule as calldata `approve`), and decides; the broker only shapes
/// the wire message.
pub fn build_typed_data_envelope(
    identity: &CapsuleIdentity,
    chain_id: u64,
    params: &serde_json::Value,
) -> Result<Envelope, ProviderError> {
    let arr = params
        .as_array()
        .ok_or_else(|| ProviderError::invalid_params("params must be an array"))?;
    let from = arr
        .first()
        .and_then(|v| v.as_str())
        .ok_or_else(|| ProviderError::invalid_params("params[0] must be the signing address"))?;
    let typed_data_value = arr.get(1).ok_or_else(|| {
        ProviderError::invalid_params("params[1] must be the eth_signTypedData_v4 JSON payload")
    })?;
    let typed_data_json = match typed_data_value {
        serde_json::Value::String(s) => s.clone(),
        other => serde_json::to_string(other).map_err(|e| {
            ProviderError::invalid_params(&format!("typed data payload not serializable: {e}"))
        })?,
    };
    Ok(Envelope {
        schema_version: "1.1".into(),
        request_id: uuid::Uuid::now_v7(),
        received_at: chrono::Utc::now(),
        protocol_version: Some(transport::PROTOCOL_VERSION.into()),
        caller: identity.as_caller(),
        chain_id,
        access: "signing".into(),
        payload: EnvelopePayload {
            from: from.to_string(),
            to: None,
            value_wei: None,
            data: None,
            gas_limit: None,
            nonce: None,
            kind: Some("typed_data".into()),
            typed_data_json: Some(typed_data_json),
            message_hex: None,
        },
    })
}

/// Build a `transaction-request-envelope.v1` (`payload.kind =
/// "personal_message"`, protocol v0.4) from `personal_sign` params:
/// `[messageHex, address]` (EIP-1193/JSON-RPC convention — the message
/// arrives already `0x`-hex encoded, since the raw bytes need not be valid
/// UTF-8). Same full pipeline as a transaction; the wallet decodes (valid
/// UTF-8 shown as text, otherwise hex) and decides.
pub fn build_personal_message_envelope(
    identity: &CapsuleIdentity,
    chain_id: u64,
    params: &serde_json::Value,
) -> Result<Envelope, ProviderError> {
    let arr = params
        .as_array()
        .ok_or_else(|| ProviderError::invalid_params("params must be an array"))?;
    let message_hex = arr
        .first()
        .and_then(|v| v.as_str())
        .ok_or_else(|| ProviderError::invalid_params("params[0] must be the 0x-hex message"))?;
    let stripped = message_hex
        .strip_prefix("0x")
        .ok_or_else(|| ProviderError::invalid_params("personal_sign message must be 0x-hex"))?;
    if stripped.len() % 2 != 0 || !stripped.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ProviderError::invalid_params(
            "personal_sign message must be well-formed 0x-hex",
        ));
    }
    let from = arr
        .get(1)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ProviderError::invalid_params("params[1] must be the signing address"))?;
    Ok(Envelope {
        schema_version: "1.1".into(),
        request_id: uuid::Uuid::now_v7(),
        received_at: chrono::Utc::now(),
        protocol_version: Some(transport::PROTOCOL_VERSION.into()),
        caller: identity.as_caller(),
        chain_id,
        access: "signing".into(),
        payload: EnvelopePayload {
            from: from.to_string(),
            to: None,
            value_wei: None,
            data: None,
            gas_limit: None,
            nonce: None,
            kind: Some("personal_message".into()),
            typed_data_json: None,
            message_hex: Some(message_hex.to_string()),
        },
    })
}

fn hex_quantity_to_decimal(h: &str) -> Result<String, ProviderError> {
    let stripped = h
        .strip_prefix("0x")
        .ok_or_else(|| ProviderError::invalid_params("quantities must be 0x-prefixed hex"))?;
    if stripped.is_empty()
        || stripped.len() > 64
        || !stripped.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(ProviderError::invalid_params("malformed hex quantity"));
    }
    // u128 covers gas; values above go through as hex untouched (the wallet
    // accepts decimal only in v1, so oversize values must fail loudly here
    // rather than be truncated).
    match u128::from_str_radix(stripped, 16) {
        Ok(v) => Ok(v.to_string()),
        Err(_) => Err(ProviderError::invalid_params(
            "quantity exceeds the broker's u128 passthrough; split or reduce",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn identity() -> CapsuleIdentity {
        CapsuleIdentity::new("https://dapp.example", "capsule-42").unwrap()
    }

    #[test]
    fn unknown_methods_are_rejected_not_forwarded() {
        for m in [
            "eth_sign",
            "wallet_addEthereumChain",
            "wallet_switchEthereumChain",
            "wallet_watchAsset",
            "eth_exportPrivateKey",
            "made_up_method",
            // Off-whitelist and rpc-adjacent but consequence-bearing: must
            // never be treated as a read (ADR-019 deliberately excludes it
            // from RPC_QUERY_WHITELIST; it broadcasts a signed transaction).
            "eth_sendRawTransaction",
        ] {
            let e = classify_method(m).unwrap_err();
            assert_eq!(e.code, 4200, "{m} must map to unsupported-method");
        }
    }

    /// v0.4 (ADR-019): eth_signTypedData_v4/personal_sign become forwardable
    /// consequence-bearing requests; the read-only whitelist becomes
    /// RpcQuery. Nothing else changes classification.
    #[test]
    fn v04_methods_classify_correctly() {
        assert_eq!(
            classify_method("eth_signTypedData_v4").unwrap(),
            MethodClass::TypedData
        );
        assert_eq!(
            classify_method("personal_sign").unwrap(),
            MethodClass::PersonalMessage
        );
        for m in [
            "eth_call",
            "eth_estimateGas",
            "eth_blockNumber",
            "eth_getBalance",
            "eth_gasPrice",
            "eth_getTransactionByHash",
            "eth_getTransactionReceipt",
            "net_version",
        ] {
            assert_eq!(
                classify_method(m).unwrap(),
                MethodClass::RpcQuery,
                "{m} must classify as RpcQuery"
            );
        }
    }

    #[test]
    fn identity_cannot_be_empty() {
        assert!(CapsuleIdentity::new("", "c").is_err());
        assert!(CapsuleIdentity::new("https://x", " ").is_err());
    }

    #[test]
    fn envelope_carries_identity_and_decimal_quantities() {
        let env = build_transaction_envelope(
            &identity(),
            31337,
            &json!([{
                "from": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
                "to": "0x00000000000000000000000000000000000000bb",
                "value": "0xde0b6b3a7640000",
                "gas": "0x5208"
            }]),
        )
        .unwrap();
        assert_eq!(env.caller.origin.as_deref(), Some("https://dapp.example"));
        assert_eq!(env.caller.capsule_id.as_deref(), Some("capsule-42"));
        assert_eq!(
            env.payload.value_wei.as_deref(),
            Some("1000000000000000000")
        );
        assert_eq!(env.payload.gas_limit.as_deref(), Some("21000"));
        assert_eq!(env.access, "signing");
    }

    #[test]
    fn malformed_quantities_fail_loudly() {
        let bad =
            json!([{ "from": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266", "value": "1000" }]);
        assert_eq!(
            build_transaction_envelope(&identity(), 31337, &bad)
                .unwrap_err()
                .code,
            -32602
        );
    }

    /// The emitted envelope must validate against the *pinned* wallet schema —
    /// this is the browser-side half of the contract conformance story.
    #[test]
    fn envelope_conforms_to_pinned_schema() {
        let schema_src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../imports/wallet/transaction-request-envelope.v1.json"
        ))
        .expect("pinned schema present");
        let schema: serde_json::Value = serde_json::from_str(&schema_src).unwrap();
        let compiled = jsonschema::validator_for(&schema).expect("schema compiles");
        let env = build_transaction_envelope(
            &identity(),
            31337,
            &json!([{
                "from": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
                "to": "0x00000000000000000000000000000000000000bb",
                "value": "0x0",
                "data": "0x"
            }]),
        )
        .unwrap();
        let instance = serde_json::to_value(&env).unwrap();
        let msgs: Vec<String> = compiled
            .iter_errors(&instance)
            .map(|e| e.to_string())
            .collect();
        if !msgs.is_empty() {
            panic!("envelope violates pinned schema: {msgs:?}");
        }
    }

    fn compiled_envelope_schema() -> jsonschema::Validator {
        let schema_src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../imports/wallet/transaction-request-envelope.v1.json"
        ))
        .expect("pinned schema present");
        let schema: serde_json::Value = serde_json::from_str(&schema_src).unwrap();
        jsonschema::validator_for(&schema).expect("schema compiles")
    }

    const EIP2612_UNLIMITED_PERMIT: &str = r#"{
        "types": {
            "EIP712Domain": [
                {"name":"name","type":"string"},
                {"name":"version","type":"string"},
                {"name":"chainId","type":"uint256"},
                {"name":"verifyingContract","type":"address"}
            ],
            "Permit": [
                {"name":"owner","type":"address"},
                {"name":"spender","type":"address"},
                {"name":"value","type":"uint256"},
                {"name":"nonce","type":"uint256"},
                {"name":"deadline","type":"uint256"}
            ]
        },
        "primaryType": "Permit",
        "domain": {
            "name": "TestToken", "version": "1", "chainId": 31337,
            "verifyingContract": "0x0000000000000000000000000000000000009999"
        },
        "message": {
            "owner": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "spender": "0x0000000000000000000000000000000000002222",
            "value": "115792089237316195423570985008687907853269984665640564039457584007913129639935",
            "nonce": "0",
            "deadline": "0"
        }
    }"#;

    /// v0.4: eth_signTypedData_v4 params `[address, typedDataJson]` build a
    /// `payload.kind = "typed_data"` envelope that validates against the
    /// pinned schema — a JSON *string* payload (the common case: dapps pass
    /// `JSON.stringify(typedData)` as params[1]).
    #[test]
    fn typed_data_envelope_from_string_payload_conforms_to_pinned_schema() {
        let env = build_typed_data_envelope(
            &identity(),
            31337,
            &json!([
                "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
                EIP2612_UNLIMITED_PERMIT
            ]),
        )
        .unwrap();
        assert_eq!(env.payload.kind.as_deref(), Some("typed_data"));
        assert!(env.payload.typed_data_json.is_some());
        assert!(env.payload.to.is_none());
        let instance = serde_json::to_value(&env).unwrap();
        let schema = compiled_envelope_schema();
        let msgs: Vec<String> = schema
            .iter_errors(&instance)
            .map(|e| e.to_string())
            .collect();
        if !msgs.is_empty() {
            panic!("typed_data envelope violates pinned schema: {msgs:?}");
        }
    }

    /// Some dapps pass the typed-data object directly (not pre-stringified);
    /// the broker must normalize it to a JSON string either way.
    #[test]
    fn typed_data_envelope_from_object_payload_is_normalized_to_a_string() {
        let obj: serde_json::Value = serde_json::from_str(EIP2612_UNLIMITED_PERMIT).unwrap();
        let env = build_typed_data_envelope(
            &identity(),
            31337,
            &json!(["0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266", obj]),
        )
        .unwrap();
        let carried: serde_json::Value =
            serde_json::from_str(&env.payload.typed_data_json.unwrap()).unwrap();
        assert_eq!(carried["primaryType"], "Permit");
    }

    /// v0.4: personal_sign params `[messageHex, address]` build a
    /// `payload.kind = "personal_message"` envelope conforming to schema.
    #[test]
    fn personal_message_envelope_conforms_to_pinned_schema() {
        let msg_hex = format!("0x{}", hex::encode(b"sign in to example.org"));
        let env = build_personal_message_envelope(
            &identity(),
            31337,
            &json!([msg_hex, "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"]),
        )
        .unwrap();
        assert_eq!(env.payload.kind.as_deref(), Some("personal_message"));
        assert!(env.payload.message_hex.is_some());
        let instance = serde_json::to_value(&env).unwrap();
        let schema = compiled_envelope_schema();
        let msgs: Vec<String> = schema
            .iter_errors(&instance)
            .map(|e| e.to_string())
            .collect();
        if !msgs.is_empty() {
            panic!("personal_message envelope violates pinned schema: {msgs:?}");
        }
    }

    #[test]
    fn personal_message_rejects_non_hex_message() {
        let e = build_personal_message_envelope(
            &identity(),
            31337,
            &json!(["not-hex", "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"]),
        )
        .unwrap_err();
        assert_eq!(e.code, -32602);
    }

    /// Old (pre-v0.4) evm_transaction envelopes must stay byte-for-byte
    /// unaffected: `kind`/`typed_data_json`/`message_hex` are all absent on
    /// the wire, exactly a v0.3 client's shape.
    #[test]
    fn evm_transaction_envelope_omits_v04_fields_on_the_wire() {
        let env = build_transaction_envelope(
            &identity(),
            31337,
            &json!([{ "from": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266", "value": "0x0" }]),
        )
        .unwrap();
        let instance = serde_json::to_value(&env).unwrap();
        let payload = instance.get("payload").unwrap().as_object().unwrap();
        assert!(!payload.contains_key("kind"));
        assert!(!payload.contains_key("typed_data_json"));
        assert!(!payload.contains_key("message_hex"));
    }
}
