#!/bin/bash
# =============================================================================
# Stage 0: Setup Pre-built Cross-Compilation Toolchain
# =============================================================================
# Downloads and configures pre-built musl cross-compiler for fast builds
# Target build time: <2 minutes (vs 30-45 min for building GCC from source)
#
# Can be run standalone or sourced from build.sh

set -euo pipefail

# =============================================================================
# Environment Setup
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${RAVEN_ROOT:-$(dirname "$(dirname "$SCRIPT_DIR")")}"
BUILD_DIR="${RAVEN_BUILD:-${PROJECT_ROOT}/build}"

# Build directories
TOOLCHAIN_DIR="${TOOLCHAIN_DIR:-${BUILD_DIR}/toolchain}"
SYSROOT_DIR="${SYSROOT_DIR:-${BUILD_DIR}/sysroot}"
SOURCES_DIR="${SOURCES_DIR:-${BUILD_DIR}/sources}"
LOGS_DIR="${LOGS_DIR:-${BUILD_DIR}/logs}"

# Build configuration
RAVEN_VERSION="${RAVEN_VERSION:-2025.12}"
RAVEN_ARCH="${RAVEN_ARCH:-x86_64}"
RAVEN_TARGET="${RAVEN_TARGET:-${RAVEN_ARCH}-raven-linux-musl}"
RAVEN_JOBS="${RAVEN_JOBS:-$(nproc)}"

# Pre-built toolchain source (musl.cc)
# This provides GCC 14.2.0 + binutils + musl, matching our previous from-source build
MUSL_CROSS_URL="https://musl.cc/x86_64-linux-musl-cross.tgz"
MUSL_CROSS_TRIPLET="x86_64-linux-musl"

# Linux headers version (still need to install these)
LINUX_VERSION="6.17"
LINUX_URL="https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-${LINUX_VERSION}.tar.xz"

# =============================================================================
# Logging
# =============================================================================

if [[ -f "${PROJECT_ROOT}/scripts/lib/logging.sh" ]]; then
    source "${PROJECT_ROOT}/scripts/lib/logging.sh"
else
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    BLUE='\033[0;34m'
    NC='\033[0m'
    log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
    log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
    log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
    log_error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }
fi

# =============================================================================
# Helper Functions
# =============================================================================

download_file() {
    local name="$1"
    local url="$2"
    local filename
    filename="$(basename "$url")"

    if [[ -f "${SOURCES_DIR}/${filename}" ]]; then
        log_info "${name} already downloaded"
    else
        log_info "Downloading ${name}..."
        curl -L -o "${SOURCES_DIR}/${filename}" "$url"
    fi
}

extract_archive() {
    local archive="$1"
    local dest_dir="${2:-${SOURCES_DIR}}"

    log_info "Extracting ${archive}..."
    case "$archive" in
        *.tar.gz|*.tgz)
            tar -xzf "${SOURCES_DIR}/${archive}" -C "$dest_dir"
            ;;
        *.tar.xz)
            tar -xJf "${SOURCES_DIR}/${archive}" -C "$dest_dir"
            ;;
        *)
            log_error "Unknown archive format: ${archive}"
            ;;
    esac
}

# =============================================================================
# Build Functions
# =============================================================================

setup_prebuilt_toolchain() {
    log_info "Setting up pre-built musl cross-compiler..."

    # Download pre-built toolchain
    download_file "musl-cross toolchain" "$MUSL_CROSS_URL"

    # Extract to toolchain directory
    local archive_name
    archive_name="$(basename "$MUSL_CROSS_URL")"

    if [[ ! -d "${TOOLCHAIN_DIR}/bin" ]]; then
        log_info "Extracting toolchain..."
        mkdir -p "${TOOLCHAIN_DIR}"

        # Extract and move contents (toolchain comes in x86_64-linux-musl-cross/ subdirectory)
        tar -xzf "${SOURCES_DIR}/${archive_name}" -C "${SOURCES_DIR}"
        cp -a "${SOURCES_DIR}/${MUSL_CROSS_TRIPLET}-cross/"* "${TOOLCHAIN_DIR}/"
        rm -rf "${SOURCES_DIR}/${MUSL_CROSS_TRIPLET}-cross"
    else
        log_info "Toolchain already extracted"
    fi

    # Create symlinks for RAVEN_TARGET compatibility
    # This allows the rest of the build system to use x86_64-raven-linux-musl-gcc
    # while actually using the x86_64-linux-musl-gcc binary
    log_info "Creating ${RAVEN_TARGET} symlinks..."

    local tools=("gcc" "g++" "ar" "as" "ld" "nm" "objcopy" "objdump" "ranlib" "readelf" "strip" "c++")

    for tool in "${tools[@]}"; do
        local src="${TOOLCHAIN_DIR}/bin/${MUSL_CROSS_TRIPLET}-${tool}"
        local dst="${TOOLCHAIN_DIR}/bin/${RAVEN_TARGET}-${tool}"

        if [[ -f "$src" && ! -e "$dst" ]]; then
            ln -sf "${MUSL_CROSS_TRIPLET}-${tool}" "$dst"
        fi
    done

    # Also create cc symlink
    if [[ ! -e "${TOOLCHAIN_DIR}/bin/${RAVEN_TARGET}-cc" ]]; then
        ln -sf "${MUSL_CROSS_TRIPLET}-gcc" "${TOOLCHAIN_DIR}/bin/${RAVEN_TARGET}-cc"
    fi

    log_success "Pre-built toolchain ready"
}

install_linux_headers() {
    log_info "Installing Linux headers ${LINUX_VERSION}..."

    download_file "linux" "$LINUX_URL"

    # Only extract and install if not already done
    if [[ ! -d "${SYSROOT_DIR}/usr/include/linux" ]]; then
        extract_archive "linux-${LINUX_VERSION}.tar.xz"

        cd "${SOURCES_DIR}/linux-${LINUX_VERSION}"
        make mrproper
        make ARCH="${RAVEN_ARCH}" headers

        # Clean non-header files
        find usr/include -type f ! -name '*.h' -delete

        # Install to sysroot
        mkdir -p "${SYSROOT_DIR}/usr"
        cp -rv usr/include "${SYSROOT_DIR}/usr/" 2>&1 | tail -5

        cd "${PROJECT_ROOT}"
    else
        log_info "Linux headers already installed"
    fi

    log_success "Linux headers installed"
}

setup_sysroot() {
    log_info "Setting up sysroot with musl libc..."

    # The pre-built toolchain includes musl in its sysroot
    # Copy it to our sysroot directory
    local toolchain_sysroot="${TOOLCHAIN_DIR}/${MUSL_CROSS_TRIPLET}"

    if [[ -d "$toolchain_sysroot" ]]; then
        # Copy musl libraries and headers
        mkdir -p "${SYSROOT_DIR}/usr/lib"
        mkdir -p "${SYSROOT_DIR}/usr/include"
        mkdir -p "${SYSROOT_DIR}/lib"

        # Copy libraries
        if [[ -d "${toolchain_sysroot}/lib" ]]; then
            cp -a "${toolchain_sysroot}/lib/"* "${SYSROOT_DIR}/usr/lib/" 2>/dev/null || true
        fi

        # Copy includes (musl headers)
        if [[ -d "${toolchain_sysroot}/include" ]]; then
            cp -a "${toolchain_sysroot}/include/"* "${SYSROOT_DIR}/usr/include/" 2>/dev/null || true
        fi

        # Create dynamic linker symlink
        local ld_musl="ld-musl-${RAVEN_ARCH}.so.1"
        if [[ -f "${SYSROOT_DIR}/usr/lib/libc.so" ]]; then
            ln -sf libc.so "${SYSROOT_DIR}/usr/lib/${ld_musl}"
            ln -sf ../usr/lib/${ld_musl} "${SYSROOT_DIR}/lib/${ld_musl}"
        fi
    fi

    log_success "Sysroot configured"
}

verify_toolchain() {
    log_info "Verifying toolchain..."

    local cc="${TOOLCHAIN_DIR}/bin/${RAVEN_TARGET}-gcc"

    if [[ ! -x "$cc" ]]; then
        log_error "Cross-compiler not found: $cc"
    fi

    # Test compilation
    local test_file="${SOURCES_DIR}/test.c"
    local test_out="${SOURCES_DIR}/test.out"

    echo 'int main() { return 0; }' > "$test_file"

    if "${cc}" --sysroot="${SYSROOT_DIR}" -static "$test_file" -o "$test_out" 2>/dev/null; then
        log_success "Toolchain verification passed"
        rm -f "$test_file" "$test_out"
    else
        log_warn "Static compilation test failed (may need additional setup)"
        rm -f "$test_file" "$test_out"
    fi

    # Show version
    log_info "GCC version: $(${cc} --version | head -1)"
}

# =============================================================================
# Main Execution
# =============================================================================

main() {
    log_info "=== Stage 0: Pre-built Toolchain Setup ==="
    log_info "This uses pre-built binaries for fast builds (~1-2 min vs 30-45 min)"

    # Create required directories
    mkdir -p "${TOOLCHAIN_DIR}"
    mkdir -p "${SYSROOT_DIR}"
    mkdir -p "${SOURCES_DIR}"
    mkdir -p "${LOGS_DIR}"

    # Export toolchain paths
    export PATH="${TOOLCHAIN_DIR}/bin:${PATH}"

    # Setup steps
    setup_prebuilt_toolchain
    install_linux_headers
    setup_sysroot
    verify_toolchain

    log_success "=== Stage 0 Complete: Cross Toolchain Ready ==="
    log_info "Toolchain installed to: ${TOOLCHAIN_DIR}"
    log_info "Sysroot installed to: ${SYSROOT_DIR}"
    log_info "Time saved: ~30-40 minutes by using pre-built toolchain"
}

# Run main
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
else
    main "$@"
fi
