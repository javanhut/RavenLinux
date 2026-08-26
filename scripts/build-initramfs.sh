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
INITRAMFS_DIR="${RAVEN_INITRAMFS_DIR:-${RAVEN_BUILD}/initramfs}"
OUTPUT="${RAVEN_INITRAMFS_OUTPUT:-${RAVEN_BUILD}/initramfs-raven.img}"

# Source shared logging library
source "${SCRIPT_DIR}/lib/logging.sh"

# Options
NO_DEVNODES=false

# =============================================================================
# Argument Parsing
# =============================================================================

while [[ $# -gt 0 ]]; do
    case "$1" in
        --no-log)
            export RAVEN_NO_LOG=1
            shift
            ;;
        --no-devnodes)
            NO_DEVNODES=true
            shift
            ;;
        *)
            log_error "Unknown option: $1"
            echo "Usage: $0 [--no-log] [--no-devnodes]"
            exit 1
            ;;
    esac
done

# =============================================================================
# Functions
# =============================================================================

check_dependencies() {
    local missing=()
    
    for cmd in cpio gzip find; do
        if ! command -v "$cmd" &>/dev/null; then
            missing+=("$cmd")
        fi
    done
    
    if [[ ${#missing[@]} -gt 0 ]]; then
        echo "ERROR: Missing required tools: ${missing[*]}"
        echo ""
        echo "On Arch Linux, install with:"
        echo "  sudo pacman -S cpio gzip findutils"
        echo ""
        exit 1
    fi
}

fixup_soname_symlink() {
    local dir="$1"
    local soname="$2"

    [[ -d "$dir" ]] || return 0

    local latest
    latest="$(ls -1 "${dir}/${soname}."* 2>/dev/null | sort -V | tail -n 1 || true)"
    [[ -n "$latest" ]] || return 0

    ln -sf "$(basename "$latest")" "${dir}/${soname}" 2>/dev/null || true
}

fixup_readline_history_symlinks() {
    local dir="$1"
    fixup_soname_symlink "$dir" "libreadline.so.8"
    fixup_soname_symlink "$dir" "libhistory.so.8"
}

copy_sysroot_library_by_name() {
    local libname="$1"
    local sysroot="${RAVEN_BUILD}/sysroot"

    [[ -d "$sysroot" ]] || return 1

    local candidate=""
    local search_dirs=(
        "${sysroot}/lib"
        "${sysroot}/lib64"
        "${sysroot}/usr/lib"
        "${sysroot}/usr/lib64"
    )

    for dir in "${search_dirs[@]}"; do
        if [[ -e "${dir}/${libname}" ]]; then
            candidate="${dir}/${libname}"
            break
        fi
    done

    if [[ -z "$candidate" ]]; then
        candidate="$(find "$sysroot" -maxdepth 6 -name "$libname" \( -type f -o -type l \) 2>/dev/null | head -n 1 || true)"
    fi

    [[ -n "$candidate" ]] || return 1

    local rel="${candidate#${sysroot}}"
    local dest="${INITRAMFS_DIR}${rel}"
    mkdir -p "$(dirname "$dest")"
    cp -L "$candidate" "$dest" 2>/dev/null || true
    return 0
}

ensure_libc_compat_links() {
    # Musl binaries commonly need libc.so and/or libc.musl-ARCH.so.1, but ldd output
    # often only gives us the loader path (/lib/ld-musl-ARCH.so.1). Ensure the common
    # names exist so basic tools like mkdir/sleep work in early boot.
    local musl_loader=""
    musl_loader="$(ls -1 "${INITRAMFS_DIR}"/lib/ld-musl-*.so.1 2>/dev/null | head -n 1 || true)"

    if [[ -n "$musl_loader" ]]; then
        local loader_path="${musl_loader#${INITRAMFS_DIR}}"

        mkdir -p "${INITRAMFS_DIR}/lib" "${INITRAMFS_DIR}/usr/lib"
        ln -sf "$loader_path" "${INITRAMFS_DIR}/lib/libc.so" 2>/dev/null || true
        ln -sf "$loader_path" "${INITRAMFS_DIR}/usr/lib/libc.so" 2>/dev/null || true

        local loader_base
        loader_base="$(basename "$musl_loader")"
        local arch="${loader_base#ld-musl-}"
        arch="${arch%.so.1}"

        if [[ -n "$arch" && "$arch" != "$loader_base" ]]; then
            ln -sf "$loader_path" "${INITRAMFS_DIR}/lib/libc.musl-${arch}.so.1" 2>/dev/null || true
            ln -sf "$loader_path" "${INITRAMFS_DIR}/usr/lib/libc.musl-${arch}.so.1" 2>/dev/null || true
        fi

        return 0
    fi

    # glibc compatibility: some mislinked binaries may look for libc.so (no SONAME).
    local glibc_lib=""
    if [[ -e "${INITRAMFS_DIR}/lib/libc.so.6" ]]; then
        glibc_lib="/lib/libc.so.6"
    elif [[ -e "${INITRAMFS_DIR}/usr/lib/libc.so.6" ]]; then
        glibc_lib="/usr/lib/libc.so.6"
    elif [[ -e "${INITRAMFS_DIR}/lib64/libc.so.6" ]]; then
        glibc_lib="/lib64/libc.so.6"
    elif [[ -e "${INITRAMFS_DIR}/usr/lib64/libc.so.6" ]]; then
        glibc_lib="/usr/lib64/libc.so.6"
    fi

    if [[ -n "$glibc_lib" ]]; then
        mkdir -p "${INITRAMFS_DIR}/lib" "${INITRAMFS_DIR}/usr/lib"
        ln -sf "$glibc_lib" "${INITRAMFS_DIR}/lib/libc.so" 2>/dev/null || true
        ln -sf "$glibc_lib" "${INITRAMFS_DIR}/usr/lib/libc.so" 2>/dev/null || true
    fi
}

check_root() {
    if [[ $EUID -ne 0 ]]; then
        if [[ "$NO_DEVNODES" == "true" ]]; then
            log_warn "Running unprivileged (--no-devnodes): device nodes will not be created in initramfs"
            log_warn "This requires devtmpfs to mount successfully at boot (no /dev tmpfs fallback)"
            return 0
        fi

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

    # whoami: uutils multicall in this tree expects "coreutils whoami", so provide a standalone shim
    rm -f "${INITRAMFS_DIR}/bin/whoami" 2>/dev/null || true
    cat > "${INITRAMFS_DIR}/bin/whoami" << 'EOF'
#!/bin/sh

uid=""
if command -v id >/dev/null 2>&1; then
    uid="$(id -u 2>/dev/null || true)"
fi

case "$uid" in
    ''|*[!0-9]*) uid="" ;;
esac

if [ -z "$uid" ] && [ -r /proc/self/status ]; then
    while IFS= read -r line; do
        case "$line" in
            Uid:*)
                set -- $line
                uid="$2"
                break
                ;;
        esac
    done < /proc/self/status
fi

case "$uid" in
    ''|*[!0-9]*) uid="" ;;
esac

if [ -z "$uid" ]; then
    uid="${UID:-}"
fi

name=""
if [ -n "$uid" ] && [ -r /etc/passwd ]; then
    while IFS=: read -r pw_name _ pw_uid _ _ _ _; do
        if [ "$pw_uid" = "$uid" ]; then
            name="$pw_name"
            break
        fi
    done < /etc/passwd
fi

if [ -z "$name" ]; then
    name="${USER:-${LOGNAME:-unknown}}"
fi

printf '%s\n' "$name"
EOF
    chmod 755 "${INITRAMFS_DIR}/bin/whoami" 2>/dev/null || true

    # These need to come from host (not in uutils or need special handling)
    # Include switch_root for live boot, udevadm for device enumeration
    local host_bins=(mount umount dmesg clear reset ps kill free grep sed awk find xargs switch_root losetup blkid udevadm setsid stty)
    for bin in "${host_bins[@]}"; do
        if command -v "$bin" &>/dev/null; then
            cp "$(which "$bin")" "${INITRAMFS_DIR}/bin/" 2>/dev/null || true
        fi
    done

    # reboot/poweroff are deliberately NOT copied from the host. On a systemd
    # distro those binaries are systemd's, and in an initramfs they fail with
    #   System has not been booted with systemd as init system (PID 1)
    # leaving the rescue shell unable to do the two things it tells you to do.
    # sysrq is compiled into the kernel, so drive it directly instead.
    for action in reboot:b poweroff:o halt:o; do
        cat > "${INITRAMFS_DIR}/bin/${action%%:*}" <<EOF
#!/bin/sh
sync
[ -w /proc/sys/kernel/sysrq ] && echo 1 > /proc/sys/kernel/sysrq
echo ${action##*:} > /proc/sysrq-trigger
EOF
        chmod 755 "${INITRAMFS_DIR}/bin/${action%%:*}"
    done
    log_info "  Added reboot/poweroff/halt (sysrq-based)"

    # Copy bash
    if [[ -f "${RAVEN_BUILD}/sysroot/bin/bash" ]]; then
        cp "${RAVEN_BUILD}/sysroot/bin/bash" "${INITRAMFS_DIR}/bin/bash"
        ln -sf bash "${INITRAMFS_DIR}/bin/sh"
        log_info "  Added bash (from sysroot)"

        # Copy essential libraries for sysroot bash to ensure compatibility
        mkdir -p "${INITRAMFS_DIR}/usr/lib" "${INITRAMFS_DIR}/lib" "${INITRAMFS_DIR}/lib64"
        
        # Copy libs from sysroot/usr/lib
        pushd "${RAVEN_BUILD}/sysroot/usr/lib" >/dev/null
        # Use cp -d to preserve symlinks
        # Copy to both /usr/lib and /lib to be safe
        cp -d libreadline.so* libncursesw.so* libtinfow.so* libdl.so* libc.so* libgcc_s.so* "${INITRAMFS_DIR}/usr/lib/" 2>/dev/null || true
        cp -d libreadline.so* libncursesw.so* libtinfow.so* libdl.so* libc.so* libgcc_s.so* "${INITRAMFS_DIR}/lib/" 2>/dev/null || true
        popd >/dev/null

        # Fix up readline/history SONAME symlinks if multiple versions were copied.
        # This prevents early-boot failures like:
        #   /bin/bash: undefined symbol: rl_print_keybinding
        fixup_readline_history_symlinks "${INITRAMFS_DIR}/usr/lib"
        fixup_readline_history_symlinks "${INITRAMFS_DIR}/lib"

        # Verify readline was copied (and SONAME exists)
        if [[ ! -e "${INITRAMFS_DIR}/usr/lib/libreadline.so.8" ]] && [[ ! -e "${INITRAMFS_DIR}/lib/libreadline.so.8" ]]; then
            log_error "Failed to copy libreadline.so.8 for bash!"
        fi

        # Copy dynamic linker from sysroot if present
        if [[ -f "${RAVEN_BUILD}/sysroot/lib64/ld-linux-x86-64.so.2" ]]; then
             cp -L "${RAVEN_BUILD}/sysroot/lib64/ld-linux-x86-64.so.2" "${INITRAMFS_DIR}/lib64/" 2>/dev/null || true
             # Ensure it's also in /lib just in case
             mkdir -p "${INITRAMFS_DIR}/lib"
             cp -L "${RAVEN_BUILD}/sysroot/lib64/ld-linux-x86-64.so.2" "${INITRAMFS_DIR}/lib/" 2>/dev/null || true
        fi
    elif command -v bash &>/dev/null; then
        cp "$(which bash)" "${INITRAMFS_DIR}/bin/bash"
        ln -sf bash "${INITRAMFS_DIR}/bin/sh"
        log_info "  Added bash (from host)"
    fi

    # Copy RavenLinux custom packages (Vem, Carrion, Ivaldi)
    local PACKAGES_BIN="${RAVEN_BUILD}/packages/bin"
    if [[ -d "${PACKAGES_BIN}" ]]; then
        log_info "Copying RavenLinux custom packages..."
        for pkg in vem carrion ivaldi raven-dhcp; do
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

    local -A missing_libs=()

    # Find and copy required libraries for binaries in initramfs
    for bin in "${INITRAMFS_DIR}"/bin/*; do
        [[ -f "$bin" && -x "$bin" && ! -L "$bin" ]] || continue

        # Skip statically linked binaries (vem, carrion, ivaldi are static Go binaries)
        if file "$bin" 2>/dev/null | grep -q "statically linked"; then
            continue
        fi

        # Use timeout to avoid hanging on problematic binaries
        local ldd_out
        ldd_out="$(timeout 2 ldd "$bin" 2>/dev/null || true)"

        # Capture any "not found" dependencies so we can try to satisfy them from sysroot.
        while IFS= read -r libname; do
            [[ -n "$libname" ]] || continue
            missing_libs["$libname"]=1
        done < <(printf '%s\n' "$ldd_out" | awk '/=> not found/ {print $1}' || true)

        # Copy resolved library paths. Use process substitution to avoid pipefail subshell issues.
        while read -r lib; do
            [[ -z "$lib" || ! -f "$lib" ]] && continue
            local dest="${INITRAMFS_DIR}${lib}"
            # Check if we should copy from sysroot instead of host path
            if [[ -f "${RAVEN_BUILD}/sysroot${lib}" ]]; then
                 # Prefer sysroot lib if we have a matching path
                 if [[ ! -f "$dest" ]]; then
                    mkdir -p "$(dirname "$dest")"
                    cp -L "${RAVEN_BUILD}/sysroot${lib}" "$dest" 2>/dev/null || true
                 fi
            else
                if [[ ! -f "$dest" ]]; then
                    mkdir -p "$(dirname "$dest")"
                    cp -L "$lib" "$dest" 2>/dev/null || true
                fi
            fi
        done < <(printf '%s\n' "$ldd_out" | grep -o '/[^ ]*' || true)
    done

    # Copy dynamic linker
    for ld in /lib64/ld-linux-x86-64.so.2 /lib/ld-linux-x86-64.so.2; do
        if [[ -f "$ld" ]]; then
            mkdir -p "${INITRAMFS_DIR}$(dirname "$ld")"
            if [[ -f "${RAVEN_BUILD}/sysroot${ld}" ]]; then
                 cp -L "${RAVEN_BUILD}/sysroot${ld}" "${INITRAMFS_DIR}${ld}" 2>/dev/null || true
            else
                 cp -L "$ld" "${INITRAMFS_DIR}${ld}" 2>/dev/null || true
            fi
        fi
    done

    # Copy musl loader if present (covers musl-based hosts/sysroots)
    for ld in /lib/ld-musl-*.so.1; do
        if [[ -e "${RAVEN_BUILD}/sysroot${ld}" ]]; then
            mkdir -p "${INITRAMFS_DIR}$(dirname "$ld")"
            cp -L "${RAVEN_BUILD}/sysroot${ld}" "${INITRAMFS_DIR}${ld}" 2>/dev/null || true
        elif [[ -f "$ld" ]]; then
            mkdir -p "${INITRAMFS_DIR}$(dirname "$ld")"
            cp -L "$ld" "${INITRAMFS_DIR}${ld}" 2>/dev/null || true
        fi
    done

    # Try to satisfy any unresolved libs from sysroot by name (common when using a custom sysroot)
    if [[ ${#missing_libs[@]} -gt 0 ]]; then
        log_info "Resolving missing libraries from sysroot..."
        for libname in "${!missing_libs[@]}"; do
            # libc.so is ambiguous (glibc linker script vs musl shared libc). Handle via compat links.
            if [[ "$libname" = "libc.so" ]]; then
                continue
            fi
            if ! copy_sysroot_library_by_name "$libname"; then
                log_warn "Could not resolve missing library from sysroot: ${libname}"
            fi
        done
    fi

    # CRITICAL: Create /lib symlink to /usr/lib for library resolution
    # Many binaries are linked expecting libraries in /lib/ but we store them in /usr/lib/
    if [[ -d "${INITRAMFS_DIR}/usr/lib" ]] && [[ ! -L "${INITRAMFS_DIR}/lib" ]]; then
        # Copy essential libraries to /lib/ as well for compatibility
        log_info "Copying essential libraries to /lib/ for compatibility..."
        mkdir -p "${INITRAMFS_DIR}/lib"
        for lib in libc.so.6 libm.so.6 libdl.so.2 libpthread.so.0 librt.so.1 \
                   libgcc_s.so.1 libcrypt.so.2 libresolv.so.2 libnss_files.so.2 \
                   libnss_dns.so.2; do
            if [[ -f "${INITRAMFS_DIR}/usr/lib/${lib}" ]]; then
                cp -L "${INITRAMFS_DIR}/usr/lib/${lib}" "${INITRAMFS_DIR}/lib/${lib}" 2>/dev/null || true
            fi
        done
    fi

    ensure_libc_compat_links

    log_success "Libraries copied"
}

create_device_nodes() {
    log_step "Creating device nodes..."

    if [[ "$NO_DEVNODES" == "true" ]]; then
        log_warn "Skipping device node creation (--no-devnodes)"
        return 0
    fi

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
    echo "raven-linux" > "${INITRAMFS_DIR}/etc/hostname"

    # /etc/passwd
    cat > "${INITRAMFS_DIR}/etc/passwd" <<'PASSWD'
root:x:0:0:root:/root:/bin/bash
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
SHELLS

    # /etc/profile
    cat > "${INITRAMFS_DIR}/etc/profile" <<'PROFILE'
export PATH=/bin:/sbin:/usr/bin:/usr/sbin
export LD_LIBRARY_PATH=/lib:/usr/lib:/lib64:/usr/lib64
export HOME=/root
export TERM=linux
export PS1='[\u@raven-linux]# '
export RAVEN_LINUX=1
alias ls='ls --color=auto'
alias ll='ls -la'
PROFILE

    # Root's bashrc
    mkdir -p "${INITRAMFS_DIR}/root"
    cat > "${INITRAMFS_DIR}/root/.bashrc" <<'BASHRC'
export PATH=/bin:/sbin:/usr/bin:/usr/sbin
export LD_LIBRARY_PATH=/lib:/usr/lib:/lib64:/usr/lib64
export HOME=/root
export TERM=linux
export RAVEN_LINUX=1
PS1='[\u@raven-linux]# '
alias ls='ls --color=auto'
alias ll='ls -la'
BASHRC

    log_success "Configuration files created"
}

create_init() {
    log_step "Creating init script..."

    cat > "${INITRAMFS_DIR}/init" <<'INITSCRIPT'
#!/bin/bash
# RavenLinux Live Boot Init
# Mounts the squashfs filesystem from the live ISO and switches to it

export PATH=/bin:/sbin:/usr/bin:/usr/sbin
export LD_LIBRARY_PATH=/lib:/usr/lib:/lib64:/usr/lib64

# =============================================================================
# Color and Status Output Functions (systemd/OpenRC style)
# =============================================================================
RED='\033[1;31m'
GREEN='\033[1;32m'
YELLOW='\033[1;33m'
BLUE='\033[1;34m'
WHITE='\033[1;37m'
CYAN='\033[1;36m'
NC='\033[0m'

ok()   { echo -e "  [  ${GREEN}OK${NC}  ] $1"; }
warn() { echo -e "  [${YELLOW}WARN${NC} ] $1"; }
fail() { echo -e "  [${RED}FAIL${NC} ] $1"; }
info() { echo -e "  [ ${BLUE}**${NC}  ] $1"; }
step() { echo -e "\n${WHITE}>>>${NC} $1"; }

rescue_shell() {
    echo ""
    fail "Boot failed: $1"
    echo ""
    echo -e "  ${YELLOW}You can try to fix the problem manually.${NC}"
    echo -e "  Type ${WHITE}'reboot'${NC} to restart, ${WHITE}'poweroff'${NC} to shut down."
    echo ""
    while true; do
        if command -v setsid >/dev/null && [ -c /dev/console ]; then
            setsid -c /bin/bash --login -i </dev/console >/dev/console 2>&1 || true
        else
            /bin/bash --login -i </dev/console >/dev/console 2>&1 || true
        fi
        echo ""
        warn "Shell exited. Restarting rescue shell..."
        sleep 1
    done
}

# =============================================================================
# Boot Sequence
# =============================================================================

# Clear screen and center the boot banner
clear 2>/dev/null || printf '\033[2J\033[H'

# Add vertical spacing to center (assuming ~24 line terminal, banner is ~10 lines)
echo ""
echo ""
echo ""
echo ""
echo ""
echo ""

# The subtitle has to tell the truth. This one initramfs boots both the live
# ISO and every installed disk, and it said "Live Boot" on both -- so a machine
# with Raven installed announced itself as a live image on every boot.
#
# The kernel command line is the only thing that distinguishes them at this
# point: the ISO entries carry rdinit=/init and no root=, while raven-install
# writes root=UUID=... into boot.cfg. That is the same signal
# raven_root_from_cmdline uses further down to pick disk mode over live mode;
# this reads it early, before /proc is officially mounted, because the banner
# is printed first. `raven.live` is the documented escape hatch for booting the
# live image from a disk that does have a root=, so it wins here too.
[ -r /proc/cmdline ] || mount -t proc proc /proc 2>/dev/null || true
RAVEN_BOOT_LABEL="Live Boot"
if [ -r /proc/cmdline ]; then
    if grep -qE '(^| )root=[^ ]' /proc/cmdline 2>/dev/null \
        && ! grep -qE '(^| )raven\.live( |$)' /proc/cmdline 2>/dev/null; then
        RAVEN_BOOT_LABEL="Installed System"
    fi
fi

# Display centered boot banner
echo -e "${CYAN}              ██████╗  █████╗ ██╗   ██╗███████╗███╗   ██╗    ██╗     ██╗███╗   ██╗██╗   ██╗██╗  ██╗${NC}"
echo -e "${CYAN}              ██╔══██╗██╔══██╗██║   ██║██╔════╝████╗  ██║    ██║     ██║████╗  ██║██║   ██║╚██╗██╔╝${NC}"
echo -e "${CYAN}              ██████╔╝███████║██║   ██║█████╗  ██╔██╗ ██║    ██║     ██║██╔██╗ ██║██║   ██║ ╚███╔╝${NC}"
echo -e "${CYAN}              ██╔══██╗██╔══██║╚██╗ ██╔╝██╔══╝  ██║╚██╗██║    ██║     ██║██║╚██╗██║██║   ██║ ██╔██╗${NC}"
echo -e "${CYAN}              ██║  ██║██║  ██║ ╚████╔╝ ███████╗██║ ╚████║    ███████╗██║██║ ╚████║╚██████╔╝██╔╝ ██╗${NC}"
echo -e "${CYAN}              ╚═╝  ╚═╝╚═╝  ╚═╝  ╚═══╝  ╚══════╝╚═╝  ╚═══╝    ╚══════╝╚═╝╚═╝  ╚═══╝ ╚═════╝ ╚═╝  ╚═╝${NC}"
echo ""
# Centred under the logo. 46 was the hard-coded indent when the only possible
# label was "Live Boot" (9 characters), so the centre stays exactly where it
# was and a longer label grows evenly either side of it.
RAVEN_BOOT_PAD=$(( 46 + (9 - ${#RAVEN_BOOT_LABEL}) / 2 ))
[ "$RAVEN_BOOT_PAD" -lt 0 ] && RAVEN_BOOT_PAD=0
printf '%*s' "$RAVEN_BOOT_PAD" ""
echo -e "${WHITE}${RAVEN_BOOT_LABEL}${NC}"
echo ""
echo ""

# -----------------------------------------------------------------------------
# Mount Virtual Filesystems
# -----------------------------------------------------------------------------
step "Mounting virtual filesystems"

# Possibly already mounted: the banner above needs the command line to know
# whether this is a live boot or an installed disk, and mounts /proc to read
# it. Mounting it twice is harmless, but a kernel that refuses is not a
# failure when the thing we wanted is already there.
if mount -t proc proc /proc 2>/dev/null; then
    ok "Mounted /proc"
elif [ -r /proc/cmdline ]; then
    ok "Mounted /proc (already mounted for the boot banner)"
else
    fail "Failed to mount /proc"
    rescue_shell "Critical filesystem mount failed"
fi

if mount -t sysfs sysfs /sys 2>/dev/null; then
    ok "Mounted /sys"
else
    fail "Failed to mount /sys"
    rescue_shell "Critical filesystem mount failed"
fi

if mount -t devtmpfs devtmpfs /dev 2>/dev/null; then
    ok "Mounted /dev (devtmpfs)"
elif mount -t tmpfs tmpfs /dev 2>/dev/null; then
    ok "Mounted /dev (tmpfs fallback)"
else
    fail "Failed to mount /dev"
    rescue_shell "Critical filesystem mount failed"
fi

mkdir -p /dev/pts /dev/shm

if mount -t devpts devpts /dev/pts 2>/dev/null; then
    ok "Mounted /dev/pts"
else
    warn "Failed to mount /dev/pts (non-critical)"
fi

if mount -t tmpfs tmpfs /dev/shm 2>/dev/null; then
    ok "Mounted /dev/shm"
else
    warn "Failed to mount /dev/shm (non-critical)"
fi

# Setup /dev/fd symlinks for bash process substitution
if [ ! -e /dev/fd ] && [ -d /proc/self/fd ]; then
    ln -sf /proc/self/fd /dev/fd 2>/dev/null && \
    ln -sf /proc/self/fd/0 /dev/stdin 2>/dev/null && \
    ln -sf /proc/self/fd/1 /dev/stdout 2>/dev/null && \
    ln -sf /proc/self/fd/2 /dev/stderr 2>/dev/null && \
    ok "Created /dev/fd symlinks" || \
    warn "Could not create /dev/fd symlinks"
fi

# Create mount points
mkdir -p /mnt/cdrom /mnt/squashfs /mnt/root /mnt/overlay /mnt/work

# Suppress kernel messages for cleaner output
dmesg -n 1 2>/dev/null || true

# -----------------------------------------------------------------------------
# Wait for Devices
# -----------------------------------------------------------------------------
step "Waiting for devices to settle"

info "Waiting 3 seconds for device enumeration..."
sleep 3

if command -v udevadm &>/dev/null; then
    udevadm trigger 2>/dev/null || true
    udevadm settle --timeout=5 2>/dev/null || true
    ok "Device enumeration complete (udev)"
else
    ok "Device enumeration complete"
fi

# -----------------------------------------------------------------------------
# Root Device From the Kernel Command Line
# -----------------------------------------------------------------------------
# The live path below hunts for a squashfs on removable media. An installed
# system has no squashfs and no removable media: it passes root=UUID=... and
# expects that partition mounted directly. Both paths end at the same
# switch_root, so everything after the branch is shared.
#
# raven-install greps this initramfs for the name of the function below to tell
# whether the image it is about to copy can boot from a disk at all. Renaming it
# is fine; renaming it without updating the installer is not.

RAVEN_ROOT_MODE="live"
RAVEN_ROOT_SPEC=""
RAVEN_ROOT_FSTYPE=""
RAVEN_ROOT_FLAGS=""
RAVEN_ROOT_RW="rw"
RAVEN_ROOT_WAIT=30
RAVEN_INIT_OVERRIDE=""
# Declared out here because the switch_root preparation reads it on both paths.
RAVEN_MEDIA_MNT=""

raven_root_from_cmdline() {
    for arg in $(cat /proc/cmdline 2>/dev/null); do
        case "$arg" in
            root=*)       RAVEN_ROOT_SPEC="${arg#root=}" ;;
            rootfstype=*) RAVEN_ROOT_FSTYPE="${arg#rootfstype=}" ;;
            rootflags=*)  RAVEN_ROOT_FLAGS="${arg#rootflags=}" ;;
            rootwait)     RAVEN_ROOT_WAIT=60 ;;
            rootdelay=*)  RAVEN_ROOT_WAIT="${arg#rootdelay=}" ;;
            init=*)       RAVEN_INIT_OVERRIDE="${arg#init=}" ;;
            ro)           RAVEN_ROOT_RW="ro" ;;
            rw)           RAVEN_ROOT_RW="rw" ;;
            # Escape hatch: boot the live image even from a disk that has a
            # root= on its command line.
            raven.live)   RAVEN_ROOT_SPEC="" ; return 0 ;;
        esac
    done

    [ -n "$RAVEN_ROOT_SPEC" ] && RAVEN_ROOT_MODE="disk"
    return 0
}

# Turn a root= value into a block device path. Supports the four forms a
# bootloader can reasonably hand us plus a bare device name.
raven_resolve_root() {
    spec="$1"
    dev=""

    case "$spec" in
        UUID=*)
            dev=$(blkid -U "${spec#UUID=}" 2>/dev/null)
            ;;
        PARTUUID=*)
            dev=$(blkid -t "PARTUUID=${spec#PARTUUID=}" -o device 2>/dev/null | head -1)
            ;;
        LABEL=*)
            dev=$(blkid -L "${spec#LABEL=}" 2>/dev/null)
            ;;
        PARTLABEL=*)
            dev=$(blkid -t "PARTLABEL=${spec#PARTLABEL=}" -o device 2>/dev/null | head -1)
            ;;
        /dev/*)
            dev="$spec"
            ;;
        *)
            dev="/dev/$spec"
            ;;
    esac

    [ -n "$dev" ] && [ -b "$dev" ] || return 1
    echo "$dev"
    return 0
}

raven_mount_disk_root() {
    info "Root requested on the command line: $RAVEN_ROOT_SPEC"

    # NVMe in particular regularly needs longer than the settle above. Poll
    # rather than sleeping a fixed amount, so a fast disk costs nothing.
    root_dev=""
    waited=0
    while [ "$waited" -lt "$RAVEN_ROOT_WAIT" ]; do
        root_dev=$(raven_resolve_root "$RAVEN_ROOT_SPEC") && break
        root_dev=""
        [ "$waited" -eq 0 ] && info "Waiting for $RAVEN_ROOT_SPEC to appear..."
        sleep 1
        waited=$((waited + 1))
        command -v udevadm >/dev/null 2>&1 && udevadm settle --timeout=1 >/dev/null 2>&1
    done

    if [ -z "$root_dev" ]; then
        fail "Root device not found: $RAVEN_ROOT_SPEC"
        info "Block devices the kernel can see:"
        blkid 2>/dev/null || ls -la /dev/nvme* /dev/sd* /dev/vd* 2>/dev/null || true
        echo ""
        info "If the disk is missing entirely, check the firmware's storage mode:"
        info "RAID or Intel RST hides NVMe from Linux. AHCI is the one that works."
        rescue_shell "Cannot find $RAVEN_ROOT_SPEC"
    fi

    ok "Root device: $root_dev ($RAVEN_ROOT_SPEC)"

    mount_opts="$RAVEN_ROOT_RW"
    [ -n "$RAVEN_ROOT_FLAGS" ] && mount_opts="${mount_opts},${RAVEN_ROOT_FLAGS}"

    if [ -n "$RAVEN_ROOT_FSTYPE" ]; then
        mount -t "$RAVEN_ROOT_FSTYPE" -o "$mount_opts" "$root_dev" /mnt/root 2>/dev/null
    else
        mount -o "$mount_opts" "$root_dev" /mnt/root 2>/dev/null
    fi

    if [ $? -ne 0 ]; then
        # Second chance read-only: a filesystem with a dirty journal the kernel
        # will not replay still mounts ro, and a rescue shell on the real root
        # beats one on an empty initramfs.
        warn "Mounting $root_dev ${RAVEN_ROOT_RW} failed; retrying read-only"
        if ! mount -o ro "$root_dev" /mnt/root 2>/dev/null; then
            fail "Cannot mount $root_dev"
            rescue_shell "Root filesystem would not mount"
        fi
        warn "Root mounted READ-ONLY. Repair it, then remount rw."
    fi

    ok "Mounted root filesystem (${mount_opts})"

    # A root with no init on it is almost always the wrong partition -- the ESP,
    # or a data disk -- and saying so beats "switch_root failed" three steps on.
    if [ ! -x /mnt/root/sbin/init ] && [ ! -x /mnt/root/init ] && [ ! -x /mnt/root/bin/init ]; then
        fail "$root_dev has no init; this does not look like a RavenLinux root"
        info "Contents:"
        ls /mnt/root 2>/dev/null | head -20
        rescue_shell "No init on $root_dev"
    fi

    return 0
}

step "Determining root filesystem"
raven_root_from_cmdline

if [ "$RAVEN_ROOT_MODE" = "disk" ]; then
    raven_mount_disk_root
else
    ok "No root= on the command line; booting the live image"

# The live path runs to the end of the overlay setup, where the branch closes.
# It is left unindented on purpose: it predates the branch, and reindenting it
# would bury a two-line change in a two-hundred-line diff.

# -----------------------------------------------------------------------------
# Find Boot Device
# -----------------------------------------------------------------------------
step "Searching for boot device"

BOOT_DEVICE=""
ISO_LABEL="RAVENLINUX"

# Device globs for the fallback scans below. /dev/mapper and /dev/dm-* are
# here for Ventoy and friends: those boot the ISO as a *file* on a USB stick,
# exposing it as a device-mapper target rather than a real block device, so
# none of the conventional patterns would ever match it. Method 1's bare
# `blkid` enumeration does already catch that case; keeping the globs in step
# means Ventoy support does not rest on that one method alone.
SCAN_DEVICES="/dev/sr* /dev/sd* /dev/nvme*n*p* /dev/vd* /dev/mmcblk*p* /dev/loop* /dev/mapper/* /dev/dm-*"

# A candidate is only the boot device if it actually *mounts* and actually
# holds the squashfs. Probing by label or fstype alone is not enough: Ventoy
# and similar loaders leave other iso9660-looking devices lying around, and
# committing to the first match meant one decoy earlier in the scan order
# dead-ended the boot with "Failed to mount boot device" while the real
# device sat one entry further down the list. So: rank candidates, then try
# them in turn, and keep going until one proves itself.

# Sets SQUASHFS if /mnt/cdrom holds a recognisable live image.
find_squashfs() {
    for candidate in \
        /mnt/cdrom/raven/filesystem.squashfs \
        /mnt/cdrom/live/filesystem.squashfs \
        /mnt/cdrom/squashfs.img; do
        if [ -f "$candidate" ]; then
            SQUASHFS="$candidate"
            return 0
        fi
    done
    return 1
}

# Mount $1 and confirm it carries the squashfs. Leaves it mounted on success,
# unmounted on failure, so the next candidate starts from a clean /mnt/cdrom.
try_boot_device() {
    dev="$1"
    [ -b "$dev" ] 2>/dev/null || return 1

    mount -t iso9660 -o ro "$dev" /mnt/cdrom 2>/dev/null \
        || mount -o ro "$dev" /mnt/cdrom 2>/dev/null \
        || return 1

    if find_squashfs; then
        return 0
    fi

    info "  $dev mounted but carries no squashfs, continuing search"
    umount /mnt/cdrom 2>/dev/null || true
    return 1
}

# Candidates, best first: exact label match, then any iso9660, then the
# conventional optical/loop nodes as a last resort. Duplicates are harmless --
# try_boot_device is cheap and idempotent.
BOOT_CANDIDATES=""
add_candidate() {
    case " $BOOT_CANDIDATES " in
        *" $1 "*) ;;
        *) BOOT_CANDIDATES="$BOOT_CANDIDATES $1" ;;
    esac
}

# Pass 1: whatever blkid already knows about, matched on our label. This is the
# pass that catches device-mapper targets, which no glob below would name.
if command -v blkid &>/dev/null; then
    # Avoid bash process substitution here; it depends on /dev/fd existing.
    for dev in $(blkid 2>/dev/null | awk -F: '{print $1}'); do
        [ -b "$dev" ] 2>/dev/null || continue
        label=$(blkid -o value -s LABEL "$dev" 2>/dev/null)
        [ "$label" = "$ISO_LABEL" ] && add_candidate "$dev"
    done
fi

# Pass 2: the same label match, over the explicit globs, in case blkid's own
# enumeration came up short.
if command -v blkid &>/dev/null; then
    for pattern in $SCAN_DEVICES; do
        for dev in $pattern; do
            [ -b "$dev" ] 2>/dev/null || continue
            label=$(blkid -o value -s LABEL "$dev" 2>/dev/null)
            [ "$label" = "$ISO_LABEL" ] && add_candidate "$dev"
        done
    done
fi

# Pass 3: any iso9660 at all. A rebuilt ISO with a changed label still boots.
if command -v blkid &>/dev/null; then
    for pattern in $SCAN_DEVICES; do
        for dev in $pattern; do
            [ -b "$dev" ] 2>/dev/null || continue
            fstype=$(blkid -o value -s TYPE "$dev" 2>/dev/null)
            [ "$fstype" = "iso9660" ] && add_candidate "$dev"
        done
    done
fi

# Pass 4: the usual suspects, even if blkid could not identify them.
for dev in /dev/mapper/ventoy /dev/sr0 /dev/sr1 /dev/cdrom /dev/loop0; do
    add_candidate "$dev"
done

# Last resort: there is no iso9660 block device anywhere. That is the normal
# state under Ventoy, which keeps the ISO as a *file* on its exFAT data
# partition and relies on its own initramfs hook to expose it as a device --
# a hook that does not fire for a distro it has never seen. The tell is
# /dev/mapper holding nothing but `control` while every /dev/loop* sits
# unbacked.
#
# So do it ourselves: mount the data partitions, find the ISO, attach it to a
# loop device. The kernel carries exfat/vfat/ntfs3 built in for exactly this,
# and it makes the image bootable from a plain "copy the ISO onto a stick"
# setup too, not just Ventoy.
#
# RAVEN_MEDIA_MNT records the partition the ISO lives on. It must stay mounted
# for as long as the loop device is backed by a file inside it, so switch_root
# moves it into the new root rather than leaving it behind in the initramfs.
# Declared before the live/disk branch above, because switch_root reads it on
# both paths.

attach_iso_from_partitions() {
    command -v losetup &>/dev/null || return 1
    command -v find &>/dev/null || return 1

    for pattern in $SCAN_DEVICES; do
        for dev in $pattern; do
            [ -b "$dev" ] 2>/dev/null || continue

            fstype=$(blkid -o value -s TYPE "$dev" 2>/dev/null)
            case "$fstype" in
                exfat|vfat|ntfs|ntfs3|ext2|ext3|ext4) ;;
                *) continue ;;
            esac

            mount -o ro "$dev" /mnt/work 2>/dev/null || continue
            info "  searching $dev ($fstype) for ISO images"

            # Ours first -- a Ventoy stick usually carries several ISOs, and
            # attaching each in turn is the slow way round.
            for iso in $(find /mnt/work -maxdepth 4 -iname '*raven*.iso' 2>/dev/null) \
                       $(find /mnt/work -maxdepth 4 -iname '*.iso' 2>/dev/null); do
                loopdev=$(losetup -f 2>/dev/null) || break
                losetup -r "$loopdev" "$iso" 2>/dev/null || continue

                if try_boot_device "$loopdev"; then
                    ok "Attached $(basename "$iso") from $dev"
                    BOOT_DEVICE="$loopdev"
                    RAVEN_MEDIA_MNT="/mnt/work"
                    return 0
                fi

                losetup -d "$loopdev" 2>/dev/null || true
            done

            umount /mnt/work 2>/dev/null || true
        done
    done

    return 1
}

step "Mounting boot filesystems"

BOOT_DEVICE=""
for dev in $BOOT_CANDIDATES; do
    if try_boot_device "$dev"; then
        BOOT_DEVICE="$dev"
        break
    fi
done

[ -z "$BOOT_DEVICE" ] && attach_iso_from_partitions

if [ -z "$BOOT_DEVICE" ]; then
    fail "No bootable device found"
    info "Tried:$BOOT_CANDIDATES"
    info "Available block devices:"
    ls -la $SCAN_DEVICES 2>/dev/null || true
    info "blkid:"
    blkid 2>/dev/null || true
    rescue_shell "Boot device not found"
fi

ok "Found boot device: $BOOT_DEVICE"
ok "Found squashfs: $SQUASHFS"

if mount -t squashfs -o ro,loop "$SQUASHFS" /mnt/squashfs 2>/dev/null; then
    ok "Mounted squashfs filesystem"
else
    fail "Failed to mount squashfs"
    rescue_shell "Cannot mount squashfs image"
fi

# -----------------------------------------------------------------------------
# Setup Overlay Filesystem
# -----------------------------------------------------------------------------
step "Setting up overlay filesystem"

if mount -t tmpfs tmpfs /mnt/overlay 2>/dev/null; then
    ok "Created tmpfs for overlay"
else
    fail "Failed to create overlay tmpfs"
    rescue_shell "Cannot create overlay"
fi

mkdir -p /mnt/overlay/upper /mnt/overlay/work

if mount -t overlay overlay -o lowerdir=/mnt/squashfs,upperdir=/mnt/overlay/upper,workdir=/mnt/overlay/work /mnt/root 2>/dev/null; then
    ok "Mounted overlay filesystem (read-write)"
else
    warn "Overlay mount failed, falling back to read-only"
    if mount --bind /mnt/squashfs /mnt/root 2>/dev/null; then
        ok "Mounted root filesystem (read-only fallback)"
    else
        fail "Failed to mount root filesystem"
        rescue_shell "Cannot mount root"
    fi
fi

fi

# -----------------------------------------------------------------------------
# Prepare Switch Root
# -----------------------------------------------------------------------------
step "Preparing to switch root"

mkdir -p /mnt/root/proc /mnt/root/sys /mnt/root/dev
[ "$RAVEN_ROOT_MODE" = "live" ] && mkdir -p /mnt/root/mnt/cdrom
ok "Created mount points in new root"

if mount --move /proc /mnt/root/proc 2>/dev/null; then
    ok "Moved /proc to new root"
else
    fail "Failed to move /proc"
    rescue_shell "Cannot prepare new root"
fi

if mount --move /sys /mnt/root/sys 2>/dev/null; then
    ok "Moved /sys to new root"
else
    fail "Failed to move /sys"
    rescue_shell "Cannot prepare new root"
fi

if mount --move /dev /mnt/root/dev 2>/dev/null; then
    ok "Moved /dev to new root"
else
    fail "Failed to move /dev"
    rescue_shell "Cannot prepare new root"
fi

if [ "$RAVEN_ROOT_MODE" = "live" ]; then
    if mount --bind /mnt/cdrom /mnt/root/mnt/cdrom 2>/dev/null; then
        ok "Bind mounted /mnt/cdrom"
    else
        warn "Could not bind mount /mnt/cdrom (non-critical)"
    fi
fi

# When the ISO is a file on a data partition, the loop device backing / is
# only as alive as that partition's mount. Leaving it in the initramfs would
# tear it down at switch_root and take the root filesystem with it, so move it
# across. Not a bind: the initramfs copy has to stop existing.
if [ -n "$RAVEN_MEDIA_MNT" ]; then
    mkdir -p /mnt/root/run/raven-media
    if mount --move "$RAVEN_MEDIA_MNT" /mnt/root/run/raven-media 2>/dev/null; then
        ok "Moved boot media to /run/raven-media"
    else
        fail "Could not move boot media into the new root"
        rescue_shell "Boot media would be unmounted at switch_root"
    fi
fi

# Find init in the new root.
#
# The order differs by mode on purpose. On the live image /init is the boot
# script stage4 generates and is the right answer. On a disk it is not: an
# installed root wants raven-init, which raven-install leaves behind
# /sbin/init. Searching /init first there would run the live script against a
# real root -- it would mostly work, and quietly start no services at all.
NEW_INIT=""
if [ -n "$RAVEN_INIT_OVERRIDE" ]; then
    if [ -x "/mnt/root$RAVEN_INIT_OVERRIDE" ]; then
        NEW_INIT="$RAVEN_INIT_OVERRIDE"
        ok "Using init= from the command line: $NEW_INIT"
    else
        warn "init=$RAVEN_INIT_OVERRIDE is not executable in the new root; ignoring it"
    fi
fi

if [ -z "$NEW_INIT" ]; then
    if [ "$RAVEN_ROOT_MODE" = "disk" ]; then
        INIT_SEARCH="/sbin/init /bin/init /init"
    else
        INIT_SEARCH="/init /sbin/init /bin/init"
    fi

    for init in $INIT_SEARCH; do
        if [ -x "/mnt/root$init" ]; then
            NEW_INIT="$init"
            break
        fi
    done
fi

if [ -z "$NEW_INIT" ]; then
    NEW_INIT="/bin/bash"
    warn "No init found, falling back to $NEW_INIT"
else
    ok "Found init: $NEW_INIT"
fi

# -----------------------------------------------------------------------------
# Switch Root
# -----------------------------------------------------------------------------
step "Switching to live filesystem"

if ! command -v switch_root >/dev/null; then
    fail "switch_root command not found"
    rescue_shell "Missing switch_root"
fi

if [ ! -x "/mnt/root$NEW_INIT" ]; then
    fail "Init not executable: /mnt/root$NEW_INIT"
    rescue_shell "Invalid init"
fi

ok "Ready to switch root"
info "Executing switch_root to $NEW_INIT"

# switch_root may print "failed to unlink" warnings. They are harmless -- it is
# trying to clean up an initramfs that still has mounts -- and they used to be
# silenced with `2>/dev/null` on this line.
#
# That was a mistake with a long reach. The redirection is applied before exec
# and the exec keeps it, so it was not switch_root's stderr being discarded: it
# was *PID 1's*, permanently, for the rest of the boot. Consequences:
#
#   * init=/bin/bash gave a shell that looked hung. bash decides it is
#     interactive from isatty(0) && isatty(2); with stderr on /dev/null the
#     second test fails, so it runs non-interactively -- no prompt, no readline,
#     no job control. It is reading your keystrokes, it just never says so.
#   * every error PID 1 or its children wrote to stderr vanished, which is why
#     so much of this system failed silently.
#
# A few unlink warnings are a small price for a console that talks back.
exec switch_root /mnt/root "$NEW_INIT"

# If we get here, switch_root failed completely (exec didn't replace us)
fail "switch_root failed"
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
    # Check required tools before anything else
    check_dependencies

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
