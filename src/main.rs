//! OxyConvert desktop application entry point.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;
use oxyconvert::cli::{Cli, Commands, run_cli};
use oxyconvert::ui::OxyConvertApp;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let cli = Cli::parse();

    // If an explicit CLI command was requested (except Gui), run headlessly
    if let Some(cmd) = &cli.command
        && !matches!(cmd, Commands::Gui)
    {
        return run_cli(cli);
    }

    // Launch egui GUI application
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1150.0, 740.0])
            .with_min_inner_size([800.0, 520.0])
            .with_title("OxyConvert — Universal Addon Studio"),
        ..Default::default()
    };

    eframe::run_native(
        "OxyConvert",
        native_options,
        Box::new(|cc| Ok(Box::new(OxyConvertApp::new(cc)))),
    )
    .map_err(|e| format!("Failed to launch GUI: {e}").into())
}
