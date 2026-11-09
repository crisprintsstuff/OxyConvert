//! Script and plugin converters for ZBrush, Blender, and Nomad Sculpt.

pub mod blender;
pub mod nomad;
pub mod wasm;
pub mod zbrush;

use crate::package::manifest::{AddonCategory, AddonManifest, SourceEcosystem};

/// Options for fine-tuning the conversion process.
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct ConversionOptions {
    pub addon_id: Option<String>,
    pub addon_name: Option<String>,
    pub addon_author: Option<String>,
    pub addon_version: Option<String>,
    pub addon_description: Option<String>,
    pub category: Option<AddonCategory>,
    pub include_original_as_comment: bool,
    pub runtime: wasm::PluginRuntime,
}


/// Result of converting an external script to `.oxyaddon`.
#[derive(Debug, Clone)]
pub struct ConversionResult {
    pub manifest: AddonManifest,
    pub python_code: String,
    pub wasm_definition: Option<wasm::WasmPluginDefinition>,
    pub warnings: Vec<String>,
    pub mapped_calls: Vec<String>,
    pub source_ecosystem: SourceEcosystem,
}

/// Common trait implemented by all format transpilers.
pub trait ScriptConverter {
    fn convert(&self, source: &str, options: &ConversionOptions) -> ConversionResult;
}

/// Inspect the entries inside a .zip archive to detect whether it is ZBrush, Blender, Nomad, or Native.
pub fn detect_zip_ecosystem(path: &std::path::Path) -> SourceEcosystem {
    let Ok(file) = std::fs::File::open(path) else {
        return SourceEcosystem::Native;
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return SourceEcosystem::Native;
    };

    let mut has_zsc_or_zbrush = false;
    let mut has_manifest = false;
    let mut has_bpy = false;
    let mut has_nomad = false;

    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            let name = entry.name().to_lowercase();
            if name == "manifest.json" {
                has_manifest = true;
            } else if name.ends_with(".zsc")
                || name.contains("zbrush")
                || name.contains("zstartup")
                || name.contains("zplugs")
                || name.ends_with(".zmt")
            {
                has_zsc_or_zbrush = true;
            } else if name.ends_with(".py") || name.contains("bl_info") {
                has_bpy = true;
            } else if name.ends_with(".js") || name.contains("nomad") {
                has_nomad = true;
            }
        }
    }

    if has_manifest {
        SourceEcosystem::Native
    } else if has_zsc_or_zbrush {
        SourceEcosystem::Zbrush
    } else if has_bpy {
        SourceEcosystem::Blender
    } else if has_nomad {
        SourceEcosystem::Nomad
    } else {
        SourceEcosystem::Native
    }
}

/// Heuristically auto-detect which software a script or plugin came from.
pub fn detect_ecosystem(source: &str, filename: Option<&str>) -> SourceEcosystem {
    // 1. Check file extension if provided
    if let Some(name) = filename {
        let lower = name.to_lowercase();
        if lower.ends_with(".zsc") {
            return SourceEcosystem::Zbrush;
        }
        if lower.ends_with(".txt") {
            // Check if txt contains ZScript syntax
            if source.contains("[IButton") || source.contains("[IPress") || source.contains("[VarSet") || source.contains("ZBrush") {
                return SourceEcosystem::Zbrush;
            }
        } else if lower.ends_with(".js") || (lower.ends_with(".json") && source.contains("nomad")) {
            return SourceEcosystem::Nomad;
        } else if lower.ends_with(".py")
            && (source.contains("bpy") || source.contains("bl_info")) {
                return SourceEcosystem::Blender;
            }
    }

    // 2. Content-based heuristics
    if source.contains("[IButton")
        || source.contains("[IPress")
        || source.contains("[ISet")
        || source.contains("[SubToolSelect")
        || source.contains("[ZBrush")
    {
        return SourceEcosystem::Zbrush;
    }

    if source.contains("bl_info")
        || source.contains("bpy.ops")
        || source.contains("bpy.context")
        || source.contains("bpy.types.Operator")
        || source.contains("import bpy")
    {
        return SourceEcosystem::Blender;
    }

    if source.contains("nomad.")
        || source.contains("nomad.remesh")
        || source.contains("nomad.sculpt")
        || source.contains("nomad.tools")
        || (source.trim_start().starts_with('{') && source.contains("\"tool\":"))
    {
        return SourceEcosystem::Nomad;
    }

    SourceEcosystem::Native
}

/// Universal converter dispatching to the right converter based on ecosystem.
pub fn convert_script(
    source: &str,
    ecosystem: SourceEcosystem,
    options: &ConversionOptions,
) -> ConversionResult {
    match ecosystem {
        SourceEcosystem::Zbrush => zbrush::ZBrushConverter.convert(source, options),
        SourceEcosystem::Blender => blender::BlenderConverter.convert(source, options),
        SourceEcosystem::Nomad => nomad::NomadConverter.convert(source, options),
        SourceEcosystem::Native => {
            // Already native python: extract header metadata if present
            let mut manifest = AddonManifest {
                source_ecosystem: SourceEcosystem::Native,
                ..Default::default()
            };
            if let Some(id) = &options.addon_id {
                manifest.id = id.clone();
            }
            if let Some(name) = &options.addon_name {
                manifest.name = name.clone();
            }
            if let Some(auth) = &options.addon_author {
                manifest.author = auth.clone();
            }
            if let Some(desc) = &options.addon_description {
                manifest.description = desc.clone();
            }
            if let Some(cat) = options.category {
                manifest.category = cat;
            }

            let wasm_def = if options.runtime == wasm::PluginRuntime::Wasm {
                Some(wasm::WasmPluginDefinition::from_source(source, &manifest))
            } else {
                None
            };

            ConversionResult {
                manifest,
                python_code: source.to_string(),
                wasm_definition: wasm_def,
                warnings: Vec::new(),
                mapped_calls: vec!["Native OxyBrush SDK script passed through".to_string()],
                source_ecosystem: SourceEcosystem::Native,
            }
        }
    }
}
