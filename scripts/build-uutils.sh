#!/bin/bash
# Build uutils-coreutils from source for RavenLinux

set -euo pipefail

RAVEN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RAVEN_BUILD="${RAVEN_ROOT}/build"
UUTILS_VERSION="0.0.28"
UUTILS_DIR="${RAVEN_BUILD}/uutils-coreutils"
UUTILS_OUTPUT="${RAVEN_BUILD}/bin"

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Check for Rust
if ! command -v cargo &>/dev/null; then
    log_error "Rust/Cargo not found. Install with: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
fi

log_info "Building uutils-coreutils ${UUTILS_VERSION}..."

mkdir -p "${RAVEN_BUILD}"
cd "${RAVEN_BUILD}"

# Remove old source if exists to get fresh version
if [[ -d "${UUTILS_DIR}" ]]; then
    log_info "Removing old uutils source..."
    rm -rf "${UUTILS_DIR}"
fi

log_info "Cloning uutils-coreutils..."
git clone --depth 1 --branch "${UUTILS_VERSION}" https://github.com/uutils/coreutils.git "${UUTILS_DIR}"

cd "${UUTILS_DIR}"

# Build multicall binary
# Use system oniguruma library instead of building from source
# (the bundled oniguruma fails with newer GCC)
log_info "Compiling (this may take a few minutes)..."
RUSTONIG_SYSTEM_LIBONIG=1 cargo build --release

# Create output directory
mkdir -p "${UUTILS_OUTPUT}"

# Copy the multicall binary
cp target/release/coreutils "${UUTILS_OUTPUT}/coreutils"

log_success "uutils-coreutils built successfully"
log_info "Binary: ${UUTILS_OUTPUT}/coreutils"
log_info "Size: $(du -h "${UUTILS_OUTPUT}/coreutils" | cut -f1)"

# Show available utilities
echo ""
log_info "Available utilities:"
"${UUTILS_OUTPUT}/coreutils" | head -20

echo ""
log_success "Done! Binary is at: ${UUTILS_OUTPUT}/coreutils"
