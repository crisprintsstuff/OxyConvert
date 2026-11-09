# .oxyaddon Package Specification (Version 1.0)

The `.oxyaddon` system is the standard package distribution and installation format for plugins, tools, procedural generators, and pipeline extensions across **OxyBrush** and the **Oxy** creative suite.

---

## 1. Package Structure

An `.oxyaddon` file is a ZIP-compressed archive (or directory) containing the following structure:

```text
my_tool.oxyaddon/
├── manifest.json         # REQUIRED: Package metadata and entry points
├── main.py               # REQUIRED: Primary Python script communicating with OxyBrush
├── icon.png              # OPTIONAL: 64x64 or 128x128 PNG icon for OxyBrush menus
├── README.md             # OPTIONAL: Documentation and usage instructions
├── LICENSE               # OPTIONAL: License file
└── lib/                  # OPTIONAL: Additional python helper modules or assets
    └── utils.py
```

---

## 2. Manifest Schema (`manifest.json`)

The manifest contains all metadata needed by OxyBrush and OxyConvert:

```json
{
  "$schema": "https://raw.githubusercontent.com/crisprintsstuff/OxyBrush/main/schemas/oxyaddon-v1.json",
  "id": "com.author.addon_id",
  "name": "Human Readable Name",
  "version": "1.0.0",
  "author": "Author Name",
  "description": "Short summary of what this addon does.",
  "category": "Sculpting",
  "target_app": "OxyBrush",
  "entry_point": "main.py",
  "source_ecosystem": "native",
  "min_oxybrush_version": "0.5.0",
  "tags": ["sculpt", "dynamesh", "decimate"],
  "homepage": "https://github.com/author/my-tool",
  "license": "MIT",
  "permissions": [
    "scene:read",
    "scene:modify",
    "mesh:sculpt",
    "io:export"
  ],
  "ui": {
    "menu": "Plugins",
    "submenu": "Sculpting Tools",
    "button_label": "Run My Tool",
    "tooltip": "Execute procedural sculpt operations",
    "shortcut": "Ctrl+Shift+T"
  }
}
```

### Supported Categories:
- `Sculpting`: Brushes, strokes, deformation, curves
- `Topology`: Voxel remesh, decimation, subdivision, polygroups
- `Pipeline`: Import, export, batch processing, LOD generation
- `Procedural`: Primitive generation, spirals, algorithmic forms
- `Materials`: Matcaps, vertex coloring, PBR textures
- `Utility`: Diagnostics, statistics, scene cleanup

### Source Ecosystems:
- `zbrush`: Converted from ZBrush ZScript / ZPlugin
- `blender`: Converted from Blender Python `bpy` addon
- `nomad`: Converted from Nomad Sculpt JavaScript / JSON preset
- `native`: Developed natively with the `oxybrush` Python SDK

---

## 3. Script Execution Model

When OxyBrush runs an `.oxyaddon`:
1. OxyBrush verifies Python 3 on the host machine.
2. OxyBrush extracts or injects the pure-Python `oxybrush` SDK into `PYTHONPATH`.
3. OxyBrush spawns the worker process in an isolated background thread, passing:
   - `OXYBRUSH_MCP_ENDPOINT`: HTTP JSON-RPC endpoint (e.g. `http://127.0.0.1:41799/mcp`)
   - `OXYBRUSH_MCP_TOKEN`: Temporary authentication bearer token
4. The addon calls `app = oxybrush.attach()` to communicate with the active session.
5. All standard output printed by the script is routed to the OxyBrush status bar and log.

---

## 4. Installation Targets

OxyBrush automatically scans for plugins and `.oxyaddon` packages in:
- **Windows**: `%APPDATA%\oxybrush\plugins\` or `%LOCALAPPDATA%\OxyBrush\plugins\`
- **Linux**: `~/.local/share/oxybrush/plugins/`
- **macOS**: `~/Library/Application Support/oxybrush/plugins/`
- **Portable**: `./plugins/` relative to the `oxybrush` executable
