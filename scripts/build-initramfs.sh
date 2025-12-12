#!/bin/bash
# Build a minimal RavenLinux initramfs for testing
# Uses host system tools - not a full build, just for quick iteration

set -euo pipefail

RAVEN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RAVEN_BUILD="${RAVEN_ROOT}/build"
INITRAMFS_DIR="${RAVEN_BUILD}/initramfs"
OUTPUT="${RAVEN_BUILD}/initramfs-raven.img"

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

check_root() {
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root (need to copy device nodes)"
    fi
}

cleanup() {
    log_info "Cleaning up old build..."
    rm -rf "${INITRAMFS_DIR}"
    mkdir -p "${INITRAMFS_DIR}"
}

create_directory_structure() {
    log_info "Creating directory structure..."

    mkdir -p "${INITRAMFS_DIR}"/{bin,sbin,usr/bin,usr/sbin,usr/lib,lib,lib64}
    mkdir -p "${INITRAMFS_DIR}"/{dev,proc,sys,run,tmp,mnt,root}
    mkdir -p "${INITRAMFS_DIR}"/etc/{raven,rvn}
    mkdir -p "${INITRAMFS_DIR}"/var/{log,tmp}

    log_success "Directory structure created"
}

copy_binaries() {
    log_info "Copying essential binaries..."

    local UUTILS_BIN="${RAVEN_BUILD}/bin/coreutils"

    # Check if uutils is built
    if [[ ! -f "${UUTILS_BIN}" ]]; then
        log_error "uutils-coreutils not built. Run: ./scripts/build-uutils.sh"
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
    local host_bins=(mount umount dmesg clear reset ps kill free grep sed awk find xargs poweroff reboot)
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
    log_info "Copying required libraries..."

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
    log_info "Creating device nodes..."

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
    log_info "Creating configuration files..."

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
    log_info "Creating init script..."

    cat > "${INITRAMFS_DIR}/init" <<'INITSCRIPT'
#!/bin/bash
# RavenLinux minimal init

# Set PATH immediately so symlinked commands work
export PATH=/bin:/sbin:/usr/bin:/usr/sbin

echo "Starting Raven Linux..."

# Mount essential filesystems
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev 2>/dev/null || true
/bin/coreutils mkdir -p /dev/pts
mount -t devpts devpts /dev/pts

# Set hostname
/bin/coreutils hostname raven 2>/dev/null || hostname raven 2>/dev/null || true

# Basic kernel messages
dmesg -n 1 2>/dev/null || true

clear 2>/dev/null || true
echo ""
echo "  ====================================="
echo "  |       R A V E N   L I N U X       |"
echo "  ====================================="
echo ""
echo "  Welcome to Raven Linux!"
echo "  This is a minimal test environment."
echo ""
/bin/coreutils cat /etc/os-release
echo ""
echo "  Type 'poweroff' to shutdown"
echo "  Or press Ctrl+A, X to exit QEMU"
echo ""

# Check for zsh, fall back to bash
if [ -x /bin/zsh ]; then
    exec /bin/zsh -l
elif [ -x /bin/bash ]; then
    exec /bin/bash -l
else
    exec /bin/bash
fi
INITSCRIPT

    chmod +x "${INITRAMFS_DIR}/init"

    log_success "Init script created"
}

create_initramfs() {
    log_info "Creating initramfs image..."

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
        log_error "Initramfs creation failed - file too small"
    fi

    log_success "Initramfs created: ${OUTPUT} (${size})"
}

print_summary() {
    echo ""
    echo "========================================"
    echo "  RavenLinux Initramfs Built"
    echo "========================================"
    echo ""
    echo "  Initramfs: ${OUTPUT}"
    echo ""
    echo "  To test, run:"
    echo "    ./scripts/quick-test.sh -i ${OUTPUT}"
    echo ""
    echo "  Or with graphics:"
    echo "    ./scripts/quick-test.sh -i ${OUTPUT} -g"
    echo ""
}

# Main
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
