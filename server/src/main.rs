mod audio;
mod auth;
mod error;
mod input;
mod media;
mod niri;
mod settings;

use axum::{Json, Router, middleware, routing::get};
use serde::Serialize;
use std::net::SocketAddr;
use tower_http::services::ServeDir;

#[derive(Serialize)]
struct Status {
    version: &'static str,
    ok: bool,
}

async fn status() -> Json<Status> {
    Json(Status {
        version: "0.1.0",
        ok: true,
    })
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let port: u16 = std::env::var("NIRI_REMOTE_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(9876);

    let web_dir = std::env::var("NIRI_REMOTE_WEB_DIR").unwrap_or_else(|_| "./web/build".into());

    let api = Router::new()
        .route("/api/status", get(status))
        .layer(middleware::from_fn(auth::require_tailscale_auth));

    let app = api.fallback_service(ServeDir::new(&web_dir));

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
