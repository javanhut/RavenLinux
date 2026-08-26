#!/bin/bash
# =============================================================================
# RavenLinux Stage 4: Generate ISO Image
# =============================================================================
# Creates a bootable ISO image with:
# - RavenBoot UEFI bootloader (primary)
# - GRUB fallback for BIOS systems
# - Squashfs compressed root filesystem
# - Live boot into a console shell on tty1

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
# The EFI System Partition image. Deliberately outside ISO_ROOT: it is attached
# to the image as an appended GPT partition rather than as a file in the
# ISO9660 tree, so putting it under ISO_ROOT would ship a second 46MB copy.
EFI_IMG="${ISO_DIR}/efiboot.img"
# GPT partition type for an EFI System Partition. Firmware booting removable
# media looks for a partition of this type and loads /EFI/BOOT/BOOTX64.EFI from
# it; El Torito is only consulted for optical media. An ISO with no partition
# table therefore boots fine from a DVD or a VM's virtual CD and not at all
# from a UEFI USB stick, which is what -append_partition fixes.
ESP_TYPE_GUID="C12A7328-F81F-11D2-BA4B-00A0C93EC93B"
LOGS_DIR="${LOGS_DIR:-${BUILD_DIR}/logs}"

# Version info
RAVEN_VERSION="${RAVEN_VERSION:-2026.08}"
RAVEN_ARCH="${RAVEN_ARCH:-x86_64}"
ISO_LABEL="RAVENLINUX"
ISO_OUTPUT="${PROJECT_ROOT}/raven-${RAVEN_VERSION}-${RAVEN_ARCH}.iso"

# =============================================================================
# Logging (use shared library or define fallbacks)
# =============================================================================

# The rootfs layout is usr-merged (/bin, /sbin, /lib, /lib64 are symlinks into
# /usr). See scripts/lib/usrmerge.sh for why, and for the rule every install
# site in this file has to follow.
if [[ -f "${PROJECT_ROOT}/scripts/lib/usrmerge.sh" ]]; then
    # shellcheck disable=SC1091
    source "${PROJECT_ROOT}/scripts/lib/usrmerge.sh"
fi

# Presence, modes and accounts -- the half usrmerge.sh does not cover. An
# ABSENT path is not a merge error and not a package conflict, so a rootfs
# can pass check-layout.sh while missing /var/empty entirely (sshd then
# refuses to start). See scripts/lib/skeleton.sh.
if [[ -f "${PROJECT_ROOT}/scripts/lib/skeleton.sh" ]]; then
    # shellcheck disable=SC1091
    source "${PROJECT_ROOT}/scripts/lib/skeleton.sh"
fi

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
# Create the live-root hand-off in the sysroot
# =============================================================================
# The mounted live root uses the same PID 1 and service graph as an installed
# system. Keeping a second shell-based supervisor here caused the desktop,
# wireless, shutdown, and raven-rc paths to behave differently on the ISO.
# The initramfs still performs discovery/mounting; after switch_root this small
# hand-off makes raven-init responsible for the actual RavenLinux system.
create_managed_live_init() {
    log_step "Configuring raven-init as PID 1..."

    [[ -x "${SYSROOT_DIR}/usr/bin/raven-init" ]] || {
        log_error "${SYSROOT_DIR}/usr/bin/raven-init is missing; run stage2 first"
        return 1
    }

    cat > "${SYSROOT_DIR}/init" << 'EOF'
#!/bin/sh
# RavenLinux live-root hand-off. Early userspace has already mounted the
# squashfs/overlay and switch_root'ed here; from this point live and installed
# systems deliberately share the same init and service manager.
exec /sbin/raven-init
EOF
    chmod 0755 "${SYSROOT_DIR}/init"
    # Same-directory alias. /sbin/init -- which is what the kernel and the
    # installer name -- resolves here through /sbin -> usr/bin.
    ln -sf raven-init "${SYSROOT_DIR}/usr/bin/init"

    log_success "raven-init configured for live and installed boots"
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

    stage_boot_payload_in_sysroot "${kernel}"

    log_success "Boot files copied"
}

# The kernel, initramfs and bootloader, placed inside the sysroot so they end up
# in the squashfs and therefore on every installed system.
#
# It costs about 40MB of ISO, and buys two things. raven-install has a source
# for the boot payload even when the live media is not reachable -- installing
# from an already-installed machine, say. And the installed system owns a copy
# of its own kernel at /boot, rather than only on the ESP, so reinstalling the
# bootloader later does not require the ISO that produced it.
stage_boot_payload_in_sysroot() {
    local kernel="$1"

    mkdir -p "${SYSROOT_DIR}/boot" "${SYSROOT_DIR}/usr/share/raven/boot"

    if [[ -n "${kernel}" && -f "${kernel}" ]]; then
        cp "${kernel}" "${SYSROOT_DIR}/boot/vmlinuz"
        log_info "  Staged kernel in the sysroot at /boot/vmlinuz"
    fi

    if [[ -f "${BUILD_DIR}/initramfs-raven.img" ]]; then
        cp "${BUILD_DIR}/initramfs-raven.img" "${SYSROOT_DIR}/boot/initramfs.img"
        log_info "  Staged initramfs in the sysroot at /boot/initramfs.img"
    fi

    # RavenBoot is 47KB, so this one is free. Not under /boot: that directory
    # is the kernel's, and on an installed system /boot/efi is a mount point.
    if [[ -f "${PACKAGES_DIR}/boot/raven-boot.efi" ]]; then
        cp "${PACKAGES_DIR}/boot/raven-boot.efi" "${SYSROOT_DIR}/usr/share/raven/boot/raven-boot.efi"
        log_info "  Staged RavenBoot at /usr/share/raven/boot/raven-boot.efi"
    else
        log_warn "  No RavenBoot binary to stage; raven-install will have to find one on the media"
    fi
}

# =============================================================================
# Copy kernel modules into sysroot (needed for DRM/input/network drivers)
# =============================================================================
copy_kernel_modules() {
    log_step "Copying kernel modules..."

    local modules_root="${BUILD_DIR}/kernel/lib/modules"
    if [[ ! -d "${modules_root}" ]]; then
        log_warn "Kernel modules not found at ${modules_root}; skipping"
        return 0
    fi

    local release
    release="$(find "${modules_root}" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' 2>/dev/null | sort -V | tail -n 1)"
    if [[ -z "${release}" ]]; then
        log_warn "No kernel module directories found in ${modules_root}; skipping"
        return 0
    fi

    mkdir -p "${SYSROOT_DIR}/usr/lib/modules"
    rm -rf "${SYSROOT_DIR}/usr/lib/modules/${release}" 2>/dev/null || true
    cp -a "${modules_root}/${release}" "${SYSROOT_DIR}/usr/lib/modules/" 2>/dev/null || true

    if [[ -d "${SYSROOT_DIR}/usr/lib/modules/${release}" ]]; then
        log_info "  Copied /lib/modules/${release}"

        # Generate modules.dep/modules.alias so udev + modprobe can auto-load drivers.
        if command -v depmod &>/dev/null; then
            if depmod -b "${SYSROOT_DIR}" "${release}" 2>/dev/null; then
                log_info "  Ran depmod for ${release}"
            else
                log_warn "depmod failed for ${release}; kernel module auto-loading may not work"
            fi
        else
            log_warn "depmod not found on host; kernel module auto-loading may not work"
        fi

        log_success "Kernel modules copied"
    else
        log_warn "Failed to copy kernel modules into sysroot"
    fi
}

# =============================================================================
# Install packages to sysroot
# =============================================================================
install_packages_to_sysroot() {
    log_step "Installing packages to sysroot..."

    mkdir -p "${SYSROOT_DIR}/usr/bin"

    # Copy all built packages from packages/bin
    if [[ -d "${PACKAGES_DIR}/bin" ]]; then
        for pkg in "${PACKAGES_DIR}/bin"/*; do
            [[ -f "$pkg" ]] || continue
            local name
            name="$(basename "$pkg")"
            cp "$pkg" "${SYSROOT_DIR}/usr/bin/"
            chmod +x "${SYSROOT_DIR}/usr/bin/${name}"
            log_info "  Installed ${name}"
        done
    fi

    # Fontconfig + fonts (the console font; a missing config causes warnings).
    if [[ -d "/etc/fonts" ]]; then
        mkdir -p "${SYSROOT_DIR}/etc/fonts"
        cp -a "/etc/fonts/." "${SYSROOT_DIR}/etc/fonts/" 2>/dev/null || true
        log_info "  Copied /etc/fonts"
    elif [[ -f "${PROJECT_ROOT}/configs/fontconfig/fonts.conf" ]]; then
        mkdir -p "${SYSROOT_DIR}/etc/fonts"
        cp "${PROJECT_ROOT}/configs/fontconfig/fonts.conf" "${SYSROOT_DIR}/etc/fonts/fonts.conf" 2>/dev/null || true
        log_info "  Added minimal /etc/fonts/fonts.conf"
    fi
    if [[ -d "/usr/share/fontconfig" ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/share/fontconfig"
        cp -a "/usr/share/fontconfig/." "${SYSROOT_DIR}/usr/share/fontconfig/" 2>/dev/null || true
        log_info "  Copied /usr/share/fontconfig"
    fi
    # Copy fonts from repo only (avoid pulling host system fonts).
    local font_count
    local font_src
    font_src="${PROJECT_ROOT}/fonts"
    if [[ -d "${font_src}" ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/share/fonts"
        find "${font_src}" -type f \( -iname "*.ttf" -o -iname "*.otf" \) \
            -exec cp {} "${SYSROOT_DIR}/usr/share/fonts/" \; 2>/dev/null || true
        font_count=$(find "${SYSROOT_DIR}/usr/share/fonts" -type f 2>/dev/null | wc -l)
        log_info "  Copied custom fonts (${font_count} files)"
    else
        log_warn "  No custom fonts directory found at ${font_src}; skipping font copy"
    fi
    mkdir -p "${SYSROOT_DIR}/var/cache/fontconfig" 2>/dev/null || true

    install_console_font
    install_udev_helper

    # Ensure shared library dependencies for newly installed binaries are present.
    log_info "Copying runtime libraries for sysroot binaries..."
    for bin in "${SYSROOT_DIR}"/usr/bin/*; do
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
# Clean up sysroot to reduce ISO size
# =============================================================================
cleanup_sysroot() {
    log_step "Cleaning up sysroot to reduce size..."

    local before_size
    before_size=$(du -sh "${SYSROOT_DIR}" 2>/dev/null | cut -f1)

    # Remove unnecessary files to reduce squashfs size
    rm -rf "${SYSROOT_DIR}/usr/share/doc" 2>/dev/null || true
    rm -rf "${SYSROOT_DIR}/usr/share/man" 2>/dev/null || true
    rm -rf "${SYSROOT_DIR}/usr/share/info" 2>/dev/null || true
    rm -rf "${SYSROOT_DIR}/usr/share/locale"/*/ 2>/dev/null || true
    rm -rf "${SYSROOT_DIR}/usr/include" 2>/dev/null || true
    rm -rf "${SYSROOT_DIR}/usr/share/gtk-doc" 2>/dev/null || true
    rm -rf "${SYSROOT_DIR}/usr/share/help" 2>/dev/null || true
    
    # Remove static libraries
    find "${SYSROOT_DIR}" -name "*.a" -delete 2>/dev/null || true
    
    # Remove .pyc files
    find "${SYSROOT_DIR}" -name "*.pyc" -delete 2>/dev/null || true
    find "${SYSROOT_DIR}" -name "__pycache__" -type d -exec rm -rf {} + 2>/dev/null || true
    
    # Strip binaries (reduce size significantly)
    # One start point. /bin, /sbin and /usr/sbin are symlinks onto /usr/bin,
    # and `find` given a symlink as a start point without -H/-L prints NOTHING
    # at all -- so naming them looked like four directories were covered while
    # two of the arguments were silently no-ops.
    find "${SYSROOT_DIR}/usr/bin" \
        -type f -executable 2>/dev/null | while read -r bin; do
        strip --strip-unneeded "$bin" 2>/dev/null || true
    done
    
    # Strip shared libraries
    find "${SYSROOT_DIR}" -name "*.so*" -type f 2>/dev/null | while read -r lib; do
        strip --strip-unneeded "$lib" 2>/dev/null || true
    done

    local after_size
    after_size=$(du -sh "${SYSROOT_DIR}" 2>/dev/null | cut -f1)
    log_info "  Sysroot size: ${before_size} -> ${after_size}"
    log_success "Sysroot cleaned up"
}

# =============================================================================
# Create squashfs filesystem
# =============================================================================
create_squashfs() {
    log_step "Creating squashfs filesystem..."

    # Add the live-root hand-off if not present.
    [[ -f "${SYSROOT_DIR}/init" ]] || create_managed_live_init

    # Install packages to sysroot before creating squashfs
    install_packages_to_sysroot

    # Clean up to reduce size
    cleanup_sysroot

    # cleanup_sysroot just deleted /usr/share/man and /usr/include, both of
    # which Arch's `filesystem` package ships -- and install_packages_to_sysroot
    # ran before it with a `mkdir -p` loop over host library paths. Re-applying
    # the skeleton here, as the last writer before the squashfs is sealed, is
    # what makes the shipped tree match the table rather than match whatever
    # the previous two functions happened to leave behind. Idempotent by
    # design; on an already-correct tree it logs nothing.
    if declare -F raven_skeleton_root >/dev/null 2>&1; then
        raven_skeleton_root "${SYSROOT_DIR}" || log_fatal "rootfs skeleton failed"
    else
        log_warn "scripts/lib/skeleton.sh not loaded; the image may be missing directories"
    fi

    # The real last gate. It used to run before create_squashfs, but both
    # install_packages_to_sysroot and cleanup_sysroot mutate the tree after
    # that point -- and install_packages_to_sysroot's ldd loop does
    # `mkdir -p "$(dirname "${SYSROOT_DIR}${lib}")"` on host paths such as
    # /lib64/ld-linux-x86-64.so.2, which recreates /lib64 as a real directory
    # and puts the ELF interpreter somewhere nothing in the booted image
    # reads. Checking before those ran verified a layout that no longer
    # existed by the time it was sealed.
    check_usrmerge_layout || return 1

    local pseudo="${LOGS_DIR}/squashfs.pseudo"
    : > "${pseudo}"
    # These are paths INSIDE the squashfs, not host paths. They must name the
    # real directory: `bin` is a symlink in the image now, and a pseudo-file
    # entry on a path that goes through a symlink does not apply -- the SUID
    # bit would silently not be set on sudo or su.
    [[ -e "${SYSROOT_DIR}/usr/bin/sudo" ]] && echo "usr/bin/sudo m 4755 0 0" >> "${pseudo}"
    [[ -e "${SYSROOT_DIR}/usr/bin/su" ]] && echo "usr/bin/su m 4755 0 0" >> "${pseudo}"
    [[ -e "${SYSROOT_DIR}/etc/shadow" ]] && echo "etc/shadow m 600 0 0" >> "${pseudo}"

    mksquashfs "${SYSROOT_DIR}" "${ISO_ROOT}/raven/filesystem.squashfs" \
        -comp zstd -Xcompression-level 15 \
        -pf "${pseudo}" -pseudo-override \
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
        # Helpful warning when stage4 is run without rebuilding stage3.
        if [[ -d "${PROJECT_ROOT}/bootloader" ]]; then
            if find "${PROJECT_ROOT}/bootloader/src" \
                "${PROJECT_ROOT}/bootloader/Cargo.toml" \
                "${PROJECT_ROOT}/bootloader/Cargo.lock" \
                -type f -newer "${ravenboot}" -print -quit 2>/dev/null | grep -q .; then
                log_warn "RavenBoot binary is older than bootloader sources; run stage3 to rebuild it."
            fi
        fi

        # Copy RavenBoot as primary bootloader
        cp "${ravenboot}" "${ISO_ROOT}/EFI/BOOT/BOOTX64.EFI"
        mkdir -p "${ISO_ROOT}/EFI/raven"
        cp "${ravenboot}" "${ISO_ROOT}/EFI/raven/raven-boot.efi"

        # RavenBoot has built-in menu support with sensible defaults, so no
        # boot.cfg is needed. If you want to customize, create boot.cfg with
        # flat entries (submenus are not yet supported in config file parsing).
        log_info "  Using built-in boot menu"

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

    # Create GRUB config with clean menu structure
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

# Default: console on tty1
menuentry "Raven Linux" --class raven {
    linux /boot/vmlinuz rdinit=/init quiet loglevel=3 console=tty0
    initrd /boot/initramfs.img
}

# Serial console mode (for VMs, headless, debugging)
menuentry "Raven Linux (Serial)" --class raven {
    linux /boot/vmlinuz rdinit=/init quiet loglevel=3 console=ttyS0,115200 console=tty0
    initrd /boot/initramfs.img
}

# System options submenu
submenu "System >" --class raven {
    menuentry "Recovery Mode" --class raven {
        linux /boot/vmlinuz rdinit=/init single console=ttyS0,115200 console=tty0
        initrd /boot/initramfs.img
    }

    # fwsetup reboots into the firmware's own configuration screen. It only
    # exists on EFI, and calling it on BIOS drops an "unknown command" error
    # into the menu -- hence the platform guard rather than an unconditional
    # entry. Firmware that does not advertise OsIndicationsSupported cannot do
    # this at all, so the entry says what happened instead of failing silently.
    if [ "$grub_platform" = "efi" ]; then
        menuentry "System UEFI Settings" --class uefi {
            fwsetup || echo "This firmware does not support rebooting into its setup screen."
        }
    fi

    menuentry "Reboot" --class restart {
        reboot
    }

    menuentry "Shutdown" --class shutdown {
        halt
    }

    menuentry "< Back" --class raven {
        configfile /boot/grub/grub.cfg
    }
}
EOF

    # The desktop entry exists only if the GUI stage actually installed a
    # compositor. Offering "Raven Desktop" on an ISO with no huginn would boot
    # to a black screen with the getty already disabled, which is strictly
    # worse than not offering it -- so this is appended, not baked into the
    # heredoc above.
    #
    # It is not the default: entry 0 stays the console. A compositor that fails
    # on unfamiliar hardware should cost you a menu selection, not the machine.
    if [[ -x "${SYSROOT_DIR}/usr/bin/huginn" ]]; then
        cat >> "${ISO_ROOT}/boot/grub/grub.cfg" << 'EOF'

# Wayland session: raven-init reads raven.graphics= and raven.wayland= from the
# cmdline, disables the tty1 getty, starts seatd, and execs
# /bin/raven-wayland-session, which execs huginn. There is no shell client
# after it: the compositor draws the dock, launcher and notifications itself.
menuentry "Raven Desktop (Huginn)" --class raven {
    linux /boot/vmlinuz rdinit=/init quiet loglevel=3 raven.graphics=wayland raven.wayland=huginn console=tty0
    initrd /boot/initramfs.img
}
EOF
        log_info "  Added the Raven Desktop (Huginn) boot entry"
    else
        log_info "  No compositor installed; boot menu stays console-only"
    fi

    # Create the EFI bootloader only if RavenBoot wasn't available
    if [[ ! -f "${ISO_ROOT}/EFI/BOOT/BOOTX64.EFI" ]]; then
        if command -v grub-mkstandalone &>/dev/null; then
            grub-mkstandalone \
                --format=x86_64-efi \
                --output="${ISO_ROOT}/EFI/BOOT/BOOTX64.EFI" \
                --locales="" \
                --fonts="" \
                "boot/grub/grub.cfg=${ISO_ROOT}/boot/grub/grub.cfg" 2>/dev/null || \
                log_warn "Failed to create GRUB EFI"
        else
            log_warn "grub-mkstandalone not found and no RavenBoot; the ISO will not boot under UEFI"
        fi
    fi

    log_success "GRUB configured"
}

# =============================================================================
# Create EFI boot image
# =============================================================================
create_efi_image() {
    log_step "Creating EFI boot image..."

    local efi_img="${EFI_IMG}"
    mkdir -p "$(dirname "${efi_img}")"

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
        if [[ -f "${ISO_ROOT}/EFI/raven/boot.cfg" ]]; then
            mcopy -i "${efi_img}" "${ISO_ROOT}/EFI/raven/boot.cfg" ::/EFI/raven/ 2>/dev/null || true
            log_info "  Copied RavenBoot config (boot.cfg)"
        fi
        if [[ -f "${ISO_ROOT}/EFI/raven/boot.conf" ]]; then
            mcopy -i "${efi_img}" "${ISO_ROOT}/EFI/raven/boot.conf" ::/EFI/raven/ 2>/dev/null || true
            log_info "  Copied RavenBoot config (boot.conf)"
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
            # Use an 8.3-safe initrd filename for broad firmware compatibility.
            mcopy -i "${efi_img}" "${ISO_ROOT}/boot/initramfs.img" ::/EFI/raven/initrd.img 2>/dev/null || true
            log_info "  Copied initrd.img to EFI image"
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
# EFI-only ISO
# =============================================================================
# The degraded image: boots on UEFI, not on BIOS. Split out so both the
# "no BIOS boot image" and "hybrid xorriso failed" paths produce exactly the
# same thing instead of two near-copies that can drift.
generate_iso_efi_only() {
    xorriso -as mkisofs \
        -iso-level 3 \
        -R -J -joliet-long \
        -volid "${ISO_LABEL}" \
        -output "${ISO_OUTPUT}" \
        -append_partition 2 "${ESP_TYPE_GUID}" "${EFI_IMG}" \
        -appended_part_as_gpt \
        -eltorito-alt-boot \
        -e --interval:appended_partition_2:all:: \
        -no-emul-boot \
        "${ISO_ROOT}" 2>&1 | tee "${LOGS_DIR}/xorriso.log"
}

# =============================================================================
# BIOS boot image
# =============================================================================
# Builds boot/grub/i386-pc/eltorito.img, which xorriso needs for the BIOS half
# of a hybrid ISO.
#
# The grub package does NOT ship eltorito.img -- it ships the modules and
# cdboot.img, and the El Torito image has to be linked from them by
# grub-mkimage. Without this the hybrid xorriso run fails with
#
#   FAILURE : Cannot find in ISO image: -boot_image ... bin_path=.../eltorito.img
#
# and generate_iso falls back to an EFI-only image. That fallback is quiet
# enough to miss: the build still reports success and produces an ISO, which
# then refuses to boot on BIOS with "No bootable device" and looks like a
# broken image rather than a missing boot record.
#
# Returns non-zero if the image cannot be built, and generate_iso degrades to
# EFI-only deliberately rather than by accident.
prepare_bios_boot() {
    local grub_lib="/usr/lib/grub/i386-pc"
    local dest="${ISO_ROOT}/boot/grub/i386-pc"

    command -v grub-mkimage &>/dev/null || {
        log_warn "grub-mkimage not found; the ISO will be EFI-only"
        return 1
    }
    [[ -d "${grub_lib}" ]] || {
        log_warn "${grub_lib} not found (grub's i386-pc target is not installed); the ISO will be EFI-only"
        return 1
    }

    log_step "Building the BIOS El Torito boot image..."

    mkdir -p "${dest}"
    # The modules have to be on the ISO too: eltorito.img is a small core that
    # loads the rest from ${prefix} at boot.
    cp -a "${grub_lib}/." "${dest}/" 2>/dev/null || true

    # -p /boot/grub is where the core looks for grub.cfg and its modules.
    # The module list is the minimum for finding and reading grub.cfg off an
    # ISO9660 disc and booting a Linux kernel from it.
    if ! grub-mkimage \
            -O i386-pc-eltorito \
            -p /boot/grub \
            -o "${dest}/eltorito.img" \
            biosdisk iso9660 part_msdos part_gpt fat ext2 \
            normal linux linux16 configfile search search_fs_uuid search_label \
            echo test boot chain minicmd ls cat halt reboot \
            gfxterm gfxmenu all_video videoinfo font \
            2>&1 | tee -a "${LOGS_DIR}/grub-mkimage.log"; then
        log_warn "grub-mkimage failed; the ISO will be EFI-only"
        return 1
    fi

    [[ -s "${dest}/eltorito.img" ]] || {
        log_warn "eltorito.img was not produced; the ISO will be EFI-only"
        return 1
    }

    log_success "  eltorito.img built ($(du -h "${dest}/eltorito.img" | cut -f1))"
    return 0
}

# =============================================================================
# Shutdown commands
# =============================================================================
# The live banner tells you to type 'poweroff' or 'reboot', and until now the
# sysroot shipped neither -- so the one instruction on screen did nothing. They
# cannot simply be copied from the build host either: on a systemd distro those
# are systemd's binaries, and with raven-init as PID 1 they only ever print
# "System has not been booted with systemd as init system (PID 1)".
#
# sysrq is compiled into the kernel (CONFIG_MAGIC_SYSRQ), so ask it directly.
install_shutdown_commands() {
    log_step "Installing shutdown commands..."

    # One dispatcher, correct in both worlds:
    #
    #   installed system -- PID 1 is raven-init, so hand off to raven-rc and let
    #                       init stop services and unmount cleanly.
    #   emergency shell  -- if raven-init is unavailable, ask the kernel
    #                       directly instead (CONFIG_MAGIC_SYSRQ).
    #
    # Checking /proc/1/comm at runtime rather than guessing at build time is
    # what lets the same image do the right thing once installed to disk.
    local name key
    for spec in reboot:b poweroff:o halt:o shutdown:o; do
        name="${spec%%:*}"
        key="${spec##*:}"

        cat > "${SYSROOT_DIR}/usr/bin/${name}" << EOF
#!/bin/sh
# RavenLinux ${name}. There is no systemd here; raven-init is PID 1 on normal
# live and installed boots. The fallback below is for an emergency shell.
if [ -x /bin/raven-rc ] && grep -qs raven-init /proc/1/comm 2>/dev/null; then
    exec /bin/raven-rc ${name}
fi

sync
[ -w /proc/sys/kernel/sysrq ] && echo 1 > /proc/sys/kernel/sysrq
echo ${key} > /proc/sysrq-trigger
EOF
        chmod 0755 "${SYSROOT_DIR}/usr/bin/${name}"
    done

    log_success "Shutdown commands installed (reboot, poweroff, halt, shutdown)"
}

# =============================================================================
# Console font
# =============================================================================
# The TTFs copied above are of no use to tty1: the Linux virtual terminal draws
# from a PSF bitmap font, not a scalable one. Without this the console runs on
# the kernel's built-in 8x16 VGA font, which has no box drawing worth the name
# and no Nerd Font glyphs at all -- so crow's frames come out as ASCII soup and
# ravenshell's prompt icons as blanks. On a 2560x1600 laptop panel it is also
# about two millimetres tall.
#
# So rasterise the same typeface into PSF at four cell sizes, and let
# raven-console-font pick one at boot from the framebuffer resolution.
#
# Fail-soft, like the raven and gui stages: a build host without freetype-py
# still produces a working ISO, just one whose console looks like 1994.
install_console_font() {
    log_step "Building the console font..."

    local generator="${PROJECT_ROOT}/scripts/make-console-font.py"
    local ttf="${PROJECT_ROOT}/fonts/JetBrainsMonoNerdFontMono-Regular.ttf"
    local outdir="${SYSROOT_DIR}/usr/share/kbd/consolefonts"

    if [[ ! -f "${generator}" ]]; then
        log_warn "  ${generator} not found; console stays on the kernel font"
        return 0
    fi

    if [[ ! -f "${ttf}" ]]; then
        log_warn "  ${ttf} not found; console stays on the kernel font"
        return 0
    fi

    local python=""
    for candidate in python3 python; do
        if command -v "${candidate}" &>/dev/null && \
           "${candidate}" -c 'import freetype' 2>/dev/null; then
            python="${candidate}"
            break
        fi
    done

    if [[ -z "${python}" ]]; then
        log_warn "  No Python with freetype-py; skipping the console font"
        log_warn "  Install python-freetype-py (Arch) and rerun stage4 to get it"
        return 0
    fi

    mkdir -p "${outdir}"

    # 8x16 for VGA text mode and small VM displays, up to 16x32 for a HiDPI
    # laptop panel. raven-console-font chooses between them at boot.
    local built=0 size w h
    for size in 8x16 10x20 12x24 16x32; do
        w="${size%x*}"
        h="${size#*x}"
        if "${python}" "${generator}" "${ttf}" \
            --width "${w}" --height "${h}" \
            -o "${outdir}/raven-${size}.psfu" >/dev/null 2>&1; then
            log_info "  Built raven-${size}.psfu"
            built=$((built + 1))
        else
            log_warn "  Could not build raven-${size}.psfu"
        fi
    done

    if [[ ${built} -eq 0 ]]; then
        log_warn "  No console fonts were built"
        rmdir "${outdir}" 2>/dev/null || true
        return 0
    fi

    # The loader. Without setfont in the sysroot it is a no-op that exits 0,
    # which is the right behaviour on a build that could not supply one.
    if [[ -f "${PROJECT_ROOT}/configs/raven-console-font" ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/bin"
        cp "${PROJECT_ROOT}/configs/raven-console-font" "${SYSROOT_DIR}/usr/bin/raven-console-font"
        chmod 0755 "${SYSROOT_DIR}/usr/bin/raven-console-font"
        # init.toml names /usr/sbin/raven-console-font; that still resolves,
        # because /usr/sbin is a symlink onto bin. The compat link that used to
        # be here deleted the script and left a dangler -- and it was wrapped in
        # `2>/dev/null || true`, so it could not even report the damage.
        log_info "  Installed raven-console-font"
    fi

    if [[ ! -x "${SYSROOT_DIR}/usr/bin/setfont" ]]; then
        log_warn "  setfont is not in the sysroot, so the font cannot be loaded at boot"
        log_warn "  It comes from kbd; stage2 copies it when the build host has it"
    fi

    log_success "Console font built (${built} sizes)"
}

# raven-udev starts the device manager and coldplugs attached hardware. It has
# to ship in the sysroot rather than early userspace, because graphics and
# wireless drivers are modules and nothing binds them without a coldplug.
install_udev_helper() {
    if [[ ! -f "${PROJECT_ROOT}/configs/raven-udev" ]]; then
        log_warn "  configs/raven-udev not found; hardware will not be coldplugged"
        return 0
    fi

    mkdir -p "${SYSROOT_DIR}/usr/bin"
    cp "${PROJECT_ROOT}/configs/raven-udev" "${SYSROOT_DIR}/usr/bin/raven-udev"
    chmod 0755 "${SYSROOT_DIR}/usr/bin/raven-udev"
    # init.toml's exec = "/usr/sbin/raven-udev" resolves here via /usr/sbin -> bin.
    log_info "  Installed raven-udev"

    if [[ ! -x "${SYSROOT_DIR}/usr/bin/udevd" ]]; then
        log_warn "  udevd is not in the sysroot; modules will not autoload"
        log_warn "  It comes from eudev; stage2 copies it when the build host has it"
    fi
}

# =============================================================================
# Install raven-install into the sysroot
# =============================================================================
# It goes into the squashfs rather than onto the ISO alongside it, so the same
# copy is on the live image and on every system installed from it. Installing
# from an already-installed machine onto a second disk is then the same code
# path, not a second one.
install_installer() {
    log_step "Installing raven-install..."

    local src="${PROJECT_ROOT}/scripts/installer/raven-install"
    local post="${PROJECT_ROOT}/scripts/installer/raven-postinstall"
    local profiles="${PROJECT_ROOT}/configs/installer/profiles"

    if [[ ! -f "${src}" ]]; then
        log_warn "scripts/installer/raven-install not found; the ISO will not be able to install itself"
        return 0
    fi

    mkdir -p "${SYSROOT_DIR}/usr/bin"
    cp "${src}" "${SYSROOT_DIR}/usr/bin/raven-install"
    chmod 0755 "${SYSROOT_DIR}/usr/bin/raven-install"

    # The old /sbin compat link existed because /sbin was on root's PATH and
    # /usr/sbin was not always. The merge removes the problem at the root:
    # /bin, /sbin, /usr/bin and /usr/sbin are one directory, so raven-install
    # is on every one of those PATH entries with no link. The link itself was
    # actively harmful -- it unlinked the installer and left a dangler, so the
    # ISO shipped with no way to install itself.
    if [[ -f "${post}" ]]; then
        cp "${post}" "${SYSROOT_DIR}/usr/bin/raven-postinstall"
        chmod 0755 "${SYSROOT_DIR}/usr/bin/raven-postinstall"
    fi
    if [[ -d "${profiles}" ]]; then
        mkdir -p "${SYSROOT_DIR}/etc/raven/install-profiles"
        cp "${profiles}"/*.packages "${SYSROOT_DIR}/etc/raven/install-profiles/"
    fi

    log_success "raven-install installed to /usr/bin/raven-install (also /sbin/raven-install)"
}

# =============================================================================
# Is the layout still usr-merged?
# =============================================================================
# The last gate before the sysroot is sealed into a squashfs. Every failure
# mode this catches is silent otherwise: a stage that recreated /bin as a real
# directory un-merges the root, so `rvn install <anything>` on the shipped
# system dies with "filesystem: File exists (os error 17)"; and a symlink loop
# on /usr/bin/sh or /usr/bin/bash boots to an image with no working shell.
#
# It fails the build rather than warning. An ISO that cannot install a package
# or reach a shell is not a console-only image, it is a broken one.
check_usrmerge_layout() {
    log_step "Verifying the usr-merged layout..."

    if ! declare -F raven_usrmerge_verify >/dev/null 2>&1; then
        log_warn "scripts/lib/usrmerge.sh not loaded; skipping the layout check"
        return 0
    fi

    local rc=0

    if raven_usrmerge_verify "${SYSROOT_DIR}"; then
        log_success "  /bin /sbin /lib /lib64 -> usr, no symlink loops"
    else
        log_error "The sysroot is not usr-merged. Arch packages cannot be installed on it,"
        log_error "and a symlink loop under /usr/bin can leave the image with no shell."
        rc=1
    fi

    # Separate question, separate check: the merge can be flawless while the
    # tree is missing /var/empty, /etc/mtab or half of /var. An absent path is
    # not a merge error and not a package conflict, so nothing above sees it.
    if declare -F raven_skeleton_verify >/dev/null 2>&1; then
        if raven_skeleton_verify "${SYSROOT_DIR}" --quiet; then
            log_success "  all skeleton directories, symlinks and accounts present"
        else
            log_error "The sysroot is incomplete. The failures above are directories,"
            log_error "symlinks or accounts the running system needs but does not have."
            rc=1
        fi
    fi

    return $rc
}

# =============================================================================
# Is the sysroot actually complete?
# =============================================================================
# stage2 begins by wiping the sysroot, and the Raven and GUI layers are added
# by later stages. So `imlazy stage2 && imlazy iso` -- a sequence that looks
# perfectly reasonable -- silently produces an ISO with no shell, no package
# manager and no desktop, and every stage reports success along the way.
#
# stage4 is the last place that can notice. It does not refuse to build: a
# console-only image is a legitimate thing to want. It refuses to be quiet
# about it.
check_sysroot_layers() {
    local -a raven_missing=() gui_missing=()
    local b

    for b in ravenshell rvn caw cawd crow ivaldi oxigen poxy raven-init raven-rc; do
        find "${SYSROOT_DIR}" -name "${b}" -type f -print -quit 2>/dev/null | grep -q . \
            || raven_missing+=("${b}")
    done

    for b in huginn muninn-lock raven-wayland-session; do
        find "${SYSROOT_DIR}" -name "${b}" -type f -print -quit 2>/dev/null | grep -q . \
            || gui_missing+=("${b}")
    done

    if (( ${#raven_missing[@]} == 0 && ${#gui_missing[@]} == 0 )); then
        log_success "Sysroot carries the Raven and GUI layers"
        return 0
    fi

    echo ""
    log_warn "=============================================================="
    log_warn "  This ISO is missing whole layers of the system."
    log_warn "=============================================================="

    if (( ${#raven_missing[@]} > 0 )); then
        log_warn "  Raven layer absent: ${raven_missing[*]}"
        log_warn "    restore with: imlazy raven"
    fi

    if (( ${#gui_missing[@]} > 0 )); then
        log_warn "  GUI layer absent:   ${gui_missing[*]}"
        log_warn "    restore with: imlazy gui"
        log_warn "    Without it the compositor, the desktop shell and"
        log_warn "    libinput's library closure are all missing, so a"
        log_warn "    graphical boot has no input and nothing to draw."
    fi

    log_warn ""
    log_warn "  stage2 resets the sysroot, so anything built after it has to"
    log_warn "  be rebuilt too. 'imlazy build' runs every layer in order."
    log_warn "=============================================================="
    echo ""
}

# =============================================================================
# Generate ISO
# =============================================================================
generate_iso() {
    log_step "Generating ISO image..."

    # Both paths below attach the ESP with -append_partition, so a missing
    # image makes xorriso abort in the hybrid run *and* in the fallback. Say so
    # once, up front, rather than letting it read as two unrelated failures.
    if [[ ! -f "${EFI_IMG}" ]]; then
        log_error "No EFI boot image at ${EFI_IMG}"
        log_error "  create_efi_image() did not run or failed (needs mkfs.vfat and mtools)."
        return 1
    fi

    # Build the BIOS boot image first. Only attempt the hybrid ISO if it
    # exists -- otherwise xorriso aborts and we take the fallback anyway,
    # having written a misleading FAILURE into the log.
    if ! prepare_bios_boot; then
        log_warn "No BIOS boot image; creating an EFI-only ISO"
        generate_iso_efi_only
        return
    fi

    # Try full hybrid ISO first.
    #
    # The ESP rides along as an appended GPT partition, and the UEFI El Torito
    # entry points into that partition rather than at a file in the ISO9660
    # tree, so it is stored once instead of twice. This replaces
    # -isohybrid-gpt-basdat, which silently did nothing here: that option only
    # takes effect alongside an isolinux -isohybrid-mbr, and --grub2-mbr below
    # claims the system area instead. The result was an image with no
    # partition table at all -- bootable from a disc, invisible to UEFI
    # firmware on a USB stick.
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
        -append_partition 2 "${ESP_TYPE_GUID}" "${EFI_IMG}" \
        -appended_part_as_gpt \
        -eltorito-alt-boot \
        -e --interval:appended_partition_2:all:: \
        -no-emul-boot \
        "${ISO_ROOT}" \
        2>&1 | tee "${LOGS_DIR}/xorriso.log"; then
        log_success "Hybrid ISO created (BIOS + UEFI)"
    else
        log_warn "Hybrid ISO failed, creating EFI-only ISO..."
        generate_iso_efi_only
    fi

    # Generate checksums
    (
        cd "$(dirname "${ISO_OUTPUT}")"
        sha256sum "$(basename "${ISO_OUTPUT}")" > "$(basename "${ISO_OUTPUT}").sha256"
        md5sum "$(basename "${ISO_OUTPUT}")" > "$(basename "${ISO_OUTPUT}").md5"
    )

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
        echo "  Bootloader: RavenBoot (UEFI), GRUB (BIOS)"
    else
        echo "  Bootloader: GRUB (UEFI + BIOS)"
    fi

    echo ""
    echo "  Test in QEMU (UEFI):"
    echo "    qemu-system-x86_64 -cdrom ${ISO_OUTPUT} -m 2G \\"
    echo "      -nographic -serial mon:stdio \\"
    echo "      -bios /usr/share/edk2-ovmf/x64/OVMF_CODE.4m.fd -enable-kvm"
    echo ""
    echo "  Test in QEMU (BIOS):"
    echo "    qemu-system-x86_64 -cdrom ${ISO_OUTPUT} -m 2G -enable-kvm"
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
    check_sysroot_layers
    setup_iso_structure
    create_managed_live_init
    install_shutdown_commands
    install_installer
    copy_boot_files
    copy_kernel_modules
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
