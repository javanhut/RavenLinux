#!/bin/bash
# =============================================================================
# RavenLinux Stage 3: Build Packages
# =============================================================================
# Builds all RavenLinux custom packages:
# - Vem (text editor)
# - Carrion (programming language)
# - Ivaldi (version control)
# - rvn (package manager)
# - raven-installer (GUI installer)
# - raven-usb (USB creator)
# - RavenBoot (bootloader)

set -euo pipefail

# =============================================================================
# Environment Setup (with defaults for standalone execution)
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${RAVEN_ROOT:-$(dirname "$(dirname "$SCRIPT_DIR")")}"
BUILD_DIR="${RAVEN_BUILD:-${PROJECT_ROOT}/build}"
SYSROOT_DIR="${SYSROOT_DIR:-${BUILD_DIR}/sysroot}"
PACKAGES_DIR="${PACKAGES_DIR:-${BUILD_DIR}/packages}"
LOGS_DIR="${LOGS_DIR:-${BUILD_DIR}/logs}"
TOOLS_DIR="${PROJECT_ROOT}/tools"

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
    CYAN='\033[0;36m'
    NC='\033[0m'
    log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
    log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
    log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
    log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
    log_step() { echo -e "${CYAN}[STEP]${NC} $1"; }
fi

# =============================================================================
# Build Go packages (Vem, Carrion, Ivaldi)
# =============================================================================
build_go_packages() {
    log_step "Building Go packages..."

    if [[ -x "${PROJECT_ROOT}/scripts/build-packages.sh" ]]; then
        "${PROJECT_ROOT}/scripts/build-packages.sh" all 2>&1 | tee "${LOGS_DIR}/go-packages.log"
    else
        log_warn "build-packages.sh not found"
    fi

    # Copy to sysroot
    mkdir -p "${SYSROOT_DIR}/bin"
    for pkg in vem carrion ivaldi; do
        if [[ -f "${PACKAGES_DIR}/bin/${pkg}" ]]; then
            cp "${PACKAGES_DIR}/bin/${pkg}" "${SYSROOT_DIR}/bin/"
            log_info "  Installed ${pkg}"
        fi
    done

    log_success "Go packages built"
}

# =============================================================================
# Build rvn package manager (Rust)
# =============================================================================
build_rvn() {
    log_step "Building rvn package manager..."

    local rvn_dir="${TOOLS_DIR}/rvn"

    if [[ ! -d "${rvn_dir}" ]]; then
        log_warn "rvn source not found at ${rvn_dir}"
        return 0
    fi

    if ! command -v cargo &>/dev/null; then
        log_warn "Cargo not found, skipping rvn build"
        return 0
    fi

    cd "${rvn_dir}"

    if cargo build --release 2>&1 | tee "${LOGS_DIR}/rvn.log"; then
        mkdir -p "${PACKAGES_DIR}/bin" "${SYSROOT_DIR}/bin"
        cp target/release/rvn "${PACKAGES_DIR}/bin/"
        cp target/release/rvn "${SYSROOT_DIR}/bin/"
        log_success "rvn package manager built"
    else
        log_warn "Failed to build rvn"
    fi

    cd "${PROJECT_ROOT}"
}

# =============================================================================
# Build raven-installer (Go with Gio UI)
# =============================================================================
build_installer() {
    log_step "Building raven-installer..."

    local installer_dir="${TOOLS_DIR}/raven-installer"

    if [[ ! -d "${installer_dir}" ]]; then
        log_warn "raven-installer source not found"
        return 0
    fi

    if ! command -v go &>/dev/null; then
        log_warn "Go not found, skipping installer build"
        return 0
    fi

    cd "${installer_dir}"

    # Gio UI requires CGO
    if CGO_ENABLED=1 go build -o raven-installer . 2>&1 | tee "${LOGS_DIR}/installer.log"; then
        mkdir -p "${PACKAGES_DIR}/bin" "${SYSROOT_DIR}/bin"
        cp raven-installer "${PACKAGES_DIR}/bin/"
        cp raven-installer "${SYSROOT_DIR}/bin/"
        ln -sf raven-installer "${SYSROOT_DIR}/bin/raven-install"
        log_success "raven-installer built"
    else
        log_warn "Failed to build raven-installer"
    fi

    cd "${PROJECT_ROOT}"
}

# =============================================================================
# Build raven-usb (Go with Gio UI)
# =============================================================================
build_usb_creator() {
    log_step "Building raven-usb..."

    local usb_dir="${TOOLS_DIR}/raven-usb"

    if [[ ! -d "${usb_dir}" ]]; then
        log_warn "raven-usb source not found"
        return 0
    fi

    if ! command -v go &>/dev/null; then
        log_warn "Go not found, skipping USB creator build"
        return 0
    fi

    cd "${usb_dir}"

    if CGO_ENABLED=1 go build -o raven-usb . 2>&1 | tee "${LOGS_DIR}/usb-creator.log"; then
        mkdir -p "${PACKAGES_DIR}/bin" "${SYSROOT_DIR}/bin"
        cp raven-usb "${PACKAGES_DIR}/bin/"
        cp raven-usb "${SYSROOT_DIR}/bin/"
        log_success "raven-usb built"
    else
        log_warn "Failed to build raven-usb"
    fi

    cd "${PROJECT_ROOT}"
}

# =============================================================================
# Build RavenBoot bootloader (Rust UEFI)
# =============================================================================
build_bootloader() {
    log_step "Building RavenBoot bootloader..."

    local bootloader_dir="${PROJECT_ROOT}/bootloader"

    if [[ ! -d "${bootloader_dir}" ]]; then
        log_warn "Bootloader source not found"
        return 0
    fi

    if ! command -v cargo &>/dev/null; then
        log_warn "Cargo not found, skipping bootloader build"
        return 0
    fi

    # Check for UEFI target
    if ! rustup target list --installed 2>/dev/null | grep -q "x86_64-unknown-uefi"; then
        log_info "Adding UEFI target..."
        rustup target add x86_64-unknown-uefi 2>/dev/null || {
            log_warn "Failed to add UEFI target"
            return 0
        }
    fi

    cd "${bootloader_dir}"

    if cargo build --target x86_64-unknown-uefi --release 2>&1 | tee "${LOGS_DIR}/bootloader.log"; then
        mkdir -p "${PACKAGES_DIR}/boot"
        cp target/x86_64-unknown-uefi/release/raven-boot.efi "${PACKAGES_DIR}/boot/"
        log_success "RavenBoot bootloader built"
    else
        log_warn "Failed to build bootloader"
    fi

    cd "${PROJECT_ROOT}"
}

# =============================================================================
# Summary
# =============================================================================
print_summary() {
    echo ""
    echo -e "${CYAN}=========================================="
    echo "  Package Build Summary"
    echo "==========================================${NC}"
    echo ""

    local packages=(vem carrion ivaldi rvn raven-installer raven-usb)
    for pkg in "${packages[@]}"; do
        if [[ -f "${SYSROOT_DIR}/bin/${pkg}" ]]; then
            local size
            size=$(du -h "${SYSROOT_DIR}/bin/${pkg}" | cut -f1)
            echo -e "  ${GREEN}[OK]${NC} ${pkg} (${size})"
        else
            echo -e "  ${YELLOW}[--]${NC} ${pkg} (not built)"
        fi
    done

    if [[ -f "${PACKAGES_DIR}/boot/raven-boot.efi" ]]; then
        local size
        size=$(du -h "${PACKAGES_DIR}/boot/raven-boot.efi" | cut -f1)
        echo -e "  ${GREEN}[OK]${NC} raven-boot.efi (${size})"
    else
        echo -e "  ${YELLOW}[--]${NC} raven-boot.efi (not built)"
    fi

    echo ""
}

# =============================================================================
# Main
# =============================================================================
main() {
    echo ""
    echo "=========================================="
    echo "  Stage 3: Building Packages"
    echo "=========================================="
    echo ""

    mkdir -p "${LOGS_DIR}" "${PACKAGES_DIR}/bin" "${PACKAGES_DIR}/boot"

    build_go_packages
    build_rvn
    build_installer
    build_usb_creator
    build_bootloader

    print_summary

    log_success "Stage 3 complete!"
    echo ""
}

# Run main (whether executed directly or sourced)
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
else
    main "$@"
fi
