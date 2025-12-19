# Raven Linux Wayland Session Boot Flow

This document explains the complete boot flow when selecting a Wayland desktop from the UEFI boot menu, for developers looking to customize or improve the desktop experience.

## Boot Sequence Overview

```
UEFI Boot Menu
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  1. GRUB/RavenBoot loads kernel with parameters:                │
│     rdinit=/init raven.graphics=wayland raven.wayland=weston    │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  2. RavenInit (/init) starts as PID 1                           │
│     - Mounts /proc, /sys, /dev                                  │
│     - Reads /proc/cmdline                                       │
│     - Detects raven.graphics=wayland                            │
│     - Disables getty-tty1                                       │
│     - Starts seatd + wayland-session services                   │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  3. /bin/raven-wayland-session script executes                  │
│     - Sets environment variables                                │
│     - Starts seatd if needed                                    │
│     - Launches compositor (weston/hyprland/raven-compositor)    │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  4. Compositor initializes                                      │
│     - Opens /dev/dri/card* (DRM/KMS)                            │
│     - Creates /run/user/0/wayland-0 socket                      │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  5. Session components start (terminal auto-launches)           │
│     - weston: weston-terminal / foot / alacritty                │
│     - hyprland: foot / alacritty                                │
│     - raven-compositor: (not implemented yet)                   │
└─────────────────────────────────────────────────────────────────┘
```

---

## Key Files Reference

| File | Purpose | Key Lines |
|------|---------|-----------|
| `build/iso/iso-root/boot/grub/grub.cfg` | GRUB menu entries with kernel cmdline | Menu entries |
| `bootloader/src/config.rs` | UEFI bootloader hardcoded entries | 176-183 |
| `init/src/main.rs` | RavenInit - processes cmdline, starts services | 134-242 |
| `configs/raven-wayland-session` | Session startup script | All (347 lines) |
| `desktop/compositor/src/` | Raven compositor source (Rust/Smithay) | - |

---

## Kernel Parameters

### `raven.graphics=`

| Value | Effect |
|-------|--------|
| `wayland` | Enables Wayland graphics mode, disables TTY getty, starts compositor |
| (not set) | Normal TTY boot with getty login |

### `raven.wayland=`

| Value | Compositor Started |
|-------|-------------------|
| `weston` | Weston compositor with DRM backend |
| `hyprland` or `Hyprland` | Hyprland dynamic tiling compositor |
| `raven` or `raven-compositor` | Custom Raven compositor (Rust/Smithay) |
| (not set) | Auto-detects: prefers raven-compositor → weston → hyprland |

**Example GRUB entry:**
```
menuentry "Raven Desktop (Wayland)" {
    linux /boot/vmlinuz rdinit=/init raven.graphics=wayland raven.wayland=weston
    initrd /boot/initramfs.img
}
```

---

## Init System Flow

**File:** `init/src/main.rs`

### Function: `apply_kernel_cmdline_overrides()` (lines 134-242)

```rust
// 1. Parse kernel cmdline
let cmdline = fs::read_to_string("/proc/cmdline")?;
let graphics = cmdline.find("raven.graphics=").map(|_| "wayland");
let wayland_choice = cmdline.find("raven.wayland=");

// 2. If raven.graphics=wayland:
//    - Disable getty-tty1 (avoid conflict with compositor)
//    - Ensure seatd service is enabled
//    - Create /run/user/0 directory
//    - Start wayland-session service with RAVEN_WAYLAND_COMPOSITOR env var
```

### Services Started for Wayland

1. **seatd** - Seat management daemon (hardware access without root)
2. **wayland-session** - Runs `/bin/raven-wayland-session` script

---

## Session Script Breakdown

**File:** `configs/raven-wayland-session`

### Environment Setup (lines 17-54)

```sh
# Runtime directory for Wayland socket
export XDG_RUNTIME_DIR="/run/user/0"

# Seat management backend
export LIBSEAT_BACKEND="seatd"

# Rust debugging
export RUST_BACKTRACE="1"

# Font configuration
export FONTCONFIG_PATH="/etc/fonts"
export FONTCONFIG_FILE="/etc/fonts/fonts.conf"

# Cursor theme
export XCURSOR_PATH="/usr/share/icons"
export XCURSOR_THEME="breeze_cursors"  # or Adwaita

# Keyboard layouts
export XKB_CONFIG_ROOT="/usr/share/xkeyboard-config-2"
```

### Compositor Selection (lines 208-220)

```sh
preferred="${RAVEN_WAYLAND_COMPOSITOR:-}"
if [ -z "$preferred" ]; then
    if [ -n "${wayland_choice:-}" ]; then
        preferred="$wayland_choice"  # From raven.wayland= cmdline
    elif command -v raven-compositor >/dev/null 2>&1; then
        preferred="raven-compositor"
    elif command -v weston >/dev/null 2>&1; then
        preferred="weston"
    else
        preferred="raven-compositor"
    fi
fi
```

### Compositor Launch Functions

#### Weston (lines 291-316)
```sh
start_weston() {
    # Check for Xwayland support
    have_xwayland=0
    if command -v Xwayland >/dev/null 2>&1; then
        for m in /usr/lib/libweston-*/xwayland.so; do
            [ -f "$m" ] && have_xwayland=1 && break
        done
    fi

    weston_config=""
    [ -f /etc/xdg/weston/weston.ini ] && weston_config="--config=/etc/xdg/weston/weston.ini"

    # Launch weston with DRM backend
    start_compositor "weston" weston ${weston_config} \
        --backend=drm-backend.so \
        --xwayland
}
```

#### Raven Compositor (lines 272-278)
```sh
start_raven_compositor() {
    start_compositor "raven-compositor" raven-compositor && return 0
    # Fallback with legacy mode
    start_compositor "raven-compositor (legacy)" \
        env SMITHAY_USE_LEGACY=1 raven-compositor && return 0
    return 1
}
```

---

## Terminal Auto-Launch

**File:** `configs/raven-wayland-session` (lines 119-154)

### Function: `start_session_components()`

This function runs after the compositor creates its Wayland socket:

```sh
start_session_components() {
    compositor="$1"

    # Start D-Bus session bus if available
    if [ -z "${DBUS_SESSION_BUS_ADDRESS:-}" ] && command -v dbus-daemon >/dev/null 2>&1; then
        eval "$(dbus-daemon --session --fork --print-address | sed 's/^/export DBUS_SESSION_BUS_ADDRESS=/')"
    fi

    case "$compositor" in
        weston*)
            # Try terminals in order of preference
            if command -v weston-terminal >/dev/null 2>&1; then
                weston-terminal &
            elif command -v foot >/dev/null 2>&1; then
                foot &
            elif command -v alacritty >/dev/null 2>&1; then
                alacritty &
            fi
            ;;
        hyprland*|Hyprland*)
            if command -v foot >/dev/null 2>&1; then
                foot &
            elif command -v alacritty >/dev/null 2>&1; then
                alacritty &
            fi
            ;;
        raven-compositor*)
            # Not implemented yet
            log "Note: Raven compositor shell components are not implemented yet."
            ;;
    esac
}
```

---

## Customization Guide

### Adding Auto-Start Applications

Edit `configs/raven-wayland-session`, find `start_session_components()` (line 119), and add your applications:

```sh
case "$compositor" in
    weston*)
        # Existing terminal launch...

        # ADD YOUR APPLICATIONS HERE:
        # Example: Start a panel
        if command -v waybar >/dev/null 2>&1; then
            waybar &
        fi

        # Example: Set wallpaper
        if command -v swaybg >/dev/null 2>&1; then
            swaybg -i /usr/share/backgrounds/default.png &
        fi
        ;;
esac
```

### Weston Configuration

Create/edit `/etc/xdg/weston/weston.ini`:

```ini
[core]
# Number of workspaces
numworkspaces=4
# Idle timeout (ms)
idle-time=300

[shell]
# Panel position: top, bottom, left, right, none
panel-position=top
# Background color (ARGB)
background-color=0xff002244
# Background image
background-image=/usr/share/backgrounds/raven.png
# Background type: scale, scale-crop, tile, centered
background-type=scale-crop
# Clock format
clock-format=minutes

[launcher]
# Add launcher icons to panel
icon=/usr/share/icons/hicolor/24x24/apps/terminal.png
path=/usr/bin/weston-terminal

[launcher]
icon=/usr/share/icons/hicolor/24x24/apps/firefox.png
path=/usr/bin/firefox

[keyboard]
keymap_layout=us

[input-method]
path=/usr/libexec/weston-keyboard

[terminal]
font=monospace
font-size=14
```

### Changing Default Compositor

**Option 1: Kernel cmdline (GRUB)**

Edit `build/iso/iso-root/boot/grub/grub.cfg`:
```
linux /boot/vmlinuz rdinit=/init raven.graphics=wayland raven.wayland=hyprland
```

**Option 2: Environment variable**

Set before session starts in `configs/raven-wayland-session`:
```sh
export RAVEN_WAYLAND_COMPOSITOR="hyprland"
```

### Adding a New Compositor

1. Add launch function in `configs/raven-wayland-session`:
```sh
start_mycompositor() {
    tried_mycomp=1
    start_compositor "mycompositor" mycompositor --some-flags && return 0
    return 1
}
```

2. Add to the selection case (around line 318):
```sh
case "$preferred" in
    mycompositor)
        start_mycompositor && exit 0
        ;;
    # ... existing cases
esac
```

3. Add terminal auto-launch in `start_session_components()`:
```sh
case "$compositor" in
    mycompositor*)
        foot &  # or your preferred terminal
        ;;
esac
```

---

## Environment Variables Reference

| Variable | Default | Purpose |
|----------|---------|---------|
| `XDG_RUNTIME_DIR` | `/run/user/0` | Wayland socket location |
| `WAYLAND_DISPLAY` | `wayland-0` | Socket name (set after compositor starts) |
| `LIBSEAT_BACKEND` | `seatd` | Seat management (seatd or logind) |
| `RAVEN_WAYLAND_COMPOSITOR` | (auto) | Force specific compositor |
| `RAVEN_FORCE_MODE` | (unset) | Force display resolution in VMs (e.g., `1024x768`) |
| `XCURSOR_THEME` | `breeze_cursors` | Cursor theme |
| `XKB_CONFIG_ROOT` | `/usr/share/xkeyboard-config-2` | Keyboard layout definitions |
| `FONTCONFIG_PATH` | `/etc/fonts` | Font configuration |
| `RUST_BACKTRACE` | `1` | Enable Rust backtraces for debugging |

---

## Debugging

### Log Files

- Session log: `/run/raven-wayland-session.log`
- Serial console: `/dev/ttyS0` (if available)

### Common Issues

**No display output:**
```sh
# Check DRM devices
ls -la /dev/dri/

# Check connector status
cat /sys/class/drm/*/status

# For VMs without connected display, set resolution:
export RAVEN_FORCE_MODE="1920x1080"
```

**Compositor crashes immediately:**
```sh
# Check the log
cat /run/raven-wayland-session.log

# Try software rendering (weston)
weston --backend=drm-backend.so --renderer=pixman
```

**No seat access:**
```sh
# Check seatd is running
pgrep seatd

# Check socket exists
ls -la /run/seatd.sock
```
