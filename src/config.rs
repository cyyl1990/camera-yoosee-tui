//! Configuration loading from TOML files.

use std::path::PathBuf;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Root configuration structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// List of camera definitions.
    pub cameras: Vec<CameraConfig>,
    /// Global settings.
    #[serde(default)]
    pub settings: Settings,
}

/// Individual camera configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfig {
    /// Display name for this camera.
    pub name: String,
    /// IP address or hostname.
    pub host: String,
    /// RTSP port (default: 554).
    #[serde(default = "default_rtsp_port")]
    pub rtsp_port: u16,
    /// ONVIF port (default: 5000).
    #[serde(default = "default_onvif_port")]
    pub onvif_port: u16,
    /// RTSP username.
    #[serde(default = "default_username")]
    pub username: String,
    /// RTSP password (optional, will be prompted if missing).
    pub password: Option<String>,
}

fn default_rtsp_port() -> u16 { 554 }
fn default_onvif_port() -> u16 { 5000 }
fn default_username() -> String { "admin".to_string() }

/// Global settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Default layout: "single" | "dual" | "grid".
    #[serde(default = "default_layout")]
    pub default_layout: String,
    /// Snapshot save directory.
    #[serde(default = "default_snapshot_dir")]
    pub snapshot_dir: PathBuf,
    /// Recording save directory.
    #[serde(default = "default_record_dir")]
    pub record_dir: PathBuf,
    /// RTSP transport: "tcp" | "udp" | "auto".
    #[serde(default = "default_transport")]
    pub rtsp_transport: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_layout: "grid".to_string(),
            snapshot_dir: PathBuf::from("/tmp/yoosee-snapshots"),
            record_dir: PathBuf::from("/tmp/yoosee-recordings"),
            rtsp_transport: "auto".to_string(),
        }
    }
}

fn default_layout() -> String { "grid".to_string() }
fn default_snapshot_dir() -> PathBuf { PathBuf::from("/tmp/yoosee-snapshots") }
fn default_record_dir() -> PathBuf { PathBuf::from("/tmp/yoosee-recordings") }
fn default_transport() -> String { "auto".to_string() }

impl Config {
    /// Load config from a TOML file.
    pub fn load(path: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config: {}", path.display()))?;
        toml::from_str(&content)
            .with_context(|| format!("Failed to parse config: {}", path.display()))
    }

    /// Get default config path: ~/.config/yoosee-tui/cameras.toml
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("yoosee-tui")
            .join("cameras.toml")
    }
}
