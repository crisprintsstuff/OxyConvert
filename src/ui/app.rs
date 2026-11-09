//! Main desktop GUI application for OxyConvert.

use crate::converters::blender::convert_blender_zip;
use crate::converters::detect_zip_ecosystem;
use crate::converters::zbrush::{convert_zbrush_zip, convert_zbrush_zsc};
use crate::converters::{ConversionOptions, ConversionResult, convert_script, detect_ecosystem};
use crate::package::bundle::AddonPackage;
use crate::package::installer::{
    InstalledAddonInfo, install_addon, list_installed_addons, oxybrush_plugins_dir, uninstall_addon,
};
use crate::package::manifest::{AddonCategory, AddonManifest, SourceEcosystem};
use crate::ui::file_dialog::InAppFileDialog;
use crate::ui::theme::{AccentPreset, ThemePreset, apply_oxy_theme};
use crate::ui::widgets::{self, primary_button, section_header};
use egui::{Color32, Margin, RichText, Ui, vec2};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightTab {
    PythonCode,
    WasmCode,
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
    pub source_path_input: String,
    pub source_code: String,
    pub selected_ecosystem: SourceEcosystem,
    pub auto_detect: bool,

    // Converted result
    pub conversion: Option<ConversionResult>,
    pub extra_files: std::collections::BTreeMap<String, Vec<u8>>,

    // Manifest edit state
    pub manifest_draft: AddonManifest,
    pub raw_manifest_mode: bool,
    pub raw_manifest_text: String,

    pub runtime_mode: crate::converters::wasm::PluginRuntime,

    // UI state
    pub right_tab: RightTab,
    pub status_message: String,
    pub is_error_status: bool,
    pub show_about_dialog: bool,
    pub show_spec_dialog: bool,
    pub file_dialog: InAppFileDialog,

    // Installed addons cache
    pub installed_addons: Vec<InstalledAddonInfo>,

    // Suite API loopback server
    pub api_server: Option<crate::api::ServerHandle>,
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
            source_path_input: String::new(),
            source_code: default_source,
            selected_ecosystem: detected,
            auto_detect: true,
            conversion: Some(res.clone()),
            extra_files: std::collections::BTreeMap::new(),
            manifest_draft: res.manifest,
            raw_manifest_mode: false,
            raw_manifest_text: manifest_text,
            runtime_mode: crate::converters::wasm::PluginRuntime::Python,
            right_tab: RightTab::PythonCode,
            status_message: "Ready. Loaded example ZBrush script.".to_string(),
            is_error_status: false,
            show_about_dialog: false,
            show_spec_dialog: false,
            file_dialog: InAppFileDialog::default(),
            installed_addons: Vec::new(),
            api_server: crate::api::start_server(8990, None, true)
                .or_else(|_| crate::api::start_server(0, None, true))
                .ok(),
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
            let filename = self
                .source_path
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str());
            let detected = detect_ecosystem(&self.source_code, filename);
            self.selected_ecosystem = detected;
            detected
        } else {
            self.selected_ecosystem
        };

        let mut options = ConversionOptions {
            runtime: self.runtime_mode,
            ..Default::default()
        };
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

    /// Load any supported file: .oxyaddon, .zip, or text script (.txt, .zsc, .py, .js, .json).
    pub fn load_file(&mut self, path: PathBuf) {
        self.source_path_input = path.to_string_lossy().into_owned();

        if !path.exists() {
            self.status_message = format!("File does not exist: {}", path.display());
            self.is_error_status = true;
            return;
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // 1. Direct .oxyaddon archive import
        if ext == "oxyaddon" || (path.is_dir() && path.join("manifest.json").is_file()) {
            match AddonPackage::open(&path) {
                Ok(pkg) => {
                    self.source_path = Some(path.clone());
                    self.source_code = pkg.main_script.clone();
                    self.extra_files = pkg.extra_files.clone();
                    self.manifest_draft = pkg.manifest.clone();
                    self.raw_manifest_text =
                        serde_json::to_string_pretty(&pkg.manifest).unwrap_or_default();
                    self.selected_ecosystem = pkg.manifest.source_ecosystem;
                    let wasm_def = pkg.wat_script.map(|w| {
                        crate::converters::wasm::WasmPluginDefinition::from_source(
                            &w,
                            &pkg.manifest,
                        )
                    });
                    self.conversion = Some(ConversionResult {
                        manifest: pkg.manifest.clone(),
                        python_code: pkg.main_script,
                        wasm_definition: wasm_def,
                        warnings: Vec::new(),
                        mapped_calls: vec!["Opened native .oxyaddon package".into()],
                        source_ecosystem: pkg.manifest.source_ecosystem,
                    });
                    self.status_message = format!(
                        "Successfully imported .oxyaddon package: '{}' ({})",
                        pkg.manifest.name, pkg.manifest.id
                    );
                    self.is_error_status = false;
                }
                Err(e) => {
                    self.status_message = format!("Failed to open .oxyaddon package: {e}");
                    self.is_error_status = true;
                }
            }
            return;
        }

        // 2. Generic .zip package: check if ZBrush, Blender, Nomad, or Native
        if ext == "zip" {
            let options = ConversionOptions::default();
            let eco = detect_zip_ecosystem(&path);
            match eco {
                SourceEcosystem::Zbrush => match convert_zbrush_zip(&path, &options) {
                    Ok((res, extra)) => {
                        self.source_path = Some(path.clone());
                        self.source_code = res.python_code.clone();
                        self.extra_files = extra;
                        self.manifest_draft = res.manifest.clone();
                        self.raw_manifest_text =
                            serde_json::to_string_pretty(&res.manifest).unwrap_or_default();
                        self.selected_ecosystem = SourceEcosystem::Zbrush;
                        self.status_message = format!(
                            "Successfully converted ZBrush zip plugin: '{}' (bundled {} companion assets)",
                            res.manifest.name,
                            self.extra_files.len()
                        );
                        self.is_error_status = false;
                        self.conversion = Some(res);
                    }
                    Err(e) => {
                        self.status_message = format!("Failed to convert ZBrush zip: {e}");
                        self.is_error_status = true;
                    }
                },
                SourceEcosystem::Blender => match convert_blender_zip(&path, &options) {
                    Ok(res) => {
                        self.source_path = Some(path.clone());
                        self.source_code = res.python_code.clone();
                        self.extra_files.clear();
                        self.manifest_draft = res.manifest.clone();
                        self.raw_manifest_text =
                            serde_json::to_string_pretty(&res.manifest).unwrap_or_default();
                        self.selected_ecosystem = SourceEcosystem::Blender;
                        self.status_message = format!(
                            "Successfully converted Blender zip addon: '{}'",
                            res.manifest.name
                        );
                        self.is_error_status = false;
                        self.conversion = Some(res);
                    }
                    Err(e) => {
                        self.status_message = format!("Failed to convert Blender zip: {e}");
                        self.is_error_status = true;
                    }
                },
                _ => {
                    // Try AddonPackage or ZBrush fallback
                    if let Ok(pkg) = AddonPackage::open(&path) {
                        self.source_path = Some(path.clone());
                        self.source_code = pkg.main_script.clone();
                        self.extra_files = pkg.extra_files.clone();
                        self.manifest_draft = pkg.manifest.clone();
                        self.raw_manifest_text =
                            serde_json::to_string_pretty(&pkg.manifest).unwrap_or_default();
                        self.selected_ecosystem = pkg.manifest.source_ecosystem;
                        self.status_message = format!("Opened package: '{}'", pkg.manifest.name);
                        self.is_error_status = false;
                        let wasm_def = pkg.wat_script.map(|w| {
                            crate::converters::wasm::WasmPluginDefinition::from_source(
                                &w,
                                &pkg.manifest,
                            )
                        });
                        self.conversion = Some(ConversionResult {
                            manifest: pkg.manifest,
                            python_code: pkg.main_script,
                            wasm_definition: wasm_def,
                            warnings: Vec::new(),
                            mapped_calls: vec!["Opened package".into()],
                            source_ecosystem: SourceEcosystem::Native,
                        });
                    } else if let Ok((res, extra)) = convert_zbrush_zip(&path, &options) {
                        self.source_path = Some(path.clone());
                        self.source_code = res.python_code.clone();
                        self.extra_files = extra;
                        self.manifest_draft = res.manifest.clone();
                        self.raw_manifest_text =
                            serde_json::to_string_pretty(&res.manifest).unwrap_or_default();
                        self.selected_ecosystem = SourceEcosystem::Zbrush;
                        self.status_message =
                            format!("Converted ZBrush archive: '{}'", res.manifest.name);
                        self.is_error_status = false;
                        self.conversion = Some(res);
                    } else {
                        self.status_message =
                            "Could not parse zip archive. No supported plugin files found inside."
                                .into();
                        self.is_error_status = true;
                    }
                }
            }
            return;
        }

        // 3. Compiled ZBrush binary (.zsc)
        if ext == "zsc" {
            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("plugin.zsc");
            match fs::read(&path) {
                Ok(bytes) => {
                    let options = ConversionOptions::default();
                    let res = convert_zbrush_zsc(&bytes, filename, &options);
                    self.source_path = Some(path.clone());
                    self.source_code = res.python_code.clone();
                    self.extra_files.clear();
                    self.manifest_draft = res.manifest.clone();
                    self.raw_manifest_text =
                        serde_json::to_string_pretty(&res.manifest).unwrap_or_default();
                    self.selected_ecosystem = SourceEcosystem::Zbrush;
                    self.status_message = format!(
                        "Converted compiled ZBrush plugin (.zsc): '{}'",
                        res.manifest.name
                    );
                    self.is_error_status = false;
                    self.conversion = Some(res);
                }
                Err(e) => {
                    self.status_message = format!("Failed to read .zsc file: {e}");
                    self.is_error_status = true;
                }
            }
            return;
        }

        // 4. Regular script (.txt, .py, .js, .json)
        match fs::read(&path) {
            Ok(bytes) => {
                let content = String::from_utf8_lossy(&bytes).into_owned();
                self.source_path = Some(path.clone());
                self.source_code = content;

                // Reset manifest draft name/author so trigger_conversion derives unique info from new file
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let formatted = format_clean_name(stem);
                    self.manifest_draft.name = formatted;
                    self.manifest_draft.id.clear();
                }

                self.trigger_conversion();
                self.status_message = format!("Successfully imported {}", path.display());
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

        let mut package = AddonPackage::new(manifest, &res.python_code);
        if self.runtime_mode == crate::converters::wasm::PluginRuntime::Wasm
            && let Some(def) = &res.wasm_definition
        {
            package.wat_script = Some(def.wat_code.clone());
        }
        package.extra_files = self.extra_files.clone();
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

        let mut package = AddonPackage::new(manifest, &res.python_code);
        if self.runtime_mode == crate::converters::wasm::PluginRuntime::Wasm
            && let Some(def) = &res.wasm_definition
        {
            package.wat_script = Some(def.wat_code.clone());
        }
        package.extra_files = self.extra_files.clone();
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

        // Handle Drag & Drop files directly onto the window
        ctx.input(|i| {
            for dropped in &i.raw.dropped_files {
                let path = dropped.path();
                if !path.as_os_str().is_empty() && path.exists() {
                    self.load_file(path.to_path_buf());
                } else if let Ok(bytes) = dropped.bytes() {
                    let text = String::from_utf8_lossy(&bytes).into_owned();
                    self.source_code = text;
                    self.source_path = None;
                    self.trigger_conversion();
                }
            }
        });

        // Top menu bar (OxyBrush style: File, Convert, View/Appearance, Help)
        egui::Panel::top("menu_bar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open / Import…\tCtrl+O").clicked() {
                        self.file_dialog = InAppFileDialog::open_file();
                        ui.close();
                    }

                    if ui.button("System File Browser…").clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter(
                                "All Plugins & Scripts",
                                &["txt", "zsc", "py", "js", "json", "oxyaddon", "zip"],
                            )
                            .pick_file()
                    {
                        self.load_file(path);
                        ui.close();
                    }

                    ui.menu_button("Load Example", |ui| {
                        if ui.button("ZBrush: Dynamesh & Decimate").clicked() {
                            self.source_code = sample_zbrush_script();
                            self.source_path = None;
                            self.source_path_input = "example:zbrush".into();
                            self.manifest_draft.name = "Dynamesh Decimate".into();
                            self.manifest_draft.id.clear();
                            self.trigger_conversion();
                            ui.close();
                        }
                        if ui.button("Blender: Sculpt Remesh Tool").clicked() {
                            self.source_code = sample_blender_script();
                            self.source_path = None;
                            self.source_path_input = "example:blender".into();
                            self.manifest_draft.name = "Quick Sculpt Remesh".into();
                            self.manifest_draft.id.clear();
                            self.trigger_conversion();
                            ui.close();
                        }
                        if ui.button("Nomad: Organic Remesh Pass").clicked() {
                            self.source_code = sample_nomad_script();
                            self.source_path = None;
                            self.source_path_input = "example:nomad_js".into();
                            self.manifest_draft.name = "Nomad Organic Sculpt Pass".into();
                            self.manifest_draft.id.clear();
                            self.trigger_conversion();
                            ui.close();
                        }
                        if ui.button("Nomad: Heavy Clay Preset").clicked() {
                            self.source_code = sample_nomad_json();
                            self.source_path = None;
                            self.source_path_input = "example:nomad_json".into();
                            self.manifest_draft.name = "Heavy Clay Tool".into();
                            self.manifest_draft.id.clear();
                            self.trigger_conversion();
                            ui.close();
                        }
                    });

                    ui.separator();
                    if ui.button("Save .oxyaddon Package As…\tCtrl+S").clicked() {
                        let stem = self.manifest_draft.safe_stem();
                        self.file_dialog = InAppFileDialog::save_file(&format!("{stem}.oxyaddon"));
                        ui.close();
                    }

                    if ui.button("Install to OxyBrush\tCtrl+I").clicked() {
                        self.install_to_oxybrush();
                        ui.close();
                    }

                    ui.separator();
                    if ui.button("Quit\tCtrl+Q").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("Convert", |ui| {
                    if ui.button("Convert Now\tF5").clicked() {
                        self.trigger_conversion();
                        ui.close();
                    }
                    ui.checkbox(&mut self.auto_detect, "Auto-detect ecosystem");
                    ui.separator();
                    ui.label(RichText::new("Force Ecosystem").weak());
                    for eco in SourceEcosystem::ALL {
                        if ui
                            .selectable_value(&mut self.selected_ecosystem, eco, eco.display_name())
                            .clicked()
                        {
                            self.auto_detect = false;
                            self.trigger_conversion();
                            ui.close();
                        }
                    }
                });

                ui.menu_button("Appearance", |ui| {
                    ui.label(RichText::new("Theme Preset").strong());
                    for t in ThemePreset::ALL {
                        ui.selectable_value(&mut self.theme, t, t.display_name());
                    }
                    ui.separator();
                    ui.label(RichText::new("Accent Color").strong());
                    for a in AccentPreset::ALL {
                        ui.selectable_value(&mut self.accent, a, a.display_name());
                    }
                });

                ui.menu_button("Plugins", |ui| {
                    if ui.button("Refresh Installed List").clicked() {
                        self.refresh_installed();
                        self.right_tab = RightTab::InstalledManager;
                        ui.close();
                    }
                    if let Some(dir) = oxybrush_plugins_dir()
                        && ui.button("Open OxyBrush Plugins Folder…").clicked()
                    {
                        let _ = rfd::FileDialog::new().set_directory(&dir).pick_folder();
                        ui.close();
                    }
                });

                ui.menu_button("Suite", |ui| {
                    if let Some(srv) = &self.api_server {
                        ui.label(RichText::new("OxyLabs Suite API Server").strong());
                        ui.label(
                            RichText::new(format!("Endpoint: {}", srv.endpoint))
                                .monospace()
                                .small(),
                        );
                        if ui.button("📋 Copy API URL").clicked() {
                            ui.ctx().copy_text(srv.endpoint.clone());
                            self.status_message =
                                format!("Copied API URL {} to clipboard", srv.endpoint);
                            ui.close();
                        }
                    } else {
                        ui.label(RichText::new("API Server: Inactive").weak());
                        if ui.button("Start Loopback API").clicked() {
                            self.api_server = crate::api::start_server(8990, None, true).ok();
                            ui.close();
                        }
                    }
                    ui.separator();
                    ui.label(
                        RichText::new(
                            "Connected Suite Apps: OxyBrush, OxyLauncher, OxyPixel, OxyChar",
                        )
                        .weak()
                        .small(),
                    );
                });

                ui.menu_button("Help", |ui| {
                    if ui.button(".oxyaddon Specification…").clicked() {
                        self.show_spec_dialog = true;
                        ui.close();
                    }
                    if ui.button("About OxyConvert…").clicked() {
                        self.show_about_dialog = true;
                        ui.close();
                    }
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("OxyConvert 0.1.1")
                            .color(Color32::from_gray(130))
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
                    if let Some(srv) = &self.api_server {
                        ui.label(
                            RichText::new(format!("● Suite API: {}", srv.endpoint))
                                .color(Color32::from_rgb(80, 220, 120))
                                .small(),
                        );
                        ui.separator();
                    }

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

        // Top toolbar (matching OxyBrush toolbar layout and spacing)
        egui::Panel::top("toolbar")
            .frame(
                egui::Frame::new()
                    .fill(ui.style().visuals.panel_fill)
                    .inner_margin(Margin::symmetric(10, 6)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .button("Open / Import…")
                        .on_hover_text("Open or import a script, .zsc, or .zip archive")
                        .clicked()
                    {
                        self.file_dialog = InAppFileDialog::open_file();
                    }

                    if primary_button(ui, "Convert to .oxyaddon", self.accent.color())
                        .on_hover_text("Transpile active script to OxyBrush Python SDK addon")
                        .clicked()
                    {
                        self.trigger_conversion();
                    }

                    if ui
                        .button("Save Package…")
                        .on_hover_text("Save converted addon to .oxyaddon archive file")
                        .clicked()
                    {
                        let stem = self.manifest_draft.safe_stem();
                        self.file_dialog = InAppFileDialog::save_file(&format!("{stem}.oxyaddon"));
                    }

                    if ui
                        .button("Install to OxyBrush")
                        .on_hover_text(
                            "Deploy addon directly to your local OxyBrush plugins folder",
                        )
                        .clicked()
                    {
                        self.install_to_oxybrush();
                    }

                    ui.separator();
                    ui.label(RichText::new("Target Runtime:").weak());
                    let prev_runtime = self.runtime_mode;
                    egui::ComboBox::from_id_salt("runtime_select")
                        .selected_text(self.runtime_mode.badge())
                        .show_ui(ui, |ui| {
                            for r in crate::converters::wasm::PluginRuntime::ALL {
                                ui.selectable_value(&mut self.runtime_mode, r, r.display_name());
                            }
                        });
                    if prev_runtime != self.runtime_mode {
                        self.trigger_conversion();
                        if self.runtime_mode == crate::converters::wasm::PluginRuntime::Wasm {
                            self.right_tab = RightTab::WasmCode;
                        }
                    }

                    ui.separator();
                    ui.label(RichText::new("Ecosystem:").weak());
                    ui.checkbox(&mut self.auto_detect, "Auto");

                    if !self.auto_detect {
                        egui::ComboBox::from_id_salt("ecosystem_select")
                            .selected_text(self.selected_ecosystem.display_name())
                            .show_ui(ui, |ui| {
                                for eco in SourceEcosystem::ALL {
                                    if ui
                                        .selectable_value(
                                            &mut self.selected_ecosystem,
                                            eco,
                                            eco.display_name(),
                                        )
                                        .clicked()
                                    {
                                        self.trigger_conversion();
                                    }
                                }
                            });
                    } else {
                        widgets::badge_chip(
                            ui,
                            &format!(
                                "{} {}",
                                self.selected_ecosystem.icon(),
                                self.selected_ecosystem.display_name()
                            ),
                            Color32::from_gray(40),
                            self.accent.color(),
                        );
                    }
                });

                // Path input & quick loader bar
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("File:").weak());
                    let path_edit = ui.add(
                        egui::TextEdit::singleline(&mut self.source_path_input)
                            .desired_width(ui.available_width() - 250.0)
                            .hint_text("Drop script/addon file here or enter file path…"),
                    );
                    if (path_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                        || ui.button("Load").clicked()
                    {
                        let p = PathBuf::from(self.source_path_input.trim());
                        self.load_file(p);
                    }

                    ui.separator();
                    ui.label(RichText::new("Presets:").weak());
                    if ui.button("ZBrush").clicked() {
                        self.source_code = sample_zbrush_script();
                        self.source_path = None;
                        self.source_path_input = "example:zbrush".into();
                        self.manifest_draft.name = "Dynamesh Decimate".into();
                        self.manifest_draft.id.clear();
                        self.trigger_conversion();
                    }
                    if ui.button("Blender").clicked() {
                        self.source_code = sample_blender_script();
                        self.source_path = None;
                        self.source_path_input = "example:blender".into();
                        self.manifest_draft.name = "Quick Sculpt Remesh".into();
                        self.manifest_draft.id.clear();
                        self.trigger_conversion();
                    }
                    if ui.button("Nomad").clicked() {
                        self.source_code = sample_nomad_script();
                        self.source_path = None;
                        self.source_path_input = "example:nomad_js".into();
                        self.manifest_draft.name = "Nomad Organic Sculpt Pass".into();
                        self.manifest_draft.id.clear();
                        self.trigger_conversion();
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
                            ui.label(
                                RichText::new(p.file_name().unwrap().to_string_lossy()).strong(),
                            );
                        } else {
                            ui.label(RichText::new("(Unsaved buffer / Example)").weak());
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(format!("{} lines", self.source_code.lines().count()));
                            if ui.button("📋 Clear").clicked() {
                                self.source_code.clear();
                                self.source_path = None;
                            }
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
                        ui.selectable_value(
                            &mut self.right_tab,
                            RightTab::PythonCode,
                            "🐍 main.py",
                        );
                        ui.selectable_value(
                            &mut self.right_tab,
                            RightTab::WasmCode,
                            "⚡ plugin.wat (Wasm)",
                        );
                        ui.selectable_value(
                            &mut self.right_tab,
                            RightTab::Manifest,
                            "📋 manifest.json",
                        );
                        ui.selectable_value(
                            &mut self.right_tab,
                            RightTab::Report,
                            "📊 Translation Report",
                        );
                        ui.selectable_value(
                            &mut self.right_tab,
                            RightTab::InstalledManager,
                            format!("📦 OxyBrush Addons ({})", self.installed_addons.len()),
                        );
                    });

                    ui.add_space(4.0);

                    match self.right_tab {
                        RightTab::PythonCode => self.render_python_code_view(ui),
                        RightTab::WasmCode => self.render_wasm_code_view(ui),
                        RightTab::Manifest => self.render_manifest_view(ui),
                        RightTab::Report => self.render_report_view(ui),
                        RightTab::InstalledManager => self.render_installed_manager(ui),
                    }
                });
            });
        });

        // In-App File Browser Dialog Modal
        if let Some(picked) = self.file_dialog.show(&ctx) {
            if self.file_dialog.is_save_mode {
                self.save_oxyaddon(picked);
            } else {
                self.load_file(picked);
            }
        }

        // About Dialog
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

        // Specification Dialog
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

    fn render_wasm_code_view(&mut self, ui: &mut Ui) {
        if let Some(res) = &mut self.conversion {
            if let Some(wasm_def) = &mut res.wasm_definition {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Target WebAssembly Text (plugin.wat):").strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("📋 Copy WAT").clicked() {
                            ui.ctx().copy_text(wasm_def.wat_code.clone());
                            self.status_message = "Copied WebAssembly text to clipboard!".into();
                        }
                    });
                });

                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .id_salt("wasm_scroll")
                    .max_height(ui.available_height() - 10.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut wasm_def.wat_code)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .code_editor(),
                        );
                    });
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(30.0);
                    ui.label("WebAssembly code is generated when 'Target Runtime' is set to WebAssembly.");
                    if ui.button("⚡ Generate WebAssembly Plugin").clicked() {
                        self.runtime_mode = crate::converters::wasm::PluginRuntime::Wasm;
                        self.trigger_conversion();
                    }
                });
            }
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
                                        ui.selectable_value(
                                            &mut self.manifest_draft.category,
                                            c,
                                            c.display_name(),
                                        );
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
                ui.label(format!(
                    "Source Ecosystem: {}",
                    res.source_ecosystem.display_name()
                ));
                ui.add_space(6.0);

                if !res.warnings.is_empty() {
                    ui.label(
                        RichText::new(format!("⚠ Warnings ({})", res.warnings.len()))
                            .strong()
                            .color(Color32::from_rgb(255, 180, 80)),
                    );
                    for w in &res.warnings {
                        ui.label(format!("• {w}"));
                    }
                    ui.add_space(8.0);
                }

                ui.label(
                    RichText::new(format!(
                        "✔ Mapped API Operations ({})",
                        res.mapped_calls.len()
                    ))
                    .strong()
                    .color(Color32::from_rgb(100, 220, 140)),
                );
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
                    && ui.button("📁 Open Plugins Folder").clicked()
                {
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
                                widgets::badge_chip(
                                    ui,
                                    badge,
                                    Color32::from_gray(30),
                                    self.accent.color(),
                                );
                                ui.label(RichText::new(&addon.name).strong());

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button("🗑 Uninstall").clicked() {
                                            remove_path = Some(addon.path.clone());
                                        }
                                    },
                                );
                            });
                        });
                    ui.add_space(3.0);
                }
            });

            if let Some(path) = remove_path
                && let Ok(()) = uninstall_addon(&path)
            {
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
"#
    .to_string()
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
"#
    .to_string()
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
"#
    .to_string()
}

fn sample_nomad_json() -> String {
    r#"{
    "name": "Heavy Clay Tool",
    "author": "NomadPro",
    "description": "Custom clay brush preset with 40px radius and 0.85 strength",
    "tool": "clay",
    "radius": 40.0,
    "intensity": 0.85,
    "clay_build_up": true
}
"#
    .to_string()
}

fn format_clean_name(stem: &str) -> String {
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
        "Custom Addon".into()
    } else {
        words.join(" ")
    }
}
