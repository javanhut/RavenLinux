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
    
    for cmd in cpio zstd find; do
        if ! command -v "$cmd" &>/dev/null; then
            missing+=("$cmd")
        fi
    done
    
    if [[ ${#missing[@]} -gt 0 ]]; then
        echo "ERROR: Missing required tools: ${missing[*]}"
        echo ""
        echo "On Arch Linux, install with:"
        echo "  sudo pacman -S cpio zstd findutils"
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

# The path a library has on the booted system, given the path ldd resolved
# on the build host. Anything under a sysroot -- this build's, or one the
# linker found by RPATH -- is reported relative to that sysroot; everything
# else is already a runtime path.
runtime_lib_path() {
    local lib="$1"
    case "$lib" in
        "${RAVEN_BUILD}/sysroot"/*) printf '%s\n' "${lib#"${RAVEN_BUILD}/sysroot"}" ;;
        */sysroot/*) printf '/%s\n' "${lib#*/sysroot/}" ;;
        *) printf '%s\n' "$lib" ;;
    esac
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
            # Where the library lives at run time. A binary linked against the
            # sysroot resolves to /raven/build/sysroot/usr/lib/libc.so.6, and
            # copying that verbatim put the build tree's layout into the boot
            # image: a libc under raven/build/... that nothing could ever load.
            local rt
            rt="$(runtime_lib_path "$lib")"
            local dest="${INITRAMFS_DIR}${rt}"
            # Check if we should copy from sysroot instead of host path
            if [[ -f "${RAVEN_BUILD}/sysroot${rt}" ]]; then
                 # Prefer sysroot lib if we have a matching path
                 if [[ ! -f "$dest" ]]; then
                    mkdir -p "$(dirname "$dest")"
                    cp -L "${RAVEN_BUILD}/sysroot${rt}" "$dest" 2>/dev/null || true
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

# Firmware for drivers that are built into the kernel rather than modules.
#
# cfg80211 is =y, and it asks for regulatory.db from a late_initcall -- before
# /init has run, when the only filesystem is this initramfs. The copy that
# stage2 puts in the squashfs is not mounted yet, so it is never seen, and
# dmesg says "cfg80211: failed to load regulatory.db" on every boot. cfg80211
# then falls back to the built-in world domain: the card still associates,
# but with fewer channels and lower transmit power. With
# CFG80211_REQUIRE_SIGNED_REGDB the .p7s signature has to travel with it.
#
# The kernel searches /lib/firmware, not /usr/lib/firmware, and this tree is
# not usr-merged, so the files go under /lib explicitly.
copy_builtin_firmware() {
    log_step "Copying firmware for built-in drivers..."

    local sysroot="${RAVEN_BUILD}/sysroot"
    local src=""
    for candidate in "${sysroot}/usr/lib/firmware" /usr/lib/firmware /lib/firmware; do
        if [[ -f "${candidate}/regulatory.db" ]]; then
            src="$candidate"
            break
        fi
    done

    if [[ -z "$src" ]]; then
        log_warn "regulatory.db not found in the sysroot or on the build host;"
        log_warn "  cfg80211 will fall back to the world regulatory domain."
        log_warn "  Install wireless-regdb on the build host (the Dockerfile does) and rebuild."
        return 0
    fi

    mkdir -p "${INITRAMFS_DIR}/lib/firmware"
    local f copied=0
    for f in regulatory.db regulatory.db.p7s; do
        if [[ -f "${src}/${f}" ]]; then
            cp -L "${src}/${f}" "${INITRAMFS_DIR}/lib/firmware/${f}"
            copied=$((copied + 1))
        else
            log_warn "  ${src}/${f} missing; the kernel rejects an unsigned regulatory.db"
        fi
    done

    log_success "Regulatory database installed (${copied} files from ${src})"
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
# Boot Timing
# =============================================================================
# Everything that happens before switch_root is invisible to `raven-rc blame`.
# Its clock starts when raven-init starts, so on this machine blame opens with
# "init started 5.059" and has nothing whatsoever to say about the five seconds
# in front of that -- which is two thirds of the boot. The only evidence left
# was the shape of the holes in dmesg, and a hole tells you where the time went
# only if you already know what the initramfs was doing while it was quiet.
#
# So the initramfs times itself. Each milestone records the kernel's own clock
# from /proc/uptime -- CLOCK_MONOTONIC, the same clock dmesg stamps its lines
# with -- so a mark here and a kernel message there can be read against each
# other directly, with no guessing about offsets. raven_write_timings() hands
# the whole set to the new root at switch_root.
#
# The cost has to stay near zero or the instrument changes what it measures.
# `read` with a redirect is a bash builtin: one open/read/close of a procfs
# file the kernel generates on demand, no fork and no subshell, on the order of
# ten microseconds. Marks accumulate in a string rather than a file, so nothing
# here ever waits on I/O, and nothing is formatted unless somebody asked to see
# it. Seven marks across a three-second boot do not register.
#
# Recording is therefore unconditional, and that is the point: a measurement
# you have to enable in advance is a measurement you do not have for the boot
# that was actually slow. The `raven.timing` kernel argument only controls
# whether the marks are also *printed* -- each one as it is reached, and a
# summary before switch_root -- which is what you want when the machine is in
# front of you and the boot is the thing under investigation.
RAVEN_TIMING=0
RAVEN_TIMING_MARKS=""

# Where the marks are handed over. /dev is deliberate, and the obvious
# alternatives all fail:
#
#   * the initramfs itself stops existing at switch_root, so anywhere in it is
#     worthless;
#   * /run on the new root is where this kind of thing belongs, but raven-init
#     mounts a tmpfs over /run (init/src/main.rs:705) before any of its own
#     code could read a file there, which buries it;
#   * /var wants a writable root, and raven_mount_disk_root's second-chance
#     mount can legitimately leave the root read-only -- exactly the boot whose
#     timings you would most want.
#
# devtmpfs has none of those problems. It is *moved* into the new root rather
# than torn down, it is always writable even when the root is not, and it is a
# single kernel-wide instance, so raven-init mounting devtmpfs on /dev a second
# time (init/src/main.rs:685) shows the same file rather than covering it. A
# dotfile and a couple of hundred bytes, which is the same bargain dracut has
# had with /dev for years.
RAVEN_TIMING_FILE=.raven-initramfs-timings

# Record that boot reached $1, now.
raven_mark() {
    read -r raven_mark_now _ < /proc/uptime 2>/dev/null || return 0
    RAVEN_TIMING_MARKS="${RAVEN_TIMING_MARKS}${1} ${raven_mark_now}
"
    if [ "$RAVEN_TIMING" = "1" ]; then
        printf '  [ %8ss ] %s\n' "$raven_mark_now" "$1"
    fi
    return 0
}

# The same clock as an integer, in hundredths of a second, for the two device
# waits below.
#
# They need arithmetic, and bash has no decimals. Truncating to whole seconds
# was tried first and it quietly lies: the start of the wait gets truncated
# too, so `rootdelay=2` started at uptime 100.95 expired at 102.00 -- 1.05
# seconds, on a machine that asked for two because its disk needs two. The
# whole point of rootdelay is to be believed.
#
# The kernel prints this field as "%lu.%02lu" (fs/proc/uptime.c), so two digits
# after the point is a guarantee rather than an observation, but it is
# normalised anyway: the cost is a parameter expansion and the failure mode of
# getting it wrong is a root device given a tenth of the time it was promised.
# 10# because a fraction of "08" or "09" is not valid octal and $(( )) would
# refuse it.
raven_uptime_cs() {
    read -r raven_up _ < /proc/uptime 2>/dev/null || return 1
    case "$raven_up" in
        *.*) raven_up_frac="${raven_up#*.}00"; raven_up="${raven_up%%.*}" ;;
        *)   raven_up_frac="00" ;;
    esac
    raven_up_frac="${raven_up_frac:0:2}"
    RAVEN_UPTIME_CS=$(( 10#$raven_up * 100 + 10#$raven_up_frac ))
    return 0
}

# The timeline, with the gap in front of each step, printed only on request.
# One awk for the whole table rather than arithmetic per mark, because bash
# cannot do decimals and this is the one place where a fork is affordable: it
# happens after the last measurement, and only when somebody asked.
raven_print_timings() {
    [ "$RAVEN_TIMING" = "1" ] || return 0

    step "Initramfs timeline"
    if command -v awk >/dev/null 2>&1; then
        printf '%s' "$RAVEN_TIMING_MARKS" | awk '
            NR == 1 { first = $2; printf "  %-18s %9.3f\n", $1, $2 }
            NR >  1 { printf "  %-18s %9.3f  +%.3f\n", $1, $2, $2 - prev }
            { prev = $2 }
            END { printf "\n  %-18s %9.3f\n", "in the initramfs", prev - first }
        '
    else
        printf '%s' "$RAVEN_TIMING_MARKS"
    fi
    echo ""
    return 0
}

# Hand the marks to the new root. $1 is the new root's /dev, which by the time
# this is called is where this initramfs's own /dev has been moved to.
#
# Every failure here is silent on purpose. This is an instrument: a boot that
# cannot write its timings is still a boot, and refusing to switch_root over a
# diagnostic file would be the single worst trade in this script.
raven_write_timings() {
    raven_dev_dir="$1"
    [ -d "$raven_dev_dir" ] || return 0

    {
        echo "# RavenLinux initramfs milestones."
        echo "#"
        echo "# Seconds since the kernel started, read from /proc/uptime -- the same"
        echo "# clock dmesg stamps its lines with, so these line up with the kernel"
        echo "# log directly. Written by the initramfs /init immediately before"
        echo "# switch_root, so everything here happened before PID 1 existed."
        echo "#"
        echo "# Fields: <milestone> <seconds>"
        printf '%s' "$RAVEN_TIMING_MARKS"
    } > "${raven_dev_dir}/${RAVEN_TIMING_FILE}" 2>/dev/null || return 0

    return 0
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

# The command line is already open; read the timing knob out of it while we are
# here. A second pass rather than folding it into the test above, because that
# test answers one question with two greps and is load-bearing for what the
# banner says; this one is a shell-builtin loop over the same line and forks
# nothing.
#
#   raven.timing            print each milestone as it is reached, then a
#   raven.timing=1|on|yes   summary table before switch_root
#   raven.timing=0|off      the default: record silently, write the file
#
# Recording happens either way -- see the Boot Timing section above for why.
if [ -r /proc/cmdline ]; then
    read -r raven_cmdline < /proc/cmdline 2>/dev/null || raven_cmdline=""
    for arg in $raven_cmdline; do
        case "$arg" in
            raven.timing|raven.timing=1|raven.timing=on|raven.timing=yes)
                RAVEN_TIMING=1 ;;
            raven.timing=*)
                RAVEN_TIMING=0 ;;
        esac
    done
fi

# The first milestone, and the most informative one in the file: /init is
# running, which means the kernel has finished unpacking the initramfs and
# handed over. Everything before it -- firmware, the bootloader reading the
# image off the ESP, kernel self-init, cpio extraction and zstd decompression
# -- is the gap between 0 and this number, and the kernel's own
# "Run /init as init process" line is the same instant seen from the other
# side.
raven_mark initramfs

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

raven_mark vfs_mounted

# The wait for devices used to be here, as an unconditional `sleep 3`. It now
# lives in raven_wait_for_devices() and runs after the command line has been
# parsed, because what is worth waiting for depends on which root we are
# heading for. See that function for the measurements.

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
# The live image's volume label. Declared up here rather than beside the
# squashfs hunt that matches on it, because raven_wait_for_devices() waits for
# a device carrying exactly this label and runs first.
ISO_LABEL="RAVENLINUX"
# Declared out here because the switch_root preparation reads it on both paths.
RAVEN_MEDIA_MNT=""

# Is a udev daemon actually running?
#
# Nothing in this initramfs starts one. There is no udevd in the image and no
# rule set for it to read; udevadm is copied in (copy_binaries()) but it has
# nobody to talk to, so `udevadm trigger` writes "change" to a few thousand
# sysfs uevent files that no listener will ever read, and `udevadm settle`
# looks for a queue that does not exist and returns. Neither is a disaster --
# this is why the old flat `sleep 3` was doing all of the waiting -- but the
# trigger is real kernel work done for nothing, and a settle that quietly
# cannot settle anything is the kind of line people later mistake for a
# guarantee.
#
# So both are guarded on the daemon actually being there. They are kept rather
# than deleted because the day something does start udevd in here, coldplug and
# settle become the right thing to do again, and a guarded call is much easier
# to find than a deleted one. The control socket is udev's own liveness test --
# it is what udevadm itself checks before waiting on anything.
#
# None of this affects device nodes: /dev is devtmpfs, and the kernel creates
# the node when the device is registered, with no uevent round trip and no
# userspace involved. That is also why this initramfs can get away without
# udev at all.
raven_udev_active() {
    command -v udevadm >/dev/null 2>&1 || return 1
    [ -S /run/udev/control ] || return 1
    return 0
}

# The gap between polls, resolved once on first use.
#
# A flat `sleep 1` rounds every wait up to a whole second: a disk that shows up
# 2.05 seconds in costs very nearly three. A quarter second costs a fast
# machine nothing extra (it never reaches the sleep at all) and gets a slow one
# moving as soon as its hardware is ready.
#
# It has to be probed rather than assumed. `sleep` in this image is a symlink
# into the uutils multicall binary and takes fractions, but this file has
# outlived more than one set of assumptions about which binaries end up in it,
# and a `sleep 0.25` that errors out instead of sleeping would turn a
# thirty-second poll into a thirty-second spin on blkid. Asking costs 50ms, and
# only on a boot that polls at all.
RAVEN_POLL_INTERVAL=""
raven_poll_sleep() {
    if [ -z "$RAVEN_POLL_INTERVAL" ]; then
        if sleep 0.05 2>/dev/null; then
            RAVEN_POLL_INTERVAL=0.25
        else
            RAVEN_POLL_INTERVAL=1
        fi
    fi
    sleep "$RAVEN_POLL_INTERVAL" 2>/dev/null || sleep 1
}

# How long the live path will wait for its boot medium. Three seconds because
# that is exactly what the unconditional sleep this replaced cost every boot:
# the change is allowed to make the good case faster, it is not allowed to make
# the worst case slower.
RAVEN_MEDIA_WAIT=3

# Wait for the hardware we are about to ask for -- and only for as long as it
# takes.
#
# What used to be here was `sleep 3`, and it was the single largest cost in the
# boot. The evidence, from this machine's own dmesg, with a 12MB zstd -19
# initramfs:
#
#     [ 0.786] Trying to unpack rootfs image as initramfs...
#     [ 1.931] Run /init as init process
#     [ 1.933] clear (163) used greatest stack depth      <- the banner, this file
#     [ 2.883] hid-generic ... Realtek HID Device         <- last kernel message
#     [ 5.051] EXT4-fs (nvme0n1p3): mounted filesystem
#
# 3.118 seconds between /init starting and the root being mounted, of which
# three were this sleep; the "2.168s gap" that started the investigation is
# just the tail of it, after the kernel had finished enumerating USB in the
# background and gone quiet. Everything else on that path -- banner, five
# mounts, blkid, the ext4 mount itself -- adds up to about 120ms. Decompression
# is not in this window at all, and is not large: the kernel had unpacked the
# whole 38.7MB image before 1.931, and libzstd decompresses that same image in
# 0.08-0.13s on this CPU whether it was packed at level 3 or level 19.
#
# The sleep was not wrong to exist, it was wrong to be unconditional. The two
# paths need completely different things:
#
#   * With root= on the command line, the wait belongs in
#     raven_mount_disk_root(), which already polls for that exact device for up
#     to RAVEN_ROOT_WAIT seconds and gives up the instant it appears. Sleeping
#     here first meant every machine paid three seconds for a disk the kernel
#     had usually registered before /init even started -- nvme0n1p3 was there
#     at 1.4s on this one. So: do not wait at all, and let the poll that was
#     always going to run do the waiting on the boots that need it.
#
#   * Booting live, something does have to be waited for, because the squashfs
#     hunt further down is a single pass -- it asks each candidate once and
#     gives up -- and a USB stick that has not finished enumerating is not a
#     candidate yet. So keep a wait, but poll for the thing we actually need
#     instead of guessing at a duration.
#
# There are no kernel modules in this image to probe, incidentally, and no
# modprobe to probe them with: everything on the path to a root filesystem is
# built into the kernel (see configs/kernel/config-6.17-raven). That is the
# other reason this can be as short as it is.
raven_wait_for_devices() {
    step "Waiting for devices to settle"

    # If a udev is somehow running, it knows more about when enumeration is
    # finished than any poll here could, so ask it and take its answer.
    if raven_udev_active; then
        udevadm trigger 2>/dev/null || true
        udevadm settle --timeout=5 2>/dev/null || true
        ok "Device enumeration complete (udev)"
        return 0
    fi

    if [ "$RAVEN_ROOT_MODE" = "disk" ]; then
        ok "Root named on the command line; waiting for that device, not for a clock"
        return 0
    fi

    # No clock or no blkid means no way to tell when the medium has arrived, so
    # fall back to precisely the old behaviour rather than pressing on blind.
    if ! command -v blkid >/dev/null 2>&1 || ! raven_uptime_cs; then
        info "Cannot detect when the boot medium arrives; waiting ${RAVEN_MEDIA_WAIT}s"
        sleep "$RAVEN_MEDIA_WAIT"
        ok "Device enumeration complete"
        return 0
    fi

    raven_wait_start="$RAVEN_UPTIME_CS"
    while :; do
        # Our own label, not merely "an iso9660 somewhere". The difference
        # matters: a machine can easily have a decoy -- an optical drive with
        # some other disc in it, another loader's ISO on a second stick -- and
        # stopping the wait the instant one of those appears would let the
        # candidate scan below run before the real medium had enumerated, then
        # fail with "No bootable device found" on hardware where the old flat
        # sleep worked. Waiting for the label means an early exit only ever
        # happens because the thing we are about to mount is already there.
        #
        # A rebuilt ISO with a changed label never matches, so it spends the
        # full budget and is then found by pass 3 of the scan exactly as
        # before. Waiting too long is a slow boot; not waiting long enough is
        # no boot.
        if blkid -t "LABEL=$ISO_LABEL" -o device >/dev/null 2>&1; then
            ok "Boot medium is enumerated"
            return 0
        fi

        raven_uptime_cs || break
        [ $(( RAVEN_UPTIME_CS - raven_wait_start )) -lt $(( RAVEN_MEDIA_WAIT * 100 )) ] || break
        raven_poll_sleep
    done

    # Ventoy, and "copy the ISO onto a stick" generally, never produce a
    # labelled block device at all: the ISO is a *file* on an exFAT or vfat
    # partition and attach_iso_from_partitions() is what finds it. Those boots
    # spend the whole budget here -- the same three seconds they always spent,
    # and the data partition they need has had exactly as long to show up.
    info "No $ISO_LABEL device yet; the ISO may be a file on a data partition"
    ok "Device enumeration complete"
    return 0
}

raven_root_from_cmdline() {
    for arg in $(cat /proc/cmdline 2>/dev/null); do
        case "$arg" in
            root=*)       RAVEN_ROOT_SPEC="${arg#root=}" ;;
            rootfstype=*) RAVEN_ROOT_FSTYPE="${arg#rootfstype=}" ;;
            rootflags=*)  RAVEN_ROOT_FLAGS="${arg#rootflags=}" ;;
            rootwait)     RAVEN_ROOT_WAIT=60 ;;
            # Digits or nothing. The poll in raven_mount_disk_root turns this
            # into a deadline with arithmetic, and a rootdelay= somebody
            # fat-fingered at the boot prompt would otherwise produce a shell
            # arithmetic error on every pass of the loop and no waiting at all
            # -- which is the exact opposite of what was asked for, on the one
            # boot where somebody was asking.
            rootdelay=*)
                case "${arg#rootdelay=}" in
                    ''|*[!0-9]*) warn "Ignoring malformed $arg; keeping ${RAVEN_ROOT_WAIT}s" ;;
                    *)           RAVEN_ROOT_WAIT="${arg#rootdelay=}" ;;
                esac
                ;;
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

    # NVMe in particular regularly needs longer than the kernel's own probe,
    # and since raven_wait_for_devices() no longer sleeps before this point,
    # this loop is now the *only* thing waiting for the root device. It is the
    # right place for it: it waits for the one device we actually need and
    # stops the instant blkid can resolve it, so a disk the kernel registered
    # before /init started -- which is the normal case -- costs a single blkid
    # call and no sleep at all.
    #
    # The budget is wall clock rather than iterations. Each pass runs blkid,
    # which probes every block device the kernel knows about and is not free,
    # so counting one sleep per pass made rootdelay=30 mean "thirty sleeps plus
    # thirty blkid runs" -- comfortably more than thirty seconds on a machine
    # with a lot of disks, which is the machine most likely to need it. Reading
    # the deadline off /proc/uptime, the same clock the milestones use, makes
    # rootdelay= mean what it says.
    root_dev=""
    raven_uptime_cs || RAVEN_UPTIME_CS=0
    wait_start="$RAVEN_UPTIME_CS"
    announced=0
    while :; do
        root_dev=$(raven_resolve_root "$RAVEN_ROOT_SPEC") && break
        root_dev=""
        if [ "$announced" -eq 0 ]; then
            info "Waiting for $RAVEN_ROOT_SPEC to appear..."
            announced=1
        fi
        raven_uptime_cs || break
        [ $(( RAVEN_UPTIME_CS - wait_start )) -lt $(( RAVEN_ROOT_WAIT * 100 )) ] || break
        raven_poll_sleep
        raven_udev_active && udevadm settle --timeout=1 >/dev/null 2>&1
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
    raven_mark root_found

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
    raven_mark root_mounted

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
raven_mark cmdline_parsed

raven_wait_for_devices
raven_mark devices_ready

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
# Named root_found on this path too, rather than something live-specific, so
# that a timings file has the same milestones in the same order whichever way
# the machine booted and the two can be compared line for line.
raven_mark root_found

if mount -t squashfs -o ro,loop "$SQUASHFS" /mnt/squashfs 2>/dev/null; then
    ok "Mounted squashfs filesystem"
    raven_mark squashfs_mounted
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
    raven_mark root_mounted
else
    warn "Overlay mount failed, falling back to read-only"
    if mount --bind /mnt/squashfs /mnt/root 2>/dev/null; then
        ok "Mounted root filesystem (read-only fallback)"
        raven_mark root_mounted
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

# Last mark, then hand the timeline over. It goes into the *new root's* /dev,
# because /dev was moved there a few steps above and this initramfs's copy no
# longer exists; after the exec below it is readable as
# /dev/.raven-initramfs-timings, on a devtmpfs raven-init will mount over with
# the same instance rather than covering.
#
# Ordering: the mark has to be the last thing measured and the write has to be
# the last thing done, so the file records the whole of the initramfs and the
# only thing left unaccounted for is the exec itself.
raven_mark switch_root
raven_print_timings
raven_write_timings /mnt/root/dev

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

    # A build-host path inside the image is a bug, not a warning: a library
    # at raven/build/sysroot/usr/lib/... is one the loader never finds, and
    # the boot that follows fails somewhere far from here. Refuse to pack it.
    local leaked
    leaked="$(find . \( -path './raven' -o -path '*/sysroot' \) -prune -print 2>/dev/null | head -n 5)"
    if [[ -n "$leaked" ]]; then
        log_fatal "Build-tree paths inside the initramfs: ${leaked//$'\n'/ } -- see runtime_lib_path()"
    fi

    # Create uncompressed cpio archive first
    find . | cpio -o -H newc > "${RAVEN_BUILD}/initramfs.cpio" 2>/dev/null

    # zstd, not gzip. The kernel is built with RD_ZSTD, and zstd unpacks
    # several times faster than gzip on a small core -- which is what an
    # initramfs is measured by, since it is unpacked on every boot and
    # packed once. Level 19 buys the smaller image at build time; the
    # decompression speed is the same at any level.
    zstd -19 -T0 -q -f "${RAVEN_BUILD}/initramfs.cpio" -o "${OUTPUT}"
    rm -f "${RAVEN_BUILD}/initramfs.cpio"

    # Verify it worked
    local size
    size=$(du -h "${OUTPUT}" | cut -f1)

    if [[ $(stat -c%s "${OUTPUT}") -lt 1000 ]]; then
        log_fatal "Initramfs creation failed - file too small"
    fi

    log_success "Initramfs created: ${OUTPUT} (${size})"
}

# Early CPU microcode, prepended to the packed image.
#
# The kernel is built with CONFIG_MICROCODE=y and CONFIG_MICROCODE_LATE_LOADING
# off (configs/kernel/config-6.17-raven:432-433), so there is exactly one way a
# Raven machine ever runs anything newer than the revision its board firmware
# carries: the loader that runs out of the initrd before the rest of the kernel
# is up. Without it dmesg says
#
#   x86/CPU: Running old microcode
#
# on hardware whose vendor published the fix years ago, and every erratum in
# that gap -- the speculative-execution ones included -- stays unmitigated on a
# machine that did not have to be vulnerable. The blobs are about ten megabytes
# of initrd, they are already on the build host, and none of this is
# per-machine -- it is the cheapest security this image will ever buy.
#
# The awkward part is *where* they have to be. The microcode loader runs long
# before the initrd is decompressed, so it cannot read a compressed archive: it
# scans the raw initrd image for a cpio member named
# kernel/x86/microcode/<vendor>.bin. That rules out the obvious thing -- adding
# the blobs to the tree create_initramfs() packs, next to regulatory.db -- since
# that tree ends up inside the zstd frame where the loader cannot see it, and
# there is no second chance, because late loading is compiled out. They have to
# arrive as their own *uncompressed* cpio concatenated in front of the
# compressed image. The kernel's unpacker walks concatenated archives in order,
# so the real initramfs behind it is unpacked exactly as it was before.
#
# Prepending was chosen over the other route -- a second initrd entry in a
# bootloader config -- and it is not a close call here:
#
#   * RavenBoot, which is what every installed Raven machine actually boots
#     with, takes one initrd per entry: bootloader/src/config.rs:22 declares
#     `initrd: Option<String>`, boot.cfg's parser has a single `initrd` key, and
#     boot_efi_stub() (bootloader/src/linux.rs:226) reads that one file into one
#     contiguous buffer to hand over through LoadFile2. A second initrd is a
#     change to the struct, the parser and the loader.
#   * Even after all of that, machines already installed keep the boot.cfg
#     raven-install wrote for them. A bootloader-config route only reaches them
#     if something rewrites that file on an upgrade; the image route reaches
#     them the next time they take a new initramfs, with no per-machine
#     bootloader change at all. That is the whole reason to prefer it.
#   * The live ISO's BIOS GRUB menu (scripts/stages/stage4-iso.sh:setup_grub)
#     gets it for free as well, because every entry there already names
#     /boot/initramfs.img and that file now carries the microcode itself. No
#     `initrd ucode.img /boot/initramfs.img` line to keep in sync with anything.
#   * The ISO's ESP is sized from the initramfs file (stage4-iso.sh:829), so a
#     bigger single file is accounted for; a separate second file would not be.
#
# Both vendors go into the one archive. Arch splits them into intel-ucode and
# amd-ucode, but the kernel reads whichever member matches the CPUID vendor
# string and ignores the other, so a single image installs correctly on an Intel
# or an AMD machine. Roughly nine of those ten megabytes are Intel's and will be
# dead weight on an AMD box, which is the right price for not having to ask an
# ISO which CPU it is about to be installed on. The ESP the ISO builds sizes
# itself from the initramfs file, so the growth is already accounted for.

# Which vendors actually made it in, for print_summary(), and the exact member
# names verify_early_microcode() has to find. Empty means the image went out
# without microcode -- a warning, deliberately not a failed build, because an
# ISO built on a host that never installed the blobs is still a working ISO.
MICROCODE_VENDORS=""
MICROCODE_MEMBERS=()

# Stage one vendor's blob as kernel/x86/microcode/<bin_name> under $staging.
#
# Two source shapes, because build hosts disagree about who does the packing.
# Arch ships the finished cpio and nothing else -- intel-ucode owns
# /boot/intel-ucode.img, amd-ucode owns /boot/amd-ucode.img -- while Debian and
# Fedora ship the raw per-CPU blobs under /lib/firmware/{intel-ucode,amd-ucode}
# and leave the packing to the initramfs generator. Raw blobs win when both are
# present, because concatenating them is how the vendor images are built in the
# first place (Documentation/arch/x86/microcode.rst spells the Intel one out as
# `cat intel-ucode/* > kernel/x86/microcode/GenuineIntel.bin`, and Arch's
# amd-ucode is `cat microcode_amd*.bin`), and it keeps one code path producing
# the .bin. A finished image is unpacked into the same staging tree instead, so
# either way exactly one archive comes out of build_microcode_cpio() and there
# is one thing to verify rather than three.
#
# The sysroot is searched first for the same reason copy_builtin_firmware()
# searches it first: what the image ships should win over whatever happens to be
# installed on the machine doing the build.
stage_microcode_vendor() {
    local staging="$1" bin_name="$2" blob_subdir="$3" blob_glob="$4" image_name="$5"
    local sysroot="${RAVEN_BUILD}/sysroot"
    local dest="${staging}/kernel/x86/microcode/${bin_name}"
    local dir img f
    local blobs=()

    for dir in "${sysroot}/usr/lib/firmware/${blob_subdir}" \
               "/usr/lib/firmware/${blob_subdir}" \
               "/lib/firmware/${blob_subdir}"; do
        [[ -d "$dir" ]] || continue

        blobs=()
        # Unquoted on purpose: $blob_glob is a pattern, not a filename. The AMD
        # one has to be microcode_amd*.bin and not * -- the directory also holds
        # detached .asc signatures, and concatenating one of those into the blob
        # gives the loader a container header it cannot parse.
        # shellcheck disable=SC2231
        for f in "${dir}"/${blob_glob}; do
            [[ -f "$f" ]] && blobs+=("$f")
        done
        [[ ${#blobs[@]} -gt 0 ]] || continue

        cat "${blobs[@]}" > "$dest"
        log_info "  ${bin_name}: ${#blobs[@]} blobs from ${dir}"
        return 0
    done

    for img in "${sysroot}/boot/${image_name}" "/boot/${image_name}"; do
        [[ -f "$img" ]] || continue

        # The vendor image already stores the file at the path this wants it at,
        # so extracting into the staging root puts it exactly where it belongs.
        if ( cd "$staging" && cpio -i -d --quiet --no-absolute-filenames \
                 "kernel/x86/microcode/${bin_name}" < "$img" ) 2>/dev/null \
           && [[ -s "$dest" ]]; then
            log_info "  ${bin_name}: unpacked from ${img}"
            return 0
        fi

        log_warn "  ${img} carries no kernel/x86/microcode/${bin_name}"
    done

    return 1
}

# Pack whatever was staged into one uncompressed newc archive at $1.
#
# Returns 1, having written nothing, when the host has no microcode at all --
# the caller turns that into a warning naming the packages to install.
build_microcode_cpio() {
    local out="$1"
    local staging="${RAVEN_BUILD}/microcode"
    local members=(kernel kernel/x86 kernel/x86/microcode)
    local vendors=()

    rm -rf "$staging" "$out"
    mkdir -p "${staging}/kernel/x86/microcode"

    if stage_microcode_vendor "$staging" GenuineIntel.bin intel-ucode '*' intel-ucode.img; then
        members+=(kernel/x86/microcode/GenuineIntel.bin)
        vendors+=(Intel)
    fi
    if stage_microcode_vendor "$staging" AuthenticAMD.bin amd-ucode 'microcode_amd*.bin' amd-ucode.img; then
        members+=(kernel/x86/microcode/AuthenticAMD.bin)
        vendors+=(AMD)
    fi

    if [[ ${#vendors[@]} -eq 0 ]]; then
        rm -rf "$staging"
        return 1
    fi

    # The member list is fed in by hand rather than with `find .`, which is what
    # the kernel documentation uses, because the stored name has to be exactly
    # kernel/x86/microcode/GenuineIntel.bin: find_cpio_data() prefix-matches
    # that literal against the name field, and a name stored as ./kernel/...
    # never matches it. GNU cpio happens to strip the leading ./ for you and
    # bsdcpio does not, so `find .` would quietly produce a perfect-looking
    # archive that the loader never finds on a host with the wrong cpio. Naming
    # the members also fixes their order and keeps the "." entry out.
    #
    # -R 0:0 because the staging directory is owned by whoever ran the build and
    # an archive the kernel unpacks into its own rootfs should not be.
    ( cd "$staging" && printf '%s\n' "${members[@]}" \
        | cpio -o -H newc -R 0:0 --quiet > "$out" )

    rm -rf "$staging"

    MICROCODE_VENDORS="${vendors[*]}"
    # Without the three leading directory entries: the verifier checks for the
    # files, which are the only thing the early loader ever looks for.
    MICROCODE_MEMBERS=("${members[@]:3}")
    return 0
}

# What can be checked without booting.
#
# None of this proves a CPU took the update -- only a boot does that, and the
# proof is a dmesg line: "microcode: Current revision: 0x..." plus, on a machine
# that was behind, "microcode: updated early: 0x<old> -> 0x<new>", and the
# absence of the "x86/CPU: Running old microcode" complaint. But all three ways
# this goes wrong are silent at boot -- the machine comes up perfectly and
# simply never updates -- so the structural half is asserted here, where it is
# cheap, and log_fatal rather than a warning because a silently useless
# mitigation is worse than a build that stops and says why.
verify_early_microcode() {
    local img="$1" ucode_size="$2"
    local magic listing member tail_magic

    # If the concatenation order were reversed the image would start with the
    # zstd magic instead, the kernel would unpack the real initramfs and never
    # look past it, and the microcode would be dead weight at the end of a file
    # nothing reads.
    magic="$(od -An -N6 -c "$img" 2>/dev/null | tr -d ' \n')"
    if [[ "$magic" != "070701" ]]; then
        log_fatal "Microcode prepend failed: ${img} starts with $(od -An -N4 -tx1 "$img" | tr -d ' \n'), not an uncompressed newc cpio header (070701). Check that prepend_microcode ran after create_initramfs."
    fi

    # The kernel's unpacker expects the next archive on a 4-byte boundary. cpio
    # pads to its 512-byte block, so this only fires if something downstream
    # starts trimming the archive -- but when it fires at boot it is not silent,
    # it is "Kernel panic - not syncing: VFS: Unable to mount root fs".
    if (( ucode_size % 4 != 0 )); then
        log_fatal "Microcode archive is ${ucode_size} bytes, not a multiple of 4 -- the kernel would lose the initramfs concatenated behind it"
    fi

    listing="$(head -c "$ucode_size" "$img" | cpio -t --quiet 2>/dev/null)"
    for member in "${MICROCODE_MEMBERS[@]}"; do
        if ! grep -qxF "$member" <<< "$listing"; then
            log_fatal "Microcode archive does not contain ${member} -- the early loader looks for that exact path and nothing else"
        fi
    done

    # Checked against the archive's own bytes rather than the listing above,
    # because `cpio -t` prints kernel/x86/... whether or not the ./ is really
    # stored, which is exactly the failure this is looking for.
    if head -c "$ucode_size" "$img" | grep -aqF './kernel/x86/microcode'; then
        log_fatal "Microcode archive stores its members as ./kernel/... -- find_cpio_data() prefix-matches 'kernel/x86/microcode/<vendor>.bin' and will never find them"
    fi

    # Hard-coded to zstd because create_initramfs() is: if the compressor ever
    # changes this should fail loudly and be updated, rather than accept an
    # offset that happens to land on something the kernel cannot unpack.
    tail_magic="$(od -An -j "$ucode_size" -N4 -tx1 "$img" 2>/dev/null | tr -d ' \n')"
    if [[ "$tail_magic" != "28b52ffd" ]]; then
        log_fatal "The compressed initramfs does not begin at offset ${ucode_size} (found '${tail_magic}', expected the zstd magic 28b52ffd) -- the concatenation is wrong"
    fi
}

prepend_microcode() {
    log_step "Adding early CPU microcode..."

    local cpio_img="${RAVEN_BUILD}/microcode.cpio"

    if ! build_microcode_cpio "$cpio_img"; then
        log_warn "No CPU microcode found in the sysroot or on the build host."
        log_warn "  Machines will boot on whatever revision their board firmware"
        log_warn "  carries, and dmesg will say 'x86/CPU: Running old microcode'."
        log_warn "  Install intel-ucode and amd-ucode on the build host (the"
        log_warn "  Dockerfile does) and rebuild."
        return 0
    fi

    local ucode_size combined
    ucode_size="$(stat -c%s "$cpio_img")"
    combined="${OUTPUT}.ucode"

    cat "$cpio_img" "${OUTPUT}" > "$combined"
    mv -f "$combined" "${OUTPUT}"
    rm -f "$cpio_img"

    verify_early_microcode "${OUTPUT}" "$ucode_size"

    log_success "Early microcode prepended: ${MICROCODE_VENDORS// /, } ($(( ucode_size / 1024 )) KiB, uncompressed, ahead of the zstd image)"
}

print_summary() {
    log_section "RavenLinux Initramfs Built"

    echo "  Initramfs: ${OUTPUT}"
    if [[ -n "${MICROCODE_VENDORS}" ]]; then
        echo "  Early microcode: ${MICROCODE_VENDORS// /, }"
    fi
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
    copy_builtin_firmware
    create_device_nodes
    create_config_files
    create_init
    create_initramfs
    prepend_microcode
    print_summary

    finalize_logging 0
}

main "$@"
