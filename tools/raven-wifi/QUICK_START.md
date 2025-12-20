# Raven WiFi - Quick Start Guide

## Build & Run

```bash
# Build optimized binary
go build -ldflags="-s -w" -o raven-wifi .

# Run (requires root for WiFi management)
sudo -E ./raven-wifi

# Note: -E preserves environment variables needed for GUI display
```

## Usage

### Main Window
- **Network List** - All available WiFi networks
- **Status Card** - Current connection status and IP address
- **Refresh Button** (↻) - Rescan for networks
- **Saved Button** - Manage saved networks
- **Disconnect Button** - Disconnect from current network

### Connecting to Networks

**Open Network (No Password)**
1. Click network name → Connects immediately

**Secured Network (First Time)**
1. Click network name → Password dialog appears
2. Enter password → Click "Connect"

**Secured Network (Known Password)**
1. Click network name → Connects immediately (password remembered)

**Already Connected Network**
1. Click network name → No action (already connected)

### Managing Saved Networks
1. Click "Saved" button
2. View all saved network passwords
3. Click trash icon (🗑) to forget a network
4. Confirm deletion

### Window Behavior
- **Resize:** Drag window edges (minimum 300x400)
- **Persistence:** Window size is automatically saved
- **Next Launch:** Opens at last used size

## Visual Indicators

### Signal Strength
- 🟢 **Green** - Strong signal (≥70%)
- 🟡 **Amber** - Medium signal (40-69%)
- 🔴 **Red** - Weak signal (<40%)

### Network Icons
- 🔒 **Lock** - Secured network (password required)
- ✓ **Checkmark** - Currently connected
- ⋯ **Dots** - Connecting...

### WiFi Signal Bars
- 📶 4 bars - 80%+
- 📶 3 bars - 60-79%
- 📶 2 bars - 40-59%
- 📶 1 bar - 20-39%
- 📶 0 bars - <20%

## Troubleshooting

### "Error: XDG_RUNTIME_DIR is invalid or not set"
**Cause:** Running `sudo` without preserving environment variables

**Solution:**
```bash
# Use -E flag to preserve environment
sudo -E ./raven-wifi
```

**Alternative solutions:**
```bash
# Set XDG_RUNTIME_DIR explicitly
sudo XDG_RUNTIME_DIR=/run/user/$(id -u $SUDO_USER) ./raven-wifi

# Configure sudo to preserve environment (add to /etc/sudoers)
Defaults env_keep += "XDG_RUNTIME_DIR WAYLAND_DISPLAY DISPLAY"
```

### "This tool requires root privileges"
**Solution:** Run with `sudo -E ./raven-wifi`

### No networks found
**Possible causes:**
- WiFi adapter not detected
- Incorrect wireless interface
- iwd/wpa_supplicant not running

**Check WiFi adapter:**
```bash
ip link show | grep wlan
iw dev
```

**Check WiFi daemon:**
```bash
# For iwd
systemctl status iwd

# For wpa_supplicant
systemctl status wpa_supplicant
```

### Connection fails
**Check password:** Saved passwords are stored in:
- iwd: `/var/lib/iwd/`
- wpa_supplicant: `/etc/wpa_supplicant/wpa_supplicant.conf`

### Window doesn't remember size
**Check config file:**
```bash
cat ~/.config/raven-wifi/window.json
```

**Expected format:**
```json
{
  "window": {
    "width": 400,
    "height": 550
  }
}
```

## Configuration Files

### Window Settings
- **Location:** `~/.config/raven-wifi/window.json`
- **Auto-created:** Yes
- **Format:** JSON
- **Editable:** Yes (app must be closed)

### WiFi Credentials

**iwd** (preferred)
- **Location:** `/var/lib/iwd/*.psk` or `*.open`
- **Managed by:** iwd daemon
- **Format:** INI-style

**wpa_supplicant**
- **Location:** `/etc/wpa_supplicant/wpa_supplicant.conf`
- **Managed by:** wpa_supplicant
- **Format:** wpa_supplicant config

## Keyboard Shortcuts

Currently, all interactions are mouse/touch-based.
Keyboard shortcuts may be added in future versions.

## System Requirements

### Minimum
- OpenGL 2.0+ (or software rendering)
- Wayland or X11
- One of: iwd, wpa_supplicant, or iw command
- Root privileges

### Recommended
- Wayland compositor (for best performance)
- iwd daemon (for reliable WiFi management)
- HiDPI display support (automatic scaling)

## Performance

- **Memory usage:** ~20-30MB
- **CPU usage:** <1% idle, <5% when scanning
- **Startup time:** <1 second
- **UI responsiveness:** 60fps smooth scrolling

## Support

For issues specific to RavenLinux, refer to the main RavenLinux documentation.

For Gio framework issues, see: https://gioui.org

## Advanced

### Custom Build Flags

**Debug build:**
```bash
go build -gcflags="all=-N -l" -o raven-wifi .
```

**Static binary (if possible):**
```bash
CGO_ENABLED=1 go build -ldflags="-s -w -extldflags '-static'" -o raven-wifi .
```
Note: May not work due to EGL/Wayland dependencies

**Cross-compile for ARM:**
```bash
GOARCH=arm64 go build -ldflags="-s -w" -o raven-wifi-arm64 .
```

### Environment Variables

**Force X11 backend:**
```bash
GIO_BACKEND=x11 sudo ./raven-wifi
```

**Force Wayland backend:**
```bash
GIO_BACKEND=wayland sudo ./raven-wifi
```

**Enable debug logging:**
```bash
GIO_DEBUG=1 sudo ./raven-wifi
```

## FAQ

**Q: Does this work without iwd?**  
A: Yes, it supports wpa_supplicant and raw iw commands as fallbacks.

**Q: Can I run this without root?**  
A: No, WiFi management requires root privileges for security reasons.

**Q: Why is the binary 7.3MB?**  
A: It includes the entire Gio UI framework, font rendering, and icons. This is normal for standalone Go GUI apps.

**Q: Does it work on Wayland?**  
A: Yes! Native Wayland support with X11 fallback.

**Q: Can I customize the theme?**  
A: Currently no GUI for theme customization. Edit `theme.go` and rebuild.

**Q: Why single-click instead of double-click?**  
A: Modern UX pattern, faster workflow. Same as GNOME Settings and most modern WiFi managers.

**Q: Can I see the password for saved networks?**  
A: Check the config files manually (see Configuration Files section above).

**Q: How do I uninstall?**  
A: Delete the binary and `~/.config/raven-wifi/` directory.
