# Raven Desktop Features

## Status

The legacy Go/GTK4 desktop applications have been removed. Desktop features are
being rebuilt natively on top of RavenCompositor as Rust binaries.

### Current Components (from RavenCompositor)

- **raven-compositor** - Wayland compositor built with Smithay
- **raven-shell** - Panel/taskbar (Rust, built as part of RavenCompositor)
- **raven-settings** - Settings application (Rust, built as part of RavenCompositor)

### Planned Features

The following features from the legacy desktop apps will be reimplemented:

- App pinning system with fuzzy finder
- Wallpaper management
- Desktop icons
- Right-click context menus
- Power management
- Keybinding configuration
- File manager (see RavenFileManager repo)

## Configuration Files

| File | Purpose |
|------|---------|
| `~/.config/raven/settings.json` | General settings including wallpaper path |
| `~/.config/raven/pinned-apps.json` | List of pinned desktop applications |

## Keyboard Shortcuts (raven-compositor)

| Shortcut | Action |
|----------|--------|
| `Super + Enter` | Launch terminal |
| `Super + Space` | Open menu |
| `Super + Q` | Close focused window |
| `Super + Alt + Q` | Exit compositor (nested mode) |

## Signal Support

The compositor responds to signals for launching components:

```bash
# Open fuzzy finder (when reimplemented)
pkill -USR1 raven-compositor

# Open fuzzy finder in pin mode (when reimplemented)
pkill -USR2 raven-compositor
```
