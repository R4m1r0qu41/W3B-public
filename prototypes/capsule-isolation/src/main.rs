//! Capsule-isolation measurement experiment (ADR-B001's second sanctioned
//! experiment; feeds engine ADR-B002).
//!
//! Question: does the wallet's capsule-keyed permission model actually
//! isolate one dapp capsule from another, as `dapp-capsules.md` and
//! `per-site-permissions.md` claim? This harness drives the real wallet over
//! the v0.2 transport with distinct capsule identities and measures the
//! boundary. It is *not* a shell — it makes no rendering claims; it produces
//! the isolation-mechanics data the engine decision needs (how much isolation
//! the wallet already enforces vs how much a webview engine must add itself).
//!
//! Run:  wallet$ make anvil-up && cargo run -p cipherlayer-cli -- connect serve --yes
//!       browser$ cargo run -p capsule-isolation

use broker_core::transport::WalletClient;
use broker_core::{build_transaction_envelope, CapsuleIdentity};
use serde_json::json;
use std::path::PathBuf;

const ACCOUNT: &str = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";

fn socket_path() -> PathBuf {
    std::env::var("CIPHERLAYER_CONNECT_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../wallet/data/browser-connect.sock"))
}

/// One capsule: an origin + a generation-stamped capsule id. "Reset capsule"
/// = new generation = new id (dapp-capsules.md), which is exactly what we
/// exercise below.
fn capsule(origin: &str, gen: u32) -> CapsuleIdentity {
    CapsuleIdentity::new(origin, &format!("{origin}#gen{gen}")).expect("static identity")
}

fn transfer(client: &mut WalletClient, id: &CapsuleIdentity) -> String {
    let env = build_transaction_envelope(
        id,
        31337,
        &json!([{ "from": ACCOUNT, "to": "0x00000000000000000000000000000000000000bb", "value": "0x0" }]),
    )
    .expect("envelope");
    match client.transaction_request(&env) {
        Ok(r) => r.state,
        Err(e) => format!("transport-error: {e}"),
    }
}

fn main() {
    let mut pass = 0u32;
    let mut fail = 0u32;
    let mut note = |name: &str, ok: bool, detail: String| {
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

    // One connection carries every capsule: capsule identity travels in each
    // envelope's `caller`, not in the socket. (The v0.2 transport serves
    // connections sequentially — itself a finding for ADR-B002: a real shell
    // needs concurrent capsules, so the transport needs multiplexing or a
    // connection-per-capsule server.)
    let socket = socket_path();
    let mut wallet = match WalletClient::connect(&socket) {
        Ok(c) => c,
        Err(e) => {
            println!(
                "cannot reach wallet transport at {} — {e}",
                socket.display()
            );
            std::process::exit(2);
        }
    };
    // v0.3: the host attests once; only then may it assert capsule identity.
    if let Err(e) = wallet.attest_from_env() {
        println!(
            "cannot attest as host — {e}\nexport CIPHERLAYER_CONNECT_HOST_TOKEN to match the wallet"
        );
        std::process::exit(2);
    }

    let cap_a = capsule("https://alpha-dapp.example", 1);
    let cap_b = capsule("https://beta-dapp.example", 1);

    // Grant signing to capsule A only.
    let grant_a = wallet
        .connect_request(
            cap_a.origin(),
            cap_a.capsule_id(),
            31337,
            "signing",
            &[ACCOUNT.into()],
            1,
        )
        .expect("A connect");
    note(
        "capsule A obtains a scoped signing grant",
        grant_a.access == "signing",
        format!(
            "A permission_id={} caller_key={}",
            grant_a.permission_id, grant_a.caller_key
        ),
    );

    // MEASUREMENT 1 — confused-deputy defense (T-19): capsule B never
    // requested a grant. Its transaction must be refused, proving permissions
    // are capsule-scoped, not ambient to the transport/process.
    let b_state = transfer(&mut wallet, &cap_b);
    note(
        "capsule B cannot ride capsule A's grant (permissions are not ambient)",
        b_state == "REJECTED",
        format!("B transfer without its own grant → {b_state}"),
    );

    // A, with its grant, succeeds — isolation is a boundary, not a wall.
    let a_state = transfer(&mut wallet, &cap_a);
    note(
        "capsule A, holding its own grant, transacts",
        a_state == "CONFIRMED",
        format!("A transfer with grant → {a_state}"),
    );

    // MEASUREMENT 3 — reset capsule orphans grants: bump A's generation. The
    // new capsule_id has no grant; its transaction is refused even though the
    // origin is identical. "Reset this dapp" = clean slate.
    let cap_a2 = capsule("https://alpha-dapp.example", 2);
    let reset_state = transfer(&mut wallet, &cap_a2);
    note(
        "reset-capsule (new generation) orphans the prior grant",
        reset_state == "REJECTED",
        format!("A gen2 (same origin, new capsule id) → {reset_state}"),
    );

    // MEASUREMENT 2 — capsule_id integrity (C-1, closed at v0.3). A hostile
    // local process opens its OWN connection and, WITHOUT the host token,
    // forges A's exact origin + capsule_id to ride A's still-active grant. At
    // v0.2 this succeeded (the finding that drove ADR-013); at v0.3 the
    // un-attested connection may not assert capsule identity at all.
    //
    // The transport serves connections sequentially, so the host connection
    // is dropped first (A's grant persists wallet-side, so the forge is still
    // meaningful) and the attacker uses a fresh, un-attested connection.
    drop(wallet);
    let mut attacker = WalletClient::connect(&socket).expect("attacker connects (same user)");
    let forged = CapsuleIdentity::new(cap_a.origin(), cap_a.capsule_id()).expect("forge");
    let forged_env = build_transaction_envelope(
        &forged,
        31337,
        &json!([{ "from": ACCOUNT, "to": "0x00000000000000000000000000000000000000bb", "value": "0x0" }]),
    )
    .expect("envelope");
    let forged_result = match attacker.transaction_request(&forged_env) {
        Ok(r) => format!("signed:{}", r.state),
        Err(e) => format!("{e}"),
    };
    note(
        "C-1 CLOSED (v0.3): an un-attested process forging A's capsule_id is refused",
        forged_result.contains("attestation_required"),
        format!("attacker (no host token) forging A's capsule_id → {forged_result}"),
    );
    drop(attacker);

    // MEASUREMENT 4 — revocation is per-capsule and immediate. Re-attest on a
    // fresh host connection (A's grant is still active), revoke, and confirm
    // A's next request is refused.
    let mut host2 = WalletClient::connect(&socket).expect("host reconnects");
    host2.attest_from_env().expect("host re-attests");
    let revoked = host2
        .permission_revoke(grant_a.permission_id)
        .expect("revoke");
    let after = transfer(&mut host2, &cap_a);
    note(
        "revoking A's grant blocks A's next request, nothing else needed",
        revoked && after == "REJECTED",
        format!("revoked={revoked}, A next transfer → {after}"),
    );

    println!("== capsule-isolation: {pass} passed, {fail} failed ==");
    println!(
        "\nISOLATION SUMMARY (for ADR-B002 / ADR-013):\n\
         · permission scoping by capsule identity: ENFORCED wallet-side (measurements 1,3,4)\n\
         · capsule_id integrity vs a non-host local process: ENFORCED at v0.3 (measurement 2)\n\
           → host attestation: only the launched host (holding the wallet's launch token)\n\
             may assert capsule identity; a peer that is not the host cannot forge one.\n\
           → honest residual: a same-user attacker who reads the token is local malware\n\
             (wallet T-14), still accepted; and per-capsule OS isolation remains the\n\
             engine's job for defence in depth.\n\
         · storage/cookie/SW isolation: NOT measurable here (webview-engine property,\n\
           see webview-harness findings) — the other half of the engine decision."
    );
    if fail > 0 {
        std::process::exit(1);
    }
}
