
//! LAN camera discovery via UDP broadcast and ONVIF WS-Discovery.

use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::Duration;
use anyhow::{Context, Result};
use tracing::{info, debug};

/// Discovery service for finding cameras on the local network.
pub struct DiscoveryService {
    /// Timeout for discovery probes.
    timeout: Duration,
}

impl DiscoveryService {
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(3),
        }
    }

    /// Discover cameras using Yoosee UDP broadcast on port 32108.
    ///
    /// Sends MSG_LAN_SEARCH broadcast and waits for MSG_PUNCH_PKT responses.
    pub fn discover_yoosee(&self) -> Result<Vec<DiscoveredCamera>> {
        let socket = UdpSocket::bind("0.0.0.0:0")
            .context("Failed to bind UDP socket for discovery")?;
        socket.set_read_timeout(Some(self.timeout))?;
        socket.set_broadcast(true)?;

        // Yoosee MSG_LAN_SEARCH magic bytes
        let probe = Self::yoosee_lan_search();
        socket.send_to(&probe, "255.255.255.255:32108")?;

        let mut results = Vec::new();
        let mut buf = [0u8; 1024];

        loop {
            match socket.recv_from(&mut buf) {
                Ok((len, addr)) => {
                    debug!("Received {} bytes from {}", len, addr);
                    if let Some(cam) = self.parse_yoosee_reply(&buf[..len], addr) {
                        results.push(cam);
                    }
                }
                Err(_) => break, // Timeout
            }
        }

        info!("Discovered {} Yoosee cameras", results.len());
        Ok(results)
    }

    /// Build Yoosee MSG_LAN_SEARCH packet.
    fn yoosee_lan_search() -> Vec<u8> {
        // Magic header for MSG_LAN_SEARCH (0x14) + padding
        let mut packet = vec![0x14, 0x00, 0x00, 0x00];
        // Add padding to make it look like a real probe
        packet.extend_from_slice(&[0x00; 28]);
        packet
    }

    /// Parse a Yoosee reply packet.
    fn parse_yoosee_reply(&self, data: &[u8], addr: SocketAddr) -> Option<DiscoveredCamera> {
        if data.len() < 8 {
            return None;
        }

        // Check for MSG_PUNCH_PKT (0x16) or MSG_P2P_RDY (0x1a)
        let msg_type = data[0];
        if msg_type != 0x16 && msg_type != 0x1a {
            return None;
        }

        // Extract UID (usually starts at offset 4-20 for Yoosee PPPP protocol)
        let uid = Self::extract_uid(data).unwrap_or_else(|| addr.ip().to_string());

        Some(DiscoveredCamera {
            uid,
            host: addr.ip(),
            port: 554, // Default, will probe ONVIF for actual ports
            name: format!("Camera @ {}", addr.ip()),
            protocol: DiscoveryProtocol::YooseePPPP,
        })
    }

    /// Extract UID from PPPP packet.
    fn extract_uid(data: &[u8]) -> Option<String> {
        // PPPP UID is typically 20 bytes starting at offset 4
        // Try to extract printable ASCII
        if data.len() >= 24 {
            let uid_bytes = &data[4..24];
            let uid: String = uid_bytes.iter()
                .take_while(|&&b| b >= 0x20 && b < 0x7f)
                .map(|&b| b as char)
                .collect();
            if !uid.is_empty() && uid.len() >= 8 {
                return Some(uid);
            }
        }
        None
    }

    /// Discover cameras using ONVIF WS-Discovery probe.
    pub fn discover_onvif(&self, network: &str) -> Result<Vec<DiscoveredCamera>> {
        let socket = UdpSocket::bind("0.0.0.0:0")
            .context("Failed to bind UDP socket for ONVIF discovery")?;
        socket.set_read_timeout(Some(self.timeout))?;
        socket.set_broadcast(true)?;

        let probe = Self::onvif_ws_discovery_probe();
        socket.send_to(&probe, &format!("{}:3702", network))?;

        let mut results = Vec::new();
        let mut buf = [0u8; 4096];

        loop {
            match socket.recv_from(&mut buf) {
                Ok((len, _addr)) => {
                    if let Some(cam) = self.parse_onvif_reply(&buf[..len]) {
                        results.push(cam);
                    }
                }
                Err(_) => break,
            }
        }

        info!("Discovered {} ONVIF cameras", results.len());
        Ok(results)
    }

    /// Build ONVIF WS-Discovery Probe message.
    fn onvif_ws_discovery_probe() -> Vec<u8> {
        let body = r#"<?xml version="1.0" encoding="UTF-8"?>
<soap-env:Envelope xmlns:soap-env="http://www.w3.org/2003/05/soap-envelope"
    xmlns:dn="http://www.onvif.org/ver10/network/wsdl">
  <soap-env:Header/>
  <soap-env:Body>
    <dn:Probe/>
  </soap-env:Body>
</soap-env:Envelope>"#;
        body.as_bytes().to_vec()
    }

    /// Parse ONVIF discovery reply.
    fn parse_onvif_reply(&self, data: &[u8]) -> Option<DiscoveredCamera> {
        let reply = String::from_utf8_lossy(data);
        
        // Extract XAddrs from reply
        if let Some(xaddrs_start) = reply.find("XAddrs>") {
            let remainder = &reply[xaddrs_start + 7..];
            let xaddrs_end = remainder.find('<')?;
            let xaddrs = &remainder[..xaddrs_end];
            
            // Parse http://IP:PORT/...
            if let Some(ip_start) = xaddrs.find("://") {
                let after_scheme = &xaddrs[ip_start + 3..];
                let host_port = after_scheme.split('/').next().unwrap_or(after_scheme);
                let (host, port_str) = host_port.split_once(':').unwrap_or((host_port, "80"));
                let port: u16 = port_str.parse().unwrap_or(80);

                return Some(DiscoveredCamera {
                    uid: host.to_string(),
                    host: host.parse().ok()?,
                    port,
                    name: format!("ONVIF @ {}", host),
                    protocol: DiscoveryProtocol::OnvifWS,
                });
            }
        }
        None
    }
}

impl Default for DiscoveryService {
    fn default() -> Self {
        Self::new()
    }
}

/// A camera discovered on the network.
#[derive(Debug, Clone)]
pub struct DiscoveredCamera {
    pub uid: String,
    pub host: IpAddr,
    pub port: u16,
    pub name: String,
    pub protocol: DiscoveryProtocol,
}

#[derive(Debug, Clone, Copy)]
pub enum DiscoveryProtocol {
    YooseePPPP,
    OnvifWS,
}
