//! Lightweight pure safe Rust HTTP & JSON-RPC / MCP loopback server for OxyConvert.

use crate::api::discovery::{remove_discovery, write_discovery};
use crate::api::types::*;
use crate::converters::wasm::PluginRuntime;
use crate::converters::{ConversionOptions, convert_script, detect_ecosystem};
use crate::package::bundle::AddonPackage;
use crate::package::installer::{
    install_addon, list_installed_addons, oxybrush_plugins_dir, uninstall_addon,
};
use crate::package::manifest::{AddonCategory, SourceEcosystem};
use base64::Engine;
use std::fs;
use std::io::{BufRead, BufReader, Cursor, Read, Write};
use std::net::{IpAddr, Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

/// Handle for controlling the running loopback API server.
pub struct ServerHandle {
    pub endpoint: String,
    pub port: u16,
    pub token: Option<String>,
    stop_flag: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl ServerHandle {
    /// Request server stop and clean up discovery.
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        remove_discovery();
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Start the OxyConvert loopback API server.
pub fn start_server(
    port: u16,
    token: Option<String>,
    write_discovery_file: bool,
) -> Result<ServerHandle, String> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .map_err(|e| format!("could not bind 127.0.0.1:{port}: {e}"))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;

    let addr = listener.local_addr().map_err(|e| e.to_string())?;
    if addr.ip() != IpAddr::V4(Ipv4Addr::LOCALHOST) {
        return Err(format!(
            "refusing to listen on non-localhost IP: {}",
            addr.ip()
        ));
    }

    let actual_port = addr.port();
    let endpoint = format!("http://127.0.0.1:{actual_port}");

    if write_discovery_file
        && let Err(e) = write_discovery(&endpoint, actual_port, token.as_deref())
    {
        log::warn!("Could not write discovery file: {e}");
    }

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_thread = Arc::clone(&stop_flag);
    let token_thread = token.clone();

    let thread = std::thread::Builder::new()
        .name("oxc-api".into())
        .spawn(move || {
            accept_loop(listener, token_thread, stop_thread);
        })
        .map_err(|e| e.to_string())?;

    log::info!("OxyConvert Suite API listening on {endpoint}");

    Ok(ServerHandle {
        endpoint,
        port: actual_port,
        token,
        stop_flag,
        thread: Some(thread),
    })
}

fn accept_loop(listener: TcpListener, token: Option<String>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, peer)) => {
                if peer.ip() != IpAddr::V4(Ipv4Addr::LOCALHOST) {
                    continue;
                }
                let _ = stream.set_nonblocking(false);
                let token_ref = token.as_deref();
                handle_http_connection(stream, token_ref);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => break,
        }
    }
}

fn handle_http_connection(mut stream: TcpStream, required_token: Option<&str>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let mut reader = BufReader::new(&stream);

    let mut first_line = String::new();
    if reader.read_line(&mut first_line).unwrap_or(0) == 0 {
        return;
    }

    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }

    let method = parts[0].to_uppercase();
    let raw_path = parts[1];
    let path = raw_path.split('?').next().unwrap_or(raw_path);

    let mut content_length = 0usize;
    let mut auth_token = None;

    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some((k, v)) = trimmed.split_once(':') {
            let key = k.trim().to_lowercase();
            let val = v.trim();
            if key == "content-length" {
                content_length = val.parse().unwrap_or(0);
            } else if key == "authorization" {
                if let Some(bearer) = val.strip_prefix("Bearer ") {
                    auth_token = Some(bearer.to_string());
                } else {
                    auth_token = Some(val.to_string());
                }
            } else if key == "x-oxy-token" {
                auth_token = Some(val.to_string());
            }
        }
    }

    // CORS preflight
    if method == "OPTIONS" {
        let response = "HTTP/1.1 204 No Content\r\n\
            Access-Control-Allow-Origin: *\r\n\
            Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
            Access-Control-Allow-Headers: Content-Type, Authorization, X-Oxy-Token\r\n\
            Content-Length: 0\r\n\r\n";
        let _ = stream.write_all(response.as_bytes());
        return;
    }

    // Token check if configured
    if let Some(req_tok) = required_token
        && auth_token.as_deref() != Some(req_tok)
    {
        let body = r#"{"error":"unauthorized","message":"invalid or missing auth token"}"#;
        let response = format!(
            "HTTP/1.1 401 Unauthorized\r\n\
            Content-Type: application/json\r\n\
            Access-Control-Allow-Origin: *\r\n\
            Content-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes());
        return;
    }

    // Read request body
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        let _ = reader.read_exact(&mut body);
    }

    let (status_code, json_resp) = dispatch_route(&method, path, &body);

    let response = format!(
        "HTTP/1.1 {status_code}\r\n\
        Content-Type: application/json\r\n\
        Access-Control-Allow-Origin: *\r\n\
        Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
        Access-Control-Allow-Headers: Content-Type, Authorization, X-Oxy-Token\r\n\
        Content-Length: {}\r\n\r\n{}",
        json_resp.len(),
        json_resp
    );

    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

fn dispatch_route(method: &str, path: &str, body: &[u8]) -> (&'static str, String) {
    match (method, path) {
        ("GET", "/") | ("GET", "/api/v1/status") | ("GET", "/status") => {
            let res = handle_status();
            ("200 OK", serde_json::to_string(&res).unwrap_or_default())
        }
        ("POST", "/api/v1/detect") | ("POST", "/detect") => {
            let req: DetectRequest = serde_json::from_slice(body).unwrap_or_default();
            let res = handle_detect(req);
            ("200 OK", serde_json::to_string(&res).unwrap_or_default())
        }
        ("POST", "/api/v1/convert") | ("POST", "/convert") => {
            let req: ConvertRequest = match serde_json::from_slice(body) {
                Ok(r) => r,
                Err(e) => {
                    return (
                        "400 Bad Request",
                        format!(r#"{{"success":false,"error":"invalid json: {}"}}"#, e),
                    );
                }
            };
            let res = handle_convert(req);
            ("200 OK", serde_json::to_string(&res).unwrap_or_default())
        }
        ("POST", "/api/v1/inspect") | ("POST", "/inspect") => {
            let req: InspectRequest = serde_json::from_slice(body).unwrap_or_default();
            let res = handle_inspect(req);
            ("200 OK", serde_json::to_string(&res).unwrap_or_default())
        }
        ("POST", "/api/v1/install") | ("POST", "/install") => {
            let req: InstallRequest = serde_json::from_slice(body).unwrap_or_default();
            let res = handle_install(req);
            ("200 OK", serde_json::to_string(&res).unwrap_or_default())
        }
        ("GET", "/api/v1/list") | ("GET", "/list") => {
            let res = handle_list();
            ("200 OK", serde_json::to_string(&res).unwrap_or_default())
        }
        ("POST", "/api/v1/uninstall") | ("POST", "/uninstall") => {
            let req: UninstallRequest = match serde_json::from_slice(body) {
                Ok(r) => r,
                Err(e) => {
                    return (
                        "400 Bad Request",
                        format!(r#"{{"success":false,"message":"invalid json: {}"}}"#, e),
                    );
                }
            };
            let res = handle_uninstall(req);
            ("200 OK", serde_json::to_string(&res).unwrap_or_default())
        }
        ("POST", "/rpc") | ("POST", "/mcp") => {
            let resp = handle_jsonrpc(body);
            ("200 OK", serde_json::to_string(&resp).unwrap_or_default())
        }
        _ => (
            "404 Not Found",
            r#"{"error":"not_found","message":"endpoint does not exist"}"#.to_string(),
        ),
    }
}

pub fn handle_status() -> ApiStatusResponse {
    let plugins_dir = oxybrush_plugins_dir();
    let plugins_count = list_installed_addons().map(|l| l.len()).unwrap_or(0);

    ApiStatusResponse {
        app: "OxyConvert".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        status: "ready".to_string(),
        supported_ecosystems: vec![
            "zbrush".to_string(),
            "blender".to_string(),
            "nomad".to_string(),
        ],
        supported_runtimes: vec!["wasm".to_string(), "python".to_string()],
        oxybrush_plugins_dir: plugins_dir.map(|p| p.to_string_lossy().to_string()),
        installed_addons_count: plugins_count,
        endpoint: "http://127.0.0.1".to_string(),
    }
}

pub fn handle_detect(req: DetectRequest) -> DetectResponse {
    let source = req.code.as_deref().unwrap_or("");
    let filename = req.filename.as_deref().or(req.file_path.as_deref());
    let eco = detect_ecosystem(source, filename);
    let (conf, details) = match eco {
        SourceEcosystem::Zbrush => (0.95, "Detected ZBrush ZScript syntax or .zsc binary"),
        SourceEcosystem::Blender => (0.95, "Detected Blender bpy module or bl_info definition"),
        SourceEcosystem::Nomad => (0.90, "Detected Nomad Sculpt JavaScript or JSON preset"),
        SourceEcosystem::Native => (0.50, "OxyBrush native format or unknown script"),
    };
    DetectResponse {
        ecosystem: format!("{eco:?}").to_lowercase(),
        confidence: conf,
        details: details.to_string(),
    }
}

pub fn handle_convert(req: ConvertRequest) -> ConvertResponse {
    let runtime = match req.runtime.as_deref().unwrap_or("wasm") {
        "python" => PluginRuntime::Python,
        _ => PluginRuntime::Wasm,
    };

    let cat = req
        .category
        .as_deref()
        .and_then(|c| match c.to_lowercase().as_str() {
            "sculpting" => Some(AddonCategory::Sculpting),
            "topology" => Some(AddonCategory::Topology),
            "pipeline" => Some(AddonCategory::Pipeline),
            "procedural" => Some(AddonCategory::Procedural),
            "materials" => Some(AddonCategory::Materials),
            "utility" => Some(AddonCategory::Utility),
            _ => None,
        });

    let mut options = ConversionOptions {
        addon_id: None,
        addon_name: req.name.clone(),
        addon_author: req.author.clone(),
        addon_version: None,
        addon_description: None,
        category: cat,
        include_original_as_comment: true,
        runtime,
    };

    let script_source = if let Some(src) = req.source {
        src
    } else if let Some(path_str) = req.file_path.as_deref() {
        let p = Path::new(path_str);
        if !p.exists() {
            return ConvertResponse {
                success: false,
                ecosystem: "unknown".into(),
                runtime: format!("{runtime:?}").to_lowercase(),
                manifest: None,
                wat_code: None,
                python_code: None,
                warnings: Vec::new(),
                package_path: None,
                installed_path: None,
                bundle_base64: None,
                error: Some(format!("Source file not found: {path_str}")),
            };
        }
        if options.addon_name.is_none()
            && let Some(stem) = p.file_stem().and_then(|s| s.to_str())
        {
            options.addon_name = Some(stem.replace(['_', '-'], " "));
        }
        match fs::read_to_string(p) {
            Ok(s) => s,
            Err(e) => {
                return ConvertResponse {
                    success: false,
                    ecosystem: "unknown".into(),
                    runtime: format!("{runtime:?}").to_lowercase(),
                    manifest: None,
                    wat_code: None,
                    python_code: None,
                    warnings: Vec::new(),
                    package_path: None,
                    installed_path: None,
                    bundle_base64: None,
                    error: Some(format!("Failed to read source file: {e}")),
                };
            }
        }
    } else if let Some(b64) = req.source_base64 {
        match base64::engine::general_purpose::STANDARD.decode(b64) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(s) => s,
                Err(e) => {
                    return ConvertResponse {
                        success: false,
                        ecosystem: "unknown".into(),
                        runtime: format!("{runtime:?}").to_lowercase(),
                        manifest: None,
                        wat_code: None,
                        python_code: None,
                        warnings: Vec::new(),
                        package_path: None,
                        installed_path: None,
                        bundle_base64: None,
                        error: Some(format!("Decoded bytes are not valid UTF-8: {e}")),
                    };
                }
            },
            Err(e) => {
                return ConvertResponse {
                    success: false,
                    ecosystem: "unknown".into(),
                    runtime: format!("{runtime:?}").to_lowercase(),
                    manifest: None,
                    wat_code: None,
                    python_code: None,
                    warnings: Vec::new(),
                    package_path: None,
                    installed_path: None,
                    bundle_base64: None,
                    error: Some(format!("Base64 decode failed: {e}")),
                };
            }
        }
    } else {
        return ConvertResponse {
            success: false,
            ecosystem: "unknown".into(),
            runtime: format!("{runtime:?}").to_lowercase(),
            manifest: None,
            wat_code: None,
            python_code: None,
            warnings: Vec::new(),
            package_path: None,
            installed_path: None,
            bundle_base64: None,
            error: Some("No source script, file_path, or source_base64 provided".into()),
        };
    };

    let eco = if let Some(e) = req.ecosystem.as_deref() {
        match e.to_lowercase().as_str() {
            "zbrush" => SourceEcosystem::Zbrush,
            "blender" => SourceEcosystem::Blender,
            "nomad" => SourceEcosystem::Nomad,
            _ => SourceEcosystem::Native,
        }
    } else {
        detect_ecosystem(&script_source, req.file_path.as_deref())
    };

    let result = convert_script(&script_source, eco, &options);

    let wat_code = result.wasm_definition.as_ref().map(|w| w.wat_code.clone());
    let mut pkg = AddonPackage::new(result.manifest.clone(), result.python_code.clone());
    pkg.wat_script = wat_code.clone();

    let mut written_path = None;
    let mut installed_path = None;
    let mut bundle_base64 = None;

    // Save package if output_path is requested
    if let Some(out_str) = req.output_path {
        let out_path = PathBuf::from(out_str);
        if let Err(e) = pkg.save_to_archive(&out_path) {
            return ConvertResponse {
                success: false,
                ecosystem: format!("{:?}", result.source_ecosystem).to_lowercase(),
                runtime: format!("{runtime:?}").to_lowercase(),
                manifest: Some(result.manifest),
                wat_code,
                python_code: Some(result.python_code),
                warnings: result.warnings,
                package_path: None,
                installed_path: None,
                bundle_base64: None,
                error: Some(format!("Failed to save .oxyaddon package: {e}")),
            };
        }
        written_path = Some(out_path.to_string_lossy().to_string());
    }

    // Direct install if requested
    if req.install_direct.unwrap_or(false) {
        match install_addon(&pkg, false, true) {
            Ok(p) => installed_path = Some(p.to_string_lossy().to_string()),
            Err(e) => log::warn!("Direct install failed: {e}"),
        }
    }

    // Base64 bundle payload if requested
    if req.return_bundle_base64.unwrap_or(false)
        && let Ok(bytes) = pkg.to_bytes()
    {
        bundle_base64 = Some(base64::engine::general_purpose::STANDARD.encode(bytes));
    }

    ConvertResponse {
        success: true,
        ecosystem: format!("{:?}", result.source_ecosystem).to_lowercase(),
        runtime: format!("{runtime:?}").to_lowercase(),
        manifest: Some(result.manifest),
        wat_code,
        python_code: Some(result.python_code),
        warnings: result.warnings,
        package_path: written_path,
        installed_path,
        bundle_base64,
        error: None,
    }
}

pub fn handle_inspect(req: InspectRequest) -> InspectResponse {
    let pkg = if let Some(p) = req.package_path {
        let path = PathBuf::from(&p);
        if !path.exists() {
            return InspectResponse {
                success: false,
                manifest: None,
                entries: Vec::new(),
                total_size_bytes: 0,
                error: Some(format!("Package file not found: {p}")),
            };
        }
        match AddonPackage::open(&path) {
            Ok(pkg) => pkg,
            Err(e) => {
                return InspectResponse {
                    success: false,
                    manifest: None,
                    entries: Vec::new(),
                    total_size_bytes: 0,
                    error: Some(format!("Failed to open .oxyaddon package: {e}")),
                };
            }
        }
    } else if let Some(b64) = req.package_base64 {
        match base64::engine::general_purpose::STANDARD.decode(b64) {
            Ok(bytes) => match AddonPackage::from_reader(Cursor::new(bytes)) {
                Ok(pkg) => pkg,
                Err(e) => {
                    return InspectResponse {
                        success: false,
                        manifest: None,
                        entries: Vec::new(),
                        total_size_bytes: 0,
                        error: Some(format!("Failed to parse archive from base64: {e}")),
                    };
                }
            },
            Err(e) => {
                return InspectResponse {
                    success: false,
                    manifest: None,
                    entries: Vec::new(),
                    total_size_bytes: 0,
                    error: Some(format!("Invalid base64 payload: {e}")),
                };
            }
        }
    } else {
        return InspectResponse {
            success: false,
            manifest: None,
            entries: Vec::new(),
            total_size_bytes: 0,
            error: Some("No package_path or package_base64 provided".into()),
        };
    };

    let mut entries = vec![
        "manifest.json".to_string(),
        pkg.manifest.entry_point.clone(),
    ];
    if pkg.wat_script.is_some() {
        entries.push("plugin.wat".to_string());
    }
    for extra in pkg.extra_files.keys() {
        entries.push(extra.clone());
    }

    let total_size = pkg.to_bytes().map(|b| b.len()).unwrap_or(0);

    InspectResponse {
        success: true,
        manifest: Some(pkg.manifest),
        entries,
        total_size_bytes: total_size,
        error: None,
    }
}

pub fn handle_install(req: InstallRequest) -> InstallResponse {
    let pkg = if let Some(p) = req.package_path {
        let path = PathBuf::from(&p);
        if !path.exists() {
            return InstallResponse {
                success: false,
                plugin_id: String::new(),
                install_dir: String::new(),
                manifest: None,
                error: Some(format!("Package file not found: {p}")),
            };
        }
        match AddonPackage::open(&path) {
            Ok(p) => p,
            Err(e) => {
                return InstallResponse {
                    success: false,
                    plugin_id: String::new(),
                    install_dir: String::new(),
                    manifest: None,
                    error: Some(format!("Failed to read package: {e}")),
                };
            }
        }
    } else if let Some(b64) = req.package_base64 {
        match base64::engine::general_purpose::STANDARD.decode(b64) {
            Ok(bytes) => match AddonPackage::from_reader(Cursor::new(bytes)) {
                Ok(p) => p,
                Err(e) => {
                    return InstallResponse {
                        success: false,
                        plugin_id: String::new(),
                        install_dir: String::new(),
                        manifest: None,
                        error: Some(format!("Failed to read package from base64: {e}")),
                    };
                }
            },
            Err(e) => {
                return InstallResponse {
                    success: false,
                    plugin_id: String::new(),
                    install_dir: String::new(),
                    manifest: None,
                    error: Some(format!("Invalid base64 payload: {e}")),
                };
            }
        }
    } else {
        return InstallResponse {
            success: false,
            plugin_id: String::new(),
            install_dir: String::new(),
            manifest: None,
            error: Some("No package_path or package_base64 provided".into()),
        };
    };

    let manifest = pkg.manifest.clone();
    let plugin_id = manifest.id.clone();

    match install_addon(&pkg, false, true) {
        Ok(dest) => InstallResponse {
            success: true,
            plugin_id,
            install_dir: dest.to_string_lossy().to_string(),
            manifest: Some(manifest),
            error: None,
        },
        Err(e) => InstallResponse {
            success: false,
            plugin_id,
            install_dir: String::new(),
            manifest: Some(manifest),
            error: Some(format!("Installation failed: {e}")),
        },
    }
}

pub fn handle_list() -> ListResponse {
    let plugins_dir = oxybrush_plugins_dir().unwrap_or_else(|| PathBuf::from("plugins"));
    let raw_list = list_installed_addons().unwrap_or_default();

    let addons = raw_list
        .into_iter()
        .map(|info| InstalledAddonInfo {
            id: info.name.clone(),
            name: info.name.clone(),
            version: "unknown".to_string(),
            ecosystem: "native".to_string(),
            kind: if info.is_archive {
                "archive".into()
            } else if info.is_directory {
                "directory".into()
            } else {
                "python_script".into()
            },
            directory: info.path.to_string_lossy().to_string(),
            has_package_archive: info.is_archive,
            has_python_bridge: info.is_standalone_py,
        })
        .collect();

    ListResponse {
        plugins_dir: plugins_dir.to_string_lossy().to_string(),
        addons,
    }
}

pub fn handle_uninstall(req: UninstallRequest) -> GenericResponse {
    let plugins_dir = match oxybrush_plugins_dir() {
        Some(d) => d,
        None => {
            return GenericResponse {
                success: false,
                message: "OxyBrush plugins directory not found".into(),
            };
        }
    };

    let folder = plugins_dir.join(&req.plugin_id);
    let pkg_file = plugins_dir.join(format!("{}.oxyaddon", req.plugin_id));
    let py_file = plugins_dir.join(format!("{}.py", req.plugin_id));

    let mut removed = false;
    if folder.exists() && uninstall_addon(&folder).is_ok() {
        removed = true;
    }
    if pkg_file.exists() && uninstall_addon(&pkg_file).is_ok() {
        removed = true;
    }
    if py_file.exists() && uninstall_addon(&py_file).is_ok() {
        removed = true;
    }

    if removed {
        GenericResponse {
            success: true,
            message: format!("Plugin '{}' uninstalled successfully", req.plugin_id),
        }
    } else {
        GenericResponse {
            success: false,
            message: format!("Plugin '{}' was not found in plugins folder", req.plugin_id),
        }
    }
}

fn handle_jsonrpc(body: &[u8]) -> JsonRpcResponse {
    let req: JsonRpcRequest = match serde_json::from_slice(body) {
        Ok(r) => r,
        Err(e) => {
            return JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: None,
                result: None,
                error: Some(JsonRpcError {
                    code: -32700,
                    message: format!("Parse error: {e}"),
                    data: None,
                }),
            };
        }
    };

    let id = req.id.clone();
    let params = req.params.unwrap_or(serde_json::Value::Null);

    let result = match req.method.as_str() {
        "oxc.status" | "status" => serde_json::to_value(handle_status()).ok(),
        "oxc.detect" | "detect" => {
            let d_req: DetectRequest = serde_json::from_value(params).unwrap_or_default();
            serde_json::to_value(handle_detect(d_req)).ok()
        }
        "oxc.convert" | "convert" => {
            let c_req: ConvertRequest = serde_json::from_value(params).unwrap_or_default();
            serde_json::to_value(handle_convert(c_req)).ok()
        }
        "oxc.inspect" | "inspect" => {
            let i_req: InspectRequest = serde_json::from_value(params).unwrap_or_default();
            serde_json::to_value(handle_inspect(i_req)).ok()
        }
        "oxc.install" | "install" => {
            let in_req: InstallRequest = serde_json::from_value(params).unwrap_or_default();
            serde_json::to_value(handle_install(in_req)).ok()
        }
        "oxc.list" | "list" => serde_json::to_value(handle_list()).ok(),
        "oxc.uninstall" | "uninstall" => {
            let u_req: UninstallRequest =
                serde_json::from_value(params).unwrap_or(UninstallRequest {
                    plugin_id: String::new(),
                });
            serde_json::to_value(handle_uninstall(u_req)).ok()
        }
        "initialize" => Some(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "oxyconvert",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
        "tools/list" => Some(serde_json::json!({
            "tools": [
                {
                    "name": "oxc_status",
                    "description": "Check OxyConvert status and installed OxyBrush plugins",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "oxc_detect",
                    "description": "Detect source ecosystem (ZBrush, Blender, Nomad Sculpt)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "code": { "type": "string" },
                            "filename": { "type": "string" }
                        }
                    }
                },
                {
                    "name": "oxc_convert",
                    "description": "Convert ZBrush/Blender/Nomad script to .oxyaddon",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "source": { "type": "string" },
                            "file_path": { "type": "string" },
                            "runtime": { "type": "string", "enum": ["wasm", "python"] },
                            "install_direct": { "type": "boolean" }
                        }
                    }
                },
                {
                    "name": "oxc_install",
                    "description": "Install .oxyaddon into OxyBrush plugins folder",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "package_path": { "type": "string" }
                        },
                        "required": ["package_path"]
                    }
                },
                {
                    "name": "oxc_list",
                    "description": "List all installed plugins in OxyBrush",
                    "inputSchema": { "type": "object", "properties": {} }
                }
            ]
        })),
        "tools/call" => {
            let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or(serde_json::Value::Null);

            let text_result = match tool_name {
                "oxc_status" => serde_json::to_string_pretty(&handle_status()).unwrap_or_default(),
                "oxc_detect" => {
                    let d: DetectRequest = serde_json::from_value(arguments).unwrap_or_default();
                    serde_json::to_string_pretty(&handle_detect(d)).unwrap_or_default()
                }
                "oxc_convert" => {
                    let c: ConvertRequest = serde_json::from_value(arguments).unwrap_or_default();
                    serde_json::to_string_pretty(&handle_convert(c)).unwrap_or_default()
                }
                "oxc_install" => {
                    let i: InstallRequest = serde_json::from_value(arguments).unwrap_or_default();
                    serde_json::to_string_pretty(&handle_install(i)).unwrap_or_default()
                }
                "oxc_list" => serde_json::to_string_pretty(&handle_list()).unwrap_or_default(),
                _ => format!("Unknown tool: {tool_name}"),
            };

            Some(serde_json::json!({
                "content": [
                    {
                        "type": "text",
                        "text": text_result
                    }
                ]
            }))
        }
        _ => {
            return JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: format!("Method not found: {}", req.method),
                    data: None,
                }),
            };
        }
    };

    JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id,
        result,
        error: None,
    }
}
