#!/bin/bash
# Stage 4: Generate bootable ISO image
# Creates a bootable RavenLinux ISO with live environment

set -euo pipefail

ISO_DIR="${RAVEN_BUILD}/iso"
ISO_ROOT="${ISO_DIR}/root"
ISO_OUTPUT="${RAVEN_ROOT}/raven-${RAVEN_VERSION}-${RAVEN_ARCH}.iso"

setup_iso_structure() {
    log_info "Setting up ISO directory structure..."

    rm -rf "${ISO_DIR}"
    mkdir -p "${ISO_ROOT}"/{boot/grub,EFI/BOOT,live,raven}

    log_success "ISO directory structure created"
}

copy_system_files() {
    log_info "Copying system files to ISO..."

    # Copy the built system
    cp -a "${SYSROOT_DIR}"/* "${ISO_ROOT}/live/"

    # Create squashfs for the live system
    log_info "Creating squashfs filesystem..."
    mksquashfs "${ISO_ROOT}/live" "${ISO_ROOT}/raven/filesystem.squashfs" \
        -comp zstd -Xcompression-level 19 \
        -b 1M -no-duplicates

    # Remove the live directory after squashing
    rm -rf "${ISO_ROOT}/live"

    log_success "System files copied"
}

setup_bootloader() {
    log_info "Setting up bootloader..."

    # Copy kernel and initramfs
    cp "${SYSROOT_DIR}/boot/vmlinuz"* "${ISO_ROOT}/boot/vmlinuz" 2>/dev/null || \
        log_warn "Kernel not found, using placeholder"

    cp "${SYSROOT_DIR}/boot/initramfs"* "${ISO_ROOT}/boot/initramfs.img" 2>/dev/null || \
        log_warn "Initramfs not found, will need to be generated"

    # GRUB configuration for BIOS boot
    cat > "${ISO_ROOT}/boot/grub/grub.cfg" << 'EOF'
set default=0
set timeout=5

insmod all_video
insmod gfxterm
terminal_output gfxterm

set gfxmode=auto
set gfxpayload=keep

menuentry "RavenLinux Live" {
    linux /boot/vmlinuz root=live:CDLABEL=RAVEN_LIVE rd.live.image quiet splash
    initrd /boot/initramfs.img
}

menuentry "RavenLinux Live (Safe Mode)" {
    linux /boot/vmlinuz root=live:CDLABEL=RAVEN_LIVE rd.live.image nomodeset
    initrd /boot/initramfs.img
}

menuentry "RavenLinux Install" {
    linux /boot/vmlinuz root=live:CDLABEL=RAVEN_LIVE rd.live.image raven.installer quiet
    initrd /boot/initramfs.img
}

menuentry "Memory Test" {
    linux16 /boot/memtest86+
}
EOF

    # EFI bootloader
    if command -v grub-mkstandalone &>/dev/null; then
        grub-mkstandalone \
            --format=x86_64-efi \
            --output="${ISO_ROOT}/EFI/BOOT/BOOTX64.EFI" \
            --locales="" \
            --fonts="" \
            "boot/grub/grub.cfg=${ISO_ROOT}/boot/grub/grub.cfg"
    else
        log_warn "grub-mkstandalone not found, EFI boot may not work"
    fi

    log_success "Bootloader configured"
}

create_iso_info() {
    log_info "Creating ISO metadata..."

    # OS release info for the live environment
    cat > "${ISO_ROOT}/raven/os-release" << EOF
NAME="Raven Linux"
PRETTY_NAME="Raven Linux ${RAVEN_VERSION}"
ID=raven
VERSION="${RAVEN_VERSION}"
VERSION_ID="${RAVEN_VERSION}"
BUILD_ID=rolling
ANSI_COLOR="38;2;23;147;209"
HOME_URL="https://ravenlinux.org"
DOCUMENTATION_URL="https://docs.ravenlinux.org"
SUPPORT_URL="https://github.com/ravenlinux/ravenlinux/issues"
BUG_REPORT_URL="https://github.com/ravenlinux/ravenlinux/issues"
LOGO=raven-logo
EOF

    # Version file
    echo "${RAVEN_VERSION}" > "${ISO_ROOT}/raven/version"

    # Checksums
    (cd "${ISO_ROOT}" && find . -type f -exec sha256sum {} \;) > "${ISO_ROOT}/raven/checksums.sha256"

    log_success "ISO metadata created"
}

generate_iso() {
    log_info "Generating ISO image..."

    # Check for required tools
    if ! command -v xorriso &>/dev/null; then
        log_error "xorriso not found. Install it with: pacman -S libisoburn"
    fi

    # Create the ISO
    xorriso -as mkisofs \
        -iso-level 3 \
        -full-iso9660-filenames \
        -volid "RAVEN_LIVE" \
        -eltorito-boot boot/grub/i386-pc/eltorito.img \
        -no-emul-boot \
        -boot-load-size 4 \
        -boot-info-table \
        --eltorito-catalog boot/grub/boot.cat \
        --grub2-boot-info \
        --grub2-mbr /usr/lib/grub/i386-pc/boot_hybrid.img \
        -eltorito-alt-boot \
        -e EFI/efiboot.img \
        -no-emul-boot \
        -append_partition 2 0xef "${ISO_ROOT}/EFI/efiboot.img" \
        -output "${ISO_OUTPUT}" \
        -graft-points \
        "${ISO_ROOT}" \
        /boot/grub/i386-pc=/usr/lib/grub/i386-pc \
        2>&1 | tee "${LOGS_DIR}/iso-generation.log" || {
            # Fallback to simpler ISO generation
            log_warn "Full ISO generation failed, trying simpler method..."
            genisoimage -o "${ISO_OUTPUT}" \
                -b boot/grub/i386-pc/eltorito.img \
                -no-emul-boot \
                -boot-load-size 4 \
                -boot-info-table \
                -V "RAVEN_LIVE" \
                -R -J \
                "${ISO_ROOT}" 2>&1 | tee -a "${LOGS_DIR}/iso-generation.log"
        }

    log_success "ISO generated: ${ISO_OUTPUT}"
}

calculate_checksums() {
    log_info "Calculating ISO checksums..."

    cd "$(dirname "${ISO_OUTPUT}")"

    sha256sum "$(basename "${ISO_OUTPUT}")" > "${ISO_OUTPUT}.sha256"
    md5sum "$(basename "${ISO_OUTPUT}")" > "${ISO_OUTPUT}.md5"

    log_success "Checksums generated"
}

print_summary() {
    local iso_size
    iso_size=$(du -h "${ISO_OUTPUT}" 2>/dev/null | cut -f1 || echo "unknown")

    echo
    echo "========================================"
    echo "  RavenLinux ISO Build Complete"
    echo "========================================"
    echo "  ISO:      ${ISO_OUTPUT}"
    echo "  Size:     ${iso_size}"
    echo "  Version:  ${RAVEN_VERSION}"
    echo "  Arch:     ${RAVEN_ARCH}"
    echo "========================================"
    echo
    echo "To test in QEMU:"
    echo "  qemu-system-x86_64 -cdrom ${ISO_OUTPUT} -m 4G -enable-kvm"
    echo
    echo "To write to USB:"
    echo "  sudo dd if=${ISO_OUTPUT} of=/dev/sdX bs=4M status=progress"
    echo
}

# Main Stage 4 execution
setup_iso_structure
copy_system_files
setup_bootloader
create_iso_info
generate_iso
calculate_checksums
print_summary

log_success "=== Stage 4 Complete: ISO Generated ==="
