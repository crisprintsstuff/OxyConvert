//! API service discovery file management across the OxyLabs suite.

use crate::api::types::DiscoveryInfo;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Returns the standard path to the OxyLabs discovery file.
pub fn discovery_file_path() -> PathBuf {
    if let Some(mut dir) = dirs::data_local_dir() {
        dir.push("OxyLabs");
        dir.push("oxyconvert-api.json");
        dir
    } else {
        PathBuf::from(".oxyconvert-api.json")
    }
}

/// Write discovery file to disk so other suite apps (OxyLauncher, OxyBrush, etc.) can discover this instance.
pub fn write_discovery(endpoint: &str, port: u16, token: Option<&str>) -> Result<PathBuf, String> {
    let path = discovery_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create dir {}: {e}", parent.display()))?;
    }

    let started_at_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let info = DiscoveryInfo {
        app: "OxyConvert".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        endpoint: endpoint.to_string(),
        port,
        token: token.map(|s| s.to_string()),
        pid: std::process::id(),
        started_at_secs,
    };

    let json = serde_json::to_string_pretty(&info)
        .map_err(|e| format!("failed to serialize discovery info: {e}"))?;

    fs::write(&path, json)
        .map_err(|e| format!("failed to write discovery file {}: {e}", path.display()))?;
    log::info!("Wrote OxyConvert API discovery file to {}", path.display());
    Ok(path)
}

/// Read the current discovery file if an active OxyConvert instance is running.
pub fn read_discovery() -> Result<DiscoveryInfo, String> {
    let path = discovery_file_path();
    if !path.exists() {
        return Err(format!(
            "no active OxyConvert discovery file found at {}",
            path.display()
        ));
    }

    let data = fs::read_to_string(&path)
        .map_err(|e| format!("failed to read discovery file {}: {e}", path.display()))?;

    serde_json::from_str::<DiscoveryInfo>(&data)
        .map_err(|e| format!("failed to parse discovery file: {e}"))
}

/// Remove the discovery file upon server shutdown.
pub fn remove_discovery() {
    let path = discovery_file_path();
    if path.exists() {
        if let Err(e) = fs::remove_file(&path) {
            log::warn!("Could not remove discovery file {}: {e}", path.display());
        } else {
            log::info!("Cleaned up OxyConvert API discovery file");
        }
    }
}
