//! Packing, unpacking, reading, and writing `.oxyaddon` archives.

use crate::package::manifest::AddonManifest;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Cursor, Read, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

pub const ADDON_EXTENSION: &str = "oxyaddon";
pub const MANIFEST_FILENAME: &str = "manifest.json";
pub const PLUGIN_JSON_FILENAME: &str = "plugin.json";
pub const DEFAULT_ENTRY_POINT: &str = "main.py";

#[derive(Debug)]
pub enum BundleError {
    Io(io::Error),
    Zip(zip::result::ZipError),
    Json(serde_json::Error),
    MissingManifest,
    MissingEntryPoint(String),
    InvalidPath(String),
}

impl std::fmt::Display for BundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Zip(e) => write!(f, "Zip error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::MissingManifest => write!(f, "Manifest file 'manifest.json' is missing from package"),
            Self::MissingEntryPoint(ep) => write!(f, "Entry point '{ep}' specified in manifest was not found in package"),
            Self::InvalidPath(p) => write!(f, "Invalid package path: {p}"),
        }
    }
}

impl std::error::Error for BundleError {}

impl From<io::Error> for BundleError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<zip::result::ZipError> for BundleError {
    fn from(e: zip::result::ZipError) -> Self {
        Self::Zip(e)
    }
}

impl From<serde_json::Error> for BundleError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

/// In-memory representation of an `.oxyaddon` package.
#[derive(Debug, Clone)]
pub struct AddonPackage {
    pub manifest: AddonManifest,
    pub main_script: String,
    pub wat_script: Option<String>,
    pub icon_png: Option<Vec<u8>>,
    pub extra_files: BTreeMap<String, Vec<u8>>,
}

impl AddonPackage {
    /// Create a new package from manifest and main python script.
    pub fn new(manifest: AddonManifest, main_script: impl Into<String>) -> Self {
        Self {
            manifest,
            main_script: main_script.into(),
            wat_script: None,
            icon_png: None,
            extra_files: BTreeMap::new(),
        }
    }

    /// Read an `.oxyaddon` from a file path (either a `.oxyaddon` zip file or an unpacked directory).
    pub fn open(path: impl AsRef<Path>) -> Result<Self, BundleError> {
        let p = path.as_ref();
        if p.is_dir() {
            Self::from_directory(p)
        } else if p.is_file() {
            Self::from_archive_file(p)
        } else {
            Err(BundleError::InvalidPath(p.display().to_string()))
        }
    }

    /// Read package from a ZIP archive file.
    pub fn from_archive_file(path: &Path) -> Result<Self, BundleError> {
        let file = File::open(path)?;
        Self::from_reader(file)
    }

    /// Read package from any reader containing ZIP bytes.
    pub fn from_reader<R: Read + io::Seek>(reader: R) -> Result<Self, BundleError> {
        let mut zip = ZipArchive::new(reader)?;
        let mut manifest_bytes = None;
        let mut files = BTreeMap::new();

        for i in 0..zip.len() {
            let mut file = zip.by_index(i)?;
            if file.is_dir() {
                continue;
            }
            let name = file.name().to_string();
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;

            if name == MANIFEST_FILENAME {
                manifest_bytes = Some(buf);
            } else if name == PLUGIN_JSON_FILENAME {
                // If manifest.json is not present, plugin_json can be fallback
                if manifest_bytes.is_none() {
                    manifest_bytes = Some(buf.clone());
                }
                files.insert(name, buf);
            } else {
                files.insert(name, buf);
            }
        }

        let raw_manifest = manifest_bytes.ok_or(BundleError::MissingManifest)?;
        let manifest: AddonManifest = serde_json::from_slice(&raw_manifest)?;

        let entry_name = &manifest.entry_point;
        let script_bytes = files
            .remove(entry_name)
            .ok_or_else(|| BundleError::MissingEntryPoint(entry_name.clone()))?;

        let main_script = String::from_utf8(script_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let wat_bytes = files.remove("plugin.wat").or_else(|| files.remove("plugin.wasm"));
        let wat_script = wat_bytes.and_then(|b| String::from_utf8(b).ok());

        let icon_png = files.remove("icon.png");

        Ok(Self {
            manifest,
            main_script,
            wat_script,
            icon_png,
            extra_files: files,
        })
    }

    /// Read package from an uncompressed directory.
    pub fn from_directory(dir: &Path) -> Result<Self, BundleError> {
        let manifest_path = dir.join(MANIFEST_FILENAME);
        if !manifest_path.is_file() {
            return Err(BundleError::MissingManifest);
        }
        let manifest_content = fs::read_to_string(&manifest_path)?;
        let manifest: AddonManifest = serde_json::from_str(&manifest_content)?;

        let entry_path = dir.join(&manifest.entry_point);
        if !entry_path.is_file() {
            return Err(BundleError::MissingEntryPoint(manifest.entry_point.clone()));
        }
        let main_script = fs::read_to_string(&entry_path)?;

        let icon_path = dir.join("icon.png");
        let icon_png = if icon_path.is_file() {
            fs::read(&icon_path).ok()
        } else {
            None
        };

        let mut extra_files = BTreeMap::new();
        scan_directory_files(dir, dir, &mut extra_files, &[MANIFEST_FILENAME, &manifest.entry_point, "icon.png"])?;

        let wat_script = if dir.join("plugin.wat").is_file() {
            fs::read_to_string(dir.join("plugin.wat")).ok()
        } else {
            None
        };

        Ok(Self {
            manifest,
            main_script,
            wat_script,
            icon_png,
            extra_files,
        })
    }

    /// Pack the addon into a `.oxyaddon` ZIP archive file.
    pub fn save_to_archive(&self, out_path: impl AsRef<Path>) -> Result<(), BundleError> {
        let path = out_path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = File::create(path)?;
        self.write_zip(file)?;
        Ok(())
    }

    /// Pack into an in-memory byte buffer.
    pub fn to_bytes(&self) -> Result<Vec<u8>, BundleError> {
        let mut cursor = Cursor::new(Vec::new());
        self.write_zip(&mut cursor)?;
        Ok(cursor.into_inner())
    }

    /// Write ZIP to a writable stream.
    pub fn write_zip<W: Write + io::Seek>(&self, writer: W) -> Result<(), BundleError> {
        let mut zip = ZipWriter::new(writer);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

        // 1. Write plugin.json (native OxyBrush manifest)
        let plugin_json_content = if let Some(wat) = &self.wat_script {
            let def = crate::converters::wasm::WasmPluginDefinition::from_source(wat, &self.manifest);
            def.to_plugin_json(&self.manifest.safe_stem())
        } else {
            self.manifest.to_plugin_json()
        };
        zip.start_file(PLUGIN_JSON_FILENAME, options)?;
        zip.write_all(plugin_json_content.as_bytes())?;

        // 1b. Write manifest.json (extended OxyConvert manifest)
        zip.start_file(MANIFEST_FILENAME, options)?;
        let manifest_json = serde_json::to_string_pretty(&self.manifest)?;
        zip.write_all(manifest_json.as_bytes())?;

        // 2. Write WebAssembly plugin.wat if present
        if let Some(wat) = &self.wat_script {
            zip.start_file("plugin.wat", options)?;
            zip.write_all(wat.as_bytes())?;
        }

        // 2b. Write main python script
        zip.start_file(&self.manifest.entry_point, options)?;
        zip.write_all(self.main_script.as_bytes())?;

        // 3. Write optional icon
        if let Some(icon) = &self.icon_png {
            zip.start_file("icon.png", options)?;
            zip.write_all(icon)?;
        }

        // 4. Write any extra files
        for (rel_path, content) in &self.extra_files {
            zip.start_file(rel_path, options)?;
            zip.write_all(content)?;
        }

        zip.finish()?;
        Ok(())
    }

    /// Unpack to an output directory on disk.
    pub fn unpack_to_directory(&self, dest: impl AsRef<Path>) -> Result<(), BundleError> {
        let d = dest.as_ref();
        fs::create_dir_all(d)?;

        // plugin.json (native OxyBrush manifest)
        let plugin_json_content = if let Some(wat) = &self.wat_script {
            let def = crate::converters::wasm::WasmPluginDefinition::from_source(wat, &self.manifest);
            def.to_plugin_json(&self.manifest.safe_stem())
        } else {
            self.manifest.to_plugin_json()
        };
        fs::write(d.join(PLUGIN_JSON_FILENAME), plugin_json_content)?;

        // manifest.json
        let manifest_json = serde_json::to_string_pretty(&self.manifest)?;
        fs::write(d.join(MANIFEST_FILENAME), manifest_json)?;

        // WebAssembly plugin.wat if present
        if let Some(wat) = &self.wat_script {
            fs::write(d.join("plugin.wat"), wat)?;
        }

        // main script
        let script_path = d.join(&self.manifest.entry_point);
        if let Some(parent) = script_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(script_path, &self.main_script)?;

        // icon
        if let Some(icon) = &self.icon_png {
            fs::write(d.join("icon.png"), icon)?;
        }

        // extra files
        for (rel, bytes) in &self.extra_files {
            let out_file = d.join(rel);
            if let Some(p) = out_file.parent() {
                fs::create_dir_all(p)?;
            }
            fs::write(out_file, bytes)?;
        }

        Ok(())
    }
}

fn scan_directory_files(
    root: &Path,
    current: &Path,
    out: &mut BTreeMap<String, Vec<u8>>,
    ignore: &[&str],
) -> io::Result<()> {
    if !current.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let p = entry.path();
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if file_name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            scan_directory_files(root, &p, out, ignore)?;
        } else if p.is_file()
            && let Ok(rel) = p.strip_prefix(root) {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                if !ignore.iter().any(|&ig| rel_str == ig) {
                    let bytes = fs::read(&p)?;
                    out.insert(rel_str, bytes);
                }
            }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_roundtrip() {
        let manifest = AddonManifest {
            id: "com.test.addon".into(),
            name: "Test Addon".into(),
            version: "1.2.0".into(),
            author: "Tester".into(),
            description: "Testing roundtrip".into(),
            ..Default::default()
        };
        let script = "import oxybrush\nprint('hello world')\n";
        let package = AddonPackage::new(manifest.clone(), script);

        let zip_bytes = package.to_bytes().expect("pack should succeed");
        let unpacked = AddonPackage::from_reader(Cursor::new(zip_bytes)).expect("unpack should succeed");

        assert_eq!(unpacked.manifest.id, "com.test.addon");
        assert_eq!(unpacked.manifest.name, "Test Addon");
        assert_eq!(unpacked.main_script, script);
    }
}
