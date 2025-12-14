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
# Build raven-compositor (Wayland compositor)
# =============================================================================
build_compositor() {
    log_step "Building raven-compositor..."

    local compositor_dir="${PROJECT_ROOT}/desktop/compositor"
    local vendor_dir="${BUILD_DIR}/vendor/raven-compositor"
    local require_compositor="${RAVEN_REQUIRE_COMPOSITOR:-1}"
    local build_log="${LOGS_DIR}/raven-compositor.log"
    local vendor_log="${LOGS_DIR}/raven-compositor-vendor.log"

    if [[ ! -d "${compositor_dir}" ]]; then
        log_warn "raven-compositor source not found"
        return 0
    fi

    if ! command -v cargo &>/dev/null; then
        log_warn "Cargo not found, skipping raven-compositor build"
        [[ "${require_compositor}" == "1" ]] && return 1 || return 0
    fi

    cd "${compositor_dir}"

    vendor_compositor() {
        rm -rf "${vendor_dir}"
        mkdir -p "${vendor_dir}" 2>/dev/null || true

        log_info "Vendoring Rust dependencies (raven-compositor)..."
        if cargo vendor --locked --offline "${vendor_dir}" > "${vendor_dir}/cargo-config.toml" 2>&1 | tee "${vendor_log}"; then
            touch "${vendor_dir}/.cargo-vendor-ok" 2>/dev/null || true
            log_success "Dependencies vendored -> ${vendor_dir}"
            return 0
        fi

        log_warn "Offline vendoring failed; retrying with network..."
        if cargo vendor --locked "${vendor_dir}" > "${vendor_dir}/cargo-config.toml" 2>&1 | tee "${vendor_log}"; then
            touch "${vendor_dir}/.cargo-vendor-ok" 2>/dev/null || true
            log_success "Dependencies vendored -> ${vendor_dir}"
            return 0
        fi

        return 1
    }

    # Preflight native deps (keep aligned with `desktop/compositor/Cargo.toml`).
    if command -v pkg-config &>/dev/null; then
        local missing=()
        local pcs=(
            libdrm
            wayland-client
            wayland-server
            xkbcommon
        )
        for pc in "${pcs[@]}"; do
            if ! pkg-config --exists "${pc}" 2>/dev/null; then
                missing+=("${pc}")
            fi
        done
        if [[ ${#missing[@]} -gt 0 ]]; then
            log_error "Missing system libraries for raven-compositor: ${missing[*]}"
            log_error "Install the -dev packages for these (and ensure pkg-config can find them)."
            [[ "${require_compositor}" == "1" ]] && return 1 || return 0
        fi
    else
        log_warn "pkg-config not found; raven-compositor may fail to build due to missing system libs"
    fi

    # Ensure a lockfile exists (required for `--locked`/vendoring).
    if [[ ! -f Cargo.lock ]]; then
        log_info "Generating Cargo.lock (raven-compositor)..."
        if cargo generate-lockfile 2>&1 | tee "${LOGS_DIR}/raven-compositor-lock.log"; then
            log_success "Generated Cargo.lock"
        else
            log_warn "Failed to generate Cargo.lock (likely no network)"
            [[ "${require_compositor}" == "1" ]] && return 1 || return 0
        fi
    fi

    # Prefer vendored deps (offline/reproducible). If not present, try to create them.
    if [[ ! -f "${vendor_dir}/.cargo-vendor-ok" ]]; then
        if ! vendor_compositor; then
            log_warn "Failed to vendor deps (likely no network); attempting build with existing cache..."
            [[ "${require_compositor}" == "1" ]] && return 1 || true
        fi
    fi

    if CARGO_TARGET_DIR=target-user cargo build --release --locked --offline \
        --config "source.crates-io.replace-with=\"vendored-sources\"" \
        --config "source.vendored-sources.directory=\"${vendor_dir}\"" \
        2>&1 | tee "${build_log}"; then
        mkdir -p "${PACKAGES_DIR}/bin" "${SYSROOT_DIR}/bin"
        cp target-user/release/raven-compositor "${PACKAGES_DIR}/bin/"
        cp target-user/release/raven-compositor "${SYSROOT_DIR}/bin/"
        log_success "raven-compositor built"
    else
        # If vendored sources were modified/corrupted, re-vendor and retry once.
        if rg -n "listed checksum of .* has changed" "${build_log}" >/dev/null 2>&1; then
            log_warn "Vendored sources checksum mismatch detected; re-vendoring and retrying..."
            if vendor_compositor; then
                if CARGO_TARGET_DIR=target-user cargo build --release --locked --offline \
                    --config "source.crates-io.replace-with=\"vendored-sources\"" \
                    --config "source.vendored-sources.directory=\"${vendor_dir}\"" \
                    2>&1 | tee "${build_log}"; then
                    mkdir -p "${PACKAGES_DIR}/bin" "${SYSROOT_DIR}/bin"
                    cp target-user/release/raven-compositor "${PACKAGES_DIR}/bin/"
                    cp target-user/release/raven-compositor "${SYSROOT_DIR}/bin/"
                    log_success "raven-compositor built"
                    cd "${PROJECT_ROOT}"
                    return 0
                fi
            fi
        fi

        log_warn "Failed to build raven-compositor (Wayland entry will fall back to shell)"
        log_warn "See: ${build_log}"
        [[ "${require_compositor}" == "1" ]] && return 1 || return 0
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
# Build WiFi tools (Go)
# =============================================================================
build_wifi_tools() {
    log_step "Building WiFi tools..."

    # Build wifi TUI
    local wifi_tui_dir="${PROJECT_ROOT}/tools/raven-wifi-tui"
    if [[ -d "${wifi_tui_dir}" ]]; then
        if ! command -v go &>/dev/null; then
            log_warn "Go not found, skipping wifi TUI build"
        else
            cd "${wifi_tui_dir}"
            go mod tidy 2>/dev/null || true

            if CGO_ENABLED=0 go build -o wifi . 2>&1 | tee "${LOGS_DIR}/wifi-tui.log"; then
                mkdir -p "${PACKAGES_DIR}/bin" "${SYSROOT_DIR}/bin"
                cp wifi "${PACKAGES_DIR}/bin/"
                cp wifi "${SYSROOT_DIR}/bin/"
                log_success "wifi (TUI) built"
            else
                log_warn "Failed to build wifi TUI"
            fi
            cd "${PROJECT_ROOT}"
        fi
    else
        log_warn "WiFi TUI source not found at ${wifi_tui_dir}"
    fi

    # Build raven-wifi GUI
    local wifi_gui_dir="${PROJECT_ROOT}/tools/raven-wifi"
    if [[ -d "${wifi_gui_dir}" ]]; then
        if ! command -v go &>/dev/null; then
            log_warn "Go not found, skipping raven-wifi GUI build"
        else
            cd "${wifi_gui_dir}"
            go mod tidy 2>/dev/null || true

            if CGO_ENABLED=1 go build -o raven-wifi . 2>&1 | tee "${LOGS_DIR}/wifi-gui.log"; then
                mkdir -p "${PACKAGES_DIR}/bin" "${SYSROOT_DIR}/bin"
                cp raven-wifi "${PACKAGES_DIR}/bin/"
                cp raven-wifi "${SYSROOT_DIR}/bin/"
                log_success "raven-wifi (GUI) built"
            else
                log_warn "Failed to build raven-wifi GUI"
            fi
            cd "${PROJECT_ROOT}"
        fi
    else
        log_warn "WiFi GUI source not found at ${wifi_gui_dir}"
    fi
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

    local packages=(vem carrion ivaldi rvn raven-compositor raven-installer raven-usb)
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
    build_compositor
    build_installer
    build_usb_creator
    build_wifi_tools
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
