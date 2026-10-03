
//! RTSP session management via FFmpeg subprocess bridge.

use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use anyhow::{Context, Result};
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tracing::{info, debug, error, warn};

/// RTSP transport mode.
#[derive(Debug, Clone, Copy, Default)]
pub enum Transport {
    /// TCP transport (recommended)
    Tcp,
    /// UDP transport
    Udp,
    /// Auto-detect
    #[default]
    Auto,
}

/// RTSP frame with raw pixel data.
#[derive(Debug, Clone)]
pub struct RtspFrame {
    pub pts: u64,
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// RTSP streaming session.
pub struct RtspSession {
    camera_id: String,
    /// FFmpeg process handle
    ffmpeg: Arc<RwLock<Option<Child>>>,
    transport: Transport,
    frame_rx: mpsc::Receiver<RtspFrame>,
    is_streaming: bool,
    /// Handle to the reader task for cleanup
    _reader_handle: Option<JoinHandle<()>>,
}

impl RtspSession {
    pub fn new(camera_id: &str, transport: Transport) -> Result<Self> {
        let (frame_tx, frame_rx) = mpsc::channel(32);

        let ffmpeg = Arc::new(RwLock::new(None));

        // Spawn a persistent reader task that reads from a shared process handle
        let cam_id = camera_id.to_string();
        let ffmpeg_ref = Arc::clone(&ffmpeg);
        let tx = Arc::new(frame_tx);

        let handle = tokio::spawn(async move {
            Self::reader_task(&cam_id, ffmpeg_ref, tx).await;
        });

        let session = Self {
            camera_id: camera_id.to_string(),
            ffmpeg,
            transport,
            frame_rx,
            is_streaming: false,
            _reader_handle: Some(handle),
        };

        info!("Created RTSP session for camera {}", camera_id);
        Ok(session)
    }

    /// Background reader task — waits for FFmpeg process to be set, then reads frames.
    async fn reader_task(
        camera_id: &str,
        ffmpeg: Arc<RwLock<Option<Child>>>,
        tx: Arc<mpsc::Sender<RtspFrame>>,
    ) {
        const WIDTH: u32 = 640;
        const HEIGHT: u32 = 480;
        const BYTES_PER_FRAME: usize = (WIDTH * HEIGHT * 3) as usize;

        loop {
            // Wait for a process to be assigned
            loop {
                let has_child = {
                    let guard = ffmpeg.read().await;
                    guard.is_some()
                };
                if has_child {
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }

            // Get the stdout handle
            let mut child_guard = ffmpeg.write().await;
            if let Some(ref mut child) = *child_guard {
                if let Some(stdout) = child.stdout.take() {
                    drop(child_guard);

                    // Read using spawn_blocking (sync read)
                    let bytes_read = tokio::task::spawn_blocking(move || {
                        let mut buf = vec![0u8; BYTES_PER_FRAME];
                        let mut stdin = stdout;
                        std::io::Read::read(&mut stdin, &mut buf)
                    }).await;

                    match bytes_read {
                        Ok(Ok(n)) if n == BYTES_PER_FRAME => {
                            let frame = RtspFrame {
                                pts: 0,
                                data: vec![0u8; BYTES_PER_FRAME],
                                width: WIDTH,
                                height: HEIGHT,
                            };
                            let _ = tx.send(frame).await;
                        }
                        Ok(Ok(0)) => {
                            debug!("FFmpeg EOF for camera {}", camera_id);
                        }
                        Ok(Ok(n)) => {
                            warn!("Short read ({} bytes) for {}", n, camera_id);
                        }
                        Ok(Err(e)) => {
                            error!("FFmpeg read error for {}: {}", camera_id, e);
                        }
                        Err(e) => {
                            error!("Reader task panicked for {}: {}", camera_id, e);
                        }
                    }

                    // Re-check process status
                    let mut guard = ffmpeg.write().await;
                    if let Some(ref mut child) = *guard {
                        let _ = child.try_wait();
                    }
                }
            }
        }
    }
    /// Start FFmpeg subprocess and begin streaming.
    pub async fn start(&mut self, rtsp_url: &str) -> Result<()> {
        if self.is_streaming {
            info!("RTSP session already streaming");
            return Ok(());
        }

        self.kill_process().await;

        let transport_arg = match self.transport {
            Transport::Tcp => "tcp",
            Transport::Udp => "udp",
            Transport::Auto => "tcp",
        };

        let mut cmd = Command::new("ffmpeg");
        cmd.args([
            "-hide_banner", "-loglevel", "quiet",
            "-rtsp_transport", transport_arg,
            "-i", rtsp_url,
            "-f", "rawvideo", "-pix_fmt", "bgr24",
            "-an", "-vsync", "cfr", "-reorder_queue_size", "5",
            "pipe:1",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

        let child = cmd.spawn()
            .context("Failed to spawn FFmpeg. Is ffmpeg installed?")?;

        *self.ffmpeg.write().await = Some(child);
        self.is_streaming = true;

        info!("Started RTSP stream for camera {}", self.camera_id);
        Ok(())
    }

    async fn kill_process(&self) {
        let mut guard = self.ffmpeg.write().await;
        if let Some(ref mut child) = *guard {
            let _ = child.kill();
            let _ = child.wait();
        }
        *guard = None;
    }

    pub async fn recv_frame(&mut self) -> Option<RtspFrame> {
        self.frame_rx.recv().await
    }

    pub async fn stop(&mut self) {
        self.kill_process().await;
        self.is_streaming = false;
        info!("Stopped RTSP stream for camera {}", self.camera_id);
    }

    pub fn is_streaming(&self) -> bool {
        self.is_streaming
    }

    pub fn transport(&self) -> Transport {
        self.transport
    }

    pub async fn reconnect(&mut self, rtsp_url: &str) -> Result<()> {
        self.stop().await;
        self.transport = match self.transport {
            Transport::Tcp => Transport::Udp,
            Transport::Udp => Transport::Tcp,
            Transport::Auto => Transport::Tcp,
        };
        self.start(rtsp_url).await
    }
}

impl Drop for RtspSession {
    fn drop(&mut self) {}
}
