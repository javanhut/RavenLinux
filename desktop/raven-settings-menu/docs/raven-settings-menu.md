# Raven Settings Menu

A graphical settings application for configuring the Raven desktop environment.

## Overview

`raven-settings-menu` provides a user-friendly interface for customizing various aspects of the Raven desktop environment. It uses GTK4 with gtk4-layer-shell for Wayland integration and saves settings to a JSON configuration file.

## Features

### Appearance Settings
- **Theme**: Choose between Dark, Light, or System theme
- **Accent Color**: Select from predefined accent colors for UI highlights
- **Font Size**: Adjust the base font size (10-24px)
- **Panel Opacity**: Control transparency level of panels (0-100%)
- **Animations**: Enable or disable UI animations

### Desktop Settings
- **Wallpaper**: Browse and select a wallpaper image
- **Wallpaper Mode**: Choose how the wallpaper is displayed (Fill, Fit, Stretch, Center, Tile)
- **Desktop Icons**: Toggle desktop icon visibility

### Panel Settings
- **Panel Position**: Set panel to top or bottom of screen
- **Panel Height**: Adjust panel height in pixels (24-64px)
- **Show Clock**: Toggle clock widget visibility
- **Clock Format**: Choose between 24-hour or 12-hour format
- **Show Workspaces**: Toggle workspace indicator visibility

### Window Settings
- **Border Width**: Set window border thickness (0-10px)
- **Gap Size**: Configure space between tiled windows (0-32px)
- **Focus Follows Mouse**: Enable focus-follows-cursor behavior
- **Titlebar Buttons**: Configure which buttons appear on window titlebars

### Input Settings
- **Keyboard Layout**: Select keyboard language layout (US, UK, DE, FR, ES, IT, RU, JP)
- **Mouse Speed**: Adjust pointer acceleration (0-100%)
- **Natural Scrolling**: Enable/disable natural scroll direction for touchpad
- **Tap to Click**: Enable/disable tap-to-click on touchpad

### Power Settings
- **Screen Timeout**: Set display power-off timer (Never to 30 minutes)
- **Suspend Timeout**: Configure auto-suspend timer (Never to 2 hours)
- **Lid Close Action**: Choose action when laptop lid is closed (Suspend, Hibernate, Power Off, Do Nothing)

### Sound Settings
- **Master Volume**: Adjust system volume (0-100%)
- **Mute on Lock**: Automatically mute audio when screen is locked
- **Test Audio**: Play a test sound to verify audio output

### About
- Displays Raven Linux version information
- Shows system information (hostname, kernel version)
- Links to website and documentation

## Usage

Launch the settings menu:
```bash
raven-settings-menu
```

### Keyboard Shortcuts
- **Escape**: Close the settings window

## Configuration File

Settings are stored in JSON format at:
```
~/.config/raven/settings.json
```

### Example Configuration
```json
{
  "theme": "dark",
  "accent_color": "#009688",
  "font_size": 14,
  "icon_theme": "Papirus-Dark",
  "cursor_theme": "Adwaita",
  "panel_opacity": 0.95,
  "enable_animations": true,
  "wallpaper_path": "/home/user/Pictures/wallpaper.jpg",
  "wallpaper_mode": "fill",
  "show_desktop_icons": false,
  "panel_position": "top",
  "panel_height": 36,
  "show_clock": true,
  "clock_format": "24h",
  "show_workspaces": true,
  "border_width": 2,
  "gap_size": 8,
  "focus_follows_mouse": false,
  "titlebar_buttons": "close,minimize,maximize",
  "keyboard_layout": "us",
  "mouse_speed": 0.5,
  "touchpad_natural_scroll": true,
  "touchpad_tap_to_click": true,
  "screen_timeout": 300,
  "suspend_timeout": 900,
  "lid_close_action": "suspend",
  "master_volume": 80,
  "mute_on_lock": false
}
```

## Dependencies

- GTK4
- gtk4-layer-shell
- Go 1.21+
- gotk4 (Go GTK4 bindings)

### Runtime Dependencies (for full functionality)
- `swaybg` - For wallpaper changes
- `wpctl` - For volume control (WirePlumber)
- `paplay` - For audio testing (PulseAudio utilities)
- `xdg-open` - For opening external links

## Building

```bash
cd raven-settings-menu
go build -o raven-settings-menu
```

## Integration with Other Raven Components

The settings in `~/.config/raven/settings.json` can be read by other Raven components:
- `raven-shell` - Panel configuration
- `raven-desktop` - Desktop background and icons
- Window managers/compositors - Window behavior settings

Other components should watch this file for changes and apply settings dynamically where possible.

## Architecture

The application follows the standard Raven application structure:

1. **GTK4 Application**: Uses `gtk.Application` for lifecycle management
2. **Layer Shell**: Positions window as an overlay on Wayland
3. **Sidebar Navigation**: Category-based navigation with smooth transitions
4. **Immediate Apply**: Settings are saved and applied immediately when changed
5. **Escape to Close**: Standard keyboard shortcut for dismissal

## Styling

The application uses a dark theme consistent with other Raven applications:
- Background: `#0f1720`
- Secondary: `#1a2332`
- Accent: `#009688` (teal)
- Text: `#e0e0e0`
- Muted: `#888888`

CSS styling is embedded in the application and applied at runtime.
