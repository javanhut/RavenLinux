#!/bin/bash
# Test the compositor with built binaries

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
export RAVEN_ROOT="$PROJECT_ROOT"
export RAVEN_BUILD="${PROJECT_ROOT}/build"

# Source repos library
source "${SCRIPT_DIR}/lib/repos.sh"

COMPOSITOR_DIR="$(get_repo_dir compositor)"

# Add compositor binaries to PATH
export PATH="${COMPOSITOR_DIR}/target/release:$PATH"
export PATH="${RAVEN_BUILD}/packages/bin:$PATH"

# Set up XDG_RUNTIME_DIR if not set
if [ -z "$XDG_RUNTIME_DIR" ]; then
    export XDG_RUNTIME_DIR="/run/user/$(id -u)"
    mkdir -p "$XDG_RUNTIME_DIR" 2>/dev/null || true
fi

echo "=== Raven Desktop Test ==="
echo "Binaries in PATH:"
which raven-compositor || echo "  raven-compositor: not found"
which raven-terminal || echo "  raven-terminal: not found"
echo ""

# Check for seatd
echo "Checking prerequisites..."
if ! pgrep -x seatd >/dev/null; then
    echo "ERROR: seatd is not running!"
    echo ""
    echo "Please start seatd first:"
    echo "  sudo seatd -g video"
    echo ""
    exit 1
fi

echo "seatd is running"

# Check for /dev/dri
if [ ! -d /dev/dri ]; then
    echo "ERROR: /dev/dri not found - no DRM/KMS device available"
    echo "Are you running in a VM with proper GPU device?"
    exit 1
fi

echo "/dev/dri exists"
ls -la /dev/dri/

echo ""
echo "Starting raven-compositor..."
echo "Watch for log output below:"
echo ""

exec raven-compositor
