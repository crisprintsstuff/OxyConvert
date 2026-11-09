//! OxyBrush installation directory detection and package deployment.

use crate::package::bundle::{ADDON_EXTENSION, AddonPackage};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Installed addon information detected in OxyBrush's plugin directory.
#[derive(Debug, Clone)]
pub struct InstalledAddonInfo {
    pub name: String,
    pub path: PathBuf,
    pub is_archive: bool,
    pub is_directory: bool,
    pub is_standalone_py: bool,
}

/// Locates the OxyBrush data directory across platforms.
pub fn oxybrush_data_dir() -> Option<PathBuf> {
    if let Some(override_path) = std::env::var_os("OXYBRUSH_DATA_DIR") {
        return Some(PathBuf::from(override_path));
    }

    #[cfg(windows)]
    {
        // First check standard APPDATA (Roaming)
        if let Some(appdata) = dirs::config_dir() {
            let candidate = appdata.join("oxybrush");
            if candidate.exists() {
                return Some(candidate);
            }
            let candidate_upper = appdata.join("OxyBrush");
            if candidate_upper.exists() {
                return Some(candidate_upper);
            }
        }
        // Then check LOCALAPPDATA
        if let Some(local) = dirs::data_local_dir() {
            let candidate = local.join("OxyBrush");
            if candidate.exists() {
                return Some(candidate);
            }
            let candidate_lower = local.join("oxybrush");
            if candidate_lower.exists() {
                return Some(candidate_lower);
            }
        }
        // Default to LOCALAPPDATA / OxyBrush or APPDATA / oxybrush
        dirs::data_local_dir().map(|d| d.join("OxyBrush"))
    }

    #[cfg(target_os = "macos")]
    {
        dirs::data_dir().map(|d| d.join("oxybrush"))
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    {
        dirs::data_local_dir().map(|d| d.join("oxybrush"))
    }
}

/// Locates the OxyBrush plugins directory where `.oxyaddon` and `.py` scripts live.
pub fn oxybrush_plugins_dir() -> Option<PathBuf> {
    if let Some(override_path) = std::env::var_os("OXYBRUSH_PLUGINS_DIR") {
        return Some(PathBuf::from(override_path));
    }

    oxybrush_data_dir().map(|d| d.join("plugins"))
}

/// Install an `.oxyaddon` package into the OxyBrush plugins folder.
///
/// If `as_folder` is true, unpacks into `<plugins_dir>/<stem>/`.
/// Otherwise, writes `<plugins_dir>/<stem>.oxyaddon` as a self-contained archive,
/// and also writes a top-level `<stem>.py` bridge if requested for seamless backward compatibility.
pub fn install_addon(
    package: &AddonPackage,
    as_folder: bool,
    write_py_bridge: bool,
) -> Result<PathBuf, io::Error> {
    let plugins_dir = oxybrush_plugins_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Could not determine OxyBrush plugins directory on this system.",
        )
    })?;

    fs::create_dir_all(&plugins_dir)?;

    let stem = package.manifest.safe_stem();

    let target_path = if as_folder {
        let folder = plugins_dir.join(&stem);
        package
            .unpack_to_directory(&folder)
            .map_err(|e| io::Error::other(e.to_string()))?;
        folder
    } else {
        let archive_path = plugins_dir.join(format!("{stem}.{ADDON_EXTENSION}"));
        package
            .save_to_archive(&archive_path)
            .map_err(|e| io::Error::other(e.to_string()))?;
        archive_path
    };

    // OxyBrush 0.5.0 scans for *.py files in plugins_dir. To make sure the addon immediately
    // appears in the OxyBrush top menu even in older OxyBrush versions without needing an update,
    // we also generate a clean bridge script with the standard OxyBrush metadata header!
    if write_py_bridge {
        let bridge_script_path = plugins_dir.join(format!("{stem}.py"));
        let header = format!(
            "# Name: {}\n# Description: {}\n# Addon ID: {}\n# Version: {}\n# Author: {}\n\n",
            package.manifest.name,
            package.manifest.description,
            package.manifest.id,
            package.manifest.version,
            package.manifest.author
        );
        let full_script = format!("{}{}", header, package.main_script);
        fs::write(bridge_script_path, full_script)?;
    }

    Ok(target_path)
}

/// List all installed addons and plugins in the OxyBrush plugins directory.
pub fn list_installed_addons() -> io::Result<Vec<InstalledAddonInfo>> {
    let mut list = Vec::new();
    let Some(plugins_dir) = oxybrush_plugins_dir() else {
        return Ok(list);
    };

    if !plugins_dir.is_dir() {
        return Ok(list);
    }

    for entry in fs::read_dir(plugins_dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();

        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext.eq_ignore_ascii_case(ADDON_EXTENSION) {
                    list.push(InstalledAddonInfo {
                        name,
                        path,
                        is_archive: true,
                        is_directory: false,
                        is_standalone_py: false,
                    });
                } else if ext.eq_ignore_ascii_case("py") {
                    list.push(InstalledAddonInfo {
                        name,
                        path,
                        is_archive: false,
                        is_directory: false,
                        is_standalone_py: true,
                    });
                }
            }
        } else if path.is_dir() && path.join("manifest.json").is_file() {
            list.push(InstalledAddonInfo {
                name,
                path,
                is_archive: false,
                is_directory: true,
                is_standalone_py: false,
            });
        }
    }

    list.sort_by_key(|a| a.name.to_lowercase());
    Ok(list)
}

/// Uninstall an addon by removing its file or folder from the plugins directory.
pub fn uninstall_addon(path: &Path) -> io::Result<()> {
    if path.is_file() {
        fs::remove_file(path)?;
        // If it was an .oxyaddon, also clean up corresponding .py bridge if it exists
        if let Some(stem) = path.file_stem()
            && let Some(parent) = path.parent() {
                let py_bridge = parent.join(format!("{}.py", stem.to_string_lossy()));
                if py_bridge.is_file() {
                    let _ = fs::remove_file(py_bridge);
                }
            }
    } else if path.is_dir() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}
