# Raven Desktop - Quick Start

## TL;DR - Get Running in 3 Steps

### 1. Fix Permissions
```bash
sudo chown -R $USER:$USER ~/Development/CustomLinux/RavenLinux/desktop/compositor/target*
```

### 2. Build Everything
```bash
cd ~/Development/CustomLinux/RavenLinux
./scripts/build-desktop-local.sh
```

### 3. Run in QEMU
```bash
# In your QEMU VM (with proper GPU device):
export PATH="$PWD/desktop/compositor/target-user/release:$PATH"
export PATH="$PWD/desktop/raven-shell:$PATH"
export PATH="$PWD/desktop/raven-desktop:$PATH"
export PATH="$PWD/desktop/raven-menu:$PATH"
export PATH="$PWD/tools/raven-terminal:$PATH"

# Start it!
raven-compositor
```

## What to Expect

You should see:
- 🖥️ Dark background
- 📊 Panel at top with "Raven" button and clock
- 🖱️ Working mouse cursor

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Super + Enter` | Open terminal |
| `Super + Space` | Open start menu |
| `Super + Q` | Close window |

## Troubleshooting 1-Liners

**No display?**
```bash
ls -la /dev/dri/  # Should show card0 or card1
```

**Components not launching?**
```bash
WAYLAND_DISPLAY=wayland-0 ./desktop/raven-shell/raven-shell
# Check error messages
```

**seatd not running?**
```bash
sudo seatd -g video &
```

## QEMU Command

```bash
qemu-system-x86_64 \
  -enable-kvm -m 4G -cpu host -smp 4 \
  -drive file=ravenlinux.img,format=raw,if=virtio \
  -device virtio-vga-gl -display gtk,gl=on \
  -device virtio-keyboard-pci -device virtio-mouse-pci \
  -serial mon:stdio
```

**Key flags:** `virtio-vga-gl` (GPU) + `gtk,gl=on` (display)

## Files to Read

- **Full guide**: `desktop/TESTING.md`
- **Implementation details**: `desktop/IMPLEMENTATION_SUMMARY.md`
- **Architecture**: `desktop/DESIGN.md`

## Status Check

```bash
# After running compositor, check:
ps aux | grep raven-compositor  # Should be running
ls $XDG_RUNTIME_DIR/wayland-*   # Should exist
dmesg | grep drm                # Check DRM init
```

## Got Issues?

1. Check serial console output (QEMU terminal)
2. Read `/run/raven-wayland-session.log`
3. Try components individually with `WAYLAND_DISPLAY=wayland-0`

## What Works

✅ Software rendering  
✅ Keyboard shortcuts  
✅ Mouse input  
✅ Window focus  
✅ Panel and desktop  
✅ Layer shell integration  

## What Doesn't (Yet)

❌ Window dragging  
❌ Window decorations  
❌ XWayland (X11 apps)  
❌ GPU acceleration  

---

**Need more help?** Read `desktop/TESTING.md` for detailed troubleshooting!
