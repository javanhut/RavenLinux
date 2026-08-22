#!/bin/bash
# =============================================================================
# RavenLinux Quick Test Script
# =============================================================================
# Boot a minimal Linux environment in QEMU for testing
# This uses your host kernel for rapid testing
#
# Usage: ./scripts/quick-test.sh [OPTIONS]
#
# Options:
#   -k, --kernel PATH    Kernel image
#   -i, --initrd PATH    Initramfs image
#   -m, --memory SIZE    RAM size (default: 2G)
#   -g, --graphics       Enable graphical display
#   -c, --cmdline ARGS   Kernel command line
#   --no-log             Disable file logging
#   -h, --help           Show this help

set -euo pipefail

# =============================================================================
# Configuration
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export RAVEN_ROOT="$(dirname "$SCRIPT_DIR")"
export RAVEN_BUILD="${RAVEN_ROOT}/build"

# Source shared logging library
source "${SCRIPT_DIR}/lib/logging.sh"

# Options
MEMORY="2G"
GRAPHICS=""
CMDLINE="rdinit=/init"

# Use RavenLinux kernel/initramfs if available, otherwise fall back to host
if [[ -f "${RAVEN_BUILD}/kernel/boot/vmlinuz-raven" ]]; then
    KERNEL="${RAVEN_BUILD}/kernel/boot/vmlinuz-raven"
else
    KERNEL="/boot/vmlinuz-linux"
fi

if [[ -f "${RAVEN_BUILD}/initramfs-raven.img" ]]; then
    INITRD="${RAVEN_BUILD}/initramfs-raven.img"
else
    INITRD="/boot/initramfs-linux.img"
fi

# =============================================================================
# Functions
# =============================================================================

show_help() {
    cat << EOF
RavenLinux Quick Test - Boot Linux in QEMU

Usage: $0 [OPTIONS]

Options:
    -k, --kernel PATH    Kernel image (default: RavenLinux kernel or host)
    -i, --initrd PATH    Initramfs image (default: RavenLinux initramfs or host)
    -m, --memory SIZE    RAM size (default: 2G)
    -g, --graphics       Enable graphical display
    -c, --cmdline ARGS   Kernel command line
    --no-log             Disable file logging
    -h, --help           Show this help

Examples:
    $0                              # Boot with defaults (serial console)
    $0 -g                           # Boot with graphics
    $0 -k ./my-kernel -i ./my-initrd
    $0 -c "console=ttyS0 debug"     # Custom kernel args

Controls:
    Ctrl+A, X    Exit QEMU (serial mode)
    Ctrl+A, C    QEMU monitor console
EOF
}

# =============================================================================
# Argument Parsing
# =============================================================================

while [[ $# -gt 0 ]]; do
    case "$1" in
        -k|--kernel) KERNEL="$2"; shift 2 ;;
        -i|--initrd) INITRD="$2"; shift 2 ;;
        -m|--memory) MEMORY="$2"; shift 2 ;;
        -g|--graphics) GRAPHICS="yes"; shift ;;
        -c|--cmdline) CMDLINE="$2"; shift 2 ;;
        --no-log) export RAVEN_NO_LOG=1; shift ;;
        -h|--help) show_help; exit 0 ;;
        *) log_fatal "Unknown option: $1" ;;
    esac
done

# =============================================================================
# Main
# =============================================================================

main() {
    # Initialize logging
    init_logging "quick-test" "QEMU Test Session"

    log_section "RavenLinux Quick Test"

    # Check for QEMU
    if ! command -v qemu-system-x86_64 &>/dev/null; then
        log_fatal "QEMU not found. Install with: sudo pacman -S qemu-full"
    fi

    # Check for KVM
    KVM_OPTS=""
    if [[ -r /dev/kvm ]]; then
        KVM_OPTS="-enable-kvm"
        log_info "KVM available - using hardware acceleration"
    else
        log_warn "KVM not available - using software emulation (slower)"
    fi

    # Verify kernel exists
    if [[ ! -f "$KERNEL" ]]; then
        log_fatal "Kernel not found: $KERNEL"
    fi

    # Build display args
    DISPLAY_ARGS=""
    if [[ -n "$GRAPHICS" ]]; then
        DISPLAY_ARGS="-display gtk -vga virtio"
    else
        DISPLAY_ARGS="-nographic"
        CMDLINE="$CMDLINE console=ttyS0"
    fi

    # Build initrd args
    INITRD_ARGS=""
    if [[ -f "$INITRD" ]]; then
        INITRD_ARGS="-initrd $INITRD"
    else
        log_warn "No initrd specified, booting without"
    fi

    log_info "Configuration:"
    echo "  Kernel:  $KERNEL"
    echo "  Initrd:  ${INITRD:-none}"
    echo "  Memory:  $MEMORY"
    echo "  Cmdline: $CMDLINE"
    echo "  Graphics: ${GRAPHICS:-no}"
    if is_logging_enabled; then
        echo "  Log:     $(get_log_file)"
    fi
    echo ""
    log_info "Press Ctrl+A, X to exit QEMU"
    echo ""

    log_step "Starting QEMU..."

    # Log the QEMU command
    local qemu_cmd="qemu-system-x86_64 $KVM_OPTS -m $MEMORY -kernel $KERNEL $INITRD_ARGS -append \"$CMDLINE\" $DISPLAY_ARGS -device virtio-rng-pci -netdev user,id=net0 -device virtio-net-pci,netdev=net0"
    log_debug "Command: $qemu_cmd"

    # Run QEMU
    qemu-system-x86_64 \
        $KVM_OPTS \
        -m "$MEMORY" \
        -kernel "$KERNEL" \
        $INITRD_ARGS \
        -append "$CMDLINE" \
        $DISPLAY_ARGS \
        -device virtio-rng-pci \
        -netdev user,id=net0 \
        -device virtio-net-pci,netdev=net0

    local exit_code=$?

    if [[ $exit_code -eq 0 ]]; then
        log_success "QEMU session ended normally"
    else
        log_warn "QEMU exited with code: $exit_code"
    fi

    finalize_logging $exit_code
}

main "$@"
