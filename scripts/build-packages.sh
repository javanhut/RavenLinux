#!/bin/bash
# =============================================================================
# RavenLinux Custom Packages Build Script
# =============================================================================
# Builds custom Go packages from GitHub for RavenLinux
#
# Usage: ./scripts/build-packages.sh [OPTIONS] [package-name|all]
#
# Packages:
#   vem        Build Vem text editor
#   carrion    Build Carrion programming language
#   ivaldi     Build Ivaldi VCS
#   installer  Build Raven Installer
#   rvn        Build rvn package manager
#   dhcp       Build raven-dhcp DHCP client
#   usb        Build USB creator tool
#   bootloader Build RavenBoot bootloader
#   all        Build all packages (default)
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

# Source shared logging library
source "${SCRIPT_DIR}/lib/logging.sh"

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

# Build a Go package from GitHub
build_go_package() {
    local name="$1"
    local repo="$2"
    local binary="$3"
    local cgo="${4:-0}"

    log_info "Building ${name}..."

    local src_dir="${SOURCES_DIR}/${name}"

    # Clone or update repository
    if [ -d "$src_dir" ]; then
        log_info "Updating ${name} from GitHub..."
        cd "$src_dir"
        run_logged git fetch origin main
        run_logged git reset --hard origin/main
    else
        log_info "Cloning ${name} from GitHub..."
        run_logged git clone --depth 1 "https://github.com/${repo}.git" "$src_dir"
        cd "$src_dir"
    fi

    # Download Go dependencies
    log_info "Downloading dependencies for ${name}..."
    run_logged go mod download

    # Build
    log_info "Compiling ${name}..."
    if ! run_logged env CGO_ENABLED="$cgo" go build -o "${binary}" .; then
        log_error "Failed to build ${name}"
        return 1
    fi

    # Copy to output
    mkdir -p "${OUTPUT_DIR}/bin"
    cp "${binary}" "${OUTPUT_DIR}/bin/"

    log_success "${name} built successfully -> ${OUTPUT_DIR}/bin/${binary}"
}

# Build Vem text editor
build_vem() {
    log_section "Building Vem Text Editor"

    # Vem requires CGO for Gio UI/Wayland support
    build_go_package "vem" "javanhut/Vem" "vem" "1"
}

# Build Carrion programming language
build_carrion() {
    log_section "Building Carrion Language"

    local name="carrion"
    local repo="javanhut/TheCarrionLanguage"
    local src_dir="${SOURCES_DIR}/${name}"

    log_info "Building ${name}..."

    # Clone or update repository
    if [ -d "$src_dir" ]; then
        log_info "Updating ${name} from GitHub..."
        cd "$src_dir"
        run_logged git fetch origin main
        run_logged git reset --hard origin/main
    else
        log_info "Cloning ${name} from GitHub..."
        run_logged git clone --depth 1 "https://github.com/${repo}.git" "$src_dir"
        cd "$src_dir"
    fi

    # Download Go dependencies
    log_info "Downloading dependencies for ${name}..."
    run_logged go mod download

    # Build - Carrion has main.go in src/ directory
    log_info "Compiling ${name}..."
    if ! run_logged env CGO_ENABLED=0 go build -o carrion ./src/main.go; then
        log_error "Failed to build ${name}"
        return 1
    fi

    # Copy to output
    mkdir -p "${OUTPUT_DIR}/bin"
    cp carrion "${OUTPUT_DIR}/bin/"

    log_success "${name} built successfully -> ${OUTPUT_DIR}/bin/carrion"
}

# Build Ivaldi VCS
build_ivaldi() {
    log_section "Building Ivaldi VCS"

    build_go_package "ivaldi" "javanhut/IvaldiVCS" "ivaldi" "0"
}

# Build Raven Installer
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

# Build rvn package manager
build_rvn() {
    log_section "Building rvn Package Manager"

    local rvn_dir="${PROJECT_ROOT}/tools/rvn"

    if [ -d "$rvn_dir" ]; then
        cd "$rvn_dir"

        # Check for cargo
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

# Build USB creator tool
build_usb_creator() {
    log_section "Building Raven USB Creator"

    local usb_dir="${PROJECT_ROOT}/tools/raven-usb"

    if [ -d "$usb_dir" ]; then
        cd "$usb_dir"
        log_info "Downloading dependencies..."
        run_logged go mod download 2>/dev/null || run_logged go mod tidy

        log_info "Compiling USB creator..."
        # Requires CGO for Gio UI graphics
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

# Build RavenBoot bootloader
build_bootloader() {
    log_section "Building RavenBoot Bootloader"

    local bootloader_dir="${PROJECT_ROOT}/bootloader"

    if [ -d "$bootloader_dir" ]; then
        cd "$bootloader_dir"

        # Check for cargo
        if ! command -v cargo &>/dev/null; then
            log_warn "Rust/Cargo not found, skipping bootloader build"
            cd "${PROJECT_ROOT}"
            return
        fi

        # Check for UEFI target
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

# Build WiFi tools
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

    # Build raven-wifi GUI
    local wifi_gui_dir="${PROJECT_ROOT}/tools/raven-wifi"
    if [ -d "$wifi_gui_dir" ]; then
        cd "$wifi_gui_dir"
        log_info "Downloading dependencies for raven-wifi GUI..."
        run_logged go mod download 2>/dev/null || run_logged go mod tidy

        log_info "Compiling raven-wifi GUI..."
        if run_logged env CGO_ENABLED=1 go build -o raven-wifi .; then
            mkdir -p "${OUTPUT_DIR}/bin"
            cp raven-wifi "${OUTPUT_DIR}/bin/"
            log_success "raven-wifi (GUI) built -> ${OUTPUT_DIR}/bin/raven-wifi"
        else
            log_error "Failed to build raven-wifi GUI"
        fi
        cd "${PROJECT_ROOT}"
    else
        log_warn "WiFi GUI source not found, skipping"
    fi
}

# Build all packages
build_all() {
    build_vem
    build_carrion
    build_ivaldi
    build_installer
    build_rvn
    build_raven_dhcp
    build_usb_creator
    build_wifi_tools
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
            vem|carrion|ivaldi|installer|rvn|dhcp|usb|bootloader|all)
                target="$1"
                shift
                ;;
            *)
                log_error "Unknown package or option: $1"
                echo "Usage: $0 [--no-log] [vem|carrion|ivaldi|installer|rvn|dhcp|usb|bootloader|all]"
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
        vem)
            build_vem
            ;;
        carrion)
            build_carrion
            ;;
        ivaldi)
            build_ivaldi
            ;;
        installer)
            build_installer
            ;;
        rvn)
            build_rvn
            ;;
        dhcp)
            build_raven_dhcp
            ;;
        usb)
            build_usb_creator
            ;;
        bootloader)
            build_bootloader
            ;;
        all)
            build_all
            ;;
    esac

    print_summary
    finalize_logging 0
}

main "$@"
