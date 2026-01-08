# TopBar

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Windows](https://img.shields.io/badge/platform-Windows-blue)](https://github.com/microsoft/windows-rs)

A sleek, native Windows 11 topbar application inspired by macOS, built with Rust for optimal performance and modern aesthetics.

![TopBar Screenshot](docs/screenshot.png)

## 📋 Table of Contents

- [✨ Features](#-features)
- [🚀 Quick Start](#-quick-start)
- [📦 Installation](#-installation)
- [⚙️ Configuration](#️-configuration)
- [🧩 Modules](#-modules)
- [⌨️ Hotkeys](#️-hotkeys)
- [🔍 Quick Search](#-quick-search)
- [💻 System Requirements](#-system-requirements)
- [🛠️ Development](#️-development)
- [🤝 Contributing](#-contributing)
- [📄 License](#-license)
- [🙏 Acknowledgments](#-acknowledgments)
- [🗺️ Roadmap](#️-roadmap)

## ✨ Features

- **🎨 macOS-Inspired Design**: Clean, minimal topbar with native Windows 11 integration
- **📊 Real-Time System Monitoring**: CPU, memory, battery, network, and more
- **🌓 Smart Theming**: Automatic light/dark mode switching with manual override
- **🌟 Windows 11 Effects**: Acrylic blur, rounded corners, and Mica support
- **🎛️ Customizable Modules**: Drag-and-drop reordering with extensive configuration
- **⚡ Low Resource Usage**: Native Rust implementation for minimal CPU/memory footprint
- **🔥 Hotkey Support**: Global shortcuts for quick access
- **📱 System Tray Integration**: Background operation with tray icon
- **🔎 Fast Search**: Built-in filename search with real-time indexing

## 🚀 Quick Start

1. **Download** the latest MSI installer from [Releases](https://github.com/yourusername/topbar/releases)
2. **Install** by running the MSI (requires admin privileges)
3. **Launch** TopBar from the Start Menu or system tray
4. **Customize** via the config file at `%APPDATA%\topbar\config.toml`

That's it! TopBar will appear at the top of your screen with default modules.

## 📦 Installation

### 🏆 Recommended: MSI Installer

For the best experience, use our production-ready MSI installer:

1. Download `topbar-x.x.x-x86_64.msi` from [Releases](https://github.com/yourusername/topbar/releases)
2. Run the installer (administrator privileges required)
3. Follow the setup wizard
4. Launch from Start Menu or system tray

**What the installer provides:**
- ✅ System-wide installation to `Program Files`
- ✅ PATH environment variable addition
- ✅ Start Menu shortcut
- ✅ Automatic uninstaller registration
- ✅ License agreement and documentation

### 🔧 From Source

If you prefer building from source:

**Prerequisites:**
- [Rust](https://rustup.rs/) 1.70 or later
- Windows 10 build 1903+ or Windows 11

```bash
# Clone the repository
git clone https://github.com/yourusername/topbar.git
cd topbar

# Build in release mode
cargo build --release

# Run
./target/release/topbar.exe
```

### 📥 Pre-built Binaries

Download standalone executables from [Releases](https://github.com/yourusername/topbar/releases) for manual installation.

## ⚙️ Configuration

TopBar uses a TOML configuration file located at `%APPDATA%\topbar\config.toml`. The file is created automatically on first launch with sensible defaults.

### Key Configuration Sections

```toml
[general]
start_with_windows = false  # Auto-start on login
show_in_taskbar = false     # Show in taskbar
language = "en"             # Interface language

[appearance]
theme_mode = "Auto"         # "Light", "Dark", or "Auto"
bar_height = 28             # Height in pixels
opacity = 0.85              # Background opacity (0.0-1.0)
blur_enabled = true         # Enable acrylic blur
position = "Top"            # "Top" or "Bottom"

[modules.clock]
format_24h = false          # 12/24 hour format
show_seconds = false        # Display seconds
show_date = true            # Show date
show_day = true             # Show day of week

[modules.system_info]
show_cpu = true             # CPU usage percentage
show_memory = true          # Memory usage
update_interval_ms = 2000   # Refresh rate

[behavior]
auto_hide = false           # Auto-hide when not hovered
reserve_space = true        # Reserve screen space
all_desktops = true         # Show on all virtual desktops
```

**Pro Tip:** Changes to module order via drag-and-drop are saved automatically!

## 🧩 Modules

TopBar's functionality comes from customizable modules. Each module can be enabled/disabled and configured independently.

| Module | Description | Configurable |
|--------|-------------|--------------|
| **App Menu** | macOS-style system menu with controls | Layout, actions |
| **Active Window** | Shows current focused application | Display format |
| **Clock** | Date and time with multiple formats | 12h/24h, date display |
| **Battery** | Battery status and charging info | Icons, percentages |
| **Volume** | Audio controls with scroll support | Device selection |
| **Network** | WiFi/Ethernet status and speeds | Speed display, icons |
| **System Info** | CPU/memory usage graphs | Update intervals |
| **Media** | Now playing info and controls | Player integration |
| **Weather** | Current conditions (API required) | Location, units |
| **GPU** | Graphics card monitoring | Usage graphs |
| **Bluetooth** | Bluetooth device status | Device list |
| **Night Light** | Blue light filter toggle | Schedule |
| **Uptime** | System uptime display | Format |

## ⌨️ Hotkeys

TopBar supports global hotkeys for quick access (customizable in config):

| Hotkey | Action | Default |
|--------|--------|---------|
| `Alt + T` | Toggle bar visibility | ✓ |
| `Alt + Space` | Open application menu | ✓ |
| `Alt + S` | Quick search | ✓ |
| `Alt + D` | Toggle theme | ✓ |
| `Alt + M` | Media controls | - |
| `Alt + V` | Volume mixer | - |

## 🔍 Quick Search

TopBar includes a fast, filename-based search feature powered by `fst` and `walkdir`.

**How it works:**
- Builds a compact index of filenames at startup
- Real-time updates via filesystem watcher
- Prefix matching for instant results
- Opens files/folders with Enter

**Configuration:**
```toml
[search]
enabled = true
index_paths = ["C:\\Users", "D:\\Documents"]  # Paths to index
exclude_patterns = ["node_modules", ".git"]   # Patterns to skip
```

**Future enhancements:** Content indexing, fuzzy matching, Windows Everything integration.

## 💻 System Requirements

**Minimum:**
- Windows 10 version 1903 (19H1) or later
- 4GB RAM
- 100MB free disk space

**Recommended:**
- Windows 11 22H2 or later (for Mica/Acrylic effects)
- 8GB RAM
- SSD storage

**Performance:** Typically uses <10MB RAM and <1% CPU in idle state.

## 🛠️ Development

### Building

```bash
# Debug build
cargo build

# Optimized release build
cargo build --release

# Create MSI installer
cargo install cargo-wix
cargo wix
```

### Running with Debug Logging

```powershell
$env:RUST_LOG="topbar=debug"; cargo run
```

### Project Structure

```
src/
├── main.rs              # Application entry point
├── app.rs               # Core application logic
├── config.rs            # Configuration management
├── window/              # Window management
│   ├── manager.rs       # Main window controller
│   ├── renderer.rs      # Rendering engine
│   └── menus.rs         # Context menus
├── render/              # Rendering system
│   ├── modules.rs       # Module rendering
│   ├── drawing.rs       # Drawing utilities
│   └── icons.rs         # Icon management
├── modules/             # TopBar modules
│   ├── clock.rs         # Time/date display
│   ├── battery.rs       # Power management
│   └── *.rs             # Other modules
└── utils.rs             # Shared utilities
```

### Testing

```bash
cargo test
cargo test --release  # Test optimized build
```

## 🤝 Contributing

We welcome contributions! Here's how to get started:

1. **Fork** the repository
2. **Create** a feature branch: `git checkout -b feature/amazing-feature`
3. **Commit** your changes: `git commit -m 'Add amazing feature'`
4. **Push** to the branch: `git push origin feature/amazing-feature`
5. **Open** a Pull Request

### Development Guidelines

- Follow Rust's official style guidelines
- Add tests for new features
- Update documentation
- Ensure Windows 10/11 compatibility

### Reporting Issues

- Use [GitHub Issues](https://github.com/yourusername/topbar/issues) for bugs
- Include Windows version, TopBar version, and steps to reproduce
- Attach config files and log output when possible

## 📄 License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.

## 🙏 Acknowledgments

- **Inspiration**: [Droptop Four](https://github.com/Droptop-Four/Droptop-Four) Rainmeter skin
- **Design**: macOS menu bar aesthetics
- **Technology**: [windows-rs](https://github.com/microsoft/windows-rs) for Win32 API bindings
- **Community**: Rust and Windows developer communities

## 🗺️ Roadmap

### 🚀 High Priority
- [ ] Plugin system for third-party modules
- [ ] Multi-monitor support
- [ ] Settings GUI application
- [ ] Enhanced search with content indexing

### 🔄 Medium Priority
- [ ] Calendar widget integration
- [ ] Notification center
- [ ] Custom theme engine
- [ ] Advanced hotkey customization

### 💡 Future Ideas
- [ ] Touch/screen reader support
- [ ] Linux/macOS ports
- [ ] Cloud sync for settings
- [ ] Hardware monitoring expansion

---

**Made with ❤️ in Rust for Windows users**
