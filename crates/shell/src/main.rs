//! CipherLayer Browser MVP shell — the Tauri host selected by ADR-B002 and
//! unlocked by ADR-B003 (see browser ADR-B004 for this shell's design).
//!
//! Process picture (all separate OS processes, per ADR-B002):
//!   shell (this binary, presentation only)
//!     └─ Unix socket, protocol v0.3, host-attested ──► wallet `connect serve`
//!            └─ signing-request IPC ──► `cipherlayer-vaultd` (keys + trusted
//!               confirmation surface — the ONLY place approve/reject happens)
//!
//! Two webviews, host-assigned labels:
//!   "main"    — shell evidence panel (`ui/`, trusted shell chrome, Zone C).
//!   "capsule" — the dapp capsule (`capsule://dapp`, hostile Zone B content)
//!               with the EIP-1193 provider injected before page scripts.
//!
//! The page↔host bridge is the `clbridge` custom scheme; authorization is
//! keyed on the host-supplied webview label (see `host::route`). No key
//! material, no policy logic, no confirmation surface exists in this process
//! (browser AGENTS #2/#6); the shell displays evidence, the wallet decides.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod host;

use host::{ShellHost, CAPSULE_LABEL};
use std::sync::Arc;
use tauri::http::{header, Method, Response, StatusCode};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

/// The fixture dapp served into the capsule (untrusted Zone B content).
const FIXTURE_DAPP: &str = include_str!("../../../fixtures/sample-dapp.html");

/// EIP-1193 provider injected before page scripts (reused from the sanctioned
/// provider-harness prototype), plus the page→host bridge.
///
/// Transport (real-dapp finding, 2026-07-17): remote https pages (OpenSea was
/// the discovery case) block `fetch()` to custom schemes via CSP/WebKit
/// mixed-content rules, so the primary channel is Tauri command IPC
/// (`provider_msg` — native postMessage under the hood, not a network fetch,
/// so page CSP cannot break it; granted to the capsule label only, see
/// `capabilities/capsule-provider.json`). The original `clbridge://` fetch
/// stays as a fallback for local capsule pages if IPC internals are absent.
const CAPSULE_INIT_JS: &str = concat!(
    include_str!("../../../prototypes/provider-harness/provider.js"),
    r#"
window.__cipherlayerBrokerPost = function (s) {
  var fail = function (msg) {
    try {
      var id = JSON.parse(s).id;
      window.__cipherlayerBrokerReply(id, null, { code: 4900, message: msg });
    } catch (e) {}
  };
  var internals = window.__TAURI_INTERNALS__;
  if (internals && typeof internals.invoke === "function") {
    internals
      .invoke("provider_msg", { payload: s })
      .then((m) => window.__cipherlayerBrokerReply(m.id, m.result ?? null, m.error ?? null))
      .catch((err) => fail("bridge error: " + err));
    return;
  }
  fetch("clbridge://localhost/provider", {
    method: "POST",
    headers: { "content-type": "text/plain" },
    body: s,
  })
    .then((r) => r.json())
    .then((m) => window.__cipherlayerBrokerReply(m.id, m.result ?? null, m.error ?? null))
    .catch((err) => fail("bridge unreachable: " + err));
};
"#
);

/// The capsule's one IPC command: forward a raw EIP-1193 provider message to
/// the broker host. Zero authority — identical entry point, authorization,
/// and coarse-error discipline as the `clbridge` Provider route; the label
/// check mirrors `host::route` (a future non-capsule webview granted IPC by
/// mistake still cannot reach the provider path).
#[tauri::command]
async fn provider_msg(
    webview: tauri::Webview,
    state: tauri::State<'_, Arc<ShellHost>>,
    payload: String,
) -> Result<serde_json::Value, String> {
    if webview.label() != CAPSULE_LABEL {
        return Err("provider bridge is capsule-only".into());
    }
    let shell = state.inner().clone();
    // Worker thread, same reason as serve_bridge: a signing round trip can
    // block on the human decision at the vault surface.
    tauri::async_runtime::spawn_blocking(move || shell.provider_call(&payload))
        .await
        .map_err(|e| format!("bridge worker failed: {e}"))
}

/// AUTODRIVE test hook (see `setup`): clicks the fixture's own buttons after
/// load, exercising injection + the in-page fetch bridge end-to-end.
const AUTODRIVE_JS: &str = r#"
window.addEventListener("load", () => {
  const click = (id) => document.getElementById(id).click();
  const log = () => document.getElementById("log").textContent;
  document.getElementById("log").textContent += "AUTODRIVE: shell test hook driving the page\n";
  click("detect");
  click("connect");
  setTimeout(() => { click("send"); }, 1500);
  setTimeout(() => { click("drain"); }, 4000);
  setTimeout(() => { console.log("AUTODRIVE RESULT\n" + log()); }, 8000);
});
"#;

fn plain(status: StatusCode, body: &str) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(body.as_bytes().to_vec())
        .expect("static response builds")
}

fn json_ok(body: String) -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(body.into_bytes())
        .expect("json response builds")
}

/// Serve one bridge request. Runs on a worker thread: a signing request
/// blocks here until the human decides on the wallet's trusted surface
/// (up to the vault dialog's own timeout), and the UI stays live.
fn serve_bridge(
    shell: &ShellHost,
    label: &str,
    method: &Method,
    path: &str,
    body: &[u8],
) -> Response<Vec<u8>> {
    if method == Method::OPTIONS {
        // CORS preflight for the cross-origin fetch (page → clbridge).
        return Response::builder()
            .status(StatusCode::NO_CONTENT)
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, POST, OPTIONS")
            .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "content-type")
            .body(Vec::new())
            .expect("preflight response builds");
    }
    match host::route(label, method.as_str(), path) {
        Ok(host::Route::Provider) => {
            let raw = String::from_utf8_lossy(body);
            json_ok(shell.provider_call(&raw).to_string())
        }
        Ok(host::Route::State) => json_ok(shell.state_json()),
        // Handled synchronously in the scheme-handler closure before this
        // function is ever called (see the comment at the call site) —
        // reachable only if that interception is ever removed.
        Ok(host::Route::Navigate) => plain(
            StatusCode::INTERNAL_SERVER_ERROR,
            "navigate should never reach serve_bridge",
        ),
        Err((code, detail)) => plain(
            StatusCode::from_u16(code).unwrap_or(StatusCode::FORBIDDEN),
            detail,
        ),
    }
}

/// Headless-ish end-to-end self test (CIPHERLAYER_SHELL_SELFTEST=1): drives
/// the same provider entry point the capsule page uses, through the real
/// shell process, against a live wallet + vaultd + Anvil. Exits the process
/// with 0/1. Used by scripts/demo-headless.sh; not part of the demo itself.
fn run_selftest(shell: Arc<ShellHost>) -> ! {
    let mut failures = 0u32;
    let mut check = |name: &str, ok: bool, detail: String| {
        println!(
            "[{}] {name}\n      {detail}",
            if ok { "PASS" } else { "FAIL" }
        );
        if !ok {
            failures += 1;
        }
    };

    let connect = shell.provider_call(r#"{"id":1,"method":"eth_requestAccounts","params":[]}"#);
    check(
        "connect grants accounts through the shell bridge",
        connect["error"].is_null() && connect["result"][0].is_string(),
        connect.to_string(),
    );
    let account = connect["result"][0]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let send = shell.provider_call(&format!(
        r#"{{"id":2,"method":"eth_sendTransaction","params":[{{"from":"{account}","to":"0x00000000000000000000000000000000000000bb","value":"0x0"}}]}}"#
    ));
    check(
        "0-wei transfer confirmed end-to-end (vault surface → sign → broadcast)",
        send["error"].is_null() && send["result"].as_str().is_some_and(|h| h.starts_with("0x")),
        send.to_string(),
    );

    let drain = shell.provider_call(&format!(
        r#"{{"id":3,"method":"eth_sendTransaction","params":[{{"from":"{account}","to":"0x00000000000000000000000000000000000000cc","data":"0x095ea7b30000000000000000000000000707070707070707070707070707070707070707ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"}}]}}"#
    ));
    check(
        "unlimited approval denied; page sees coarse 4001",
        drain["result"].is_null() && drain["error"]["code"] == 4001,
        drain.to_string(),
    );

    let state: serde_json::Value = serde_json::from_str(&shell.state_json()).expect("state parses");
    let evidence = &state["last_signing"]["evidence"];
    check(
        "evidence panel state carries review_evidence (verdict + reasons)",
        state["last_signing"]["state"] == "POLICY_DENIED"
            && evidence["policy_verdict"] == "Deny"
            && evidence["policy_reasons"]
                .as_array()
                .is_some_and(|r| !r.is_empty()),
        serde_json::to_string_pretty(&state["last_signing"]).unwrap_or_default(),
    );

    let unsupported = shell.provider_call(r#"{"id":4,"method":"eth_sign","params":[]}"#);
    check(
        "unsupported method refused broker-side (4200)",
        unsupported["error"]["code"] == 4200,
        unsupported.to_string(),
    );

    println!(
        "== shell selftest: {} ==",
        if failures == 0 { "PASS" } else { "FAIL" }
    );
    std::process::exit(if failures == 0 { 0 } else { 1 });
}

fn main() {
    let shell = Arc::new(ShellHost::new(ShellHost::socket_path_default()));
    shell.log("shell started — presentation only; approval stays on the wallet's trusted surface");

    let bridge_shell = shell.clone();
    let setup_shell = shell.clone();

    tauri::Builder::default()
        .manage(shell.clone())
        .invoke_handler(tauri::generate_handler![provider_msg])
        // The capsule's origin: one window == one capsule == one custom-
        // protocol origin (ADR-B002 measured model). Served only to the
        // host-labeled capsule webview.
        .register_uri_scheme_protocol("capsule", move |ctx, _request| {
            if ctx.webview_label() != CAPSULE_LABEL {
                return plain(StatusCode::FORBIDDEN, "capsule content is capsule-only");
            }
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/html")
                .body(FIXTURE_DAPP.as_bytes().to_vec())
                .expect("fixture response builds")
        })
        .register_asynchronous_uri_scheme_protocol("clbridge", move |ctx, request, responder| {
            let shell = bridge_shell.clone();
            let label = ctx.webview_label().to_string();
            let method = request.method().clone();
            let path = request.uri().path().to_string();
            let body = request.body().clone();

            // Navigate is host-chrome-only (route() refuses it for the
            // capsule label) and touches a live Webview handle, which on
            // most platforms (WKWebView included) must be driven from the
            // main thread — unlike Provider/State it is handled here,
            // synchronously with respect to routing, rather than in the
            // generic worker-thread path below.
            if method != Method::OPTIONS
                && host::route(&label, method.as_str(), &path) == Ok(host::Route::Navigate)
            {
                let raw = String::from_utf8_lossy(&body).to_string();
                match shell.validate_navigate(&raw) {
                    Ok(url) => {
                        let app = ctx.app_handle().clone();
                        let app_for_nav = app.clone();
                        let shell_for_nav = shell.clone();
                        let _ = app.run_on_main_thread(move || {
                            let Some(webview) = app_for_nav.get_webview_window(CAPSULE_LABEL)
                            else {
                                shell_for_nav.log("navigate failed: capsule webview not found");
                                return;
                            };
                            match webview.navigate(url.clone()) {
                                Ok(()) => shell_for_nav.record_navigation(&url),
                                Err(e) => shell_for_nav.log(format!("navigate failed: {e}")),
                            }
                        });
                        responder.respond(json_ok(serde_json::json!({"ok": true}).to_string()));
                    }
                    Err(msg) => responder.respond(plain(StatusCode::BAD_REQUEST, &msg)),
                }
                return;
            }

            // Worker thread: wallet round trips (including the human decision
            // on the vault surface) must never block the UI event loop.
            std::thread::spawn(move || {
                responder.respond(serve_bridge(&shell, &label, &method, &path, &body));
            });
        })
        .setup(move |app| {
            // Verification hook, mirroring the family's explicit-test-hook
            // discipline (`--yes`, CIPHERLAYER_CONFIRM_AUTO): AUTODRIVE=1
            // makes the fixture page drive itself once loaded, so the real
            // in-page path (init-script injection → provider → fetch bridge)
            // can be exercised without a human click. Loud in the page log;
            // never set for a real demo.
            let mut init_js = CAPSULE_INIT_JS.to_string();
            if std::env::var("CIPHERLAYER_SHELL_AUTODRIVE").as_deref() == Ok("1") {
                init_js.push_str(AUTODRIVE_JS);
            }
            WebviewWindowBuilder::new(
                app,
                CAPSULE_LABEL,
                WebviewUrl::External("capsule://dapp/index.html".parse()?),
            )
            .title("Dapp capsule — untrusted content (capsule://dapp), NOT a confirmation surface")
            .initialization_script(&init_js)
            .inner_size(780.0, 720.0)
            .position(640.0, 60.0)
            .build()?;

            if std::env::var("CIPHERLAYER_SHELL_SELFTEST").as_deref() == Ok("1") {
                let shell = setup_shell.clone();
                std::thread::spawn(move || run_selftest(shell));
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("cipherlayer-shell failed to run");
}
