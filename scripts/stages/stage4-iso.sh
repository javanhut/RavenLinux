#!/bin/bash
# =============================================================================
# RavenLinux Stage 4: Generate ISO Image
# =============================================================================
# Creates a bootable ISO image with:
# - RavenBoot UEFI bootloader (primary)
# - GRUB fallback for BIOS systems
# - Squashfs compressed root filesystem
# - Live boot support

set -euo pipefail

# Allow running standalone or sourced from build.sh
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${RAVEN_ROOT:-$(dirname "$(dirname "$SCRIPT_DIR")")}"
BUILD_DIR="${RAVEN_BUILD:-${PROJECT_ROOT}/build}"
SYSROOT_DIR="${BUILD_DIR}/sysroot"
PACKAGES_DIR="${BUILD_DIR}/packages"
ISO_DIR="${BUILD_DIR}/iso"
ISO_ROOT="${ISO_DIR}/iso-root"
LOGS_DIR="${BUILD_DIR}/logs"

# Version info
RAVEN_VERSION="${RAVEN_VERSION:-2025.12}"
RAVEN_ARCH="${RAVEN_ARCH:-x86_64}"
ISO_LABEL="RAVEN_LIVE"
ISO_OUTPUT="${PROJECT_ROOT}/raven-${RAVEN_VERSION}-${RAVEN_ARCH}.iso"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Use existing log functions or define them
type log_info &>/dev/null || log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
type log_success &>/dev/null || log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
type log_warn &>/dev/null || log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
type log_error &>/dev/null || log_error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }
log_step() { echo -e "${CYAN}[STEP]${NC} $1"; }

# =============================================================================
# Check dependencies
# =============================================================================
check_deps() {
    log_info "Checking dependencies..."

    local missing=()
    for cmd in mksquashfs xorriso; do
        if ! command -v "$cmd" &>/dev/null; then
            missing+=("$cmd")
        fi
    done

    if [[ ${#missing[@]} -gt 0 ]]; then
        log_error "Missing: ${missing[*]}. Install with: sudo pacman -S squashfs-tools libisoburn"
    fi

    log_success "Dependencies OK"
}

# =============================================================================
# Setup ISO directory structure
# =============================================================================
setup_iso_structure() {
    log_step "Setting up ISO structure..."

    rm -rf "${ISO_ROOT}"
    mkdir -p "${ISO_ROOT}"/{boot/grub,EFI/BOOT,EFI/raven,raven}

    log_success "ISO structure created"
}

# =============================================================================
# Create live init script in sysroot
# =============================================================================
create_live_init() {
    log_step "Creating live init system..."

    mkdir -p "${SYSROOT_DIR}"/{bin,sbin,etc}

    cat > "${SYSROOT_DIR}/init" << 'INIT'
#!/bin/bash
# RavenLinux Live Init

export PATH=/bin:/sbin:/usr/bin:/usr/sbin

echo "Starting Raven Linux..."

# Mount essential filesystems
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev 2>/dev/null || true
mkdir -p /dev/pts /dev/shm
mount -t devpts devpts /dev/pts
mount -t tmpfs tmpfs /dev/shm
mount -t tmpfs tmpfs /tmp
mount -t tmpfs tmpfs /run

# Set hostname
hostname raven

# Start udevd if available
if [[ -x /sbin/udevd ]]; then
    /sbin/udevd --daemon 2>/dev/null
    udevadm trigger 2>/dev/null
    udevadm settle 2>/dev/null
fi

# Configure networking
for iface in /sys/class/net/e*; do
    [[ -d "$iface" ]] || continue
    name="$(basename "$iface")"
    ip link set "$name" up 2>/dev/null
    dhcpcd "$name" 2>/dev/null &
done

# Clear screen and show banner
clear
cat << 'BANNER'

    ██████╗  █████╗ ██╗   ██╗███████╗███╗   ██╗
    ██╔══██╗██╔══██╗██║   ██║██╔════╝████╗  ██║
    ██████╔╝███████║██║   ██║█████╗  ██╔██╗ ██║
    ██╔══██╗██╔══██║╚██╗ ██╔╝██╔══╝  ██║╚██╗██║
    ██║  ██║██║  ██║ ╚████╔╝ ███████╗██║ ╚████║
    ╚═╝  ╚═╝╚═╝  ╚═╝  ╚═══╝  ╚══════╝╚═╝  ╚═══╝
                  L I N U X

BANNER

echo "  Welcome to Raven Linux Live!"
echo "  Version: $(grep VERSION_ID /etc/os-release 2>/dev/null | cut -d= -f2 || echo "2025.12")"
echo ""
echo "  Tools: vem (editor), carrion (lang), ivaldi (vcs), rvn (pkg)"
echo ""
echo "  Type 'raven-install' to install to disk"
echo ""

# Start login shell
exec /bin/zsh -l 2>/dev/null || exec /bin/bash -l || exec /bin/sh
INIT
    chmod +x "${SYSROOT_DIR}/init"

    # Also create /sbin/init symlink
    mkdir -p "${SYSROOT_DIR}/sbin"
    ln -sf /init "${SYSROOT_DIR}/sbin/init" 2>/dev/null || true

    log_success "Live init created"
}

# =============================================================================
# Copy kernel and initramfs
# =============================================================================
copy_boot_files() {
    log_step "Copying boot files..."

    # Kernel - try multiple locations
    local kernel=""
    for k in "${BUILD_DIR}/kernel/boot/vmlinuz-raven" \
             "${BUILD_DIR}/kernel/boot/vmlinuz-6.17-raven" \
             "${SYSROOT_DIR}/boot/vmlinuz"*; do
        if [[ -f "$k" ]]; then
            kernel="$k"
            break
        fi
    done

    if [[ -n "$kernel" ]]; then
        cp "$kernel" "${ISO_ROOT}/boot/vmlinuz"
        log_info "  Copied kernel: $(basename "$kernel")"
    else
        log_error "Kernel not found! Run stage1 first."
    fi

    # Initramfs
    if [[ -f "${BUILD_DIR}/initramfs-raven.img" ]]; then
        cp "${BUILD_DIR}/initramfs-raven.img" "${ISO_ROOT}/boot/initramfs.img"
        log_info "  Copied initramfs"
    else
        log_warn "Initramfs not found, ISO may not boot correctly"
    fi

    log_success "Boot files copied"
}

# =============================================================================
# Create squashfs filesystem
# =============================================================================
create_squashfs() {
    log_step "Creating squashfs filesystem..."

    # Add live init if not present
    [[ -f "${SYSROOT_DIR}/init" ]] || create_live_init

    mksquashfs "${SYSROOT_DIR}" "${ISO_ROOT}/raven/filesystem.squashfs" \
        -comp zstd -Xcompression-level 15 \
        -b 1M -no-duplicates -quiet \
        2>&1 | tee "${LOGS_DIR}/squashfs.log"

    local size
    size=$(du -h "${ISO_ROOT}/raven/filesystem.squashfs" | cut -f1)
    log_success "Squashfs created (${size})"
}

# =============================================================================
# Setup RavenBoot (UEFI)
# =============================================================================
setup_ravenboot() {
    log_step "Setting up RavenBoot (UEFI)..."

    local ravenboot="${PACKAGES_DIR}/boot/raven-boot.efi"

    if [[ -f "${ravenboot}" ]]; then
        # Copy RavenBoot as primary bootloader
        cp "${ravenboot}" "${ISO_ROOT}/EFI/BOOT/BOOTX64.EFI"
        mkdir -p "${ISO_ROOT}/EFI/raven"
        cp "${ravenboot}" "${ISO_ROOT}/EFI/raven/raven-boot.efi"

        # Create RavenBoot config
        cat > "${ISO_ROOT}/EFI/raven/boot.conf" << EOF
# RavenBoot Configuration
timeout = 5
default = 0

[entry]
name = "Raven Linux Live"
kernel = "\\boot\\vmlinuz"
initrd = "\\boot\\initramfs.img"
cmdline = "rdinit=/init quiet loglevel=3"
type = linux-efi

[entry]
name = "Raven Linux Live (Verbose)"
kernel = "\\boot\\vmlinuz"
initrd = "\\boot\\initramfs.img"
cmdline = "rdinit=/init"
type = linux-efi

[entry]
name = "Raven Linux (Recovery)"
kernel = "\\boot\\vmlinuz"
initrd = "\\boot\\initramfs.img"
cmdline = "rdinit=/init single"
type = linux-efi
EOF

        log_success "RavenBoot configured"
        return 0
    else
        log_warn "RavenBoot not found, using GRUB fallback"
        return 1
    fi
}

# =============================================================================
# Setup GRUB (fallback/BIOS)
# =============================================================================
setup_grub() {
    log_step "Setting up GRUB bootloader..."

    # Create GRUB config
    cat > "${ISO_ROOT}/boot/grub/grub.cfg" << 'EOF'
set default=0
set timeout=5

insmod all_video
insmod gfxterm
terminal_output gfxterm
set gfxmode=auto
set gfxpayload=keep

set color_normal=cyan/black
set color_highlight=white/blue

menuentry "Raven Linux Live" --class raven {
    linux /boot/vmlinuz rdinit=/init quiet loglevel=3
    initrd /boot/initramfs.img
}

menuentry "Raven Linux Live (Verbose)" --class raven {
    linux /boot/vmlinuz rdinit=/init
    initrd /boot/initramfs.img
}

menuentry "Raven Linux (Recovery)" --class raven {
    linux /boot/vmlinuz rdinit=/init single
    initrd /boot/initramfs.img
}

menuentry "Reboot" --class restart {
    reboot
}

menuentry "Shutdown" --class shutdown {
    halt
}
EOF

    # Create EFI bootloader if RavenBoot wasn't available
    if [[ ! -f "${ISO_ROOT}/EFI/BOOT/BOOTX64.EFI" ]]; then
        if command -v grub-mkstandalone &>/dev/null; then
            grub-mkstandalone \
                --format=x86_64-efi \
                --output="${ISO_ROOT}/EFI/BOOT/BOOTX64.EFI" \
                --locales="" \
                --fonts="" \
                "boot/grub/grub.cfg=${ISO_ROOT}/boot/grub/grub.cfg" 2>/dev/null || \
                log_warn "Failed to create GRUB EFI"
        fi
    fi

    log_success "GRUB configured"
}

# =============================================================================
# Create EFI boot image
# =============================================================================
create_efi_image() {
    log_step "Creating EFI boot image..."

    local efi_img="${ISO_ROOT}/boot/efiboot.img"

    # Create FAT image for EFI
    dd if=/dev/zero of="${efi_img}" bs=1M count=10 2>/dev/null

    if command -v mkfs.vfat &>/dev/null; then
        mkfs.vfat "${efi_img}" 2>/dev/null
    elif command -v mformat &>/dev/null; then
        mformat -i "${efi_img}" ::
    else
        log_warn "No FAT formatter found"
        return 1
    fi

    # Copy EFI files using mtools
    if command -v mcopy &>/dev/null; then
        mmd -i "${efi_img}" ::/EFI 2>/dev/null || true
        mmd -i "${efi_img}" ::/EFI/BOOT 2>/dev/null || true
        mcopy -i "${efi_img}" "${ISO_ROOT}/EFI/BOOT/BOOTX64.EFI" ::/EFI/BOOT/ 2>/dev/null || true

        if [[ -d "${ISO_ROOT}/EFI/raven" ]]; then
            mmd -i "${efi_img}" ::/EFI/raven 2>/dev/null || true
            for f in "${ISO_ROOT}/EFI/raven"/*; do
                [[ -f "$f" ]] && mcopy -i "${efi_img}" "$f" ::/EFI/raven/ 2>/dev/null || true
            done
        fi
        log_success "EFI image created"
    else
        log_warn "mtools not found, EFI boot may not work"
    fi
}

# =============================================================================
# Create ISO metadata
# =============================================================================
create_iso_info() {
    log_step "Creating ISO metadata..."

    cat > "${ISO_ROOT}/raven/os-release" << EOF
NAME="Raven Linux"
PRETTY_NAME="Raven Linux ${RAVEN_VERSION}"
ID=raven
VERSION="${RAVEN_VERSION}"
VERSION_ID="${RAVEN_VERSION}"
BUILD_ID=rolling
ANSI_COLOR="38;2;23;147;209"
HOME_URL="https://ravenlinux.org"
LOGO=raven-logo
EOF

    echo "${RAVEN_VERSION}" > "${ISO_ROOT}/raven/version"

    log_success "ISO metadata created"
}

# =============================================================================
# Generate ISO
# =============================================================================
generate_iso() {
    log_step "Generating ISO image..."

    # Try full hybrid ISO first
    if xorriso -as mkisofs \
        -iso-level 3 \
        -full-iso9660-filenames \
        -volid "${ISO_LABEL}" \
        -output "${ISO_OUTPUT}" \
        -eltorito-boot boot/grub/i386-pc/eltorito.img \
        -no-emul-boot \
        -boot-load-size 4 \
        -boot-info-table \
        --grub2-boot-info \
        --grub2-mbr /usr/lib/grub/i386-pc/boot_hybrid.img \
        -eltorito-alt-boot \
        -e boot/efiboot.img \
        -no-emul-boot \
        -isohybrid-gpt-basdat \
        -graft-points \
        "${ISO_ROOT}" \
        /boot/grub/i386-pc=/usr/lib/grub/i386-pc \
        2>&1 | tee "${LOGS_DIR}/xorriso.log"; then
        log_success "Hybrid ISO created"
    else
        # Fallback to simpler EFI-only ISO
        log_warn "Hybrid ISO failed, creating EFI-only ISO..."
        xorriso -as mkisofs \
            -R -J -joliet-long \
            -volid "${ISO_LABEL}" \
            -output "${ISO_OUTPUT}" \
            -eltorito-alt-boot \
            -e boot/efiboot.img \
            -no-emul-boot \
            "${ISO_ROOT}" 2>&1 | tee "${LOGS_DIR}/xorriso.log"
    fi

    # Generate checksums
    sha256sum "${ISO_OUTPUT}" > "${ISO_OUTPUT}.sha256"
    md5sum "${ISO_OUTPUT}" > "${ISO_OUTPUT}.md5"

    log_success "ISO generated: ${ISO_OUTPUT}"
}

# =============================================================================
# Summary
# =============================================================================
print_summary() {
    local iso_size
    iso_size=$(du -h "${ISO_OUTPUT}" 2>/dev/null | cut -f1 || echo "unknown")

    echo ""
    echo -e "${CYAN}=========================================="
    echo "  RavenLinux ISO Build Complete"
    echo "==========================================${NC}"
    echo ""
    echo "  ISO:      ${ISO_OUTPUT}"
    echo "  Size:     ${iso_size}"
    echo "  Version:  ${RAVEN_VERSION}"
    echo "  Arch:     ${RAVEN_ARCH}"
    echo ""

    if [[ -f "${PACKAGES_DIR}/boot/raven-boot.efi" ]]; then
        echo "  Bootloader: RavenBoot (UEFI)"
    else
        echo "  Bootloader: GRUB"
    fi

    echo ""
    echo "  Test in QEMU (UEFI):"
    echo "    qemu-system-x86_64 -cdrom ${ISO_OUTPUT} -m 4G \\"
    echo "      -bios /usr/share/edk2-ovmf/x64/OVMF_CODE.4m.fd -enable-kvm"
    echo ""
    echo "  Test in QEMU (BIOS):"
    echo "    qemu-system-x86_64 -cdrom ${ISO_OUTPUT} -m 4G -enable-kvm"
    echo ""
    echo "  Write to USB:"
    echo "    sudo dd if=${ISO_OUTPUT} of=/dev/sdX bs=4M status=progress"
    echo ""
}

# =============================================================================
# Main
# =============================================================================
main() {
    echo ""
    echo "=========================================="
    echo "  Stage 4: Generating ISO Image"
    echo "=========================================="
    echo ""

    mkdir -p "${LOGS_DIR}"

    check_deps
    setup_iso_structure
    create_live_init
    copy_boot_files
    create_squashfs
    setup_ravenboot || true  # Continue even if RavenBoot not available
    setup_grub
    create_efi_image
    create_iso_info
    generate_iso
    print_summary

    log_success "Stage 4 complete!"
}

# Run main function
main "$@"
