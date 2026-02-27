#!/bin/bash
# =============================================================================
# RavenLinux Desktop Test Script
# =============================================================================
# Quick way to test the desktop environment without building a full ISO
#
# Usage:
#   ./scripts/test-desktop.sh              # Test with Raven Compositor (default)
#   ./scripts/test-desktop.sh --rebuild    # Rebuild components first, then test
#   ./scripts/test-desktop.sh --nested     # Run nested in current session (no QEMU)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
export RAVEN_ROOT="$PROJECT_ROOT"
export RAVEN_BUILD="${PROJECT_ROOT}/build"
BUILD_DIR="${RAVEN_BUILD}"
SYSROOT="${BUILD_DIR}/sysroot"
PACKAGES_BIN="${BUILD_DIR}/packages/bin"
ISO_ROOT="${BUILD_DIR}/iso/iso-root"

# Source repos library
source "${SCRIPT_DIR}/lib/repos.sh"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Default compositor
COMPOSITOR="raven"
REBUILD=false
NESTED=false
SERIAL=false

# Parse arguments
for arg in "$@"; do
    case "$arg" in
        raven|raven-compositor)
            COMPOSITOR="raven"
            ;;
        --rebuild|-r)
            REBUILD=true
            ;;
        --nested|-n)
            NESTED=true
            ;;
        --serial|-s)
            SERIAL=true
            ;;
        --help|-h)
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --rebuild, -r   Rebuild desktop components before testing"
            echo "  --nested, -n    Run nested in current Wayland session (no QEMU)"
            echo "  --serial, -s    Enable serial console output"
            echo "  --help, -h      Show this help"
            exit 0
            ;;
    esac
done

COMPOSITOR_DIR="$(get_repo_dir compositor)"

# =============================================================================
# Build Functions
# =============================================================================

build_desktop_components() {
    log_info "Building desktop components..."

    # Fetch and build compositor from GitHub
    log_info "Fetching and building raven-compositor..."
    fetch_repo compositor
    cd "$COMPOSITOR_DIR"
    if cargo build --release 2>&1; then
        log_success "raven-compositor built"
    else
        log_warn "raven-compositor build failed"
    fi
    cd "${PROJECT_ROOT}"
}

copy_to_sysroot() {
    log_info "Copying binaries to sysroot..."

    sudo mkdir -p "${SYSROOT}/usr/bin" "${PACKAGES_BIN}"

    # Copy compositor binaries
    for bin in raven-compositor raven-shell raven-settings; do
        local src="${COMPOSITOR_DIR}/target/release/${bin}"
        if [[ -f "$src" ]]; then
            sudo cp "$src" "${SYSROOT}/usr/bin/"
            sudo cp "$src" "${PACKAGES_BIN}/" 2>/dev/null || true
            log_success "Copied ${bin}"
        fi
    done

    # Copy Xwayland if not present
    if [[ ! -f "${SYSROOT}/usr/bin/Xwayland" ]] && [[ -f "/usr/bin/Xwayland" ]]; then
        log_info "Copying Xwayland..."
        sudo cp /usr/bin/Xwayland "${SYSROOT}/usr/bin/"
        log_success "Copied Xwayland"
    fi
}

# =============================================================================
# Test Functions
# =============================================================================

run_nested() {
    log_info "Running nested test in current Wayland session..."

    if [[ -z "${WAYLAND_DISPLAY:-}" ]]; then
        log_error "No Wayland session detected. Run from a Wayland desktop or use QEMU mode."
        exit 1
    fi

    log_info "Starting Raven Compositor nested..."
    "${COMPOSITOR_DIR}/target/release/raven-compositor" --nested
}

run_qemu() {
    log_info "Starting QEMU test environment..."

    # Check for existing ISO first
    local iso_file=""
    for iso in "${BUILD_DIR}/iso/ravenlinux.iso" "${BUILD_DIR}/iso/"*.iso; do
        if [[ -f "$iso" ]]; then
            iso_file="$iso"
            break
        fi
    done

    # If no ISO, create a quick one from iso-root
    if [[ -z "$iso_file" ]] && [[ -d "${ISO_ROOT}" ]] && [[ -f "${ISO_ROOT}/raven/filesystem.squashfs" ]]; then
        log_info "Creating quick test ISO..."
        iso_file="/tmp/raven-test.iso"

        if command -v xorriso &>/dev/null; then
            xorriso -as mkisofs \
                -o "$iso_file" \
                -V "RAVENLINUX" \
                -J -R -l \
                -b boot/grub/i386-pc/eltorito.img \
                -no-emul-boot \
                -boot-load-size 4 \
                -boot-info-table \
                --grub2-boot-info \
                --grub2-mbr /usr/lib/grub/i386-pc/boot_hybrid.img \
                "${ISO_ROOT}" 2>/dev/null || \
            xorriso -as mkisofs -o "$iso_file" -V "RAVENLINUX" -J -R "${ISO_ROOT}" 2>/dev/null || \
            genisoimage -o "$iso_file" -V "RAVENLINUX" -J -R "${ISO_ROOT}" 2>/dev/null

            if [[ -f "$iso_file" ]]; then
                log_success "Created test ISO: $iso_file"
            fi
        elif command -v genisoimage &>/dev/null; then
            genisoimage -o "$iso_file" -V "RAVENLINUX" -J -R "${ISO_ROOT}" 2>/dev/null
            log_success "Created test ISO: $iso_file"
        fi
    fi

    # Check for required files
    local kernel=""
    local initramfs=""

    for k in "${ISO_ROOT}/boot/vmlinuz" "${SYSROOT}/boot/vmlinuz"; do
        if [[ -f "$k" ]]; then
            kernel="$k"
            break
        fi
    done

    if [[ -z "$kernel" ]]; then
        log_error "No kernel found. Run the build first."
        exit 1
    fi

    for i in "${ISO_ROOT}/boot/initramfs.img" "${SYSROOT}/boot/initramfs.img"; do
        if [[ -f "$i" ]]; then
            initramfs="$i"
            break
        fi
    done

    if [[ -z "$initramfs" ]]; then
        log_error "No initramfs found. Run: ./scripts/build-initramfs.sh"
        exit 1
    fi

    log_info "Kernel: $kernel"
    log_info "Initramfs: $initramfs"
    log_info "Compositor: $COMPOSITOR"
    [[ -n "$iso_file" ]] && log_info "ISO: $iso_file"

    local cmdline="rdinit=/init raven.graphics=wayland raven.wayland=${COMPOSITOR}"

    if [[ "$SERIAL" == true ]]; then
        cmdline="${cmdline} console=ttyS0,115200 console=tty0"
    else
        cmdline="${cmdline} console=tty0 quiet loglevel=3"
    fi

    log_info "Cmdline: $cmdline"
    echo ""
    log_info "Starting QEMU... (Ctrl+Alt+G to release mouse, Ctrl+C to quit)"
    echo ""

    local qemu_cmd=(
        qemu-system-x86_64
        -enable-kvm
        -m 4G
        -cpu host
        -smp 4
        -device virtio-gpu-pci
        -display gtk,gl=on,grab-on-hover=on
        -device usb-ehci
        -device usb-tablet
        -device usb-kbd
        -kernel "$kernel"
        -initrd "$initramfs"
        -append "$cmdline"
    )

    if [[ -n "$iso_file" ]] && [[ -f "$iso_file" ]]; then
        qemu_cmd+=(-cdrom "$iso_file")
    fi

    if [[ "$SERIAL" == true ]]; then
        qemu_cmd+=(-serial stdio)
    fi

    "${qemu_cmd[@]}"
}

# =============================================================================
# Main
# =============================================================================

echo ""
echo "=========================================="
echo "  Raven Desktop Test Environment"
echo "=========================================="
echo ""

# Rebuild if requested
if [[ "$REBUILD" == true ]]; then
    build_desktop_components
    copy_to_sysroot
    echo ""
fi

# Run test
if [[ "$NESTED" == true ]]; then
    run_nested
else
    run_qemu
fi
