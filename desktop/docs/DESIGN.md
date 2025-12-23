# RavenDE - Raven Desktop Environment

## Design Philosophy

RavenDE is a modern, developer-focused desktop environment built from scratch for RavenLinux. It prioritizes:

1. **Performance** - Fast startup, low memory footprint, smooth animations
2. **Developer Workflow** - Keyboard-driven, configurable, distraction-free
3. **Modern Tech** - Wayland-native, GPU-accelerated, HDR support
4. **Aesthetics** - Clean, modern dark theme with customizable accents

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         raven-session                                │
│                    (Session Manager / Login)                         │
├─────────────────────────────────────────────────────────────────────┤
│                      raven-compositor                                │
│              (Wayland Compositor - wlroots based)                    │
│  ┌─────────────┬──────────────┬─────────────┬──────────────────┐    │
│  │   Windows   │  Workspaces  │  Tiling/    │  Animations/     │    │
│  │  Management │  Management  │  Floating   │  Effects         │    │
│  └─────────────┴──────────────┴─────────────┴──────────────────┘    │
├─────────────────────────────────────────────────────────────────────┤
│  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐    │
│  │raven-panel │  │raven-      │  │raven-notify│  │raven-      │    │
│  │  (Top Bar) │  │ launcher   │  │(Notif.     │  │ settings   │    │
│  │            │  │(App Launch)│  │ Daemon)    │  │            │    │
│  └────────────┘  └────────────┘  └────────────┘  └────────────┘    │
├─────────────────────────────────────────────────────────────────────┤
│                         Applications                                 │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐    │
│  │raven-files │  │raven-term  │  │raven-code* │  │  3rd Party │    │
│  │(File Mgr)  │  │(Terminal)  │  │(Editor Int)│  │    Apps    │    │
│  └────────────┘  └────────────┘  └────────────┘  └────────────┘    │
└─────────────────────────────────────────────────────────────────────┘

* raven-code is optional - focuses on integrations with existing editors
```

## Components

### 1. raven-compositor

The Wayland compositor is the core of the desktop. Built on wlroots for compatibility.

**Features:**
- Hybrid tiling/floating window management
- Workspace management with dynamic workspaces
- Smooth animations (60fps target)
- Multi-monitor support with per-monitor scaling
- HiDPI and fractional scaling support
- GPU acceleration via Vulkan/OpenGL
- Screen recording and screenshots built-in
- Input method support (ibus, fcitx5)

**Window Management Modes:**
1. **Floating** - Traditional overlapping windows
2. **Tiling** - Automatic window arrangement (BSP, columns, etc.)
3. **Stacking** - Tab-like grouping of windows
4. **Monocle** - Single window maximized

**Keybindings (Default):**
```
Super + Enter       → Open terminal
Super + Space       → Open launcher
Super + Q           → Close window
Super + 1-9         → Switch workspace
Super + Shift + 1-9 → Move window to workspace
Super + H/J/K/L     → Focus left/down/up/right
Super + Shift + H/J/K/L → Move window
Super + F           → Toggle fullscreen
Super + T           → Toggle tiling mode
Super + Tab         → Window switcher
```

### 2. raven-panel

Top panel providing system information and quick access.

**Sections:**
- **Left**: Application menu, workspace indicator
- **Center**: Window title / Clock
- **Right**: System tray, quick settings, user menu

**Features:**
- Auto-hide option
- Customizable modules
- Transparency effects
- Per-workspace customization

### 3. raven-launcher

Application launcher and command palette.

**Features:**
- Fuzzy search for applications
- Calculator mode (type math expressions)
- File search integration
- Command palette (: prefix for system commands)
- Custom actions and shortcuts
- Plugin system for extensions

**Modes:**
- `/` - File search
- `:` - System commands
- `>` - Terminal commands
- `=` - Calculator
- `@` - Contacts/communication
- Default - Application search

### 4. raven-notify

Notification daemon with do-not-disturb mode.

**Features:**
- Action buttons in notifications
- Notification history
- Do Not Disturb mode
- Per-app notification settings
- Notification grouping
- Critical notification handling

### 5. raven-files

Modern file manager with developer features.

**Features:**
- Miller columns and list/grid views
- Tabs and split panes
- Integrated terminal panel
- Git status integration
- Quick preview (images, text, code)
- Batch rename with regex
- Custom actions per file type
- Cloud storage integration

### 6. raven-term

GPU-accelerated terminal emulator.

**Features:**
- GPU-rendered text (like Alacritty)
- Tabs and splits
- Ligature support
- True color (24-bit)
- Configurable via TOML
- Shell integration (directory tracking, etc.)
- Clickable URLs
- Image protocol support (Sixel, Kitty)

### 7. raven-settings

System settings application.

**Categories:**
- Appearance (themes, fonts, colors)
- Display (resolution, scaling, night light)
- Input (keyboard, mouse, touchpad)
- Sound
- Network
- Power
- Users & Accounts
- Developer Tools
- About

## Technology Stack

### Core Libraries
- **wlroots** - Wayland compositor library
- **GTK4** or **Qt6** - UI toolkit (decision needed)
- **Vulkan/OpenGL** - Graphics rendering
- **D-Bus** - IPC
- **libinput** - Input handling
- **PipeWire** - Audio/screen capture

### Languages
- **Rust** - Primary language (compositor, terminal, core services)
- **C** - wlroots integration
- **Python/JavaScript** - Plugin scripting

### Configuration
- TOML for all configuration files
- Hot-reload support
- Schema validation

## Theming System

### Theme Components
1. **Color Scheme** - Base colors (background, foreground, accents)
2. **Icon Theme** - raven-icons (custom) or compatible themes
3. **Cursor Theme** - raven-cursors
4. **Font Configuration** - System fonts, code fonts
5. **Window Decorations** - Border styles, shadows

### Default Theme: "Raven Dark"
```toml
[colors]
background = "#0d1117"
background_alt = "#161b22"
foreground = "#c9d1d9"
foreground_dim = "#8b949e"
accent = "#58a6ff"
accent_alt = "#1f6feb"
success = "#3fb950"
warning = "#d29922"
error = "#f85149"
border = "#30363d"

[fonts]
ui = "Inter"
monospace = "JetBrains Mono"
ui_size = 11
monospace_size = 12

[window]
border_width = 2
border_radius = 8
shadow_size = 12
gap = 8
```

## Developer Integration

### Built-in Developer Features

1. **Quick Commands** (Super + P)
   - "Git: Commit" → Opens commit dialog
   - "Terminal: Here" → Opens terminal in current directory
   - "Project: Open" → Quick project switcher

2. **Workspace Templates**
   - "Code" workspace with specific layouts
   - "Terminal" workspace for terminal multiplexing
   - "Design" workspace with reference panels

3. **Environment Indicators**
   - Show active development environment in panel
   - Indicate running development servers
   - Container/VM status

4. **Integration Points**
   - VS Code / VSCodium
   - JetBrains IDEs
   - Neovim/Vim
   - Emacs

## Configuration Files

```
~/.config/raven/
├── compositor.toml      # Window management, keybindings
├── panel.toml           # Panel configuration
├── launcher.toml        # Launcher settings
├── terminal.toml        # Terminal configuration
├── theme.toml           # Colors and appearance
├── keybindings.toml     # Global keybindings
└── autostart/           # Autostart applications
    ├── polkit.desktop
    └── ...
```

## Implementation Roadmap

### Phase 1: Core (Foundation)
- [ ] Basic Wayland compositor with wlroots
- [ ] Window management (open, close, move, resize)
- [ ] Basic tiling support
- [ ] Keyboard shortcuts
- [ ] Multi-monitor support

### Phase 2: Shell Components
- [ ] Panel with basic modules
- [ ] Application launcher
- [ ] Notification daemon
- [ ] Settings app (basic)

### Phase 3: Applications
- [ ] File manager
- [ ] Terminal emulator
- [ ] Screenshot tool

### Phase 4: Polish
- [ ] Animations and effects
- [ ] Theming system
- [ ] Developer integrations
- [ ] Documentation

### Phase 5: Ecosystem
- [ ] Plugin system
- [ ] Theme repository
- [ ] Extension marketplace

## Building

```bash
# Build all RavenDE components
cd desktop
meson setup build
ninja -C build

# Run in nested Wayland session (for testing)
./build/raven-compositor --nested
```

## Dependencies

Build dependencies:
- meson, ninja
- rustc, cargo
- wayland-protocols
- wlroots (>= 0.17)
- gtk4 or qt6
- libinput
- pixman
- libdrm

Runtime dependencies:
- wayland
- xwayland (optional, for X11 app support)
- polkit
- dbus
