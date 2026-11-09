//! Manifest definitions for the `.oxyaddon` packaging system.

use serde::{Deserialize, Serialize};

/// Categories of addons supported by the Oxy ecosystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AddonCategory {
    #[default]
    Sculpting,
    Topology,
    Pipeline,
    Procedural,
    Materials,
    Utility,
}

impl AddonCategory {
    pub const ALL: [AddonCategory; 6] = [
        AddonCategory::Sculpting,
        AddonCategory::Topology,
        AddonCategory::Pipeline,
        AddonCategory::Procedural,
        AddonCategory::Materials,
        AddonCategory::Utility,
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Sculpting => "Sculpting",
            Self::Topology => "Topology & Remeshing",
            Self::Pipeline => "Pipeline & I/O",
            Self::Procedural => "Procedural Generation",
            Self::Materials => "Materials & Matcaps",
            Self::Utility => "Utility & Tools",
        }
    }
}

/// The original ecosystem a script was created for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SourceEcosystem {
    #[default]
    Native,
    Zbrush,
    Blender,
    Nomad,
}

impl SourceEcosystem {
    pub const ALL: [SourceEcosystem; 4] = [
        SourceEcosystem::Native,
        SourceEcosystem::Zbrush,
        SourceEcosystem::Blender,
        SourceEcosystem::Nomad,
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Native => "OxyBrush Native",
            Self::Zbrush => "ZBrush ZScript / ZPlugin",
            Self::Blender => "Blender Python (bpy)",
            Self::Nomad => "Nomad Sculpt (JS / Preset)",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Native => "🖌",
            Self::Zbrush => "🗿",
            Self::Blender => "⚡",
            Self::Nomad => "📱",
        }
    }
}

/// Optional in-app UI integration metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UiBinding {
    #[serde(default = "default_menu")]
    pub menu: String,
    #[serde(default)]
    pub submenu: Option<String>,
    #[serde(default)]
    pub button_label: Option<String>,
    #[serde(default)]
    pub tooltip: Option<String>,
    #[serde(default)]
    pub shortcut: Option<String>,
}

fn default_menu() -> String {
    "Plugins".to_string()
}

/// Top-level `.oxyaddon` manifest format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddonManifest {
    #[serde(default = "default_schema")]
    pub schema: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    #[serde(default)]
    pub category: AddonCategory,
    #[serde(default = "default_target_app")]
    pub target_app: String,
    #[serde(default = "default_entry_point")]
    pub entry_point: String,
    #[serde(default)]
    pub source_ecosystem: SourceEcosystem,
    #[serde(default = "default_min_version")]
    pub min_oxybrush_version: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub ui: UiBinding,
}

fn default_schema() -> String {
    "https://raw.githubusercontent.com/crisprintsstuff/OxyBrush/main/schemas/oxyaddon-v1.json".to_string()
}

fn default_target_app() -> String {
    "OxyBrush".to_string()
}

fn default_entry_point() -> String {
    "main.py".to_string()
}

fn default_min_version() -> String {
    "0.5.0".to_string()
}

impl Default for AddonManifest {
    fn default() -> Self {
        Self {
            schema: default_schema(),
            id: "com.example.addon".to_string(),
            name: "Untitled Addon".to_string(),
            version: "1.0.0".to_string(),
            author: "Unknown".to_string(),
            description: "No description provided.".to_string(),
            category: AddonCategory::Sculpting,
            target_app: default_target_app(),
            entry_point: default_entry_point(),
            source_ecosystem: SourceEcosystem::Native,
            min_oxybrush_version: default_min_version(),
            tags: Vec::new(),
            permissions: vec!["scene:read".into(), "scene:modify".into()],
            homepage: None,
            license: Some("MIT".into()),
            ui: UiBinding::default(),
        }
    }
}

impl AddonManifest {
    /// Format id into a safe filename/directory stem.
    pub fn safe_stem(&self) -> String {
        // If id is empty or default generic id, use the addon's unique name
        let raw = if self.id.is_empty() || self.id == "com.example.addon" || self.id == "com_example_addon" {
            if !self.name.is_empty() && self.name != "Untitled Addon" {
                &self.name
            } else {
                &self.id
            }
        } else {
            &self.id
        };
        let stem: String = raw.chars()
            .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
            .collect::<String>()
            .trim_matches('_')
            .to_lowercase();
        if stem.is_empty() {
            "addon".to_string()
        } else {
            stem
        }
    }

    /// Generate native OxyBrush `plugin.json` content matching `oxy_plugin::addon::inspect`.
    pub fn to_plugin_json(&self) -> String {
        let stem = self.safe_stem();
        let value = serde_json::json!({
            "name": self.name,
            "id": stem,
            "kind": "python",
            "entry": self.entry_point,
            "version": self.version,
            "description": self.description,
            "author": self.author,
            "category": self.category.display_name(),
        });
        serde_json::to_string_pretty(&value).unwrap_or_default()
    }
}
