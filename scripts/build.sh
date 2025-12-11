#!/bin/bash
# RavenLinux Build System
# Main build orchestration script

set -euo pipefail

RAVEN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RAVEN_BUILD="${RAVEN_ROOT}/build"
RAVEN_PACKAGES="${RAVEN_ROOT}/packages"
RAVEN_TOOLS="${RAVEN_ROOT}/tools"
RAVEN_CONFIGS="${RAVEN_ROOT}/configs"

# Build configuration
export RAVEN_VERSION="2025.12"
export RAVEN_ARCH="${RAVEN_ARCH:-x86_64}"
export RAVEN_TARGET="${RAVEN_ARCH}-raven-linux-musl"
export RAVEN_JOBS="${RAVEN_JOBS:-$(nproc)}"

# Directory structure
TOOLCHAIN_DIR="${RAVEN_BUILD}/toolchain"
SYSROOT_DIR="${RAVEN_BUILD}/sysroot"
STAGING_DIR="${RAVEN_BUILD}/staging"
SOURCES_DIR="${RAVEN_BUILD}/sources"
LOGS_DIR="${RAVEN_BUILD}/logs"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

setup_directories() {
    log_info "Setting up build directories..."
    mkdir -p "${TOOLCHAIN_DIR}"
    mkdir -p "${SYSROOT_DIR}"
    mkdir -p "${STAGING_DIR}"
    mkdir -p "${SOURCES_DIR}"
    mkdir -p "${LOGS_DIR}"

    # Create sysroot directory structure
    mkdir -p "${SYSROOT_DIR}"/{bin,boot,dev,etc,home,lib,mnt,opt,proc,root,run,sbin,sys,tmp,usr,var}
    mkdir -p "${SYSROOT_DIR}"/usr/{bin,include,lib,share,src}
    mkdir -p "${SYSROOT_DIR}"/var/{cache,lib,log,tmp}

    log_success "Build directories created"
}

download_source() {
    local name="$1"
    local url="$2"
    local filename="${3:-$(basename "$url")}"

    if [[ ! -f "${SOURCES_DIR}/${filename}" ]]; then
        log_info "Downloading ${name}..."
        curl -L -o "${SOURCES_DIR}/${filename}" "$url"
    else
        log_info "${name} already downloaded"
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

# Stage 0: Build cross-compilation toolchain
build_stage0() {
    log_info "=== Stage 0: Building Cross Toolchain ==="

    # This builds binutils, gcc, and musl for cross-compilation
    source "${RAVEN_ROOT}/scripts/stages/stage0-toolchain.sh"
}

# Stage 1: Build base system with cross toolchain
build_stage1() {
    log_info "=== Stage 1: Building Base System (Cross) ==="

    source "${RAVEN_ROOT}/scripts/stages/stage1-base.sh"
}

# Stage 2: Native rebuild
build_stage2() {
    log_info "=== Stage 2: Native Rebuild ==="

    source "${RAVEN_ROOT}/scripts/stages/stage2-native.sh"
}

# Stage 3: Build additional packages
build_stage3() {
    log_info "=== Stage 3: Building Packages ==="

    source "${RAVEN_ROOT}/scripts/stages/stage3-packages.sh"
}

# Stage 4: Generate ISO
build_stage4() {
    log_info "=== Stage 4: Generating ISO ==="

    source "${RAVEN_ROOT}/scripts/stages/stage4-iso.sh"
}

show_help() {
    cat << EOF
RavenLinux Build System

Usage: $(basename "$0") [OPTIONS] [STAGE]

Stages:
    all         Build everything (default)
    stage0      Build cross-compilation toolchain
    stage1      Build base system with cross toolchain
    stage2      Native rebuild of entire system
    stage3      Build additional packages
    stage4      Generate bootable ISO image

Options:
    -j, --jobs N    Number of parallel jobs (default: $(nproc))
    -a, --arch ARCH Target architecture (default: x86_64)
    -c, --clean     Clean build directory before building
    -h, --help      Show this help message

Environment Variables:
    RAVEN_ARCH      Target architecture
    RAVEN_JOBS      Number of parallel build jobs
    RAVEN_VERSION   Distribution version string

Examples:
    $(basename "$0")                    # Build everything
    $(basename "$0") stage0             # Build only toolchain
    $(basename "$0") -j 8 stage1        # Build stage1 with 8 jobs
    $(basename "$0") --clean all        # Clean build from scratch
EOF
}

clean_build() {
    log_warn "Cleaning build directory..."
    rm -rf "${RAVEN_BUILD}"
    log_success "Build directory cleaned"
}

main() {
    local stage="all"
    local clean=false

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -h|--help)
                show_help
                exit 0
                ;;
            -j|--jobs)
                RAVEN_JOBS="$2"
                shift 2
                ;;
            -a|--arch)
                RAVEN_ARCH="$2"
                RAVEN_TARGET="${RAVEN_ARCH}-raven-linux-musl"
                shift 2
                ;;
            -c|--clean)
                clean=true
                shift
                ;;
            all|stage0|stage1|stage2|stage3|stage4)
                stage="$1"
                shift
                ;;
            *)
                log_error "Unknown option: $1"
                ;;
        esac
    done

    echo "========================================"
    echo "  RavenLinux Build System v${RAVEN_VERSION}"
    echo "========================================"
    echo "  Architecture: ${RAVEN_ARCH}"
    echo "  Target:       ${RAVEN_TARGET}"
    echo "  Jobs:         ${RAVEN_JOBS}"
    echo "========================================"
    echo

    if $clean; then
        clean_build
    fi

    setup_directories

    case "$stage" in
        all)
            build_stage0
            build_stage1
            build_stage2
            build_stage3
            build_stage4
            ;;
        stage0)
            build_stage0
            ;;
        stage1)
            build_stage1
            ;;
        stage2)
            build_stage2
            ;;
        stage3)
            build_stage3
            ;;
        stage4)
            build_stage4
            ;;
    esac

    log_success "Build complete!"
}

main "$@"
