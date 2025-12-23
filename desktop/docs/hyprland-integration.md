# Raven Desktop - Hyprland Integration

This document describes how all Raven desktop components work together with the Hyprland compositor.

## Architecture Overview

```
Hyprland (Compositor)
    |
    +-- raven-desktop (Background layer - wallpaper & desktop icons)
    |
    +-- raven-shell (Panel/taskbar - top layer)
    |       |
    |       +-- Dock (running apps via hyprctl clients)
    |       +-- Clock
    |       +-- Settings menu
    |       +-- Power menu
    |
    +-- raven-menu (Application launcher - overlay layer)
    |
    +-- raven-settings-menu (Settings application - overlay layer)
```

## Components

### raven-shell (Panel/Taskbar)
The main panel that provides:
- Raven button (opens raven-menu)
- Application dock with running app indicators
- Clock display
- Settings quick access
- Power menu (logout, lock, reboot, shutdown)

Uses `hyprctl` for:
- Window discovery (`hyprctl clients -j`)
- Window focus (`hyprctl dispatch focuswindow`)
- Window close (`hyprctl dispatch closewindow`)
- Minimize to special workspace (`hyprctl dispatch movetoworkspacesilent`)
- Session logout (`hyprctl dispatch exit`)

### raven-desktop (Desktop Background)
Provides:
- Wallpaper display (via layer-shell background)
- Desktop icons
- Right-click context menu

### raven-menu (Application Launcher)
Start menu featuring:
- Application search
- Category browsing
- Power controls (logout, reboot, shutdown)

### raven-settings-menu (Settings Application)
Full settings panel for:
- Appearance (theme, colors, fonts)
- Desktop (wallpaper, icons)
- Panel (position, height, clock)
- Windows (borders, gaps, focus)
- Input (keyboard, mouse, touchpad)
- Power management
- Sound settings

## Shared Configuration

All components share settings via `~/.config/raven/settings.json`:

```json
{
  "theme": "dark",
  "accent_color": "#009688",
  "panel_position": "top",
  "panel_height": 38,
  "wallpaper_path": "/path/to/wallpaper.png",
  "wallpaper_mode": "fill",
  "border_width": 2,
  "gap_size": 8,
  "keyboard_layout": "us"
}
```

Component-specific configs:
- `~/.config/raven-shell/dock.json` - Pinned applications

## Installation

### Prerequisites

Install the following packages:
```bash
# Core
hyprland
gtk4
gtk4-layer-shell

# Utilities
swaybg          # Wallpaper
mako            # Notifications (or dunst)
wl-clipboard    # Clipboard support
grim            # Screenshot tool
slurp           # Region selection
brightnessctl   # Brightness control
playerctl       # Media control
wireplumber     # Audio (wpctl)
```

### Install Configuration

```bash
cd desktop/config
chmod +x install.sh
./install.sh
```

This installs:
- `~/.config/hypr/hyprland.conf` - Hyprland configuration
- `~/.config/raven/scripts/` - Helper scripts
- `~/.config/raven/settings.json` - Default settings

### Build Components

```bash
# Build all desktop components
cd desktop/raven-shell && go build -o raven-shell
cd desktop/raven-desktop && go build -o raven-desktop
cd desktop/raven-menu && go build -o raven-menu
cd desktop/raven-settings-menu && go build -o raven-settings-menu

# Install to path (example)
sudo cp raven-shell/raven-shell /usr/local/bin/
sudo cp raven-desktop/raven-desktop /usr/local/bin/
sudo cp raven-menu/raven-menu /usr/local/bin/
sudo cp raven-settings-menu/raven-settings-menu /usr/local/bin/
```

## Hyprland Configuration

The provided `hyprland.conf` includes:

### Startup
```conf
exec-once = raven-desktop
exec-once = raven-shell
exec-once = ~/.config/raven/scripts/set-wallpaper.sh
```

### Key Bindings

| Binding | Action |
|---------|--------|
| `Super + Return` | Open terminal |
| `Super + D` | Open raven-menu |
| `Super + Space` | Open launcher |
| `Super + Q` | Close window |
| `Super + Shift + Q` | Exit Hyprland |
| `Super + F` | Fullscreen |
| `Super + V` | Toggle floating |
| `Super + 1-0` | Switch workspace |
| `Super + Shift + 1-0` | Move to workspace |
| `Super + I` | Open settings |
| `Super + Escape` | Lock screen |

### Window Rules
```conf
windowrulev2 = float,class:^(raven-menu)$
windowrulev2 = float,class:^(raven-settings)$
windowrulev2 = float,class:^(raven-wifi)$

layerrule = blur,raven-shell
layerrule = blur,raven-desktop
```

## IPC Communication

### Hyprland to Raven

raven-shell polls `hyprctl clients -j` every 500ms to:
- Detect new windows
- Track window state changes
- Update dock indicators

### Raven to Hyprland

Components use `hyprctl dispatch` for:
- `focuswindow address:<addr>` - Focus a window
- `closewindow address:<addr>` - Close a window
- `movetoworkspacesilent special:minimized,address:<addr>` - Minimize
- `exit` - Logout

## Styling

All components use GTK4 with consistent CSS theming:
- Dark theme (`#0f1720` background)
- Teal accent (`#009688`)
- Semi-transparent panels
- Rounded corners (8px)

## Troubleshooting

### Panel not appearing
1. Check if gtk4-layer-shell is installed: `ls /usr/lib/libgtk4-layer-shell*`
2. Verify Hyprland is running: `hyprctl version`
3. Check logs: `journalctl --user -f`
4. Check if binary runs manually: `/bin/raven-shell 2>&1`

### Live ISO Issues

If desktop components don't render in the live ISO:

1. **Check library dependencies**:
   ```bash
   ldd /bin/raven-shell
   ldd /bin/raven-desktop
   ```
   Look for "not found" libraries.

2. **Check Wayland session log**:
   ```bash
   cat /run/raven-wayland-session.log
   ```

3. **Manually start components**:
   ```bash
   # In a terminal within Hyprland
   raven-shell 2>&1 | tee /tmp/shell.log &
   raven-desktop 2>&1 | tee /tmp/desktop.log &
   ```

4. **Verify environment**:
   ```bash
   echo $WAYLAND_DISPLAY
   echo $XDG_RUNTIME_DIR
   ls -la $XDG_RUNTIME_DIR/wayland-*
   ```

5. **Check Hyprland config loaded**:
   ```bash
   cat ~/.config/hypr/hyprland.conf | grep exec-once
   ```

6. **Check input devices**:
   ```bash
   ls -la /dev/input/
   cat /run/raven-wayland-session.log | grep -i input
   ```

7. **Check seatd is running**:
   ```bash
   ls -la /run/seatd.sock
   ```

### Dock not showing windows
1. Verify hyprctl works: `hyprctl clients -j`
2. Check window class isn't excluded
3. Restart raven-shell

### Settings not applying
1. Check settings.json syntax
2. Run apply-settings.sh manually
3. Restart affected component

## Development

### Building with Debug
```bash
go build -gcflags="all=-N -l" -o raven-shell
```

### Testing Layer Shell
```bash
# Check layer shell support
pkg-config --exists gtk4-layer-shell-0 && echo "OK"
```

### Environment Variables
```bash
export GTK_DEBUG=interactive  # GTK inspector
export G_MESSAGES_DEBUG=all   # GLib debug messages
```
