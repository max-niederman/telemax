# niri-remote — Design Spec

## Overview

A remote control system for a Niri-based Linux desktop, accessed from a mobile phone over Tailscale. Consists of a Rust server daemon and a Svelte web frontend, exposed via Tailscale Serve for zero-config TLS and authentication.

## Components

### niri-remote-server (Rust)

A systemd user service that:

- Serves the Svelte SPA as static files
- Exposes a REST API for discrete actions
- Exposes a WebSocket endpoint for real-time bidirectional streaming
- Binds to `127.0.0.1` only — Tailscale Serve reverse-proxies to it
- Authenticates requests via Tailscale Serve headers (`Tailscale-User-Login`)

**System integrations:**

| Integration | Mechanism | Purpose |
|---|---|---|
| Mouse/keyboard input | uinput (`/dev/uinput`) | Virtual mouse + keyboard devices |
| Media playback | D-Bus MPRIS | Track info, cover art, playback control |
| Audio levels | PulseAudio API (`libpulse-binding`) via PipeWire's PulseAudio compat layer | Real-time VU meter data from default sink monitor |
| Window/workspace mgmt | Niri IPC socket (`$NIRI_SOCKET`) | Window list, workspace state, compositor actions |
| App launching | Niri IPC (`spawn`) | Launch user-configured application shortcuts |
| Monitor power | Niri IPC | Power on/off monitors via DPMS |

### niri-remote-web (Svelte)

A mobile-first SPA with four tabs:

**Tab 1 — Trackpad/Keyboard**
- Full-screen touch area for relative mouse movement
- Tap = left click, two-finger tap = right click, two-finger swipe = scroll
- No bottom bar — all clicks via gestures
- Keyboard toggle opens a text input that streams keypresses over WebSocket
- Modifier key row (Ctrl, Alt, Super, Shift) with toggle behavior

**Tab 2 — Media**
- Cover art prominently displayed
- Track title, artist, album text
- Seekable progress bar
- Transport controls: play/pause, prev, next, shuffle, repeat
- Volume slider
- Real-time audio level visualizer
- Player selector when multiple MPRIS players are active

**Tab 3 — Window/Power Management**
- Live workspace and window list (from niri event stream)
- Tap window to focus, swipe to close
- Workspace switcher
- Action button grid: screenshot, lock, monitors on/off, fullscreen toggle, etc.
- App launcher section with user-configured shortcuts

**Tab 4 — Settings**
- Connection status and Tailscale identity display
- Trackpad sensitivity / acceleration curve
- Light/dark theme toggle
- Configure which action buttons appear on tab 3
- Configure app launch shortcuts (name + command pairs)
- Audio output device selector (changes which device the VU meter monitors and volume controls target — does not change system-wide default)

## Concurrency

Multiple clients can connect simultaneously (e.g., phone + tablet). Behavior:

- All clients share the same uinput virtual devices — input from any client moves the same cursor / types on the same keyboard. This is intentional; it matches KDE Connect behavior.
- Each client gets its own independent WebSocket stream.
- Settings are global (single-user desktop). Any client can read/write them. Last write wins.

## API Design

### Key Naming Convention

All key names use JavaScript `KeyboardEvent.key` values (e.g., `"a"`, `"Enter"`, `"ArrowLeft"`, `"Control"`). The server maps these to Linux input event keycodes internally.

### Error Response Format

All error responses use:
```json
{ "error": { "code": "NIRI_UNAVAILABLE", "message": "Niri IPC socket not found" } }
```
With appropriate HTTP status codes (400 for bad requests, 503 for unavailable subsystems, 403 for auth failures).

### REST Endpoints

**Status:**
- `GET /api/status` — health check, server version, Tailscale identity, subsystem availability

**Media (MPRIS):**
- `GET /api/media` — current state: track, artist, album, art URL, position, duration, volume, shuffle, repeat, active player
- `GET /api/media/art` — proxied cover art image (cached, max 2MB, cleared on track change)
- `GET /api/media/players` — list active MPRIS players `[{ "id": "spotify", "name": "Spotify" }]`
- `POST /api/media/player` — select active player `{ "id": "spotify" }`
- `POST /api/media/play` — play
- `POST /api/media/pause` — pause
- `POST /api/media/next` — next track
- `POST /api/media/prev` — previous track
- `POST /api/media/stop` — stop
- `POST /api/media/seek` — seek to position `{ "position_ms": 12345 }`
- `POST /api/media/volume` — set volume `{ "volume": 0.75 }`
- `POST /api/media/shuffle` — toggle shuffle
- `POST /api/media/repeat` — cycle repeat mode (none → track → playlist)

**Input:**
- `POST /api/input/key` — single keypress or combo `{ "key": "Enter", "modifiers": ["Control"] }`
- `POST /api/input/type` — type a string `{ "text": "hello" }`

**Niri (whitelisted actions):**
- `POST /api/niri/action` — execute a whitelisted action `{ "action": "close-window", "args": {} }`
- `GET /api/niri/windows` — window list
- `GET /api/niri/workspaces` — workspace list
- `GET /api/niri/outputs` — monitor info

**App launching (from configured shortcuts only):**
- `GET /api/apps` — list configured app shortcuts
- `POST /api/apps/launch` — launch by shortcut ID `{ "id": "terminal" }`

**Settings:**
- `GET /api/settings` — current user settings
- `PUT /api/settings` — update settings

### Whitelisted Niri Actions

Window management:
- `focus-window` — `{ "args": { "id": 42 } }`
- `close-window` — no args (acts on focused window)
- `fullscreen-window` — no args
- `focus-column-left`, `focus-column-right` — no args
- `focus-window-up`, `focus-window-down` — no args
- `move-column-left`, `move-column-right` — no args
- `move-window-up`, `move-window-down` — no args

Workspace:
- `focus-workspace-up`, `focus-workspace-down` — no args
- `focus-workspace` — `{ "args": { "reference": "browser" } }` (name or index)
- `move-window-to-workspace-up`, `move-window-to-workspace-down` — no args

Monitor:
- `focus-monitor-left`, `focus-monitor-right` — no args
- `focus-monitor-up`, `focus-monitor-down` — no args
- `move-window-to-monitor-left`, `move-window-to-monitor-right` — no args

System:
- `power-off-monitors`, `power-on-monitors` — no args
- `screenshot`, `screenshot-screen`, `screenshot-window` — no args (uses niri's default screenshot behavior, saves to user's screenshot directory)

Layout:
- `maximize-column`, `toggle-window-floating` — no args
- `switch-preset-column-width` — no args

### WebSocket `/api/ws`

Auth is checked on the HTTP upgrade request (axum extractor reads `Tailscale-User-Login` header during handshake). Tailscale Serve injects headers on WebSocket upgrade requests.

**Upstream (client → server):**

Mouse move events should be batched client-side (aggregate over 16ms, send one message per frame) to avoid flooding.

```json
{ "type": "mouse_move", "dx": 5.2, "dy": -3.1 }
{ "type": "mouse_button", "button": "left", "state": "press" }
{ "type": "mouse_button", "button": "left", "state": "release" }
{ "type": "scroll", "dx": 0, "dy": -3.0 }
{ "type": "key_press", "key": "a", "modifiers": [] }
{ "type": "key_release", "key": "a" }
```

**Downstream (server → client):**
```json
{ "type": "audio_level", "left": 0.42, "right": 0.38 }
{ "type": "media_progress", "position_ms": 45000, "duration_ms": 210000 }
{ "type": "media_changed", "track": "...", "artist": "...", "album": "...", "art_url": "/api/media/art" }
{ "type": "niri_event", "event": { ... } }
```

Audio levels streamed at ~30fps. Media progress at ~1fps (or on seek). Niri events pushed as they occur.

## Deployment

### Nix Flake Structure

```
niri-remote/
├── flake.nix              # outputs: packages.niri-remote, nixosModules.default
├── flake.lock
├── server/
│   ├── Cargo.toml
│   ├── Cargo.lock
│   └── src/
│       ├── main.rs
│       ├── api/           # axum route handlers
│       ├── input/         # uinput virtual devices
│       ├── media/         # MPRIS D-Bus client
│       ├── audio/         # PipeWire audio level monitor (via PulseAudio compat)
│       ├── niri/          # niri IPC client
│       └── settings/      # persistent config
├── web/
│   ├── package.json
│   ├── svelte.config.js
│   ├── vite.config.js
│   └── src/
│       ├── routes/
│       ├── lib/
│       └── app.html
└── module.nix             # NixOS module (systemd service + tailscale serve)
```

### NixOS Module

The `module.nix` provides:
- `services.niri-remote.enable` — enable the service
- `services.niri-remote.port` — port for the HTTP server (default: 9876, binds 127.0.0.1 only)
- A systemd user service running the daemon
- `Environment=NIRI_SOCKET=%t/niri-socket` and `Environment=DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/%U/bus` in the unit file for socket discovery
- udev rule: `KERNEL=="uinput", SUBSYSTEM=="misc", MODE="0660", GROUP="uinput"` with a dedicated `uinput` group, user added to group
- Tailscale Serve configuration pointing to `127.0.0.1:<port>`

Tailscale Serve is always required — the server binds to localhost only and has no fallback auth mechanism.

### Integration with nixfiles

Add as a flake input in `~/nixfiles/flake.nix`, include the module in the host config, and enable:

```nix
services.niri-remote = {
  enable = true;
};
```

## Auth Model

Tailscale Serve terminates TLS and injects identity headers. The server:
1. Binds to `127.0.0.1` only — not reachable without Tailscale Serve
2. Reads `Tailscale-User-Login` from each request (including WebSocket upgrade)
3. Rejects requests without the header (403)

No passwords, tokens, or certificates to manage. No option to disable Tailscale Serve — it is the auth layer.

## Persistence

Settings stored in `$XDG_CONFIG_HOME/niri-remote/settings.json`:
- Trackpad sensitivity
- Theme preference
- Configured app shortcuts (name + command pairs, each with a stable ID)
- Visible action buttons
- Audio monitoring device preference

## Error Handling

- uinput device creation failure → clear error on settings tab, input tabs disabled
- Niri IPC unavailable → window/workspace tabs show "niri not running", app launching disabled
- No MPRIS players → media tab shows "no active player"
- PipeWire/PulseAudio unavailable → audio level visualizer hidden
- WebSocket disconnect → auto-reconnect with exponential backoff, status indicator in UI
- Auth failure (missing header) → 403 JSON error, frontend shows "connect via Tailscale" message
