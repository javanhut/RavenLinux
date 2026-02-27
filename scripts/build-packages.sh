#!/bin/bash
# =============================================================================
# RavenLinux Custom Packages Build Script
# =============================================================================
# Builds custom packages from GitHub for RavenLinux
#
# Usage: ./scripts/build-packages.sh [OPTIONS] [package-name|all]
#
# Packages (from GitHub repos):
#   compositor    Build RavenCompositor (Rust)
#   file-manager  Build RavenFileManager (Rust)
#   terminal      Build RavenTerminal (Go, CGO=1)
#   shell         Build RavenShell utils (Go, CGO=0)
#   poxy          Build Poxy (Go, CGO=0)
#   vem           Build Vem text editor (Go, CGO=1)
#   carrion       Build Carrion language (Go, CGO=0)
#   ivaldi        Build Ivaldi VCS (Go, CGO=0)
#
# Local tools (embedded in tools/):
#   installer     Build Raven Installer
#   rvn           Build rvn package manager
#   dhcp          Build raven-dhcp DHCP client
#   usb           Build USB creator tool
#   bootloader    Build RavenBoot bootloader
#   wifi          Build WiFi tools
#   launcher      Build raven-launcher
#
#   all           Build all packages (default)
#
# Options:
#   --no-log   Disable file logging

set -e

# =============================================================================
# Configuration
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
export RAVEN_ROOT="$PROJECT_ROOT"
export RAVEN_BUILD="${PROJECT_ROOT}/build"
SOURCES_DIR="${RAVEN_BUILD}/sources"
OUTPUT_DIR="${RAVEN_BUILD}/packages"

# Avoid relying on ~/.cache in restricted environments
export GOCACHE="${GOCACHE:-${RAVEN_BUILD}/.gocache}"
mkdir -p "${GOCACHE}" 2>/dev/null || true

# Source shared libraries
source "${SCRIPT_DIR}/lib/logging.sh"
source "${SCRIPT_DIR}/lib/repos.sh"

# =============================================================================
# Functions
# =============================================================================

check_dependencies() {
    log_step "Checking build dependencies..."

    local missing=()

    if ! command -v go &> /dev/null; then
        missing+=("go")
    fi

    if ! command -v git &> /dev/null; then
        missing+=("git")
    fi

    if [ ${#missing[@]} -ne 0 ]; then
        log_fatal "Missing dependencies: ${missing[*]}"
    fi

    log_success "All dependencies found (Go $(go version | awk '{print $3}'))"
}

# =============================================================================
# GitHub repo builds (via repos.sh)
# =============================================================================

build_compositor() {
    log_section "Building RavenCompositor"
    if ! command -v cargo &>/dev/null; then
        log_warn "Rust/Cargo not found, skipping compositor build"
        return 0
    fi
    fetch_repo compositor
    build_cargo_repo compositor
}

build_file_manager() {
    log_section "Building RavenFileManager"
    if ! command -v cargo &>/dev/null; then
        log_warn "Rust/Cargo not found, skipping file-manager build"
        return 0
    fi
    fetch_repo file-manager
    build_cargo_repo file-manager
}

build_terminal() {
    log_section "Building RavenTerminal"
    fetch_repo terminal
    build_go_repo terminal
}

build_shell() {
    log_section "Building RavenShell"
    fetch_repo shell
    build_go_repo shell
}

build_poxy() {
    log_section "Building Poxy"
    fetch_repo poxy
    build_go_repo poxy
}

build_vem() {
    log_section "Building Vem Text Editor"
    fetch_repo vem
    build_go_repo vem
}

build_carrion() {
    log_section "Building Carrion Language"
    fetch_repo carrion
    build_go_repo carrion
}

build_ivaldi() {
    log_section "Building Ivaldi VCS"
    fetch_repo ivaldi
    build_go_repo ivaldi
}

# =============================================================================
# Local tool builds (embedded in tools/ or bootloader/)
# =============================================================================

build_raven_dhcp() {
    log_section "Building raven-dhcp DHCP Client"

    local dhcp_dir="${PROJECT_ROOT}/tools/raven-dhcp"

    if [ -d "$dhcp_dir" ]; then
        cd "$dhcp_dir"

        log_info "Compiling raven-dhcp..."
        if run_logged env CGO_ENABLED=0 go build -o raven-dhcp .; then
            mkdir -p "${OUTPUT_DIR}/bin"
            cp raven-dhcp "${OUTPUT_DIR}/bin/"
            log_success "raven-dhcp built -> ${OUTPUT_DIR}/bin/raven-dhcp"
        else
            log_error "Failed to build raven-dhcp"
        fi

        cd "${PROJECT_ROOT}"
    else
        log_warn "raven-dhcp source not found, skipping"
    fi
}

build_installer() {
    log_section "Building Raven Installer"

    local installer_dir="${PROJECT_ROOT}/tools/raven-installer"

    if [ -d "$installer_dir" ]; then
        cd "$installer_dir"
        log_info "Downloading dependencies..."
        run_logged go mod download 2>/dev/null || run_logged go mod tidy

        log_info "Compiling installer..."
        if run_logged env CGO_ENABLED=1 go build -o raven-installer .; then
            mkdir -p "${OUTPUT_DIR}/bin"
            cp raven-installer "${OUTPUT_DIR}/bin/"
            log_success "Installer built -> ${OUTPUT_DIR}/bin/raven-installer"
        else
            log_error "Failed to build installer"
        fi

        cd "${PROJECT_ROOT}"
    else
        log_warn "Installer source not found, skipping"
    fi
}

build_rvn() {
    log_section "Building rvn Package Manager"

    local rvn_dir="${PROJECT_ROOT}/tools/rvn"

    if [ -d "$rvn_dir" ]; then
        cd "$rvn_dir"

        if ! command -v cargo &>/dev/null; then
            log_warn "Rust/Cargo not found, skipping rvn build"
            cd "${PROJECT_ROOT}"
            return
        fi

        log_info "Building rvn with Cargo..."
        if run_logged cargo build --release; then
            mkdir -p "${OUTPUT_DIR}/bin"
            cp target/release/rvn "${OUTPUT_DIR}/bin/"
            log_success "rvn built -> ${OUTPUT_DIR}/bin/rvn"
        else
            log_error "Failed to build rvn"
        fi

        cd "${PROJECT_ROOT}"
    else
        log_warn "rvn source not found, skipping"
    fi
}

build_usb_creator() {
    log_section "Building Raven USB Creator"

    local usb_dir="${PROJECT_ROOT}/tools/raven-usb"

    if [ -d "$usb_dir" ]; then
        cd "$usb_dir"
        log_info "Downloading dependencies..."
        run_logged go mod download 2>/dev/null || run_logged go mod tidy

        log_info "Compiling USB creator..."
        if run_logged env CGO_ENABLED=1 go build -o raven-usb .; then
            mkdir -p "${OUTPUT_DIR}/bin"
            cp raven-usb "${OUTPUT_DIR}/bin/"
            log_success "USB creator built -> ${OUTPUT_DIR}/bin/raven-usb"
        else
            log_error "Failed to build USB creator"
        fi

        cd "${PROJECT_ROOT}"
    else
        log_warn "USB creator source not found, skipping"
    fi
}

build_bootloader() {
    log_section "Building RavenBoot Bootloader"

    local bootloader_dir="${PROJECT_ROOT}/bootloader"

    if [ -d "$bootloader_dir" ]; then
        cd "$bootloader_dir"

        if ! command -v cargo &>/dev/null; then
            log_warn "Rust/Cargo not found, skipping bootloader build"
            cd "${PROJECT_ROOT}"
            return
        fi

        if ! rustup target list --installed 2>/dev/null | grep -q "x86_64-unknown-uefi"; then
            log_info "Adding UEFI target..."
            if ! run_logged rustup target add x86_64-unknown-uefi; then
                log_warn "Failed to add UEFI target, skipping bootloader"
                cd "${PROJECT_ROOT}"
                return
            fi
        fi

        log_info "Building RavenBoot with Cargo..."
        if run_logged cargo build --target x86_64-unknown-uefi --release; then
            mkdir -p "${OUTPUT_DIR}/boot"
            cp target/x86_64-unknown-uefi/release/raven-boot.efi "${OUTPUT_DIR}/boot/"
            log_success "RavenBoot built -> ${OUTPUT_DIR}/boot/raven-boot.efi"
        else
            log_error "Failed to build RavenBoot"
        fi

        cd "${PROJECT_ROOT}"
    else
        log_warn "Bootloader source not found, skipping"
    fi
}

build_wifi_tools() {
    log_section "Building WiFi Tools"

    # Build wifi TUI
    local wifi_tui_dir="${PROJECT_ROOT}/tools/raven-wifi-tui"
    if [ -d "$wifi_tui_dir" ]; then
        cd "$wifi_tui_dir"
        log_info "Downloading dependencies for wifi TUI..."
        run_logged go mod download 2>/dev/null || run_logged go mod tidy

        log_info "Compiling wifi TUI..."
        if run_logged env CGO_ENABLED=0 go build -o wifi .; then
            mkdir -p "${OUTPUT_DIR}/bin"
            cp wifi "${OUTPUT_DIR}/bin/"
            log_success "wifi (TUI) built -> ${OUTPUT_DIR}/bin/wifi"
        else
            log_error "Failed to build wifi TUI"
        fi
        cd "${PROJECT_ROOT}"
    else
        log_warn "WiFi TUI source not found, skipping"
    fi

    # Build raven-wifi GUI with Wayland support
    local wifi_gui_dir="${PROJECT_ROOT}/tools/raven-wifi"
    if [ -d "$wifi_gui_dir" ]; then
        cd "$wifi_gui_dir"
        log_info "Downloading dependencies for raven-wifi GUI..."
        run_logged go mod download 2>/dev/null || run_logged go mod tidy

        log_info "Compiling raven-wifi GUI with Wayland support..."
        local cgo_cflags=""
        local cgo_ldflags=""

        if pkg-config --exists wayland-client wayland-egl xkbcommon 2>/dev/null; then
            cgo_cflags="$(pkg-config --cflags wayland-client wayland-egl xkbcommon 2>/dev/null || true)"
            cgo_ldflags="$(pkg-config --libs wayland-client wayland-egl xkbcommon 2>/dev/null || true)"
            log_info "Building with Wayland support: ${cgo_ldflags}"
        fi

        if run_logged env CGO_ENABLED=1 \
            CGO_CFLAGS="${cgo_cflags}" \
            CGO_LDFLAGS="${cgo_ldflags}" \
            go build -ldflags="-s -w" -o raven-wifi .; then
            mkdir -p "${OUTPUT_DIR}/bin"
            cp raven-wifi "${OUTPUT_DIR}/bin/"
            chmod +x "${OUTPUT_DIR}/bin/raven-wifi"
            log_success "raven-wifi (GUI) built -> ${OUTPUT_DIR}/bin/raven-wifi"
        else
            log_error "Failed to build raven-wifi GUI"
        fi
        cd "${PROJECT_ROOT}"
    else
        log_warn "WiFi GUI source not found, skipping"
    fi
}

build_raven_launcher() {
    log_section "Building raven-launcher"

    local launcher_dir="${PROJECT_ROOT}/tools/raven-launcher"
    if [ ! -d "$launcher_dir" ]; then
        log_warn "raven-launcher source not found at ${launcher_dir}, skipping"
        return 0
    fi

    cd "$launcher_dir"
    log_info "Downloading dependencies for raven-launcher..."
    run_logged go mod download 2>/dev/null || run_logged go mod tidy

    log_info "Compiling raven-launcher..."
    local cgo_cflags=""
    local cgo_ldflags=""

    if pkg-config --exists wayland-client wayland-egl xkbcommon 2>/dev/null; then
        cgo_cflags="$(pkg-config --cflags wayland-client wayland-egl xkbcommon 2>/dev/null || true)"
        cgo_ldflags="$(pkg-config --libs wayland-client wayland-egl xkbcommon 2>/dev/null || true)"
    fi

    if run_logged env CGO_ENABLED=1 \
        CGO_CFLAGS="${cgo_cflags}" \
        CGO_LDFLAGS="${cgo_ldflags}" \
        go build -o raven-launcher .; then
        mkdir -p "${OUTPUT_DIR}/bin"
        cp raven-launcher "${OUTPUT_DIR}/bin/"
        log_success "raven-launcher built -> ${OUTPUT_DIR}/bin/raven-launcher"
    else
        log_error "Failed to build raven-launcher"
    fi
    cd "${PROJECT_ROOT}"
}

# Build all packages
build_all() {
    # GitHub repos
    build_compositor
    build_file_manager
    build_terminal
    build_shell
    build_poxy
    build_vem
    build_carrion
    build_ivaldi
    # Local tools
    build_installer
    build_rvn
    build_raven_dhcp
    build_usb_creator
    build_wifi_tools
    build_raven_launcher
    build_bootloader
}

print_summary() {
    log_section "Build Complete!"

    echo "  Built packages are in: ${OUTPUT_DIR}/bin/"
    echo ""

    if [[ -d "${OUTPUT_DIR}/bin" ]]; then
        ls -lh "${OUTPUT_DIR}/bin/" 2>/dev/null || true
    fi

    if [[ -d "${OUTPUT_DIR}/boot" ]]; then
        echo ""
        echo "  Boot files in: ${OUTPUT_DIR}/boot/"
        ls -lh "${OUTPUT_DIR}/boot/" 2>/dev/null || true
    fi

    echo ""
    if is_logging_enabled; then
        echo "  Build Log: $(get_log_file)"
        echo ""
    fi
}

# =============================================================================
# Main
# =============================================================================

main() {
    local target="all"

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --no-log)
                export RAVEN_NO_LOG=1
                shift
                ;;
            compositor|file-manager|terminal|shell|poxy|vem|carrion|ivaldi|installer|rvn|dhcp|usb|bootloader|wifi|launcher|all)
                target="$1"
                shift
                ;;
            *)
                log_error "Unknown package or option: $1"
                echo "Usage: $0 [--no-log] [compositor|file-manager|terminal|shell|poxy|vem|carrion|ivaldi|installer|rvn|dhcp|usb|bootloader|wifi|launcher|all]"
                exit 1
                ;;
        esac
    done

    # Initialize logging
    init_logging "build-packages" "RavenLinux Custom Packages Builder"
    enable_logging_trap

    log_section "RavenLinux Custom Packages Builder"

    echo "  Target: ${target}"
    if is_logging_enabled; then
        echo "  Log:    $(get_log_file)"
    fi
    echo ""

    mkdir -p "$SOURCES_DIR" "$OUTPUT_DIR"

    check_dependencies

    case "$target" in
        compositor)     build_compositor ;;
        file-manager)   build_file_manager ;;
        terminal)       build_terminal ;;
        shell)          build_shell ;;
        poxy)           build_poxy ;;
        vem)            build_vem ;;
        carrion)        build_carrion ;;
        ivaldi)         build_ivaldi ;;
        installer)      build_installer ;;
        rvn)            build_rvn ;;
        dhcp)           build_raven_dhcp ;;
        usb)            build_usb_creator ;;
        bootloader)     build_bootloader ;;
        wifi)           build_wifi_tools ;;
        launcher)       build_raven_launcher ;;
        all)            build_all ;;
    esac

    print_summary
    finalize_logging 0
}

main "$@"
