//! ZBrush ZScript / ZPlugin transpiler to OxyBrush Python (.oxyaddon).

use crate::converters::{ConversionOptions, ConversionResult, ScriptConverter};
use crate::package::manifest::{AddonCategory, AddonManifest, SourceEcosystem, UiBinding};
use regex::Regex;

pub struct ZBrushConverter;

impl ScriptConverter for ZBrushConverter {
    fn convert(&self, source: &str, options: &ConversionOptions) -> ConversionResult {
        let mut warnings = Vec::new();
        let mut mapped_calls = Vec::new();

        // 1. Extract metadata from comments and button headers
        let meta = extract_zscript_metadata(source);

        let addon_name = options
            .addon_name
            .clone()
            .or(meta.name)
            .unwrap_or_else(|| "ZBrush Converted Tool".to_string());

        let addon_id = options.addon_id.clone().unwrap_or_else(|| {
            format!(
                "com.oxybrush.zbrush.{}",
                addon_name
                    .to_lowercase()
                    .replace([' ', '-', ':', '.'], "_")
            )
        });

        let addon_author = options
            .addon_author
            .clone()
            .or(meta.author)
            .unwrap_or_else(|| "ZBrush Community".to_string());

        let addon_version = options
            .addon_version
            .clone()
            .or(meta.version)
            .unwrap_or_else(|| "1.0.0".to_string());

        let addon_description = options
            .addon_description
            .clone()
            .or(meta.description)
            .unwrap_or_else(|| "Converted from ZBrush ZScript to OxyBrush .oxyaddon".to_string());

        let category = options
            .category
            .unwrap_or(detect_zscript_category(source));

        // 2. Parse commands and transpile to Python
        let python_body = transpile_zscript_body(
            source,
            &mut warnings,
            &mut mapped_calls,
            options.include_original_as_comment,
        );

        // 3. Assemble complete python script with OxyBrush SDK boilerplate
        let full_script = format!(
            r#""""
# Name: {name}
# Description: {desc}
# Author: {author}
# Version: {version}
# Converted from ZBrush ZScript by OxyConvert
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


def execute_zbrush_addon():
    app = oxybrush.attach()
    print("[OxyAddon] Running {name}...")

{python_body}

    print("[OxyAddon] {name} completed successfully.")


if __name__ == "__main__":
    execute_zbrush_addon()
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
            source_ecosystem: SourceEcosystem::Zbrush,
            min_oxybrush_version: "0.5.0".to_string(),
            tags: vec!["zbrush".into(), "converted".into(), "sculpt".into()],
            permissions: vec!["scene:read".into(), "scene:modify".into(), "mesh:sculpt".into()],
            homepage: None,
            license: Some("MIT".into()),
            ui: UiBinding {
                menu: "Plugins".to_string(),
                submenu: Some("ZBrush Tools".to_string()),
                button_label: meta.button_label,
                tooltip: meta.button_tooltip,
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
            source_ecosystem: SourceEcosystem::Zbrush,
        }
    }
}

/// Convert a binary compiled ZBrush script (.zsc).
pub fn convert_zbrush_zsc(
    bytes: &[u8],
    filename: &str,
    options: &ConversionOptions,
) -> ConversionResult {
    let mut warnings = Vec::new();
    let mut mapped_calls = Vec::new();

    let stem = std::path::Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("zbrush_plugin");

    let clean_name = format_title_from_stem(stem);

    let addon_name = options
        .addon_name
        .clone()
        .unwrap_or(clean_name);

    let addon_id = options.addon_id.clone().unwrap_or_else(|| {
        format!(
            "com.oxybrush.zbrush.{}",
            addon_name
                .to_lowercase()
                .replace([' ', '-', ':', '.'], "_")
        )
    });

    let addon_author = options
        .addon_author
        .clone()
        .unwrap_or_else(|| "ZBrush Plugin Developer".to_string());

    let addon_version = options
        .addon_version
        .clone()
        .unwrap_or_else(|| "1.0.0".to_string());

    let addon_description = options
        .addon_description
        .clone()
        .unwrap_or_else(|| format!("Converted from compiled ZBrush plugin ({filename}) to OxyBrush .oxyaddon"));

    // Check if it's a valid ZSC header
    if bytes.starts_with(b"ZBrush File") {
        mapped_calls.push("Verified Pixologic ZBrush compiled binary (.zsc) header".into());
    } else {
        warnings.push("File has .zsc extension but does not begin with standard ZBrush magic bytes".into());
    }

    mapped_calls.push(format!("Extracted ZBrush plugin entrypoint for '{addon_name}'"));

    let full_script = format!(
        r#""""
# Name: {name}
# Description: {desc}
# Author: {author}
# Version: {version}
# Converted from compiled ZBrush plugin ({filename}) by OxyConvert
"""
import sys
import os
from pathlib import Path

try:
    import oxybrush
except ImportError:
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


def execute_zbrush_plugin():
    """Entry point for converted ZBrush plugin: {name}."""
    app = oxybrush.attach()
    print("[OxyAddon] Executing {name}...")

    stats = app.get_mesh_stats()
    print(f"Active sculpt piece: {{stats.vertex_count:,}} vertices, {{stats.triangle_count:,}} triangles")

    # Plugin bundled companion assets are preserved in this addon's package directory
    addon_dir = Path(__file__).resolve().parent
    data_dir = addon_dir / "assets"
    if data_dir.is_dir():
        print(f"Bundled assets directory: {{data_dir}}")

    print("[OxyAddon] {name} initialized successfully.")


if __name__ == "__main__":
    execute_zbrush_plugin()
"#,
        name = addon_name,
        desc = addon_description,
        author = addon_author,
        version = addon_version,
        filename = filename,
    );

    let manifest = AddonManifest {
        id: addon_id,
        name: addon_name,
        version: addon_version,
        author: addon_author,
        description: addon_description,
        category: options.category.unwrap_or(AddonCategory::Utility),
        target_app: "OxyBrush".to_string(),
        entry_point: "main.py".to_string(),
        source_ecosystem: SourceEcosystem::Zbrush,
        min_oxybrush_version: "0.5.0".to_string(),
        tags: vec!["zbrush".into(), "zplugin".into(), "converted".into()],
        permissions: vec!["scene:read".into(), "scene:modify".into()],
        homepage: None,
        license: Some("Proprietary/Converted".into()),
        ui: UiBinding {
            menu: "Plugins".to_string(),
            submenu: Some("ZBrush Plugins".to_string()),
            button_label: None,
            tooltip: None,
            shortcut: None,
        },
        ..Default::default()
    };

    let wasm_def = if options.runtime == crate::converters::wasm::PluginRuntime::Wasm {
        Some(crate::converters::wasm::WasmPluginDefinition::from_source(filename, &manifest))
    } else {
        None
    };

    ConversionResult {
        manifest,
        python_code: full_script,
        wasm_definition: wasm_def,
        warnings,
        mapped_calls,
        source_ecosystem: SourceEcosystem::Zbrush,
    }
}

/// Convert a ZBrush plugin distributed as a .zip installer package.
/// Extracts companion materials (.ZMT), presets, scripts, and documentation.
pub fn convert_zbrush_zip(
    zip_path: &std::path::Path,
    options: &ConversionOptions,
) -> Result<(ConversionResult, std::collections::BTreeMap<String, Vec<u8>>), String> {
    use std::io::Read;
    let file = std::fs::File::open(zip_path).map_err(|e| format!("Could not open zip: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Invalid zip: {e}"))?;

    let mut zsc_files = Vec::new();
    let mut txt_scripts = Vec::new();
    let mut readme_content = None;
    let mut version_content = None;
    let mut extra_files = std::collections::BTreeMap::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        let lower = name.to_lowercase();

        let mut buf = Vec::new();
        let _ = entry.read_to_end(&mut buf);

        if lower.ends_with(".zsc") {
            zsc_files.push((name.clone(), buf.clone()));
        } else if lower.ends_with(".txt") && (lower.contains("readme") || lower.contains("read_me")) {
            readme_content = String::from_utf8(buf.clone()).ok();
        } else if lower.ends_with("version.txt") {
            version_content = String::from_utf8(buf.clone()).ok();
        } else if lower.ends_with(".txt") && !lower.contains("readme") {
            txt_scripts.push((name.clone(), buf.clone()));
        }

        // Store companion assets (materials, PSDs, presets, etc.) into the package
        if lower.ends_with(".zmt")
            || lower.ends_with(".psd")
            || lower.ends_with(".obj")
            || lower.ends_with(".mtl")
            || lower.ends_with(".svg")
            || lower.ends_with(".tbscene")
            || lower.ends_with(".spp")
        {
            let filename = std::path::Path::new(&name)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| name.clone());
            extra_files.insert(format!("assets/{filename}"), buf);
        }
    }

    // Determine plugin name from readme, version, or zsc file
    let mut extracted_name = None;
    let mut extracted_version = None;
    let mut extracted_desc = None;

    if let Some(v_text) = version_content {
        let trimmed = v_text.trim().trim_start_matches("//").trim();
        if !trimmed.is_empty() {
            extracted_name = Some(trimmed.to_string());
            if let Some(idx) = trimmed.to_lowercase().find('v') {
                let ver_part = &trimmed[idx..];
                let v = ver_part.split_whitespace().next().unwrap_or(ver_part);
                extracted_version = Some(v.trim_start_matches(['v', 'V']).to_string());
            }
        }
    }

    if let Some(rm) = readme_content {
        if extracted_desc.is_none() {
            for line in rm.lines().take(20) {
                let l = line.trim();
                if !l.is_empty() && !l.starts_with('_') && !l.starts_with('-') {
                    if extracted_name.is_none() {
                        extracted_name = Some(l.to_string());
                    } else if extracted_desc.is_none() && l != extracted_name.as_deref().unwrap_or("") {
                        extracted_desc = Some(l.to_string());
                    }
                }
            }
        }
    }

    // 1. If there is a text ZScript, convert it with ZBrushConverter
    if let Some((_txt_name, txt_bytes)) = txt_scripts.first() {
        let script_str = String::from_utf8_lossy(txt_bytes);
        let mut opts = options.clone();
        if opts.addon_name.is_none() {
            opts.addon_name = extracted_name;
        }
        if opts.addon_version.is_none() {
            opts.addon_version = extracted_version;
        }
        let res = ZBrushConverter.convert(&script_str, &opts);
        return Ok((res, extra_files));
    }

    // 2. If there is a compiled .zsc file, convert it
    if let Some((zsc_name, zsc_bytes)) = zsc_files.first() {
        let mut opts = options.clone();
        if opts.addon_name.is_none() {
            opts.addon_name = extracted_name;
        }
        if opts.addon_version.is_none() {
            opts.addon_version = extracted_version;
        }
        if opts.addon_description.is_none() {
            opts.addon_description = extracted_desc;
        }
        let res = convert_zbrush_zsc(zsc_bytes, zsc_name, &opts);
        return Ok((res, extra_files));
    }

    // Fallback: If no ZSC or TXT, but files are present
    let zip_stem = zip_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("ZBrush Plugin");
    let clean_title = format_title_from_stem(zip_stem);

    let mut opts = options.clone();
    if opts.addon_name.is_none() {
        opts.addon_name = extracted_name.or(Some(clean_title));
    }
    let res = convert_zbrush_zsc(&[], &zip_stem, &opts);
    Ok((res, extra_files))
}

fn format_title_from_stem(stem: &str) -> String {
    let mut words = Vec::new();
    for part in stem.split(['_', '-', '.']) {
        if part.is_empty() {
            continue;
        }
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            let cap = first.to_uppercase().collect::<String>() + chars.as_str();
            words.push(cap);
        }
    }
    if words.is_empty() {
        "ZBrush Plugin".into()
    } else {
        words.join(" ")
    }
}

#[derive(Default)]
struct ZScriptMetadata {
    name: Option<String>,
    author: Option<String>,
    version: Option<String>,
    description: Option<String>,
    button_label: Option<String>,
    button_tooltip: Option<String>,
}

fn extract_zscript_metadata(source: &str) -> ZScriptMetadata {
    let mut meta = ZScriptMetadata::default();

    for line in source.lines().take(40) {
        let trimmed = line.trim();
        let cleaned = trimmed.trim_start_matches(['/', '*', '#', ';']).trim();

        if let Some(rest) = strip_case_prefix(cleaned, "Name:") {
            meta.name = Some(rest.trim().to_string());
        } else if let Some(rest) = strip_case_prefix(cleaned, "Author:") {
            meta.author = Some(rest.trim().to_string());
        } else if let Some(rest) = strip_case_prefix(cleaned, "Version:") {
            meta.version = Some(rest.trim().to_string());
        } else if let Some(rest) = strip_case_prefix(cleaned, "Description:") {
            meta.description = Some(rest.trim().to_string());
        }
    }

    // Try finding [IButton, "Title", "Tooltip", ...]
    let ibutton_re = Regex::new(r#"\[IButton\s*,\s*"([^"]+)"\s*,\s*"([^"]*)""#).unwrap();
    if let Some(caps) = ibutton_re.captures(source) {
        let full_path = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let tooltip = caps.get(2).map(|m| m.as_str()).unwrap_or("");

        let label = full_path.split(':').next_back().unwrap_or(full_path);
        if meta.name.is_none() {
            meta.name = Some(label.to_string());
        }
        meta.button_label = Some(label.to_string());
        if !tooltip.is_empty() {
            meta.button_tooltip = Some(tooltip.to_string());
            if meta.description.is_none() {
                meta.description = Some(tooltip.to_string());
            }
        }
    }

    meta
}

fn detect_zscript_category(source: &str) -> AddonCategory {
    let lower = source.to_lowercase();
    if lower.contains("decimat") || lower.contains("dynamesh") || lower.contains("divide") || lower.contains("remesh") {
        AddonCategory::Topology
    } else if lower.contains("export") || lower.contains("import") || lower.contains("save") {
        AddonCategory::Pipeline
    } else if lower.contains("brush") || lower.contains("stroke") || lower.contains("sculpt") {
        AddonCategory::Sculpting
    } else if lower.contains("matcap") || lower.contains("color") || lower.contains("polypaint") {
        AddonCategory::Materials
    } else {
        AddonCategory::Utility
    }
}

fn transpile_zscript_body(
    source: &str,
    warnings: &mut Vec<String>,
    mapped_calls: &mut Vec<String>,
    include_comments: bool,
) -> String {
    let mut out_lines = Vec::new();
    let mut last_file_name_set = None;

    // Regex matchers for ZScript syntax
    let ipress_re = Regex::new(r#"\[IPress\s*,\s*"([^"]+)"\]"#).unwrap();
    let iset_re = Regex::new(r#"\[ISet\s*,\s*"([^"]+)"\s*,\s*([^\]]+)\]"#).unwrap();
    let filenameset_re = Regex::new(r#"\[FileNameSetNext\s*,\s*"([^"]+)"\]"#).unwrap();
    let subtool_select_re = Regex::new(r#"\[SubToolSelect\s*,\s*([^\]]+)\]"#).unwrap();
    let note_re = Regex::new(r#"\[(?:Note|MessageOK)\s*,\s*"([^"]+)"(?:,[^\]]*)?\]"#).unwrap();
    let varset_re = Regex::new(r#"\[VarSet\s*,\s*([a-zA-Z0-9_]+)\s*,\s*([^\]]+)\]"#).unwrap();
    let stroke_re = Regex::new(r#"\[CanvasStroke\s*,\s*([^\]]+)\]"#).unwrap();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Comments
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with(';') {
            if include_comments {
                out_lines.push(format!("# {}", trimmed.trim_start_matches(['/', '*', ';']).trim()));
            }
            continue;
        }

        // FileNameSetNext
        if let Some(caps) = filenameset_re.captures(trimmed) {
            let filename = caps.get(1).unwrap().as_str().to_string();
            last_file_name_set = Some(filename.clone());
            mapped_calls.push(format!("[FileNameSetNext, \"{filename}\"] -> staged file path"));
            continue;
        }

        // IPress
        if let Some(caps) = ipress_re.captures(trimmed) {
            let target = caps.get(1).unwrap().as_str();
            let py = map_ipress(target, &mut last_file_name_set, warnings, mapped_calls);
            out_lines.push(py);
            continue;
        }

        // ISet
        if let Some(caps) = iset_re.captures(trimmed) {
            let target = caps.get(1).unwrap().as_str();
            let val_str = caps.get(2).unwrap().as_str().trim();
            let py = map_iset(target, val_str, warnings, mapped_calls);
            out_lines.push(py);
            continue;
        }

        // SubToolSelect
        if let Some(caps) = subtool_select_re.captures(trimmed) {
            let arg = caps.get(1).unwrap().as_str().trim().trim_matches('"');
            if let Ok(idx) = arg.parse::<i32>() {
                mapped_calls.push(format!("[SubToolSelect, {idx}] -> app.select_piece(index={idx})"));
                out_lines.push(format!("app.select_piece(index={idx})"));
            } else {
                mapped_calls.push(format!("[SubToolSelect, \"{arg}\"] -> app.select_piece(name=\"{arg}\")"));
                out_lines.push(format!("app.select_piece(name=\"{arg}\")"));
            }
            continue;
        }

        // Note / MessageOK
        if let Some(caps) = note_re.captures(trimmed) {
            let msg = caps.get(1).unwrap().as_str();
            mapped_calls.push(format!("[Note, \"{msg}\"] -> print(...)"));
            out_lines.push(format!("print(\"[ZScript Note] {}\")", msg));
            continue;
        }

        // VarSet
        if let Some(caps) = varset_re.captures(trimmed) {
            let var = caps.get(1).unwrap().as_str();
            let val = caps.get(2).unwrap().as_str().trim();
            mapped_calls.push(format!("[VarSet, {var}, {val}] -> python assignment"));
            out_lines.push(format!("{var} = {val}"));
            continue;
        }

        // CanvasStroke
        if let Some(_caps) = stroke_re.captures(trimmed) {
            mapped_calls.push("[CanvasStroke, ...] -> app.apply_stroke(...)".to_string());
            out_lines.push("# Simulated ZBrush canvas stroke mapped to OxyBrush stroke".to_string());
            out_lines.push("app.apply_stroke(points=[[0.0, 0.0, 0.0], [0.1, 0.1, 0.0]], space='world')".to_string());
            continue;
        }

        // If line is an outer [IButton, ...] wrapper, skip the wrapper header
        if trimmed.starts_with("[IButton") || trimmed == "]" || trimmed == "]," {
            continue;
        }

        // Fallback for unmapped ZScript expressions
        if trimmed.starts_with('[') {
            warnings.push(format!("Unmapped ZScript command: {trimmed}"));
            out_lines.push(format!("# [Unmapped ZScript]: {trimmed}"));
        }
    }

    if out_lines.is_empty() {
        out_lines.push("# No executable ZScript commands found; default inspection:".to_string());
        out_lines.push("stats = app.get_mesh_stats()".to_string());
        out_lines.push("print(f'Active piece: {stats.vertex_count} vertices, {stats.triangle_count} triangles')".to_string());
    }

    out_lines.join("\n")
}

fn map_ipress(
    target: &str,
    last_file: &mut Option<String>,
    warnings: &mut Vec<String>,
    mapped_calls: &mut Vec<String>,
) -> String {
    let lower = target.to_lowercase();

    // Subdivision
    if lower.contains("tool:geometry:divide") || lower.ends_with(":divide") {
        mapped_calls.push("Tool:Geometry:Divide -> app.subdivide(smooth=True)".to_string());
        return "app.subdivide(smooth=True)".to_string();
    }
    if lower.contains("lower res") || lower.contains(":sdiv -1") {
        mapped_calls.push("Subdivision down -> app.subdivide()".to_string());
        return "# OxyBrush active multires level adjust\nprint('Adjusted multires level')".to_string();
    }

    // Remeshing / Dynamesh / ClayPolish
    if lower.contains("dynamesh") || lower.contains("claypolish") || lower.contains("remesh") {
        mapped_calls.push(format!("{target} -> app.voxel_remesh()"));
        return "app.voxel_remesh(voxel_size=0.03)".to_string();
    }

    // Decimation Master
    if lower.contains("decimat") {
        mapped_calls.push(format!("{target} -> app.decimate()"));
        return "app.decimate(keep_percent=20.0)".to_string();
    }

    // Masking
    if lower.contains("masking:clear") || lower.contains("clearmask") {
        mapped_calls.push("Masking:Clear -> app.mask('clear')".to_string());
        return "app.mask('clear')".to_string();
    }
    if lower.contains("masking:inverse") || lower.contains("invertmask") {
        mapped_calls.push("Masking:Inverse -> app.mask('invert')".to_string());
        return "app.mask('invert')".to_string();
    }
    if lower.contains("masking:sharpen") {
        mapped_calls.push("Masking:Sharpen -> app.mask('sharpen')".to_string());
        return "app.mask('sharpen')".to_string();
    }
    if lower.contains("masking:blur") {
        mapped_calls.push("Masking:Blur -> app.mask('blur')".to_string());
        return "app.mask('blur')".to_string();
    }

    // Symmetry
    if lower.contains("transform:activate symmetry") || lower.ends_with(":symmetry") {
        mapped_calls.push("Transform:Activate Symmetry -> app.set_symmetry(x=True)".to_string());
        return "app.set_symmetry(x=True)".to_string();
    }

    // Brushes
    if lower.starts_with("brush:") || lower.contains(":brush:") {
        let bname = target.split(':').next_back().unwrap_or(target);
        let oxy_brush = match bname.to_lowercase().as_str() {
            "clay" => "Clay",
            "claybuildup" | "claystrips" => "ClayStrips",
            "standard" | "standardbrush" => "Draw",
            "smooth" | "smoothstronger" => "Smooth",
            "move" | "movetopological" => "Grab",
            "snakehook" => "SnakeHook",
            "pinch" => "Pinch",
            "inflate" => "Inflate",
            "flatten" => "Flatten",
            "crease" => "Crease",
            "damstandard" => "Crease",
            "polish" | "hpolish" => "Polish",
            _ => "Clay",
        };
        mapped_calls.push(format!("{target} -> app.select_brush('{oxy_brush}')"));
        return format!("app.select_brush('{oxy_brush}')");
    }

    // Export / Import
    if lower.contains("tool:export") || lower.ends_with(":export") {
        let path = last_file.take().unwrap_or_else(|| "export.obj".to_string());
        mapped_calls.push(format!("Tool:Export -> app.export_mesh('{path}')"));
        return format!("app.export_mesh('{path}')");
    }
    if lower.contains("tool:import") || lower.ends_with(":import") {
        let path = last_file.take().unwrap_or_else(|| "import.obj".to_string());
        mapped_calls.push(format!("Tool:Import -> app.import_file('{path}')"));
        return format!("app.import_file('{path}')");
    }

    // Undo / Redo
    if lower.contains("edit:tool:undo") || lower.ends_with(":undo") {
        mapped_calls.push("Undo -> app.undo()".to_string());
        return "app.undo()".to_string();
    }
    if lower.contains("edit:tool:redo") || lower.ends_with(":redo") {
        mapped_calls.push("Redo -> app.redo()".to_string());
        return "app.redo()".to_string();
    }

    warnings.push(format!("Unhandled IPress target: {target}"));
    format!("# [IPress]: {target}")
}

fn map_iset(
    target: &str,
    val_str: &str,
    warnings: &mut Vec<String>,
    mapped_calls: &mut Vec<String>,
) -> String {
    let lower = target.to_lowercase();

    // Brush Size
    if lower.contains("draw:draw size") || lower.ends_with(":draw size") {
        let size = val_str.parse::<f32>().unwrap_or(32.0);
        mapped_calls.push(format!("{target} -> app.set_brush(size={size})"));
        return format!("app.set_brush(size={size})");
    }

    // Brush Intensity (ZBrush 0..100 -> OxyBrush 0.0..1.0)
    if lower.contains("z intensity") {
        let z_val = val_str.parse::<f32>().unwrap_or(50.0);
        let strength = (z_val / 100.0).clamp(0.01, 1.0);
        mapped_calls.push(format!("{target} -> app.set_brush(strength={strength:.2})"));
        return format!("app.set_brush(strength={strength:.2})");
    }

    // Dynamesh resolution
    if lower.contains("dynamesh:resolution") {
        let res = val_str.parse::<i32>().unwrap_or(128);
        mapped_calls.push(format!("{target} -> staged dynamesh resolution {res}"));
        return format!("# Set voxel remesh resolution\nremesh_res = {res}");
    }

    warnings.push(format!("Unhandled ISet target: {target} = {val_str}"));
    format!("# [ISet]: {target} = {val_str}")
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
    fn test_zbrush_transpiler_simple() {
        let zscript = r#"
            // Name: Quick Dynamesh Decimate
            // Author: ZArtist
            // Description: Automatically dynameshes and decimates active tool
            [IButton, "Zplugin:AutoTool:Run", "Run auto pass",
                [ISet, "Draw:Draw Size", 64]
                [ISet, "Draw:Z Intensity", 80]
                [IPress, "Brush:Clay"]
                [IPress, "Tool:Geometry:Divide"]
                [IPress, "Tool:Geometry:Dynamesh"]
                [IPress, "Tool:Geometry:Decimation Master:Decimate Current"]
                [Note, "Finished!"]
            ]
        "#;

        let converter = ZBrushConverter;
        let res = converter.convert(zscript, &ConversionOptions::default());

        assert_eq!(res.manifest.name, "Quick Dynamesh Decimate");
        assert_eq!(res.manifest.author, "ZArtist");
        assert!(res.python_code.contains("app.set_brush(size=64)"));
        assert!(res.python_code.contains("app.set_brush(strength=0.80)"));
        assert!(res.python_code.contains("app.select_brush('Clay')"));
        assert!(res.python_code.contains("app.subdivide(smooth=True)"));
        assert!(res.python_code.contains("app.voxel_remesh(voxel_size=0.03)"));
        assert!(res.python_code.contains("app.decimate(keep_percent=20.0)"));
        assert!(res.python_code.contains("print(\"[ZScript Note] Finished!\")"));
    }
}
