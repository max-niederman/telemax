# niri-remote Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a remote control system for a Niri Linux desktop, with a Rust server daemon and Svelte web UI, accessible over Tailscale.

**Architecture:** Rust axum server handles REST + WebSocket APIs, integrating with uinput (mouse/keyboard), D-Bus MPRIS (media), PulseAudio (audio levels), and niri IPC (window management). Svelte SPA frontend served as static files. Exposed via Tailscale Serve for auth.

**Tech Stack:** Rust (axum, tokio, zbus, niri-ipc, input-linux, libpulse-binding), Svelte 5 / SvelteKit 2, Nix flake packaging.

---

## Chunk 1: Project Scaffolding & Core Server

### Task 1: Initialize project structure and Nix flake

**Files:**
- Create: `flake.nix`
- Create: `server/Cargo.toml`
- Create: `server/src/main.rs`
- Create: `web/package.json`
- Create: `.gitignore`

- [ ] **Step 1: Initialize git repo**

```bash
cd /home/max/Projects/tool/niri-remote
git init
```

- [ ] **Step 2: Create .gitignore**

```gitignore
/target
/result
/server/target
/web/node_modules
/web/.svelte-kit
/web/build
.direnv
.superpowers
```

- [ ] **Step 3: Create server/Cargo.toml**

```toml
[package]
name = "niri-remote-server"
version = "0.1.0"
edition = "2024"

[dependencies]
axum = { version = "0.8", features = ["ws"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tower-http = { version = "0.6", features = ["fs"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

- [ ] **Step 4: Create server/src/main.rs — minimal axum server**

```rust
use std::net::SocketAddr;

use axum::{Json, Router, routing::get};
use serde::Serialize;
use tower_http::services::ServeDir;
use tracing_subscriber::EnvFilter;

#[derive(Serialize)]
struct Status {
    version: &'static str,
    ok: bool,
}

async fn status() -> Json<Status> {
    Json(Status {
        version: env!("CARGO_PKG_VERSION"),
        ok: true,
    })
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let web_dir =
        std::env::var("NIRI_REMOTE_WEB_DIR").unwrap_or_else(|_| "./web/build".to_string());

    let app = Router::new()
        .route("/api/status", get(status))
        .fallback_service(ServeDir::new(&web_dir));

    let port: u16 = std::env::var("NIRI_REMOTE_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(9876);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

- [ ] **Step 5: Create flake.nix with Rust + Svelte builds**

```nix
{
  description = "niri-remote — remote control for Niri desktop over Tailscale";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        web = pkgs.buildNpmPackage {
          pname = "niri-remote-web";
          version = "0.1.0";
          src = ./web;
          npmDepsHash = ""; # fill after first build
          installPhase = ''
            runHook preInstall
            cp -r build $out
            runHook postInstall
          '';
        };

        server = pkgs.rustPlatform.buildRustPackage {
          pname = "niri-remote-server";
          version = "0.1.0";
          src = ./server;
          cargoHash = ""; # fill after first build
          nativeBuildInputs = with pkgs; [ pkg-config ];
          buildInputs = with pkgs; [ libpulseaudio ];
        };
      in {
        packages = {
          default = server;
          inherit server web;
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            cargo
            rustc
            rust-analyzer
            clippy
            pkg-config
            nodejs
            npm-check-updates
          ];
          buildInputs = with pkgs; [
            libpulseaudio
          ];
        };
      }
    ) // {
      nixosModules.default = import ./module.nix self;
    };
}
```

- [ ] **Step 6: Create minimal web/package.json placeholder**

```json
{
  "name": "niri-remote-web",
  "version": "0.1.0",
  "private": true,
  "scripts": {
    "dev": "vite dev",
    "build": "vite build",
    "preview": "vite preview"
  }
}
```

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: initial project scaffolding with flake, axum server, web placeholder"
```

---

### Task 2: Auth middleware

**Files:**
- Create: `server/src/auth.rs`
- Modify: `server/src/main.rs`

- [ ] **Step 1: Create server/src/auth.rs**

Extract `Tailscale-User-Login` header. Reject requests without it (403).

```rust
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: &'static str,
}

#[derive(Clone, Debug)]
pub struct TailscaleIdentity {
    pub login: String,
    pub name: Option<String>,
}

pub async fn require_tailscale_auth(mut req: Request, next: Next) -> Response {
    let login = req
        .headers()
        .get("Tailscale-User-Login")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let name = req
        .headers()
        .get("Tailscale-User-Name")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    match login {
        Some(login) => {
            let identity = TailscaleIdentity { login, name };
            req.extensions_mut().insert(identity);
            next.run(req).await
        }
        None => (
            StatusCode::FORBIDDEN,
            Json(ErrorBody {
                error: ErrorDetail {
                    code: "AUTH_REQUIRED",
                    message: "Tailscale-User-Login header missing. Access via Tailscale Serve.",
                },
            }),
        )
            .into_response(),
    }
}
```

- [ ] **Step 2: Wire auth into main.rs**

Add `mod auth;` and wrap API routes with the middleware layer:

```rust
use axum::middleware;

// In main():
let api = Router::new()
    .route("/api/status", get(status))
    .layer(middleware::from_fn(auth::require_tailscale_auth));

let app = Router::new()
    .merge(api)
    .fallback_service(ServeDir::new(&web_dir));
```

- [ ] **Step 3: Commit**

```bash
git add server/src/auth.rs server/src/main.rs
git commit -m "feat: add Tailscale auth middleware"
```

---

### Task 3: Settings persistence

**Files:**
- Create: `server/src/settings.rs`
- Modify: `server/src/main.rs`

- [ ] **Step 1: Create server/src/settings.rs**

```rust
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_sensitivity")]
    pub trackpad_sensitivity: f64,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub app_shortcuts: Vec<AppShortcut>,
    #[serde(default)]
    pub visible_actions: Vec<String>,
    #[serde(default)]
    pub audio_device: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppShortcut {
    pub id: String,
    pub name: String,
    pub command: Vec<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

fn default_sensitivity() -> f64 {
    1.0
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            trackpad_sensitivity: default_sensitivity(),
            theme: Theme::default(),
            app_shortcuts: vec![],
            visible_actions: vec![
                "close-window".into(),
                "fullscreen-window".into(),
                "maximize-column".into(),
                "screenshot".into(),
                "power-off-monitors".into(),
                "power-on-monitors".into(),
            ],
            audio_device: None,
        }
    }
}

pub type SharedSettings = Arc<RwLock<Settings>>;

fn settings_path() -> PathBuf {
    let config = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("HOME not set");
            PathBuf::from(home).join(".config")
        });
    config.join("niri-remote").join("settings.json")
}

pub fn load() -> Settings {
    let path = settings_path();
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

pub fn save(settings: &Settings) -> Result<(), std::io::Error> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(settings)?;
    std::fs::write(&path, data)
}
```

- [ ] **Step 2: Add settings API routes to main.rs**

```rust
mod settings;

use axum::{extract::State, routing::{get, put}};

type AppState = Arc<settings::SharedSettings>;

async fn get_settings(State(settings): State<SharedSettings>) -> Json<settings::Settings> {
    Json(settings.read().await.clone())
}

async fn put_settings(
    State(settings): State<SharedSettings>,
    Json(new): Json<settings::Settings>,
) -> StatusCode {
    *settings.write().await = new.clone();
    if let Err(e) = settings::save(&new) {
        tracing::error!("failed to save settings: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    StatusCode::OK
}
```

Add routes: `.route("/api/settings", get(get_settings).put(put_settings))`

Add shared state: `let shared_settings = Arc::new(RwLock::new(settings::load()));` and `.with_state(shared_settings)`.

- [ ] **Step 3: Commit**

```bash
git add server/src/settings.rs server/src/main.rs
git commit -m "feat: add settings persistence and API endpoints"
```

---

### Task 4: Error response types

**Files:**
- Create: `server/src/error.rs`

- [ ] **Step 1: Create server/src/error.rs**

Standardized error responses matching the spec.

```rust
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: ApiErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct ApiErrorDetail {
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: ApiErrorDetail {
                code: code.into(),
                message: message.into(),
            },
        }
    }

    pub fn bad_request(message: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::BAD_REQUEST,
            Json(Self::new("BAD_REQUEST", message)),
        )
    }

    pub fn unavailable(code: impl Into<String>, message: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(Self::new(code, message)),
        )
    }

    pub fn not_found(message: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::NOT_FOUND,
            Json(Self::new("NOT_FOUND", message)),
        )
    }
}

pub type ApiResult<T> = Result<T, (StatusCode, Json<ApiError>)>;
```

- [ ] **Step 2: Add `mod error;` to main.rs**

- [ ] **Step 3: Commit**

```bash
git add server/src/error.rs server/src/main.rs
git commit -m "feat: add standardized API error types"
```

---

## Chunk 2: Input Subsystem (uinput)

### Task 5: Virtual mouse and keyboard via uinput

**Files:**
- Create: `server/src/input/mod.rs`
- Create: `server/src/input/keymap.rs`
- Modify: `server/Cargo.toml` (add `input-linux`)
- Modify: `server/src/main.rs`

- [ ] **Step 1: Add input-linux dependency**

Add to `server/Cargo.toml`:
```toml
input-linux = "0.7"
```

- [ ] **Step 2: Create server/src/input/keymap.rs**

Map JavaScript `KeyboardEvent.key` names to Linux `Key` codes.

```rust
use input_linux::Key;

pub fn js_key_to_linux(key: &str) -> Option<Key> {
    Some(match key {
        "a" | "A" => Key::A,
        "b" | "B" => Key::B,
        "c" | "C" => Key::C,
        "d" | "D" => Key::D,
        "e" | "E" => Key::E,
        "f" | "F" => Key::F,
        "g" | "G" => Key::G,
        "h" | "H" => Key::H,
        "i" | "I" => Key::I,
        "j" | "J" => Key::J,
        "k" | "K" => Key::K,
        "l" | "L" => Key::L,
        "m" | "M" => Key::M,
        "n" | "N" => Key::N,
        "o" | "O" => Key::O,
        "p" | "P" => Key::P,
        "q" | "Q" => Key::Q,
        "r" | "R" => Key::R,
        "s" | "S" => Key::S,
        "t" | "T" => Key::T,
        "u" | "U" => Key::U,
        "v" | "V" => Key::V,
        "w" | "W" => Key::W,
        "x" | "X" => Key::X,
        "y" | "Y" => Key::Y,
        "z" | "Z" => Key::Z,
        "0" => Key::Num0,
        "1" => Key::Num1,
        "2" => Key::Num2,
        "3" => Key::Num3,
        "4" => Key::Num4,
        "5" => Key::Num5,
        "6" => Key::Num6,
        "7" => Key::Num7,
        "8" => Key::Num8,
        "9" => Key::Num9,
        "Enter" => Key::Enter,
        "Escape" => Key::Esc,
        "Backspace" => Key::Backspace,
        "Tab" => Key::Tab,
        " " => Key::Space,
        "-" => Key::Minus,
        "=" => Key::Equal,
        "[" => Key::LeftBrace,
        "]" => Key::RightBrace,
        "\\" => Key::BackSlash,
        ";" => Key::Semicolon,
        "'" => Key::Apostrophe,
        "`" => Key::Grave,
        "," => Key::Comma,
        "." => Key::Dot,
        "/" => Key::Slash,
        "F1" => Key::F1,
        "F2" => Key::F2,
        "F3" => Key::F3,
        "F4" => Key::F4,
        "F5" => Key::F5,
        "F6" => Key::F6,
        "F7" => Key::F7,
        "F8" => Key::F8,
        "F9" => Key::F9,
        "F10" => Key::F10,
        "F11" => Key::F11,
        "F12" => Key::F12,
        "ArrowUp" => Key::Up,
        "ArrowDown" => Key::Down,
        "ArrowLeft" => Key::Left,
        "ArrowRight" => Key::Right,
        "Home" => Key::Home,
        "End" => Key::End,
        "PageUp" => Key::PageUp,
        "PageDown" => Key::PageDown,
        "Insert" => Key::Insert,
        "Delete" => Key::Delete,
        "Control" => Key::LeftCtrl,
        "Shift" => Key::LeftShift,
        "Alt" => Key::LeftAlt,
        "Meta" => Key::LeftMeta,
        _ => return None,
    })
}
```

- [ ] **Step 3: Create server/src/input/mod.rs**

```rust
pub mod keymap;

use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;
use std::sync::Arc;

use input_linux::{
    AbsoluteAxis, EventKind, InputId, Key, RelativeAxis, SynchronizeKind,
    UInputHandle, InputEvent,
};
use tokio::sync::Mutex;

pub struct VirtualInput {
    mouse: Mutex<UInputHandle<File>>,
    keyboard: Mutex<UInputHandle<File>>,
}

impl VirtualInput {
    pub fn new() -> Result<Arc<Self>, std::io::Error> {
        let mouse = Self::create_mouse()?;
        let keyboard = Self::create_keyboard()?;
        Ok(Arc::new(Self {
            mouse: Mutex::new(mouse),
            keyboard: Mutex::new(keyboard),
        }))
    }

    fn create_mouse() -> Result<UInputHandle<File>, std::io::Error> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/uinput")?;
        let handle = UInputHandle::new(file);

        handle.set_evbit(EventKind::Relative)?;
        handle.set_relbit(RelativeAxis::X)?;
        handle.set_relbit(RelativeAxis::Y)?;
        handle.set_relbit(RelativeAxis::Wheel)?;
        handle.set_relbit(RelativeAxis::HorizontalWheel)?;

        handle.set_evbit(EventKind::Key)?;
        handle.set_keybit(Key::ButtonLeft)?;
        handle.set_keybit(Key::ButtonRight)?;
        handle.set_keybit(Key::ButtonMiddle)?;

        let id = InputId {
            bustype: input_linux::sys::BUS_USB,
            vendor: 0x1234,
            product: 0x5678,
            version: 1,
        };
        handle.create(&id, b"niri-remote mouse", 0, &[])?;
        Ok(handle)
    }

    fn create_keyboard() -> Result<UInputHandle<File>, std::io::Error> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/uinput")?;
        let handle = UInputHandle::new(file);

        handle.set_evbit(EventKind::Key)?;
        // Enable all standard keys
        for code in 1..=248 {
            if let Some(key) = Key::from_code(code) {
                let _ = handle.set_keybit(key);
            }
        }

        let id = InputId {
            bustype: input_linux::sys::BUS_USB,
            vendor: 0x1234,
            product: 0x5679,
            version: 1,
        };
        handle.create(&id, b"niri-remote keyboard", 0, &[])?;
        Ok(handle)
    }

    fn syn_event() -> InputEvent {
        InputEvent::new(
            input_linux::EventTime::default(),
            SynchronizeKind::Report,
            0,
        )
    }

    pub async fn mouse_move(&self, dx: f64, dy: f64) -> Result<(), std::io::Error> {
        let mouse = self.mouse.lock().await;
        let events = [
            InputEvent::new(
                input_linux::EventTime::default(),
                RelativeAxis::X,
                dx as i32,
            ),
            InputEvent::new(
                input_linux::EventTime::default(),
                RelativeAxis::Y,
                dy as i32,
            ),
            Self::syn_event(),
        ];
        mouse.write(&events)
    }

    pub async fn mouse_button(
        &self,
        button: &str,
        pressed: bool,
    ) -> Result<(), std::io::Error> {
        let key = match button {
            "left" => Key::ButtonLeft,
            "right" => Key::ButtonRight,
            "middle" => Key::ButtonMiddle,
            _ => return Ok(()),
        };
        let mouse = self.mouse.lock().await;
        let events = [
            InputEvent::new(
                input_linux::EventTime::default(),
                key,
                if pressed { 1 } else { 0 },
            ),
            Self::syn_event(),
        ];
        mouse.write(&events)
    }

    pub async fn scroll(&self, dx: f64, dy: f64) -> Result<(), std::io::Error> {
        let mouse = self.mouse.lock().await;
        let events = [
            InputEvent::new(
                input_linux::EventTime::default(),
                RelativeAxis::Wheel,
                -(dy as i32),
            ),
            InputEvent::new(
                input_linux::EventTime::default(),
                RelativeAxis::HorizontalWheel,
                dx as i32,
            ),
            Self::syn_event(),
        ];
        mouse.write(&events)
    }

    pub async fn key_press(&self, key: Key) -> Result<(), std::io::Error> {
        let kb = self.keyboard.lock().await;
        let events = [
            InputEvent::new(input_linux::EventTime::default(), key, 1),
            Self::syn_event(),
        ];
        kb.write(&events)
    }

    pub async fn key_release(&self, key: Key) -> Result<(), std::io::Error> {
        let kb = self.keyboard.lock().await;
        let events = [
            InputEvent::new(input_linux::EventTime::default(), key, 0),
            Self::syn_event(),
        ];
        kb.write(&events)
    }

    pub async fn type_text(&self, text: &str) -> Result<(), std::io::Error> {
        for ch in text.chars() {
            if let Some(key) = keymap::js_key_to_linux(&ch.to_string()) {
                let needs_shift = ch.is_uppercase()
                    || matches!(
                        ch,
                        '!' | '@' | '#' | '$' | '%' | '^' | '&' | '*' | '(' | ')'
                            | '_' | '+' | '{' | '}' | '|' | ':' | '"' | '<' | '>'
                            | '?' | '~'
                    );
                if needs_shift {
                    self.key_press(Key::LeftShift).await?;
                }
                self.key_press(key).await?;
                self.key_release(key).await?;
                if needs_shift {
                    self.key_release(Key::LeftShift).await?;
                }
            }
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Add input REST routes and wire into main.rs**

Add `mod input;` and create handlers for `POST /api/input/key` and `POST /api/input/type`.

```rust
// In api routes:
.route("/api/input/key", post(input_key))
.route("/api/input/type", post(input_type))
```

Handlers extract `VirtualInput` from state, call the appropriate methods.

- [ ] **Step 5: Commit**

```bash
git add server/src/input/ server/Cargo.toml server/src/main.rs
git commit -m "feat: add uinput virtual mouse and keyboard"
```

---

## Chunk 3: Niri IPC Integration

### Task 6: Niri IPC client

**Files:**
- Create: `server/src/niri/mod.rs`
- Modify: `server/Cargo.toml` (add `niri-ipc`)
- Modify: `server/src/main.rs`

- [ ] **Step 1: Add niri-ipc dependency**

```toml
niri-ipc = "25.11"
```

- [ ] **Step 2: Create server/src/niri/mod.rs**

Connect to niri socket, implement whitelisted actions, window/workspace queries, event stream forwarding.

```rust
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use niri_ipc::{Action, Request, Response, Event};
use serde::{Deserialize, Serialize};

use crate::error::ApiError;

const ALLOWED_ACTIONS: &[&str] = &[
    "focus-window",
    "close-window",
    "fullscreen-window",
    "focus-column-left",
    "focus-column-right",
    "focus-window-up",
    "focus-window-down",
    "move-column-left",
    "move-column-right",
    "move-window-up",
    "move-window-down",
    "focus-workspace-up",
    "focus-workspace-down",
    "focus-workspace",
    "move-window-to-workspace-up",
    "move-window-to-workspace-down",
    "focus-monitor-left",
    "focus-monitor-right",
    "focus-monitor-up",
    "focus-monitor-down",
    "move-window-to-monitor-left",
    "move-window-to-monitor-right",
    "power-off-monitors",
    "power-on-monitors",
    "screenshot",
    "screenshot-screen",
    "screenshot-window",
    "maximize-column",
    "toggle-window-floating",
    "switch-preset-column-width",
    "spawn",
];

fn socket_path() -> Option<PathBuf> {
    std::env::var("NIRI_SOCKET").ok().map(PathBuf::from)
}

fn send_request(request: Request) -> Result<Response, String> {
    let path = socket_path().ok_or("NIRI_SOCKET not set")?;
    let mut stream =
        UnixStream::connect(&path).map_err(|e| format!("Failed to connect to niri: {e}"))?;

    let json = serde_json::to_string(&request).map_err(|e| e.to_string())?;
    stream
        .write_all(json.as_bytes())
        .map_err(|e| e.to_string())?;
    stream.write_all(b"\n").map_err(|e| e.to_string())?;
    stream.shutdown(std::net::Shutdown::Write).map_err(|e| e.to_string())?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;

    serde_json::from_str(&line).map_err(|e| e.to_string())
}

pub fn is_action_allowed(action: &str) -> bool {
    ALLOWED_ACTIONS.contains(&action)
}

pub fn perform_action(action: Action) -> Result<Response, String> {
    send_request(Request::Action(action))
}

pub fn get_windows() -> Result<Response, String> {
    send_request(Request::Windows)
}

pub fn get_workspaces() -> Result<Response, String> {
    send_request(Request::Workspaces)
}

pub fn get_outputs() -> Result<Response, String> {
    send_request(Request::Outputs)
}

/// Connect to niri event stream. Returns a blocking reader.
/// Should be run in a spawn_blocking or dedicated thread.
pub fn event_stream() -> Result<BufReader<UnixStream>, String> {
    let path = socket_path().ok_or("NIRI_SOCKET not set")?;
    let mut stream =
        UnixStream::connect(&path).map_err(|e| format!("Failed to connect to niri: {e}"))?;

    let json =
        serde_json::to_string(&Request::EventStream).map_err(|e| e.to_string())?;
    stream
        .write_all(json.as_bytes())
        .map_err(|e| e.to_string())?;
    stream.write_all(b"\n").map_err(|e| e.to_string())?;

    // Read initial Response::Handled
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;

    Ok(reader)
}
```

- [ ] **Step 3: Add niri REST routes**

```rust
// POST /api/niri/action
// GET /api/niri/windows
// GET /api/niri/workspaces
// GET /api/niri/outputs
// GET /api/apps (reads from settings.app_shortcuts)
// POST /api/apps/launch (resolves shortcut ID to command, calls niri spawn)
```

Parse action name from request body, validate against whitelist, map to `niri_ipc::Action` enum variant, call `perform_action`. For `POST /api/apps/launch`, look up the shortcut ID in settings, reject unknown IDs.

- [ ] **Step 4: Commit**

```bash
git add server/src/niri/ server/Cargo.toml server/src/main.rs
git commit -m "feat: add niri IPC client with whitelisted actions"
```

---

## Chunk 4: Media & Audio Subsystems

### Task 7: MPRIS media control via D-Bus

**Files:**
- Create: `server/src/media/mod.rs`
- Modify: `server/Cargo.toml` (add `zbus`)
- Modify: `server/src/main.rs`

- [ ] **Step 1: Add zbus dependency**

```toml
zbus = "5"
```

- [ ] **Step 2: Create server/src/media/mod.rs**

Use zbus to:
- List MPRIS players by enumerating D-Bus names matching `org.mpris.MediaPlayer2.*`
- Read `org.mpris.MediaPlayer2.Player` properties (Metadata, PlaybackStatus, Position, Volume, Shuffle, LoopStatus)
- Call methods (Play, Pause, Next, Previous, Stop, Seek, SetPosition)
- Set properties (Volume, Shuffle, LoopStatus)
- Proxy cover art: read `mpris:artUrl` from Metadata, if `file://` read and serve the image, if `http://` fetch and forward.

Key implementation points:
- Use `zbus::Connection::session().await` to connect to session bus
- Use `zbus::fdo::DBusProxy` to list names
- Use `zbus::proxy` macro or raw method calls for MPRIS interface
- Cache current art bytes + URL, clear on track change
- Track the "active player" selection in app state

- [ ] **Step 3: Add media REST routes**

All the `/api/media/*` endpoints from the spec. Wire into main.rs router.

- [ ] **Step 4: Commit**

```bash
git add server/src/media/ server/Cargo.toml server/src/main.rs
git commit -m "feat: add MPRIS media control via D-Bus"
```

---

### Task 8: PulseAudio audio level monitoring

**Files:**
- Create: `server/src/audio/mod.rs`
- Modify: `server/Cargo.toml` (add `libpulse-binding`)
- Modify: `server/src/main.rs`

- [ ] **Step 1: Add libpulse-binding dependency**

```toml
libpulse-binding = "2"
```

- [ ] **Step 2: Create server/src/audio/mod.rs**

Use PulseAudio's monitor source on the default sink to get peak audio levels:
- Connect to PulseAudio server
- Get default sink info, find its monitor source
- Create a recording stream on that monitor source with `PA_STREAM_PEAK_DETECT`
- Read peak values at ~30fps
- Broadcast via a `tokio::sync::broadcast` channel that WebSocket connections subscribe to

This runs in a dedicated thread (PulseAudio has its own event loop) and sends levels to the async world via the broadcast channel.

- [ ] **Step 3: Wire audio broadcast channel into app state**

Add `broadcast::Sender<AudioLevel>` to the shared app state struct.

- [ ] **Step 4: Commit**

```bash
git add server/src/audio/ server/Cargo.toml server/src/main.rs
git commit -m "feat: add PulseAudio audio level monitoring"
```

---

## Chunk 5: WebSocket

### Task 9: WebSocket endpoint

**Files:**
- Create: `server/src/ws.rs`
- Modify: `server/src/main.rs`

- [ ] **Step 1: Create server/src/ws.rs**

Handle WebSocket upgrade at `/api/ws`:
- Auth check on upgrade (extract `Tailscale-User-Login`)
- Upstream: deserialize JSON messages, dispatch to `VirtualInput` for mouse/keyboard events
- Downstream: subscribe to audio level broadcast, media progress ticker (1fps interval reading MPRIS position), and niri event stream (spawned blocking reader forwarded via channel)
- Apply trackpad sensitivity multiplier from settings to mouse_move dx/dy

```rust
use axum::{
    extract::{State, WebSocketUpgrade, ws::{Message, WebSocket}},
    response::Response,
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "type")]
enum WsInput {
    #[serde(rename = "mouse_move")]
    MouseMove { dx: f64, dy: f64 },
    #[serde(rename = "mouse_button")]
    MouseButton { button: String, state: String },
    #[serde(rename = "scroll")]
    Scroll { dx: f64, dy: f64 },
    #[serde(rename = "key_press")]
    KeyPress { key: String, modifiers: Vec<String> },
    #[serde(rename = "key_release")]
    KeyRelease { key: String },
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Spawn downstream tasks (audio, media, niri events)
    // Each writes to sender via a shared channel

    // Process upstream input messages
    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg {
            if let Ok(input) = serde_json::from_str::<WsInput>(&text) {
                match input {
                    WsInput::MouseMove { dx, dy } => {
                        let sens = state.settings.read().await.trackpad_sensitivity;
                        let _ = state.input.mouse_move(dx * sens, dy * sens).await;
                    }
                    WsInput::MouseButton { button, state: btn_state } => {
                        let pressed = btn_state == "press";
                        let _ = state.input.mouse_button(&button, pressed).await;
                    }
                    WsInput::Scroll { dx, dy } => {
                        let _ = state.input.scroll(dx, dy).await;
                    }
                    WsInput::KeyPress { key, modifiers } => {
                        // Press modifiers first, then the key
                        for m in &modifiers {
                            if let Some(k) = keymap::js_key_to_linux(m) {
                                let _ = state.input.key_press(k).await;
                            }
                        }
                        if let Some(k) = keymap::js_key_to_linux(&key) {
                            let _ = state.input.key_press(k).await;
                        }
                    }
                    WsInput::KeyRelease { key } => {
                        if let Some(k) = keymap::js_key_to_linux(&key) {
                            let _ = state.input.key_release(k).await;
                        }
                        // Release all modifiers on key release for safety
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 2: Add route `.route("/api/ws", get(ws::ws_handler))`**

- [ ] **Step 3: Commit**

```bash
git add server/src/ws.rs server/src/main.rs
git commit -m "feat: add WebSocket endpoint for input streaming and real-time data"
```

---

## Chunk 6: Shared App State & Server Assembly

### Task 10: Consolidate app state and wire everything together

**Files:**
- Create: `server/src/state.rs`
- Modify: `server/src/main.rs` (final assembly)

- [ ] **Step 1: Create server/src/state.rs**

```rust
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use crate::audio::AudioLevel;
use crate::input::VirtualInput;
use crate::settings::Settings;

#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<RwLock<Settings>>,
    pub input: Arc<VirtualInput>,
    pub audio_tx: broadcast::Sender<AudioLevel>,
}
```

- [ ] **Step 2: Update main.rs to build AppState and start all subsystems**

```rust
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let settings = Arc::new(RwLock::new(settings::load()));
    let input = VirtualInput::new().expect("Failed to create virtual input devices");
    let (audio_tx, _) = broadcast::channel(64);

    // Start audio monitoring in background
    audio::start_monitoring(audio_tx.clone());

    let state = AppState {
        settings,
        input,
        audio_tx,
    };

    let web_dir =
        std::env::var("NIRI_REMOTE_WEB_DIR").unwrap_or_else(|_| "./web/build".to_string());

    let app = Router::new()
        .route("/api/status", get(status))
        .route("/api/settings", get(get_settings).put(put_settings))
        .route("/api/input/key", post(input_key))
        .route("/api/input/type", post(input_type))
        .route("/api/media", get(get_media))
        .route("/api/media/art", get(get_media_art))
        .route("/api/media/players", get(get_media_players))
        .route("/api/media/player", post(select_media_player))
        .route("/api/media/play", post(media_play))
        .route("/api/media/pause", post(media_pause))
        .route("/api/media/next", post(media_next))
        .route("/api/media/prev", post(media_prev))
        .route("/api/media/stop", post(media_stop))
        .route("/api/media/seek", post(media_seek))
        .route("/api/media/volume", post(media_volume))
        .route("/api/media/shuffle", post(media_shuffle))
        .route("/api/media/repeat", post(media_repeat))
        .route("/api/niri/action", post(niri_action))
        .route("/api/niri/windows", get(niri_windows))
        .route("/api/niri/workspaces", get(niri_workspaces))
        .route("/api/niri/outputs", get(niri_outputs))
        .route("/api/apps", get(get_apps))
        .route("/api/apps/launch", post(launch_app))
        .route("/api/ws", get(ws::ws_handler))
        .layer(middleware::from_fn(auth::require_tailscale_auth))
        .with_state(state)
        .fallback_service(ServeDir::new(&web_dir));

    let port: u16 = std::env::var("NIRI_REMOTE_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(9876);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

- [ ] **Step 3: Verify server compiles**

```bash
cd server && cargo check
```

- [ ] **Step 4: Commit**

```bash
git add server/src/state.rs server/src/main.rs
git commit -m "feat: consolidate app state and assemble full server"
```

---

## Chunk 7: Svelte Frontend — Scaffolding & Trackpad Tab

### Task 11: Scaffold SvelteKit project

**Files:**
- Create: `web/` directory (SvelteKit project)

- [ ] **Step 1: Create SvelteKit project**

```bash
cd /home/max/Projects/tool/niri-remote
npm create svelte@latest web -- --template skeleton --types typescript
cd web && npm install
```

- [ ] **Step 2: Install dependencies**

```bash
npm install -D @sveltejs/adapter-static
```

- [ ] **Step 3: Configure adapter-static in svelte.config.js**

SPA mode — single page app with client-side routing:

```js
import adapter from '@sveltejs/adapter-static';

export default {
  kit: {
    adapter: adapter({
      fallback: 'index.html'
    })
  }
};
```

- [ ] **Step 4: Create web/src/lib/api.ts — API client**

WebSocket connection manager with auto-reconnect, REST helper functions.

```typescript
const WS_RECONNECT_BASE = 1000;
const WS_RECONNECT_MAX = 30000;

export class ApiClient {
  private ws: WebSocket | null = null;
  private reconnectDelay = WS_RECONNECT_BASE;
  private listeners: Map<string, Set<(data: any) => void>> = new Map();

  async get<T>(path: string): Promise<T> {
    const res = await fetch(`/api${path}`);
    if (!res.ok) throw await res.json();
    return res.json();
  }

  async post<T>(path: string, body?: unknown): Promise<T> {
    const res = await fetch(`/api${path}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: body ? JSON.stringify(body) : undefined,
    });
    if (!res.ok) throw await res.json();
    return res.json();
  }

  async put<T>(path: string, body: unknown): Promise<T> {
    const res = await fetch(`/api${path}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw await res.json();
    return res.json();
  }

  connectWs() {
    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    this.ws = new WebSocket(`${proto}//${location.host}/api/ws`);

    this.ws.onopen = () => {
      this.reconnectDelay = WS_RECONNECT_BASE;
    };

    this.ws.onmessage = (ev) => {
      const msg = JSON.parse(ev.data);
      const handlers = this.listeners.get(msg.type);
      if (handlers) handlers.forEach(h => h(msg));
    };

    this.ws.onclose = () => {
      setTimeout(() => this.connectWs(), this.reconnectDelay);
      this.reconnectDelay = Math.min(this.reconnectDelay * 2, WS_RECONNECT_MAX);
    };
  }

  send(msg: unknown) {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(msg));
    }
  }

  on(type: string, handler: (data: any) => void) {
    if (!this.listeners.has(type)) this.listeners.set(type, new Set());
    this.listeners.get(type)!.add(handler);
    return () => this.listeners.get(type)?.delete(handler);
  }
}

export const api = new ApiClient();
```

- [ ] **Step 5: Create web/src/lib/stores.ts — shared Svelte stores**

```typescript
import { writable } from 'svelte/store';

export const connected = writable(false);
export const audioLevels = writable({ left: 0, right: 0 });
export const mediaState = writable<any>(null);
```

- [ ] **Step 6: Commit**

```bash
git add web/
git commit -m "feat: scaffold SvelteKit project with API client and stores"
```

---

### Task 12: Tab layout and trackpad tab

**Files:**
- Create: `web/src/routes/+layout.svelte`
- Create: `web/src/routes/+page.svelte` (redirects to /trackpad)
- Create: `web/src/routes/trackpad/+page.svelte`
- Create: `web/src/lib/components/TabBar.svelte`

- [ ] **Step 1: Create TabBar.svelte**

Mobile bottom tab bar with four tabs: Trackpad, Media, Windows, Settings. Uses icons and highlights active tab.

- [ ] **Step 2: Create +layout.svelte**

Root layout that initializes the API client, connects WebSocket, renders tab bar at bottom.

- [ ] **Step 3: Create trackpad/+page.svelte**

Full-screen touch area:
- `touchstart`/`touchmove`/`touchend` handlers
- Track finger count for gesture detection
- Single finger drag → `mouse_move` (batched at 16ms via requestAnimationFrame)
- Single tap (< 200ms, < 10px movement) → left click
- Two-finger tap → right click
- Two-finger scroll → `scroll` events
- Keyboard toggle button → shows/hides text input
- Modifier key row (Ctrl, Alt, Super, Shift) with toggle state
- Keypresses from the text input sent as `key_press`/`key_release` via WebSocket

- [ ] **Step 4: Commit**

```bash
git add web/src/
git commit -m "feat: add tab layout and trackpad page with touch input"
```

---

## Chunk 8: Frontend — Media, Windows, Settings Tabs

### Task 13: Media tab

**Files:**
- Create: `web/src/routes/media/+page.svelte`

- [ ] **Step 1: Build media tab UI**

- On mount, fetch `GET /api/media` and `GET /api/media/players`
- Subscribe to WebSocket `media_progress`, `media_changed`, `audio_level` events
- Cover art: `<img src="/api/media/art">`
- Track info display
- Seekable progress bar (click/drag sends `POST /api/media/seek`)
- Transport buttons calling respective POST endpoints
- Volume slider calling `POST /api/media/volume`
- Audio level visualizer using canvas or CSS bars, driven by `audio_level` WebSocket events
- Player selector dropdown if multiple players

- [ ] **Step 2: Commit**

```bash
git add web/src/routes/media/
git commit -m "feat: add media tab with playback controls and audio visualizer"
```

---

### Task 14: Windows/Power tab

**Files:**
- Create: `web/src/routes/windows/+page.svelte`

- [ ] **Step 1: Build windows tab UI**

- On mount, fetch `GET /api/niri/windows` and `GET /api/niri/workspaces`
- Subscribe to WebSocket `niri_event` for live updates
- Workspace list — tap to switch (`focus-workspace`)
- Window list per workspace — tap to focus, swipe left to close
- Action button grid — buttons for each action in `settings.visible_actions`, calling `POST /api/niri/action`
- App launcher section — buttons from `settings.app_shortcuts`, calling `POST /api/apps/launch`

- [ ] **Step 2: Commit**

```bash
git add web/src/routes/windows/
git commit -m "feat: add windows/power management tab"
```

---

### Task 15: Settings tab

**Files:**
- Create: `web/src/routes/settings/+page.svelte`

- [ ] **Step 1: Build settings tab UI**

- On mount, fetch `GET /api/settings` and `GET /api/status`
- Display connection status and Tailscale identity from status endpoint
- Trackpad sensitivity slider (0.1 — 3.0)
- Theme toggle (dark/light) — applies CSS class to root
- Action button configurator: checklist of all available niri actions, check/uncheck to show/hide on windows tab
- App shortcut editor: list of name + command pairs with add/remove/edit
- Audio device selector (future: populated from PulseAudio sink list)
- Save button calls `PUT /api/settings`

- [ ] **Step 2: Commit**

```bash
git add web/src/routes/settings/
git commit -m "feat: add settings tab"
```

---

## Chunk 9: NixOS Module & Packaging

### Task 16: NixOS module

**Files:**
- Create: `module.nix`

- [ ] **Step 1: Create module.nix**

```nix
self:
{ config, lib, pkgs, ... }:

let
  cfg = config.services.niri-remote;
  package = self.packages.${pkgs.system}.default;
  webPackage = self.packages.${pkgs.system}.web;
in {
  options.services.niri-remote = {
    enable = lib.mkEnableOption "niri-remote desktop remote control";

    port = lib.mkOption {
      type = lib.types.port;
      default = 9876;
      description = "Port for the HTTP server (binds 127.0.0.1 only)";
    };
  };

  config = lib.mkIf cfg.enable {
    # uinput group and udev rule
    users.groups.uinput = {};
    users.users.${config.users.users.max.name}.extraGroups = [ "uinput" ];
    services.udev.extraRules = ''
      KERNEL=="uinput", SUBSYSTEM=="misc", MODE="0660", GROUP="uinput"
    '';

    # Tailscale serve config
    services.tailscale.permitCertUid = "root";

    systemd.user.services.niri-remote = {
      description = "niri-remote desktop remote control";
      wantedBy = [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];

      environment = {
        NIRI_REMOTE_PORT = toString cfg.port;
        NIRI_REMOTE_WEB_DIR = "${webPackage}";
        NIRI_SOCKET = "%t/niri-socket";
        RUST_LOG = "info";
      };

      serviceConfig = {
        ExecStart = "${package}/bin/niri-remote-server";
        Restart = "on-failure";
        RestartSec = 5;
      };
    };
  };
}
```

- [ ] **Step 2: Finalize flake.nix**

Ensure `packages.default`, `packages.web`, `packages.server`, and `nixosModules.default` are all properly exported. Fix hashes after first build.

- [ ] **Step 3: Commit**

```bash
git add module.nix flake.nix
git commit -m "feat: add NixOS module with systemd service and uinput setup"
```

---

### Task 17: Integration with nixfiles

**Files:**
- Modify: `~/nixfiles/flake.nix` (add input)
- Modify: `~/nixfiles/hosts/tar-elendil/default.nix` (enable service)

- [ ] **Step 1: Add niri-remote as flake input**

In `~/nixfiles/flake.nix` inputs:
```nix
niri-remote.url = "path:/home/max/Projects/tool/niri-remote";
```

Pass through to modules and enable in tar-elendil host config:
```nix
services.niri-remote.enable = true;
```

- [ ] **Step 2: Test build**

```bash
cd ~/nixfiles && nix build .#nixosConfigurations.tar-elendil.config.system.build.toplevel --dry-run
```

- [ ] **Step 3: Commit nixfiles changes**

```bash
cd ~/nixfiles
git add flake.nix hosts/tar-elendil/default.nix
git commit -m "feat: add niri-remote integration"
```

---

## Chunk 10: End-to-End Testing & Polish

### Task 18: Manual integration test

- [ ] **Step 1: Build and run server locally**

```bash
cd /home/max/Projects/tool/niri-remote/server
NIRI_REMOTE_WEB_DIR=../web/build RUST_LOG=debug cargo run
```

- [ ] **Step 2: Test status endpoint**

```bash
curl -H "Tailscale-User-Login: test@test.com" http://127.0.0.1:9876/api/status
```

- [ ] **Step 3: Test web UI loads**

Open http://127.0.0.1:9876 in browser (will need to bypass auth for local testing or add a dev mode flag).

- [ ] **Step 4: Test input, media, niri endpoints**

Verify each subsystem responds correctly.

- [ ] **Step 5: Configure Tailscale Serve and test from phone**

```bash
sudo tailscale serve --bg 9876
```

Open https://tar-elendil.<tailnet-name>.ts.net from phone.

- [ ] **Step 6: Fix issues found during testing**

- [ ] **Step 7: Final commit**

```bash
git add -A
git commit -m "fix: integration testing fixes"
```
