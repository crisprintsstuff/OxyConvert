//! OxyConvert: Universal ZBrush, Blender, and Nomad Sculpt to .oxyaddon converter.

pub mod cli;
pub mod converters;
pub mod package;
pub mod ui;

pub use cli::{Cli, Commands};
pub use converters::{ConversionOptions, ConversionResult, convert_script, detect_ecosystem};
pub use package::{AddonCategory, AddonManifest, AddonPackage, SourceEcosystem};
pub use ui::OxyConvertApp;
