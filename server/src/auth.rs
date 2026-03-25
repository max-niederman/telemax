use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: &'static str,
}

pub struct PairingState {
    pub code: RwLock<String>,
    pub sessions: RwLock<HashSet<String>>,
}

impl PairingState {
    pub fn new() -> Arc<Self> {
        let sessions = load_sessions();
        let code = generate_code();
        show_pairing_code(&code);

        Arc::new(Self {
            code: RwLock::new(code),
            sessions: RwLock::new(sessions),
        })
    }

    pub async fn verify_token(&self, token: &str) -> bool {
        self.sessions.read().await.contains(token)
    }

    pub async fn pair(&self, submitted_code: &str) -> Option<String> {
        let mut code = self.code.write().await;
        if submitted_code != *code {
            return None;
        }
        // Code matched — generate session token, rotate code
        let token = uuid::Uuid::new_v4().to_string();
        self.sessions.write().await.insert(token.clone());
        save_sessions(&self.sessions).await;
        *code = generate_code();
        show_pairing_code(&code);
        Some(token)
    }

    pub async fn refresh_code(&self) {
        let mut code = self.code.write().await;
        *code = generate_code();
        show_pairing_code(&code);
    }
}

fn generate_code() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let n: u32 = rng.gen_range(0..1_000_000);
    format!("{n:06}")
}

fn show_pairing_code(code: &str) {
    tracing::info!("Pairing code: {code}");
    let _ = std::process::Command::new("notify-send")
        .arg("Telemax Pairing Code")
        .arg(code)
        .spawn();
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
    let path = sessions_path();
    let sessions = sessions.read().await;
    let tokens: Vec<&String> = sessions.iter().collect();
    if let Ok(json) = serde_json::to_string_pretty(&tokens) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, json);
    }
}

/// Extract a session token from either the Authorization header or the telemax_session cookie.
fn extract_token(req: &Request) -> Option<String> {
    // Check Authorization: Bearer <token>
    if let Some(auth) = req.headers().get("authorization").and_then(|v| v.to_str().ok()) {
        if let Some(token) = auth.strip_prefix("Bearer ") {
            return Some(token.to_string());
        }
    }

    // Check cookie
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

use axum::extract::State;

#[derive(Deserialize)]
pub struct PairRequest {
    pub code: String,
}

#[derive(Serialize)]
pub struct PairResponse {
    pub token: String,
}
