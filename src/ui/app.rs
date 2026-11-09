//! Main desktop GUI application for OxyConvert.

use crate::converters::{
    convert_script, detect_ecosystem, ConversionOptions, ConversionResult,
};
use crate::package::bundle::AddonPackage;
use crate::package::installer::{
    install_addon, list_installed_addons, oxybrush_plugins_dir, uninstall_addon,
    InstalledAddonInfo,
};
use crate::package::manifest::{AddonCategory, AddonManifest, SourceEcosystem};
use crate::ui::theme::{apply_oxy_theme, AccentPreset, ThemePreset};
use crate::ui::widgets::{self, primary_button, section_header};
use egui::{Color32, Margin, RichText, Ui, vec2};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightTab {
    PythonCode,
    Manifest,
    Report,
    InstalledManager,
}

pub struct OxyConvertApp {
    // Theme & styling
    pub theme: ThemePreset,
    pub accent: AccentPreset,

    // Source script state
    pub source_path: Option<PathBuf>,
    pub source_code: String,
    pub selected_ecosystem: SourceEcosystem,
    pub auto_detect: bool,

    // Converted result
    pub conversion: Option<ConversionResult>,

    // Manifest edit state
    pub manifest_draft: AddonManifest,
    pub raw_manifest_mode: bool,
    pub raw_manifest_text: String,

    // UI state
    pub right_tab: RightTab,
    pub status_message: String,
    pub is_error_status: bool,
    pub show_about_dialog: bool,
    pub show_spec_dialog: bool,

    // Installed addons cache
    pub installed_addons: Vec<InstalledAddonInfo>,
}

impl Default for OxyConvertApp {
    fn default() -> Self {
        let default_source = sample_zbrush_script();
        let detected = detect_ecosystem(&default_source, Some("sample.txt"));
        let res = convert_script(&default_source, detected, &ConversionOptions::default());
        let manifest_text = serde_json::to_string_pretty(&res.manifest).unwrap_or_default();

        let mut app = Self {
            theme: ThemePreset::Graphite,
            accent: AccentPreset::ClayOrange,
            source_path: None,
            source_code: default_source,
            selected_ecosystem: detected,
            auto_detect: true,
            conversion: Some(res.clone()),
            manifest_draft: res.manifest,
            raw_manifest_mode: false,
            raw_manifest_text: manifest_text,
            right_tab: RightTab::PythonCode,
            status_message: "Ready. Loaded example ZBrush script.".to_string(),
            is_error_status: false,
            show_about_dialog: false,
            show_spec_dialog: false,
            installed_addons: Vec::new(),
        };

        app.refresh_installed();
        app
    }
}

impl OxyConvertApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    pub fn refresh_installed(&mut self) {
        if let Ok(list) = list_installed_addons() {
            self.installed_addons = list;
        }
    }

    pub fn trigger_conversion(&mut self) {
        let ecosystem = if self.auto_detect {
            let filename = self.source_path.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str());
            let detected = detect_ecosystem(&self.source_code, filename);
            self.selected_ecosystem = detected;
            detected
        } else {
            self.selected_ecosystem
        };

        let mut options = ConversionOptions::default();
        if !self.manifest_draft.name.is_empty() && self.manifest_draft.name != "Untitled Addon" {
            options.addon_name = Some(self.manifest_draft.name.clone());
        }
        if !self.manifest_draft.author.is_empty() && self.manifest_draft.author != "Unknown" {
            options.addon_author = Some(self.manifest_draft.author.clone());
        }

        let res = convert_script(&self.source_code, ecosystem, &options);
        self.raw_manifest_text = serde_json::to_string_pretty(&res.manifest).unwrap_or_default();
        self.manifest_draft = res.manifest.clone();

        let count = res.mapped_calls.len();
        self.status_message = format!(
            "Converted {} script to .oxyaddon (mapped {} operations).",
            ecosystem.display_name(),
            count
        );
        self.is_error_status = false;
        self.conversion = Some(res);
    }

    pub fn load_file(&mut self, path: PathBuf) {
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                self.source_path = Some(path.clone());
                self.source_code = content;
                self.trigger_conversion();
                self.status_message = format!("Loaded {}", path.display());
                self.is_error_status = false;
            }
            Err(e) => {
                self.status_message = format!("Failed to read file: {e}");
                self.is_error_status = true;
            }
        }
    }

    pub fn save_oxyaddon(&mut self, target_path: PathBuf) {
        let Some(res) = &self.conversion else {
            self.status_message = "No converted addon to save.".into();
            self.is_error_status = true;
            return;
        };

        let manifest = if self.raw_manifest_mode {
            match serde_json::from_str::<AddonManifest>(&self.raw_manifest_text) {
                Ok(m) => m,
                Err(e) => {
                    self.status_message = format!("Manifest JSON error: {e}");
                    self.is_error_status = true;
                    return;
                }
            }
        } else {
            self.manifest_draft.clone()
        };

        let package = AddonPackage::new(manifest, &res.python_code);
        match package.save_to_archive(&target_path) {
            Ok(()) => {
                self.status_message = format!("Saved package to {}", target_path.display());
                self.is_error_status = false;
            }
            Err(e) => {
                self.status_message = format!("Failed to save package: {e}");
                self.is_error_status = true;
            }
        }
    }

    pub fn install_to_oxybrush(&mut self) {
        let Some(res) = &self.conversion else {
            self.status_message = "No converted addon to install.".into();
            self.is_error_status = true;
            return;
        };

        let manifest = if self.raw_manifest_mode {
            match serde_json::from_str::<AddonManifest>(&self.raw_manifest_text) {
                Ok(m) => m,
                Err(e) => {
                    self.status_message = format!("Manifest JSON error: {e}");
                    self.is_error_status = true;
                    return;
                }
            }
        } else {
            self.manifest_draft.clone()
        };

        let package = AddonPackage::new(manifest, &res.python_code);
        match install_addon(&package, false, true) {
            Ok(p) => {
                self.status_message = format!(
                    "Successfully installed '{}' to OxyBrush at {}",
                    package.manifest.name,
                    p.display()
                );
                self.is_error_status = false;
                self.refresh_installed();
                self.right_tab = RightTab::InstalledManager;
            }
            Err(e) => {
                self.status_message = format!("Installation failed: {e}");
                self.is_error_status = true;
            }
        }
    }
}

impl eframe::App for OxyConvertApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        apply_oxy_theme(&ctx, self.theme, self.accent);

        // Top menu bar
        egui::Panel::top("menu_bar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open Source Script... (Ctrl+O)").clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("Scripts", &["txt", "zsc", "py", "js", "json"])
                            .add_filter("All Files", &["*"])
                            .pick_file()
                        {
                            self.load_file(path);
                        }

                    ui.separator();
                    ui.label(RichText::new("Load Examples:").weak());
                    if ui.button("ZBrush: Dynamesh & Decimate").clicked() {
                        self.source_code = sample_zbrush_script();
                        self.source_path = None;
                        self.trigger_conversion();
                    }
                    if ui.button("Blender: Sculpt Remesh Tool").clicked() {
                        self.source_code = sample_blender_script();
                        self.source_path = None;
                        self.trigger_conversion();
                    }
                    if ui.button("Nomad: Organic Remesh Pass").clicked() {
                        self.source_code = sample_nomad_script();
                        self.source_path = None;
                        self.trigger_conversion();
                    }

                    ui.separator();
                    if ui.button("Save .oxyaddon Package As... (Ctrl+S)").clicked() {
                        let stem = self.manifest_draft.safe_stem();
                        if let Some(dest) = rfd::FileDialog::new()
                            .set_file_name(format!("{stem}.oxyaddon"))
                            .add_filter("OxyAddon Package", &["oxyaddon"])
                            .save_file()
                        {
                            self.save_oxyaddon(dest);
                        }
                    }

                    if ui.button("Install Directly to OxyBrush (Ctrl+I)").clicked() {
                        self.install_to_oxybrush();
                    }

                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("Convert", |ui| {
                    if ui.button("Convert Now (F5)").clicked() {
                        self.trigger_conversion();
                    }
                    ui.checkbox(&mut self.auto_detect, "Auto-detect source ecosystem");
                });

                ui.menu_button("Theme", |ui| {
                    ui.label(RichText::new("Visual Preset").strong());
                    for t in ThemePreset::ALL {
                        ui.selectable_value(&mut self.theme, t, t.display_name());
                    }
                    ui.separator();
                    ui.label(RichText::new("Accent Color").strong());
                    for a in AccentPreset::ALL {
                        ui.selectable_value(&mut self.accent, a, a.display_name());
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button(".oxyaddon Specification").clicked() {
                        self.show_spec_dialog = true;
                    }
                    if ui.button("About OxyConvert").clicked() {
                        self.show_about_dialog = true;
                    }
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("OxyConvert v0.1.0")
                            .color(Color32::from_gray(140))
                            .small(),
                    );
                });
            });
        });

        // Bottom status bar
        egui::Panel::bottom("status_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                let status_color = if self.is_error_status {
                    Color32::from_rgb(255, 100, 100)
                } else {
                    Color32::from_gray(190)
                };
                ui.label(RichText::new(&self.status_message).color(status_color));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(dir) = oxybrush_plugins_dir() {
                        let exists = dir.exists();
                        let dot = if exists { "●" } else { "○" };
                        let dot_col = if exists {
                            Color32::from_rgb(80, 220, 120)
                        } else {
                            Color32::from_rgb(220, 180, 80)
                        };
                        ui.label(
                            RichText::new(format!("{dot} OxyBrush Plugins: {}", dir.display()))
                                .color(dot_col)
                                .small(),
                        );
                    } else {
                        ui.label(RichText::new("OxyBrush Plugins: Not found").weak().small());
                    }
                });
            });
        });

        // Top toolbar
        egui::Panel::top("toolbar")
            .frame(
                egui::Frame::new()
                    .fill(ui.style().visuals.panel_fill)
                    .inner_margin(Margin::symmetric(10, 6)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("📂 Open Script...").clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("Scripts", &["txt", "zsc", "py", "js", "json"])
                            .pick_file()
                        {
                            self.load_file(path);
                        }

                    if primary_button(ui, "⚡ Convert to .oxyaddon", self.accent.color()).clicked() {
                        self.trigger_conversion();
                    }

                    if ui.button("💾 Save Package...").clicked() {
                        let stem = self.manifest_draft.safe_stem();
                        if let Some(dest) = rfd::FileDialog::new()
                            .set_file_name(format!("{stem}.oxyaddon"))
                            .add_filter("OxyAddon Package", &["oxyaddon"])
                            .save_file()
                        {
                            self.save_oxyaddon(dest);
                        }
                    }

                    if ui.button("🚀 Direct Install to OxyBrush").clicked() {
                        self.install_to_oxybrush();
                    }

                    ui.separator();
                    ui.label("Ecosystem:");
                    ui.checkbox(&mut self.auto_detect, "Auto");

                    if !self.auto_detect {
                        egui::ComboBox::from_id_salt("ecosystem_select")
                            .selected_text(self.selected_ecosystem.display_name())
                            .show_ui(ui, |ui| {
                                for eco in SourceEcosystem::ALL {
                                    if ui.selectable_value(&mut self.selected_ecosystem, eco, eco.display_name()).clicked() {
                                        self.trigger_conversion();
                                    }
                                }
                            });
                    } else {
                        widgets::badge_chip(
                            ui,
                            &format!("{} {}", self.selected_ecosystem.icon(), self.selected_ecosystem.display_name()),
                            Color32::from_gray(40),
                            self.accent.color(),
                        );
                    }
                });
            });

        // Central dual-pane split view
        egui::CentralPanel::default().show(ui, |ui| {
            let total_width = ui.available_width();
            let left_width = (total_width * 0.48).max(280.0);

            ui.horizontal(|ui| {
                // LEFT PANE: Source Script Editor
                ui.vertical(|ui| {
                    ui.set_width(left_width);
                    section_header(ui, "Source Script / Plugin", self.accent.color());

                    ui.horizontal(|ui| {
                        if let Some(p) = &self.source_path {
                            ui.label(RichText::new(p.file_name().unwrap().to_string_lossy()).strong());
                        } else {
                            ui.label(RichText::new("(Unsaved buffer / Example)").weak());
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(format!("{} lines", self.source_code.lines().count()));
                        });
                    });

                    ui.add_space(4.0);
                    egui::ScrollArea::vertical()
                        .id_salt("source_scroll")
                        .max_height(ui.available_height() - 8.0)
                        .show(ui, |ui| {
                            let text_edit = egui::TextEdit::multiline(&mut self.source_code)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .desired_rows(30)
                                .code_editor();
                            if ui.add(text_edit).changed() {
                                self.trigger_conversion();
                            }
                        });
                });

                ui.separator();

                // RIGHT PANE: .oxyaddon Studio & Diagnostics
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width());

                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.right_tab, RightTab::PythonCode, "🐍 main.py");
                        ui.selectable_value(&mut self.right_tab, RightTab::Manifest, "📋 manifest.json");
                        ui.selectable_value(&mut self.right_tab, RightTab::Report, "📊 Translation Report");
                        ui.selectable_value(
                            &mut self.right_tab,
                            RightTab::InstalledManager,
                            format!("📦 OxyBrush Addons ({})", self.installed_addons.len()),
                        );
                    });

                    ui.add_space(4.0);

                    match self.right_tab {
                        RightTab::PythonCode => self.render_python_code_view(ui),
                        RightTab::Manifest => self.render_manifest_view(ui),
                        RightTab::Report => self.render_report_view(ui),
                        RightTab::InstalledManager => self.render_installed_manager(ui),
                    }
                });
            });
        });

        // Dialogs
        let mut show_about = self.show_about_dialog;
        if show_about {
            let mut close_about = false;
            egui::Window::new("About OxyConvert")
                .collapsible(false)
                .resizable(false)
                .open(&mut show_about)
                .show(&ctx, |ui| {
                    ui.heading("OxyConvert");
                    ui.label("Universal Addon & Plugin Converter for the Oxy Ecosystem.");
                    ui.add_space(6.0);
                    ui.label("Transpiles scripts from ZBrush, Blender, and Nomad Sculpt into self-contained .oxyaddon packages running on the OxyBrush Python SDK.");
                    ui.add_space(8.0);
                    ui.label("Built with Rust & egui.");
                    ui.add_space(8.0);
                    if ui.button("Close").clicked() {
                        close_about = true;
                    }
                });
            self.show_about_dialog = show_about && !close_about;
        }

        let mut show_spec = self.show_spec_dialog;
        if show_spec {
            egui::Window::new(".oxyaddon Specification")
                .default_size(vec2(520.0, 400.0))
                .open(&mut show_spec)
                .show(&ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.heading(".oxyaddon Package Format");
                        ui.label("A .oxyaddon file is a standard ZIP archive containing:");
                        ui.monospace("├── manifest.json   (Metadata, ID, Author, Category)");
                        ui.monospace("├── main.py         (Pure-Python entrypoint using 'oxybrush')");
                        ui.monospace("├── icon.png        (Optional 64x64 or 128x128 icon)");
                        ui.monospace("└── lib/            (Optional additional modules/resources)");
                        ui.add_space(8.0);
                        ui.label("OxyBrush detects .oxyaddon files and extracted folders in:");
                        ui.label("• Windows: %APPDATA%\\oxybrush\\plugins\\ or %LOCALAPPDATA%\\OxyBrush\\plugins\\");
                        ui.label("• Linux: ~/.local/share/oxybrush/plugins/");
                        ui.label("• macOS: ~/Library/Application Support/oxybrush/plugins/");
                    });
                });
            self.show_spec_dialog = show_spec;
        }
    }
}

impl OxyConvertApp {
    fn render_python_code_view(&mut self, ui: &mut Ui) {
        if let Some(res) = &mut self.conversion {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Target Python Code (OxyBrush SDK):").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("📋 Copy Code").clicked() {
                        ui.ctx().copy_text(res.python_code.clone());
                        self.status_message = "Copied Python code to clipboard!".into();
                    }
                });
            });

            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .id_salt("py_scroll")
                .max_height(ui.available_height() - 10.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut res.python_code)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .code_editor(),
                    );
                });
        } else {
            ui.label("No conversion generated yet.");
        }
    }

    fn render_manifest_view(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Addon Manifest:").strong());
            ui.checkbox(&mut self.raw_manifest_mode, "Raw JSON Editor");
        });

        ui.add_space(4.0);

        if self.raw_manifest_mode {
            egui::ScrollArea::vertical()
                .id_salt("manifest_json_scroll")
                .max_height(ui.available_height() - 10.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.raw_manifest_text)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .code_editor(),
                    );
                });
        } else {
            egui::ScrollArea::vertical()
                .id_salt("manifest_form_scroll")
                .show(ui, |ui| {
                    egui::Grid::new("manifest_grid")
                        .num_columns(2)
                        .spacing([12.0, 8.0])
                        .show(ui, |ui| {
                            ui.label("Addon ID:");
                            ui.text_edit_singleline(&mut self.manifest_draft.id);
                            ui.end_row();

                            ui.label("Display Name:");
                            ui.text_edit_singleline(&mut self.manifest_draft.name);
                            ui.end_row();

                            ui.label("Version:");
                            ui.text_edit_singleline(&mut self.manifest_draft.version);
                            ui.end_row();

                            ui.label("Author:");
                            ui.text_edit_singleline(&mut self.manifest_draft.author);
                            ui.end_row();

                            ui.label("Category:");
                            egui::ComboBox::from_id_salt("cat_combo")
                                .selected_text(self.manifest_draft.category.display_name())
                                .show_ui(ui, |ui| {
                                    for c in AddonCategory::ALL {
                                        ui.selectable_value(&mut self.manifest_draft.category, c, c.display_name());
                                    }
                                });
                            ui.end_row();

                            ui.label("Description:");
                            ui.text_edit_multiline(&mut self.manifest_draft.description);
                            ui.end_row();

                            ui.label("Min OxyBrush Version:");
                            ui.text_edit_singleline(&mut self.manifest_draft.min_oxybrush_version);
                            ui.end_row();

                            ui.label("UI Menu Placement:");
                            ui.text_edit_singleline(&mut self.manifest_draft.ui.menu);
                            ui.end_row();
                        });
                });
        }
    }

    fn render_report_view(&mut self, ui: &mut Ui) {
        if let Some(res) = &self.conversion {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Conversion Diagnostics");
                ui.label(format!("Source Ecosystem: {}", res.source_ecosystem.display_name()));
                ui.add_space(6.0);

                if !res.warnings.is_empty() {
                    ui.label(RichText::new(format!("⚠ Warnings ({})", res.warnings.len())).strong().color(Color32::from_rgb(255, 180, 80)));
                    for w in &res.warnings {
                        ui.label(format!("• {w}"));
                    }
                    ui.add_space(8.0);
                }

                ui.label(RichText::new(format!("✔ Mapped API Operations ({})", res.mapped_calls.len())).strong().color(Color32::from_rgb(100, 220, 140)));
                for m in &res.mapped_calls {
                    ui.label(format!("• {m}"));
                }
            });
        }
    }

    fn render_installed_manager(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.heading("Installed OxyBrush Addons & Plugins");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🔄 Refresh").clicked() {
                    self.refresh_installed();
                }
                if let Some(dir) = oxybrush_plugins_dir()
                    && ui.button("📁 Open Plugins Folder").clicked() {
                        let _ = std::fs::create_dir_all(&dir);
                        #[cfg(target_os = "windows")]
                        let _ = std::process::Command::new("explorer").arg(&dir).spawn();
                        #[cfg(target_os = "macos")]
                        let _ = std::process::Command::new("open").arg(&dir).spawn();
                        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
                        let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
                    }
            });
        });

        ui.add_space(6.0);

        if self.installed_addons.is_empty() {
            ui.label("No addons found in OxyBrush plugins folder.");
        } else {
            let mut remove_path = None;
            egui::ScrollArea::vertical().show(ui, |ui| {
                for addon in &self.installed_addons {
                    egui::Frame::new()
                        .fill(ui.style().visuals.faint_bg_color)
                        .corner_radius(egui::CornerRadius::same(4))
                        .inner_margin(Margin::symmetric(8, 6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let badge = if addon.is_archive {
                                    ".oxyaddon"
                                } else if addon.is_directory {
                                    "folder"
                                } else {
                                    ".py script"
                                };
                                widgets::badge_chip(ui, badge, Color32::from_gray(30), self.accent.color());
                                ui.label(RichText::new(&addon.name).strong());

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button("🗑 Uninstall").clicked() {
                                        remove_path = Some(addon.path.clone());
                                    }
                                });
                            });
                        });
                    ui.add_space(3.0);
                }
            });

            if let Some(path) = remove_path
                && let Ok(()) = uninstall_addon(&path) {
                    self.status_message = format!("Removed {}", path.display());
                    self.refresh_installed();
                }
        }
    }
}

fn sample_zbrush_script() -> String {
    r#"// Name: Auto Dynamesh & Decimate Tool
// Author: DigitalSculptor
// Description: Prepares high-poly sculpts with instant remesh and decimation
[IButton, "Zplugin:OxyTools:AutoClean", "Remesh and decimate active subtool",
    [ISet, "Draw:Draw Size", 45]
    [ISet, "Draw:Z Intensity", 75]
    [IPress, "Brush:Clay"]
    [IPress, "Transform:Activate Symmetry"]
    [IPress, "Tool:Geometry:Divide"]
    [IPress, "Tool:Geometry:Dynamesh"]
    [IPress, "Tool:Geometry:Decimation Master:Decimate Current"]
    [Note, "Auto clean pass finished!"]
]
"#.to_string()
}

fn sample_blender_script() -> String {
    r#"bl_info = {
    "name": "Quick Sculpt Remesher",
    "author": "BlendArtist",
    "version": (1, 2, 0),
    "description": "Blender sculpt operator for automated voxel remesh",
    "category": "Sculpt",
}

import bpy

class SCULPT_OT_quick_remesh(bpy.types.Operator):
    bl_idname = "sculpt.quick_remesh"
    bl_label = "Quick Sculpt Remesh"

    def execute(self, context):
        context.object.data.remesh_voxel_size = 0.035
        bpy.ops.object.voxel_remesh()
        bpy.ops.object.subdivision_set(level=1)
        return {'FINISHED'}
"#.to_string()
}

fn sample_nomad_script() -> String {
    r#"// Name: Nomad Organic Sculpt Pass
// Author: MobileArtist
// Description: Configures clay brush, runs organic remesh and decimation
nomad.setBrush('clay');
nomad.symmetry = true;
nomad.topology.remesh(0.03);
nomad.decimate(30);
console.log("Nomad organic pass executed");
"#.to_string()
}
