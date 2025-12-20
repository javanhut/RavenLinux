# Running Raven Desktop Environment

## 🚀 Quick Start

### Method 1: Automated Startup (Recommended)

This script starts seatd and the compositor automatically:

```bash
cd ~/Development/CustomLinux/RavenLinux
sudo ./scripts/start-raven-desktop.sh
```

### Method 2: Manual Startup

```bash
# 1. Start seatd (in one terminal)
sudo seatd -g video

# 2. Start compositor (in another terminal)
cd ~/Development/CustomLinux/RavenLinux
sudo -E ./scripts/test-compositor.sh
```

---

## ✅ Prerequisites

Before running, ensure:

1. **seatd is installed**
   ```bash
   which seatd
   ```

2. **DRM/KMS device exists**
   ```bash
   ls /dev/dri/
   ```
   Should show `card0`, `card1`, or similar

3. **User is in video group** (optional but recommended)
   ```bash
   sudo usermod -a -G video $USER
   # Then logout/login
   ```

---

## 📊 Expected Output

### Successful Start

```
=== Raven Desktop Environment Startup ===

Running as: root (for seatd)
User: javanstorm
Home: /home/javanstorm

✓ seatd started (PID: 12345)

Environment:
  XDG_RUNTIME_DIR: /run/user/1000
  USER: javanstorm
  HOME: /home/javanstorm

Checking binaries...
/tmp/raven-compositor-build/release/raven-compositor

Checking DRM/KMS...
drwxr-xr-x 2 root root card1

=== Starting Raven Compositor ===

=== RAVEN-COMPOSITOR STARTING ===
PID: 12346
...
INFO Starting native backend with libseat session
INFO Initializing session (libseat)
INFO Session created on seat: seat0
INFO Found DRM device: "/dev/dri/card1"
INFO Using mode: 1280x800@75Hz
...
INFO Initializing Wayland display
INFO Wayland socket: "wayland-0"
...
=== ENTERING MAIN EVENT LOOP ===
```

### If It Fails

**Error: "seatd is not running"**
```bash
# Start seatd first:
sudo seatd -g video
```

**Error: "Failed to create session"**
```bash
# Check if seatd socket exists:
ls -la /run/seatd.sock

# If not, restart seatd:
sudo pkill seatd
sudo seatd -g video
```

**Error: "/dev/dri not found"**
- You need a GPU device in your VM or on hardware
- For QEMU, use: `-device virtio-vga-gl` or `-device qxl-vga`

---

## 🧪 Testing Client Connections

Once the compositor is running, test with a simple Wayland client:

```bash
# In another terminal
export WAYLAND_DISPLAY=wayland-0

# Try a simple client (if you have it installed)
weston-terminal
# or
foot
# or
gtk4-demo
```

If the client starts without errors, the compositor is working!

---

## 🛑 Stopping

Press `Ctrl+C` in the compositor terminal, or:

```bash
sudo pkill raven-compositor
sudo pkill seatd  # If you want to stop seatd too
```

---

## 📝 Logs

**Compositor logs:** Appear in the terminal where you ran it

**seatd logs:** `/tmp/seatd.log` (if using start-raven-desktop.sh)

**Session logs:** `/run/raven-wayland-session.log` (if using session script)

---

## ⚠️ Known Issues

### No Visual Output

**Symptom:** Compositor starts but screen stays black

**Cause:** virtio-vga doesn't support dumb buffers

**Solutions:**
1. Try different QEMU GPU: `-device qxl-vga`
2. Test on real hardware
3. Check logs for "Failed to create dumb buffer" warning

**Workaround:** The compositor still works! Clients can connect even without visual output.

### Permission Errors

**Symptom:** "Permission denied" accessing `/dev/dri`

**Solution:**
```bash
# Add user to video group
sudo usermod -a -G video $USER
# Logout and login

# Or run with sudo
sudo -E ./scripts/start-raven-desktop.sh
```

---

## 🎯 Success Criteria

You'll know it's working when:

1. ✅ Compositor starts without errors
2. ✅ "ENTERING MAIN EVENT LOOP" appears
3. ✅ Wayland socket created: `ls $XDG_RUNTIME_DIR/wayland-*`
4. ✅ Clients can connect: `export WAYLAND_DISPLAY=wayland-0 && <some-client>`

Visual output is optional - the compositor works even without it!

---

## 🚀 Next Steps

Once running:

1. **Test shell components:**
   ```bash
   export WAYLAND_DISPLAY=wayland-0
   ./desktop/raven-shell/raven-shell &
   ./desktop/raven-desktop/raven-desktop &
   ```

2. **Test terminal:**
   ```bash
   ./tools/raven-terminal/raven-terminal &
   ```

3. **Test keyboard shortcuts:**
   - `Super + Enter` should launch terminal
   - `Super + Space` should launch menu
   - `Super + Q` should close focused window

4. **Check compositor state:**
   - Watch for "Adding layer surface" messages
   - Watch VBlank counter increase

---

## 📚 Troubleshooting

See `desktop/TESTING.md` for comprehensive troubleshooting guide.

Quick checks:

```bash
# Is compositor running?
ps aux | grep raven-compositor

# Is seatd running?
ps aux | grep seatd

# Wayland socket exists?
ls -la $XDG_RUNTIME_DIR/wayland-*

# Check DRM devices
ls -la /dev/dri/

# Check seatd socket
ls -la /run/seatd.sock
```

---

## 🎉 You're Ready!

Run the compositor and watch it start up. Even without visual output, you've built a fully functional Wayland compositor from scratch!

```bash
sudo ./scripts/start-raven-desktop.sh
```

Good luck! 🚀
