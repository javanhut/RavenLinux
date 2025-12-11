#!/bin/bash
# RavenLinux Development Environment
# Run and test RavenLinux interactively without installing

set -euo pipefail

RAVEN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RAVEN_BUILD="${RAVEN_ROOT}/build"
DEV_ROOT="${RAVEN_BUILD}/dev-root"
DEV_WORK="${RAVEN_BUILD}/dev-work"
DEV_MERGED="${RAVEN_BUILD}/dev-merged"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

show_help() {
    cat << 'EOF'
RavenLinux Development Environment

Usage: ./dev-env.sh [COMMAND] [OPTIONS]

Commands:
    setup       Create development environment directories
    chroot      Enter RavenLinux environment via chroot
    qemu        Boot RavenLinux in QEMU virtual machine
    qemu-iso    Boot from ISO in QEMU
    mount       Mount overlay filesystem for testing
    umount      Unmount overlay filesystem
    shell       Quick shell in the dev environment
    clean       Remove development environment
    status      Show current environment status

Options:
    -m, --memory SIZE   RAM for QEMU (default: 4G)
    -c, --cpus NUM      CPUs for QEMU (default: 4)
    -g, --graphics      Enable graphics (default: headless)
    -d, --debug         Enable debug output
    -h, --help          Show this help

Examples:
    ./dev-env.sh setup              # Initialize dev environment
    ./dev-env.sh chroot             # Enter chroot environment
    ./dev-env.sh qemu -g            # Boot in QEMU with graphics
    ./dev-env.sh mount              # Mount for file editing
    ./dev-env.sh shell              # Quick test shell

Interactive Testing Workflow:
    1. ./dev-env.sh setup           # One-time setup
    2. ./dev-env.sh mount           # Mount overlay FS
    3. # Edit files in build/dev-merged/
    4. ./dev-env.sh chroot          # Test changes
    5. ./dev-env.sh umount          # Cleanup

EOF
}

check_dependencies() {
    local missing=()

    for cmd in qemu-system-x86_64 chroot mount umount; do
        if ! command -v "$cmd" &>/dev/null; then
            missing+=("$cmd")
        fi
    done

    if [[ ${#missing[@]} -gt 0 ]]; then
        log_warn "Missing dependencies: ${missing[*]}"
        log_info "Install with: sudo pacman -S qemu-full"
    fi
}

setup_dev_env() {
    log_info "Setting up development environment..."

    mkdir -p "${DEV_ROOT}"
    mkdir -p "${DEV_WORK}"
    mkdir -p "${DEV_MERGED}"
    mkdir -p "${RAVEN_BUILD}/qemu"

    # Create basic directory structure if sysroot doesn't exist
    if [[ ! -d "${RAVEN_BUILD}/sysroot/usr" ]]; then
        log_info "Creating minimal sysroot structure..."
        local sysroot="${RAVEN_BUILD}/sysroot"
        mkdir -p "${sysroot}"/{bin,boot,dev,etc,home,lib,lib64,mnt,opt,proc,root,run,sbin,sys,tmp,usr,var}
        mkdir -p "${sysroot}"/usr/{bin,include,lib,share,src}
        mkdir -p "${sysroot}"/var/{cache,lib,log,tmp}

        # Copy os-release
        cp "${RAVEN_ROOT}/etc/os-release" "${sysroot}/etc/"

        # Create basic /etc files
        echo "root:x:0:0:root:/root:/bin/bash" > "${sysroot}/etc/passwd"
        echo "root:x:0:" > "${sysroot}/etc/group"
        echo "raven" > "${sysroot}/etc/hostname"
    fi

    log_success "Development environment ready"
    log_info "Sysroot: ${RAVEN_BUILD}/sysroot"
    log_info "Dev merged: ${DEV_MERGED}"
}

mount_overlay() {
    log_info "Mounting overlay filesystem..."

    local sysroot="${RAVEN_BUILD}/sysroot"

    if ! [[ -d "$sysroot" ]]; then
        log_error "Sysroot not found. Run 'setup' first or build stage1."
    fi

    # Check if already mounted
    if mountpoint -q "${DEV_MERGED}" 2>/dev/null; then
        log_warn "Already mounted at ${DEV_MERGED}"
        return 0
    fi

    # Mount overlay: base (sysroot) + changes (dev-work) = merged view
    sudo mount -t overlay overlay \
        -o lowerdir="${sysroot}",upperdir="${DEV_ROOT}",workdir="${DEV_WORK}" \
        "${DEV_MERGED}"

    log_success "Overlay mounted at ${DEV_MERGED}"
    log_info "Changes go to: ${DEV_ROOT}"
    log_info "Edit files in: ${DEV_MERGED}"
}

umount_overlay() {
    log_info "Unmounting overlay filesystem..."

    if mountpoint -q "${DEV_MERGED}" 2>/dev/null; then
        sudo umount "${DEV_MERGED}"
        log_success "Unmounted ${DEV_MERGED}"
    else
        log_warn "Not mounted"
    fi
}

enter_chroot() {
    log_info "Entering RavenLinux chroot environment..."

    local target="${DEV_MERGED}"

    if ! mountpoint -q "${target}" 2>/dev/null; then
        log_info "Mounting overlay first..."
        mount_overlay
    fi

    # Mount essential filesystems
    sudo mount --bind /dev "${target}/dev" 2>/dev/null || true
    sudo mount --bind /dev/pts "${target}/dev/pts" 2>/dev/null || true
    sudo mount -t proc proc "${target}/proc" 2>/dev/null || true
    sudo mount -t sysfs sysfs "${target}/sys" 2>/dev/null || true
    sudo mount -t tmpfs tmpfs "${target}/tmp" 2>/dev/null || true

    # Copy resolv.conf for networking
    sudo cp /etc/resolv.conf "${target}/etc/resolv.conf" 2>/dev/null || true

    log_success "Entering chroot. Type 'exit' to leave."
    echo ""

    # Enter chroot
    sudo chroot "${target}" /bin/bash -l || sudo chroot "${target}" /bin/sh -l || {
        log_warn "No shell available in chroot. You may need to build the base system first."
    }

    # Cleanup mounts on exit
    log_info "Cleaning up mounts..."
    sudo umount "${target}/tmp" 2>/dev/null || true
    sudo umount "${target}/sys" 2>/dev/null || true
    sudo umount "${target}/proc" 2>/dev/null || true
    sudo umount "${target}/dev/pts" 2>/dev/null || true
    sudo umount "${target}/dev" 2>/dev/null || true
}

run_qemu() {
    local memory="4G"
    local cpus="4"
    local graphics=""
    local extra_args=()

    # Parse options
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -m|--memory) memory="$2"; shift 2 ;;
            -c|--cpus) cpus="$2"; shift 2 ;;
            -g|--graphics) graphics="yes"; shift ;;
            *) extra_args+=("$1"); shift ;;
        esac
    done

    local disk="${RAVEN_BUILD}/qemu/raven-dev.qcow2"

    # Create disk image if it doesn't exist
    if [[ ! -f "$disk" ]]; then
        log_info "Creating QEMU disk image (20GB)..."
        qemu-img create -f qcow2 "$disk" 20G
    fi

    # Build kernel/initramfs args
    local kernel_args=()
    if [[ -f "${RAVEN_BUILD}/sysroot/boot/vmlinuz" ]]; then
        kernel_args+=(-kernel "${RAVEN_BUILD}/sysroot/boot/vmlinuz")

        if [[ -f "${RAVEN_BUILD}/sysroot/boot/initramfs.img" ]]; then
            kernel_args+=(-initrd "${RAVEN_BUILD}/sysroot/boot/initramfs.img")
        fi

        kernel_args+=(-append "root=/dev/sda1 console=ttyS0 rw")
    fi

    # Display args
    local display_args=()
    if [[ -n "$graphics" ]]; then
        display_args+=(-display gtk -vga virtio)
    else
        display_args+=(-nographic -serial mon:stdio)
    fi

    log_info "Starting QEMU..."
    log_info "  Memory: ${memory}"
    log_info "  CPUs: ${cpus}"
    log_info "  Graphics: ${graphics:-no}"
    log_info ""
    log_info "Press Ctrl+A, X to exit (or close window)"
    echo ""

    qemu-system-x86_64 \
        -enable-kvm \
        -m "$memory" \
        -smp "$cpus" \
        -drive file="$disk",format=qcow2,if=virtio \
        "${kernel_args[@]}" \
        "${display_args[@]}" \
        -netdev user,id=net0,hostfwd=tcp::2222-:22 \
        -device virtio-net-pci,netdev=net0 \
        -device virtio-rng-pci \
        "${extra_args[@]}"
}

run_qemu_iso() {
    local iso="${RAVEN_ROOT}/raven-"*".iso"
    local memory="4G"
    local graphics=""

    while [[ $# -gt 0 ]]; do
        case "$1" in
            -m|--memory) memory="$2"; shift 2 ;;
            -g|--graphics) graphics="yes"; shift ;;
            *) iso="$1"; shift ;;
        esac
    done

    if ! ls $iso &>/dev/null; then
        log_error "No ISO found. Build with: ./scripts/build.sh stage4"
    fi

    local display_args=()
    if [[ -n "$graphics" ]]; then
        display_args+=(-display gtk -vga virtio)
    else
        display_args+=(-nographic -serial mon:stdio)
    fi

    log_info "Booting ISO: $iso"

    qemu-system-x86_64 \
        -enable-kvm \
        -m "$memory" \
        -smp 4 \
        -cdrom $iso \
        -boot d \
        "${display_args[@]}" \
        -device virtio-rng-pci
}

quick_shell() {
    log_info "Quick shell in dev environment..."

    if [[ -d "${DEV_MERGED}/bin" ]] && mountpoint -q "${DEV_MERGED}" 2>/dev/null; then
        cd "${DEV_MERGED}"
        export RAVEN_DEV=1
        export PS1="[raven-dev] \w $ "
        exec bash
    else
        log_info "Overlay not mounted. Using sysroot directly."
        cd "${RAVEN_BUILD}/sysroot"
        export RAVEN_DEV=1
        export PS1="[raven-sysroot] \w $ "
        exec bash
    fi
}

show_status() {
    echo ""
    echo "RavenLinux Development Environment Status"
    echo "=========================================="
    echo ""

    echo "Directories:"
    echo "  Root:     ${RAVEN_ROOT}"
    echo "  Build:    ${RAVEN_BUILD}"
    echo "  Sysroot:  ${RAVEN_BUILD}/sysroot"
    echo "  Merged:   ${DEV_MERGED}"
    echo ""

    echo "Overlay Mount:"
    if mountpoint -q "${DEV_MERGED}" 2>/dev/null; then
        echo -e "  Status: ${GREEN}Mounted${NC}"
    else
        echo -e "  Status: ${YELLOW}Not mounted${NC}"
    fi
    echo ""

    echo "Build Artifacts:"
    [[ -d "${RAVEN_BUILD}/sysroot/usr" ]] && echo "  ✓ Sysroot exists" || echo "  ✗ Sysroot missing"
    [[ -f "${RAVEN_BUILD}/sysroot/boot/vmlinuz" ]] && echo "  ✓ Kernel built" || echo "  ✗ Kernel not built"
    ls "${RAVEN_ROOT}"/raven-*.iso &>/dev/null && echo "  ✓ ISO exists" || echo "  ✗ ISO not built"
    echo ""

    echo "QEMU:"
    [[ -f "${RAVEN_BUILD}/qemu/raven-dev.qcow2" ]] && echo "  ✓ Disk image exists" || echo "  ✗ No disk image"
    echo ""
}

clean_dev_env() {
    log_warn "This will remove all development environment data!"
    read -p "Continue? [y/N] " -n 1 -r
    echo

    if [[ $REPLY =~ ^[Yy]$ ]]; then
        umount_overlay 2>/dev/null || true
        rm -rf "${DEV_ROOT}" "${DEV_WORK}" "${DEV_MERGED}" "${RAVEN_BUILD}/qemu"
        log_success "Development environment cleaned"
    else
        log_info "Cancelled"
    fi
}

# Main
check_dependencies

case "${1:-}" in
    setup)      setup_dev_env ;;
    chroot)     enter_chroot ;;
    qemu)       shift; run_qemu "$@" ;;
    qemu-iso)   shift; run_qemu_iso "$@" ;;
    mount)      mount_overlay ;;
    umount)     umount_overlay ;;
    shell)      quick_shell ;;
    status)     show_status ;;
    clean)      clean_dev_env ;;
    -h|--help|help|"")  show_help ;;
    *)          log_error "Unknown command: $1" ;;
esac
