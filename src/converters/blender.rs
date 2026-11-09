//! Blender Python (`bpy`) addon transpiler to OxyBrush Python (.oxyaddon).

use crate::converters::{ConversionOptions, ConversionResult, ScriptConverter};
use crate::package::manifest::{AddonCategory, AddonManifest, SourceEcosystem, UiBinding};
use regex::Regex;

pub struct BlenderConverter;

impl ScriptConverter for BlenderConverter {
    fn convert(&self, source: &str, options: &ConversionOptions) -> ConversionResult {
        let mut warnings = Vec::new();
        let mut mapped_calls = Vec::new();

        // 1. Extract bl_info dictionary
        let bl_info = extract_bl_info(source);

        let addon_name = options
            .addon_name
            .clone()
            .or(bl_info.name)
            .unwrap_or_else(|| "Blender Converted Addon".to_string());

        let addon_id = options.addon_id.clone().unwrap_or_else(|| {
            format!(
                "com.oxybrush.blender.{}",
                addon_name
                    .to_lowercase()
                    .replace([' ', '-', ':', '.'], "_")
            )
        });

        let addon_author = options
            .addon_author
            .clone()
            .or(bl_info.author)
            .unwrap_or_else(|| "Blender Creator".to_string());

        let addon_version = options
            .addon_version
            .clone()
            .or(bl_info.version)
            .unwrap_or_else(|| "1.0.0".to_string());

        let addon_description = options
            .addon_description
            .clone()
            .or(bl_info.description)
            .unwrap_or_else(|| "Converted from Blender Python addon to OxyBrush .oxyaddon".to_string());

        let category = options
            .category
            .or_else(|| bl_info.category.as_deref().map(map_blender_category))
            .unwrap_or(AddonCategory::Sculpting);

        // 2. Transpile bpy operations to oxybrush SDK
        let python_body = transpile_blender_body(
            source,
            &mut warnings,
            &mut mapped_calls,
            options.include_original_as_comment,
        );

        let full_script = format!(
            r#""""
# Name: {name}
# Description: {desc}
# Author: {author}
# Version: {version}
# Converted from Blender Addon (bpy) by OxyConvert
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


def execute_blender_addon():
    app = oxybrush.attach()
    print("[OxyAddon] Running {name}...")

{python_body}

    print("[OxyAddon] {name} finished successfully.")


if __name__ == "__main__":
    execute_blender_addon()
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
            source_ecosystem: SourceEcosystem::Blender,
            min_oxybrush_version: "0.5.0".to_string(),
            tags: vec!["blender".into(), "converted".into(), "python".into()],
            permissions: vec!["scene:read".into(), "scene:modify".into(), "mesh:sculpt".into()],
            homepage: None,
            license: Some("GPL-3.0-or-later".into()),
            ui: UiBinding {
                menu: "Plugins".to_string(),
                submenu: Some("Blender Tools".to_string()),
                button_label: bl_info.bl_label,
                tooltip: bl_info.bl_tooltip,
                shortcut: None,
            },
            ..Default::default()
        };

        let wasm_def = if options.runtime == crate::converters::wasm::PluginRuntime::Wasm {
            Some(crate::converters::wasm::WasmPluginDefinition::from_source(source, &manifest))
        } else {
            None
        };

        ConversionResult {
            manifest,
            python_code: full_script,
            wasm_definition: wasm_def,
            warnings,
            mapped_calls,
            source_ecosystem: SourceEcosystem::Blender,
        }
    }
}

/// Convert a Blender addon distributed as a .zip archive.
pub fn convert_blender_zip(zip_path: &std::path::Path, options: &ConversionOptions) -> Result<ConversionResult, String> {
    use std::io::Read;
    let file = std::fs::File::open(zip_path).map_err(|e| format!("Could not open zip: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Invalid zip: {e}"))?;

    let mut py_files = Vec::new();
    let mut init_content = None;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if name.ends_with(".py") {
            let mut text = String::new();
            let _ = entry.read_to_string(&mut text);
            if name.ends_with("__init__.py") || init_content.is_none() {
                init_content = Some(text.clone());
            }
            py_files.push((name, text));
        }
    }

    let source = init_content.or_else(|| py_files.first().map(|(_, t)| t.clone())).ok_or_else(|| {
        "No Python (.py) files found in Blender addon .zip".to_string()
    })?;

    Ok(BlenderConverter.convert(&source, options))
}

#[derive(Default)]
struct BlenderInfo {
    name: Option<String>,
    author: Option<String>,
    version: Option<String>,
    description: Option<String>,
    category: Option<String>,
    bl_label: Option<String>,
    bl_tooltip: Option<String>,
}

fn extract_bl_info(source: &str) -> BlenderInfo {
    let mut info = BlenderInfo::default();

    // Regex for bl_info key-value pairs
    let name_re = Regex::new(r#""name"\s*:\s*"([^"]+)""#).unwrap();
    let author_re = Regex::new(r#""author"\s*:\s*"([^"]+)""#).unwrap();
    let desc_re = Regex::new(r#""description"\s*:\s*"([^"]+)""#).unwrap();
    let cat_re = Regex::new(r#""category"\s*:\s*"([^"]+)""#).unwrap();
    let ver_re = Regex::new(r#""version"\s*:\s*\(([0-9,\s]+)\)"#).unwrap();

    if let Some(caps) = name_re.captures(source) {
        info.name = Some(caps.get(1).unwrap().as_str().to_string());
    }
    if let Some(caps) = author_re.captures(source) {
        info.author = Some(caps.get(1).unwrap().as_str().to_string());
    }
    if let Some(caps) = desc_re.captures(source) {
        info.description = Some(caps.get(1).unwrap().as_str().to_string());
    }
    if let Some(caps) = cat_re.captures(source) {
        info.category = Some(caps.get(1).unwrap().as_str().to_string());
    }
    if let Some(caps) = ver_re.captures(source) {
        let parts: Vec<&str> = caps.get(1).unwrap().as_str().split(',').map(|s| s.trim()).collect();
        info.version = Some(parts.join("."));
    }

    // Try finding bl_label inside an Operator class
    let bllabel_re = Regex::new(r#"bl_label\s*=\s*"([^"]+)""#).unwrap();
    if let Some(caps) = bllabel_re.captures(source) {
        let label = caps.get(1).unwrap().as_str().to_string();
        info.bl_label = Some(label.clone());
        if info.name.is_none() {
            info.name = Some(label);
        }
    }

    info
}

fn map_blender_category(cat: &str) -> AddonCategory {
    match cat.to_lowercase().as_str() {
        "sculpt" | "sculpting" | "paint" => AddonCategory::Sculpting,
        "mesh" | "retopology" => AddonCategory::Topology,
        "import-export" | "io" | "pipeline" => AddonCategory::Pipeline,
        "add mesh" | "curve" => AddonCategory::Procedural,
        "material" | "render" => AddonCategory::Materials,
        _ => AddonCategory::Utility,
    }
}

fn transpile_blender_body(
    source: &str,
    warnings: &mut Vec<String>,
    mapped_calls: &mut Vec<String>,
    include_comments: bool,
) -> String {
    let mut out_lines = Vec::new();
    let mut inside_operator_execute = false;
    let mut execute_indent_level = 0;

    for line in source.lines() {
        let trimmed = line.trim();

        // Skip import bpy or register boilerplate
        if trimmed.starts_with("import bpy")
            || trimmed.starts_with("from bpy")
            || trimmed.starts_with("def register():")
            || trimmed.starts_with("def unregister():")
            || trimmed.starts_with("bpy.utils.register_class")
            || trimmed.starts_with("bpy.utils.unregister_class")
        {
            continue;
        }

        // Track operator execute method
        if trimmed.starts_with("def execute(self, context):") || trimmed.starts_with("def execute(self, context,") {
            inside_operator_execute = true;
            execute_indent_level = line.chars().take_while(|c| c.is_whitespace()).count();
            out_lines.push("# Begin operator execution:".to_string());
            continue;
        }

        if inside_operator_execute {
            let current_indent = line.chars().take_while(|c| c.is_whitespace()).count();
            if trimmed.is_empty() {
                continue;
            }
            if current_indent <= execute_indent_level && !trimmed.starts_with('#') {
                inside_operator_execute = false;
            }
        }

        // Skip return {'FINISHED'}
        if trimmed.contains("return {'FINISHED'}") || trimmed.contains("return {'CANCELLED'}") {
            continue;
        }

        // Comments
        if trimmed.starts_with('#') {
            if include_comments {
                out_lines.push(trimmed.to_string());
            }
            continue;
        }

        // Match bpy.ops calls and translate
        if trimmed.contains("bpy.ops.") || trimmed.contains("context.") {
            let translated = translate_bpy_expression(trimmed, warnings, mapped_calls);
            out_lines.push(translated);
            continue;
        }

        // Non-bpy Python statements (math, loops, logic) pass through if relevant
        if !trimmed.starts_with("class ") && !trimmed.starts_with("bl_") && !trimmed.is_empty() {
            out_lines.push(trimmed.to_string());
        }
    }

    if out_lines.is_empty() {
        out_lines.push("# Default mesh statistics inspection:".to_string());
        out_lines.push("stats = app.get_mesh_stats()".to_string());
        out_lines.push("print(f'Active mesh triangles: {stats.triangle_count}, pieces: {stats.piece_count}')".to_string());
    }

    out_lines.join("\n")
}

fn translate_bpy_expression(
    expr: &str,
    warnings: &mut Vec<String>,
    mapped_calls: &mut Vec<String>,
) -> String {
    let lower = expr.to_lowercase();

    // Voxel Remesh
    if lower.contains("bpy.ops.object.voxel_remesh") {
        mapped_calls.push("bpy.ops.object.voxel_remesh -> app.voxel_remesh()".into());
        return "app.voxel_remesh()".into();
    }
    if lower.contains("remesh_voxel_size") {
        let val_re = Regex::new(r"remesh_voxel_size\s*=\s*([0-9.]+)").unwrap();
        if let Some(caps) = val_re.captures(expr) {
            let v = caps.get(1).unwrap().as_str();
            mapped_calls.push(format!("remesh_voxel_size = {v} -> app.voxel_remesh(voxel_size={v})"));
            return format!("app.voxel_remesh(voxel_size={v})");
        }
    }

    // Decimate
    if lower.contains("decimate") {
        mapped_calls.push("bpy.ops.object.modifier_add(type='DECIMATE') -> app.decimate()".into());
        return "app.decimate(keep_percent=25.0)".into();
    }

    // Subdivision
    if lower.contains("subdivision_set") || lower.contains("subsurf") {
        mapped_calls.push("bpy.ops.object.subdivision_set -> app.subdivide(smooth=True)".into());
        return "app.subdivide(smooth=True)".into();
    }

    // Brush select
    if lower.contains("paint.brush_select") || lower.contains("sculpt.brush") {
        let brush_name = if lower.contains("clay") {
            "Clay"
        } else if lower.contains("smooth") {
            "Smooth"
        } else if lower.contains("grab") || lower.contains("move") {
            "Grab"
        } else if lower.contains("inflate") {
            "Inflate"
        } else if lower.contains("crease") {
            "Crease"
        } else {
            "Draw"
        };
        mapped_calls.push(format!("sculpt brush select -> app.select_brush('{brush_name}')"));
        return format!("app.select_brush('{brush_name}')");
    }

    // Primitives
    if lower.contains("primitive_uv_sphere_add") || lower.contains("primitive_ico_sphere_add") {
        mapped_calls.push("primitive_uv_sphere_add -> app.add_primitive('sphere')".into());
        return "app.add_primitive('sphere')".into();
    }
    if lower.contains("primitive_cube_add") {
        mapped_calls.push("primitive_cube_add -> app.add_primitive('cube')".into());
        return "app.add_primitive('cube')".into();
    }
    if lower.contains("primitive_plane_add") {
        mapped_calls.push("primitive_plane_add -> app.add_primitive('plane')".into());
        return "app.add_primitive('plane')".into();
    }

    // Export / Import
    if lower.contains("export_scene.gltf") || lower.contains("export_scene.obj") {
        mapped_calls.push("export_scene -> app.export_mesh('exported_scene.glb')".into());
        return "app.export_mesh('exported_scene.glb')".into();
    }
    if lower.contains("import_scene.obj") || lower.contains("import_scene.gltf") {
        mapped_calls.push("import_scene -> app.import_file('model.obj')".into());
        return "app.import_file('model.obj')".into();
    }

    // Undo / Redo
    if lower.contains("ed.undo") {
        mapped_calls.push("bpy.ops.ed.undo -> app.undo()".into());
        return "app.undo()".into();
    }
    if lower.contains("ed.redo") {
        mapped_calls.push("bpy.ops.ed.redo -> app.redo()".into());
        return "app.redo()".into();
    }

    warnings.push(format!("Unhandled Blender call: {expr}"));
    format!("# [Unmapped Blender]: {expr}")
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
    fn test_blender_transpiler_simple() {
        let code = r#"
bl_info = {
    "name": "Sculpt Helper",
    "author": "BlendDev",
    "version": (1, 5),
    "description": "Voxel remesh and decimate operator",
    "category": "Sculpt",
}

import bpy

class SCULPT_OT_remesh_decimate(bpy.types.Operator):
    bl_idname = "sculpt.remesh_decimate"
    bl_label = "Remesh and Decimate"

    def execute(self, context):
        context.object.data.remesh_voxel_size = 0.04
        bpy.ops.object.voxel_remesh()
        bpy.ops.object.modifier_add(type='DECIMATE')
        return {'FINISHED'}
"#;

        let converter = BlenderConverter;
        let res = converter.convert(code, &ConversionOptions::default());

        assert_eq!(res.manifest.name, "Sculpt Helper");
        assert_eq!(res.manifest.author, "BlendDev");
        assert_eq!(res.manifest.version, "1.5");
        assert_eq!(res.manifest.category, AddonCategory::Sculpting);
        assert!(res.python_code.contains("app.voxel_remesh(voxel_size=0.04)"));
        assert!(res.python_code.contains("app.voxel_remesh()"));
        assert!(res.python_code.contains("app.decimate(keep_percent=25.0)"));
    }
}
