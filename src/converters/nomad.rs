//! Nomad Sculpt JavaScript / JSON preset transpiler to OxyBrush Python (.oxyaddon).

use crate::converters::{ConversionOptions, ConversionResult, ScriptConverter};
use crate::package::manifest::{AddonCategory, AddonManifest, SourceEcosystem, UiBinding};
use regex::Regex;
use serde_json::Value;

pub struct NomadConverter;

impl ScriptConverter for NomadConverter {
    fn convert(&self, source: &str, options: &ConversionOptions) -> ConversionResult {
        let mut warnings = Vec::new();
        let mut mapped_calls = Vec::new();

        // Check if input is a JSON preset/tool file or a JS script
        let is_json = source.trim_start().starts_with('{');

        let (meta, python_body) = if is_json {
            parse_nomad_json(source, &mut warnings, &mut mapped_calls)
        } else {
            parse_nomad_js(source, &mut warnings, &mut mapped_calls, options.include_original_as_comment)
        };

        let addon_name = options
            .addon_name
            .clone()
            .or(meta.name)
            .unwrap_or_else(|| "Nomad Sculpt Converted Tool".to_string());

        let addon_id = options.addon_id.clone().unwrap_or_else(|| {
            format!(
                "com.oxybrush.nomad.{}",
                addon_name
                    .to_lowercase()
                    .replace([' ', '-', ':', '.'], "_")
            )
        });

        let addon_author = options
            .addon_author
            .clone()
            .or(meta.author)
            .unwrap_or_else(|| "Nomad Sculpt Community".to_string());

        let addon_version = options
            .addon_version
            .clone()
            .or(meta.version)
            .unwrap_or_else(|| "1.0.0".to_string());

        let addon_description = options
            .addon_description
            .clone()
            .or(meta.description)
            .unwrap_or_else(|| "Converted from Nomad Sculpt script to OxyBrush .oxyaddon".to_string());

        let category = options.category.unwrap_or(AddonCategory::Sculpting);

        let full_script = format!(
            r#""""
# Name: {name}
# Description: {desc}
# Author: {author}
# Version: {version}
# Converted from Nomad Sculpt by OxyConvert
"""
import sys
import math
from pathlib import Path

# Ensure OxyBrush pure-Python SDK is accessible
try:
    import oxybrush
except ImportError:
    import os
    for candidate in [
        Path(os.environ.get("LOCALAPPDATA", "")) / "OxyBrush" / "python",
        Path(os.environ.get("APPDATA", "")) / "OxyBrush" / "python",
        Path.home() / "AppData" / "Local" / "OxyBrush" / "python",
        Path.home() / ".local" / "share" / "oxybrush" / "python",
        Path.home() / "Library" / "Application Support" / "oxybrush" / "python",
    ]:
        if (candidate / "oxybrush" / "__init__.py").is_file() and str(candidate) not in sys.path:
            sys.path.insert(0, str(candidate))
            break
    import oxybrush


def execute_nomad_addon():
    app = oxybrush.attach()
    print("[OxyAddon] Running {name}...")

{python_body}

    print("[OxyAddon] {name} completed successfully.")


if __name__ == "__main__":
    execute_nomad_addon()
"#,
            name = addon_name,
            desc = addon_description,
            author = addon_author,
            version = addon_version,
            python_body = indent_block(&python_body, 4),
        );

        let manifest = AddonManifest {
            id: addon_id,
            name: addon_name,
            version: addon_version,
            author: addon_author,
            description: addon_description,
            category,
            target_app: "OxyBrush".to_string(),
            entry_point: "main.py".to_string(),
            source_ecosystem: SourceEcosystem::Nomad,
            min_oxybrush_version: "0.5.0".to_string(),
            tags: vec!["nomad".into(), "converted".into(), "sculpt".into()],
            permissions: vec!["scene:read".into(), "scene:modify".into(), "mesh:sculpt".into()],
            homepage: None,
            license: Some("MIT".into()),
            ui: UiBinding {
                menu: "Plugins".to_string(),
                submenu: Some("Nomad Sculpt Tools".to_string()),
                button_label: None,
                tooltip: None,
                shortcut: None,
            },
            ..Default::default()
        };

        ConversionResult {
            manifest,
            python_code: full_script,
            warnings,
            mapped_calls,
            source_ecosystem: SourceEcosystem::Nomad,
        }
    }
}

#[derive(Default)]
struct NomadMeta {
    name: Option<String>,
    author: Option<String>,
    version: Option<String>,
    description: Option<String>,
}

fn parse_nomad_json(
    source: &str,
    warnings: &mut Vec<String>,
    mapped_calls: &mut Vec<String>,
) -> (NomadMeta, String) {
    let mut meta = NomadMeta::default();
    let mut out_lines = Vec::new();

    let json_val: Result<Value, _> = serde_json::from_str(source);
    match json_val {
        Ok(v) => {
            if let Some(name) = v.get("name").and_then(|n| n.as_str()) {
                meta.name = Some(name.to_string());
            }
            if let Some(author) = v.get("author").and_then(|a| a.as_str()) {
                meta.author = Some(author.to_string());
            }
            if let Some(desc) = v.get("description").and_then(|d| d.as_str()) {
                meta.description = Some(desc.to_string());
            }

            // Extract tool
            if let Some(tool) = v.get("tool").and_then(|t| t.as_str()) {
                let oxy_brush = map_nomad_tool(tool);
                mapped_calls.push(format!("Nomad tool '{tool}' -> app.select_brush('{oxy_brush}')"));
                out_lines.push(format!("app.select_brush('{oxy_brush}')"));
            }

            // Extract brush radius / intensity
            let radius = v.get("radius").and_then(|r| r.as_f64());
            let intensity = v.get("intensity").and_then(|i| i.as_f64());
            if radius.is_some() || intensity.is_some() {
                let r_str = radius.map(|r| format!("size={r}")).unwrap_or_default();
                let i_str = intensity.map(|i| format!("strength={i}")).unwrap_or_default();
                let args = [r_str, i_str].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
                mapped_calls.push(format!("Nomad brush settings -> app.set_brush({args})"));
                out_lines.push(format!("app.set_brush({args})"));
            }

            // Remesh
            if let Some(remesh) = v.get("remesh").and_then(|r| r.as_f64()) {
                mapped_calls.push(format!("Nomad remesh {remesh} -> app.voxel_remesh(voxel_size={remesh})"));
                out_lines.push(format!("app.voxel_remesh(voxel_size={remesh})"));
            }

            // Symmetry
            if let Some(sym_x) = v.get("symmetryX").and_then(|s| s.as_bool()) {
                mapped_calls.push(format!("Nomad symmetryX {sym_x} -> app.set_symmetry(x={sym_x})"));
                out_lines.push(format!("app.set_symmetry(x={sym_x})"));
            }
        }
        Err(e) => {
            warnings.push(format!("Failed to parse Nomad JSON: {e}"));
        }
    }

    if out_lines.is_empty() {
        out_lines.push("# Inspect active Nomad-converted mesh:".to_string());
        out_lines.push("stats = app.get_mesh_stats()".to_string());
        out_lines.push("print(f'Piece count: {stats.piece_count}, triangles: {stats.triangle_count}')".to_string());
    }

    (meta, out_lines.join("\n"))
}

fn parse_nomad_js(
    source: &str,
    warnings: &mut Vec<String>,
    mapped_calls: &mut Vec<String>,
    include_comments: bool,
) -> (NomadMeta, String) {
    let mut meta = NomadMeta::default();
    let mut out_lines = Vec::new();

    // Check for comment headers
    for line in source.lines().take(30) {
        let trimmed = line.trim().trim_start_matches(['/', '*', '#']).trim();
        if let Some(rest) = strip_case_prefix(trimmed, "Name:") {
            meta.name = Some(rest.trim().to_string());
        } else if let Some(rest) = strip_case_prefix(trimmed, "Author:") {
            meta.author = Some(rest.trim().to_string());
        } else if let Some(rest) = strip_case_prefix(trimmed, "Description:") {
            meta.description = Some(rest.trim().to_string());
        }
    }

    let remesh_re = Regex::new(r"nomad\.(?:topology\.)?remesh\s*\(\s*\{?[^0-9.]*([0-9.]+)?").unwrap();
    let decimate_re = Regex::new(r"nomad\.decimate\s*\(\s*\{?[^0-9.]*([0-9.]+)?").unwrap();
    let brush_re = Regex::new(r#"nomad\.(?:setBrush|tool)\s*\(?\s*['"]([^'"]+)['"]"#).unwrap();
    let primitive_re = Regex::new(r#"nomad\.addPrimitive\s*\(\s*['"]([^'"]+)['"]"#).unwrap();
    let export_re = Regex::new(r#"nomad\.export\s*\(\s*\{?[^'"]*['"]?([^'"}]+)"#).unwrap();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("//") || trimmed.starts_with("/*") {
            if include_comments {
                out_lines.push(format!("# {}", trimmed.trim_start_matches(['/', '*']).trim()));
            }
            continue;
        }

        // Remesh
        if let Some(caps) = remesh_re.captures(trimmed) {
            let size = caps.get(1).map(|m| m.as_str()).unwrap_or("0.03");
            mapped_calls.push(format!("nomad.remesh -> app.voxel_remesh(voxel_size={size})"));
            out_lines.push(format!("app.voxel_remesh(voxel_size={size})"));
            continue;
        }

        // Decimate
        if let Some(caps) = decimate_re.captures(trimmed) {
            let ratio = caps.get(1).map(|m| m.as_str()).unwrap_or("50.0");
            mapped_calls.push(format!("nomad.decimate -> app.decimate(keep_percent={ratio})"));
            out_lines.push(format!("app.decimate(keep_percent={ratio})"));
            continue;
        }

        // Brush
        if let Some(caps) = brush_re.captures(trimmed) {
            let bname = caps.get(1).unwrap().as_str();
            let oxy = map_nomad_tool(bname);
            mapped_calls.push(format!("nomad.setBrush('{bname}') -> app.select_brush('{oxy}')"));
            out_lines.push(format!("app.select_brush('{oxy}')"));
            continue;
        }

        // Primitive
        if let Some(caps) = primitive_re.captures(trimmed) {
            let prim = caps.get(1).unwrap().as_str();
            mapped_calls.push(format!("nomad.addPrimitive('{prim}') -> app.add_primitive('{prim}')"));
            out_lines.push(format!("app.add_primitive('{prim}')"));
            continue;
        }

        // Export
        if let Some(caps) = export_re.captures(trimmed) {
            let path = caps.get(1).unwrap().as_str();
            mapped_calls.push(format!("nomad.export('{path}') -> app.export_mesh('{path}')"));
            out_lines.push(format!("app.export_mesh('{path}')"));
            continue;
        }

        // Mask
        if trimmed.contains("nomad.mask.clear") {
            mapped_calls.push("nomad.mask.clear() -> app.mask('clear')".into());
            out_lines.push("app.mask('clear')".into());
            continue;
        }
        if trimmed.contains("nomad.mask.invert") {
            mapped_calls.push("nomad.mask.invert() -> app.mask('invert')".into());
            out_lines.push("app.mask('invert')".into());
            continue;
        }

        // Symmetry
        if trimmed.contains("nomad.symmetry") {
            mapped_calls.push("nomad.symmetry -> app.set_symmetry(x=True)".into());
            out_lines.push("app.set_symmetry(x=True)".into());
            continue;
        }

        // Console log
        if trimmed.starts_with("console.log(") {
            let log_text = trimmed.trim_start_matches("console.log(").trim_end_matches([')', ';']);
            out_lines.push(format!("print({log_text})"));
            continue;
        }

        warnings.push(format!("Unhandled Nomad expression: {trimmed}"));
        out_lines.push(format!("# [Unmapped Nomad]: {trimmed}"));
    }

    if out_lines.is_empty() {
        out_lines.push("stats = app.get_mesh_stats()".into());
        out_lines.push("print(f'Active piece vertices: {stats.vertex_count}')".into());
    }

    (meta, out_lines.join("\n"))
}

fn map_nomad_tool(tool: &str) -> &'static str {
    match tool.to_lowercase().as_str() {
        "clay" => "Clay",
        "claystrips" => "ClayStrips",
        "smooth" => "Smooth",
        "brush" => "Draw",
        "drag" | "move" => "Grab",
        "inflate" => "Inflate",
        "crease" => "Crease",
        "pinch" => "Pinch",
        "flatten" => "Flatten",
        "trim" => "TrimCurve",
        "split" => "SliceCurve",
        "mask" => "Mask",
        "paint" => "Paint",
        _ => "Clay",
    }
}

fn strip_case_prefix<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

fn indent_block(text: &str, spaces: usize) -> String {
    let pad = " ".repeat(spaces);
    text.lines()
        .map(|line| {
            if line.is_empty() {
                line.to_string()
            } else {
                format!("{pad}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nomad_js_conversion() {
        let js = r#"
            // Name: Organic Remesh Pass
            // Author: NomadSculptor
            // Description: Smooths, remeshes and decimates mesh
            nomad.setBrush('clay');
            nomad.topology.remesh(0.04);
            nomad.decimate(25);
            console.log("Nomad pass complete");
        "#;

        let converter = NomadConverter;
        let res = converter.convert(js, &ConversionOptions::default());

        assert_eq!(res.manifest.name, "Organic Remesh Pass");
        assert_eq!(res.manifest.author, "NomadSculptor");
        assert!(res.python_code.contains("app.select_brush('Clay')"));
        assert!(res.python_code.contains("app.voxel_remesh(voxel_size=0.04)"));
        assert!(res.python_code.contains("app.decimate(keep_percent=25)"));
        assert!(res.python_code.contains("print(\"Nomad pass complete\")"));
    }

    #[test]
    fn test_nomad_json_conversion() {
        let json = r#"{
            "name": "Heavy Clay Tool",
            "author": "NomadPro",
            "tool": "clay",
            "radius": 40.0,
            "intensity": 0.85,
            "remesh": 0.02
        }"#;

        let converter = NomadConverter;
        let res = converter.convert(json, &ConversionOptions::default());

        assert_eq!(res.manifest.name, "Heavy Clay Tool");
        assert_eq!(res.manifest.author, "NomadPro");
        assert!(res.python_code.contains("app.select_brush('Clay')"));
        assert!(res.python_code.contains("app.set_brush(size=40, strength=0.85)"));
        assert!(res.python_code.contains("app.voxel_remesh(voxel_size=0.02)"));
    }
}
