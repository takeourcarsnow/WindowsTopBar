# TopBar

A native Windows 11 topbar application inspired by macOS and [Droptop Four](https://github.com/Droptop-Four/Droptop-Four).

![TopBar Screenshot](docs/screenshot.png)

## Features

- **macOS-style Menu Bar**: Clean, minimal design that sits at the top of your screen
- **System Information**: CPU, memory usage, battery status, network connectivity
- **Clock & Date**: Customizable date/time display
- **Light/Dark Themes**: Automatic theme switching based on Windows settings, or manual override
- **Windows 11 Effects**: Acrylic blur, rounded corners, and modern styling
- **System Tray Icon**: Quick access and background operation
- **Hotkey Support**: Customizable keyboard shortcuts
- **Drag & Drop Reordering**: Reorder modules by dragging items in the bar; order is saved automatically
- **Low Resource Usage**: Native Rust implementation for optimal performance

## Modules

| Module | Description |
|--------|-------------|
| **App Menu** | macOS-style application menu with system controls |
| **Active Window** | Shows the currently focused application |
| **Clock** | Date and time display (12h/24h format) |
| **Battery** | Battery percentage and charging status |
| **Volume** | Audio volume control with scroll support |
| **Network** | WiFi/Ethernet connection status and speeds (MB/s) |
| **System Info** | CPU and memory usage |
| **Media** | Now playing info and playback controls |
| **Weather** | Current weather conditions (requires API key) |

## Installation

### MSI Installer (Recommended)

Download the latest `topbar-x.x.x-x86_64.msi` from the [Releases](https://github.com/yourusername/topbar/releases) page and run it. The installer will:

- Install TopBar to `Program Files\topbar`
- Add TopBar to your PATH
- Create a Start Menu shortcut
- Include the license and uninstaller

### From Source

1. Install [Rust](https://rustup.rs/) (1.70+)
2. Clone this repository:
   ```powershell
   git clone https://github.com/yourusername/topbar.git
   cd topbar
   ```
3. Build and run:
   ```powershell
   cargo build --release
   .\target\release\topbar.exe
   ```

### Pre-built Binaries

Download the latest release from the [Releases](https://github.com/yourusername/topbar/releases) page.

## Configuration

Configuration is stored in `%APPDATA%\topbar\config.toml`. The file is created automatically on first run. Module order changes made via drag-and-drop are saved to this file automatically.

### Example Configuration

```toml
[general]
start_with_windows = false
show_in_taskbar = false
language = "en"

[appearance]
theme_mode = "Auto"  # "Light", "Dark", or "Auto"
bar_height = 28
opacity = 0.85
blur_enabled = true
font_family = "Segoe UI Variable"
font_size = 13
position = "Top"  # "Top" or "Bottom"

[modules.clock]
format_24h = false
show_seconds = false
show_date = true
show_day = true

[modules.network]
show_icon = true
show_name = false
show_speed = false

[modules.system_info]
show_cpu = true
show_memory = true
update_interval_ms = 2000

[behavior]
auto_hide = false
reserve_space = true
all_desktops = true

[hotkeys]
toggle_bar = "Alt+T"
open_menu = "Alt+Space"
toggle_theme = "Alt+D"
```

## Hotkeys

| Hotkey | Action |
|--------|--------|
| `Alt+T` | Toggle bar visibility |
| `Alt+Space` | Open application menu |
| `Alt+S` | Quick search |
| `Alt+D` | Toggle dark/light theme |

## Quick Search (Prototype)

A fast filename-only quick search was added using `fst` + `walkdir`.

- Builds a compact prefix-search index of filenames (stored as `lowercase_filename\0full_path`).
- Index is built in a background thread at startup and kept up-to-date via a filesystem watcher (`notify`).
- Open the search popup with the hotkey (`Alt+S` by default). Type to search filename prefixes; press Enter to open the first match.

Configuration (in `config.toml` under `[search]`):

- `enabled` (bool) — enable/disable indexing (default: true)
- `index_paths` (array) — paths to index (defaults to your home directory)
- `exclude_patterns` (array) — simple substrings to ignore (currently used for default excludes)

This is an initial prototype; future improvements include content indexing (via `tantivy`), fuzzy matching, faster persistence, and optional Windows Everything SDK integration for ultra-fast filename-only searches.

## System Requirements

- Windows 10 (build 1903+) or Windows 11
- For best visual effects: Windows 11 22H2+

## Architecture

```
src/
├── main.rs           # Entry point
├── app.rs            # Main application logic
├── config.rs         # Configuration management
├── window.rs         # Window creation and management
├── theme.rs          # Theming system
├── effects.rs        # Windows 11 blur/mica effects
├── hotkey.rs         # Global hotkey handling
├── tray.rs           # System tray icon
├── utils.rs          # Utility functions
├── error.rs          # Error types
├── render/           # Rendering system
│   ├── mod.rs        # Main renderer
│   ├── dropdown.rs   # Dropdown menus
│   └── icons.rs      # Icon management
└── modules/          # Topbar modules
    ├── mod.rs        # Module registry
    ├── clock.rs      # Clock module
    ├── battery.rs    # Battery module
    ├── volume.rs     # Volume module
    ├── network.rs    # Network module
    ├── system_info.rs # System info module
    ├── app_menu.rs   # Application menu
    ├── active_window.rs # Active window tracking
    ├── media.rs      # Media controls
    └── weather.rs    # Weather display
```

## Building

### Debug Build
```powershell
cargo build
```

### Release Build (Optimized)
```powershell
cargo build --release
```

### Run with Logging
```powershell
$env:RUST_LOG="info"; cargo run
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Inspired by [Droptop Four](https://github.com/Droptop-Four/Droptop-Four) Rainmeter skin
- Design influenced by macOS menu bar
- Built with the [windows-rs](https://github.com/microsoft/windows-rs) crate

## Roadmap

- [ ] Plugin system for custom modules
- [ ] Multi-monitor support
- [ ] Calendar widget
- [ ] Settings GUI
- [ ] Notification center integration
- [ ] Customizable themes
- [ ] More widgets (Spotify, Weather, etc.)
