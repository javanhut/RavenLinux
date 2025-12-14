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

# =============================================================================
# Environment Setup (with defaults for standalone execution)
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${RAVEN_ROOT:-$(dirname "$(dirname "$SCRIPT_DIR")")}"
BUILD_DIR="${RAVEN_BUILD:-${PROJECT_ROOT}/build}"
SYSROOT_DIR="${SYSROOT_DIR:-${BUILD_DIR}/sysroot}"
PACKAGES_DIR="${PACKAGES_DIR:-${BUILD_DIR}/packages}"
ISO_DIR="${BUILD_DIR}/iso"
ISO_ROOT="${ISO_DIR}/iso-root"
LOGS_DIR="${LOGS_DIR:-${BUILD_DIR}/logs}"

# Version info
RAVEN_VERSION="${RAVEN_VERSION:-2025.12}"
RAVEN_ARCH="${RAVEN_ARCH:-x86_64}"
ISO_LABEL="RAVEN_LIVE"
ISO_OUTPUT="${PROJECT_ROOT}/raven-${RAVEN_VERSION}-${RAVEN_ARCH}.iso"

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
    log_error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }
    log_step() { echo -e "${CYAN}[STEP]${NC} $1"; }
fi

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
export HOME=/root
export TERM=linux
export LANG=en_US.UTF-8
export PS1='[\u@raven-linux]# '

# Mount essential filesystems if not already mounted
mountpoint -q /proc || mount -t proc proc /proc
mountpoint -q /sys || mount -t sysfs sysfs /sys
mountpoint -q /dev || mount -t devtmpfs devtmpfs /dev 2>/dev/null || mount -t tmpfs tmpfs /dev
mkdir -p /dev/pts /dev/shm /tmp /run
mountpoint -q /dev/pts || mount -t devpts devpts /dev/pts
mountpoint -q /dev/shm || mount -t tmpfs tmpfs /dev/shm
mountpoint -q /tmp || mount -t tmpfs tmpfs /tmp
mountpoint -q /run || mount -t tmpfs tmpfs /run

# Set hostname
hostname raven-linux 2>/dev/null || true

# Suppress kernel messages
dmesg -n 1 2>/dev/null || true

# Clear screen and show banner
clear 2>/dev/null || printf '\033[2J\033[H'
printf '\033[1;36m'
cat << 'BANNER'

  ╔═══════════════════════════════════════════════════════════════════════════╗
  ║                                                                           ║
  ║    ██████╗  █████╗ ██╗   ██╗███████╗███╗   ██╗    ██╗     ██╗███╗   ██╗   ║
  ║    ██╔══██╗██╔══██╗██║   ██║██╔════╝████╗  ██║    ██║     ██║████╗  ██║   ║
  ║    ██████╔╝███████║██║   ██║█████╗  ██╔██╗ ██║    ██║     ██║██╔██╗ ██║   ║
  ║    ██╔══██╗██╔══██║╚██╗ ██╔╝██╔══╝  ██║╚██╗██║    ██║     ██║██║╚██╗██║   ║
  ║    ██║  ██║██║  ██║ ╚████╔╝ ███████╗██║ ╚████║    ███████╗██║██║ ╚████║   ║
  ║    ╚═╝  ╚═╝╚═╝  ╚═╝  ╚═══╝  ╚══════╝╚═╝  ╚═══╝    ╚══════╝╚═╝╚═╝  ╚═══╝   ║
  ║                                                                           ║
  ║                    A Developer-Focused Linux Distribution                 ║
  ║                                                                           ║
  ╚═══════════════════════════════════════════════════════════════════════════╝

BANNER
printf '\033[0m'
printf '\033[1;33m'
echo "                              Version 2025.12"
printf '\033[0m'
echo ""
printf '\033[1;37m'
echo "  ┌─────────────────────────────────────────────────────────────────────────┐"
echo "  │  BUILT-IN TOOLS:                                                        │"
echo "  │    vem        - Text editor           wifi       - WiFi manager         │"
echo "  │    carrion    - Programming language  rvn        - Package manager      │"
echo "  │    ivaldi     - Version control       raven-install - System installer  │"
echo "  └─────────────────────────────────────────────────────────────────────────┘"
printf '\033[0m'
echo ""
printf '\033[0;32m'
echo "  Type 'poweroff' to shutdown, 'reboot' to restart"
printf '\033[0m'
echo ""

cmdline="$(cat /proc/cmdline 2>/dev/null || true)"

start_shell_loop() {
    cd /root
    while true; do
        if [ -x /bin/bash ]; then
            /bin/bash --login -i
        elif [ -x /bin/sh ]; then
            /bin/sh -l
        else
            echo "No shell available! Sleeping..."
            sleep 10
        fi
        echo ""
        echo "Shell exited. Restarting..."
        sleep 1
    done
}

if echo "$cmdline" | grep -qE '(^| )raven\.graphics=wayland($| )'; then
    echo ""
    echo "Starting Wayland graphics..."

    if [ -x /bin/raven-wayland-session ]; then
        if command -v openvt >/dev/null 2>&1; then
            if openvt -c 1 -s -f -- /bin/raven-wayland-session; then
                :
            else
                echo "Wayland session exited; falling back to shell."
            fi
        elif /bin/raven-wayland-session; then
            :
        else
            echo "Wayland session exited; falling back to shell."
        fi
    else
        echo "raven-wayland-session not found; falling back to shell."
    fi
fi

start_shell_loop
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
# Install packages to sysroot
# =============================================================================
install_packages_to_sysroot() {
    log_step "Installing packages to sysroot..."

    mkdir -p "${SYSROOT_DIR}/bin"

    # Copy all built packages from packages/bin
    if [[ -d "${PACKAGES_DIR}/bin" ]]; then
        for pkg in "${PACKAGES_DIR}/bin"/*; do
            [[ -f "$pkg" ]] || continue
            local name
            name="$(basename "$pkg")"
            cp "$pkg" "${SYSROOT_DIR}/bin/"
            chmod +x "${SYSROOT_DIR}/bin/${name}"
            log_info "  Installed ${name}"
        done
    fi

    # Create raven-install symlink for the installer
    if [[ -f "${SYSROOT_DIR}/bin/raven-installer" ]]; then
        ln -sf raven-installer "${SYSROOT_DIR}/bin/raven-install"
        log_info "  Created raven-install symlink"
    fi

    # Copy desktop entries if present
    if [[ -d "${PROJECT_ROOT}/configs/desktop" ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/share/applications"
        cp "${PROJECT_ROOT}/configs/desktop"/*.desktop "${SYSROOT_DIR}/usr/share/applications/" 2>/dev/null || true
    fi

    # Copy session helper scripts
    if [[ -f "${PROJECT_ROOT}/configs/raven-wayland-session" ]]; then
        cp "${PROJECT_ROOT}/configs/raven-wayland-session" "${SYSROOT_DIR}/bin/raven-wayland-session" 2>/dev/null || true
        chmod +x "${SYSROOT_DIR}/bin/raven-wayland-session" 2>/dev/null || true
    fi

    # Ensure shared library dependencies for newly installed binaries are present.
    log_info "Copying runtime libraries for sysroot binaries..."
    for bin in "${SYSROOT_DIR}"/bin/* "${SYSROOT_DIR}"/sbin/*; do
        [[ -f "$bin" && -x "$bin" && ! -L "$bin" ]] || continue
        if file "$bin" 2>/dev/null | grep -q "statically linked"; then
            continue
        fi
        timeout 2 ldd "$bin" 2>/dev/null | grep -o '/[^ ]*' | while read -r lib; do
            [[ -z "$lib" || ! -f "$lib" ]] && continue
            dest="${SYSROOT_DIR}${lib}"
            if [[ ! -f "$dest" ]]; then
                mkdir -p "$(dirname "$dest")"
                cp -L "$lib" "$dest" 2>/dev/null || true
            fi
        done || true
    done

    log_success "Packages installed to sysroot"
}

# =============================================================================
# Create squashfs filesystem
# =============================================================================
create_squashfs() {
    log_step "Creating squashfs filesystem..."

    # Add live init if not present
    [[ -f "${SYSROOT_DIR}/init" ]] || create_live_init

    # Install packages to sysroot before creating squashfs
    install_packages_to_sysroot

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
kernel = "\\EFI\\raven\\vmlinuz"
initrd = "\\EFI\\raven\\initramfs.img"
cmdline = "rdinit=/init quiet loglevel=3 console=tty0 console=ttyS0,115200"
type = linux-efi

[entry]
name = "Raven Linux Live (Wayland)"
kernel = "\\EFI\\raven\\vmlinuz"
initrd = "\\EFI\\raven\\initramfs.img"
cmdline = "rdinit=/init quiet loglevel=3 raven.graphics=wayland raven.wayland=weston console=tty0 console=ttyS0,115200"
type = linux-efi

[entry]
name = "Raven Linux Live (Wayland - Raven Compositor WIP)"
kernel = "\\EFI\\raven\\vmlinuz"
initrd = "\\EFI\\raven\\initramfs.img"
cmdline = "rdinit=/init quiet loglevel=3 raven.graphics=wayland raven.wayland=raven console=tty0 console=ttyS0,115200"
type = linux-efi

[entry]
name = "Raven Linux Live (Verbose)"
kernel = "\\EFI\\raven\\vmlinuz"
initrd = "\\EFI\\raven\\initramfs.img"
cmdline = "rdinit=/init console=tty0 console=ttyS0,115200"
type = linux-efi

[entry]
name = "Raven Linux (Recovery)"
kernel = "\\EFI\\raven\\vmlinuz"
initrd = "\\EFI\\raven\\initramfs.img"
cmdline = "rdinit=/init single console=tty0 console=ttyS0,115200"
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
    linux /boot/vmlinuz rdinit=/init quiet loglevel=3 console=tty0 console=ttyS0,115200
    initrd /boot/initramfs.img
}

menuentry "Raven Linux Live (Wayland)" --class raven {
    linux /boot/vmlinuz rdinit=/init quiet loglevel=3 raven.graphics=wayland raven.wayland=weston console=tty0 console=ttyS0,115200
    initrd /boot/initramfs.img
}

menuentry "Raven Linux Live (Wayland - Raven Compositor WIP)" --class raven {
    linux /boot/vmlinuz rdinit=/init quiet loglevel=3 raven.graphics=wayland raven.wayland=raven console=tty0 console=ttyS0,115200
    initrd /boot/initramfs.img
}

menuentry "Raven Linux Live (Verbose)" --class raven {
    linux /boot/vmlinuz rdinit=/init console=tty0 console=ttyS0,115200
    initrd /boot/initramfs.img
}

menuentry "Raven Linux (Recovery)" --class raven {
    linux /boot/vmlinuz rdinit=/init single console=tty0 console=ttyS0,115200
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

    # Calculate size needed: kernel + initramfs + bootloader + some headroom
    local kernel_size=0
    local initrd_size=0
    [[ -f "${ISO_ROOT}/boot/vmlinuz" ]] && kernel_size=$(stat -c%s "${ISO_ROOT}/boot/vmlinuz")
    [[ -f "${ISO_ROOT}/boot/initramfs.img" ]] && initrd_size=$(stat -c%s "${ISO_ROOT}/boot/initramfs.img")

    # Size in MB: (kernel + initramfs + 5MB headroom) / 1MB, minimum 40MB
    local size_mb=$(( (kernel_size + initrd_size + 5*1024*1024) / (1024*1024) ))
    [[ $size_mb -lt 40 ]] && size_mb=40

    log_info "Creating ${size_mb}MB EFI boot image..."

    # Create FAT image for EFI
    dd if=/dev/zero of="${efi_img}" bs=1M count=${size_mb} 2>/dev/null

    if command -v mkfs.vfat &>/dev/null; then
        mkfs.vfat "${efi_img}" 2>/dev/null
    elif command -v mformat &>/dev/null; then
        mformat -i "${efi_img}" ::
    else
        log_warn "No FAT formatter found"
        return 1
    fi

    # Copy files using mtools
    if command -v mcopy &>/dev/null; then
        # Create directory structure
        mmd -i "${efi_img}" ::/EFI 2>/dev/null || true
        mmd -i "${efi_img}" ::/EFI/BOOT 2>/dev/null || true
        mmd -i "${efi_img}" ::/EFI/raven 2>/dev/null || true
        mmd -i "${efi_img}" ::/boot 2>/dev/null || true
        mmd -i "${efi_img}" ::/boot/grub 2>/dev/null || true

        # Copy bootloader (RavenBoot or GRUB)
        mcopy -i "${efi_img}" "${ISO_ROOT}/EFI/BOOT/BOOTX64.EFI" ::/EFI/BOOT/ 2>/dev/null || true
        log_info "  Copied EFI bootloader"

        # Copy RavenBoot config if present
        if [[ -f "${ISO_ROOT}/EFI/raven/boot.conf" ]]; then
            mcopy -i "${efi_img}" "${ISO_ROOT}/EFI/raven/boot.conf" ::/EFI/raven/ 2>/dev/null || true
            log_info "  Copied RavenBoot config"
        fi

        # Copy GRUB config as fallback
        if [[ -f "${ISO_ROOT}/boot/grub/grub.cfg" ]]; then
            mcopy -i "${efi_img}" "${ISO_ROOT}/boot/grub/grub.cfg" ::/boot/grub/ 2>/dev/null || true
        fi

        # Copy kernel and initramfs to EFI/raven/ for RavenBoot
        if [[ -f "${ISO_ROOT}/boot/vmlinuz" ]]; then
            mcopy -i "${efi_img}" "${ISO_ROOT}/boot/vmlinuz" ::/EFI/raven/ 2>/dev/null || true
            log_info "  Copied kernel to EFI image"
        fi
        if [[ -f "${ISO_ROOT}/boot/initramfs.img" ]]; then
            mcopy -i "${efi_img}" "${ISO_ROOT}/boot/initramfs.img" ::/EFI/raven/ 2>/dev/null || true
            log_info "  Copied initramfs to EFI image"
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
        echo "  Bootloader: GRUB (UEFI)"
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
    setup_grub  # GRUB as fallback for BIOS
    create_efi_image
    create_iso_info
    generate_iso
    print_summary

    log_success "Stage 4 complete!"
}

# Run main function
main "$@"
