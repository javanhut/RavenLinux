# Raven Keybindings

A fullscreen overlay that displays all configured keyboard shortcuts for the Raven desktop environment.

## Features

- Fullscreen overlay with semi-transparent background
- Organized by category (Applications, Windows, Focus, Movement, Workspaces, Media, Screenshots, System)
- Two-column layout for easy reading
- Closes on any key press or mouse click
- Layer-shell integration for proper overlay behavior

## Usage

### Launch Methods

1. **Keybinding**: Press `Super + K` to show the keybindings overlay
2. **Command Line**: Run `raven-keybindings`

### Closing

- Press any key
- Click anywhere on the screen
- Press Escape

## Keybinding Categories

### Applications
- `Super + T` - Open Terminal
- `Super + M` - Open Menu
- `Super + S` - Open Settings
- `Super + F` - Fuzzy Finder / Launcher
- `Super + P` - Power Menu
- `Super + K` - Show Keybindings (this overlay)
- `Super + W` - WiFi Settings
- `Super + Shift + E` - File Manager

### Windows
- `Super + Q` - Close Window
- `Super + V` - Toggle Floating
- `Super + Shift + F` - Fullscreen
- `Super + J` - Toggle Split
- `Super + R` - Enter Resize Mode

### Focus
- `Super + Arrow Keys` - Move Focus
- `Super + H/J/K/L` - Move Focus (Vim-style)
- `Alt + Tab` - Cycle Windows

### Movement
- `Super + Shift + Arrows` - Move Window
- `Super + Shift + H/J/K/L` - Move Window (Vim-style)
- `Super + Mouse Drag` - Move Window
- `Super + Right Click Drag` - Resize Window

### Workspaces
- `Super + 1-0` - Switch to Workspace 1-10
- `Super + Shift + 1-0` - Move Window to Workspace
- `Super + Tab` - Next Workspace
- `Super + Shift + Tab` - Previous Workspace
- `Super + Scroll` - Cycle Workspaces

### Media
- Volume Keys - Adjust Volume
- Brightness Keys - Adjust Brightness
- Play/Pause - Media Play/Pause
- Next/Prev - Media Next/Previous

### Screenshots
- `Print` - Screenshot Region to Clipboard
- `Shift + Print` - Screenshot Full to Clipboard
- `Super + Print` - Screenshot Region to File
- `Super + Shift + Print` - Screenshot Full to File

### System
- `Super + Escape` - Lock Screen
- `Super + Shift + Q` - Exit Hyprland

## Building

```bash
cd raven-keybindings
go build -o raven-keybindings
```

## Dependencies

- GTK4
- gtk4-layer-shell
- Hyprland (compositor)

## Configuration

The keybindings displayed are hardcoded to match the Hyprland configuration in `hyprland-config.sh`. If you modify the Hyprland keybindings, update the `initBindings()` function in `main.go` accordingly.
