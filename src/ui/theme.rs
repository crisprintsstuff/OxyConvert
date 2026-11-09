//! Visual theme system matching OxyBrush.

use egui::{Color32, Stroke, Visuals};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemePreset {
    #[default]
    Graphite,
    ZBrushClassic,
    Charcoal,
    Slate,
    WarmUmber,
    HighContrast,
}

impl ThemePreset {
    pub const ALL: [ThemePreset; 6] = [
        ThemePreset::Graphite,
        ThemePreset::ZBrushClassic,
        ThemePreset::Charcoal,
        ThemePreset::Slate,
        ThemePreset::WarmUmber,
        ThemePreset::HighContrast,
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Graphite => "Default Studio (Graphite)",
            Self::ZBrushClassic => "ZBrush Classic",
            Self::Charcoal => "Charcoal",
            Self::Slate => "Slate Blue",
            Self::WarmUmber => "Warm Umber",
            Self::HighContrast => "High Contrast",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AccentPreset {
    #[default]
    ClayOrange,
    ZBrushClassicOrange,
    SculptTeal,
    Gold,
    Crimson,
}

impl AccentPreset {
    pub const ALL: [AccentPreset; 5] = [
        AccentPreset::ClayOrange,
        AccentPreset::ZBrushClassicOrange,
        AccentPreset::SculptTeal,
        AccentPreset::Gold,
        AccentPreset::Crimson,
    ];

    pub fn color(&self) -> Color32 {
        match self {
            Self::ClayOrange => Color32::from_rgb(0xE0, 0x7A, 0x4F), // OxyBrush clay accent
            Self::ZBrushClassicOrange => Color32::from_rgb(0xF5, 0x82, 0x20),
            Self::SculptTeal => Color32::from_rgb(0x3B, 0xBA, 0x9F),
            Self::Gold => Color32::from_rgb(0xDF, 0xB1, 0x43),
            Self::Crimson => Color32::from_rgb(0xE0, 0x43, 0x58),
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::ClayOrange => "Clay Orange (OxyBrush)",
            Self::ZBrushClassicOrange => "ZBrush Orange",
            Self::SculptTeal => "Sculpt Teal",
            Self::Gold => "Amber Gold",
            Self::Crimson => "Crimson Red",
        }
    }
}

pub fn apply_oxy_theme(ctx: &egui::Context, theme: ThemePreset, accent: AccentPreset) {
    let mut visuals = Visuals::dark();
    let accent_col = accent.color();

    struct Palette {
        #[allow(dead_code)]
        bg: Color32,
        panel: Color32,
        window: Color32,
        faint: Color32,
        text: Color32,
        inactive_weak: Color32,
        inactive_fill: Color32,
        hovered_weak: Color32,
        hovered_fill: Color32,
        stroke: Color32,
        window_stroke: Color32,
    }

    let p = match theme {
        ThemePreset::Graphite => Palette {
            bg: Color32::from_gray(24),
            panel: Color32::from_gray(34),
            window: Color32::from_gray(39),
            faint: Color32::from_gray(40),
            text: Color32::from_gray(228),
            inactive_weak: Color32::from_gray(47),
            inactive_fill: Color32::from_gray(56),
            hovered_weak: Color32::from_gray(59),
            hovered_fill: Color32::from_gray(68),
            stroke: Color32::from_gray(48),
            window_stroke: Color32::from_gray(60),
        },
        ThemePreset::ZBrushClassic => Palette {
            bg: Color32::from_gray(16),
            panel: Color32::from_gray(26),
            window: Color32::from_gray(32),
            faint: Color32::from_gray(20),
            text: Color32::from_gray(222),
            inactive_weak: Color32::from_gray(36),
            inactive_fill: Color32::from_gray(46),
            hovered_weak: Color32::from_gray(54),
            hovered_fill: Color32::from_gray(64),
            stroke: Color32::from_gray(42),
            window_stroke: Color32::from_gray(54),
        },
        ThemePreset::Charcoal => Palette {
            bg: Color32::from_gray(12),
            panel: Color32::from_gray(18),
            window: Color32::from_gray(24),
            faint: Color32::from_gray(26),
            text: Color32::from_gray(236),
            inactive_weak: Color32::from_gray(30),
            inactive_fill: Color32::from_gray(38),
            hovered_weak: Color32::from_gray(44),
            hovered_fill: Color32::from_gray(54),
            stroke: Color32::from_gray(36),
            window_stroke: Color32::from_gray(48),
        },
        ThemePreset::Slate => Palette {
            bg: Color32::from_rgb(22, 26, 32),
            panel: Color32::from_rgb(30, 35, 44),
            window: Color32::from_rgb(36, 42, 53),
            faint: Color32::from_rgb(42, 49, 61),
            text: Color32::from_rgb(226, 232, 240),
            inactive_weak: Color32::from_rgb(45, 52, 64),
            inactive_fill: Color32::from_rgb(51, 60, 75),
            hovered_weak: Color32::from_rgb(60, 71, 88),
            hovered_fill: Color32::from_rgb(71, 84, 103),
            stroke: Color32::from_rgb(52, 62, 77),
            window_stroke: Color32::from_rgb(64, 76, 95),
        },
        ThemePreset::WarmUmber => Palette {
            bg: Color32::from_rgb(28, 24, 22),
            panel: Color32::from_rgb(38, 33, 30),
            window: Color32::from_rgb(46, 40, 36),
            faint: Color32::from_rgb(52, 45, 41),
            text: Color32::from_rgb(240, 232, 226),
            inactive_weak: Color32::from_rgb(54, 47, 43),
            inactive_fill: Color32::from_rgb(64, 56, 51),
            hovered_weak: Color32::from_rgb(74, 65, 59),
            hovered_fill: Color32::from_rgb(86, 76, 69),
            stroke: Color32::from_rgb(62, 53, 47),
            window_stroke: Color32::from_rgb(76, 66, 60),
        },
        ThemePreset::HighContrast => Palette {
            bg: Color32::BLACK,
            panel: Color32::from_gray(10),
            window: Color32::from_gray(10),
            faint: Color32::from_gray(16),
            text: Color32::WHITE,
            inactive_weak: Color32::from_gray(20),
            inactive_fill: Color32::from_gray(30),
            hovered_weak: Color32::from_gray(40),
            hovered_fill: Color32::from_gray(50),
            stroke: Color32::WHITE,
            window_stroke: Color32::WHITE,
        },
    };

    visuals.panel_fill = p.panel;
    visuals.window_fill = p.window;
    visuals.faint_bg_color = p.faint;
    visuals.override_text_color = Some(p.text);
    visuals.window_stroke = Stroke::new(1.0, p.window_stroke);
    visuals.selection.bg_fill = accent_col.linear_multiply(0.28);
    visuals.selection.stroke = Stroke::new(1.0, accent_col);

    let rounding = egui::CornerRadius::same(4);

    visuals.widgets.noninteractive.corner_radius = rounding;
    visuals.widgets.noninteractive.bg_fill = p.panel;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.stroke);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);

    visuals.widgets.inactive.corner_radius = rounding;
    visuals.widgets.inactive.bg_fill = p.inactive_fill;
    visuals.widgets.inactive.weak_bg_fill = p.inactive_weak;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, p.stroke);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);

    visuals.widgets.hovered.corner_radius = rounding;
    visuals.widgets.hovered.bg_fill = p.hovered_fill;
    visuals.widgets.hovered.weak_bg_fill = p.hovered_weak;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, accent_col);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.2, Color32::WHITE);

    visuals.widgets.active.corner_radius = rounding;
    visuals.widgets.active.bg_fill = accent_col.linear_multiply(0.85);
    visuals.widgets.active.weak_bg_fill = accent_col.linear_multiply(0.65);
    visuals.widgets.active.bg_stroke = Stroke::new(1.5, accent_col);
    visuals.widgets.active.fg_stroke = Stroke::new(1.5, Color32::WHITE);

    ctx.set_visuals(visuals);
}
