//! # Yoosee TUI — Core Library
//!
//! Core modules for camera discovery, RTSP streaming, and ONVIF control.

pub mod camera;
pub mod config;
pub mod discovery;
pub mod onvif;
pub mod rtsp;

pub use camera::{Camera, CameraStatus};
pub use config::Config;
pub use discovery::DiscoveryService;
pub use onvif::OnvifClient;
pub use rtsp::RtspSession;
