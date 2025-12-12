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

    if [[ -f "${BUILD_DIR}/kernel/boot/vmlinuz-raven" ]]; then
        log_info "Kernel already built, skipping"
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

    if [[ -f "${BUILD_DIR}/initramfs-raven.img" ]]; then
        log_info "Initramfs already built, skipping"
        return 0
    fi

    if [[ -x "${PROJECT_ROOT}/scripts/build-initramfs.sh" ]]; then
        "${PROJECT_ROOT}/scripts/build-initramfs.sh" 2>&1 | tee "${LOGS_DIR}/initramfs.log"
    else
        log_warn "build-initramfs.sh not found, creating minimal initramfs"

        local initramfs_dir="${BUILD_DIR}/initramfs"
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

    mkdir -p "${SYSROOT_DIR}"/{bin,sbin,lib,lib64,usr/{bin,sbin,lib,lib64,include,share},etc,var,tmp,root,home,dev,proc,sys,run,mnt,opt,boot}
    mkdir -p "${SYSROOT_DIR}"/var/{log,cache,lib,tmp,run}
    mkdir -p "${SYSROOT_DIR}"/etc/{skel,xdg}

    # Install coreutils to sysroot
    if [[ -f "${BUILD_DIR}/bin/coreutils" ]]; then
        cp "${BUILD_DIR}/bin/coreutils" "${SYSROOT_DIR}/bin/"

        local utils=(
            cat cp mv rm ln mkdir rmdir touch chmod chown chgrp
            ls dir vdir head tail cut paste sort uniq wc tr tee
            echo printf yes df du stat sync id whoami groups
            uname hostname date sleep basename dirname realpath
            readlink pwd md5sum sha256sum test true false env
            seq dd install mktemp mknod tty
        )

        for util in "${utils[@]}"; do
            ln -sf coreutils "${SYSROOT_DIR}/bin/${util}"
        done
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

# Run if executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
