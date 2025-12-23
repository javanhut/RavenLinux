# Raven Shell Panel

The Raven Shell panel provides a customizable taskbar with application dock, system controls, and quick settings.

## Features

### Panel Orientation
The panel can be positioned on any edge of the screen:
- **Top** (default)
- **Bottom**
- **Left**
- **Right**

Change orientation via Settings menu > Panel Position. Changes are applied instantly without restart.

### Raven Button
The panel features a Raven-branded start button that opens the application menu (raven-menu).

### Running Application Tracking
The dock automatically detects running graphical applications via Hyprland's IPC and displays them in the center dock area. Running applications are indicated by a teal underline.

### Pinned Applications
Applications can be pinned to the dock for quick access. Pinned apps remain visible even when not running and are indicated by a slightly brighter background.

### Context Menu Actions
Right-click on any dock item to access:

- **Pin to Dock / Unpin from Dock**: Toggle whether the application stays in the dock
- **Minimize / Restore**: Minimize a running application to Hyprland's special workspace or restore it
- **Close**: Close the window via Hyprland

## Configuration

Raven Shell uses two configuration files:

### Dock Configuration
Pinned applications are saved to:
```
~/.config/raven-shell/dock.json
```

Example configuration:
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

### Raven Settings (Shared)
Panel position and other shared settings are stored in:
```
~/.config/raven/settings.json
```

This file is shared with raven-settings-menu and other Raven components. The relevant fields for raven-shell are:

```json
{
  "panel_position": "top",
  "panel_height": 38
}
```

### Panel Position Values
| Value | Position |
|-------|----------|
| "top" | Top (default) |
| "bottom" | Bottom |
| "left" | Left |
| "right" | Right |

## Hyprland Integration

The dock uses `hyprctl` to communicate with Hyprland for window management:

### Window Discovery
Windows are discovered by polling `hyprctl clients -j` every 500ms. This returns JSON data about all open windows including:
- Window address (unique identifier)
- Process ID
- Window class
- Title
- Workspace info
- Minimized state (special workspace)

### Window Operations

| Action | Hyprland Command |
|--------|------------------|
| Focus window | `hyprctl dispatch focuswindow address:<addr>` |
| Close window | `hyprctl dispatch closewindow address:<addr>` |
| Minimize | `hyprctl dispatch movetoworkspacesilent special:minimized,address:<addr>` |
| Restore | `hyprctl dispatch movetoworkspacesilent e+0,address:<addr>` |
| Logout | `hyprctl dispatch exit` |

### Requirements
- Hyprland compositor must be running
- `hyprctl` must be in PATH

## Supported Applications

The dock automatically recognizes the following window classes:

| Window Class | Display Name | Icon |
|--------------|--------------|------|
| raven-terminal | Terminal | utilities-terminal |
| raven-wifi | WiFi | network-wireless |
| raven-menu | Menu | application-menu |
| raven-settings | Settings | preferences-system |
| raven-files | Files | system-file-manager |
| raven-editor | Editor | text-editor |
| raven-launcher | Launcher | system-search |
| raven-installer | Installer | system-software-install |
| kitty | Terminal | utilities-terminal |
| Alacritty | Terminal | utilities-terminal |
| foot | Terminal | utilities-terminal |
| firefox | Firefox | firefox |
| chromium | Chromium | chromium |
| org.gnome.Nautilus | Files | system-file-manager |
| thunar | Files | system-file-manager |
| code / Code | VS Code | visual-studio-code |

Unknown window classes will use the window title as the display name.

### Excluded Windows
The following window classes are excluded from the dock:
- `raven-shell` (the panel itself)
- `raven-desktop` (desktop background)
- `raven-panel`
- Windows with empty class

## Visual Indicators

- **Running apps**: Teal bottom border (2px solid)
- **Pinned apps**: Slightly brighter background
- **Minimized apps**: Reduced opacity (60%)

## Settings Menu

The settings button in the panel provides quick access to system settings:

### Quick Settings
- **WiFi**: Opens raven-wifi network manager
- **Bluetooth**: Opens blueman-manager, blueberry, or GNOME bluetooth panel
- **Sound**: Opens pavucontrol, pwvucontrol, or GNOME sound settings

### System Settings
- **Display**: Opens wdisplays, nwg-displays, or GNOME display settings
- **Network**: Opens nm-connection-editor or network settings
- **Power & Battery**: Opens power management settings
- **Keyboard**: Opens keyboard/input settings

### Appearance
- **Theme & Appearance**: Opens nwg-look, lxappearance, or GNOME appearance settings
- **Wallpaper**: Opens waypaper, nitrogen, or background settings

### Panel Position
Change the panel orientation:
- **Top**: Panel at top of screen (default)
- **Bottom**: Panel at bottom of screen
- **Left**: Vertical panel on left side
- **Right**: Vertical panel on right side

Changes are applied instantly - the panel rebuilds itself with the new orientation.

### All Settings
Opens the full settings application (raven-settings, GNOME Control Center, or alternatives)

## Power Menu

The power button in the panel provides:

- **Logout**: Exits Hyprland session (`hyprctl dispatch exit`)
- **Lock Screen**: Attempts swaylock, hyprlock, or loginctl lock-session
- **Reboot**: System reboot via systemctl or raven-powerctl
- **Shutdown**: System shutdown via systemctl or raven-powerctl

## CSS Classes

The panel uses the following CSS classes for styling:

### Start Button
- `.start-button`: Raven start button
- `.raven-icon`: Raven SVG icon displayed via CSS background-image

### Dock
- `.dock-container`: The center dock wrapper
- `.dock-item`: Individual dock buttons
- `.dock-item-running`: Running application indicator
- `.dock-item-pinned`: Pinned application indicator
- `.dock-item-minimized`: Minimized application indicator

### Menus
- `.context-menu`: Right-click menu container
- `.context-menu-close`: Close/shutdown button styling
- `.settings-menu`: Settings menu container
- `.settings-section-label`: Section header labels
- `.settings-menu-separator`: Separator lines
- `.quick-toggle`: Quick toggle buttons (WiFi, Bluetooth, Sound)
- `.quick-toggle-active`: Active state for toggles/selected position

### Panel
- `.panel-container`: Main panel container
- `.panel-section`: Panel section grouping
- `.settings-button`: Settings button styling
- `.power-button`: Power button styling
- `.clock`: Clock label styling
