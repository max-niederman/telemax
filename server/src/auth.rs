use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::RwLock;

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: &'static str,
}

/// A pending pairing request from a client device.
struct PendingPair {
    /// The session token (raw) once approved, None while pending.
    token: Option<String>,
    /// When this request was created.
    created: Instant,
}

const PAIR_TTL_SECS: u64 = 300; // 5 minutes

pub struct PairingState {
    /// Active session token hashes.
    pub sessions: RwLock<HashSet<String>>,
    /// Pending pairing requests: code -> PendingPair.
    pending: RwLock<HashMap<String, PendingPair>>,
}

impl PairingState {
    pub fn new() -> Arc<Self> {
        let sessions = load_sessions();
        Arc::new(Self {
            sessions: RwLock::new(sessions),
            pending: RwLock::new(HashMap::new()),
        })
    }

    pub async fn verify_token(&self, token: &str) -> bool {
        let hashed = hash_token(token);
        self.sessions.read().await.contains(&hashed)
    }

    /// Register a pending pairing request with a client-generated code.
    /// Returns false if the code is already pending.
    pub async fn register_request(&self, code: &str) -> bool {
        let mut pending = self.pending.write().await;
        // Clean expired
        pending.retain(|_, p| p.created.elapsed().as_secs() < PAIR_TTL_SECS);

        if pending.contains_key(code) {
            return false;
        }
        pending.insert(code.to_string(), PendingPair {
            token: None,
            created: Instant::now(),
        });
        true
    }

    /// Approve a pending request (called from the local socket).
    /// Returns true if the code was found and approved.
    pub async fn approve(&self, code: &str) -> bool {
        let mut pending = self.pending.write().await;
        if let Some(req) = pending.get_mut(code) {
            if req.token.is_some() {
                return false; // already approved
            }
            let token = uuid::Uuid::new_v4().to_string();
            let hashed = hash_token(&token);
            self.sessions.write().await.insert(hashed);
            save_sessions(&self.sessions).await;
            req.token = Some(token);
            true
        } else {
            false
        }
    }

    /// Poll for the result of a pairing request.
    /// Returns Some(token) if approved, None if still pending or expired.
    pub async fn poll(&self, code: &str) -> PollResult {
        let mut pending = self.pending.write().await;
        match pending.get(code) {
            Some(req) if req.created.elapsed().as_secs() >= PAIR_TTL_SECS => {
                pending.remove(code);
                PollResult::Expired
            }
            Some(req) => match &req.token {
                Some(token) => {
                    let token = token.clone();
                    pending.remove(code);
                    PollResult::Approved(token)
                }
                None => PollResult::Pending,
            },
            None => PollResult::NotFound,
        }
    }
}

pub enum PollResult {
    Pending,
    Approved(String),
    Expired,
    NotFound,
}

/// Listen on the pairing socket. When a client writes a code, approve it.
pub fn start_code_socket(pairing: Arc<PairingState>) {
    use tokio::net::UnixListener;

    tokio::spawn(async move {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .unwrap_or_else(|_| "/tmp".into());
        let sock_path = PathBuf::from(&runtime_dir).join("telemax-code.sock");

        let _ = std::fs::remove_file(&sock_path);

        let listener = match UnixListener::bind(&sock_path) {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("Failed to bind telemax-code.sock: {e}");
                return;
            }
        };
        tracing::info!("Pairing socket: {}", sock_path.display());

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let pairing = pairing.clone();
                    tokio::spawn(async move {
                        let (reader, mut writer) = tokio::io::split(stream);
                        let mut reader = BufReader::new(reader);
                        let mut line = String::new();
                        if reader.read_line(&mut line).await.is_ok() {
                            let code = line.trim();
                            if code.is_empty() {
                                let _ = writer.write_all(b"ERR empty code\n").await;
                                return;
                            }
                            if pairing.approve(code).await {
                                let _ = writer.write_all(b"OK paired\n").await;
                                tracing::info!("Pairing approved for code {code}");
                            } else {
                                let _ = writer.write_all(b"ERR no matching request\n").await;
                            }
                        }
                    });
                }
                Err(e) => {
                    tracing::warn!("Code socket accept error: {e}");
                }
            }
        }
    });
}

fn sessions_path() -> PathBuf {
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            PathBuf::from(home).join(".config")
        });
    config_dir.join("telemax").join("sessions.json")
}

fn load_sessions() -> HashSet<String> {
    let path = sessions_path();
    match std::fs::read_to_string(&path) {
        Ok(contents) => {
            let tokens: Vec<String> = serde_json::from_str(&contents).unwrap_or_default();
            tokens.into_iter().collect()
        }
        Err(_) => HashSet::new(),
    }
}

async fn save_sessions(sessions: &RwLock<HashSet<String>>) {
    use std::os::unix::fs::OpenOptionsExt;

    let path = sessions_path();
    let sessions = sessions.read().await;
    let tokens: Vec<&String> = sessions.iter().collect();
    if let Ok(json) = serde_json::to_string_pretty(&tokens) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, json.as_bytes()));
    }
}

fn extract_token(req: &Request) -> Option<String> {
    if let Some(auth) = req.headers().get("authorization").and_then(|v| v.to_str().ok()) {
        if let Some(token) = auth.strip_prefix("Bearer ") {
            return Some(token.to_string());
        }
    }
    if let Some(cookie_header) = req.headers().get("cookie").and_then(|v| v.to_str().ok()) {
        for cookie in cookie_header.split(';') {
            let cookie = cookie.trim();
            if let Some(token) = cookie.strip_prefix("telemax_session=") {
                return Some(token.to_string());
            }
        }
    }
    None
}

pub async fn require_auth(
    State(pairing): State<Arc<PairingState>>,
    req: Request,
    next: Next,
) -> Response {
    match extract_token(&req) {
        Some(ref token) if pairing.verify_token(token).await => {
            next.run(req).await
        }
        _ => (
            StatusCode::FORBIDDEN,
            Json(ErrorBody {
                error: ErrorDetail {
                    code: "AUTH_REQUIRED",
                    message: "Pairing required",
                },
            }),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct PairRequest {
    pub code: String,
}

#[derive(Serialize)]
pub struct PairPollResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}
