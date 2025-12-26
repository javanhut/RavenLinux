#!/bin/bash
# Test the compositor with built binaries

cd /home/javanstorm/Development/CustomLinux/RavenLinux

# Use the compositor from /tmp
export PATH="/tmp/raven-compositor-build/release:$PATH"
export PATH="$PWD/desktop/raven-shell/target/release:$PATH"
export PATH="$PWD/tools/raven-terminal:$PATH"

# Set up XDG_RUNTIME_DIR if not set
if [ -z "$XDG_RUNTIME_DIR" ]; then
    export XDG_RUNTIME_DIR="/run/user/$(id -u)"
    mkdir -p "$XDG_RUNTIME_DIR" 2>/dev/null || true
fi

echo "=== Raven Desktop Test ==="
echo "Binaries in PATH:"
which raven-compositor
which raven-shell
which raven-desktop
which raven-menu
which raven-terminal
echo ""

# Check for seatd
echo "Checking prerequisites..."
if ! pgrep -x seatd >/dev/null; then
    echo "ERROR: seatd is not running!"
    echo ""
    echo "Please start seatd first:"
    echo "  sudo seatd -g video"
    echo ""
    echo "Or run this script with seatd startup:"
    echo "  sudo seatd -g video & sleep 1 && sudo -E ./scripts/test-compositor.sh"
    echo ""
    exit 1
fi

echo "✓ seatd is running"

# Check for /dev/dri
if [ ! -d /dev/dri ]; then
    echo "ERROR: /dev/dri not found - no DRM/KMS device available"
    echo "Are you running in a VM with proper GPU device?"
    exit 1
fi

echo "✓ /dev/dri exists"
ls -la /dev/dri/

echo ""
echo "Starting raven-compositor..."
echo "Watch for log output below:"
echo ""

exec raven-compositor
