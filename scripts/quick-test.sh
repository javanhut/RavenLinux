#!/bin/bash
# Quick test script - boot a minimal Linux environment in QEMU
# This uses your host kernel for rapid testing

set -euo pipefail

RAVEN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RAVEN_BUILD="${RAVEN_ROOT}/build"

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Check for QEMU
if ! command -v qemu-system-x86_64 &>/dev/null; then
    log_error "QEMU not found. Install with: sudo pacman -S qemu-full"
fi

# Check for KVM
KVM_OPTS=""
if [[ -r /dev/kvm ]]; then
    KVM_OPTS="-enable-kvm"
    log_info "KVM available - using hardware acceleration"
else
    log_info "KVM not available - using software emulation (slower)"
fi

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

show_help() {
    cat << EOF
Quick Test - Boot Linux in QEMU

Usage: $0 [OPTIONS]

Options:
    -k, --kernel PATH    Kernel image (default: /boot/vmlinuz-linux)
    -i, --initrd PATH    Initramfs image (default: /boot/initramfs-linux.img)
    -m, --memory SIZE    RAM size (default: 2G)
    -g, --graphics       Enable graphical display
    -c, --cmdline ARGS   Kernel command line
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

while [[ $# -gt 0 ]]; do
    case "$1" in
        -k|--kernel) KERNEL="$2"; shift 2 ;;
        -i|--initrd) INITRD="$2"; shift 2 ;;
        -m|--memory) MEMORY="$2"; shift 2 ;;
        -g|--graphics) GRAPHICS="yes"; shift ;;
        -c|--cmdline) CMDLINE="$2"; shift 2 ;;
        -h|--help) show_help; exit 0 ;;
        *) log_error "Unknown option: $1" ;;
    esac
done

# Verify kernel exists
if [[ ! -f "$KERNEL" ]]; then
    log_error "Kernel not found: $KERNEL"
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
    log_info "No initrd specified, booting without"
fi

log_info "Starting QEMU..."
log_info "  Kernel:  $KERNEL"
log_info "  Initrd:  ${INITRD:-none}"
log_info "  Memory:  $MEMORY"
log_info "  Cmdline: $CMDLINE"
echo ""
log_info "Press Ctrl+A, X to exit"
echo ""

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
