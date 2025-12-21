#!/bin/bash
# =============================================================================
# RavenLinux Compositor Build Script
# =============================================================================
# Builds the raven-compositor Wayland compositor

set -euo pipefail

# =============================================================================
# Environment Setup
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${RAVEN_ROOT:-$(dirname "$SCRIPT_DIR")}"
COMPOSITOR_DIR="${PROJECT_ROOT}/desktop/compositor"
OUTPUT_DIR="${RAVEN_BUILD:-${PROJECT_ROOT}/build}/packages/bin"

# =============================================================================
# Logging (use shared library or define fallbacks)
# =============================================================================

if [[ -f "${SCRIPT_DIR}/lib/logging.sh" ]]; then
    source "${SCRIPT_DIR}/lib/logging.sh"
else
    # Fallback logging functions
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    BLUE='\033[0;34m'
    NC='\033[0m'
    log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
    log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
    log_error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }
fi

echo ""
echo "=========================================="
echo "  Building Raven Compositor"
echo "=========================================="
echo ""

# Check for Rust
if ! command -v cargo &>/dev/null; then
    log_error "Rust/Cargo not found. Install with: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
fi

# Check for required system libraries
log_info "Checking system dependencies..."

required_libs=(libwayland-dev libinput-dev libudev-dev libgbm-dev libdrm-dev libseat-dev libxkbcommon-dev)
# On Arch, these are different package names - just warn
log_info "Required: wayland libinput udev libgbm libdrm seatd libxkbcommon"

cd "$COMPOSITOR_DIR"

log_info "Building compositor..."
if CARGO_TARGET_DIR=target-user cargo build --release 2>&1; then
    mkdir -p "$OUTPUT_DIR"
    cp target-user/release/raven-compositor "$OUTPUT_DIR/"
    log_success "Compositor built -> ${OUTPUT_DIR}/raven-compositor"
else
    log_error "Build failed"
fi

# Build raven-shell (GTK4 panel/taskbar)
SHELL_DIR="${PROJECT_ROOT}/desktop/raven-shell"
if [[ -d "$SHELL_DIR" ]]; then
    echo ""
    echo "=========================================="
    echo "  Building Raven Shell"
    echo "=========================================="
    echo ""

    cd "$SHELL_DIR"
    log_info "Building raven-shell (GTK4 panel)..."

    if CGO_ENABLED=1 go build -o raven-shell . 2>&1; then
        cp raven-shell "$OUTPUT_DIR/"
        log_success "Shell built -> ${OUTPUT_DIR}/raven-shell"
    else
        log_info "Shell build failed (may need gtk4-layer-shell installed)"
    fi
fi

# Build raven-menu (GTK4 start menu)
MENU_DIR="${PROJECT_ROOT}/desktop/raven-menu"
if [[ -d "$MENU_DIR" ]]; then
    echo ""
    echo "=========================================="
    echo "  Building Raven Menu"
    echo "=========================================="
    echo ""

    cd "$MENU_DIR"
    log_info "Building raven-menu (GTK4 start menu)..."

    if CGO_ENABLED=1 go build -o raven-menu . 2>&1; then
        cp raven-menu "$OUTPUT_DIR/"
        log_success "Menu built -> ${OUTPUT_DIR}/raven-menu"
    else
        log_info "Menu build failed (may need gtk4-layer-shell installed)"
    fi
fi

# Build raven-desktop (GTK4 desktop with icons)
DESKTOP_DIR="${PROJECT_ROOT}/desktop/raven-desktop"
if [[ -d "$DESKTOP_DIR" ]]; then
    echo ""
    echo "=========================================="
    echo "  Building Raven Desktop"
    echo "=========================================="
    echo ""

    cd "$DESKTOP_DIR"
    log_info "Building raven-desktop (GTK4 desktop)..."

    if CGO_ENABLED=1 go build -o raven-desktop . 2>&1; then
        cp raven-desktop "$OUTPUT_DIR/"
        log_success "Desktop built -> ${OUTPUT_DIR}/raven-desktop"
    else
        log_info "Desktop build failed (may need gtk4-layer-shell installed)"
    fi
fi

echo ""
echo "To run the compositor:"
echo "  Native:  switch to TTY and run: raven-compositor"
echo "  Nested:  raven-compositor --nested"
echo ""
echo "The raven-shell panel will auto-start with the compositor."
echo ""
