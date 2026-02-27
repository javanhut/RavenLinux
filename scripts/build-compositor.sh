#!/bin/bash
# =============================================================================
# RavenLinux Compositor Build Script
# =============================================================================
# Builds the raven-compositor Wayland compositor from GitHub
# Produces: raven-compositor, raven-shell, raven-settings

set -euo pipefail

# =============================================================================
# Environment Setup
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${RAVEN_ROOT:-$(dirname "$SCRIPT_DIR")}"
OUTPUT_DIR="${RAVEN_BUILD:-${PROJECT_ROOT}/build}/packages/bin"

# =============================================================================
# Source libraries
# =============================================================================

if [[ -f "${SCRIPT_DIR}/lib/logging.sh" ]]; then
    source "${SCRIPT_DIR}/lib/logging.sh"
else
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    BLUE='\033[0;34m'
    NC='\033[0m'
    log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
    log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
    log_error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }
fi

source "${SCRIPT_DIR}/lib/repos.sh"

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

log_info "Required: wayland libinput udev libgbm libdrm seatd libxkbcommon"

# Fetch and build compositor from GitHub
fetch_repo compositor
COMPOSITOR_DIR="$(get_repo_dir compositor)"

cd "$COMPOSITOR_DIR"

log_info "Building compositor..."
if cargo build --release 2>&1; then
    mkdir -p "$OUTPUT_DIR"
    for bin in raven-compositor raven-shell raven-settings; do
        if [[ -f "target/release/${bin}" ]]; then
            cp "target/release/${bin}" "$OUTPUT_DIR/"
            log_success "${bin} built -> ${OUTPUT_DIR}/${bin}"
        fi
    done
else
    log_error "Build failed"
fi

echo ""
echo "To run the compositor:"
echo "  Native:  switch to TTY and run: raven-compositor"
echo "  Nested:  raven-compositor --nested"
echo ""
