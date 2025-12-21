# Raven Linux Wayland Session Boot Flow

This document explains the complete boot flow when selecting a Wayland desktop from the UEFI boot menu, for developers looking to customize or improve the desktop experience.

## Boot Sequence Overview

```
UEFI Boot Menu
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  1. GRUB/RavenBoot loads kernel with parameters:                │
│     rdinit=/init raven.graphics=wayland raven.wayland=raven     │
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
│     - Launches compositor (raven-compositor or hyprland)        │
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
│     - raven-compositor: raven-desktop, raven-shell, raven-terminal │
│     - hyprland: raven-terminal / foot                           │
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
| `raven` or `raven-compositor` | Raven compositor (Rust/Smithay) - **default** |
| `hyprland` or `Hyprland` | Hyprland dynamic tiling compositor |
| (not set) | Auto-detects: prefers raven-compositor → hyprland |

**Example GRUB entry:**
```
menuentry "Raven Desktop (Wayland)" {
    linux /boot/vmlinuz rdinit=/init raven.graphics=wayland raven.wayland=raven
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

### Compositor Selection

```sh
preferred="${RAVEN_WAYLAND_COMPOSITOR:-}"
if [ -z "$preferred" ]; then
    if [ -n "${wayland_choice:-}" ]; then
        preferred="$wayland_choice"  # From raven.wayland= cmdline
    else
        # Always default to raven-compositor
        preferred="raven-compositor"
    fi
fi
```

### Compositor Launch Functions

#### Raven Compositor (default)
```sh
start_raven_compositor() {
    start_compositor "raven-compositor" raven-compositor && return 0
    # Fallback with legacy mode
    start_compositor "raven-compositor (legacy)" \
        env SMITHAY_USE_LEGACY=1 raven-compositor && return 0
    return 1
}
```

#### Hyprland
```sh
start_hyprland() {
    if command -v Hyprland >/dev/null 2>&1; then
        start_compositor "Hyprland" Hyprland && return 0
    fi
    if command -v hyprland >/dev/null 2>&1; then
        start_compositor "hyprland" hyprland && return 0
    fi
    return 1
}
```

---

## Session Components Auto-Launch

**File:** `configs/raven-wayland-session`

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
        raven-compositor*|raven*)
            # Start Raven shell components
            # Start desktop first (background layer)
            if command -v raven-desktop >/dev/null 2>&1; then
                raven-desktop &
            fi
            # Then start the panel (top layer)
            if command -v raven-shell >/dev/null 2>&1; then
                sleep 0.2
                raven-shell &
            fi
            if command -v raven-terminal >/dev/null 2>&1; then
                sleep 0.5
                raven-terminal &
            fi
            ;;
        hyprland*|Hyprland*)
            if command -v raven-terminal >/dev/null 2>&1; then
                raven-terminal &
            elif command -v foot >/dev/null 2>&1; then
                foot &
            fi
            ;;
    esac
}
```

---

## Customization Guide

### Adding Auto-Start Applications

Edit `configs/raven-wayland-session`, find `start_session_components()`, and add your applications:

```sh
case "$compositor" in
    raven-compositor*|raven*)
        # Existing components...

        # ADD YOUR APPLICATIONS HERE:
        # Example: Start a status bar
        if command -v waybar >/dev/null 2>&1; then
            waybar &
        fi
        ;;
esac
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

2. Add to the selection case:
```sh
case "$preferred" in
    mycompositor)
        start_mycompositor && exit 0
        ;;
    # ... existing cases
esac
```

3. Add session components in `start_session_components()`:
```sh
case "$compositor" in
    mycompositor*)
        raven-terminal &  # or your preferred terminal
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
