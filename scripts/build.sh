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
#   --check-deps    Check and install missing dependencies before building
#   --no-log        Disable file logging
#   -h, --help      Show this help message
#
# Note: 
#   - Automatically checks for missing dependencies and offers to install them
#   - If build directory exists with root ownership, prompts for sudo to fix
#
# Stages:
#   all      Build everything (default)
#   stage0   Build cross-compilation toolchain
#   stage1   Build base system with cross toolchain
#   stage2   Native rebuild of entire system
#   stage3   Build base packages (core libraries, shells, OpenSSH)
#   raven    Build the Raven self-hosted toolchain (shell, rvn, ivaldi, ...)
#   gui      Build the compositor and desktop shell (huginn, muninn)
#   stage4   Generate bootable ISO image

set -euo pipefail

# =============================================================================
# Configuration
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export RAVEN_ROOT="$(dirname "$SCRIPT_DIR")"
export RAVEN_BUILD="${RAVEN_ROOT}/build"
RAVEN_PACKAGES="${RAVEN_ROOT}/packages"
RAVEN_CONFIGS="${RAVEN_ROOT}/configs"

# Build configuration
export RAVEN_VERSION="2026.08"
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

# Check for required build dependencies
check_build_dependencies() {
    if [[ "${SKIP_DEP_CHECK:-false}" == "true" ]]; then
        return 0
    fi
    
    if [[ -f "${SCRIPT_DIR}/check-deps.sh" ]]; then
        # Run in quiet mode first to check if anything is missing
        local missing
        missing=$("${SCRIPT_DIR}/check-deps.sh" -q 2>&1) || true
        
        if [[ -n "$missing" ]]; then
            echo ""
            echo "Some build dependencies are missing."
            echo ""
            # Run interactively to show details and offer installation
            "${SCRIPT_DIR}/check-deps.sh"
            local result=$?
            if [[ $result -ne 0 ]]; then
                echo ""
                echo "Cannot proceed without required dependencies."
                echo "Please install them and try again."
                exit 1
            fi
        fi
    fi
}

# Verify the Rust toolchain can actually execute before starting a long build.
#
# RavenLinux is x86_64-only, so on an arm64 host (e.g. Apple Silicon) the amd64
# build image runs under qemu-user emulation. amd64 rustc/LLVM segfaults under
# qemu-user, so every Rust compile in the build (sudo-rs, uutils-coreutils)
# would crash the same way — but only minutes into the build,
# deep inside a stage, with a cryptic "qemu: uncaught target signal 11" and
# leftover core dumps. Detect it here and fail fast with actionable guidance.
check_rust_toolchain() {
    if [[ "${RAVEN_SKIP_RUST_CHECK:-false}" == "true" ]]; then
        return 0
    fi

    # If rustc is absent the dependency check already covers it; nothing to probe.
    command -v rustc &>/dev/null || return 0

    # A trivial invocation is enough: under qemu-user rustc crashes at startup
    # (SIGSEGV -> exit 139), long before it does any real work. Run it in a
    # subshell with core dumps disabled so the probe itself never litters a
    # *.core file into the working directory when it crashes.
    local rc=0
    ( ulimit -c 0 2>/dev/null; rustc --version &>/dev/null ) || rc=$?
    if [[ $rc -eq 0 ]]; then
        return 0
    fi

    log_error "The Rust compiler (rustc) cannot execute in this environment."
    echo ""
    echo "  'rustc --version' exited with status ${rc} (137/139 means it crashed)."
    echo ""
    echo "  This almost always means the amd64 build image is running under"
    echo "  qemu-user emulation on an arm64 host (e.g. Apple Silicon). amd64"
    echo "  rustc/LLVM segfaults under qemu-user, so every Rust compile in the"
    echo "  build (sudo-rs, uutils-coreutils) would fail the same way."
    echo ""
    echo "  RavenLinux is x86_64-only, so run the build where amd64 code executes"
    echo "  natively or via a complete translator:"
    echo ""
    echo "    - Build on a real x86_64 host (Linux PC, Intel Mac, or an x86_64"
    echo "      cloud/CI runner). Most reliable."
    echo "    - On Apple Silicon, use Docker Desktop or colima with Rosetta, which"
    echo "      translates amd64 Linux far more completely than qemu-user:"
    echo "        Docker Desktop: enable Settings > General >"
    echo "          'Use Rosetta for x86/amd64 emulation', then run:"
    echo "            RAVEN_ENGINE=docker make build"
    echo "        colima: colima start --arch x86_64 --vm-type vz --vz-rosetta"
    echo "                RAVEN_ENGINE=docker make build"
    echo ""
    echo "  podman + libkrun only offers qemu-user for amd64 on arm64 and cannot"
    echo "  run rustc; there is no in-container workaround for that engine."
    echo ""
    echo "  To skip this check anyway, set RAVEN_SKIP_RUST_CHECK=1."
    echo ""
    exit 1
}

# Check and fix build directory permissions if owned by root
fix_build_permissions() {
    local current_user
    current_user="$(id -un)"
    
    # Skip if running as root
    if [[ "$current_user" == "root" ]]; then
        return 0
    fi
    
    # Check if build directory exists and is owned by someone else
    if [[ -d "${RAVEN_BUILD}" ]]; then
        local owner
        owner="$(stat -c '%U' "${RAVEN_BUILD}" 2>/dev/null || echo "unknown")"
        
        if [[ "$owner" != "$current_user" ]]; then
            echo ""
            echo "Build directory '${RAVEN_BUILD}' is owned by '$owner'."
            echo "You are running as '$current_user'."
            echo ""
            echo "This will cause permission errors. Fixing ownership..."
            echo ""
            
            if sudo chown -R "${current_user}:$(id -gn)" "${RAVEN_BUILD}"; then
                echo "Ownership fixed successfully."
                echo ""
            else
                echo "ERROR: Failed to fix permissions. Please run manually:"
                echo "  sudo chown -R ${current_user}:$(id -gn) ${RAVEN_BUILD}"
                exit 1
            fi
        fi
    fi
}

setup_directories() {
    log_step "Setting up build directories..."

    mkdir -p "${TOOLCHAIN_DIR}"
    mkdir -p "${SYSROOT_DIR}"
    mkdir -p "${STAGING_DIR}"
    mkdir -p "${SOURCES_DIR}"
    mkdir -p "${RAVEN_LOG_DIR}"

    # Create sysroot directory structure (only if writable).
    # Some users may have an old sysroot created as root; stage4 can still run
    # from a read-only sysroot by copying it into an ISO workspace.
    if [[ -d "${SYSROOT_DIR}" ]] && [[ ! -w "${SYSROOT_DIR}" ]]; then
        log_warn "Sysroot is not writable: ${SYSROOT_DIR}"
        log_warn "Skipping sysroot directory creation. To rebuild sysroot stages, delete/chown it or set RAVEN_BUILD to a new directory."
    else
        mkdir -p "${SYSROOT_DIR}"/{bin,boot,dev,etc,home,lib,mnt,opt,proc,root,run,sbin,sys,tmp,usr,var}
        mkdir -p "${SYSROOT_DIR}"/usr/{bin,include,lib,share,src}
        mkdir -p "${SYSROOT_DIR}"/var/{cache,lib,log,tmp}
    fi

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

# Stage 3: Build base packages (shells, OpenSSH, core libraries)
build_stage3() {
    log_section "Stage 3: Building Base Packages"

    if [[ -f "${RAVEN_ROOT}/scripts/stages/stage3-packages.sh" ]]; then
        run_logged source "${RAVEN_ROOT}/scripts/stages/stage3-packages.sh"
    else
        log_warn "Stage 3 script not found, skipping"
    fi
}

# Raven stage: the self-hosted toolchain (ravenshell, rvn, poxy, ivaldi,
# crow, imlazy, oxigen). Runs after stage3 and before stage4, because stage4
# squashes the sysroot into the ISO -- anything installed later would not ship.
# Unnumbered on purpose: the base system (stages 0-4) stands without it.
build_raven() {
    log_section "Raven Stage: Self-Hosted Toolchain"

    if [[ -f "${RAVEN_ROOT}/scripts/stages/stage-raven.sh" ]]; then
        run_logged source "${RAVEN_ROOT}/scripts/stages/stage-raven.sh"
    else
        log_warn "Raven stage script not found, skipping"
    fi
}

# GUI stage: the compositor and desktop shell (huginn, muninn, muninn-lock).
# Runs after raven and before stage4. Separate from the Raven stage because
# huginn links seventeen shared libraries and the Raven layer's defining
# property is that it links none -- see the header of stage-gui.sh.
build_gui() {
    log_section "GUI Stage: Compositor and Shell"

    if [[ -f "${RAVEN_ROOT}/scripts/stages/stage-gui.sh" ]]; then
        run_logged source "${RAVEN_ROOT}/scripts/stages/stage-gui.sh"
    else
        log_warn "GUI stage script not found, skipping"
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
    stage3      Build base packages (core libraries, shells, OpenSSH)
    raven       Build the Raven self-hosted toolchain (ravenshell, rvn, poxy,
                ivaldi, crow, imlazy, oxigen) into the sysroot
    gui         Build the compositor and desktop shell (huginn, muninn,
                muninn-lock). Runs after raven, before stage4.
    stage4      Generate bootable ISO image

Options:
    -j, --jobs N    Number of parallel jobs (default: $(nproc))
    -a, --arch ARCH Target architecture (default: x86_64)
    -c, --clean     Clean build directory before building
    --skip-deps     Skip automatic dependency check
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
    $(basename "$0") raven              # Build only the Raven toolchain
    $(basename "$0") gui                # Build only the compositor and shell
    RAVEN_ONLY=crow $(basename "$0") raven   # ... or just one component
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
            --skip-deps)
                SKIP_DEP_CHECK=true
                shift
                ;;
            all|stage0|stage1|stage2|stage3|raven|gui|stage4)
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

    # Check for required dependencies first
    check_build_dependencies

    # Fail fast if rustc can't run (qemu-user emulation on arm64). Skip for the
    # pure-C cross toolchain (stage0) and ISO assembly (stage4), which build no
    # Rust; every other path compiles Rust (sudo-rs, uutils) and would crash
    # without this.
    case "$stage" in
        all|stage1|stage2|stage3|raven|gui)
            check_rust_toolchain
            ;;
    esac

    # Fix build directory permissions before anything else
    fix_build_permissions

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
            build_raven
            build_gui
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
        raven)
            build_raven
            ;;
        gui)
            build_gui
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
