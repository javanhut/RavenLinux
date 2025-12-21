# Raven WiFi Manager

A lightweight WiFi management GUI for Linux built with Gio (pure Go, no CGo dependencies beyond system graphics libraries).

## Features

- ✅ **Zero GTK dependencies** - Uses Gio for pure Go GUI
- ✅ **Native Wayland support** - First-class Wayland with X11 fallback
- ✅ **Material Design dark theme** - Modern UI with teal accent
- ✅ **Single-click connect** - Fast, modern workflow
- ✅ **Persistent window size** - Remembers your preferred window dimensions
- ✅ **Minimal footprint** - 7.3MB optimized binary
- ✅ **Supports iwd, wpa_supplicant, or raw iw commands**

## Requirements

### System Dependencies
- OpenGL 2.0+ or EGL
- Wayland (recommended) or X11
- One of:
  - iwd (preferred)
  - wpa_supplicant
  - iw command

### Runtime
- Root privileges (for WiFi management)

## Building

```bash
# Standard build
go build -o raven-wifi .

# Optimized build
go build -ldflags="-s -w" -o raven-wifi .
```

## Usage

```bash
# Easy way: Use the wrapper script (automatically preserves environment)
./raven-wifi.sh

# Or run directly with environment preservation:
sudo -E ./raven-wifi

# Or if you have sudo configured to preserve environment:
sudo ./raven-wifi
```

**Important:** The `-E` flag preserves environment variables like `XDG_RUNTIME_DIR` which are needed for GUI display. Without it, you'll see an error about XDG_RUNTIME_DIR. The wrapper script (`raven-wifi.sh`) handles this automatically.

## UI Interactions

- **Click network** → Connect immediately (password dialog if needed)
- **Click connected network** → No action (already connected)
- **Saved button** → View and manage saved networks
- **Disconnect button** → Disconnect from current network
- **Refresh button** → Rescan for networks

## Configuration

Window size is automatically saved to:
```
~/.config/raven-wifi/window.json
```

## Architecture

### Files
- `main.go` - Application entry point and event loop
- `state.go` - Application state management
- `ui.go` - Main UI layout components
- `dialogs.go` - Password, error, and saved networks dialogs
- `theme.go` - Material Design dark theme
- `config.go` - Window geometry persistence
- `wifi.go` - WiFi backend (iwd/wpa_supplicant/iw)

### Design Philosophy
- **Immediate mode UI** - Gio's efficient rendering paradigm
- **Thread-safe state** - Mutex-protected shared state
- **Background operations** - Non-blocking network scans and connections
- **Responsive** - Minimum 300x400 window size, fully resizable

## Migration from GTK

This version replaces the previous GTK4/Adwaita implementation with Gio.

**Benefits:**
- No CGo compilation issues on custom distros
- Smaller binary (7.3MB vs GTK's system dependencies)
- Faster build times
- Same functionality, better portability

**Backed up files:**
- `main.go.gtk-backup` - Original GTK implementation

## License

Same as RavenLinux project.
