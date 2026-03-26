# telemax

Remote control for [Niri](https://github.com/YaLTeR/niri) Linux desktops, accessed from a phone over [Tailscale](https://tailscale.com).

Rust server + Svelte web client. Swiss typographic UI.

## Features

- **Trackpad** — touch-to-mouse with tap/scroll gestures, mobile keyboard input
- **Media** — MPRIS playback control with real-time FFT spectrum visualizer
- **Windows** — spatial overview matching niri's exploded view, tap to focus
- **Niri IPC** — window management, workspace switching, screenshots, monitor control

## Architecture

- **Server** (`server/`) — Rust (axum) serving HTTP over a Unix socket. Integrates with uinput, D-Bus MPRIS, PulseAudio, and niri IPC.
- **Web** (`web/`) — SvelteKit SPA with WebSocket for real-time state. Designed for mobile.
- **Native host** (`server/src/bin/plasma_integration_host.rs`) — MPRIS bridge for the [KDE Plasma Browser Integration](https://addons.mozilla.org/en-US/firefox/addon/plasma-integration/) extension.

## Auth

Pairing code flow — phone generates a code, user approves it locally via a Unix socket. Session tokens stored as SHA-256 hashes.

## NixOS

```nix
# flake.nix inputs
telemax.url = "github:max-niederman/telemax";

# configuration.nix
services.telemax.enable = true;
```

Exposed via Tailscale Serve at `/telemax`.
