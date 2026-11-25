//! Provider-connect compatibility harness — the first sanctioned browser-repo
//! experiment (ADR-B001). Plays a dapp capsule + broker end-to-end against a
//! live wallet over the protocol v0.4 socket (ADR-019) and reports each
//! contract path, including the v0.4 additions: typed_data/personal_message
//! signing and the read-only `rpc_query` proxy.
//!
//! Run (two terminals):
//!   wallet$  make anvil-up && cargo run -p cipherlayer-cli -- connect serve --yes
//!   browser$ cargo run -p provider-harness
//!
//! Explicitly NOT a product: no rendering, no shell, no engine. It exists to
//! exercise the pinned contract and produce data for the engine ADR.

use broker_core::eip1193::{map_signing_response, DappResult, UNSUPPORTED_METHOD};
use broker_core::transport::WalletClient;
use broker_core::{
    build_personal_message_envelope, build_transaction_envelope, build_typed_data_envelope,
    classify_method, CapsuleIdentity,
};
use serde_json::json;
use std::path::PathBuf;

const UNLIMITED_APPROVE_TO_SELF: &str =
    "0x095ea7b30000000000000000000000000707070707070707070707070707070707070707ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

/// EIP-2612 `permit` typed data granting an unlimited (`type(uint256).max`)
/// allowance — the typed-data twin of `UNLIMITED_APPROVE_TO_SELF` above.
/// `owner` matches the harness account so the wallet's permit classifier has
/// a coherent signer to reason about.
fn eip2612_unlimited_permit_json(owner: &str, chain_id: u64) -> String {
    json!({
        "types": {
            "EIP712Domain": [
                {"name": "name", "type": "string"},
                {"name": "version", "type": "string"},
                {"name": "chainId", "type": "uint256"},
                {"name": "verifyingContract", "type": "address"}
            ],
            "Permit": [
                {"name": "owner", "type": "address"},
                {"name": "spender", "type": "address"},
                {"name": "value", "type": "uint256"},
                {"name": "nonce", "type": "uint256"},
                {"name": "deadline", "type": "uint256"}
            ]
        },
        "primaryType": "Permit",
        "domain": {
            "name": "HarnessToken",
            "version": "1",
            "chainId": chain_id,
            "verifyingContract": "0x0000000000000000000000000000000000009999"
        },
        "message": {
            "owner": owner,
            "spender": "0x0000000000000000000000000000000000002222",
            "value": "115792089237316195423570985008687907853269984665640564039457584007913129639935",
            "nonce": "0",
            "deadline": "0"
        }
    })
    .to_string()
}

fn socket_path() -> PathBuf {
    std::env::var("CIPHERLAYER_CONNECT_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../wallet/data/browser-connect.sock"))
}

fn main() {
    println!("== CipherLayer provider-connect harness (not a product) ==\n");
    let mut pass = 0u32;
    let mut fail = 0u32;
    let mut check = |name: &str, ok: bool, detail: String| {
        println!(
            "[{}] {name}\n      {detail}\n",
            if ok { "PASS" } else { "FAIL" }
        );
        if ok {
            pass += 1
        } else {
            fail += 1
        }
    };

    // Path 0 — broker-side deny-by-default (no wallet needed).
    let unknown = classify_method("wallet_watchAsset").unwrap_err();
    check(
        "unknown method rejected broker-side (4200, never forwarded)",
        unknown.code == UNSUPPORTED_METHOD,
        format!("code={} message={}", unknown.code, unknown.message),
    );

    let identity = CapsuleIdentity::new("https://harness.example", "capsule-harness-1")
        .expect("static identity");

    // Path 0.5 — FPL origin evidence (display-only; the wallet still decides).
    // The broker never links scam-guard code: when the operator points
    // CIPHERLAYER_FPL_BIN at a scam-guard binary, the harness asks it about a
    // known-hostile origin and shows the evidence a real shell would render.
    if let Ok(fpl) = std::env::var("CIPHERLAYER_FPL_BIN") {
        let hostile = "metamask.io.wallet-verify.top";
        let out = std::process::Command::new(&fpl)
            .args(["--json", "check", "domain", hostile])
            .output();
        let flagged = out
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| serde_json::from_slice::<serde_json::Value>(&o.stdout).ok())
            .map(|a| {
                a["overall"] == "high" && a["findings"].as_array().is_some_and(|f| !f.is_empty())
            })
            .unwrap_or(false);
        check(
            "FPL origin evidence: hostile lookalike origin flagged before connect",
            flagged,
            format!("scam-guard check domain {hostile} → overall=high (evidence display only)"),
        );
    }

    let socket = socket_path();
    let mut wallet = match WalletClient::connect(&socket) {
        Ok(w) => w,
        Err(e) => {
            println!(
                "cannot reach wallet transport at {} — {e}\nstart it with: cargo run -p cipherlayer-cli -- connect serve --yes",
                socket.display()
            );
            std::process::exit(2);
        }
    };

    // Path 1 — capability handshake.
    let (proto, cap) = wallet.capability_query().expect("capability_query");
    check(
        "capability advertisement received and sane",
        proto == "0.4"
            && cap.chains.contains(&31337)
            && cap.methods.contains(&"rpc_query".to_string())
            && !cap.methods.contains(&"eth_exportPrivateKey".into()),
        format!(
            "protocol={proto} chains={:?} methods={:?}",
            cap.chains, cap.methods
        ),
    );

    // Path 1b — host attestation (v0.3): the broker proves it is the trusted
    // host before asserting capsule identity. Token from
    // CIPHERLAYER_CONNECT_HOST_TOKEN (or the wallet's connect-host-token file).
    match wallet.attest_from_env() {
        Ok(()) => check("host attestation accepted (v0.3)", true, "attested".into()),
        Err(e) => {
            println!(
                "cannot attest as host — {e}\nexport CIPHERLAYER_CONNECT_HOST_TOKEN to match the wallet's \
                 (printed at `connect serve` start, or in <socket-dir>/connect-host-token)"
            );
            std::process::exit(2);
        }
    }

    // Path 2 — connect (permission grant), idempotent on reconnect.
    let account = std::env::var("CIPHERLAYER_HARNESS_ACCOUNT")
        .unwrap_or_else(|_| "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".into());
    let grant = wallet
        .connect_request(
            identity.origin(),
            identity.capsule_id(),
            31337,
            "signing",
            std::slice::from_ref(&account),
            1,
        )
        .expect("connect_request");
    let grant2 = wallet
        .connect_request(
            identity.origin(),
            identity.capsule_id(),
            31337,
            "signing",
            std::slice::from_ref(&account),
            1,
        )
        .expect("reconnect");
    check(
        "connect grants scoped permission; reconnect reuses it",
        grant.access == "signing" && grant2.permission_id == grant.permission_id,
        format!(
            "permission_id={} expires_at={}",
            grant.permission_id, grant.expires_at
        ),
    );

    // Path 3 — happy path: 0-wei native transfer through the full pipeline.
    let env = build_transaction_envelope(
        &identity,
        31337,
        &json!([{ "from": account, "to": "0x00000000000000000000000000000000000000bb", "value": "0x0" }]),
    )
    .expect("envelope");
    let resp = wallet
        .transaction_request(&env)
        .expect("transaction_request");
    let confirmed = matches!(map_signing_response(&resp), DappResult::TxHash(_));
    check(
        "native transfer confirmed end-to-end (audit-referenced)",
        confirmed && resp.signing_request_id.is_some() && resp.audit_head_sequence > 0,
        format!(
            "state={} tx_hash={:?} audit_head={}",
            resp.state, resp.tx_hash, resp.audit_head_sequence
        ),
    );
    // v1.2 review_evidence (wallet ADR-014): a confirmation carries the
    // display-only pipeline evidence a shell renders — decoded intent plus
    // simulation/risk provenance. Display-only: nothing here decides.
    check(
        "confirmation carries review_evidence (decoded intent + simulation/risk)",
        resp.review_evidence.as_ref().is_some_and(|ev| {
            !ev.decoded_action.is_empty() && ev.simulation.is_some() && ev.risk_level.is_some()
        }),
        format!(
            "evidence={:?}",
            resp.review_evidence.as_ref().map(|ev| (
                &ev.decoded_action,
                &ev.simulation,
                &ev.risk_level
            ))
        ),
    );

    // Path 4 — denial path: unlimited ERC-20 approve must come back
    // POLICY_DENIED, mapped to a coarse 4001 for the page.
    let env = build_transaction_envelope(
        &identity,
        31337,
        &json!([{ "from": account, "to": "0x00000000000000000000000000000000000000cc", "data": UNLIMITED_APPROVE_TO_SELF }]),
    )
    .expect("envelope");
    let resp = wallet
        .transaction_request(&env)
        .expect("transaction_request");
    let coarse = match map_signing_response(&resp) {
        DappResult::Refused(e) => e.code == 4001 && !e.message.contains("POL-"),
        _ => false,
    };
    check(
        "unlimited approval denied; dapp sees coarse 4001, reasons stay browser-side",
        resp.state == "POLICY_DENIED"
            && resp.signing_request_id.is_none()
            && resp
                .refusal_reasons
                .iter()
                .any(|r| r.code.starts_with("POL-APR"))
            && coarse,
        format!(
            "state={} reasons={:?}",
            resp.state,
            resp.refusal_reasons
                .iter()
                .map(|r| r.code.clone())
                .collect::<Vec<_>>()
        ),
    );
    // The denial's review_evidence names the verdict and the POL reason codes
    // so the shell can display *why* — while the page above only ever saw
    // the coarse 4001.
    check(
        "denial carries review_evidence (verdict Deny + policy reasons)",
        resp.review_evidence.as_ref().is_some_and(|ev| {
            ev.policy_verdict.as_deref() == Some("Deny")
                && ev.policy_reasons.iter().any(|r| r.contains("POL-APR"))
        }),
        format!(
            "evidence={:?}",
            resp.review_evidence
                .as_ref()
                .map(|ev| (&ev.policy_verdict, &ev.policy_reasons))
        ),
    );

    // ------------------------------------------------------------------
    // v0.4 (ADR-019) extended checks: typed_data/personal_message signing
    // and the read-only rpc_query proxy, run against the same live wallet
    // and the still-active permission grant from Path 2.
    // ------------------------------------------------------------------

    // Path (a) — eth_signTypedData_v4: an unlimited EIP-2612 permit hits the
    // same default-deny policy rule an unlimited calldata approve does; the
    // page sees a coarse 4001, never the POL-APR-001 reason code.
    let permit_json = eip2612_unlimited_permit_json(&account, 31337);
    let td_env = build_typed_data_envelope(&identity, 31337, &json!([account, permit_json]))
        .expect("typed_data envelope");
    let resp = wallet
        .transaction_request(&td_env)
        .expect("transaction_request (typed_data)");
    let coarse = match map_signing_response(&resp) {
        DappResult::Refused(e) => e.code == 4001 && !e.message.contains("POL-"),
        _ => false,
    };
    check(
        "(a) eth_signTypedData_v4: unlimited EIP-2612 permit denied; page sees coarse 4001",
        resp.state == "POLICY_DENIED"
            && resp.signature_hex.is_none()
            && resp
                .refusal_reasons
                .iter()
                .any(|r| r.code.starts_with("POL-APR"))
            && coarse,
        format!(
            "state={} reasons={:?} signature_hex={:?}",
            resp.state,
            resp.refusal_reasons
                .iter()
                .map(|r| r.code.clone())
                .collect::<Vec<_>>(),
            resp.signature_hex
        ),
    );

    // Path (b) — personal_sign happy path: SIGNED is the legitimate terminal
    // state for a message signature (no tx hash, no broadcast); requires the
    // wallet to have been started with `--yes` (auto-approve USER_REVIEW —
    // harness/tests only, explicit and logged).
    let msg_hex = format!("0x{}", hex::encode(b"sign in to harness.example"));
    let pm_env = build_personal_message_envelope(&identity, 31337, &json!([msg_hex, account]))
        .expect("personal_message envelope");
    let resp = wallet
        .transaction_request(&pm_env)
        .expect("transaction_request (personal_message)");
    let signed = matches!(map_signing_response(&resp), DappResult::Signature(_));
    check(
        "(b) personal_sign happy path returns signature_hex",
        signed && resp.state == "SIGNED" && resp.signature_hex.is_some(),
        format!(
            "state={} signature_hex={:?}",
            resp.state, resp.signature_hex
        ),
    );

    // Path (c) — rpc_query: eth_blockNumber, a pure chain-state read proxied
    // to Anvil through the wallet's own RPC endpoint (never a caller-
    // supplied one). Requires Anvil running (make -C ../wallet anvil-up).
    match wallet.rpc_query(
        identity.origin(),
        identity.capsule_id(),
        31337,
        "eth_blockNumber",
        json!([]),
    ) {
        Ok(result) => check(
            "(c) rpc_query eth_blockNumber returns a result",
            result.is_string() || result.is_number(),
            format!("result={result}"),
        ),
        Err(e) => check(
            "(c) rpc_query eth_blockNumber returns a result",
            false,
            format!("rpc_query failed: {e} (is Anvil running? make -C ../wallet anvil-up)"),
        ),
    }

    // Path (d) — off-whitelist method: eth_sendRawTransaction sounds
    // rpc-adjacent but is consequence-bearing (broadcasts an already-signed
    // transaction) and is not on RPC_QUERY_WHITELIST. Refused broker-side,
    // never forwarded to the wallet at all — the same deny-by-default path
    // as Path 0's unknown method.
    let e = classify_method("eth_sendRawTransaction").unwrap_err();
    check(
        "(d) off-whitelist method (eth_sendRawTransaction) refused broker-side 4200",
        e.code == UNSUPPORTED_METHOD,
        format!("code={} message={}", e.code, e.message),
    );

    // Path 5 — revoke, then verify the wallet refuses the very next request.
    let revoked = wallet
        .permission_revoke(grant.permission_id)
        .expect("revoke");
    let env = build_transaction_envelope(
        &identity,
        31337,
        &json!([{ "from": account, "to": "0x00000000000000000000000000000000000000bb", "value": "0x0" }]),
    )
    .expect("envelope");
    let resp = wallet
        .transaction_request(&env)
        .expect("transaction_request");
    check(
        "revocation takes effect on the next request",
        revoked && resp.state == "REJECTED" && resp.signing_request_id.is_none(),
        format!("revoked={revoked} state={}", resp.state),
    );

    println!("== harness result: {pass} passed, {fail} failed ==");
    if fail > 0 {
        std::process::exit(1);
    }
}
