#!/bin/bash
# =============================================================================
# RavenLinux Build System
# =============================================================================
# Main build orchestration script
#
# Usage: ./scripts/build.sh [OPTIONS] [STAGE]
#
# Options:
#   -j, --jobs N    Number of parallel jobs (default: nproc)
#   -a, --arch ARCH Target architecture (default: x86_64)
#   -c, --clean     Clean build directory before building
#   --no-log        Disable file logging
#   -h, --help      Show this help message
#
# Stages:
#   all      Build everything (default)
#   stage0   Build cross-compilation toolchain
#   stage1   Build base system with cross toolchain
#   stage2   Native rebuild of entire system
#   stage3   Build additional packages
#   stage4   Generate bootable ISO image

set -euo pipefail

# =============================================================================
# Configuration
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export RAVEN_ROOT="$(dirname "$SCRIPT_DIR")"
export RAVEN_BUILD="${RAVEN_ROOT}/build"
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
PACKAGES_DIR="${RAVEN_BUILD}/packages"

# Export all variables for stage scripts
export RAVEN_ROOT RAVEN_BUILD RAVEN_VERSION RAVEN_ARCH RAVEN_TARGET RAVEN_JOBS
export TOOLCHAIN_DIR SYSROOT_DIR STAGING_DIR SOURCES_DIR LOGS_DIR PACKAGES_DIR

# Source shared logging library
source "${SCRIPT_DIR}/lib/logging.sh"

# =============================================================================
# Functions
# =============================================================================

setup_directories() {
    log_step "Setting up build directories..."

    mkdir -p "${TOOLCHAIN_DIR}"
    mkdir -p "${SYSROOT_DIR}"
    mkdir -p "${STAGING_DIR}"
    mkdir -p "${SOURCES_DIR}"
    mkdir -p "${RAVEN_LOG_DIR}"

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
        run_logged curl -L -o "${SOURCES_DIR}/${filename}" "$url"
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
            run_logged tar -xzf "${SOURCES_DIR}/${archive}" -C "$dest_dir"
            ;;
        *.tar.xz)
            run_logged tar -xJf "${SOURCES_DIR}/${archive}" -C "$dest_dir"
            ;;
        *.tar.bz2)
            run_logged tar -xjf "${SOURCES_DIR}/${archive}" -C "$dest_dir"
            ;;
        *)
            log_fatal "Unknown archive format: ${archive}"
            ;;
    esac
}

# Build sudo-rs (sudo/su/visudo) into build/bin/
build_sudo_rs() {
    log_section "Building sudo-rs"

    if [[ -f "${RAVEN_BUILD}/bin/sudo" ]] && [[ -f "${RAVEN_BUILD}/bin/su" ]]; then
        log_info "sudo-rs already built, skipping"
        return 0
    fi

    if ! command -v cargo &>/dev/null; then
        log_warn "Cargo not found, skipping sudo-rs build"
        return 0
    fi
    if ! command -v git &>/dev/null; then
        log_warn "git not found, skipping sudo-rs build"
        return 0
    fi

    local repo="https://github.com/trifectatechfoundation/sudo-rs.git"
    local src_dir="${SOURCES_DIR}/sudo-rs"
    local commit="11af1a320d5c447e2c36ad9a0c14c6c7c638d3fc"

    mkdir -p "${SOURCES_DIR}" "${RAVEN_BUILD}/bin"

    if [[ -d "${src_dir}/.git" ]]; then
        log_step "Updating sudo-rs source..."
        (cd "${src_dir}" && run_logged git fetch --depth 1 origin "${commit}" && run_logged git reset --hard "${commit}")
    else
        log_step "Cloning sudo-rs source..."
        rm -rf "${src_dir}"
        run_logged git clone --depth 1 "${repo}" "${src_dir}"
        (cd "${src_dir}" && run_logged git fetch --depth 1 origin "${commit}" && run_logged git reset --hard "${commit}")
    fi

    log_step "Compiling sudo-rs (release)..."
    (cd "${src_dir}" && run_logged cargo build --release)

    for bin in sudo su visudo; do
        if [[ -f "${src_dir}/target/release/${bin}" ]]; then
            cp "${src_dir}/target/release/${bin}" "${RAVEN_BUILD}/bin/${bin}"
            chmod +x "${RAVEN_BUILD}/bin/${bin}"
            log_info "  Built ${RAVEN_BUILD}/bin/${bin}"
        else
            log_warn "Expected sudo-rs binary missing: ${bin}"
        fi
    done
}

# Stage 0: Build cross-compilation toolchain
build_stage0() {
    log_section "Stage 0: Building Cross Toolchain"

    # This builds binutils, gcc, and musl for cross-compilation
    if [[ -f "${RAVEN_ROOT}/scripts/stages/stage0-toolchain.sh" ]]; then
        run_logged source "${RAVEN_ROOT}/scripts/stages/stage0-toolchain.sh"
    else
        log_warn "Stage 0 script not found, skipping"
    fi
}

# Stage 1: Build base system with cross toolchain
build_stage1() {
    log_section "Stage 1: Building Base System (Cross)"

    if [[ -f "${RAVEN_ROOT}/scripts/stages/stage1-base.sh" ]]; then
        build_sudo_rs
        run_logged source "${RAVEN_ROOT}/scripts/stages/stage1-base.sh"
    else
        log_warn "Stage 1 script not found, skipping"
    fi
}

# Stage 2: Native rebuild
build_stage2() {
    log_section "Stage 2: Native Rebuild"

    if [[ -f "${RAVEN_ROOT}/scripts/stages/stage2-native.sh" ]]; then
        run_logged source "${RAVEN_ROOT}/scripts/stages/stage2-native.sh"
    else
        log_warn "Stage 2 script not found, skipping"
    fi
}

# Stage 3: Build additional packages
build_stage3() {
    log_section "Stage 3: Building Packages"

    if [[ -f "${RAVEN_ROOT}/scripts/stages/stage3-packages.sh" ]]; then
        run_logged source "${RAVEN_ROOT}/scripts/stages/stage3-packages.sh"
    else
        log_warn "Stage 3 script not found, skipping"
    fi
}

# Stage 4: Generate ISO
build_stage4() {
    log_section "Stage 4: Generating ISO"

    if [[ -f "${RAVEN_ROOT}/scripts/stages/stage4-iso.sh" ]]; then
        run_logged source "${RAVEN_ROOT}/scripts/stages/stage4-iso.sh"
    else
        log_warn "Stage 4 script not found, skipping"
    fi
}

show_help() {
    cat << EOF
RavenLinux Build System v${RAVEN_VERSION}

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
    --no-log        Disable file logging
    -h, --help      Show this help message

Environment Variables:
    RAVEN_ARCH      Target architecture
    RAVEN_JOBS      Number of parallel build jobs
    RAVEN_VERSION   Distribution version string
    RAVEN_NO_LOG    Set to "1" to disable logging

Log Files:
    Build logs are saved to: ${RAVEN_BUILD}/logs/

Examples:
    $(basename "$0")                    # Build everything
    $(basename "$0") stage0             # Build only toolchain
    $(basename "$0") -j 8 stage1        # Build stage1 with 8 jobs
    $(basename "$0") --clean all        # Clean build from scratch
    $(basename "$0") --no-log stage4    # Build ISO without logging
EOF
}

clean_build() {
    log_warn "Cleaning build directory..."
    rm -rf "${RAVEN_BUILD}"
    log_success "Build directory cleaned"
}

# =============================================================================
# Main
# =============================================================================

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
            --no-log)
                export RAVEN_NO_LOG=1
                shift
                ;;
            all|stage0|stage1|stage2|stage3|stage4)
                stage="$1"
                shift
                ;;
            *)
                log_error "Unknown option: $1"
                show_help
                exit 1
                ;;
        esac
    done

    # Initialize logging
    init_logging "build" "RavenLinux Full Build - Stage: ${stage}"
    enable_logging_trap

    log_section "RavenLinux Build System v${RAVEN_VERSION}"

    echo "  Architecture: ${RAVEN_ARCH}"
    echo "  Target:       ${RAVEN_TARGET}"
    echo "  Jobs:         ${RAVEN_JOBS}"
    if is_logging_enabled; then
        echo "  Log File:     $(get_log_file)"
    fi
    echo ""

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

    log_section "Build Complete!"

    echo "  Stage:    ${stage}"
    echo "  Duration: $(format_duration $(($(date +%s) - _RAVEN_BUILD_START_TIME)))"
    if is_logging_enabled; then
        echo "  Log:      $(get_log_file)"
    fi
    echo ""

    finalize_logging 0
}

main "$@"
