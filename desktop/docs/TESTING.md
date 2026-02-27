# Raven Desktop Environment - Testing Guide

## Quick Test with Nested Compositor (Recommended)

The fastest way to test the compositor without a full build:

```bash
cd RavenLinux/desktop
./test-nested.sh    # Launch raven-compositor nested in a window
```

This launches `raven-compositor` with the winit backend in a window.
Use `Super+Alt+Q` to exit the nested session.

### Requirements for Testing

- Running Wayland compositor (for nested winit backend)
- Rust/Cargo (to build compositor)
- `swaybg` for wallpaper

---

## Building

All external repos are fetched from GitHub at build time. No embedded sources.

### Build Compositor

```bash
./scripts/build-packages.sh compositor
```

This fetches `RavenCompositor` from GitHub and builds:
- `raven-compositor` (Wayland compositor)
- `raven-shell` (panel, Rust)
- `raven-settings` (settings app, Rust)

### Build All Packages

```bash
./scripts/build-packages.sh all
```

### Local Development Build

```bash
./scripts/build-desktop-local.sh
```

Fetches and builds compositor + terminal, installs config.

## Architecture Overview

```
raven-compositor (Wayland Compositor - Smithay)
  Software Renderer (XRGB8888 framebuffer)
  Layer Stack (rendered back-to-front):
  1. Background layer
  2. Bottom layer       (reserved)
  3. Toplevels          raven-terminal, apps
  4. Top layer          raven-shell (panel)
  5. Overlay layer

  Input Handling:
  - Keyboard shortcuts (Super+Enter, Super+Space, etc.)
  - Mouse/pointer events (click to focus, panel clicks)
  - libinput integration for all input devices
```

## Testing in QEMU

### QEMU Command

```bash
qemu-system-x86_64 \
  -enable-kvm \
  -m 4G \
  -cpu host \
  -smp 4 \
  -device virtio-gpu-pci \
  -display gtk,gl=on,grab-on-hover=on \
  -device usb-ehci \
  -device usb-tablet \
  -device usb-kbd \
  -serial mon:stdio
```

**Important flags:**
- `virtio-gpu-pci`: Provides DRM/KMS device (required!)
- `gtk,gl=on`: GTK display with OpenGL
- `usb-tablet`: Absolute mouse positioning
- Don't use `-nographic` or `-display none`

### Using the Test Script

```bash
./scripts/test-desktop.sh              # QEMU mode
./scripts/test-desktop.sh --nested     # Nested in current session
./scripts/test-desktop.sh --rebuild    # Rebuild then test
```

## What to Expect

### On Successful Start:

1. Panel at top with clock (raven-shell)
2. Keyboard shortcuts:
   - `Super + Enter` - Launch terminal
   - `Super + Space` - Open menu
   - `Super + Q` - Close focused window

## Troubleshooting

### No Display / Black Screen
- Check `/dev/dri/` exists
- Ensure QEMU has `virtio-gpu-pci`
- Check serial console for errors

### Input Not Working
- Start seatd: `sudo seatd -g video`
- Add user to video group: `sudo usermod -a -G video $USER`

### Compositor Crashes
Check serial console for backtrace. Common issues:
1. Permission denied on /dev/dri - add to video group
2. Failed to create session - seatd not running
3. No display modes - QEMU missing GPU device

## Performance Tips

With software rendering, expect 30-60 FPS for basic desktop.

To improve in QEMU:
1. Use `-m 4G -smp 4`
2. Use `-device usb-tablet`
3. Try `-display sdl,gl=on` on X11 hosts

## Keyboard Shortcuts Reference

| Shortcut | Action |
|----------|--------|
| `Super + Enter` | Launch terminal |
| `Super + Space` | Open menu |
| `Super + Q` | Close focused window |

## Logs and Debugging

Enable verbose logging:
```bash
RUST_LOG=debug raven-compositor
```

## Desktop Apps Status

The legacy Go/GTK4 desktop apps (raven-shell, raven-menu, raven-desktop,
raven-settings-menu, raven-power, raven-keybindings, raven-file-manager) have
been removed. New GUI features are being rebuilt natively on top of
RavenCompositor as Rust binaries.
