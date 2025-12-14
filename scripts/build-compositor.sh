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

echo ""
echo "To run the compositor:"
echo "  Native:  switch to TTY and run: raven-compositor"
echo "  Nested:  raven-compositor --nested"
echo ""
