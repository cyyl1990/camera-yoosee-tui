
//! ONVIF SOAP client for PTZ control and device management.

use std::time::Duration;
use anyhow::{Context, Result};
use reqwest::Client;
use tracing::{debug, error};
use serde::Deserialize;

/// ONVIF client for camera control.
#[allow(dead_code)]
pub struct OnvifClient {
    /// HTTP client
    client: Client,
    /// Camera IP/hostname
    host: String,
    /// ONVIF port
    port: u16,
    /// Username for auth
    username: String,
    /// Password for auth
    password: String,
    /// Base endpoint URL
    endpoint: String,
}

impl OnvifClient {
    /// Create a new ONVIF client.
    pub fn new(host: &str, port: u16, username: &str, password: &str) -> Self {
        let endpoint = format!("http://{}:{}/onvif/media_service", host, port);
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            host: host.to_string(),
            port,
            username: username.to_string(),
            password: password.to_string(),
            endpoint,
        }
    }

    /// Get device information.
    pub async fn get_device_info(&self) -> Result<DeviceInfoResponse> {
        let body = self.soap_request("GetDeviceInformation", "")?;
        self.send_request::<DeviceInfoResponse>(&body).await
    }

    /// Get media profiles.
    pub async fn get_profiles(&self) -> Result<Vec<Profile>> {
        let body = self.soap_request("GetProfiles", "")?;
        let resp: ProfilesResponse = self.send_request(&body).await?;
        Ok(resp.profiles)
    }

    /// Get stream URI for a profile.
    pub async fn get_stream_uri(&self, profile_token: &str) -> Result<String> {
        let body = self.soap_request("GetStreamUri", &format!(
            r#"<trt:ProfileToken>{}</trt:ProfileToken>"#, profile_token
        ))?;
        let resp: StreamUriResponse = self.send_request(&body).await?;
        Ok(resp.media_uri.uri)
    }

    /// PTZ: Continuous move (pan/tilt/zoom).
    pub async fn ptz_continuous_move(
        &self,
        profile_token: &str,
        pan: f64,
        tilt: f64,
        zoom: f64,
    ) -> Result<()> {
        let body = self.soap_request("ContinuousMove", &format!(
            r#"<tptz:ProfileToken>{}</tptz:ProfileToken>
            <tptz:Velocity>
                <tt:PanTilt x="{}" y="{}"/>
                <tt:Zoom x="{}"/>
            </tptz:Velocity>"#,
            profile_token, pan, tilt, zoom
        ))?;
        let _resp: () = self.send_request(&body).await?;
        Ok(())
    }

    /// PTZ: Stop all motion.
    pub async fn ptz_stop(&self, profile_token: &str) -> Result<()> {
        let body = self.soap_request("Stop", &format!(
            r#"<tptz:ProfileToken>{}</tptz:ProfileToken>
            <tptz:PanTilt>true</tptz:PanTilt>
            <tptz:Zoom>true</tptz:Zoom>"#,
            profile_token
        ))?;
        let _resp: () = self.send_request(&body).await?;
        Ok(())
    }

    /// PTZ: Go to preset position.
    pub async fn ptz_goto_preset(&self, profile_token: &str, preset_token: &str) -> Result<()> {
        let body = self.soap_request("GotoPreset", &format!(
            r#"<tptz:ProfileToken>{}</tptz:ProfileToken>
            <tptz:PresetToken>{}</tptz:PresetToken>"#,
            profile_token, preset_token
        ))?;
        let _resp: () = self.send_request(&body).await?;
        Ok(())
    }

    /// Build SOAP request envelope.
    fn soap_request(&self, action: &str, body_extra: &str) -> Result<String> {
        Ok(format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<soap-env:Envelope xmlns:soap-env="http://www.w3.org/2003/05/soap-envelope"
    xmlns:wsdl="http://www.onvif.org/ver10/device/wsdl"
    xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
    xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
    xmlns:tptz="http://www.onvif.org/ver10/ptz/wsdl"
    xmlns:tt="http://www.onvif.org/ver10/schema">
    <soap-env:Header/>
    <soap-env:Body>
        <wsdl:{action}>
        {body_extra}
        </wsdl:{action}>
    </soap-env:Body>
</soap-env:Envelope>"#,
            action = action,
            body_extra = body_extra
        ))
    }

    /// Send a SOAP request and parse response.
    async fn send_request<T: for<'de> Deserialize<'de>>(&self, body: &str) -> Result<T> {
        debug!("ONVIF request to {}", self.endpoint);

        let resp = self.client
            .post(&self.endpoint)
            .header("Content-Type", "application/soap+xml; charset=utf-8")
            .basic_auth(&self.username, Some(&self.password))
            .body(body.to_string())
            .send()
            .await
            .context("ONVIF request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            error!("ONVIF error {}: {}", status, body_text);
            anyhow::bail!("ONVIF request failed: {}", status);
        }

        let text = resp.text().await?;
        debug!("ONVIF response: {} bytes", text.len());

        serde_xml_rs::from_str(&text)
            .context("Failed to parse ONVIF response")
    }
    /// Get a JPEG snapshot from the camera.
    /// Tries multiple known Yoosee/ONVIF snapshot endpoints.
    pub async fn get_snapshot_jpeg(&self) -> Result<Vec<u8>> {
        // Try ONVIF GetSnapshotUri first
        let media_url = format!("http://{}:{}/onvif/media_service", self.host, self.port);
        let profile_token = "IPCProfilesToken0";  // Common Yoosee default

        // Try Media2 GetSnapshotUri
        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
    xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
    xmlns:tt="http://www.onvif.org/ver10/schema">
  <soap:Header/>
  <soap:Body>
    <trt:GetSnapshotUri>
      <trt:ProfileToken>{profile_token}</trt:ProfileToken>
    </trt:GetSnapshotUri>
  </soap:Body>
</soap:Envelope>"#,
            profile_token = profile_token
        );

        let resp = self.client
            .post(&media_url)
            .header("Content-Type", "application/soap+xml; charset=utf-8")
            .basic_auth(&self.username, Some(&self.password))
            .body(body)
            .send()
            .await
            .context("Snapshot SOAP request failed")?;

        if resp.status().is_success() {
            let text = resp.text().await?;
            debug!("Snapshot URI response: {}", text);
            // Parse URI from <Uri>...</Uri>
            if let Some(start) = text.find("<Uri>") {
                if let Some(end) = text[start..].find("</Uri>") {
                    let uri = &text[start + 5..start + end];
                    debug!("Snapshot URI: {}", uri);
                    // Fetch the JPEG
                    return self.fetch_jpeg(uri).await;
                }
            }
        }

        // Fallback: try common Yoosee HTTP snapshot paths
        let paths = [
            "/onvif/snapshot", "/snapshot.jpg", "/cgi-bin/snapshot.cgi",
            "/web/tmpfs/snap.jpg", "/ISAPI/Streaming/channels/1/picture",
            "/Streaming/channels/1/picture", "/Streaming/channels/2/picture",
        ];

        for path in paths {
            let url = format!("http://{}:{}{}", self.host, self.port, path);
            match self.fetch_jpeg(&url).await {
                Ok(data) if data.len() > 100 => {
                    debug!("Got snapshot from {}", url);
                    return Ok(data);
                }
                _ => continue,
            }
        }

        anyhow::bail!("No snapshot endpoint worked for camera at {}", self.host)
    }

    /// Fetch a JPEG image from a URL with auth.
    async fn fetch_jpeg(&self, url: &str) -> Result<Vec<u8>> {
        let resp = self.client
            .get(url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("HTTP {}", resp.status());
        }

        let bytes = resp.bytes().await?;
        Ok(bytes.to_vec())
    }

    /// Get stream URI via ONVIF (for proper cameras).
    pub async fn get_stream_uri_via_onvif(&self, profile_token: &str) -> Result<String> {
        let media_url = format!("http://{}:{}/onvif/media_service", self.host, self.port);
        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
    xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
    xmlns:tt="http://www.onvif.org/ver10/schema">
  <soap:Header/>
  <soap:Body>
    <trt:GetStreamUri>
      <trt:StreamSetup>
        <tt:Stream>RTP-Unicast</tt:Stream>
        <tt:Transport><tt:Protocol>RTSP</tt:Protocol></tt:Transport>
      </trt:StreamSetup>
      <trt:ProfileToken>{profile_token}</trt:ProfileToken>
    </trt:GetStreamUri>
  </soap:Body>
</soap:Envelope>"#,
            profile_token = profile_token
        );

        let resp = self.client
            .post(&media_url)
            .header("Content-Type", "application/soap+xml; charset=utf-8")
            .basic_auth(&self.username, Some(&self.password))
            .body(body)
            .send()
            .await?;

        let text = resp.text().await?;
        if let Some(start) = text.find("<Uri>") {
            if let Some(end) = text[start..].find("</Uri>") {
                return Ok(text[start + 5..start + end].to_string());
            }
        }

        anyhow::bail!("No stream URI in response")
    }


}

// Response structures
#[derive(Debug, Deserialize)]
pub struct DeviceInfoResponse {
    #[serde(rename = "Manufacturer")]
    pub manufacturer: Option<String>,
    #[serde(rename = "Model")]
    pub model: Option<String>,
    #[serde(rename = "FirmwareVersion")]
    pub firmware_version: Option<String>,
    #[serde(rename = "SerialNumber")]
    pub serial_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProfilesResponse {
    #[serde(rename = "Profiles")]
    pub profiles: Vec<Profile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Profile {
    #[serde(rename = "token")]
    pub token: String,
    #[serde(rename = "Name")]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StreamUriResponse {
    #[serde(rename = "MediaUri")]
    pub media_uri: MediaUri,
}

#[derive(Debug, Deserialize)]
pub struct MediaUri {
    #[serde(rename = "Uri")]
    pub uri: String,
}
