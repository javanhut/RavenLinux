#!/bin/bash
# Start Raven Desktop Environment with all prerequisites

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

echo "=== Raven Desktop Environment Startup ==="
echo ""

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "This script needs to be run with sudo to start seatd"
    echo ""
    echo "Usage: sudo ./scripts/start-raven-desktop.sh"
    echo ""
    exit 1
fi

# Get the actual user (not root)
ACTUAL_USER="${SUDO_USER:-$(logname 2>/dev/null || echo $USER)}"
ACTUAL_HOME=$(eval echo ~$ACTUAL_USER)

echo "Running as: root (for seatd)"
echo "User: $ACTUAL_USER"
echo "Home: $ACTUAL_HOME"
echo ""

# Start seatd if not running
if pgrep -x seatd >/dev/null; then
    echo "✓ seatd is already running"
else
    echo "Starting seatd..."
    seatd -g video >/tmp/seatd.log 2>&1 &
    SEATD_PID=$!
    
    # Wait for seatd socket
    for i in {1..50}; do
        if [ -S /run/seatd.sock ]; then
            echo "✓ seatd started (PID: $SEATD_PID)"
            break
        fi
        sleep 0.1
    done
    
    if [ ! -S /run/seatd.sock ]; then
        echo "ERROR: seatd failed to start"
        cat /tmp/seatd.log
        exit 1
    fi
fi

echo ""

# Set up environment
export XDG_RUNTIME_DIR="/run/user/$(id -u $ACTUAL_USER)"
mkdir -p "$XDG_RUNTIME_DIR" 2>/dev/null || true
chmod 700 "$XDG_RUNTIME_DIR"

export HOME="$ACTUAL_HOME"
export USER="$ACTUAL_USER"
export PATH="/tmp/raven-compositor-build/release:$PATH"
export PATH="$PWD/desktop/raven-shell:$PATH"
export PATH="$PWD/desktop/raven-desktop:$PATH"
export PATH="$PWD/desktop/raven-menu:$PATH"
export PATH="$PWD/tools/raven-terminal:$PATH"

echo "Environment:"
echo "  XDG_RUNTIME_DIR: $XDG_RUNTIME_DIR"
echo "  USER: $USER"
echo "  HOME: $HOME"
echo ""

echo "Checking binaries..."
which raven-compositor || { echo "ERROR: raven-compositor not found"; exit 1; }
echo ""

echo "Checking DRM/KMS..."
if [ ! -d /dev/dri ]; then
    echo "ERROR: /dev/dri not found"
    exit 1
fi
ls -la /dev/dri/
echo ""

echo "=== Starting Raven Compositor ==="
echo ""

# Run compositor
exec raven-compositor
