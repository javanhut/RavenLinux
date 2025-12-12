#!/bin/bash
# =============================================================================
# Stage 0: Build Cross-Compilation Toolchain
# =============================================================================
# This builds binutils, GCC, and musl libc for cross-compiling RavenLinux
#
# Can be run standalone or sourced from build.sh
# When standalone, it sets up its own environment

set -euo pipefail

# =============================================================================
# Environment Setup (with defaults for standalone execution)
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

# Package versions
BINUTILS_VERSION="2.42"
GCC_VERSION="14.2.0"
MUSL_VERSION="1.2.5"
LINUX_VERSION="6.17"
GMP_VERSION="6.3.0"
MPFR_VERSION="4.2.1"
MPC_VERSION="1.3.1"

# URLs
BINUTILS_URL="https://ftp.gnu.org/gnu/binutils/binutils-${BINUTILS_VERSION}.tar.xz"
GCC_URL="https://ftp.gnu.org/gnu/gcc/gcc-${GCC_VERSION}/gcc-${GCC_VERSION}.tar.xz"
MUSL_URL="https://musl.libc.org/releases/musl-${MUSL_VERSION}.tar.gz"
LINUX_URL="https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-${LINUX_VERSION}.tar.xz"
GMP_URL="https://ftp.gnu.org/gnu/gmp/gmp-${GMP_VERSION}.tar.xz"
MPFR_URL="https://ftp.gnu.org/gnu/mpfr/mpfr-${MPFR_VERSION}.tar.xz"
MPC_URL="https://ftp.gnu.org/gnu/mpc/mpc-${MPC_VERSION}.tar.gz"

# =============================================================================
# Logging (use shared library or define fallbacks)
# =============================================================================

if [[ -f "${PROJECT_ROOT}/scripts/lib/logging.sh" ]]; then
    source "${PROJECT_ROOT}/scripts/lib/logging.sh"
else
    # Fallback logging functions
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    BLUE='\033[0;34m'
    NC='\033[0m'
    log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
    log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
    log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
    log_error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }
    run_logged() { "$@"; }
fi

# =============================================================================
# Helper Functions
# =============================================================================

download_source() {
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

extract_source() {
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
        *.tar.bz2)
            tar -xjf "${SOURCES_DIR}/${archive}" -C "$dest_dir"
            ;;
        *)
            log_error "Unknown archive format: ${archive}"
            ;;
    esac
}

# =============================================================================
# Build Functions
# =============================================================================

download_toolchain_sources() {
    log_info "Downloading toolchain sources..."

    mkdir -p "${SOURCES_DIR}"

    download_source "binutils" "$BINUTILS_URL"
    download_source "gcc" "$GCC_URL"
    download_source "musl" "$MUSL_URL"
    download_source "linux" "$LINUX_URL"
    download_source "gmp" "$GMP_URL"
    download_source "mpfr" "$MPFR_URL"
    download_source "mpc" "$MPC_URL"

    log_success "All toolchain sources downloaded"
}

build_binutils() {
    log_info "Building binutils ${BINUTILS_VERSION}..."

    local build_dir="${SOURCES_DIR}/binutils-build"
    rm -rf "$build_dir"
    mkdir -p "$build_dir"

    extract_source "binutils-${BINUTILS_VERSION}.tar.xz"

    cd "$build_dir"
    "${SOURCES_DIR}/binutils-${BINUTILS_VERSION}/configure" \
        --prefix="${TOOLCHAIN_DIR}" \
        --target="${RAVEN_TARGET}" \
        --with-sysroot="${SYSROOT_DIR}" \
        --disable-nls \
        --disable-werror \
        --disable-multilib \
        --disable-gprofng \
        2>&1 | tee "${LOGS_DIR}/binutils-configure.log"

    make -j"${RAVEN_JOBS}" 2>&1 | tee "${LOGS_DIR}/binutils-build.log"
    make install 2>&1 | tee "${LOGS_DIR}/binutils-install.log"

    cd "${PROJECT_ROOT}"
    rm -rf "$build_dir"

    log_success "binutils built successfully"
}

install_linux_headers() {
    log_info "Installing Linux headers ${LINUX_VERSION}..."

    extract_source "linux-${LINUX_VERSION}.tar.xz"

    cd "${SOURCES_DIR}/linux-${LINUX_VERSION}"
    make mrproper
    make ARCH="${RAVEN_ARCH}" headers
    find usr/include -type f ! -name '*.h' -delete
    mkdir -p "${SYSROOT_DIR}/usr"
    cp -rv usr/include "${SYSROOT_DIR}/usr/"

    cd "${PROJECT_ROOT}"

    log_success "Linux headers installed"
}

build_gcc_stage1() {
    log_info "Building GCC ${GCC_VERSION} (Stage 1 - C only)..."

    local build_dir="${SOURCES_DIR}/gcc-build-stage1"
    rm -rf "$build_dir"
    mkdir -p "$build_dir"

    extract_source "gcc-${GCC_VERSION}.tar.xz"

    # Extract and link GCC dependencies
    extract_source "gmp-${GMP_VERSION}.tar.xz"
    extract_source "mpfr-${MPFR_VERSION}.tar.xz"
    extract_source "mpc-${MPC_VERSION}.tar.gz"

    # Move dependencies into GCC source tree
    rm -rf "${SOURCES_DIR}/gcc-${GCC_VERSION}/gmp"
    rm -rf "${SOURCES_DIR}/gcc-${GCC_VERSION}/mpfr"
    rm -rf "${SOURCES_DIR}/gcc-${GCC_VERSION}/mpc"
    mv "${SOURCES_DIR}/gmp-${GMP_VERSION}" "${SOURCES_DIR}/gcc-${GCC_VERSION}/gmp"
    mv "${SOURCES_DIR}/mpfr-${MPFR_VERSION}" "${SOURCES_DIR}/gcc-${GCC_VERSION}/mpfr"
    mv "${SOURCES_DIR}/mpc-${MPC_VERSION}" "${SOURCES_DIR}/gcc-${GCC_VERSION}/mpc"

    cd "$build_dir"
    "${SOURCES_DIR}/gcc-${GCC_VERSION}/configure" \
        --prefix="${TOOLCHAIN_DIR}" \
        --target="${RAVEN_TARGET}" \
        --with-sysroot="${SYSROOT_DIR}" \
        --with-newlib \
        --without-headers \
        --disable-nls \
        --disable-shared \
        --disable-multilib \
        --disable-threads \
        --disable-libatomic \
        --disable-libgomp \
        --disable-libquadmath \
        --disable-libssp \
        --disable-libvtv \
        --disable-libstdcxx \
        --enable-languages=c \
        2>&1 | tee "${LOGS_DIR}/gcc-stage1-configure.log"

    make -j"${RAVEN_JOBS}" all-gcc all-target-libgcc 2>&1 | tee "${LOGS_DIR}/gcc-stage1-build.log"
    make install-gcc install-target-libgcc 2>&1 | tee "${LOGS_DIR}/gcc-stage1-install.log"

    cd "${PROJECT_ROOT}"
    rm -rf "$build_dir"

    log_success "GCC Stage 1 built successfully"
}

build_musl() {
    log_info "Building musl ${MUSL_VERSION}..."

    extract_source "musl-${MUSL_VERSION}.tar.gz"

    cd "${SOURCES_DIR}/musl-${MUSL_VERSION}"

    CC="${RAVEN_TARGET}-gcc" \
    CROSS_COMPILE="${RAVEN_TARGET}-" \
    ./configure \
        --prefix=/usr \
        --target="${RAVEN_TARGET}" \
        2>&1 | tee "${LOGS_DIR}/musl-configure.log"

    make -j"${RAVEN_JOBS}" 2>&1 | tee "${LOGS_DIR}/musl-build.log"
    make DESTDIR="${SYSROOT_DIR}" install 2>&1 | tee "${LOGS_DIR}/musl-install.log"

    # Create necessary symlinks
    ln -sf libc.so "${SYSROOT_DIR}/usr/lib/ld-musl-${RAVEN_ARCH}.so.1"
    mkdir -p "${SYSROOT_DIR}/lib"
    ln -sf ../usr/lib/ld-musl-${RAVEN_ARCH}.so.1 "${SYSROOT_DIR}/lib/ld-musl-${RAVEN_ARCH}.so.1"

    cd "${PROJECT_ROOT}"

    log_success "musl built successfully"
}

build_gcc_stage2() {
    log_info "Building GCC ${GCC_VERSION} (Stage 2 - Full)..."

    local build_dir="${SOURCES_DIR}/gcc-build-stage2"
    rm -rf "$build_dir"
    mkdir -p "$build_dir"

    cd "$build_dir"
    "${SOURCES_DIR}/gcc-${GCC_VERSION}/configure" \
        --prefix="${TOOLCHAIN_DIR}" \
        --target="${RAVEN_TARGET}" \
        --with-sysroot="${SYSROOT_DIR}" \
        --disable-nls \
        --disable-multilib \
        --enable-languages=c,c++ \
        --enable-default-pie \
        --enable-default-ssp \
        2>&1 | tee "${LOGS_DIR}/gcc-stage2-configure.log"

    make -j"${RAVEN_JOBS}" 2>&1 | tee "${LOGS_DIR}/gcc-stage2-build.log"
    make install 2>&1 | tee "${LOGS_DIR}/gcc-stage2-install.log"

    cd "${PROJECT_ROOT}"
    rm -rf "$build_dir"

    log_success "GCC Stage 2 built successfully"
}

# =============================================================================
# Main Execution
# =============================================================================

main() {
    # Create required directories
    mkdir -p "${TOOLCHAIN_DIR}"
    mkdir -p "${SYSROOT_DIR}"
    mkdir -p "${SOURCES_DIR}"
    mkdir -p "${LOGS_DIR}"

    # Export toolchain paths for the build
    export PATH="${TOOLCHAIN_DIR}/bin:${PATH}"

    download_toolchain_sources
    build_binutils
    install_linux_headers
    build_gcc_stage1
    build_musl
    build_gcc_stage2

    log_success "=== Stage 0 Complete: Cross Toolchain Ready ==="
    log_info "Toolchain installed to: ${TOOLCHAIN_DIR}"
    log_info "Sysroot installed to: ${SYSROOT_DIR}"
}

# Run main if executed directly (not sourced)
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
else
    # When sourced, just run the build steps
    main "$@"
fi
