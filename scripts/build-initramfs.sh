#!/bin/bash
# =============================================================================
# RavenLinux Initramfs Build Script
# =============================================================================
# Build a minimal RavenLinux initramfs for testing
# Uses host system tools - not a full build, just for quick iteration
#
# Usage: ./scripts/build-initramfs.sh [OPTIONS]
#
# Options:
#   --no-log    Disable file logging

set -euo pipefail

# =============================================================================
# Configuration
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export RAVEN_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
export RAVEN_BUILD="${RAVEN_ROOT}/build"
INITRAMFS_DIR="${RAVEN_BUILD}/initramfs"
OUTPUT="${RAVEN_BUILD}/initramfs-raven.img"

# Source shared logging library
source "${SCRIPT_DIR}/lib/logging.sh"

# =============================================================================
# Argument Parsing
# =============================================================================

while [[ $# -gt 0 ]]; do
    case "$1" in
        --no-log)
            export RAVEN_NO_LOG=1
            shift
            ;;
        *)
            log_error "Unknown option: $1"
            echo "Usage: $0 [--no-log]"
            exit 1
            ;;
    esac
done

# =============================================================================
# Functions
# =============================================================================

check_root() {
    if [[ $EUID -ne 0 ]]; then
        log_fatal "This script must be run as root (need to copy device nodes)"
    fi
}

cleanup() {
    log_step "Cleaning up old build..."
    rm -rf "${INITRAMFS_DIR}"
    mkdir -p "${INITRAMFS_DIR}"
}

create_directory_structure() {
    log_step "Creating directory structure..."

    mkdir -p "${INITRAMFS_DIR}"/{bin,sbin,usr/bin,usr/sbin,usr/lib,lib,lib64}
    mkdir -p "${INITRAMFS_DIR}"/{dev,proc,sys,run,tmp,root}
    mkdir -p "${INITRAMFS_DIR}"/mnt/{cdrom,squashfs,root,overlay,work}
    mkdir -p "${INITRAMFS_DIR}"/etc/{raven,rvn}
    mkdir -p "${INITRAMFS_DIR}"/var/{log,tmp}

    log_success "Directory structure created"
}

copy_binaries() {
    log_step "Copying essential binaries..."

    local UUTILS_BIN="${RAVEN_BUILD}/bin/coreutils"

    # Check if uutils is built
    if [[ ! -f "${UUTILS_BIN}" ]]; then
        log_fatal "uutils-coreutils not built. Run: ./scripts/build-uutils.sh"
    fi

    # Copy uutils multicall binary
    cp "${UUTILS_BIN}" "${INITRAMFS_DIR}/bin/coreutils"
    log_info "  Added uutils-coreutils"

    # Create symlinks for all utilities
    local utils=(
        # File operations
        cat cp mv rm ln mkdir rmdir touch chmod chown chgrp
        ls dir vdir
        # Text processing
        head tail cut paste sort uniq wc tr tee nl od fmt fold join split
        # Output
        echo printf yes
        # Filesystem
        df du stat sync truncate
        # User/group
        id whoami groups users who logname
        # System info
        uname hostname uptime arch nproc
        # Date/time
        date sleep
        # Path operations
        basename dirname realpath readlink pwd
        # Checksums
        md5sum sha1sum sha256sum sha512sum cksum
        # Conditionals
        test true false expr
        # Misc
        env printenv seq shuf factor base64 base32 mktemp mknod tty
        dd install
    )

    for util in "${utils[@]}"; do
        ln -sf coreutils "${INITRAMFS_DIR}/bin/${util}"
    done

    # These need to come from host (not in uutils or need special handling)
    # Include switch_root for live boot
    local host_bins=(mount umount dmesg clear reset ps kill free grep sed awk find xargs poweroff reboot switch_root losetup blkid)
    for bin in "${host_bins[@]}"; do
        if command -v "$bin" &>/dev/null; then
            cp "$(which "$bin")" "${INITRAMFS_DIR}/bin/" 2>/dev/null || true
        fi
    done

    # Copy bash
    if command -v bash &>/dev/null; then
        cp "$(which bash)" "${INITRAMFS_DIR}/bin/bash"
        ln -sf bash "${INITRAMFS_DIR}/bin/sh"
        log_info "  Added bash"
    fi

    # Copy zsh if available
    if command -v zsh &>/dev/null; then
        cp "$(which zsh)" "${INITRAMFS_DIR}/bin/zsh"
        log_info "  Added zsh"
    fi

    # Copy RavenLinux custom packages (Vem, Carrion, Ivaldi)
    local PACKAGES_BIN="${RAVEN_BUILD}/packages/bin"
    if [[ -d "${PACKAGES_BIN}" ]]; then
        log_info "Copying RavenLinux custom packages..."
        for pkg in vem carrion ivaldi; do
            if [[ -f "${PACKAGES_BIN}/${pkg}" ]]; then
                cp "${PACKAGES_BIN}/${pkg}" "${INITRAMFS_DIR}/bin/${pkg}"
                log_info "  Added ${pkg}"
            fi
        done
    fi

    log_success "Binaries copied"
}

copy_libraries() {
    log_step "Copying required libraries..."

    # Find and copy required libraries for binaries in initramfs
    for bin in "${INITRAMFS_DIR}"/bin/*; do
        [[ -f "$bin" && -x "$bin" && ! -L "$bin" ]] || continue

        # Skip statically linked binaries (vem, carrion, ivaldi are static Go binaries)
        if file "$bin" | grep -q "statically linked"; then
            continue
        fi

        # Use timeout to avoid hanging on problematic binaries
        timeout 2 ldd "$bin" 2>/dev/null | grep -o '/[^ ]*' | while read -r lib; do
            [[ -z "$lib" || ! -f "$lib" ]] && continue
            local dest="${INITRAMFS_DIR}${lib}"
            if [[ ! -f "$dest" ]]; then
                mkdir -p "$(dirname "$dest")"
                cp -L "$lib" "$dest" 2>/dev/null || true
            fi
        done
    done

    # Copy dynamic linker
    for ld in /lib64/ld-linux-x86-64.so.2 /lib/ld-linux-x86-64.so.2; do
        if [[ -f "$ld" ]]; then
            mkdir -p "${INITRAMFS_DIR}$(dirname "$ld")"
            cp -L "$ld" "${INITRAMFS_DIR}${ld}" 2>/dev/null || true
        fi
    done

    log_success "Libraries copied"
}

create_device_nodes() {
    log_step "Creating device nodes..."

    mknod -m 600 "${INITRAMFS_DIR}/dev/console" c 5 1
    mknod -m 666 "${INITRAMFS_DIR}/dev/null" c 1 3
    mknod -m 666 "${INITRAMFS_DIR}/dev/zero" c 1 5
    mknod -m 666 "${INITRAMFS_DIR}/dev/random" c 1 8
    mknod -m 666 "${INITRAMFS_DIR}/dev/urandom" c 1 9
    mknod -m 666 "${INITRAMFS_DIR}/dev/tty" c 5 0
    mknod -m 666 "${INITRAMFS_DIR}/dev/tty0" c 4 0
    mknod -m 666 "${INITRAMFS_DIR}/dev/tty1" c 4 1
    mknod -m 666 "${INITRAMFS_DIR}/dev/ptmx" c 5 2

    mkdir -p "${INITRAMFS_DIR}/dev/pts"

    log_success "Device nodes created"
}

create_config_files() {
    log_step "Creating configuration files..."

    # /etc/os-release
    cp "${RAVEN_ROOT}/etc/os-release" "${INITRAMFS_DIR}/etc/os-release"

    # /etc/hostname
    echo "raven" > "${INITRAMFS_DIR}/etc/hostname"

    # /etc/passwd
    cat > "${INITRAMFS_DIR}/etc/passwd" <<'PASSWD'
root:x:0:0:root:/root:/bin/zsh
nobody:x:65534:65534:Nobody:/:/bin/false
PASSWD

    # /etc/group
    cat > "${INITRAMFS_DIR}/etc/group" <<'GROUP'
root:x:0:
wheel:x:10:root
nobody:x:65534:
GROUP

    # /etc/shadow (root with no password for testing)
    cat > "${INITRAMFS_DIR}/etc/shadow" <<'SHADOW'
root::0:0:99999:7:::
nobody:!:0:0:99999:7:::
SHADOW
    chmod 600 "${INITRAMFS_DIR}/etc/shadow"

    # /etc/shells
    cat > "${INITRAMFS_DIR}/etc/shells" <<'SHELLS'
/bin/sh
/bin/bash
/bin/zsh
SHELLS

    # /etc/profile
    cat > "${INITRAMFS_DIR}/etc/profile" <<'PROFILE'
export PATH=/bin:/sbin:/usr/bin:/usr/sbin
export HOME=/root
export TERM=linux
export PS1='[raven:\w]# '
export RAVEN_LINUX=1
alias ls='ls --color=auto'
alias ll='ls -la'
PROFILE

    # Root's zshrc
    mkdir -p "${INITRAMFS_DIR}/root"
    cat > "${INITRAMFS_DIR}/root/.zshrc" <<'ZSHRC'
export PATH=/bin:/sbin:/usr/bin:/usr/sbin
export HOME=/root
export TERM=linux
export RAVEN_LINUX=1
PROMPT='[raven:%~]# '
alias ls='ls --color=auto'
alias ll='ls -la'
ZSHRC

    log_success "Configuration files created"
}

create_init() {
    log_step "Creating init script..."

    cat > "${INITRAMFS_DIR}/init" <<'INITSCRIPT'
#!/bin/bash
# RavenLinux Live Boot Init
# Mounts the squashfs filesystem from the live ISO and switches to it

export PATH=/bin:/sbin:/usr/bin:/usr/sbin

# Helper functions
msg() { echo -e "\033[1;34m::\033[0m $1"; }
err() { echo -e "\033[1;31mERROR:\033[0m $1"; }
rescue_shell() {
    err "Boot failed! Dropping to rescue shell..."
    err "Reason: $1"
    echo ""
    echo "  You can try to fix the problem manually."
    echo "  Type 'reboot' to restart, 'poweroff' to shut down."
    echo ""
    exec /bin/bash
}

msg "Starting Raven Linux..."

# Mount essential filesystems
msg "Mounting virtual filesystems..."
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev 2>/dev/null || mount -t tmpfs tmpfs /dev
mkdir -p /dev/pts /dev/shm
mount -t devpts devpts /dev/pts
mount -t tmpfs tmpfs /dev/shm

# Create mount points
mkdir -p /mnt/cdrom /mnt/squashfs /mnt/root /mnt/overlay /mnt/work

# Suppress kernel messages for cleaner output
dmesg -n 1 2>/dev/null || true

# Wait for devices to settle (CD-ROM may take a moment)
msg "Waiting for devices..."
sleep 2

# Find the boot device (CD-ROM with our live filesystem)
BOOT_DEVICE=""
ISO_LABEL="RAVEN_LIVE"

msg "Searching for boot device..."

# Method 1: Look for device with our label using blkid
if command -v blkid &>/dev/null; then
    for dev in /dev/sr0 /dev/sr1 /dev/cdrom /dev/dvd; do
        if [ -b "$dev" ]; then
            label=$(blkid -o value -s LABEL "$dev" 2>/dev/null)
            if [ "$label" = "$ISO_LABEL" ]; then
                BOOT_DEVICE="$dev"
                msg "Found boot device by label: $BOOT_DEVICE"
                break
            fi
        fi
    done
fi

# Method 2: Try common CD-ROM devices
if [ -z "$BOOT_DEVICE" ]; then
    for dev in /dev/sr0 /dev/sr1 /dev/cdrom /dev/loop0; do
        if [ -b "$dev" ]; then
            BOOT_DEVICE="$dev"
            msg "Using device: $BOOT_DEVICE"
            break
        fi
    done
fi

if [ -z "$BOOT_DEVICE" ]; then
    rescue_shell "No boot device found"
fi

# Mount the CD-ROM/ISO
msg "Mounting boot device..."
if ! mount -t iso9660 -o ro "$BOOT_DEVICE" /mnt/cdrom 2>/dev/null; then
    # Try auto detection
    if ! mount -o ro "$BOOT_DEVICE" /mnt/cdrom 2>/dev/null; then
        rescue_shell "Failed to mount boot device: $BOOT_DEVICE"
    fi
fi

# Check for squashfs
SQUASHFS="/mnt/cdrom/raven/filesystem.squashfs"
if [ ! -f "$SQUASHFS" ]; then
    # Try alternative locations
    for alt in "/mnt/cdrom/live/filesystem.squashfs" "/mnt/cdrom/squashfs.img"; do
        if [ -f "$alt" ]; then
            SQUASHFS="$alt"
            break
        fi
    done
fi

if [ ! -f "$SQUASHFS" ]; then
    ls -la /mnt/cdrom/ 2>/dev/null
    rescue_shell "Squashfs not found at $SQUASHFS"
fi

msg "Found squashfs: $SQUASHFS"

# Mount the squashfs
msg "Mounting squashfs filesystem..."
if ! mount -t squashfs -o ro,loop "$SQUASHFS" /mnt/squashfs 2>/dev/null; then
    rescue_shell "Failed to mount squashfs"
fi

# Create overlay filesystem for writable live system
msg "Setting up overlay filesystem..."
mount -t tmpfs tmpfs /mnt/overlay
mkdir -p /mnt/overlay/upper /mnt/overlay/work

if mount -t overlay overlay -o lowerdir=/mnt/squashfs,upperdir=/mnt/overlay/upper,workdir=/mnt/overlay/work /mnt/root 2>/dev/null; then
    msg "Overlay mounted successfully"
else
    # Fallback: mount squashfs directly (read-only)
    msg "Overlay failed, using read-only root"
    mount --bind /mnt/squashfs /mnt/root
fi

# Move the virtual filesystems to the new root
msg "Preparing to switch root..."
mkdir -p /mnt/root/proc /mnt/root/sys /mnt/root/dev /mnt/root/mnt/cdrom

mount --move /proc /mnt/root/proc
mount --move /sys /mnt/root/sys
mount --move /dev /mnt/root/dev
mount --bind /mnt/cdrom /mnt/root/mnt/cdrom 2>/dev/null || true

# Find init in the new root
NEW_INIT=""
for init in /init /sbin/init /bin/init; do
    if [ -x "/mnt/root$init" ]; then
        NEW_INIT="$init"
        break
    fi
done

if [ -z "$NEW_INIT" ]; then
    # Fallback to shell
    NEW_INIT="/bin/bash"
fi

msg "Switching to live filesystem..."
msg "Running $NEW_INIT"

# Switch to the new root
exec switch_root /mnt/root "$NEW_INIT"

# If switch_root fails
rescue_shell "switch_root failed"
INITSCRIPT

    chmod +x "${INITRAMFS_DIR}/init"

    log_success "Init script created"
}

create_initramfs() {
    log_step "Creating initramfs image..."

    cd "${INITRAMFS_DIR}"

    # Create uncompressed cpio archive first
    find . | cpio -o -H newc > "${RAVEN_BUILD}/initramfs.cpio" 2>/dev/null

    # Then compress it
    gzip -9 -f "${RAVEN_BUILD}/initramfs.cpio"
    mv "${RAVEN_BUILD}/initramfs.cpio.gz" "${OUTPUT}"

    # Verify it worked
    local size
    size=$(du -h "${OUTPUT}" | cut -f1)

    if [[ $(stat -c%s "${OUTPUT}") -lt 1000 ]]; then
        log_fatal "Initramfs creation failed - file too small"
    fi

    log_success "Initramfs created: ${OUTPUT} (${size})"
}

print_summary() {
    log_section "RavenLinux Initramfs Built"

    echo "  Initramfs: ${OUTPUT}"
    echo ""
    echo "  To test, run:"
    echo "    ./scripts/quick-test.sh -i ${OUTPUT}"
    echo ""
    echo "  Or with graphics:"
    echo "    ./scripts/quick-test.sh -i ${OUTPUT} -g"
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
    # Initialize logging
    init_logging "build-initramfs" "RavenLinux Initramfs Build"
    enable_logging_trap

    log_section "RavenLinux Initramfs Builder"

    if is_logging_enabled; then
        echo "  Log File: $(get_log_file)"
        echo ""
    fi

    check_root
    cleanup
    create_directory_structure
    copy_binaries
    copy_libraries
    create_device_nodes
    create_config_files
    create_init
    create_initramfs
    print_summary

    finalize_logging 0
}

main "$@"
