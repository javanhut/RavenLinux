#!/bin/bash
# =============================================================================
# RavenLinux Live ISO Builder
# =============================================================================
# Creates a complete live bootable ISO with all RavenLinux components
#
# Usage: ./scripts/build-live-iso.sh [options]
#   --skip-kernel     Skip kernel build (use existing)
#   --skip-packages   Skip package builds
#   --minimal         Build minimal ISO without desktop

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="${PROJECT_ROOT}/build"
ISO_DIR="${BUILD_DIR}/iso"
LIVE_ROOT="${ISO_DIR}/live-root"
SQUASHFS_DIR="${ISO_DIR}/squashfs"

# Version info
RAVEN_VERSION="2025.12"
RAVEN_ARCH="x86_64"
ISO_LABEL="RAVEN_LIVE"
ISO_OUTPUT="${PROJECT_ROOT}/raven-${RAVEN_VERSION}-${RAVEN_ARCH}.iso"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Options
SKIP_KERNEL=false
SKIP_PACKAGES=false
MINIMAL=false

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }
log_step() { echo -e "${CYAN}[STEP]${NC} $1"; }

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --skip-kernel) SKIP_KERNEL=true; shift ;;
        --skip-packages) SKIP_PACKAGES=true; shift ;;
        --minimal) MINIMAL=true; shift ;;
        *) log_error "Unknown option: $1" ;;
    esac
done

check_dependencies() {
    log_info "Checking build dependencies..."

    local missing=()

    for cmd in mksquashfs xorriso grub-mkstandalone; do
        if ! command -v "$cmd" &>/dev/null; then
            missing+=("$cmd")
        fi
    done

    if [ ${#missing[@]} -ne 0 ]; then
        log_error "Missing dependencies: ${missing[*]}"
        echo "Install with: sudo pacman -S squashfs-tools libisoburn grub"
    fi

    log_success "All dependencies found"
}

setup_live_root() {
    log_step "Setting up live root filesystem..."

    rm -rf "${LIVE_ROOT}"
    mkdir -p "${LIVE_ROOT}"/{bin,sbin,lib,lib64,usr/{bin,sbin,lib,share},etc,var,tmp,root,home,dev,proc,sys,run,mnt,opt}
    mkdir -p "${LIVE_ROOT}"/usr/share/{fonts,icons,themes,backgrounds,zsh}
    mkdir -p "${LIVE_ROOT}"/etc/{skel,xdg,rvn}
    mkdir -p "${LIVE_ROOT}"/var/{log,cache,lib,tmp}

    log_success "Live root structure created"
}

copy_kernel() {
    log_step "Copying kernel..."

    mkdir -p "${LIVE_ROOT}/boot"

    if [[ -f "${BUILD_DIR}/kernel/boot/vmlinuz-raven" ]]; then
        cp "${BUILD_DIR}/kernel/boot/vmlinuz-raven" "${LIVE_ROOT}/boot/vmlinuz"
        log_success "Kernel copied"
    else
        log_error "Kernel not found. Run ./scripts/build-kernel.sh first"
    fi
}

copy_initramfs() {
    log_step "Copying initramfs..."

    if [[ -f "${BUILD_DIR}/initramfs-raven.img" ]]; then
        cp "${BUILD_DIR}/initramfs-raven.img" "${LIVE_ROOT}/boot/initramfs.img"
        log_success "Initramfs copied"
    else
        log_warn "Initramfs not found, will create minimal one"
    fi
}

copy_coreutils() {
    log_step "Copying uutils-coreutils..."

    if [[ -f "${BUILD_DIR}/bin/coreutils" ]]; then
        cp "${BUILD_DIR}/bin/coreutils" "${LIVE_ROOT}/bin/coreutils"

        # Create symlinks for all utilities
        local utils=(
            cat cp mv rm ln mkdir rmdir touch chmod chown chgrp
            ls dir vdir head tail cut paste sort uniq wc tr tee
            echo printf yes df du stat sync id whoami groups
            uname hostname date sleep basename dirname realpath
            readlink pwd md5sum sha256sum test true false env
            seq dd install mktemp mknod tty
        )

        for util in "${utils[@]}"; do
            ln -sf coreutils "${LIVE_ROOT}/bin/${util}"
        done

        log_success "Coreutils installed"
    else
        log_error "uutils-coreutils not found. Run ./scripts/build-uutils.sh first"
    fi
}

copy_shells() {
    log_step "Copying shells..."

    # Copy zsh from host
    if command -v zsh &>/dev/null; then
        cp "$(which zsh)" "${LIVE_ROOT}/bin/zsh"

        # Copy zsh configuration files
        mkdir -p "${LIVE_ROOT}/usr/share/zsh"
        cp -r /usr/share/zsh/* "${LIVE_ROOT}/usr/share/zsh/" 2>/dev/null || true

        log_info "  Added zsh"
    fi

    # Copy bash from host
    if command -v bash &>/dev/null; then
        cp "$(which bash)" "${LIVE_ROOT}/bin/bash"
        log_info "  Added bash"
    fi

    # Create sh symlink to zsh
    ln -sf zsh "${LIVE_ROOT}/bin/sh"

    log_success "Shells installed"
}

copy_raven_packages() {
    log_step "Copying RavenLinux custom packages..."

    local packages_bin="${BUILD_DIR}/packages/bin"

    if [[ -d "${packages_bin}" ]]; then
        for pkg in vem carrion ivaldi raven-installer rvn; do
            if [[ -f "${packages_bin}/${pkg}" ]]; then
                cp "${packages_bin}/${pkg}" "${LIVE_ROOT}/bin/${pkg}"
                log_info "  Added ${pkg}"
            fi
        done
    fi

    # Create symlink for installer command
    if [[ -f "${LIVE_ROOT}/bin/raven-installer" ]]; then
        ln -sf raven-installer "${LIVE_ROOT}/bin/raven-install"
    fi

    log_success "Custom packages installed"
}

copy_package_manager() {
    log_step "Building and copying package manager (rvn)..."

    local rvn_dir="${PROJECT_ROOT}/tools/rvn"

    if [[ -d "${rvn_dir}" ]]; then
        cd "${rvn_dir}"

        # Build rvn
        if cargo build --release 2>/dev/null; then
            cp target/release/rvn "${LIVE_ROOT}/bin/rvn"
            log_success "Package manager (rvn) installed"
        else
            log_warn "Failed to build rvn, skipping"
        fi

        cd "${PROJECT_ROOT}"
    fi
}

copy_networking_tools() {
    log_step "Copying networking tools..."

    # Copy essential networking tools from host
    local net_tools=(ip ping dhcpcd wpa_supplicant iw iwconfig ifconfig route netstat ss curl wget)

    for tool in "${net_tools[@]}"; do
        if command -v "$tool" &>/dev/null; then
            cp "$(which "$tool")" "${LIVE_ROOT}/bin/" 2>/dev/null || \
            cp "$(which "$tool")" "${LIVE_ROOT}/sbin/" 2>/dev/null || true
            log_info "  Added ${tool}"
        fi
    done

    # Copy DNS resolver config
    echo "nameserver 8.8.8.8" > "${LIVE_ROOT}/etc/resolv.conf"
    echo "nameserver 1.1.1.1" >> "${LIVE_ROOT}/etc/resolv.conf"

    log_success "Networking tools installed"
}

copy_libraries() {
    log_step "Copying required libraries..."

    # Find and copy required libraries for all binaries
    for bin in "${LIVE_ROOT}"/bin/* "${LIVE_ROOT}"/sbin/*; do
        [[ -f "$bin" && -x "$bin" && ! -L "$bin" ]] || continue

        # Skip statically linked binaries
        if file "$bin" | grep -q "statically linked"; then
            continue
        fi

        timeout 2 ldd "$bin" 2>/dev/null | grep -o '/[^ ]*' | while read -r lib; do
            [[ -z "$lib" || ! -f "$lib" ]] && continue
            local dest="${LIVE_ROOT}${lib}"
            if [[ ! -f "$dest" ]]; then
                mkdir -p "$(dirname "$dest")"
                cp -L "$lib" "$dest" 2>/dev/null || true
            fi
        done
    done

    # Copy dynamic linker
    for ld in /lib64/ld-linux-x86-64.so.2 /lib/ld-linux-x86-64.so.2; do
        if [[ -f "$ld" ]]; then
            mkdir -p "${LIVE_ROOT}$(dirname "$ld")"
            cp -L "$ld" "${LIVE_ROOT}${ld}" 2>/dev/null || true
        fi
    done

    log_success "Libraries copied"
}

create_config_files() {
    log_step "Creating configuration files..."

    # /etc/os-release
    cat > "${LIVE_ROOT}/etc/os-release" << EOF
NAME="Raven Linux"
PRETTY_NAME="Raven Linux ${RAVEN_VERSION}"
ID=raven
BUILD_ID=rolling
VERSION_ID=${RAVEN_VERSION}
VERSION="${RAVEN_VERSION} (Rolling)"
ANSI_COLOR="38;2;23;147;209"
HOME_URL="https://ravenlinux.org"
DOCUMENTATION_URL="https://docs.ravenlinux.org"
SUPPORT_URL="https://github.com/ravenlinux/ravenlinux/discussions"
BUG_REPORT_URL="https://github.com/ravenlinux/ravenlinux/issues"
LOGO=raven-logo
EOF

    # /etc/hostname
    echo "raven" > "${LIVE_ROOT}/etc/hostname"

    # /etc/hosts
    cat > "${LIVE_ROOT}/etc/hosts" << EOF
127.0.0.1   localhost
::1         localhost
127.0.1.1   raven.localdomain raven
EOF

    # /etc/passwd
    cat > "${LIVE_ROOT}/etc/passwd" << EOF
root:x:0:0:root:/root:/bin/zsh
raven:x:1000:1000:Raven User:/home/raven:/bin/zsh
nobody:x:65534:65534:Nobody:/:/bin/false
EOF

    # /etc/group
    cat > "${LIVE_ROOT}/etc/group" << EOF
root:x:0:
wheel:x:10:raven
audio:x:11:raven
video:x:12:raven
users:x:100:raven
raven:x:1000:
nobody:x:65534:
EOF

    # /etc/shadow (empty passwords for live environment)
    cat > "${LIVE_ROOT}/etc/shadow" << EOF
root::0:0:99999:7:::
raven::0:0:99999:7:::
nobody:!:0:0:99999:7:::
EOF
    chmod 600 "${LIVE_ROOT}/etc/shadow"

    # /etc/shells
    cat > "${LIVE_ROOT}/etc/shells" << EOF
/bin/sh
/bin/bash
/bin/zsh
EOF

    # /etc/profile
    cat > "${LIVE_ROOT}/etc/profile" << 'EOF'
export PATH=/bin:/sbin:/usr/bin:/usr/sbin
export HOME="${HOME:-/root}"
export TERM="${TERM:-linux}"
export LANG=en_US.UTF-8
export EDITOR=vem
export VISUAL=vem
export RAVEN_LINUX=1

# Source zsh config if using zsh
if [ -n "$ZSH_VERSION" ]; then
    [ -f /etc/zsh/zshrc ] && . /etc/zsh/zshrc
fi
EOF

    # /etc/zsh/zshrc (system-wide zsh config)
    mkdir -p "${LIVE_ROOT}/etc/zsh"
    cat > "${LIVE_ROOT}/etc/zsh/zshrc" << 'EOF'
# RavenLinux ZSH Configuration

# History
HISTFILE=~/.zsh_history
HISTSIZE=10000
SAVEHIST=10000
setopt SHARE_HISTORY
setopt HIST_IGNORE_DUPS

# Completion
autoload -Uz compinit
compinit

# Prompt
autoload -Uz promptinit
promptinit

# Custom prompt
PROMPT='%F{cyan}[raven%f:%F{blue}%~%f]%# '

# Aliases
alias ls='ls --color=auto'
alias ll='ls -la'
alias la='ls -A'
alias l='ls -CF'
alias grep='grep --color=auto'
alias ..='cd ..'
alias ...='cd ../..'

# Keybindings (vim-like)
bindkey -v
bindkey '^R' history-incremental-search-backward

# Environment
export PATH=/bin:/sbin:/usr/bin:/usr/sbin:$HOME/.local/bin
export EDITOR=vem
export VISUAL=vem
EOF

    # Create raven user home directory
    mkdir -p "${LIVE_ROOT}/home/raven"
    cp "${LIVE_ROOT}/etc/zsh/zshrc" "${LIVE_ROOT}/home/raven/.zshrc"
    chown -R 1000:1000 "${LIVE_ROOT}/home/raven" 2>/dev/null || true

    # Root's zshrc
    cp "${LIVE_ROOT}/etc/zsh/zshrc" "${LIVE_ROOT}/root/.zshrc"

    log_success "Configuration files created"
}

create_init_system() {
    log_step "Creating init system..."

    # Create a proper init script for the live environment
    cat > "${LIVE_ROOT}/init" << 'INIT'
#!/bin/bash
# RavenLinux Live Init

export PATH=/bin:/sbin:/usr/bin:/usr/sbin

echo "Starting Raven Linux Live..."

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
if [ -x /sbin/udevd ]; then
    /sbin/udevd --daemon
    udevadm trigger
    udevadm settle
fi

# Configure networking (try DHCP)
if command -v dhcpcd &>/dev/null; then
    for iface in /sys/class/net/e*; do
        [ -d "$iface" ] && dhcpcd "$(basename "$iface")" 2>/dev/null &
    done
fi

# Clear screen and show welcome
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
echo "  Version: $(cat /etc/os-release | grep VERSION_ID | cut -d= -f2)"
echo ""
echo "  Available tools:"
echo "    vem      - Raven text editor"
echo "    carrion  - Carrion programming language"
echo "    ivaldi   - Ivaldi version control"
echo "    rvn      - Raven package manager"
echo ""
echo "  Type 'raven-install' to install to disk"
echo "  Type 'poweroff' to shutdown"
echo ""

# Start login shell
if [ -x /bin/zsh ]; then
    exec /bin/zsh -l
else
    exec /bin/bash -l
fi
INIT
    chmod +x "${LIVE_ROOT}/init"

    log_success "Init system created"
}

create_installer_stub() {
    log_step "Creating installer stub..."

    cat > "${LIVE_ROOT}/bin/raven-install" << 'INSTALLER'
#!/bin/bash
# RavenLinux Installer (Text-based stub)
# Full GUI installer will be implemented separately

echo ""
echo "=========================================="
echo "  RavenLinux Installer"
echo "=========================================="
echo ""
echo "This is a placeholder for the full installer."
echo "The GUI installer is under development."
echo ""
echo "For manual installation:"
echo "  1. Partition your disk with fdisk/parted"
echo "  2. Format partitions (mkfs.ext4, mkfs.vfat)"
echo "  3. Mount target to /mnt"
echo "  4. Copy live system: cp -a /* /mnt/"
echo "  5. Install bootloader"
echo "  6. Configure fstab"
echo ""
echo "Press any key to return..."
read -n 1
INSTALLER
    chmod +x "${LIVE_ROOT}/bin/raven-install"

    log_success "Installer stub created"
}

setup_iso_structure() {
    log_step "Setting up ISO structure..."

    rm -rf "${ISO_DIR}/iso-root"
    mkdir -p "${ISO_DIR}/iso-root"/{boot/grub,EFI/BOOT,raven}

    log_success "ISO structure created"
}

create_squashfs() {
    log_step "Creating squashfs filesystem..."

    mksquashfs "${LIVE_ROOT}" "${ISO_DIR}/iso-root/raven/filesystem.squashfs" \
        -comp zstd -Xcompression-level 15 \
        -b 1M -no-duplicates -quiet

    log_success "Squashfs created"
}

setup_raven_bootloader() {
    log_step "Setting up RavenBoot bootloader..."

    # Copy kernel and initramfs to ISO
    cp "${LIVE_ROOT}/boot/vmlinuz" "${ISO_DIR}/iso-root/boot/vmlinuz"
    cp "${LIVE_ROOT}/boot/initramfs.img" "${ISO_DIR}/iso-root/boot/initramfs.img" 2>/dev/null || \
        cp "${BUILD_DIR}/initramfs-raven.img" "${ISO_DIR}/iso-root/boot/initramfs.img"

    # Create GRUB config (fallback until RavenBoot is complete)
    cat > "${ISO_DIR}/iso-root/boot/grub/grub.cfg" << EOF
set default=0
set timeout=5

insmod all_video
insmod gfxterm
terminal_output gfxterm
set gfxmode=auto
set gfxpayload=keep

# RavenLinux theme colors
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

menuentry "Raven Linux Install" --class raven {
    linux /boot/vmlinuz rdinit=/init raven.installer
    initrd /boot/initramfs.img
}

menuentry "Reboot" --class restart {
    reboot
}

menuentry "Shutdown" --class shutdown {
    halt
}
EOF

    # Create EFI bootloader
    if command -v grub-mkstandalone &>/dev/null; then
        grub-mkstandalone \
            --format=x86_64-efi \
            --output="${ISO_DIR}/iso-root/EFI/BOOT/BOOTX64.EFI" \
            --locales="" \
            --fonts="" \
            "boot/grub/grub.cfg=${ISO_DIR}/iso-root/boot/grub/grub.cfg" 2>/dev/null || \
            log_warn "Failed to create EFI bootloader"
    fi

    # Create EFI boot image for ISO
    mkdir -p "${ISO_DIR}/iso-root/EFI/BOOT"
    if [[ -f "${ISO_DIR}/iso-root/EFI/BOOT/BOOTX64.EFI" ]]; then
        dd if=/dev/zero of="${ISO_DIR}/iso-root/boot/efiboot.img" bs=1M count=10 2>/dev/null
        mkfs.vfat "${ISO_DIR}/iso-root/boot/efiboot.img" 2>/dev/null || true
        mmd -i "${ISO_DIR}/iso-root/boot/efiboot.img" ::/EFI 2>/dev/null || true
        mmd -i "${ISO_DIR}/iso-root/boot/efiboot.img" ::/EFI/BOOT 2>/dev/null || true
        mcopy -i "${ISO_DIR}/iso-root/boot/efiboot.img" "${ISO_DIR}/iso-root/EFI/BOOT/BOOTX64.EFI" ::/EFI/BOOT/ 2>/dev/null || true
    fi

    log_success "Bootloader configured"
}

generate_iso() {
    log_step "Generating ISO image..."

    # Create ISO with xorriso
    xorriso -as mkisofs \
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
        "${ISO_DIR}/iso-root" \
        /boot/grub/i386-pc=/usr/lib/grub/i386-pc \
        2>/dev/null || {
            # Fallback to simpler method
            log_warn "Full ISO failed, trying simpler method..."
            xorriso -as mkisofs \
                -R -J \
                -volid "${ISO_LABEL}" \
                -output "${ISO_OUTPUT}" \
                "${ISO_DIR}/iso-root"
        }

    # Generate checksums
    sha256sum "${ISO_OUTPUT}" > "${ISO_OUTPUT}.sha256"

    log_success "ISO generated: ${ISO_OUTPUT}"
}

print_summary() {
    local iso_size
    iso_size=$(du -h "${ISO_OUTPUT}" 2>/dev/null | cut -f1 || echo "unknown")

    echo ""
    echo -e "${CYAN}=========================================="
    echo "  RavenLinux Live ISO Build Complete"
    echo "==========================================${NC}"
    echo ""
    echo "  ISO:      ${ISO_OUTPUT}"
    echo "  Size:     ${iso_size}"
    echo "  Version:  ${RAVEN_VERSION}"
    echo "  Arch:     ${RAVEN_ARCH}"
    echo ""
    echo "  Included:"
    echo "    - Linux Kernel 6.17.11"
    echo "    - Zsh (default shell)"
    echo "    - Vem (text editor)"
    echo "    - Carrion (programming language)"
    echo "    - Ivaldi (version control)"
    echo "    - rvn (package manager)"
    echo ""
    echo "  To test in QEMU:"
    echo "    qemu-system-x86_64 -cdrom ${ISO_OUTPUT} -m 4G -enable-kvm"
    echo ""
    echo "  To write to USB:"
    echo "    sudo dd if=${ISO_OUTPUT} of=/dev/sdX bs=4M status=progress"
    echo ""
}

# Main execution
main() {
    echo ""
    echo -e "${CYAN}=========================================="
    echo "  RavenLinux Live ISO Builder"
    echo "==========================================${NC}"
    echo ""

    check_dependencies
    setup_live_root
    copy_kernel
    copy_initramfs
    copy_coreutils
    copy_shells
    copy_raven_packages
    copy_package_manager
    copy_networking_tools
    copy_libraries
    create_config_files
    create_init_system
    create_installer_stub
    setup_iso_structure
    create_squashfs
    setup_raven_bootloader
    generate_iso
    print_summary
}

main "$@"
