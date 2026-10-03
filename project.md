# Camera Yoosee TUI — Project Spec

## 1. Project Overview

**Type:** Desktop TUI utility (Linux-native)
**Domain:** IP camera surveillance, home monitoring
**Tech:** Rust + Ratatui + FFmpeg + ONVIF

## 2. Tech Stack

| Layer | Technology | Notes |
|-------|------------|-------|
| TUI | ratatui 0.29 | Declarative, no runtime panics |
| Video | FFmpeg (subprocess) | RTSP decode + render |
| Async | tokio 1.46 | Multi-camera concurrency |
| HTTP | reqwest + rustls | ONVIF SOAP calls |
| Config | toml + serde | TOML config files |
| Log | tracing + tracing-subscriber | Structured logging |

## 3. Constraints

- **Blacklist:** Không chạm vào code của app Yoosee gốc, không reverse-engineer P2P cloud
- **Target:** 1-4 camera, local LAN only, 2.4GHz WiFi cameras
- **Platform:** Omarchy OS (Arch-based), Hyprland, 64-bit Linux
- **No GUI deps:** Không dùng Electron, GTK, Qt

## 4. Core Components

### 4.1 Discovery (``discovery.rs``)
- UDP broadcast ``MSG_LAN_SEARCH`` on port 32108
- ONVIF WS-Discovery probe on port 3702
- Parse ``MSG_PUNCH_PKT`` replies → extract UID + local IP

### 4.2 RTSP Session (``rtsp.rs``)
- FFmpeg subprocess bridge: spawn ``ffmpeg -rtsp_transport tcp -i rtsp://... -f rawvideo -pix_fmt bgr24 pipe:1``
- Adaptive transport: try TCP first, fallback UDP
- Frame buffer (ring buffer, ~30 frames)
- Support for ``/onvif1`` (main) and ``/onvif2`` (sub) paths

### 4.3 ONVIF Client (``onvif.rs``)
- SOAP POST to ``http://IP:5000/onvif_device_service``
- PTZ operations: ``ContinuousMove``, ``Stop``, ``GotoPreset``
- Device info: ``GetDeviceInformation``, ``GetProfiles``, ``GetStreamUri``
- No WS-Security for Yoosee (plain auth)

### 4.4 Camera State (``camera.rs``)
```rust
struct Camera {
    uid: String,
    name: String,
    host: IpAddr,
    rtsp_port: u16,
    onvif_port: u16,
    username: String,
    password: SecretString,
    status: CameraStatus,
    stream_url: Option<String>,
}
```

### 4.5 TUI (``bin/main.rs``)
- Ratatui layout: header (title + status) + grid (1-4 cameras) + footer (keybinds)
- Input: crossterm events → camera selection, PTZ, snapshots
- Render: frame buffer → ANSI 256 / true color cells

## 5. Planned Phases

| Phase | Description | Priority |
|-------|-------------|----------|
| 0 | Project setup + skeleton | Done |
| 1 | RTSP playback (1 camera, fullscreen) | Next |
| 2 | Multi-camera grid (1-4 layout) | P2 |
| 3 | LAN discovery + ONVIF | P2 |
| 4 | PTZ control | P3 |
| 5 | Snapshot + clip recording | P3 |
| 6 | Omarchy theme + notifications | P4 |
| 7 | Motion detection overlay | P5 |

## 6. Dependencies

See ``Cargo.toml`` for full list. Key external libs:
- ``ffmpeg`` system package (libavcodec, libavformat, libavutil)
- ``openssl`` or ``rustls`` for HTTPS (reqwest default)
