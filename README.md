# Yoosee TUI

A lightweight, fast TUI application to view and control Yoosee IP cameras on Linux.

> No Wine. No Electron. No cloud dependency. Just RTSP + ONVIF.

## Features

- **Multi-camera grid view** — 1, 2, or 4 camera layouts
- **RTSP streaming** — via FFmpeg (with adaptive transport: TCP/UDP fallback)
- **PTZ control** — pan, tilt, zoom via ONVIF SOAP
- **LAN auto-discovery** — UDP broadcast on port 32108 + ONVIF WS-Discovery
- **Snapshot + clip recording** — hotkey-driven, saved locally
- **Omarchy OS integration** — theme-aware, native notifications
- **Motion detection overlay** — FFmpeg scene detection

## Quick Start

```bash
# Build
cargo build --release

# First run — discover cameras automatically
cargo run --release

# Or with a config file
cargo run --release -- --config ~/my-cameras.toml
```

## Configuration

Edit ``~/.config/yoosee-tui/cameras.toml``:

```toml
[[cameras]]
name = "Living Room"
host = "192.168.1.100"
rtsp_port = 554
onvif_port = 5000
username = "admin"
# Password will be prompted if not in keyring

[[cameras]]
name = "Front Door"
host = "192.168.1.101"
# ... more cameras
```

## Keybindings

| Key | Action |
|-----|--------|
| `1-4` | Switch camera focus |
| `G` | Toggle grid layout |
| `F` | Fullscreen current camera |
| `↑↓←→` | PTZ pan/tilt |
| `+/−` | Zoom in/out |
| `S` | Take snapshot |
| `R` | Start/stop recording |
| `Q` | Quit |

## Architecture

```
yoosee-tui/
├── src/
│   ├── lib/              # Core library (camera, rtsp, onvif, discovery)
│   │   ├── camera.rs     # Camera struct + connection state
│   │   ├── rtsp.rs       # RTSP session + FFmpeg bridge
│   │   ├── onvif.rs      # ONVIF SOAP client (PTZ, discovery)
│   │   └── discovery.rs   # LAN broadcast discovery
│   ├── bin/main.rs       # Entry point + ratatui TUI
│   └── config/           # TOML config parsing
├── configs/              # Sample config files
├── scripts/              # Helper scripts (password generator, etc.)
└── docs/                # Protocol notes
```

## Requirements

- Rust 1.85+
- FFmpeg (libavcodec) installed
- Terminal with true color support (kitty, WezTerm, Alacritty, Ghostty)
- Omarchy OS recommended (for theme integration)

## Protocol Notes

Yoosee cameras expose:
- **RTSP** on port 554: ``rtsp://admin:{MD5(device_id)}@IP:554/onvif1`` (main) or ``onvif2`` (sub)
- **ONVIF** on port 5000: SOAP over HTTP for PTZ and device info
- **LAN Discovery** on UDP port 32108: broadcast ``MSG_LAN_SEARCH``

> **Note:** The RTSP password for Yoosee cameras can be computed as ``MD5(device_id)``. See ``scripts/rtsp_password_gen.py``.

## License

MIT
