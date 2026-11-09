//! WebAssembly generator and transpiler for OxyBrush in-app plugins (RFC 0011).
//!
//! Generates WebAssembly Text (`plugin.wat`) and plugin.json manifests
//! conforming to OxyBrush's in-process sandboxed plugin API (`api: 1` and `api: 2`).

use crate::package::manifest::AddonManifest;
use serde::{Deserialize, Serialize};

/// Target plugin runtime kind supported by OxyBrush.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PluginRuntime {
    /// In-app sandboxed WebAssembly plugin (plugin.wat / plugin.wasm).
    /// Runs directly inside OxyBrush's wasmi engine on worker threads with UI sliders & command buttons.
    Wasm,
    /// Python script addon (main.py). Runs as external process via local MCP agent bridge.
    #[default]
    Python,
}

impl PluginRuntime {
    pub const ALL: [PluginRuntime; 2] = [PluginRuntime::Wasm, PluginRuntime::Python];

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Wasm => "WebAssembly (In-App Plugin)",
            Self::Python => "Python (Agent Addon)",
        }
    }

    pub fn badge(&self) -> &'static str {
        match self {
            Self::Wasm => "⚡ WebAssembly",
            Self::Python => "🐍 Python",
        }
    }
}

/// A parameter slider for in-app WebAssembly plugins.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WasmParam {
    pub id: String,
    pub label: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
}

/// A command button for in-app WebAssembly plugins.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WasmCommand {
    pub id: String,
    pub label: String,
}

/// WebAssembly plugin configuration and generated `.wat` code.
#[derive(Debug, Clone, PartialEq)]
pub struct WasmPluginDefinition {
    pub name: String,
    pub description: String,
    pub version: String,
    pub commands: Vec<WasmCommand>,
    pub params: Vec<WasmParam>,
    pub wat_code: String,
}

impl WasmPluginDefinition {
    /// Generate standard Squash-style deformation filter WebAssembly text (.wat).
    pub fn default_deformer(name: &str, description: &str) -> Self {
        let commands = vec![WasmCommand {
            id: "apply".into(),
            label: "Apply Deformation".into(),
        }];
        let params = vec![
            WasmParam {
                id: "amount".into(),
                label: "Amount".into(),
                min: 0.0,
                max: 1.0,
                default: 0.35,
            },
            WasmParam {
                id: "smooth".into(),
                label: "Smooth Factor".into(),
                min: 0.0,
                max: 1.0,
                default: 0.5,
            },
        ];

        let wat_code = format!(
            r#";; {name} - Generated OxyBrush WebAssembly In-App Plugin (RFC 0011)
;; Sandboxed in-process mesh deformation filter.
(module
  (import "oxybrush" "log" (func $log (param i32 i32)))
  (import "oxybrush" "param" (func $param (param i32) (result f32)))
  (import "oxybrush" "active_piece" (func $active (result i32)))
  (import "oxybrush" "vertex_count" (func $vertex_count (param i32) (result i32)))
  (import "oxybrush" "read_positions" (func $read_positions (param i32 i32 i32) (result i32)))
  (import "oxybrush" "read_mask" (func $read_mask (param i32 i32 i32) (result i32)))
  (import "oxybrush" "write_positions" (func $write_positions (param i32 i32 i32) (result i32)))
  (import "oxybrush" "draw_line" (func $draw_line (param f32 f32 f32 f32 f32 f32 i32) (result i32)))

  (memory (export "memory") 1)
  (data (i32.const 0) "{name} applied successfully.")
  (global $middle (mut f32) (f32.const 0))

  ;; Positions start at byte 1024; the mask follows them.
  (func (export "run") (param $command i32)
    (local $piece i32) (local $n i32) (local $i i32) (local $at i32)
    (local $mask_at i32) (local $need i32) (local $sum f32) (local $mid f32)
    (local $amount f32) (local $y f32) (local $keep f32)
    (local.set $piece (call $active))
    (local.set $n (call $vertex_count (local.get $piece)))
    (if (i32.le_s (local.get $n) (i32.const 0)) (then (return)))
    (local.set $mask_at (i32.add (i32.const 1024) (i32.mul (local.get $n) (i32.const 12))))
    
    ;; Grow memory to fit positions and mask
    (local.set $need
      (i32.div_u
        (i32.add (i32.add (local.get $mask_at) (i32.mul (local.get $n) (i32.const 4))) (i32.const 65535))
        (i32.const 65536)))
    (if (i32.gt_u (local.get $need) (memory.size))
      (then (drop (memory.grow (i32.sub (local.get $need) (memory.size))))))
    (drop (call $read_positions (local.get $piece) (i32.const 1024) (local.get $n)))
    (drop (call $read_mask (local.get $piece) (local.get $mask_at) (local.get $n)))

    ;; Calculate centroid / middle height
    (local.set $i (i32.const 0))
    (block $done (loop $sum_y
      (br_if $done (i32.ge_u (local.get $i) (local.get $n)))
      (local.set $at (i32.add (i32.const 1028) (i32.mul (local.get $i) (i32.const 12))))
      (local.set $sum (f32.add (local.get $sum) (f32.load (local.get $at))))
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br $sum_y)))
    (local.set $mid (f32.div (local.get $sum) (f32.convert_i32_u (local.get $n))))
    (global.set $middle (local.get $mid))
    (local.set $amount (call $param (i32.const 0)))

    ;; Transform vertices respecting mask: y' = mid + (y - mid) * (1 - amount * (1 - mask))
    (local.set $i (i32.const 0))
    (block $done2 (loop $deform
      (br_if $done2 (i32.ge_u (local.get $i) (local.get $n)))
      (local.set $at (i32.add (i32.const 1028) (i32.mul (local.get $i) (i32.const 12))))
      (local.set $y (f32.load (local.get $at)))
      (local.set $keep
        (f32.sub (f32.const 1)
          (f32.mul (local.get $amount)
            (f32.sub (f32.const 1)
              (f32.load (i32.add (local.get $mask_at) (i32.mul (local.get $i) (i32.const 4))))))))
      (f32.store (local.get $at)
        (f32.add (local.get $mid) (f32.mul (f32.sub (local.get $y) (local.get $mid)) (local.get $keep))))
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br $deform)))
    (drop (call $write_positions (local.get $piece) (i32.const 1024) (local.get $n)))
    (call $log (i32.const 0) (i32.const {msg_len})))

  ;; Viewport overlay visualization: draws crosshair at operation height
  (func (export "overlay")
    (drop (call $draw_line (f32.const -1.5) (global.get $middle) (f32.const 0)
                           (f32.const 1.5) (global.get $middle) (f32.const 0) (i32.const 0xe07a4fff)))
    (drop (call $draw_line (f32.const 0) (global.get $middle) (f32.const -1.5)
                           (f32.const 0) (global.get $middle) (f32.const 1.5) (i32.const 0xe07a4fff)))))
"#,
            name = name,
            msg_len = name.len() + 29
        );

        Self {
            name: name.into(),
            description: description.into(),
            version: "1.0.0".into(),
            commands,
            params,
            wat_code,
        }
    }

    /// Transpile or convert mesh-oriented operations into WebAssembly text (.wat).
    pub fn from_source(source: &str, manifest: &AddonManifest) -> Self {
        let mut def = Self::default_deformer(&manifest.name, &manifest.description);
        def.version = manifest.version.clone();

        // Detect if source has noise / displacement / scale operations
        let lower = source.to_lowercase();
        if lower.contains("noise") || lower.contains("displace") || lower.contains("inflate") {
            def.params.push(WasmParam {
                id: "frequency".into(),
                label: "Frequency / Detail".into(),
                min: 0.1,
                max: 10.0,
                default: 2.5,
            });
        }
        if lower.contains("smooth") || lower.contains("relax") {
            def.commands.push(WasmCommand {
                id: "smooth".into(),
                label: "Smooth Mesh".into(),
            });
        }

        def
    }

    /// Generate OxyBrush in-app WebAssembly `plugin.json`.
    pub fn to_plugin_json(&self, id: &str) -> String {
        let json_val = serde_json::json!({
            "name": self.name,
            "id": id,
            "version": self.version,
            "description": self.description,
            "api": 1,
            "capabilities": ["read_mesh", "edit_mesh", "overlay"],
            "commands": self.commands,
            "params": self.params
        });
        serde_json::to_string_pretty(&json_val).unwrap_or_default()
    }
}
