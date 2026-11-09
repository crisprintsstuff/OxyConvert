//! Built-in cross-platform file browser modal.
//! Ensures file opening works even when OS native dialogs (zenity/portal) are absent.

use crate::package::installer::oxybrush_plugins_dir;
use egui::{Color32, RichText, vec2};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFilter {
    AllSupported,
    ZScript,
    Blender,
    Nomad,
    OxyAddon,
    AllFiles,
}

impl FileFilter {
    pub const ALL: [FileFilter; 6] = [
        FileFilter::AllSupported,
        FileFilter::ZScript,
        FileFilter::Blender,
        FileFilter::Nomad,
        FileFilter::OxyAddon,
        FileFilter::AllFiles,
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::AllSupported => "All Supported (*.txt, *.py, *.js, *.json, *.oxyaddon, *.zip)",
            Self::ZScript => "ZBrush ZScript (*.txt, *.zsc)",
            Self::Blender => "Blender Python (*.py, *.zip)",
            Self::Nomad => "Nomad Sculpt (*.js, *.json)",
            Self::OxyAddon => "OxyBrush Addon (*.oxyaddon)",
            Self::AllFiles => "All Files (*.*)",
        }
    }

    pub fn matches(&self, path: &Path) -> bool {
        if path.is_dir() {
            return true;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match self {
            Self::AllSupported => matches!(
                ext.as_str(),
                "txt" | "zsc" | "py" | "js" | "json" | "oxyaddon" | "zip"
            ),
            Self::ZScript => matches!(ext.as_str(), "txt" | "zsc"),
            Self::Blender => matches!(ext.as_str(), "py" | "zip"),
            Self::Nomad => matches!(ext.as_str(), "js" | "json"),
            Self::OxyAddon => ext == "oxyaddon",
            Self::AllFiles => true,
        }
    }
}

pub struct InAppFileDialog {
    pub is_open: bool,
    pub is_save_mode: bool,
    pub current_dir: PathBuf,
    pub path_input: String,
    pub selected_file: Option<PathBuf>,
    pub save_filename: String,
    pub filter: FileFilter,
    pub result: Option<PathBuf>,
}

impl Default for InAppFileDialog {
    fn default() -> Self {
        let initial_dir = std::env::current_dir()
            .or_else(|_| dirs::home_dir().ok_or(()))
            .unwrap_or_else(|_| PathBuf::from("."));

        Self {
            is_open: false,
            is_save_mode: false,
            path_input: initial_dir.to_string_lossy().into_owned(),
            current_dir: initial_dir,
            selected_file: None,
            save_filename: "my_addon.oxyaddon".to_string(),
            filter: FileFilter::AllSupported,
            result: None,
        }
    }
}

impl InAppFileDialog {
    pub fn open_file() -> Self {
        Self {
            is_open: true,
            is_save_mode: false,
            result: None,
            ..Default::default()
        }
    }

    pub fn save_file(default_name: &str) -> Self {
        Self {
            is_open: true,
            is_save_mode: true,
            save_filename: default_name.to_string(),
            filter: FileFilter::OxyAddon,
            result: None,
            ..Default::default()
        }
    }

    pub fn set_dir(&mut self, dir: PathBuf) {
        if dir.is_dir() {
            self.path_input = dir.to_string_lossy().into_owned();
            self.current_dir = dir;
            self.selected_file = None;
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<PathBuf> {
        if !self.is_open {
            return None;
        }

        let mut picked_file = None;
        let mut close_dialog = false;

        let title = if self.is_save_mode {
            "Save .oxyaddon Package"
        } else {
            "Import Script or Addon"
        };

        let mut open_var = self.is_open;
        egui::Window::new(title)
            .open(&mut open_var)
            .default_size(vec2(680.0, 440.0))
            .min_size(vec2(500.0, 320.0))
            .collapsible(false)
            .show(ctx, |ui| {
                // Bookmarks bar
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Quick Folders:").weak());

                    if ui.button("🏠 Home").clicked()
                        && let Some(home) = dirs::home_dir()
                    {
                        self.set_dir(home);
                    }

                    if ui.button("💼 Current Dir").clicked()
                        && let Ok(cwd) = std::env::current_dir()
                    {
                        self.set_dir(cwd);
                    }

                    if let Some(pdir) = oxybrush_plugins_dir()
                        && ui.button("🎨 OxyBrush Plugins").clicked()
                    {
                        self.set_dir(pdir);
                    }

                    // Sibling OxyBrush examples directory
                    let sibling_examples = PathBuf::from("../OxyBrush/examples/plugins");
                    if sibling_examples.is_dir() && ui.button("📚 OxyBrush Examples").clicked() {
                        if let Ok(canon) = fs::canonicalize(&sibling_examples) {
                            self.set_dir(canon);
                        } else {
                            self.set_dir(sibling_examples);
                        }
                    }

                    let local_examples = PathBuf::from("examples");
                    if local_examples.is_dir() && ui.button("📂 Converter Examples").clicked() {
                        if let Ok(canon) = fs::canonicalize(&local_examples) {
                            self.set_dir(canon);
                        } else {
                            self.set_dir(local_examples);
                        }
                    }
                });

                ui.separator();

                // Path navigation bar
                ui.horizontal(|ui| {
                    if ui.button("⬆ Up").clicked()
                        && let Some(parent) = self.current_dir.parent()
                    {
                        self.set_dir(parent.to_path_buf());
                    }

                    let path_res = ui.text_edit_singleline(&mut self.path_input);
                    if path_res.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let candidate = PathBuf::from(&self.path_input);
                        if candidate.is_dir() {
                            self.set_dir(candidate);
                        }
                    }

                    if ui.button("Go").clicked() {
                        let candidate = PathBuf::from(&self.path_input);
                        if candidate.is_dir() {
                            self.set_dir(candidate);
                        }
                    }
                });

                ui.separator();

                // Directory listing
                let scroll_height = if self.is_save_mode {
                    ui.available_height() - 80.0
                } else {
                    ui.available_height() - 50.0
                };

                egui::ScrollArea::vertical()
                    .max_height(scroll_height)
                    .show(ui, |ui| {
                        let mut next_dir = None;
                        let mut clicked_file = None;

                        if let Ok(entries) = fs::read_dir(&self.current_dir) {
                            let mut sorted_entries: Vec<_> =
                                entries.filter_map(|e| e.ok()).collect();
                            sorted_entries.sort_by_key(|e| {
                                let is_d = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                                (!is_d, e.file_name().to_string_lossy().to_lowercase())
                            });

                            for entry in sorted_entries {
                                let path = entry.path();
                                let file_name = entry.file_name().to_string_lossy().into_owned();

                                if file_name.starts_with('.') {
                                    continue;
                                }

                                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

                                if !is_dir && !self.filter.matches(&path) {
                                    continue;
                                }

                                ui.horizontal(|ui| {
                                    if is_dir {
                                        ui.label(
                                            RichText::new("📁")
                                                .color(Color32::from_rgb(230, 190, 80)),
                                        );
                                        if ui.link(&file_name).clicked() {
                                            next_dir = Some(path.clone());
                                        }
                                    } else {
                                        let is_selected =
                                            self.selected_file.as_ref() == Some(&path);
                                        let icon = match path
                                            .extension()
                                            .and_then(|e| e.to_str())
                                            .unwrap_or("")
                                        {
                                            "py" => "🐍",
                                            "txt" | "zsc" => "🗿",
                                            "js" | "json" => "📱",
                                            "oxyaddon" => "⚡",
                                            "zip" => "📦",
                                            _ => "📄",
                                        };
                                        ui.label(icon);
                                        if ui.selectable_label(is_selected, &file_name).clicked() {
                                            clicked_file = Some(path.clone());
                                        }
                                    }
                                });
                            }
                        } else {
                            ui.label("Could not read directory contents.");
                        }

                        if let Some(dir) = next_dir {
                            self.set_dir(dir);
                        }
                        if let Some(f) = clicked_file {
                            if self.is_save_mode
                                && let Some(name) = f.file_name()
                            {
                                self.save_filename = name.to_string_lossy().into_owned();
                            }
                            self.selected_file = Some(f);
                        }
                    });

                ui.separator();

                // Save mode file name entry
                if self.is_save_mode {
                    ui.horizontal(|ui| {
                        ui.label("Save As File Name:");
                        ui.text_edit_singleline(&mut self.save_filename);
                    });
                }

                // Filter & Action buttons
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("file_dialog_filter")
                        .selected_text(self.filter.display_name())
                        .show_ui(ui, |ui| {
                            for f in FileFilter::ALL {
                                ui.selectable_value(&mut self.filter, f, f.display_name());
                            }
                        });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Cancel").clicked() {
                            close_dialog = true;
                        }

                        let action_btn_text = if self.is_save_mode { "Save" } else { "Open" };
                        let can_act = if self.is_save_mode {
                            !self.save_filename.trim().is_empty()
                        } else {
                            self.selected_file.is_some()
                        };

                        if ui
                            .add_enabled(can_act, egui::Button::new(action_btn_text))
                            .clicked()
                        {
                            if self.is_save_mode {
                                let mut out = self.current_dir.join(self.save_filename.trim());
                                if out.extension().is_none() {
                                    out.set_extension("oxyaddon");
                                }
                                picked_file = Some(out);
                            } else {
                                picked_file = self.selected_file.clone();
                            }
                            close_dialog = true;
                        }
                    });
                });
            });

        if close_dialog || !open_var {
            self.is_open = false;
        }

        picked_file
    }
}
