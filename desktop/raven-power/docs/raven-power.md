# Raven Power Menu

A fullscreen power menu overlay for the Raven desktop environment, providing quick access to power management actions.

## Features

- Fullscreen overlay with semi-transparent background
- Six power options: Lock, Logout, Suspend, Hibernate, Reboot, Shutdown
- Visual icons for each option
- Color-coded buttons (red for shutdown, orange for reboot)
- Layer-shell integration for proper overlay behavior
- Closes on Escape key press

## Usage

### Launch Methods

1. **Keybinding**: Press `Super + P` to show the power menu
2. **Command Line**: Run `raven-power`
3. **Shell Power Button**: Click "Power" in the raven-shell panel

### Power Options

| Option | Description | Command |
|--------|-------------|---------|
| Lock | Lock the screen | `hyprlock` / `swaylock` / `loginctl lock-session` |
| Logout | End the Hyprland session | `hyprctl dispatch exit` |
| Suspend | Sleep the computer | `systemctl suspend` |
| Hibernate | Hibernate to disk | `systemctl hibernate` |
| Reboot | Restart the computer | `systemctl reboot` |
| Shutdown | Power off the computer | `systemctl poweroff` |

### Closing Without Action

- Press `Escape` key

## Building

```bash
cd raven-power
go build -o raven-power
```

## Dependencies

- GTK4
- gtk4-layer-shell
- Hyprland (compositor)
- systemd (for power management)
- hyprlock/swaylock (for screen locking)

## Integration

The power menu integrates with:

- **raven-shell**: The panel's Power button can launch external power menu or use its built-in popup
- **Hyprland**: Uses `hyprctl dispatch exit` for logout
- **systemd**: Uses systemctl for suspend, hibernate, reboot, shutdown
- **Screen lockers**: Supports hyprlock, swaylock, or loginctl

## Styling

The power menu uses a dark theme with:

- Semi-transparent background (92% opacity)
- Rounded button cards
- Color-coded destructive actions (red for shutdown, orange for reboot)
- Teal accent hover states for non-destructive actions
