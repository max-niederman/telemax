use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};

use crate::input::keymap;
use crate::state::AppState;
use input_linux::Key;

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
    KeyPress {
        key: String,
        modifiers: Vec<String>,
    },
    #[serde(rename = "key_release")]
    KeyRelease { key: String },
}

#[derive(Serialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
enum WsOutput {
    #[serde(rename = "audio_level")]
    AudioLevel { bands: Vec<f32> },
    #[serde(rename = "media_state")]
    MediaState {
        #[serde(flatten)]
        state: crate::media::MediaState,
        players: Vec<crate::media::PlayerInfo>,
    },
    #[serde(rename = "niri_event")]
    NiriEvent { event: serde_json::Value },
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Use a channel to multiplex outgoing messages from multiple tasks
    let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<WsOutput>(64);

    // Task: forward outgoing messages to the WebSocket sender
    let send_task = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if let Ok(text) = serde_json::to_string(&msg) {
                if sender.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    // Sender task 1: Audio levels from broadcast channel
    let audio_out_tx = out_tx.clone();
    let mut audio_rx = state.audio_tx.subscribe();
    let audio_task = tokio::spawn(async move {
        loop {
            match audio_rx.recv().await {
                Ok(level) => {
                    let msg = WsOutput::AudioLevel {
                        bands: level.bands,
                    };
                    if audio_out_tx.send(msg).await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    // Skip lagged messages
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    });

    // Sender task 2: Media state — poll at 4Hz, send full state on change, position at 1Hz
    let media_out_tx = out_tx.clone();
    let media = state.media.clone();
    let media_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(250));
        let mut last_sent: Option<String> = None; // JSON of last sent state (for diffing)
        let mut position_tick: u8 = 0;
        loop {
            interval.tick().await;
            let Ok(media_state) = media.get_state().await else { continue };
            let players = media.list_players().await.unwrap_or_default();

            // Serialize for comparison (excluding position which changes continuously)
            let mut cmp_state = media_state.clone();
            cmp_state.position_ms = None;
            let cmp_json = serde_json::to_string(&cmp_state).unwrap_or_default();

            let state_changed = last_sent.as_ref() != Some(&cmp_json);
            position_tick = position_tick.wrapping_add(1);
            let position_due = position_tick % 4 == 0; // every 1s (4 * 250ms)

            if state_changed || position_due {
                let msg = WsOutput::MediaState {
                    state: media_state,
                    players,
                };
                if media_out_tx.send(msg).await.is_err() {
                    break;
                }
                if state_changed {
                    last_sent = Some(cmp_json);
                }
            }
        }
    });

    // Receiver task: parse incoming WsInput messages
    let input = state.input.clone();
    let settings = state.settings.clone();
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                let parsed: Result<WsInput, _> = serde_json::from_str(&text);
                match parsed {
                    Ok(ws_input) => {
                        if let Some(ref input) = input {
                            handle_ws_input(&ws_input, input, &settings).await;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Invalid WS message: {e}");
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    // Client disconnected — cancel sender tasks
    send_task.abort();
    audio_task.abort();
    media_task.abort();
}

async fn handle_ws_input(
    msg: &WsInput,
    input: &crate::input::VirtualInput,
    settings: &std::sync::Arc<tokio::sync::RwLock<crate::settings::Settings>>,
) {
    match msg {
        WsInput::MouseMove { dx, dy } => {
            let sensitivity = settings.read().await.trackpad_sensitivity;
            input.mouse_move(dx * sensitivity, dy * sensitivity).await;
        }
        WsInput::MouseButton { button, state } => {
            let pressed = state == "press";
            input.mouse_button(button, pressed).await;
        }
        WsInput::Scroll { dx, dy } => {
            input.scroll(*dx, *dy).await;
        }
        WsInput::KeyPress { key, modifiers } => {
            // Press modifier keys first
            for modifier in modifiers {
                if let Some(mod_key) = keymap::js_key_to_linux(modifier) {
                    input.key_press(mod_key).await;
                }
            }
            // For single characters, use char_to_key_shifted to handle uppercase + symbols
            let chars: Vec<char> = key.chars().collect();
            if chars.len() == 1 {
                if let Some((linux_key, needs_shift)) = keymap::char_to_key_shifted(chars[0]) {
                    if needs_shift {
                        input.key_press(Key::LeftShift).await;
                    }
                    input.key_press(linux_key).await;
                }
            } else if let Some(linux_key) = keymap::js_key_to_linux(key) {
                input.key_press(linux_key).await;
            }
        }
        WsInput::KeyRelease { key } => {
            let chars: Vec<char> = key.chars().collect();
            if chars.len() == 1 {
                if let Some((linux_key, needs_shift)) = keymap::char_to_key_shifted(chars[0]) {
                    input.key_release(linux_key).await;
                    if needs_shift {
                        input.key_release(Key::LeftShift).await;
                    }
                }
            } else if let Some(linux_key) = keymap::js_key_to_linux(key) {
                input.key_release(linux_key).await;
            }
        }
    }
}
