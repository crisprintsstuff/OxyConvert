//! egui UI module for OxyConvert.

pub mod app;
pub mod file_dialog;
pub mod theme;
pub mod widgets;

pub use app::OxyConvertApp;
pub use file_dialog::InAppFileDialog;
pub use theme::{AccentPreset, ThemePreset, apply_oxy_theme};
