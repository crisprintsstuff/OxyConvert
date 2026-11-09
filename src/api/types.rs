//! Data transfer types for the OxyConvert Suite IPC & HTTP API.

use crate::package::manifest::AddonManifest;
use serde::{Deserialize, Serialize};

/// Discovery file information written when the OxyConvert API server starts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryInfo {
    pub app: String,
    pub version: String,
    pub endpoint: String,
    pub port: u16,
    pub token: Option<String>,
    pub pid: u32,
    pub started_at_secs: u64,
}

/// Service status and system environment info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiStatusResponse {
    pub app: String,
    pub version: String,
    pub status: String,
    pub supported_ecosystems: Vec<String>,
    pub supported_runtimes: Vec<String>,
    pub oxybrush_plugins_dir: Option<String>,
    pub installed_addons_count: usize,
    pub endpoint: String,
}

/// Request to detect the source ecosystem of a script or file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DetectRequest {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub file_path: Option<String>,
}

/// Result of ecosystem detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectResponse {
    pub ecosystem: String,
    pub confidence: f32,
    pub details: String,
}

/// Request to convert a script or archive to .oxyaddon.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConvertRequest {
    /// Source code as string (optional if `file_path` is given)
    #[serde(default)]
    pub source: Option<String>,
    /// Source file path on disk (.txt, .zsc, .py, .js, .json, .zip)
    #[serde(default)]
    pub file_path: Option<String>,
    /// Base64 encoded binary archive or script data
    #[serde(default)]
    pub source_base64: Option<String>,
    /// Target runtime mode: "wasm" (RFC 0011) or "python"
    #[serde(default)]
    pub runtime: Option<String>,
    /// Source ecosystem hint: "zbrush", "blender", "nomad"
    #[serde(default)]
    pub ecosystem: Option<String>,
    /// Addon display name override
    #[serde(default)]
    pub name: Option<String>,
    /// Addon category override
    #[serde(default)]
    pub category: Option<String>,
    /// Addon author override
    #[serde(default)]
    pub author: Option<String>,
    /// Destination path to write the .oxyaddon archive file
    #[serde(default)]
    pub output_path: Option<String>,
    /// If true, automatically install into OxyBrush's plugins folder
    #[serde(default)]
    pub install_direct: Option<bool>,
    /// If true, return the generated .oxyaddon archive as base64 in response
    #[serde(default)]
    pub return_bundle_base64: Option<bool>,
}

/// Result of script conversion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertResponse {
    pub success: bool,
    pub ecosystem: String,
    pub runtime: String,
    pub manifest: Option<AddonManifest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wat_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub python_code: Option<String>,
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Request to inspect an .oxyaddon package.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InspectRequest {
    #[serde(default)]
    pub package_path: Option<String>,
    #[serde(default)]
    pub package_base64: Option<String>,
}

/// Information about an inspected .oxyaddon archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectResponse {
    pub success: bool,
    pub manifest: Option<AddonManifest>,
    pub entries: Vec<String>,
    pub total_size_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Request to pack a directory into .oxyaddon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackRequest {
    pub directory_path: String,
    pub output_path: String,
}

/// Request to unpack an .oxyaddon archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnpackRequest {
    pub package_path: String,
    pub output_dir: String,
}

/// Request to install a package into OxyBrush.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstallRequest {
    #[serde(default)]
    pub package_path: Option<String>,
    #[serde(default)]
    pub package_base64: Option<String>,
}

/// Response from installing an addon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallResponse {
    pub success: bool,
    pub plugin_id: String,
    pub install_dir: String,
    pub manifest: Option<AddonManifest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Summary of an installed addon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledAddonInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub ecosystem: String,
    pub kind: String,
    pub directory: String,
    pub has_package_archive: bool,
    pub has_python_bridge: bool,
}

/// Response listing all installed addons.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResponse {
    pub plugins_dir: String,
    pub addons: Vec<InstalledAddonInfo>,
}

/// Request to uninstall an addon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UninstallRequest {
    pub plugin_id: String,
}

/// Response from uninstalling an addon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericResponse {
    pub success: bool,
    pub message: String,
}

/// JSON-RPC 2.0 Request structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 Response structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 Error structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}
