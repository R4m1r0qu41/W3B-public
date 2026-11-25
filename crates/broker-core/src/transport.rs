//! Protocol v0.3 transport client: JSON-lines over a same-user Unix socket.
//!
//! Peer authentication: the socket lives in a 0700 directory owned by the
//! user (OS is the first authenticator; the broker refuses world- or
//! group-accessible sockets). v0.3 adds **host attestation** — the broker
//! proves it is the trusted host by presenting the wallet's launch token
//! (`host_attest`) before it may assert any capsule identity. A peer that is
//! not the launched host cannot forge a capsule id (wallet C-1 / ADR-013).

use crate::wire::{Envelope, Permission, SigningResponse, WalletCapability};
use serde::Deserialize;
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::Path;

/// Browser-connect protocol version this crate speaks (declared on outbound
/// envelopes' `protocol_version` field). v0.4 (ADR-019): additive over v0.3's
/// host-attested transport — typed_data/personal_message signing, the
/// `rpc_query` read-only proxy.
pub const PROTOCOL_VERSION: &str = "0.4";

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("socket i/o: {0}")]
    Io(#[from] std::io::Error),
    #[error("socket {0} is group/world-accessible; refusing to connect")]
    InsecureSocket(String),
    #[error("wallet answered with protocol error {code}: {detail}")]
    Wallet { code: String, detail: String },
    #[error("unexpected wallet reply: {0}")]
    Unexpected(String),
    #[error("host attestation was rejected by the wallet (wrong or missing launch token)")]
    AttestationRejected,
    #[error("no host attestation token available: set CIPHERLAYER_CONNECT_HOST_TOKEN")]
    NoToken,
}

/// Wallet replies, mirroring the serve-side message set.
#[derive(Debug, Deserialize)]
#[serde(tag = "message", rename_all = "snake_case")]
enum Reply {
    Capability {
        protocol_version: String,
        payload: WalletCapability,
    },
    ConnectResponse {
        payload: Permission,
    },
    SigningResponse {
        // Boxed: v1.2 review_evidence makes this by far the largest variant
        // (clippy::large_enum_variant).
        payload: Box<SigningResponse>,
    },
    RevokeResponse {
        permission_id: uuid::Uuid,
        revoked: bool,
    },
    /// v0.4: `rpc_query`'s success reply — the raw JSON-RPC result, forwarded
    /// verbatim (rpc-query.v1 `$defs/response`).
    RpcQueryResponse {
        result: serde_json::Value,
    },
    AttestResponse {
        attested: bool,
    },
    Error {
        code: String,
        detail: String,
    },
}

pub struct WalletClient {
    stream: BufReader<UnixStream>,
}

impl WalletClient {
    pub fn connect(socket_path: &Path) -> Result<Self, TransportError> {
        let mode = std::fs::metadata(socket_path)?.permissions().mode();
        if mode & 0o077 != 0 {
            return Err(TransportError::InsecureSocket(
                socket_path.display().to_string(),
            ));
        }
        let stream = UnixStream::connect(socket_path)?;
        Ok(Self {
            stream: BufReader::new(stream),
        })
    }

    fn round_trip(&mut self, request: &serde_json::Value) -> Result<Reply, TransportError> {
        let mut line = serde_json::to_string(request).expect("requests serialize");
        line.push('\n');
        self.stream.get_mut().write_all(line.as_bytes())?;
        let mut reply = String::new();
        self.stream.read_line(&mut reply)?;
        match serde_json::from_str(&reply) {
            Ok(Reply::Error { code, detail }) => Err(TransportError::Wallet { code, detail }),
            Ok(r) => Ok(r),
            Err(e) => Err(TransportError::Unexpected(format!("{e}: {reply}"))),
        }
    }

    pub fn capability_query(&mut self) -> Result<(String, WalletCapability), TransportError> {
        match self.round_trip(&json!({"message": "capability_query"}))? {
            Reply::Capability {
                protocol_version,
                payload,
            } => Ok((protocol_version, payload)),
            other => Err(TransportError::Unexpected(format!("{other:?}"))),
        }
    }

    /// Prove this connection is the trusted host by presenting the wallet's
    /// launch token (v0.3). Must precede any capsule-bearing call.
    pub fn attest(&mut self, token: &str) -> Result<(), TransportError> {
        match self.round_trip(&json!({"message": "host_attest", "token": token}))? {
            Reply::AttestResponse { attested: true } => Ok(()),
            Reply::AttestResponse { attested: false } => Err(TransportError::AttestationRejected),
            other => Err(TransportError::Unexpected(format!("{other:?}"))),
        }
    }

    /// Attest using the token from `CIPHERLAYER_CONNECT_HOST_TOKEN`, the
    /// channel by which the wallet hands the host its launch token.
    pub fn attest_from_env(&mut self) -> Result<(), TransportError> {
        let token = std::env::var("CIPHERLAYER_CONNECT_HOST_TOKEN")
            .ok()
            .filter(|t| !t.trim().is_empty())
            .ok_or(TransportError::NoToken)?;
        self.attest(&token)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn connect_request(
        &mut self,
        origin: &str,
        capsule_id: &str,
        chain_id: u64,
        access: &str,
        accounts: &[String],
        ttl_hours: i64,
    ) -> Result<Permission, TransportError> {
        let req = json!({
            "message": "connect_request",
            "origin": origin,
            "capsule_id": capsule_id,
            "chain_id": chain_id,
            "access": access,
            "accounts": accounts,
            "ttl_hours": ttl_hours,
        });
        match self.round_trip(&req)? {
            Reply::ConnectResponse { payload } => Ok(payload),
            other => Err(TransportError::Unexpected(format!("{other:?}"))),
        }
    }

    pub fn transaction_request(
        &mut self,
        envelope: &Envelope,
    ) -> Result<SigningResponse, TransportError> {
        let req = json!({
            "message": "transaction_request",
            "envelope": serde_json::to_value(envelope).expect("envelope serializes"),
        });
        match self.round_trip(&req)? {
            Reply::SigningResponse { payload } => Ok(*payload),
            other => Err(TransportError::Unexpected(format!("{other:?}"))),
        }
    }

    /// v0.4 read-only RPC proxy (`rpc_query`): forwards one whitelisted
    /// chain-state read to the wallet's own RPC endpoint for the session's
    /// granted chain. Capsule-bearing (asserts `origin`/`capsule_id`), so it
    /// requires host attestation and an active permission grant, exactly
    /// like `connect_request`/`transaction_request` — the wallet enforces
    /// both; this method just carries the request across the wire.
    pub fn rpc_query(
        &mut self,
        origin: &str,
        capsule_id: &str,
        chain_id: u64,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, TransportError> {
        let req = json!({
            "message": "rpc_query",
            "origin": origin,
            "capsule_id": capsule_id,
            "chain_id": chain_id,
            "method": method,
            "params": params,
        });
        match self.round_trip(&req)? {
            Reply::RpcQueryResponse { result } => Ok(result),
            other => Err(TransportError::Unexpected(format!("{other:?}"))),
        }
    }

    pub fn permission_revoke(&mut self, permission_id: uuid::Uuid) -> Result<bool, TransportError> {
        let req = json!({"message": "permission_revoke", "permission_id": permission_id});
        match self.round_trip(&req)? {
            Reply::RevokeResponse {
                permission_id: echoed,
                revoked,
            } => {
                if echoed != permission_id {
                    return Err(TransportError::Unexpected(format!(
                        "revoke echoed wrong id {echoed}"
                    )));
                }
                Ok(revoked)
            }
            other => Err(TransportError::Unexpected(format!("{other:?}"))),
        }
    }
}
