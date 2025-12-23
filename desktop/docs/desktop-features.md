# Raven Desktop Features

This document describes the features available in the raven-desktop application.

## Overview

Raven Desktop provides a clean, customizable desktop experience with support for pinned applications, wallpaper management, and a system-wide fuzzy finder.

## Features

### Clean Desktop

By default, the desktop starts empty with no pre-configured icons. Users can add applications to their desktop through the pinning system.

### App Pinning System

Pin your favorite applications to the desktop for quick access.

#### Pinning Apps

1. **Via Right-Click Menu**: Right-click on the desktop and select "Pin Application..."
2. This opens the Fuzzy Finder in pin mode
3. Search for the application you want to pin
4. Press Enter or click to pin it to the desktop

#### Unpinning Apps

1. Right-click on any pinned icon on the desktop
2. Select "Unpin from Desktop"
3. The icon will be removed from the desktop

#### Storage

Pinned apps are stored in `~/.config/raven/pinned-apps.json`. This file is automatically created and managed by the application.

### Wallpaper Management

Change your desktop background easily.

#### Setting a Wallpaper

1. Right-click on the desktop
2. Select "Change Wallpaper..."
3. A file browser dialog opens
4. Navigate to your image file (PNG, JPEG, WebP supported)
5. Click "Select" to apply the wallpaper

The wallpaper setting is saved to `~/.config/raven/settings.json` and persists across sessions.

### Fuzzy Finder

A powerful search tool for quickly finding and launching applications, files, and commands.

#### Opening the Fuzzy Finder

**Via Right-Click Menu:**
- Right-click on the desktop and select "Open Fuzzy Finder"

**Via Keyboard Shortcut:**
Add this to your Hyprland config (`~/.config/hypr/hyprland.conf`):

```
bind = SUPER, SPACE, exec, pkill -USR1 raven-desktop
```

This binds `Super + Space` to open the fuzzy finder. You can change `SUPER, SPACE` to your preferred key combination.

#### Using the Fuzzy Finder

- **Search**: Start typing to search
- **Navigate**: Use Up/Down arrow keys to navigate results
- **Select**: Press Enter to launch the selected item
- **Close**: Press Escape to close without selecting

#### Search Categories

The fuzzy finder searches across three categories:

1. **Applications (APP)**: Desktop applications from .desktop files
   - Searches `/usr/share/applications`
   - Searches `/usr/local/share/applications`
   - Searches `~/.local/share/applications`

2. **Files (FILE)**: Files in common directories
   - Home directory
   - Documents, Downloads, Pictures, Videos, Music, Desktop

3. **Commands (CMD)**: Executable commands from PATH
   - All executable files in directories listed in $PATH

Results are scored and sorted by relevance, with applications prioritized.

### Right-Click Context Menu

Right-clicking on the desktop shows a context menu with:

| Option | Description |
|--------|-------------|
| Open Terminal | Launches raven-terminal |
| Open File Manager | Opens ranger in terminal |
| Open Fuzzy Finder | Opens the fuzzy finder |
| Pin Application... | Opens fuzzy finder in pin mode |
| Change Wallpaper... | Opens file chooser for wallpaper |
| Raven Settings | Opens raven-settings-menu |
| Refresh Desktop | Reloads desktop icons |

### Icon Right-Click Menu

Right-clicking on a desktop icon shows:

| Option | Description |
|--------|-------------|
| Unpin from Desktop | Removes the icon from desktop |

### Desktop File Support

The desktop also loads any `.desktop` files placed in `~/Desktop/`. These are displayed alongside pinned applications.

## Configuration Files

| File | Purpose |
|------|---------|
| `~/.config/raven/settings.json` | General settings including wallpaper path |
| `~/.config/raven/pinned-apps.json` | List of pinned desktop applications |

### Example pinned-apps.json

```json
{
  "pinned_apps": [
    {
      "name": "Firefox",
      "exec": "firefox",
      "icon": "firefox",
      "x": 0,
      "y": 0
    },
    {
      "name": "Terminal",
      "exec": "raven-terminal",
      "icon": "utilities-terminal",
      "x": 0,
      "y": 0
    }
  ]
}
```

## Keyboard Shortcuts

### Within Fuzzy Finder

| Key | Action |
|-----|--------|
| Escape | Close fuzzy finder |
| Enter | Activate selected item |
| Up Arrow | Select previous item |
| Down Arrow | Select next item |

### System-wide (Hyprland)

Add these bindings to your Hyprland config:

```
# Open fuzzy finder
bind = SUPER, SPACE, exec, pkill -USR1 raven-desktop

# Open fuzzy finder in pin mode
bind = SUPER SHIFT, SPACE, exec, pkill -USR2 raven-desktop
```

## Signal Support

The desktop application responds to UNIX signals:

| Signal | Action |
|--------|--------|
| SIGUSR1 | Open fuzzy finder |
| SIGUSR2 | Open fuzzy finder in pin mode |

Example usage from command line:

```bash
# Open fuzzy finder
pkill -USR1 raven-desktop

# Open fuzzy finder in pin mode
pkill -USR2 raven-desktop
```
