//! egui UI module for OxyConvert.

pub mod app;
pub mod theme;
pub mod widgets;

pub use app::OxyConvertApp;
pub use theme::{apply_oxy_theme, AccentPreset, ThemePreset};
