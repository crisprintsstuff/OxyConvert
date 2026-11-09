//! OxyConvert Suite API: Inter-Process Communication (IPC), HTTP REST, JSON-RPC, and MCP tools
//! allowing OxyBrush, OxyLauncher, OxyTools, OxyPixel, and OxyChar to communicate seamlessly.

pub mod client;
pub mod discovery;
pub mod server;
pub mod types;

pub use client::OxcClient;
pub use discovery::{discovery_file_path, read_discovery, remove_discovery, write_discovery};
pub use server::{ServerHandle, start_server};
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovery_read_write_cycle() {
        let endpoint = "http://127.0.0.1:8999";
        let port = 8999;
        let token = Some("test_secret_token");

        let path = write_discovery(endpoint, port, token).expect("write discovery");
        assert!(path.exists());

        let read = read_discovery().expect("read discovery");
        assert_eq!(read.app, "OxyConvert");
        assert_eq!(read.endpoint, endpoint);
        assert_eq!(read.port, port);
        assert_eq!(read.token.as_deref(), token);

        remove_discovery();
        assert!(!path.exists());
    }

    #[test]
    fn test_api_server_client_roundtrip() {
        // Bind on port 0 for ephemeral port allocation in test
        let mut server = start_server(0, None, false).expect("start test server");
        let client = OxcClient::new(&server.endpoint, None).expect("create client");

        // 1. Status test
        let status = client.status().expect("status call");
        assert_eq!(status.app, "OxyConvert");
        assert_eq!(status.status, "ready");
        assert!(status.supported_ecosystems.contains(&"zbrush".to_string()));

        // 2. Detect test
        let zscript = r#"[IButton, "Deform Mesh", "Pinch mesh inward", [VarSet, amount, 0.5]]"#;
        let detect = client.detect(Some(zscript), None).expect("detect call");
        assert_eq!(detect.ecosystem, "zbrush");
        assert!(detect.confidence > 0.8);

        // 3. Convert test (WebAssembly RFC 0011)
        let req = ConvertRequest {
            source: Some(zscript.to_string()),
            runtime: Some("wasm".to_string()),
            name: Some("Suite Test Plugin".to_string()),
            return_bundle_base64: Some(true),
            ..Default::default()
        };
        let converted = client.convert(req).expect("convert call");
        assert!(converted.success);
        assert_eq!(converted.ecosystem, "zbrush");
        assert_eq!(converted.runtime, "wasm");
        assert!(converted.wat_code.is_some());
        assert!(converted.manifest.is_some());
        assert!(converted.bundle_base64.is_some());

        // 4. Inspect test using the returned base64 bundle
        let inspect_req = InspectRequest {
            package_base64: converted.bundle_base64,
            ..Default::default()
        };
        let inspected = client.inspect(inspect_req).expect("inspect call");
        assert!(inspected.success);
        assert!(inspected.manifest.is_some());
        assert!(inspected.entries.contains(&"manifest.json".to_string()));
        assert!(inspected.entries.contains(&"plugin.wat".to_string()));

        server.stop();
    }
}
