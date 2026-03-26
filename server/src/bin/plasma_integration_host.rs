//! telemax-host: Native messaging host for KDE Plasma Browser Integration extension.
//!
//! Implements only the MPRIS bridge — receives media events from the browser extension
//! and exposes them as MPRIS2 D-Bus services. Forwards D-Bus commands back to the browser.
//!
//! Protocol: Chrome Native Messaging (4-byte LE length prefix + JSON on stdin/stdout).

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::ops::Deref;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use zbus::connection::Builder as ConnectionBuilder;
use zbus::interface;
use zbus::zvariant::Value;

// ---------------------------------------------------------------------------
// Native Messaging I/O
// ---------------------------------------------------------------------------

fn read_message() -> io::Result<Option<serde_json::Value>> {
    let mut len_buf = [0u8; 4];
    match io::stdin().read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > 1024 * 1024 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "message too large"));
    }
    let mut buf = vec![0u8; len];
    io::stdin().read_exact(&mut buf)?;
    let msg: serde_json::Value = serde_json::from_slice(&buf)?;
    Ok(Some(msg))
}

fn write_message(msg: &serde_json::Value) -> io::Result<()> {
    let data = serde_json::to_vec(msg)?;
    let len = (data.len() as u32).to_le_bytes();
    let mut stdout = io::stdout().lock();
    stdout.write_all(&len)?;
    stdout.write_all(&data)?;
    stdout.flush()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Incoming messages from the extension
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ExtensionMessage {
    subsystem: String,
    #[serde(default)]
    action: String,
    #[serde(default)]
    payload: serde_json::Value,
}

// ---------------------------------------------------------------------------
// MPRIS Player State
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct PlayerState {
    status: String, // Playing, Paused, Stopped
    title: String,
    artist: String,
    album: String,
    art_url: String,
    url: String,
    duration_us: i64,
    position_us: i64,
    volume: f64,
    rate: f64,
    loop_status: String, // None, Track, Playlist
    can_go_next: bool,
    can_go_previous: bool,
    can_play: bool,
    can_pause: bool,
    can_seek: bool,
    identity: String,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            status: "Stopped".into(),
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            art_url: String::new(),
            url: String::new(),
            duration_us: 0,
            position_us: 0,
            volume: 1.0,
            rate: 1.0,
            loop_status: "None".into(),
            can_go_next: false,
            can_go_previous: false,
            can_play: true,
            can_pause: true,
            can_seek: true,
            identity: "Browser Media".into(),
        }
    }
}

type SharedState = Arc<Mutex<PlayerState>>;
type CommandSender = mpsc::UnboundedSender<serde_json::Value>;

// ---------------------------------------------------------------------------
// MPRIS2 D-Bus Interface: org.mpris.MediaPlayer2
// ---------------------------------------------------------------------------

struct MprisRoot {
    state: SharedState,
}

#[interface(name = "org.mpris.MediaPlayer2")]
impl MprisRoot {
    #[zbus(property)]
    async fn identity(&self) -> String {
        self.state.lock().await.identity.clone()
    }

    #[zbus(property)]
    async fn can_raise(&self) -> bool {
        true
    }

    #[zbus(property)]
    async fn can_quit(&self) -> bool {
        false
    }

    #[zbus(property)]
    async fn has_track_list(&self) -> bool {
        false
    }

    #[zbus(property)]
    async fn supported_uri_schemes(&self) -> Vec<String> {
        vec![]
    }

    #[zbus(property)]
    async fn supported_mime_types(&self) -> Vec<String> {
        vec![]
    }

    async fn raise(&self) {
        // Could send raise command back to browser
    }

    async fn quit(&self) {}
}

// ---------------------------------------------------------------------------
// MPRIS2 D-Bus Interface: org.mpris.MediaPlayer2.Player
// ---------------------------------------------------------------------------

struct MprisPlayer {
    state: SharedState,
    cmd_tx: CommandSender,
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl MprisPlayer {
    async fn play(&self) {
        let _ = self.cmd_tx.send(serde_json::json!({
            "subsystem": "mpris",
            "action": "play"
        }));
    }

    async fn pause(&self) {
        let _ = self.cmd_tx.send(serde_json::json!({
            "subsystem": "mpris",
            "action": "pause"
        }));
    }

    async fn play_pause(&self) {
        let _ = self.cmd_tx.send(serde_json::json!({
            "subsystem": "mpris",
            "action": "playPause"
        }));
    }

    async fn stop(&self) {
        let _ = self.cmd_tx.send(serde_json::json!({
            "subsystem": "mpris",
            "action": "stop"
        }));
    }

    async fn next(&self) {
        let _ = self.cmd_tx.send(serde_json::json!({
            "subsystem": "mpris",
            "action": "next"
        }));
    }

    async fn previous(&self) {
        let _ = self.cmd_tx.send(serde_json::json!({
            "subsystem": "mpris",
            "action": "previous"
        }));
    }

    async fn seek(&self, offset: i64) {
        let state = self.state.lock().await;
        let new_pos = (state.position_us + offset).max(0);
        drop(state);
        let _ = self.cmd_tx.send(serde_json::json!({
            "subsystem": "mpris",
            "action": "setPosition",
            "payload": { "position": new_pos as f64 / 1_000_000.0 }
        }));
    }

    async fn set_position(&self, _track_id: zbus::zvariant::ObjectPath<'_>, position: i64) {
        let _ = self.cmd_tx.send(serde_json::json!({
            "subsystem": "mpris",
            "action": "setPosition",
            "payload": { "position": position as f64 / 1_000_000.0 }
        }));
    }

    async fn open_uri(&self, _uri: &str) {}

    #[zbus(property)]
    async fn playback_status(&self) -> String {
        self.state.lock().await.status.clone()
    }

    #[zbus(property)]
    async fn loop_status(&self) -> String {
        self.state.lock().await.loop_status.clone()
    }

    #[zbus(property)]
    async fn set_loop_status(&self, value: String) {
        self.state.lock().await.loop_status = value.clone();
        let _ = self.cmd_tx.send(serde_json::json!({
            "subsystem": "mpris",
            "action": "setLoop",
            "payload": { "loop": value }
        }));
    }

    #[zbus(property)]
    async fn rate(&self) -> f64 {
        self.state.lock().await.rate
    }

    #[zbus(property)]
    async fn set_rate(&self, value: f64) {
        let _ = self.cmd_tx.send(serde_json::json!({
            "subsystem": "mpris",
            "action": "setPlaybackRate",
            "payload": { "playbackRate": value }
        }));
    }

    #[zbus(property)]
    async fn volume(&self) -> f64 {
        self.state.lock().await.volume
    }

    #[zbus(property)]
    async fn set_volume(&self, value: f64) {
        self.state.lock().await.volume = value;
        let _ = self.cmd_tx.send(serde_json::json!({
            "subsystem": "mpris",
            "action": "setVolume",
            "payload": { "volume": value }
        }));
    }

    #[zbus(property)]
    async fn position(&self) -> i64 {
        self.state.lock().await.position_us
    }

    #[zbus(property)]
    async fn minimum_rate(&self) -> f64 {
        0.25
    }

    #[zbus(property)]
    async fn maximum_rate(&self) -> f64 {
        4.0
    }

    #[zbus(property)]
    async fn can_go_next(&self) -> bool {
        self.state.lock().await.can_go_next
    }

    #[zbus(property)]
    async fn can_go_previous(&self) -> bool {
        self.state.lock().await.can_go_previous
    }

    #[zbus(property)]
    async fn can_play(&self) -> bool {
        self.state.lock().await.can_play
    }

    #[zbus(property)]
    async fn can_pause(&self) -> bool {
        self.state.lock().await.can_pause
    }

    #[zbus(property)]
    async fn can_seek(&self) -> bool {
        self.state.lock().await.can_seek
    }

    #[zbus(property)]
    async fn can_control(&self) -> bool {
        true
    }

    #[zbus(property)]
    async fn metadata(&self) -> HashMap<String, Value<'static>> {
        let s = self.state.lock().await;
        let mut map = HashMap::new();
        map.insert(
            "mpris:trackid".into(),
            Value::new(zbus::zvariant::ObjectPath::try_from("/org/mpris/MediaPlayer2/telemax").unwrap()),
        );
        if !s.title.is_empty() {
            map.insert("xesam:title".into(), Value::new(s.title.clone()));
        }
        if !s.artist.is_empty() {
            map.insert(
                "xesam:artist".into(),
                Value::new(vec![s.artist.clone()]),
            );
        }
        if !s.album.is_empty() {
            map.insert("xesam:album".into(), Value::new(s.album.clone()));
        }
        if !s.art_url.is_empty() {
            map.insert("mpris:artUrl".into(), Value::new(s.art_url.clone()));
        }
        if !s.url.is_empty() {
            map.insert("xesam:url".into(), Value::new(s.url.clone()));
        }
        if s.duration_us > 0 {
            map.insert("mpris:length".into(), Value::new(s.duration_us));
        }
        map
    }
}

// ---------------------------------------------------------------------------
// Process extension messages
// ---------------------------------------------------------------------------

async fn handle_extension_message(
    msg: &ExtensionMessage,
    state: &SharedState,
    connection: &zbus::Connection,
) {
    if msg.subsystem != "mpris" {
        return;
    }

    let payload = &msg.payload;
    let mut s = state.lock().await;

    match msg.action.as_str() {
        "playing" => {
            s.status = "Playing".into();
            update_from_payload(&mut s, payload);
        }
        "paused" => {
            s.status = "Paused".into();
        }
        "stopped" | "gone" => {
            s.status = "Stopped".into();
        }
        "waiting" | "canplay" => {}
        "duration" => {
            if let Some(d) = payload.get("duration").and_then(|v| v.as_f64()) {
                s.duration_us = (d * 1_000_000.0) as i64;
            }
        }
        "timeupdate" => {
            if let Some(t) = payload.get("currentTime").and_then(|v| v.as_f64()) {
                s.position_us = (t * 1_000_000.0) as i64;
            }
            // Don't emit PropertiesChanged for position updates (too frequent)
            return;
        }
        "ratechange" => {
            if let Some(r) = payload.get("playbackRate").and_then(|v| v.as_f64()) {
                s.rate = r;
            }
        }
        "volumechange" => {
            if let Some(v) = payload.get("volume").and_then(|v| v.as_f64()) {
                s.volume = v;
            }
        }
        "metadata" => {
            if let Some(meta) = payload.as_object() {
                if let Some(t) = meta.get("title").and_then(|v| v.as_str()) {
                    s.title = t.to_string();
                }
                if let Some(a) = meta.get("artist").and_then(|v| v.as_str()) {
                    s.artist = a.to_string();
                }
                if let Some(a) = meta.get("album").and_then(|v| v.as_str()) {
                    s.album = a.to_string();
                }
            }
        }
        "titlechange" => {
            if let Some(t) = payload.get("pageTitle").and_then(|v| v.as_str()) {
                // Use page title as identity if we don't have a media title
                if s.title.is_empty() {
                    s.title = t.to_string();
                }
                s.identity = t.to_string();
            }
        }
        "callbacks" => {
            if let Some(cbs) = payload.as_array() {
                let has = |name: &str| cbs.iter().any(|v| v.as_str() == Some(name));
                s.can_go_next = has("nexttrack");
                s.can_go_previous = has("previoustrack");
            }
        }
        "artwork" => {
            // The extension sends artwork data, we could save to a temp file
            // For now, use the src URL directly if available
            if let Some(src) = payload.get("src").and_then(|v| v.as_str()) {
                s.art_url = src.to_string();
            }
        }
        _ => {}
    }

    // Emit PropertiesChanged signal
    drop(s);
    let _ = emit_properties_changed(connection).await;
}

fn update_from_payload(state: &mut PlayerState, payload: &serde_json::Value) {
    if let Some(t) = payload.get("tabTitle").and_then(|v| v.as_str()) {
        state.identity = t.to_string();
    }
    if let Some(d) = payload.get("duration").and_then(|v| v.as_f64()) {
        state.duration_us = (d * 1_000_000.0) as i64;
    }
    if let Some(t) = payload.get("currentTime").and_then(|v| v.as_f64()) {
        state.position_us = (t * 1_000_000.0) as i64;
    }
    if let Some(v) = payload.get("volume").and_then(|v| v.as_f64()) {
        state.volume = v;
    }
    if let Some(r) = payload.get("playbackRate").and_then(|v| v.as_f64()) {
        state.rate = r;
    }
    if let Some(url) = payload.get("url").and_then(|v| v.as_str()) {
        state.url = url.to_string();
    }
    if let Some(poster) = payload.get("poster").and_then(|v| v.as_str()) {
        if !poster.is_empty() {
            state.art_url = poster.to_string();
        }
    }
    // Check metadata sub-object
    if let Some(meta) = payload.get("metadata").and_then(|v| v.as_object()) {
        if let Some(t) = meta.get("title").and_then(|v| v.as_str()) {
            state.title = t.to_string();
        }
        if let Some(a) = meta.get("artist").and_then(|v| v.as_str()) {
            state.artist = a.to_string();
        }
        if let Some(a) = meta.get("album").and_then(|v| v.as_str()) {
            state.album = a.to_string();
        }
    }
    // Use page/tab title as fallback for media title
    if state.title.is_empty() {
        if let Some(t) = payload.get("pageTitle").and_then(|v| v.as_str()) {
            state.title = t.to_string();
        }
    }
    // Check callbacks
    if let Some(cbs) = payload.get("callbacks").and_then(|v| v.as_array()) {
        let has = |name: &str| cbs.iter().any(|v| v.as_str() == Some(name));
        state.can_go_next = has("nexttrack");
        state.can_go_previous = has("previoustrack");
    }
}

async fn emit_properties_changed(connection: &zbus::Connection) -> zbus::Result<()> {
    // Emit org.freedesktop.DBus.Properties.PropertiesChanged signal
    // with invalidated properties so clients re-fetch
    let invalidated: Vec<&str> = vec![
        "PlaybackStatus", "Metadata", "Volume", "Rate", "LoopStatus",
        "CanGoNext", "CanGoPrevious", "CanPlay", "CanPause", "CanSeek",
    ];
    let changed: HashMap<&str, Value> = HashMap::new();
    connection
        .emit_signal(
            None::<&str>,
            "/org/mpris/MediaPlayer2",
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
            &("org.mpris.MediaPlayer2.Player", &changed, &invalidated),
        )
        .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let state: SharedState = Arc::new(Mutex::new(PlayerState::default()));
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<serde_json::Value>();

    // Build D-Bus connection with MPRIS2 interfaces
    let connection = ConnectionBuilder::session()?
        .name("org.mpris.MediaPlayer2.telemax")?
        .serve_at(
            "/org/mpris/MediaPlayer2",
            MprisRoot {
                state: state.clone(),
            },
        )?
        .serve_at(
            "/org/mpris/MediaPlayer2",
            MprisPlayer {
                state: state.clone(),
                cmd_tx: cmd_tx.clone(),
            },
        )?
        .build()
        .await?;

    tracing::info!("telemax-host started, MPRIS service registered");

    // Send initial settings message to the extension
    let _ = write_message(&serde_json::json!({
        "subsystem": "settings",
        "action": "getSubsystemStatus"
    }));

    // Spawn task to forward D-Bus commands back to the extension
    tokio::spawn(async move {
        while let Some(msg) = cmd_rx.recv().await {
            if write_message(&msg).is_err() {
                break;
            }
        }
    });

    // Read messages from the extension on a blocking thread
    let state_clone = state.clone();
    let conn_clone = connection.clone();
    tokio::task::spawn_blocking(move || {
        loop {
            match read_message() {
                Ok(Some(msg)) => {
                    if let Ok(ext_msg) = serde_json::from_value::<ExtensionMessage>(msg) {
                        let state = state_clone.clone();
                        let conn = conn_clone.clone();
                        tokio::runtime::Handle::current().block_on(async {
                            handle_extension_message(&ext_msg, &state, &conn).await;
                        });
                    }
                }
                Ok(None) => break, // EOF — extension disconnected
                Err(e) => {
                    tracing::error!("Error reading message: {e}");
                    break;
                }
            }
        }
    })
    .await?;

    Ok(())
}
