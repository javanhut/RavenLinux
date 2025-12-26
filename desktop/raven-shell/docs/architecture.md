# Raven Shell Architecture

## Overview

Raven Shell is a unified desktop shell for Hyprland, consolidating all GUI components into a single Rust binary. It provides a panel/taskbar, desktop background, application menu, power menu, settings interface, keybindings overlay, file manager, and system tools (WiFi manager, USB creator, system installer).

## Project Structure

```
raven-shell/
├── Cargo.toml                    # Workspace manifest
├── crates/
│   ├── raven-shell/              # Main binary
│   │   └── src/
│   │       ├── main.rs           # Entry point, CLI
│   │       ├── daemon.rs         # Component lifecycle
│   │       └── ipc.rs            # Unix socket for raven-ctl
│   │
│   ├── raven-core/               # Shared library
│   │   └── src/
│   │       ├── config/           # Unified settings
│   │       ├── services/         # Async services
│   │       ├── state/            # Shared state
│   │       ├── messages/         # Events & commands
│   │       ├── theme/            # CSS & icons
│   │       ├── desktop/          # .desktop parsing
│   │       └── utils/
│   │
│   ├── raven-components/         # UI components
│   │   └── src/
│   │       ├── common/           # Component trait, LayerWindow
│   │       ├── panel/            # Taskbar
│   │       ├── desktop/          # Background
│   │       ├── menu/             # App launcher
│   │       ├── power/            # Power menu
│   │       ├── settings/         # Settings UI
│   │       ├── keybindings/      # Shortcuts overlay
│   │       └── file_manager/     # File browser
│   │
│   └── raven-tools/              # System tools (optional)
│       └── src/
│           ├── wifi/             # WiFi manager
│           ├── usb/              # USB creator
│           └── installer/        # System installer
```

## Core Architecture

### Service Hub

The `ServiceHub` manages all async services running on a dedicated tokio runtime:

- **HyprlandService**: Window management via Hyprland IPC
- **ProcessService**: Application launching and power commands
- **ConfigWatcher**: Hot-reload configuration files

### Message Bus

Components communicate via events and commands:

```
ShellEvent (services -> components):
  - WindowOpened, WindowClosed, WindowFocused
  - ConfigReloaded, SettingsReloaded
  - ShowComponent, HideComponent, ToggleComponent

ShellCommand (components -> services):
  - FocusWindow, CloseWindow, MinimizeWindow
  - LaunchApp, Lock, Reboot, Shutdown
  - ShowComponent, HideComponent
```

### Component Trait

All UI components implement the `Component` trait:

```rust
pub trait Component {
    fn id(&self) -> ComponentId;
    fn init(&mut self, ctx: ComponentContext);
    fn show(&self);
    fn hide(&self);
    fn is_visible(&self) -> bool;
    fn toggle(&self);
    fn handle_event(&self, event: &ShellEvent);
    fn shutdown(&self);
    fn is_always_visible(&self) -> bool;
    fn window(&self) -> Option<&gtk4::Window>;
}
```

Note: Components are not `Send` as they contain GTK widgets that must run on the main thread.

### Layer Shell Integration

Components use GTK4 with gtk4-layer-shell for proper Wayland integration:

- **Panel**: Top layer, anchored to screen edges
- **Desktop**: Background layer, full screen
- **Overlays**: Overlay layer with keyboard grab

## Configuration

### Settings File

Location: `~/.config/raven/settings.json`

```json
{
  "panel_position": "top",
  "panel_height": 38,
  "theme": "dark",
  "wallpaper_path": "/path/to/wallpaper.jpg",
  "show_clock": true,
  "enable_animations": true
}
```

### Dock Configuration

Location: `~/.config/raven-shell/dock.json`

```json
{
  "pinned_apps": [
    {
      "id": "firefox",
      "name": "Firefox",
      "command": "firefox",
      "icon": "firefox",
      "pinned": true
    }
  ]
}
```

## Usage

### Running the Daemon

```bash
# Start the shell (default)
raven-shell

# Or explicitly
raven-shell daemon
```

### Control via IPC

```bash
# Show/hide/toggle components
raven-ctl show menu
raven-ctl hide power
raven-ctl toggle settings

# Reload configuration
raven-ctl reload-config

# Show status
raven-ctl status
```

## Building

```bash
cd raven-shell
cargo build --release
```

### Feature Flags

```bash
# Desktop components only (default)
cargo build --release

# Include tools
cargo build --release --features tools

# Full build
cargo build --release --features full
```

## Implemented Components

### Phase 1: Core Infrastructure (Complete)
- ServiceHub with HyprlandService, ProcessService, ConfigWatcher
- MessageBus for inter-component communication
- Component trait and LayerWindow abstraction
- Unified configuration system

### Phase 2: Always-On Components (Complete)
- **PanelComponent**: Top bar with start button, dock, clock, and power/settings buttons
- **DesktopComponent**: Full-screen background layer with wallpaper and icon grid
  - Wallpaper display with automatic fallback to default locations
  - Desktop icons from ~/Desktop folder and pinned apps config
  - Double-click to launch applications
  - Right-click context menu with:
    - Open Terminal, Open File Manager, Applications launcher
    - New Folder creation (auto-increments name if exists)
    - New Text File creation (auto-increments name if exists)
    - Change Wallpaper with built-in file chooser dialog
    - Desktop Settings, Refresh Desktop
  - Drag-and-drop file support

### Phase 3: Overlay Components (Complete)
- **MenuComponent**: Application launcher with category sidebar, search, and app list
  - Loads applications from .desktop files
  - Category-based filtering (All, System, Utilities, Development, etc.)
  - Power buttons for quick logout/reboot/shutdown
- **PowerComponent**: Fullscreen power menu overlay
  - Six options: Lock, Logout, Suspend, Hibernate, Reboot, Shutdown
  - Press Escape to dismiss
- **KeybindingsComponent**: Keyboard shortcuts reference overlay
  - Eight categories of shortcuts displayed in two columns
  - Press any key or click to dismiss

### Phase 4: Complex Components (Complete)
- **SettingsComponent**: Multi-page settings interface
  - Eight category pages: Appearance, Desktop, Panel, Windows, Input, Power, Sound, About
  - Category sidebar with icons and descriptions
  - Real-time settings persistence to `~/.config/raven/settings.json`
  - Theme selection, accent color picker, font size, panel opacity
  - Wallpaper selection with file browser dialog
  - Panel position, clock format, workspace settings
  - Window border width, gap size, focus-follows-mouse
  - Keyboard layout, mouse speed, touchpad settings
  - Screen timeout, suspend timeout, lid close action
  - Volume control, mute on lock option
  - System information in About page
- **FileManagerComponent**: Full-featured file browser (regular window, not layer-shell)
  - Navigation: back, forward, up, home buttons with history
  - Location bar with direct path entry
  - Search with real-time filtering
  - Sidebar with bookmarks (Home, Desktop, Documents, Downloads, Pictures, Videos, Music, Trash)
  - File list view with icon, name, size, date columns
  - Multi-select with Ctrl+click
  - Preview pane with file details (toggleable with Ctrl+P)
  - File operations: copy (Ctrl+C), cut (Ctrl+X), paste (Ctrl+V)
  - Delete (to trash) and Shift+Delete (permanent)
  - Rename with F2
  - New folder with Ctrl+Shift+N
  - Toggle hidden files with Ctrl+H
  - Refresh with F5
  - Status bar with file counts and disk space

### Phase 5: Tools Integration (Complete)
- **WiFiTool**: Network manager with IWD/wpa_supplicant backends
  - Automatic backend detection (IWD, wpa_supplicant, or iw fallback)
  - Interface detection via sysfs
  - Network scanning with signal strength and security info
  - Connect/disconnect with password support
  - DHCP client fallback chain (dhcpcd, dhclient, udhcpc)
  - Saved networks management with forget capability
  - Material Design dark theme UI
- **UsbTool**: Bootable USB creator with wizard interface
  - 6-page wizard: Welcome, Device Selection, ISO Selection, Confirm, Writing, Complete
  - USB device detection via /sys/block
  - ISO/IMG file validation
  - 4MB buffered writing for performance
  - Progress tracking during write operations
  - FAT32 formatting support
  - Device eject after completion
- **InstallerTool**: System installer with guided wizard
  - 6-page wizard: Welcome, Disk Selection, Partitioning, Configuration, Installing, Complete
  - Non-removable disk detection
  - GPT partitioning with 512MB EFI + ext4 root
  - User account creation with wheel group
  - Timezone and keyboard layout selection
  - System file copy via rsync
  - fstab generation with UUID
  - GRUB bootloader installation for UEFI
  - Essential services enablement (NetworkManager, sddm)

## Dependencies

- GTK4 + gtk4-layer-shell
- tokio (async runtime)
- hyprland (IPC bindings)
- serde (serialization)
- clap (CLI)
- regex (pattern matching for WiFi/network parsing)
- glob (file pattern matching for saved networks)
