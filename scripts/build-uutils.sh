#!/bin/bash
# =============================================================================
# RavenLinux uutils-coreutils Build Script
# =============================================================================
# Build uutils-coreutils from source for RavenLinux
#
# Usage: ./scripts/build-uutils.sh [OPTIONS]
#
# Options:
#   --no-log    Disable file logging
#   --clean     Clean and rebuild from scratch

set -euo pipefail

# =============================================================================
# Configuration
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export RAVEN_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
export RAVEN_BUILD="${RAVEN_ROOT}/build"
UUTILS_VERSION="0.0.28"
UUTILS_DIR="${RAVEN_BUILD}/uutils-coreutils"
UUTILS_OUTPUT="${RAVEN_BUILD}/bin"

# Source shared logging library
source "${SCRIPT_DIR}/lib/logging.sh"

# Options
CLEAN=false

# =============================================================================
# Argument Parsing
# =============================================================================

while [[ $# -gt 0 ]]; do
    case "$1" in
        --no-log)
            export RAVEN_NO_LOG=1
            shift
            ;;
        --clean)
            CLEAN=true
            shift
            ;;
        *)
            log_error "Unknown option: $1"
            echo "Usage: $0 [--no-log] [--clean]"
            exit 1
            ;;
    esac
done

# =============================================================================
# Main
# =============================================================================

main() {
    # Initialize logging
    init_logging "build-uutils" "uutils-coreutils Build"
    enable_logging_trap

    log_section "Building uutils-coreutils ${UUTILS_VERSION}"

    # Check for Rust
    if ! command -v cargo &>/dev/null; then
        log_fatal "Rust/Cargo not found. Install with: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    fi

    log_info "Rust version: $(cargo --version)"

    mkdir -p "${RAVEN_BUILD}"
    cd "${RAVEN_BUILD}"

    # Clean if requested or remove old source
    if [[ "$CLEAN" == "true" ]] || [[ -d "${UUTILS_DIR}" ]]; then
        log_info "Removing old uutils source..."
        rm -rf "${UUTILS_DIR}"
    fi

    log_step "Cloning uutils-coreutils..."
    run_logged git clone --depth 1 --branch "${UUTILS_VERSION}" https://github.com/uutils/coreutils.git "${UUTILS_DIR}"

    cd "${UUTILS_DIR}"

    # Build multicall binary
    # Use system oniguruma library instead of building from source
    # (the bundled oniguruma fails with newer GCC)
    log_step "Compiling (this may take a few minutes)..."
    if ! run_logged env RUSTONIG_SYSTEM_LIBONIG=1 cargo build --release; then
        log_fatal "Failed to build uutils-coreutils"
    fi

    # Create output directory
    mkdir -p "${UUTILS_OUTPUT}"

    # Copy the multicall binary
    cp target/release/coreutils "${UUTILS_OUTPUT}/coreutils"

    log_success "uutils-coreutils built successfully"
    log_info "Binary: ${UUTILS_OUTPUT}/coreutils"
    log_info "Size: $(du -h "${UUTILS_OUTPUT}/coreutils" | cut -f1)"

    # Show available utilities
    log_section "Available Utilities"
    "${UUTILS_OUTPUT}/coreutils" | head -20

    log_section "Build Complete!"

    echo "  Binary: ${UUTILS_OUTPUT}/coreutils"
    echo "  Size:   $(du -h "${UUTILS_OUTPUT}/coreutils" | cut -f1)"
    echo ""
    if is_logging_enabled; then
        echo "  Log:    $(get_log_file)"
        echo ""
    fi

    finalize_logging 0
}

main "$@"
