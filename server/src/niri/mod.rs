use std::io::{BufRead, BufReader, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use niri_ipc::{Action, Reply, Request, Response};

/// Whitelisted action names (kebab-case) that clients are allowed to invoke.
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

/// Read the niri IPC socket path from the `NIRI_SOCKET` environment variable.
fn socket_path() -> Option<PathBuf> {
    std::env::var_os("NIRI_SOCKET").map(PathBuf::from)
}

/// Connect to the niri IPC socket, send a request, and read the response.
///
/// Protocol: write a JSON-encoded `Request` on a single line, shut down the
/// write half, then read a JSON-encoded `Reply` (which is `Result<Response, String>`)
/// on a single line.
fn send_request(request: Request) -> Result<Response, String> {
    let path = socket_path().ok_or_else(|| "NIRI_SOCKET is not set".to_string())?;

    let mut stream =
        UnixStream::connect(&path).map_err(|e| format!("failed to connect to niri socket: {e}"))?;

    let mut buf = serde_json::to_string(&request)
        .map_err(|e| format!("failed to serialize request: {e}"))?;
    buf.push('\n');

    stream
        .write_all(buf.as_bytes())
        .map_err(|e| format!("failed to write request: {e}"))?;

    stream
        .shutdown(Shutdown::Write)
        .map_err(|e| format!("failed to shut down write half: {e}"))?;

    let mut reader = BufReader::new(stream);
    let mut response_buf = String::new();
    reader
        .read_line(&mut response_buf)
        .map_err(|e| format!("failed to read response: {e}"))?;

    let reply: Reply =
        serde_json::from_str(&response_buf).map_err(|e| format!("failed to parse response: {e}"))?;

    reply
}

/// Check whether the given kebab-case action name is in the whitelist.
pub fn is_action_allowed(action: &str) -> bool {
    ALLOWED_ACTIONS.contains(&action)
}

/// Send an `Action` to niri for execution.
pub fn perform_action(action: Action) -> Result<Response, String> {
    send_request(Request::Action(action))
}

/// Request the list of open windows from niri.
pub fn get_windows() -> Result<Response, String> {
    send_request(Request::Windows)
}

/// Request the list of workspaces from niri.
pub fn get_workspaces() -> Result<Response, String> {
    send_request(Request::Workspaces)
}

/// Request the list of connected outputs from niri.
pub fn get_outputs() -> Result<Response, String> {
    send_request(Request::Outputs)
}

/// Open an event stream connection to niri.
///
/// Sends `Request::EventStream`, reads the initial `Response::Handled` reply,
/// then returns the `BufReader<UnixStream>` for the caller to continuously
/// read JSON-encoded `Event` lines from.
pub fn event_stream() -> Result<BufReader<UnixStream>, String> {
    let path = socket_path().ok_or_else(|| "NIRI_SOCKET is not set".to_string())?;

    let mut stream =
        UnixStream::connect(&path).map_err(|e| format!("failed to connect to niri socket: {e}"))?;

    let mut buf = serde_json::to_string(&Request::EventStream)
        .map_err(|e| format!("failed to serialize EventStream request: {e}"))?;
    buf.push('\n');

    stream
        .write_all(buf.as_bytes())
        .map_err(|e| format!("failed to write EventStream request: {e}"))?;
    stream
        .flush()
        .map_err(|e| format!("failed to flush EventStream request: {e}"))?;

    let mut reader = BufReader::new(stream);

    // Read the initial Reply (should be Ok(Response::Handled)).
    let mut initial_response = String::new();
    reader
        .read_line(&mut initial_response)
        .map_err(|e| format!("failed to read initial EventStream response: {e}"))?;

    let reply: Reply = serde_json::from_str(&initial_response)
        .map_err(|e| format!("failed to parse initial EventStream response: {e}"))?;

    match reply {
        Ok(Response::Handled) => {}
        Ok(other) => {
            return Err(format!(
                "unexpected initial EventStream response: {other:?}"
            ));
        }
        Err(e) => {
            return Err(format!("niri returned error for EventStream: {e}"));
        }
    }

    // Return the reader — caller reads Event lines continuously.
    Ok(reader)
}
