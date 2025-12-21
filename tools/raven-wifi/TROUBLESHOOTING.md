# Raven WiFi - Troubleshooting Guide

## Common Issues and Solutions

### 1. XDG_RUNTIME_DIR Error

**Error message:**
```
error: XDG_RUNTIME_DIR is invalid or not set in the environment.
```

**Cause:** When you run `sudo`, it clears most environment variables for security. The GUI needs `XDG_RUNTIME_DIR` to connect to the display server (Wayland or X11).

**Solutions (pick one):**

#### Solution A: Use the wrapper script (easiest)
```bash
./raven-wifi.sh
```
The script automatically preserves environment variables.

#### Solution B: Use sudo -E flag
```bash
sudo -E ./raven-wifi
```
The `-E` flag tells sudo to preserve environment variables.

#### Solution C: Set the variable explicitly
```bash
sudo XDG_RUNTIME_DIR=/run/user/$(id -u $SUDO_USER) \
     WAYLAND_DISPLAY=$WAYLAND_DISPLAY \
     DISPLAY=$DISPLAY \
     ./raven-wifi
```

#### Solution D: Configure sudo permanently
Edit `/etc/sudoers` (use `sudo visudo`):
```
Defaults env_keep += "XDG_RUNTIME_DIR WAYLAND_DISPLAY DISPLAY"
```

Then you can just run:
```bash
sudo ./raven-wifi
```

---

### 2. "This tool requires root privileges"

**Error message:**
```
This tool requires root privileges. Please run with sudo:
  sudo -E raven-wifi
```

**Cause:** WiFi management requires root privileges to configure network interfaces.

**Solution:**
```bash
sudo -E ./raven-wifi
# or
./raven-wifi.sh
```

---

### 3. Window doesn't open / Black screen

**Possible causes:**
- Wayland/X11 display server not accessible
- Missing graphics libraries
- OpenGL issues

**Debugging steps:**

1. Check if you're running a display server:
```bash
echo $WAYLAND_DISPLAY  # Should show something like "wayland-0"
echo $DISPLAY          # Should show something like ":0" or ":1"
```

2. Test with X11 fallback:
```bash
GIO_BACKEND=x11 sudo -E ./raven-wifi
```

3. Check OpenGL support:
```bash
glxinfo | grep "OpenGL version"
```

4. Verify EGL libraries:
```bash
ldd ./raven-wifi | grep -i egl
```

---

### 4. No networks found

**Possible causes:**
- WiFi adapter not detected
- Wrong wireless interface
- iwd/wpa_supplicant not running
- WiFi adapter not powered on

**Debugging steps:**

1. Check WiFi adapter exists:
```bash
ip link show | grep -i wlan
iw dev
```

2. Check adapter is up:
```bash
ip link show wlan0  # Replace wlan0 with your interface
```

3. Power on if needed:
```bash
sudo ip link set wlan0 up
```

4. Check WiFi daemon is running:
```bash
# For iwd
systemctl status iwd

# For wpa_supplicant
systemctl status wpa_supplicant
```

5. Start daemon if needed:
```bash
# For iwd
sudo systemctl start iwd
sudo systemctl enable iwd

# For wpa_supplicant
sudo systemctl start wpa_supplicant
sudo systemctl enable wpa_supplicant
```

6. Check if interface is soft-blocked:
```bash
rfkill list
# If WiFi is blocked:
sudo rfkill unblock wifi
```

---

### 5. Connection fails

**Symptoms:**
- Password dialog appears but connection fails
- "Connection Failed" error dialog

**Possible causes:**
- Wrong password
- Network out of range
- DHCP failure
- Conflicting network managers

**Debugging steps:**

1. Check password is correct:
   - Try connecting from another device first
   - Passwords are case-sensitive

2. Forget network and reconnect:
   - Click "Saved" button
   - Delete the network
   - Reconnect with correct password

3. Check DHCP client is available:
```bash
which dhcpcd dhclient udhcpc
```

4. Manually test connection:
```bash
# For iwd
sudo iwctl station wlan0 connect "NetworkName"

# For wpa_supplicant
sudo wpa_cli -i wlan0 scan
sudo wpa_cli -i wlan0 scan_results
```

5. Check for conflicting network managers:
```bash
systemctl status NetworkManager
systemctl status systemd-networkd
```
If using iwd, disable NetworkManager:
```bash
sudo systemctl disable --now NetworkManager
```

---

### 6. Window size not remembered

**Possible causes:**
- Config directory not writable
- Disk full
- Permissions issue

**Debugging steps:**

1. Check config file exists:
```bash
cat ~/.config/raven-wifi/window.json
```

2. Check directory permissions:
```bash
ls -la ~/.config/raven-wifi/
```

3. Manually create config:
```bash
mkdir -p ~/.config/raven-wifi
cat > ~/.config/raven-wifi/window.json << EOF
{
  "window": {
    "width": 400,
    "height": 550
  }
}
EOF
```

4. Check disk space:
```bash
df -h ~
```

---

### 7. Performance issues / Slow UI

**Possible causes:**
- Software rendering (no GPU acceleration)
- Old graphics drivers
- High CPU usage from scanning

**Debugging steps:**

1. Check if using hardware acceleration:
```bash
glxinfo | grep "direct rendering"
# Should say "Yes"
```

2. Check GPU info:
```bash
lspci | grep -i vga
glxinfo | grep "OpenGL renderer"
```

3. Force software rendering as test:
```bash
LIBGL_ALWAYS_SOFTWARE=1 sudo -E ./raven-wifi
```

4. Update graphics drivers

5. Reduce scan frequency (edit wifi.go if needed)

---

### 8. Saved passwords don't work

**Possible causes:**
- Passwords stored in different backend
- Corrupted config files
- Permission issues

**Debugging steps:**

1. Check which backend is being used:
```bash
ps aux | grep -E "(iwd|wpa_supplicant)"
```

2. Check saved passwords location:

**For iwd:**
```bash
sudo ls -la /var/lib/iwd/
sudo cat /var/lib/iwd/YourNetwork.psk
```

**For wpa_supplicant:**
```bash
sudo cat /etc/wpa_supplicant/wpa_supplicant.conf
```

3. Manually delete corrupted entries:
```bash
# For iwd
sudo rm /var/lib/iwd/YourNetwork.psk

# For wpa_supplicant
sudo nano /etc/wpa_supplicant/wpa_supplicant.conf
# Remove the network block
```

---

### 9. Can't forget network

**Symptoms:**
- Click delete button but network still appears

**Cause:** May be connected to that network

**Solution:**
1. Disconnect first
2. Then forget the network

---

### 10. Binary size larger than expected

**Explanation:** This is normal for Go GUI apps. The 7.3MB includes:
- Gio UI framework
- Font rendering engine
- Material Design icons
- All Go runtime

**To reduce size further:**
```bash
# Use UPX compression (if available)
upx --best --lzma raven-wifi
# Result: ~3-4MB
```

---

## Debug Mode

Run with verbose output:

```bash
# Enable Gio debug output
GIO_DEBUG=1 sudo -E ./raven-wifi 2>&1 | tee debug.log

# Check all environment variables being passed
sudo -E env | grep -E "(DISPLAY|WAYLAND|XDG)" 
```

---

## Checking Dependencies

Verify all required libraries are available:

```bash
# Check what libraries the binary needs
ldd ./raven-wifi

# Check for missing libraries
ldd ./raven-wifi | grep "not found"

# Check specific graphics libraries
ldd ./raven-wifi | grep -E "(EGL|wayland|X11)"
```

Expected libraries:
- ✅ libEGL.so.1
- ✅ libwayland-client.so.0
- ✅ libX11.so.6
- ✅ libm.so.6
- ✅ libc.so.6

Should NOT require:
- ❌ libgtk-4.so
- ❌ libadwaita-1.so
- ❌ Any Python libraries

---

## Platform-Specific Issues

### Wayland-specific

**Test Wayland directly:**
```bash
GIO_BACKEND=wayland sudo -E ./raven-wifi
```

**Check Wayland compositor:**
```bash
echo $WAYLAND_DISPLAY
ps aux | grep -i compositor
```

### X11-specific

**Test X11 directly:**
```bash
GIO_BACKEND=x11 sudo -E ./raven-wifi
```

**Check X11 server:**
```bash
echo $DISPLAY
xdpyinfo | head
```

---

## Getting Help

If none of the above solutions work:

1. **Collect debug info:**
```bash
# System info
uname -a
echo $XDG_SESSION_TYPE

# Display info
echo $WAYLAND_DISPLAY
echo $DISPLAY

# WiFi info
ip link show
iw dev
systemctl status iwd
systemctl status wpa_supplicant

# Binary info
ldd ./raven-wifi
file ./raven-wifi

# Config
cat ~/.config/raven-wifi/window.json
```

2. **Run with full debug:**
```bash
GIO_DEBUG=1 sudo -E ./raven-wifi 2>&1 | tee raven-wifi-debug.log
```

3. **Check logs:**
```bash
# For iwd
sudo journalctl -u iwd -n 50

# For wpa_supplicant
sudo journalctl -u wpa_supplicant -n 50

# System logs
sudo dmesg | grep -i wifi
```

4. **Report the issue** with all the above information

---

## Quick Reference

**Most common command:**
```bash
./raven-wifi.sh
```

**If that doesn't work:**
```bash
sudo -E ./raven-wifi
```

**If GUI won't start:**
```bash
GIO_BACKEND=x11 sudo -E ./raven-wifi
```

**If no networks found:**
```bash
sudo systemctl start iwd
sudo rfkill unblock wifi
sudo ip link set wlan0 up
```
