//! Packaging system for `.oxyaddon` files.

pub mod bundle;
pub mod installer;
pub mod manifest;

pub use bundle::{ADDON_EXTENSION, AddonPackage, BundleError, MANIFEST_FILENAME};
pub use installer::{install_addon, list_installed_addons, oxybrush_plugins_dir, uninstall_addon};
pub use manifest::{AddonCategory, AddonManifest, SourceEcosystem, UiBinding};
