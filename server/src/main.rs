mod audio;
mod auth;
mod error;
mod input;
mod media;
mod niri;
mod settings;
mod state;
mod ws;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, broadcast};
use tower_http::services::ServeDir;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

async fn get_settings(State(state): State<AppState>) -> Json<settings::Settings> {
    let s = state.settings.read().await;
    Json(s.clone())
}

async fn put_settings(
    State(state): State<AppState>,
    Json(new_settings): Json<settings::Settings>,
) -> ApiResult<Json<settings::Settings>> {
    if let Err(e) = settings::save(&new_settings) {
        return Err(ApiError::unavailable("SETTINGS_SAVE_FAILED", format!("Failed to save settings: {e}")));
    }
    let mut s = state.settings.write().await;
    *s = new_settings.clone();
    Ok(Json(new_settings))
}

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct KeyInput {
    key: String,
    #[serde(default)]
    modifiers: Vec<String>,
}

async fn post_input_key(
    State(state): State<AppState>,
    Json(body): Json<KeyInput>,
) -> ApiResult<StatusCode> {
    let input = state.input.as_ref()
        .ok_or_else(|| ApiError::unavailable("INPUT_UNAVAILABLE", "Virtual input not available (uinput inaccessible)"))?;

    let linux_key = input::keymap::js_key_to_linux(&body.key)
        .ok_or_else(|| ApiError::bad_request(format!("Unknown key: {}", body.key)))?;

    // Press modifiers
    for m in &body.modifiers {
        if let Some(mod_key) = input::keymap::js_key_to_linux(m) {
            input.key_press(mod_key).await;
        }
    }

    // Press and release key
    input.key_press(linux_key).await;
    input.key_release(linux_key).await;

    // Release modifiers in reverse
    for m in body.modifiers.iter().rev() {
        if let Some(mod_key) = input::keymap::js_key_to_linux(m) {
            input.key_release(mod_key).await;
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct TypeInput {
    text: String,
}

async fn post_input_type(
    State(state): State<AppState>,
    Json(body): Json<TypeInput>,
) -> ApiResult<StatusCode> {
    let input = state.input.as_ref()
        .ok_or_else(|| ApiError::unavailable("INPUT_UNAVAILABLE", "Virtual input not available"))?;
    input.type_text(&body.text).await;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Media
// ---------------------------------------------------------------------------

async fn get_media(State(state): State<AppState>) -> ApiResult<Json<media::MediaState>> {
    let media_state = state
        .media
        .get_state()
        .await
        .map_err(|e| ApiError::unavailable("MEDIA_UNAVAILABLE", e))?;
    Ok(Json(media_state))
}

async fn get_media_art(State(state): State<AppState>) -> ApiResult<impl IntoResponse> {
    let bytes = state
        .media
        .get_art_bytes()
        .await
        .map_err(|e| ApiError::unavailable("MEDIA_UNAVAILABLE", e))?;

    match bytes {
        Some(data) => Ok((
            StatusCode::OK,
            [("content-type", "image/png")],
            data,
        )
            .into_response()),
        None => Err(ApiError::not_found("No album art available")),
    }
}

async fn get_media_players(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<media::PlayerInfo>>> {
    let players = state
        .media
        .list_players()
        .await
        .map_err(|e| ApiError::unavailable("MEDIA_UNAVAILABLE", e.to_string()))?;
    Ok(Json(players))
}

#[derive(Deserialize)]
struct SelectPlayer {
    id: String,
}

async fn post_media_player(
    State(state): State<AppState>,
    Json(body): Json<SelectPlayer>,
) -> StatusCode {
    state.media.select_player(&body.id).await;
    StatusCode::NO_CONTENT
}

async fn post_media_play(State(state): State<AppState>) -> ApiResult<StatusCode> {
    state
        .media
        .play()
        .await
        .map_err(|e| ApiError::unavailable("MEDIA_UNAVAILABLE", e))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn post_media_pause(State(state): State<AppState>) -> ApiResult<StatusCode> {
    state
        .media
        .pause()
        .await
        .map_err(|e| ApiError::unavailable("MEDIA_UNAVAILABLE", e))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn post_media_next(State(state): State<AppState>) -> ApiResult<StatusCode> {
    state
        .media
        .next()
        .await
        .map_err(|e| ApiError::unavailable("MEDIA_UNAVAILABLE", e))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn post_media_prev(State(state): State<AppState>) -> ApiResult<StatusCode> {
    state
        .media
        .previous()
        .await
        .map_err(|e| ApiError::unavailable("MEDIA_UNAVAILABLE", e))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn post_media_stop(State(state): State<AppState>) -> ApiResult<StatusCode> {
    state
        .media
        .stop()
        .await
        .map_err(|e| ApiError::unavailable("MEDIA_UNAVAILABLE", e))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SeekInput {
    position_ms: i64,
}

async fn post_media_seek(
    State(state): State<AppState>,
    Json(body): Json<SeekInput>,
) -> ApiResult<StatusCode> {
    state
        .media
        .seek(body.position_ms)
        .await
        .map_err(|e| ApiError::unavailable("MEDIA_UNAVAILABLE", e))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct VolumeInput {
    volume: f64,
}

async fn post_media_volume(
    State(state): State<AppState>,
    Json(body): Json<VolumeInput>,
) -> ApiResult<StatusCode> {
    state
        .media
        .set_volume(body.volume)
        .await
        .map_err(|e| ApiError::unavailable("MEDIA_UNAVAILABLE", e))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn post_media_shuffle(State(state): State<AppState>) -> ApiResult<StatusCode> {
    state
        .media
        .toggle_shuffle()
        .await
        .map_err(|e| ApiError::unavailable("MEDIA_UNAVAILABLE", e))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn post_media_repeat(State(state): State<AppState>) -> ApiResult<StatusCode> {
    state
        .media
        .cycle_repeat()
        .await
        .map_err(|e| ApiError::unavailable("MEDIA_UNAVAILABLE", e))?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Niri
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct NiriActionInput {
    action: String,
    #[serde(default)]
    args: serde_json::Value,
}

/// Map a kebab-case action name + optional args to a `niri_ipc::Action`.
fn parse_niri_action(name: &str, args: &serde_json::Value) -> Result<niri_ipc::Action, String> {
    match name {
        "focus-window" => {
            let id = args.get("id").and_then(|v| v.as_u64())
                .ok_or("focus-window requires args.id (u64)")?;
            Ok(niri_ipc::Action::FocusWindow { id })
        }
        "close-window" => {
            let id = args.get("id").and_then(|v| v.as_u64());
            Ok(niri_ipc::Action::CloseWindow { id })
        }
        "fullscreen-window" => {
            let id = args.get("id").and_then(|v| v.as_u64());
            Ok(niri_ipc::Action::FullscreenWindow { id })
        }
        "focus-column-left" => Ok(niri_ipc::Action::FocusColumnLeft {}),
        "focus-column-right" => Ok(niri_ipc::Action::FocusColumnRight {}),
        "focus-window-up" => Ok(niri_ipc::Action::FocusWindowUp {}),
        "focus-window-down" => Ok(niri_ipc::Action::FocusWindowDown {}),
        "move-column-left" => Ok(niri_ipc::Action::MoveColumnLeft {}),
        "move-column-right" => Ok(niri_ipc::Action::MoveColumnRight {}),
        "move-window-up" => Ok(niri_ipc::Action::MoveWindowUp {}),
        "move-window-down" => Ok(niri_ipc::Action::MoveWindowDown {}),
        "focus-workspace-up" => Ok(niri_ipc::Action::FocusWorkspaceUp {}),
        "focus-workspace-down" => Ok(niri_ipc::Action::FocusWorkspaceDown {}),
        "focus-workspace" => {
            let index = args.get("index").and_then(|v| v.as_u64())
                .ok_or("focus-workspace requires args.index (u64)")?;
            Ok(niri_ipc::Action::FocusWorkspace {
                reference: niri_ipc::WorkspaceReferenceArg::Index(index as u8),
            })
        }
        "move-window-to-workspace-up" => Ok(niri_ipc::Action::MoveWindowToWorkspaceUp { focus: true }),
        "move-window-to-workspace-down" => Ok(niri_ipc::Action::MoveWindowToWorkspaceDown { focus: true }),
        "focus-monitor-left" => Ok(niri_ipc::Action::FocusMonitorLeft {}),
        "focus-monitor-right" => Ok(niri_ipc::Action::FocusMonitorRight {}),
        "focus-monitor-up" => Ok(niri_ipc::Action::FocusMonitorUp {}),
        "focus-monitor-down" => Ok(niri_ipc::Action::FocusMonitorDown {}),
        "move-window-to-monitor-left" => Ok(niri_ipc::Action::MoveWindowToMonitorLeft {}),
        "move-window-to-monitor-right" => Ok(niri_ipc::Action::MoveWindowToMonitorRight {}),
        "power-off-monitors" => Ok(niri_ipc::Action::PowerOffMonitors {}),
        "power-on-monitors" => Ok(niri_ipc::Action::PowerOnMonitors {}),
        "screenshot" => Ok(niri_ipc::Action::Screenshot {
            show_pointer: true,
            path: None,
        }),
        "screenshot-screen" => Ok(niri_ipc::Action::ScreenshotScreen {
            write_to_disk: true,
            show_pointer: true,
            path: None,
        }),
        "screenshot-window" => Ok(niri_ipc::Action::ScreenshotWindow {
            id: None,
            write_to_disk: true,
            path: None,
        }),
        "maximize-column" => Ok(niri_ipc::Action::MaximizeColumn {}),
        "toggle-window-floating" => {
            let id = args.get("id").and_then(|v| v.as_u64());
            Ok(niri_ipc::Action::ToggleWindowFloating { id })
        }
        "switch-preset-column-width" => Ok(niri_ipc::Action::SwitchPresetColumnWidth {}),
        "spawn" => {
            let command = args
                .get("command")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect::<Vec<_>>()
                })
                .ok_or("spawn requires args.command (array of strings)")?;
            Ok(niri_ipc::Action::Spawn { command })
        }
        other => Err(format!("Unknown action: {other}")),
    }
}

async fn post_niri_action(
    Json(body): Json<NiriActionInput>,
) -> ApiResult<Json<serde_json::Value>> {
    if !niri::is_action_allowed(&body.action) {
        return Err(ApiError::bad_request(format!(
            "Action '{}' is not allowed",
            body.action
        )));
    }

    let action = parse_niri_action(&body.action, &body.args)
        .map_err(|e| ApiError::bad_request(e))?;

    let response = tokio::task::spawn_blocking(move || niri::perform_action(action))
        .await
        .map_err(|e| ApiError::unavailable("NIRI_UNAVAILABLE", format!("Task join error: {e}")))?
        .map_err(|e| ApiError::unavailable("NIRI_UNAVAILABLE", e))?;

    Ok(Json(serde_json::to_value(response).unwrap_or_default()))
}

async fn get_niri_windows() -> ApiResult<Json<serde_json::Value>> {
    let response = tokio::task::spawn_blocking(niri::get_windows)
        .await
        .map_err(|e| ApiError::unavailable("NIRI_UNAVAILABLE", format!("Task join error: {e}")))?
        .map_err(|e| ApiError::unavailable("NIRI_UNAVAILABLE", e))?;

    Ok(Json(serde_json::to_value(response).unwrap_or_default()))
}

async fn get_niri_workspaces() -> ApiResult<Json<serde_json::Value>> {
    let response = tokio::task::spawn_blocking(niri::get_workspaces)
        .await
        .map_err(|e| ApiError::unavailable("NIRI_UNAVAILABLE", format!("Task join error: {e}")))?
        .map_err(|e| ApiError::unavailable("NIRI_UNAVAILABLE", e))?;

    Ok(Json(serde_json::to_value(response).unwrap_or_default()))
}

async fn get_niri_outputs() -> ApiResult<Json<serde_json::Value>> {
    let response = tokio::task::spawn_blocking(niri::get_outputs)
        .await
        .map_err(|e| ApiError::unavailable("NIRI_UNAVAILABLE", format!("Task join error: {e}")))?
        .map_err(|e| ApiError::unavailable("NIRI_UNAVAILABLE", e))?;

    Ok(Json(serde_json::to_value(response).unwrap_or_default()))
}

// ---------------------------------------------------------------------------
// Apps
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct AppEntry {
    id: String,
    name: String,
    icon: Option<String>,
}

async fn get_apps(State(state): State<AppState>) -> Json<Vec<AppEntry>> {
    let s = state.settings.read().await;
    let apps: Vec<AppEntry> = s
        .app_shortcuts
        .iter()
        .map(|shortcut| AppEntry {
            id: shortcut.id.clone(),
            name: shortcut.name.clone(),
            icon: shortcut.icon.clone(),
        })
        .collect();
    Json(apps)
}

#[derive(Deserialize)]
struct LaunchApp {
    id: String,
}

async fn post_apps_launch(
    State(state): State<AppState>,
    Json(body): Json<LaunchApp>,
) -> ApiResult<StatusCode> {
    let s = state.settings.read().await;
    let shortcut = s
        .app_shortcuts
        .iter()
        .find(|sc| sc.id == body.id)
        .ok_or_else(|| ApiError::not_found(format!("Unknown app shortcut: {}", body.id)))?;

    let command = shortcut.command.clone();
    drop(s);

    let action = niri_ipc::Action::Spawn { command };
    tokio::task::spawn_blocking(move || niri::perform_action(action))
        .await
        .map_err(|e| ApiError::unavailable("NIRI_UNAVAILABLE", format!("Task join error: {e}")))?
        .map_err(|e| ApiError::unavailable("NIRI_UNAVAILABLE", e))?;

    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let port: u16 = std::env::var("TELEMAX_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(9876);

    let web_dir =
        std::env::var("TELEMAX_WEB_DIR").unwrap_or_else(|_| "./web/build".into());

    // Load settings
    let settings = settings::load();
    let shared_settings = Arc::new(RwLock::new(settings));

    // Create VirtualInput (log error and continue if /dev/uinput unavailable)
    let virtual_input = match input::VirtualInput::new() {
        Ok(vi) => Some(vi),
        Err(e) => {
            tracing::warn!(
                "Failed to create virtual input (is /dev/uinput accessible?): {e}"
            );
            tracing::warn!("Continuing without virtual input — input endpoints will return 503");
            None
        }
    };

    // Create MediaController
    let media = match media::MediaController::new().await {
        Ok(mc) => Arc::new(mc),
        Err(e) => {
            tracing::error!("Failed to create media controller: {e}");
            tracing::error!("Cannot start without D-Bus session access");
            std::process::exit(1);
        }
    };

    // Create audio broadcast channel and start monitoring
    let (audio_tx, _audio_rx) = broadcast::channel::<audio::AudioLevel>(64);
    audio::start_monitoring(audio_tx.clone());

    // Build AppState
    let app_state = AppState {
        settings: shared_settings,
        input: virtual_input,
        media,
        audio_tx,
    };

    let base_path =
        std::env::var("TELEMAX_BASE_PATH").unwrap_or_default();

    // Build inner router with API routes + static file fallback
    let inner = Router::new()
        // Status
        .route("/api/status", get(status))
        // Settings
        .route("/api/settings", get(get_settings).put(put_settings))
        // Input
        .route("/api/input/key", post(post_input_key))
        .route("/api/input/type", post(post_input_type))
        // Media
        .route("/api/media", get(get_media))
        .route("/api/media/art", get(get_media_art))
        .route("/api/media/players", get(get_media_players))
        .route("/api/media/player", post(post_media_player))
        .route("/api/media/play", post(post_media_play))
        .route("/api/media/pause", post(post_media_pause))
        .route("/api/media/next", post(post_media_next))
        .route("/api/media/prev", post(post_media_prev))
        .route("/api/media/stop", post(post_media_stop))
        .route("/api/media/seek", post(post_media_seek))
        .route("/api/media/volume", post(post_media_volume))
        .route("/api/media/shuffle", post(post_media_shuffle))
        .route("/api/media/repeat", post(post_media_repeat))
        // Niri
        .route("/api/niri/action", post(post_niri_action))
        .route("/api/niri/windows", get(get_niri_windows))
        .route("/api/niri/workspaces", get(get_niri_workspaces))
        .route("/api/niri/outputs", get(get_niri_outputs))
        // Apps
        .route("/api/apps", get(get_apps))
        .route("/api/apps/launch", post(post_apps_launch))
        // WebSocket
        .route("/api/ws", get(ws::ws_handler))
        // Auth middleware on all API routes
        .layer(middleware::from_fn(auth::require_tailscale_auth))
        // Shared state
        .with_state(app_state)
        // Static files fallback
        .fallback_service(ServeDir::new(&web_dir));

    // Nest under base path if configured (e.g., /telemax for Tailscale Serve)
    let app = if base_path.is_empty() {
        inner
    } else {
        tracing::info!("serving under base path: {base_path}");
        Router::new().nest(&base_path, inner)
    };

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
