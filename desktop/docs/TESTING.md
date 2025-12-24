# Raven Desktop Environment - Testing Guide

## Quick Test with Nested Hyprland (Recommended)

The fastest way to test all components together without a full build is using the nested Hyprland test script:

```bash
cd /home/javanhut/Development/CustomLinux/RavenLinux/desktop
./test-nested.sh
```

This script:
- Creates a temporary Hyprland configuration
- Launches Hyprland in a nested window inside your current session
- Auto-starts `raven-desktop` and `raven-shell` using `go run`
- Sets up keybindings to launch other components on demand

### Nested Test Keybindings

| Shortcut | Action |
|----------|--------|
| `SUPER + Return` | Open terminal (foot) |
| `SUPER + Space` | App launcher (raven-menu) |
| `SUPER + E` | File manager |
| `SUPER + S` | Settings |
| `SUPER + Escape` | Power menu |
| `SUPER + Q` | Close window |
| `SUPER + M` | Exit nested session |

### Requirements for Nested Testing

- Running Hyprland or another Wayland compositor
- `go` 1.23+
- `swaybg` for wallpaper
- GTK4 and gtk4-layer-shell development files

---

## Full Build Testing

### Prerequisites

Before building, you need to fix the permission issue with the compositor target directories:

```bash
sudo chown -R $USER:$USER /home/javanstorm/Development/CustomLinux/RavenLinux/desktop/compositor/target*
```

## Building All Components

Run the build script from the project root:

```bash
./scripts/build-desktop-local.sh
```

This will build:
1. `raven-compositor` (Rust) - The Wayland compositor
2. `raven-shell` (Go/GTK4) - Top panel/taskbar
3. `raven-desktop` (Go/GTK4) - Desktop background with icons
4. `raven-menu` (Go/GTK4) - Start menu/application launcher
5. `raven-terminal` (Go/GLFW) - Terminal emulator

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     raven-compositor                         │
│              (Wayland Compositor - Smithay)                  │
│                                                              │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  Software Renderer (XRGB8888 framebuffer)            │  │
│  │  - Composites all surfaces layer by layer             │  │
│  │  - Renders to DRM dumb buffer                         │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                              │
│  Layer Stack (rendered back-to-front):                      │
│  1. Background layer   → raven-desktop (icons, wallpaper)   │
│  2. Bottom layer       → (reserved)                          │
│  3. Toplevels          → raven-terminal, apps               │
│  4. Top layer          → raven-shell (panel)                │
│  5. Overlay layer      → raven-menu (start menu)            │
│                                                              │
│  Input Handling:                                             │
│  - Keyboard shortcuts (Super+Enter, Super+Space, etc.)      │
│  - Mouse/pointer events (click to focus, panel clicks)      │
│  - libinput integration for all input devices              │
└─────────────────────────────────────────────────────────────┘
```

## Testing in QEMU

### QEMU Command

```bash
qemu-system-x86_64 \
  -enable-kvm \
  -m 4G \
  -cpu host \
  -smp 4 \
  -drive file=/path/to/ravenlinux.img,format=raw,if=virtio \
  -device virtio-gpu-pci \
  -display gtk,gl=on,grab-on-hover=on \
  -device usb-ehci \
  -device usb-tablet \
  -device usb-kbd \
  -serial mon:stdio
```

**Important flags:**
- `virtio-gpu-pci`: Provides DRM/KMS device (required!)
- `gtk,gl=on`: GTK display with OpenGL (shows compositor output)
- `usb-tablet`: Absolute mouse positioning (avoids grabbing issues)
- `grab-on-hover=on`: Better mouse capture behavior
- Don't use `-nographic` or `-display none` - you need visual output!

### Inside QEMU

1. **Fix permissions** (if needed):
   ```bash
   sudo chown -R javanstorm:javanstorm ~/Development/CustomLinux/RavenLinux/desktop/compositor/target*
   ```

2. **Build everything**:
   ```bash
   cd ~/Development/CustomLinux/RavenLinux
   ./scripts/build-desktop-local.sh
   ```

3. **Check dependencies**:
   ```bash
   ./scripts/check-desktop-deps.sh
   ```

4. **Start compositor** (manual test):
   ```bash
   # Add binaries to PATH
   export PATH="$PWD/desktop/compositor/target-user/release:$PATH"
   export PATH="$PWD/desktop/raven-shell:$PATH"
   export PATH="$PWD/desktop/raven-desktop:$PATH"
   export PATH="$PWD/desktop/raven-menu:$PATH"
   export PATH="$PWD/tools/raven-terminal:$PATH"
   
   # Start compositor (will auto-launch shell components)
   raven-compositor
   ```

5. **Or use the session script**:
   ```bash
   # If raven-wayland-session is set up:
   /etc/raven/raven-wayland-session
   ```

## What to Expect

### On Successful Start:

1. **Display should show:**
   - Dark background (from raven-desktop)
   - Panel at top with "Raven" button, clock, etc. (raven-shell)

2. **Keyboard shortcuts:**
   - `Super + Enter` → Launch raven-terminal
   - `Super + Space` → Launch raven-menu (start menu)
   - `Super + Q` → Close focused window

3. **Mouse:**
   - Click panel buttons to launch apps
   - Click windows to focus them
   - Click desktop for right-click menu

4. **Serial console output** (in QEMU terminal):
   ```
   === RAVEN-COMPOSITOR STARTING ===
   PID: 1234
   ...
   === ENTERING MAIN EVENT LOOP ===
   VBlank #0: 0 toplevels, 0 layers
   New client connected
   Adding layer surface: namespace=raven-desktop, layer=Background
   VBlank #1: 0 toplevels, 1 layers
   ...
   ```

## Troubleshooting

### No Display / Black Screen

**Check:**
1. DRM/KMS device exists: `ls -la /dev/dri/`
2. Compositor started: `ps aux | grep raven-compositor`
3. Wayland socket created: `ls -la $XDG_RUNTIME_DIR/wayland-*`
4. Serial console for errors

**Fix:**
- Ensure QEMU has `virtio-vga-gl` device
- Check `/run/raven-wayland-session.log` for errors
- Run `dmesg | grep drm` to check kernel DRM initialization

### Components Don't Launch

**Check:**
1. Binaries in PATH: `which raven-shell raven-desktop raven-menu`
2. GTK4 layer-shell installed: `pkg-config --exists gtk4-layer-shell-0`
3. Compositor logs for connection errors

**Fix:**
```bash
# Manually launch components to see errors:
WAYLAND_DISPLAY=wayland-0 ./desktop/raven-desktop/raven-desktop
WAYLAND_DISPLAY=wayland-0 ./desktop/raven-shell/raven-shell
```

### Terminal Won't Launch

**Check:**
1. GLFW Wayland support: See if raven-terminal binary uses Wayland
2. X11 fallback: If GLFW doesn't have Wayland, terminal may need XWayland

**Fix:**
- Try running terminal manually: `WAYLAND_DISPLAY=wayland-0 ./raven-terminal`
- If it needs X11, consider adding XWayland support to compositor (future work)

### Input Not Working

**Check:**
1. seatd running: `ps aux | grep seatd`
2. User in video group: `groups | grep video`
3. libinput devices detected: Check compositor logs

**Fix:**
```bash
# Start seatd if not running:
sudo seatd -g video &

# Add user to video group:
sudo usermod -a -G video javanstorm
# (logout and login for group to take effect)
```

### Compositor Crashes

**Check serial console output for backtrace**

Common issues:
1. **Permission denied on /dev/dri**: Add to video group
2. **Failed to create session**: seatd not running
3. **No display modes**: QEMU missing proper GPU device
4. **Buffer mapping failed**: DRM driver issue

## Performance Tips

With software rendering, expect:
- **30-60 FPS** for basic desktop
- **Higher CPU usage** (no GPU acceleration)
- **Smooth for static content**, may lag with animations

To improve:
1. Use smaller resolution in QEMU (800x600 instead of 1920x1080)
2. Reduce window count
3. Future: Add GPU rendering (GBM/EGL)

### VM Performance Troubleshooting

If you experience severe slowness, unresponsive mouse, or laggy input in QEMU:

**Symptoms:**
- Everything running slow
- Cannot close menus
- Terminal/vim input delayed
- Mouse doesn't respond properly
- Log shows: `NEEDS EXTENSION: falling back to kms_swrast`

**Cause:** The VM is using software rendering (llvmpipe) instead of GPU acceleration. This happens when:
- Running QEMU on a Wayland host (virgl passthrough may not work)
- Host lacks proper OpenGL/virgl support
- Missing graphics drivers

**Solutions:**

1. **Use the optimized test script:**
   ```bash
   ./scripts/test-desktop.sh
   ```
   This script includes performance-optimized settings.

2. **Increase VM resources:**
   ```bash
   -m 4G    # At least 4GB RAM
   -smp 4   # At least 4 CPU cores
   ```

3. **Use usb-tablet for mouse (already in test-desktop.sh):**
   ```bash
   -device usb-ehci
   -device usb-tablet
   ```
   This provides absolute mouse positioning, avoiding grab issues.

4. **Try different display backend (on X11 host):**
   ```bash
   -display sdl,gl=on
   # or
   -display spice-app,gl=on
   ```

5. **Disable GL if nothing works:**
   ```bash
   -device virtio-vga
   -display gtk
   ```
   This will use pure software rendering but may be more stable.

**Note:** The Hyprland configuration has been optimized for software rendering with blur, shadows, and animations disabled. This significantly improves performance in VM environments.

## Keyboard Shortcuts Reference

| Shortcut | Action |
|----------|--------|
| `Super + Enter` | Launch terminal |
| `Super + Space` | Open start menu |
| `Super + Q` | Close focused window |
| `Alt + Tab` | Window switcher (TODO) |
| `Super + 1-9` | Switch workspaces (TODO) |

## Development Workflow

1. **Make changes** to compositor or shell components
2. **Rebuild**: `./scripts/build-desktop-local.sh`
3. **Test**: Restart compositor in QEMU
4. **Iterate**: Check logs, fix bugs, repeat

## Next Steps

After basic testing works:

1. **Add more features:**
   - Window decorations (title bars, close buttons)
   - Drag-and-drop window moving
   - Workspace management
   - Animations

2. **Build ISO:**
   - Update `scripts/build.sh` to include desktop components
   - Test on real hardware

3. **Polish:**
   - Themes and styling
   - More panel widgets
   - Settings app

## Logs and Debugging

- Compositor logs: `$XDG_RUNTIME_DIR/` or serial console
- Session logs: `/run/raven-wayland-session.log`
- Component logs: Launched components log to same location

Enable verbose logging:
```bash
RUST_LOG=debug raven-compositor
```

## Success Criteria

You'll know it's working when:
- ✅ Dark desktop background visible
- ✅ Panel at top with clock
- ✅ Can click "Raven" button to open menu
- ✅ Can launch terminal with Super+Enter
- ✅ Can type in terminal
- ✅ Mouse cursor moves smoothly
- ✅ Windows render and can be focused

## Getting Help

If something doesn't work:
1. Check this guide's troubleshooting section
2. Review serial console output
3. Test components individually
4. Check build logs for compilation errors
