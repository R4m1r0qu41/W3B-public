//! Provider-injection experiment (ADR-B001, gate "narrow compatibility
//! experiment"): one system-webview window (WKWebView via wry on macOS)
//! loading the fixture dapp over a custom protocol, with the EIP-1193
//! provider injected before page scripts run and every request bridged
//! through broker-core to the live wallet socket.
//!
//! Trust posture: the page is Zone B (hostile by assumption). The bridge is
//! Zone C plumbing — it classifies, scopes, attaches capsule identity, and
//! forwards; every decision stays wallet-side, and the page only ever sees
//! coarse EIP-1193 errors. This window renders **no confirmation surface**.
//!
//! Run:  wallet$ make anvil-up && cargo run -p cipherlayer-cli -- connect serve --yes
//!       browser$ cargo run -p webview-harness
//!
//! NOT a product: one window, one capsule, no tabs, no navigation UI, no
//! engine decision implied. It exists to measure injection + bridge
//! viability for the engine ADR.

use broker_core::eip1193::{map_signing_response, DappResult, ProviderError};
use broker_core::transport::WalletClient;
use broker_core::{build_transaction_envelope, classify_method, CapsuleIdentity, MethodClass};
use std::path::PathBuf;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::http::{header::CONTENT_TYPE, Response};
use wry::WebViewBuilder;

const CHAIN_ID: u64 = 31337;
/// The capsule origin: the custom-protocol origin of the fixture page. One
/// window = one capsule in this experiment.
const ORIGIN: &str = "harness://dapp";
const CAPSULE_ID: &str = "capsule-webview-1";

fn socket_path() -> PathBuf {
    std::env::var("CIPHERLAYER_CONNECT_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../wallet/data/browser-connect.sock"))
}

#[derive(Debug)]
enum UserEvent {
    /// Raw JSON from the injected provider: {id, method, params}.
    Ipc(String),
}

/// Broker session state: the cached grant metadata the protocol doc allows
/// the browser to hold (accounts + chain for eth_accounts/eth_chainId).
struct Session {
    identity: CapsuleIdentity,
    accounts: Vec<String>,
}

fn handle_request(
    session: &mut Session,
    wallet: &mut Option<WalletClient>,
    method: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, ProviderError> {
    let class = classify_method(method)?;
    let wallet = wallet
        .as_mut()
        .ok_or_else(|| ProviderError::disconnected("wallet transport unavailable"))?;
    match class {
        MethodClass::Connect => {
            let account = std::env::var("CIPHERLAYER_HARNESS_ACCOUNT")
                .unwrap_or_else(|_| "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".into());
            let grant = wallet
                .connect_request(
                    session.identity.origin(),
                    session.identity.capsule_id(),
                    CHAIN_ID,
                    "signing",
                    std::slice::from_ref(&account),
                    12,
                )
                .map_err(|e| ProviderError::disconnected(&e.to_string()))?;
            session.accounts = grant.accounts.clone();
            Ok(serde_json::json!(session.accounts))
        }
        MethodClass::Transaction => {
            let envelope = build_transaction_envelope(&session.identity, CHAIN_ID, params)?;
            let resp = wallet
                .transaction_request(&envelope)
                .map_err(|e| ProviderError::disconnected(&e.to_string()))?;
            match map_signing_response(&resp) {
                DappResult::TxHash(h) => Ok(serde_json::json!(h)),
                // Not reachable via MethodClass::Transaction (evm_transaction
                // requests never terminate SIGNED) — exhaustive match only.
                DappResult::Signature(s) => Ok(serde_json::json!(s)),
                DappResult::Refused(e) => Err(e),
            }
        }
        MethodClass::ReadCached => match method {
            "eth_accounts" => Ok(serde_json::json!(session.accounts)),
            "eth_chainId" => Ok(serde_json::json!(format!("0x{CHAIN_ID:x}"))),
            _ => Err(ProviderError::unsupported_method(method)),
        },
        // This pre-MVP experiment (ADR-B001) predates protocol v0.4 and was
        // never extended for it — superseded by the real shell
        // (crates/shell), which does implement typed_data/personal_message/
        // rpc_query. Deny-by-default rather than silently mishandle.
        MethodClass::TypedData | MethodClass::PersonalMessage | MethodClass::RpcQuery => {
            Err(ProviderError::unsupported_method(method))
        }
    }
}

fn main() -> wry::Result<()> {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title("CipherLayer webview harness — NOT a product, NOT a confirmation surface")
        .build(&event_loop)
        .expect("window");

    let fixture = include_str!("../../../fixtures/sample-dapp.html").to_string();
    let provider_js = concat!(
        include_str!("../../provider-harness/provider.js"),
        "\nwindow.__cipherlayerBrokerPost = (s) => window.ipc.postMessage(s);\n"
    );

    let webview = WebViewBuilder::new()
        .with_custom_protocol("harness".into(), move |_id, _request| {
            Response::builder()
                .header(CONTENT_TYPE, "text/html")
                .body(std::borrow::Cow::from(fixture.clone().into_bytes()))
                .expect("fixture response")
        })
        .with_initialization_script(provider_js)
        .with_ipc_handler(move |request| {
            let _ = proxy.send_event(UserEvent::Ipc(request.body().to_string()));
        })
        .with_url("harness://dapp/sample-dapp.html")
        .build(&window)?;

    let mut session = Session {
        identity: CapsuleIdentity::new(ORIGIN, CAPSULE_ID).expect("static identity"),
        accounts: Vec::new(),
    };
    let mut wallet = match WalletClient::connect(&socket_path()) {
        Ok(w) => Some(w),
        Err(e) => {
            eprintln!(
                "wallet transport unreachable ({e}); page requests will get 4900.\nstart it with: cargo run -p cipherlayer-cli -- connect serve --yes"
            );
            None
        }
    };

    event_loop.run(move |event, _target, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::UserEvent(UserEvent::Ipc(raw)) => {
                let parsed: serde_json::Value = match serde_json::from_str(&raw) {
                    Ok(v) => v,
                    Err(_) => return, // hostile/garbled page message: drop
                };
                let (Some(id), Some(method)) = (parsed["id"].as_i64(), parsed["method"].as_str())
                else {
                    return;
                };
                let reply =
                    match handle_request(&mut session, &mut wallet, method, &parsed["params"]) {
                        Ok(result) => {
                            format!("window.__cipherlayerBrokerReply({id}, {result}, null)")
                        }
                        Err(e) => format!(
                            "window.__cipherlayerBrokerReply({id}, null, {})",
                            serde_json::to_string(&e).expect("errors serialize")
                        ),
                    };
                let _ = webview.evaluate_script(&reply);
            }
            _ => {}
        }
    });
}
