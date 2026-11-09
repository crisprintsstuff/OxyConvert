//! Pure safe Rust client for talking to the OxyConvert Suite API without external web dependencies.

use crate::api::discovery::read_discovery;
use crate::api::types::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Client for communicating with OxyConvert from other suite applications.
#[derive(Debug, Clone)]
pub struct OxcClient {
    pub endpoint: String,
    pub host: String,
    pub port: u16,
    pub token: Option<String>,
}

impl OxcClient {
    /// Create a new client pointing to the given endpoint (e.g. `http://127.0.0.1:8990`).
    pub fn new(endpoint: &str, token: Option<String>) -> Result<Self, String> {
        let stripped = endpoint
            .trim_start_matches("http://")
            .trim_start_matches("https://");
        let (host, port_str) = stripped.split_once(':').unwrap_or((stripped, "8990"));
        let port = port_str
            .trim_end_matches('/')
            .parse::<u16>()
            .map_err(|e| format!("invalid port in endpoint '{endpoint}': {e}"))?;

        Ok(Self {
            endpoint: format!("http://{host}:{port}"),
            host: host.to_string(),
            port,
            token,
        })
    }

    /// Automatically discover an active OxyConvert instance via the suite discovery file.
    pub fn discover() -> Result<Self, String> {
        let info = read_discovery()?;
        Self::new(&info.endpoint, info.token)
    }

    /// Query OxyConvert status, capabilities, and installed plugin metrics.
    pub fn status(&self) -> Result<ApiStatusResponse, String> {
        self.get_json("/api/v1/status")
    }

    /// Detect source ecosystem from code snippet or filename.
    pub fn detect(
        &self,
        code: Option<&str>,
        filename: Option<&str>,
    ) -> Result<DetectResponse, String> {
        let req = DetectRequest {
            code: code.map(str::to_string),
            filename: filename.map(str::to_string),
            file_path: None,
        };
        self.post_json("/api/v1/detect", &req)
    }

    /// Convert a script, macro, or archive to .oxyaddon.
    pub fn convert(&self, req: ConvertRequest) -> Result<ConvertResponse, String> {
        self.post_json("/api/v1/convert", &req)
    }

    /// Inspect an existing .oxyaddon archive file or base64 package.
    pub fn inspect(&self, req: InspectRequest) -> Result<InspectResponse, String> {
        self.post_json("/api/v1/inspect", &req)
    }

    /// Install an .oxyaddon package directly into OxyBrush.
    pub fn install(&self, req: InstallRequest) -> Result<InstallResponse, String> {
        self.post_json("/api/v1/install", &req)
    }

    /// List all currently installed plugins in OxyBrush.
    pub fn list(&self) -> Result<ListResponse, String> {
        self.get_json("/api/v1/list")
    }

    /// Uninstall a plugin by ID from OxyBrush.
    pub fn uninstall(&self, plugin_id: &str) -> Result<GenericResponse, String> {
        let req = UninstallRequest {
            plugin_id: plugin_id.to_string(),
        };
        self.post_json("/api/v1/uninstall", &req)
    }

    fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let body = self.raw_http("GET", path, &[])?;
        serde_json::from_slice(&body).map_err(|e| format!("failed to parse response JSON: {e}"))
    }

    fn post_json<Req: serde::Serialize, Resp: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        payload: &Req,
    ) -> Result<Resp, String> {
        let json_bytes =
            serde_json::to_vec(payload).map_err(|e| format!("failed to serialize request: {e}"))?;
        let body = self.raw_http("POST", path, &json_bytes)?;
        serde_json::from_slice(&body).map_err(|e| format!("failed to parse response JSON: {e}"))
    }

    fn raw_http(&self, method: &str, path: &str, body: &[u8]) -> Result<Vec<u8>, String> {
        let addr = format!("{}:{}", self.host, self.port);
        let mut stream = TcpStream::connect(&addr)
            .map_err(|e| format!("could not connect to OxyConvert API at {addr}: {e}"))?;
        stream.set_read_timeout(Some(Duration::from_secs(15))).ok();
        stream.set_write_timeout(Some(Duration::from_secs(15))).ok();

        let mut request = format!(
            "{method} {path} HTTP/1.1\r\n\
            Host: {addr}\r\n\
            User-Agent: OxySuite/1.0\r\n\
            Connection: close\r\n\
            Content-Type: application/json\r\n\
            Content-Length: {}\r\n",
            body.len()
        );

        if let Some(tok) = &self.token {
            request.push_str(&format!("Authorization: Bearer {tok}\r\n"));
        }
        request.push_str("\r\n");

        stream
            .write_all(request.as_bytes())
            .map_err(|e| e.to_string())?;
        if !body.is_empty() {
            stream.write_all(body).map_err(|e| e.to_string())?;
        }
        stream.flush().map_err(|e| e.to_string())?;

        let mut reader = BufReader::new(stream);
        let mut status_line = String::new();
        if reader
            .read_line(&mut status_line)
            .map_err(|e| e.to_string())?
            == 0
        {
            return Err("server closed connection prematurely".into());
        }

        let parts: Vec<&str> = status_line.split_whitespace().collect();
        if parts.len() < 2 {
            return Err(format!("malformed status line: {status_line}"));
        }
        let status_code: u16 = parts[1].parse().unwrap_or(500);

        let mut content_length = 0usize;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                break;
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }
            if let Some((k, v)) = trimmed.split_once(':')
                && k.trim().eq_ignore_ascii_case("content-length")
            {
                content_length = v.trim().parse().unwrap_or(0);
            }
        }

        let mut resp_body = vec![0u8; content_length];
        if content_length > 0 {
            reader
                .read_exact(&mut resp_body)
                .map_err(|e| e.to_string())?;
        } else {
            reader
                .read_to_end(&mut resp_body)
                .map_err(|e| e.to_string())?;
        }

        if status_code >= 400 {
            let err_text = String::from_utf8_lossy(&resp_body);
            return Err(format!("HTTP {status_code} error: {err_text}"));
        }

        Ok(resp_body)
    }
}
