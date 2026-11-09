# AGENTS.md — instructions for AI coding agents

This file tells AI agents (Claude Code, Codex, Copilot, Cursor, Antigravity, and others) how to work on OxyConvert. Humans should read [CONTRIBUTING.md](CONTRIBUTING.md); everything here is consistent with it, and where the two differ, CONTRIBUTING.md and the documents it links win.

## Project in one paragraph

OxyConvert is a pure-Rust desktop application and CLI tool that converts scripts and plugins from ZBrush (ZScript / compiled `.zsc` / ZIP installers), Blender (`bpy` Python / `.zip`), and Nomad Sculpt (JavaScript / JSON presets) into the **`.oxyaddon`** packaging standard for OxyBrush. It supports dual targeting: in-process WebAssembly plugins (`kind: "wasm"`, RFC 0011) and background Python addons (`kind: "python"`).

## Repository layout

```
OxyConvert/
  src/
    converters/
      mod.rs        ScriptConverter trait, ecosystem detection, universal conversion dispatch
      zbrush.rs     ZScript command parser, compiled .zsc reader, ZBrush installer ZIP unpacker
      blender.rs    bl_info parser, bpy.ops transpiler, Blender zip addon converter
      nomad.rs      Nomad Sculpt JavaScript & JSON preset parser/transpiler
      wasm.rs       WebAssembly in-app plugin generator (.wat) according to OxyBrush RFC 0011
    package/
      bundle.rs     AddonPackage archive packer/unpacker, ZIP entry validator
      manifest.rs   AddonManifest, SourceEcosystem, AddonCategory, UiBinding, safe_stem
      installer.rs  Detects OxyBrush plugins folder across platforms, deploys packages
    ui/
      app.rs        eframe::App implementation, dual-pane studio, menus, toolbar
      file_dialog.rs Built-in modal file browser, bookmarks, filtering
      theme.rs      Graphite, ZBrush Classic, Charcoal, Slate, Warm Umber, High Contrast
      widgets.rs    Custom buttons, badges, chips, section headers
    cli.rs          CLI subcommands (convert, pack, unpack, install, inspect, list, spec, gui)
    lib.rs          Library exports
    main.rs         Desktop application entrypoint
  packaging/
    icons/          Application icons (.ico)
    windows/        installer.nsi (NSIS installer), oxyconvert.iss (Inno Setup)
  SPEC_OXYADDON.md  Formal specification of the .oxyaddon package standard
  Cargo.toml        Project configuration and dependencies
  rust-toolchain.toml Pinned compiler toolchain
```

## Commands

The toolchain is pinned in `rust-toolchain.toml` (1.99.0 matching OxyBrush). Run these before every commit; CI runs the same:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Useful packaging & cross-compilation commands:

```bash
# Build native Linux binary
cargo build --release

# Cross-compile native Windows release binary (x86_64-pc-windows-msvc)
cargo xwin build --release --target x86_64-pc-windows-msvc

# Build Windows Setup installer (.exe) via NSIS
cd packaging/windows && makensis installer.nsi

# Run CLI conversion
cargo run -- convert <input_script_or_zip> -o <output.oxyaddon>
```

## Architecture invariants (do not break)

1. **Pure Rust and Safety.** No unsafe code (`#[forbid(unsafe_code)]` is enforced). No external C/C++ toolchain dependencies needed for core logic.
2. **Dual-Targeting Support.** An `.oxyaddon` can target either in-app WebAssembly (`kind: "wasm"`) or Python (`kind: "python"`). Both manifests must conform 100% to OxyBrush's `oxy_plugin::addon::inspect` and `Manifest::parse`.
3. **Safe File Operations.** Unpacking and installing must never write outside `<plugins>/<id>/` (no path traversal with `..`, no absolute paths, no zip bombs).
4. **Unique Package IDs.** Manifest IDs and directory stems must be uniquely formatted using `safe_stem()`. Never use hardcoded or generic IDs like `com_example_addon` that could overwrite user plugins.
5. **Lossless Companion Assets.** ZBrush materials (`.ZMT`), textures (`.psd`), Substance files (`.spp`), and meshes (`.obj`) bundled inside source archives must be preserved in the `.oxyaddon`'s `assets/` directory.

## Clean-room & Legal Guidelines

- **Clean-room transpilation only.** Transpilation relies solely on public API mapping and grammar parsing. Never decompile or circumvent proprietary software protections.
- **Licenses.** Dependencies must remain compatible with MIT OR Apache-2.0.
- **Trademarks.** Refer to external platforms (ZBrush, Blender, Nomad Sculpt) purely to describe input format compatibility.
