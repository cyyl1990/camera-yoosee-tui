//! Camera struct and connection state management.

use std::net::IpAddr;
use serde::{Deserialize, Serialize};

/// Represents a discovered or configured camera.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Camera {
    /// Unique identifier (from Yoosee UID or generated)
    pub uid: String,
    /// Display name
    pub name: String,
    /// IP address
    pub host: IpAddr,
    /// RTSP port (default: 554)
    pub rtsp_port: u16,
    /// ONVIF port (default: 5000)
    pub onvif_port: u16,
    /// RTSP username
    pub username: String,
    /// RTSP password (SecretString for security)
    #[serde(skip_serializing)]
    pub password: String,
    /// Camera status
    pub status: CameraStatus,
}

impl Camera {
    /// Build the RTSP URL for main stream
    pub fn rtsp_url(&self) -> String {
        format!(
            "rtsp://{}:{}@{}:{}/onvif1",
            self.username, self.password, self.host, self.rtsp_port
        )
    }

    /// Build the RTSP URL for sub stream (lower quality)
    pub fn rtsp_url_sub(&self) -> String {
        format!(
            "rtsp://{}:{}@{}:{}/onvif2",
            self.username, self.password, self.host, self.rtsp_port
        )
    }

    /// Build ONVIF endpoint URL
    pub fn onvif_url(&self) -> String {
        format!("http://{}:{}/onvif_device_service", self.host, self.onvif_port)
    }
}

/// Camera connection status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CameraStatus {
    /// Not yet connected
    Disconnected,
    /// Currently connecting
    Connecting,
    /// Connected and streaming
    Connected,
    /// Temporary error, will retry
    Error,
    /// Authentication failed
    AuthFailed,
}

impl Default for CameraStatus {
    fn default() -> Self {
        Self::Disconnected
    }
}
