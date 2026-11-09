//! Command-line interface for OxyConvert.

use crate::converters::{convert_script, detect_ecosystem, ConversionOptions};
use crate::package::bundle::AddonPackage;
use crate::package::installer::{install_addon, list_installed_addons, oxybrush_plugins_dir};
use crate::package::manifest::SourceEcosystem;
use clap::{Parser, Subcommand, ValueEnum};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliEcosystem {
    Zbrush,
    Blender,
    Nomad,
    Native,
}

impl From<CliEcosystem> for SourceEcosystem {
    fn from(c: CliEcosystem) -> Self {
        match c {
            CliEcosystem::Zbrush => SourceEcosystem::Zbrush,
            CliEcosystem::Blender => SourceEcosystem::Blender,
            CliEcosystem::Nomad => SourceEcosystem::Nomad,
            CliEcosystem::Native => SourceEcosystem::Native,
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "oxyconvert",
    author = "OxyLabs",
    version = "0.1.0",
    about = "Convert ZBrush, Blender, and Nomad Sculpt plugins into .oxyaddon packages for OxyBrush",
    long_about = None
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Launch the OxyConvert interactive desktop GUI
    Gui,

    /// Convert a script or plugin file to .oxyaddon
    Convert {
        /// Input script path (.txt, .zsc, .py, .js, .json)
        #[arg(value_name = "INPUT_FILE")]
        input: PathBuf,

        /// Output .oxyaddon archive path
        #[arg(short, long, value_name = "OUTPUT_FILE")]
        output: Option<PathBuf>,

        /// Explicitly specify the source ecosystem (defaults to auto-detect)
        #[arg(short, long, value_enum)]
        ecosystem: Option<CliEcosystem>,

        /// Directly install the converted addon into OxyBrush
        #[arg(short, long)]
        install: bool,

        /// Override addon display name
        #[arg(long)]
        name: Option<String>,

        /// Override addon author
        #[arg(long)]
        author: Option<String>,
    },

    /// Pack an uncompressed addon directory into a .oxyaddon archive
    Pack {
        /// Source directory containing manifest.json and main.py
        #[arg(value_name = "DIRECTORY")]
        dir: PathBuf,

        /// Target .oxyaddon file
        #[arg(short, long, value_name = "OUTPUT.oxyaddon")]
        output: PathBuf,
    },

    /// Unpack a .oxyaddon archive file into a directory
    Unpack {
        /// Input .oxyaddon archive
        #[arg(value_name = "PACKAGE.oxyaddon")]
        package: PathBuf,

        /// Target output directory
        #[arg(short, long, value_name = "DESTINATION_DIR")]
        output: PathBuf,
    },

    /// Install an existing .oxyaddon package into OxyBrush
    Install {
        /// .oxyaddon package file to install
        #[arg(value_name = "PACKAGE.oxyaddon")]
        package: PathBuf,
    },

    /// Inspect the metadata and contents of an .oxyaddon package
    Inspect {
        /// Path to .oxyaddon file or directory
        #[arg(value_name = "PACKAGE.oxyaddon")]
        package: PathBuf,
    },

    /// List all currently installed addons in OxyBrush
    List,

    /// Print the formal .oxyaddon package specification
    Spec,
}

pub fn run_cli(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        None | Some(Commands::Gui) => {
            // Handled in main.rs by launching eframe GUI
            Ok(())
        }

        Some(Commands::Convert {
            input,
            output,
            ecosystem,
            install,
            name,
            author,
        }) => {
            let options = ConversionOptions {
                addon_name: name,
                addon_author: author,
                ..Default::default()
            };

            let ext = input
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            let (result, extra_files) = if ext == "zip" {
                let eco = match ecosystem {
                    Some(e) => e.into(),
                    None => crate::converters::detect_zip_ecosystem(&input),
                };
                match eco {
                    SourceEcosystem::Zbrush => {
                        let (res, extra) = crate::converters::zbrush::convert_zbrush_zip(&input, &options)?;
                        (res, extra)
                    }
                    _ => {
                        let res = crate::converters::blender::convert_blender_zip(&input, &options)?;
                        (res, std::collections::BTreeMap::new())
                    }
                }
            } else if ext == "zsc" {
                let bytes = fs::read(&input)?;
                let file_name = input.file_name().and_then(|n| n.to_str()).unwrap_or("plugin.zsc");
                let res = crate::converters::zbrush::convert_zbrush_zsc(&bytes, file_name, &options);
                (res, std::collections::BTreeMap::new())
            } else {
                let bytes = fs::read(&input)?;
                let content = String::from_utf8_lossy(&bytes).into_owned();
                let file_name = input.file_name().and_then(|n| n.to_str());
                let eco = match ecosystem {
                    Some(e) => e.into(),
                    None => detect_ecosystem(&content, file_name),
                };
                let res = convert_script(&content, eco, &options);
                (res, std::collections::BTreeMap::new())
            };

            println!("Conversion summary:");
            println!("  Ecosystem: {}", result.source_ecosystem.display_name());
            println!("  Name:      {}", result.manifest.name);
            println!("  ID:        {}", result.manifest.id);
            println!("  Category:  {}", result.manifest.category.display_name());
            println!("  Mapped:    {} API operations", result.mapped_calls.len());
            if !extra_files.is_empty() {
                println!("  Bundled:   {} companion asset(s)", extra_files.len());
            }

            for w in &result.warnings {
                eprintln!("  [Warning] {w}");
            }

            let mut package = AddonPackage::new(result.manifest.clone(), result.python_code);
            package.extra_files = extra_files;

            // Determine output path if specified or install
            let out_path = output.unwrap_or_else(|| {
                let stem = package.manifest.safe_stem();
                PathBuf::from(format!("{stem}.oxyaddon"))
            });

            package.save_to_archive(&out_path)?;
            println!("Saved package: {}", out_path.display());

            if install {
                let installed_path = install_addon(&package, false, true)?;
                println!("Installed to OxyBrush: {}", installed_path.display());
            }

            Ok(())
        }

        Some(Commands::Pack { dir, output }) => {
            let package = AddonPackage::from_directory(&dir)?;
            package.save_to_archive(&output)?;
            println!("Packed '{}' into {}", package.manifest.name, output.display());
            Ok(())
        }

        Some(Commands::Unpack { package, output }) => {
            let pkg = AddonPackage::open(&package)?;
            pkg.unpack_to_directory(&output)?;
            println!("Unpacked '{}' into {}", pkg.manifest.name, output.display());
            Ok(())
        }

        Some(Commands::Install { package }) => {
            let pkg = AddonPackage::open(&package)?;
            let path = install_addon(&pkg, false, true)?;
            println!("Successfully installed '{}' to {}", pkg.manifest.name, path.display());
            Ok(())
        }

        Some(Commands::Inspect { package }) => {
            let pkg = AddonPackage::open(&package)?;
            println!("--------------------------------------------------");
            println!("Addon Name:    {}", pkg.manifest.name);
            println!("ID:            {}", pkg.manifest.id);
            println!("Version:       {}", pkg.manifest.version);
            println!("Author:        {}", pkg.manifest.author);
            println!("Category:      {}", pkg.manifest.category.display_name());
            println!("Target App:    {}", pkg.manifest.target_app);
            println!("Min Version:   {}", pkg.manifest.min_oxybrush_version);
            println!("Entry Point:   {}", pkg.manifest.entry_point);
            println!("Description:   {}", pkg.manifest.description);
            println!("Extra files:   {} resource(s)", pkg.extra_files.len());
            for f in pkg.extra_files.keys() {
                println!("  - {f}");
            }
            println!("--------------------------------------------------");
            Ok(())
        }

        Some(Commands::List) => {
            let list = list_installed_addons()?;
            if let Some(dir) = oxybrush_plugins_dir() {
                println!("OxyBrush Plugins Directory: {}", dir.display());
            }
            println!("Installed Addons ({}):", list.len());
            for item in list {
                let kind = if item.is_archive {
                    ".oxyaddon"
                } else if item.is_directory {
                    "directory"
                } else {
                    ".py script"
                };
                println!("  • {:<12} {}", kind, item.name);
            }
            Ok(())
        }

        Some(Commands::Spec) => {
            println!("{}", include_str!("../SPEC_OXYADDON.md"));
            Ok(())
        }
    }
}
