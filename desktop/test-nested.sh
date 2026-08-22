#!/bin/bash
# test-nested.sh - Test Raven compositor in nested mode
# Usage: ./test-nested.sh [--nested]
#   Default: Run compositor on current Wayland session (nested winit)
#   --nested: Same as default (kept for compatibility)
#
# Desktop Go apps have been removed. This script now only tests the compositor.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Source repos library
source "${PROJECT_ROOT}/scripts/lib/repos.sh"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log() { echo -e "${GREEN}[TEST]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }
info() { echo -e "${BLUE}[INFO]${NC} $1"; }

COMPOSITOR_DIR="$(get_repo_dir compositor)"

# Check Wayland environment
check_wayland_env() {
    log "Checking Wayland environment..."

    if [ -z "$XDG_RUNTIME_DIR" ]; then
        error "XDG_RUNTIME_DIR is not set. Are you in a graphical session?"
    fi

    if [ -z "$WAYLAND_DISPLAY" ] && [ -z "$DISPLAY" ]; then
        error "No display found. Run this from within a graphical session."
    fi

    if [ -n "$WAYLAND_DISPLAY" ]; then
        log "Wayland session detected: $WAYLAND_DISPLAY"
    elif [ -n "$DISPLAY" ]; then
        warn "X11 session detected."
    fi
}

# Check dependencies
check_deps() {
    log "Checking dependencies..."

    local compositor_bin="${COMPOSITOR_DIR}/target/release/raven-compositor"
    if [[ ! -f "$compositor_bin" ]]; then
        error "raven-compositor not found at ${compositor_bin}. Build with: ./scripts/build-packages.sh compositor"
    fi

    log "All dependencies found"
}

echo ""
echo "=========================================="
echo "  Raven Compositor - Nested Test"
echo "=========================================="
echo ""

check_wayland_env
check_deps

log "Starting nested raven-compositor (winit backend)..."
echo ""
echo "The compositor will open in a window."
echo "Use Super+Alt+Q to exit the nested session."
echo ""

"${COMPOSITOR_DIR}/target/release/raven-compositor" --backend winit

log "Nested session ended"
