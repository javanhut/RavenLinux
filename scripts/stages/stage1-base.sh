#!/bin/bash
# =============================================================================
# RavenLinux Stage 1: Build Base System
# =============================================================================
# Builds the base system components using the cross toolchain
# This includes musl libc, busybox/coreutils, and essential utilities

set -euo pipefail

# =============================================================================
# Environment Setup (with defaults for standalone execution)
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${RAVEN_ROOT:-$(dirname "$(dirname "$SCRIPT_DIR")")}"
BUILD_DIR="${RAVEN_BUILD:-${PROJECT_ROOT}/build}"
SOURCES_DIR="${SOURCES_DIR:-${BUILD_DIR}/sources}"
SYSROOT_DIR="${SYSROOT_DIR:-${BUILD_DIR}/sysroot}"
LOGS_DIR="${LOGS_DIR:-${BUILD_DIR}/logs}"
RAVEN_JOBS="${RAVEN_JOBS:-$(nproc)}"

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
    NC='\033[0m'
    log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
    log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
    log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
    log_error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }
fi

# =============================================================================
# Build uutils-coreutils (Rust implementation)
# =============================================================================
build_coreutils() {
    log_info "Building uutils-coreutils..."

    if [[ -f "${BUILD_DIR}/bin/coreutils" ]]; then
        log_info "Coreutils already built, skipping"
        return 0
    fi

    # Use existing build script
    if [[ -x "${PROJECT_ROOT}/scripts/build-uutils.sh" ]]; then
        "${PROJECT_ROOT}/scripts/build-uutils.sh" 2>&1 | tee "${LOGS_DIR}/coreutils.log"
    else
        log_warn "build-uutils.sh not found, attempting direct build"

        local uutils_dir="${BUILD_DIR}/uutils-coreutils"
        if [[ ! -d "${uutils_dir}" ]]; then
            git clone --depth 1 https://github.com/uutils/coreutils.git "${uutils_dir}"
        fi

        cd "${uutils_dir}"
        cargo build --release --features unix 2>&1 | tee "${LOGS_DIR}/coreutils.log"

        mkdir -p "${BUILD_DIR}/bin"
        cp target/release/coreutils "${BUILD_DIR}/bin/"
    fi

    if [[ -f "${BUILD_DIR}/bin/coreutils" ]]; then
        log_success "Coreutils built successfully"
    else
        log_error "Failed to build coreutils"
    fi
}

# =============================================================================
# Build Linux Kernel
# =============================================================================
build_kernel() {
    log_info "Building Linux kernel..."

    local kernel_out="${BUILD_DIR}/kernel/boot/vmlinuz-raven"
    if [[ -f "${kernel_out}" ]]; then
        if find "${PROJECT_ROOT}/scripts/build-kernel.sh" "${PROJECT_ROOT}/configs/kernel" \
            -type f -newer "${kernel_out}" -print -quit 2>/dev/null | grep -q .; then
            log_warn "Kernel already built but older than build scripts/config; rebuild with: ./scripts/build-kernel.sh --clean"
        else
            log_info "Kernel already built, skipping"
        fi
        return 0
    fi

    if [[ -x "${PROJECT_ROOT}/scripts/build-kernel.sh" ]]; then
        "${PROJECT_ROOT}/scripts/build-kernel.sh" 2>&1 | tee "${LOGS_DIR}/kernel.log"
    else
        log_error "build-kernel.sh not found"
    fi

    if [[ -f "${BUILD_DIR}/kernel/boot/vmlinuz-raven" ]]; then
        log_success "Kernel built successfully"
    else
        log_error "Failed to build kernel"
    fi
}

# =============================================================================
# Build Initramfs
# =============================================================================
build_initramfs() {
    log_info "Building initramfs..."

    # Always rebuild initramfs to pick up any init script changes
    if [[ -x "${PROJECT_ROOT}/scripts/build-initramfs.sh" ]]; then
        # RAVEN_NO_DEVNODES=1 is the escape hatch for builds that cannot
        # mknod -- rootless podman, most obviously, where a user namespace
        # denies char-device creation whatever the capability set says.
        # Safe on this kernel: CONFIG_DEVTMPFS_MOUNT=y mounts devtmpfs on /dev
        # before init is exec'd, so the nodes exist by the time anything wants
        # them. Expect one "unable to open an initial console" line before that
        # mount happens.
        local -a initramfs_args=()
        if [[ -n "${RAVEN_NO_DEVNODES:-}" && "${RAVEN_NO_DEVNODES}" != "0" ]]; then
            log_warn "RAVEN_NO_DEVNODES set: initramfs will ship no static /dev nodes"
            initramfs_args+=(--no-devnodes)
        fi

        "${PROJECT_ROOT}/scripts/build-initramfs.sh" "${initramfs_args[@]}" 2>&1 | tee "${LOGS_DIR}/initramfs.log"
    else
        log_warn "build-initramfs.sh not found, creating minimal initramfs"

        local initramfs_dir="${BUILD_DIR}/initramfs"
        # DELIBERATELY split-usr, unlike the sysroot. The initramfs is a
        # separate root that early userspace switch_root's away from before rvn
        # ever runs, so no Arch package is extracted into it and the EEXIST
        # problem that forces the sysroot merge cannot arise here. Its links
        # are all same-directory (`ln -sf coreutils bin/X`), so it is not
        # exposed to the merge failure modes either. Do not "fix" this line.
        mkdir -p "${initramfs_dir}"/{bin,sbin,etc,proc,sys,dev,lib,lib64,usr/bin,usr/lib,tmp,run,mnt,root}

        # Copy essential binaries
        if [[ -f "${BUILD_DIR}/bin/coreutils" ]]; then
            cp "${BUILD_DIR}/bin/coreutils" "${initramfs_dir}/bin/"
            for cmd in sh ls cat cp mv rm mkdir mount umount; do
                ln -sf coreutils "${initramfs_dir}/bin/${cmd}"
            done
        fi

        # Create init script
        cat > "${initramfs_dir}/init" << 'EOF'
#!/bin/sh
export PATH=/bin:/sbin:/usr/bin:/usr/sbin

mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev 2>/dev/null || true

echo "RavenLinux Initramfs"

# Find and mount root
for arg in $(cat /proc/cmdline); do
    case $arg in
        root=*) ROOT="${arg#root=}" ;;
    esac
done

if [ -n "$ROOT" ]; then
    mount "$ROOT" /mnt
    exec switch_root /mnt /sbin/init
fi

exec /bin/sh
EOF
        chmod +x "${initramfs_dir}/init"

        # Create cpio archive
        cd "${initramfs_dir}"
        find . | cpio -o -H newc 2>/dev/null | gzip > "${BUILD_DIR}/initramfs-raven.img"
    fi

    if [[ -f "${BUILD_DIR}/initramfs-raven.img" ]]; then
        log_success "Initramfs built successfully"
    else
        log_error "Failed to build initramfs"
    fi
}

# =============================================================================
# Setup sysroot structure
# =============================================================================
setup_sysroot() {
    log_info "Setting up sysroot..."

    # -------------------------------------------------------------------------
    # usr-merged layout. /bin, /sbin, /lib and /lib64 are SYMLINKS into /usr;
    # /usr/sbin and /usr/lib64 are symlinks back into /usr/bin and /usr/lib.
    #
    # This is not cosmetic. Arch's `filesystem` package ships those names as
    # symlinks in its payload, so on a split-usr root extracting it tries to
    # create a symlink where a directory already exists and the kernel returns
    # EEXIST -- `rvn install openssh` died with "filesystem: File exists
    # (os error 17)". `filesystem` is a dependency of essentially every Arch
    # package, so split-usr makes the whole repo uninstallable.
    #
    # Consequence for everything below: install real files into /usr/bin and
    # /usr/lib, and never create a symlink whose link path and target collapse
    # to the same file after the merge. scripts/lib/usrmerge.sh spells out the
    # three shapes that do, all of which exit 0 while destroying the binary.
    # -------------------------------------------------------------------------
    if declare -F raven_usrmerge_root >/dev/null 2>&1; then
        # log_fatal, not log_error: log_error only echoes and returns 0, so a
        # failed merge would carry on and build the rest of the system onto a
        # half-merged sysroot -- /bin a real directory, /lib a symlink. That
        # ships an image that cannot resolve its own libraries. build.sh has
        # always used log_fatal here; the others had drifted.
        raven_usrmerge_root "${SYSROOT_DIR}" || log_fatal "usr-merge skeleton failed"
    else
        log_error "scripts/lib/usrmerge.sh is missing; cannot create a usr-merged sysroot"
        return 1
    fi
    # Presence, modes and accounts. Replaces the ad-hoc `mkdir -p {a,b,c}` lists
    # that used to live here: those created a dozen directories with whatever
    # umask happened to be set and no ownership at all, so /root came out 0755
    # and /var/spool/mail lost its sticky bit. skeleton.sh carries Arch's modes
    # and is idempotent, so it is also safe to call again later.
    if declare -F raven_skeleton_root >/dev/null 2>&1; then
        raven_skeleton_root "${SYSROOT_DIR}" || log_fatal "rootfs skeleton failed"
    else
        log_error "scripts/lib/skeleton.sh is missing; the sysroot will be incomplete"
    fi

    # Install coreutils to sysroot
    if [[ -f "${BUILD_DIR}/bin/coreutils" ]]; then
        cp "${BUILD_DIR}/bin/coreutils" "${SYSROOT_DIR}/usr/bin/"

        local utils=(
            cat cp mv rm ln mkdir rmdir touch chmod chown chgrp
            ls dir vdir head tail cut paste sort uniq wc tr tee
            echo printf yes df du stat sync id whoami groups
            uname hostname date sleep basename dirname realpath
            readlink pwd md5sum sha256sum test true false env
            seq dd install mktemp mknod tty
        )

        for util in "${utils[@]}"; do
            # Same-directory alias link: correct under the merge, and the
            # only link shape that is.
            ln -sf coreutils "${SYSROOT_DIR}/usr/bin/${util}"
        done
    fi

    # Install sudo-rs bits (su/visudo). We intentionally do not ship sudo by default.
    # Set RAVEN_ENABLE_SUDO=1 to include sudo in the sysroot.
    rm -f "${SYSROOT_DIR}/usr/bin/sudo" 2>/dev/null || true
    if [[ "${RAVEN_ENABLE_SUDO:-0}" == "1" ]] && [[ -f "${BUILD_DIR}/bin/sudo" ]]; then
        cp "${BUILD_DIR}/bin/sudo" "${SYSROOT_DIR}/usr/bin/sudo"
        chmod 4755 "${SYSROOT_DIR}/usr/bin/sudo" 2>/dev/null || chmod 755 "${SYSROOT_DIR}/usr/bin/sudo"
    fi
    if [[ -f "${BUILD_DIR}/bin/su" ]]; then
        cp "${BUILD_DIR}/bin/su" "${SYSROOT_DIR}/usr/bin/su"
        chmod 4755 "${SYSROOT_DIR}/usr/bin/su" 2>/dev/null || chmod 755 "${SYSROOT_DIR}/usr/bin/su"
    fi
    if [[ -f "${BUILD_DIR}/bin/visudo" ]]; then
        cp "${BUILD_DIR}/bin/visudo" "${SYSROOT_DIR}/usr/bin/visudo"
        chmod 755 "${SYSROOT_DIR}/usr/bin/visudo" 2>/dev/null || true
    fi

    log_success "Sysroot setup complete"
}

# =============================================================================
# Main
# =============================================================================
main() {
    echo ""
    echo "=========================================="
    echo "  Stage 1: Building Base System"
    echo "=========================================="
    echo ""

    mkdir -p "${LOGS_DIR}"

    build_coreutils
    build_kernel
    build_initramfs
    setup_sysroot

    echo ""
    log_success "Stage 1 complete!"
    echo ""
}

# Run main (whether executed directly or sourced)
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
else
    main "$@"
fi
