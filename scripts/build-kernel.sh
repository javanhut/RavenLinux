#!/bin/bash
# =============================================================================
# RavenLinux Kernel Build Script
# =============================================================================
# Downloads, configures, and compiles the Linux kernel for RavenLinux
#
# Usage: ./scripts/build-kernel.sh [options]
#   --config-only    Only generate config, don't compile
#   --menuconfig     Run menuconfig for manual configuration
#   --clean          Clean kernel build directory
#   --jobs N         Number of parallel jobs (default: nproc)
#   --log-dir DIR    Directory for build logs (default: build/logs)
#   --no-log         Disable logging to file

set -e

# Configuration
KERNEL_VERSION="6.17"
KERNEL_FULL_VERSION="6.17.11"
KERNEL_URL="https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-${KERNEL_FULL_VERSION}.tar.xz"
KERNEL_DIR="linux-${KERNEL_FULL_VERSION}"

# Directories
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="${PROJECT_ROOT}/build"
SOURCES_DIR="${BUILD_DIR}/sources"
KERNEL_BUILD_DIR="${SOURCES_DIR}/${KERNEL_DIR}"
OUTPUT_DIR="${BUILD_DIR}/kernel"
CONFIG_DIR="${PROJECT_ROOT}/configs/kernel"
LOG_DIR="${BUILD_DIR}/logs"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default options
CONFIG_ONLY=false
MENUCONFIG=false
CLEAN=false
JOBS=$(nproc)
ENABLE_LOGGING=true
LOG_FILE=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --config-only)
            CONFIG_ONLY=true
            shift
            ;;
        --menuconfig)
            MENUCONFIG=true
            shift
            ;;
        --clean)
            CLEAN=true
            shift
            ;;
        --jobs)
            JOBS="$2"
            shift 2
            ;;
        --log-dir)
            LOG_DIR="$2"
            shift 2
            ;;
        --no-log)
            ENABLE_LOGGING=false
            shift
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Setup logging
setup_logging() {
    if [ "$ENABLE_LOGGING" = true ]; then
        mkdir -p "$LOG_DIR"
        local timestamp=$(date +"%Y%m%d_%H%M%S")
        LOG_FILE="${LOG_DIR}/kernel-build_${timestamp}.log"

        # Start log file with header
        {
            echo "=========================================="
            echo "  RavenLinux Kernel Build Log"
            echo "  Started: $(date)"
            echo "  Kernel Version: ${KERNEL_VERSION}"
            echo "=========================================="
            echo ""
        } > "$LOG_FILE"

        log_info "Logging to: ${LOG_FILE}"
    fi
}

# Run command with output to both terminal and log file
run_logged() {
    if [ "$ENABLE_LOGGING" = true ] && [ -n "$LOG_FILE" ]; then
        # Use unbuffered output for real-time display
        "$@" 2>&1 | tee -a "$LOG_FILE"
        return ${PIPESTATUS[0]}
    else
        "$@"
    fi
}

# Log a message to both terminal and file
log_to_file() {
    local msg="$1"
    if [ "$ENABLE_LOGGING" = true ] && [ -n "$LOG_FILE" ]; then
        echo "$msg" | tee -a "$LOG_FILE"
    else
        echo "$msg"
    fi
}

# Check for required tools
check_dependencies() {
    log_info "Checking build dependencies..."

    local missing=()

    for cmd in gcc make bc flex bison perl; do
        if ! command -v "$cmd" &> /dev/null; then
            missing+=("$cmd")
        fi
    done

    # Check for headers
    if ! pkg-config --exists libelf 2>/dev/null; then
        if [ ! -f /usr/include/libelf.h ] && [ ! -f /usr/include/gelf.h ]; then
            missing+=("libelf-dev")
        fi
    fi

    if [ ! -f /usr/include/openssl/ssl.h ]; then
        missing+=("libssl-dev")
    fi

    if [ ${#missing[@]} -ne 0 ]; then
        log_error "Missing dependencies: ${missing[*]}"
        log_info "Install build-essential bc flex bison libelf-dev libssl-dev"
        exit 1
    fi

    log_success "All dependencies found"
}

# Download kernel source
download_kernel() {
    mkdir -p "$SOURCES_DIR"

    if [ -d "$KERNEL_BUILD_DIR" ]; then
        log_info "Kernel source already exists at $KERNEL_BUILD_DIR"
        return 0
    fi

    local tarball="${SOURCES_DIR}/linux-${KERNEL_FULL_VERSION}.tar.xz"

    if [ ! -f "$tarball" ]; then
        log_info "Downloading Linux kernel ${KERNEL_FULL_VERSION}..."
        if command -v wget &> /dev/null; then
            wget -q --show-progress -O "$tarball" "$KERNEL_URL"
        elif command -v curl &> /dev/null; then
            curl -L --progress-bar -o "$tarball" "$KERNEL_URL"
        else
            log_error "Neither wget nor curl found. Install one of them."
            exit 1
        fi
    fi

    log_info "Extracting kernel source..."
    tar xf "$tarball" -C "$SOURCES_DIR"

    log_success "Kernel source ready at $KERNEL_BUILD_DIR"
}

# Generate kernel config
generate_config() {
    log_info "Generating kernel configuration..."

    cd "$KERNEL_BUILD_DIR"

    # Start with defconfig
    make defconfig

    # Apply RavenLinux-specific options using scripts/config
    local config_script="./scripts/config"

    log_info "Applying RavenLinux kernel options..."

    # ==========================================================================
    # Core system options
    # ==========================================================================
    $config_script --set-str LOCALVERSION "-raven"
    $config_script --set-str DEFAULT_HOSTNAME "raven"

    # Disable WERROR - GCC 15+ has stricter warnings that cause build failures
    $config_script --disable WERROR

    # ==========================================================================
    # EFI/UEFI Boot Support (CRITICAL for our bootloader)
    # ==========================================================================
    $config_script --enable EFI
    $config_script --enable EFI_STUB
    $config_script --enable EFI_HANDOVER_PROTOCOL
    $config_script --enable EFI_MIXED
    $config_script --enable FB_EFI
    $config_script --enable FRAMEBUFFER_CONSOLE

    # ==========================================================================
    # Kernel compression (smaller initramfs)
    # ==========================================================================
    $config_script --enable KERNEL_ZSTD
    $config_script --enable RD_ZSTD
    $config_script --enable ZSTD_COMPRESS
    $config_script --enable ZSTD_DECOMPRESS

    # ==========================================================================
    # Filesystem support
    # ==========================================================================
    # Essential filesystems
    $config_script --enable EXT4_FS
    $config_script --enable BTRFS_FS
    $config_script --enable XFS_FS
    $config_script --enable VFAT_FS
    $config_script --enable FAT_FS
    $config_script --enable MSDOS_FS
    $config_script --enable EXFAT_FS
    $config_script --enable NTFS3_FS
    $config_script --enable FUSE_FS
    $config_script --enable OVERLAY_FS
    $config_script --enable SQUASHFS
    $config_script --enable SQUASHFS_ZSTD
    $config_script --enable ISO9660_FS
    $config_script --enable UDF_FS

    # Pseudo filesystems
    $config_script --enable TMPFS
    $config_script --enable DEVTMPFS
    $config_script --enable DEVTMPFS_MOUNT
    $config_script --enable PROC_FS
    $config_script --enable SYSFS
    $config_script --enable CGROUPS

    # ==========================================================================
    # Block device support
    # ==========================================================================
    $config_script --enable BLK_DEV_LOOP
    $config_script --enable BLK_DEV_RAM
    $config_script --enable BLK_DEV_NVME
    $config_script --enable BLK_DEV_SD
    $config_script --enable BLK_DEV_SR

    # Device mapper (LVM, encryption)
    $config_script --enable MD
    $config_script --enable BLK_DEV_DM
    $config_script --enable DM_CRYPT
    $config_script --enable DM_SNAPSHOT
    $config_script --enable DM_THIN_PROVISIONING

    # LUKS/encryption support
    $config_script --enable CRYPTO
    $config_script --enable CRYPTO_XTS
    $config_script --enable CRYPTO_AES
    $config_script --enable CRYPTO_SHA256
    $config_script --enable CRYPTO_SHA512
    $config_script --enable CRYPTO_ARGON2

    # ==========================================================================
    # Storage controllers (for real hardware boot)
    # ==========================================================================
    # AHCI/SATA
    $config_script --enable ATA
    $config_script --enable SATA_AHCI
    $config_script --enable ATA_PIIX

    # NVMe
    $config_script --enable NVME_CORE
    $config_script --enable BLK_DEV_NVME

    # USB storage
    $config_script --enable USB_STORAGE
    $config_script --enable USB_UAS

    # Virtio (for QEMU/KVM)
    $config_script --enable VIRTIO
    $config_script --enable VIRTIO_PCI
    $config_script --enable VIRTIO_BLK
    $config_script --enable VIRTIO_NET
    $config_script --enable VIRTIO_CONSOLE
    $config_script --enable VIRTIO_BALLOON
    $config_script --enable VIRTIO_INPUT
    $config_script --enable VIRTIO_GPU
    $config_script --enable DRM_VIRTIO_GPU

    # ==========================================================================
    # Input devices
    # ==========================================================================
    $config_script --enable INPUT_KEYBOARD
    $config_script --enable INPUT_MOUSE
    $config_script --enable INPUT_EVDEV
    $config_script --enable KEYBOARD_ATKBD
    $config_script --enable MOUSE_PS2
    $config_script --enable HID
    $config_script --enable HID_GENERIC
    $config_script --enable USB_HID

    # ==========================================================================
    # USB support
    # ==========================================================================
    $config_script --enable USB
    $config_script --enable USB_SUPPORT
    $config_script --enable USB_XHCI_HCD
    $config_script --enable USB_EHCI_HCD
    $config_script --enable USB_OHCI_HCD
    $config_script --enable USB_UHCI_HCD

    # ==========================================================================
    # Graphics (basic framebuffer + DRM)
    # ==========================================================================
    $config_script --enable DRM
    $config_script --enable DRM_FBDEV_EMULATION
    $config_script --enable FB
    $config_script --enable FB_SIMPLE
    $config_script --enable FB_VESA
    $config_script --enable VGA_CONSOLE
    $config_script --enable FRAMEBUFFER_CONSOLE
    $config_script --enable LOGO

    # Basic GPU drivers
    $config_script --enable DRM_I915        # Intel
    $config_script --enable DRM_AMDGPU      # AMD
    $config_script --enable DRM_NOUVEAU     # NVIDIA (open source)
    $config_script --enable DRM_SIMPLEDRM   # Simple framebuffer

    # EFI/System framebuffer drivers (CRITICAL for real hardware boot)
    $config_script --enable SYSFB_SIMPLEFB  # Simple framebuffer from firmware
    $config_script --enable DRM_EFIDRM      # EFI GOP framebuffer driver

    # VM/virtual GPU drivers (VirtualBox/VMware/QEMU)
    $config_script --enable DRM_VMWGFX      # VMware + VirtualBox VMSVGA
    $config_script --enable DRM_VBOXVIDEO   # VirtualBox VBoxVGA/VBoxSVGA
    $config_script --enable DRM_QXL         # QEMU/SPICE
    $config_script --enable DRM_BOCHS       # QEMU stdvga
    $config_script --enable DRM_CIRRUS_QEMU # Legacy QEMU cirrus
    $config_script --enable DRM_VESADRM     # VESA fallback (BIOS/CSM)

    # ==========================================================================
    # Network support
    # ==========================================================================
    $config_script --enable NET
    $config_script --enable INET
    $config_script --enable IPV6
    $config_script --enable NETDEVICES
    $config_script --enable ETHERNET
    $config_script --enable NET_VENDOR_INTEL
    $config_script --enable E1000
    $config_script --enable E1000E
    $config_script --enable IGB
    $config_script --enable NET_VENDOR_REALTEK
    $config_script --enable 8139CP
    $config_script --enable 8139TOO
    $config_script --enable R8169

    # Wireless (basic stack)
    $config_script --enable WLAN
    $config_script --enable CFG80211
    $config_script --enable MAC80211
    $config_script --enable RFKILL

    # ==========================================================================
    # WiFi Drivers (common hardware support)
    # ==========================================================================

    # Intel WiFi (most common on laptops - iwlwifi)
    $config_script --enable IWLWIFI
    $config_script --enable IWLMVM
    $config_script --enable IWLDVM

    # Intel legacy WiFi
    $config_script --enable IWL4965
    $config_script --enable IWL3945

    # Atheros WiFi (common on desktops and older laptops)
    $config_script --enable ATH9K
    $config_script --enable ATH9K_PCI
    $config_script --enable ATH9K_HTC
    $config_script --enable ATH10K
    $config_script --enable ATH10K_PCI
    $config_script --enable ATH10K_USB
    $config_script --enable ATH11K
    $config_script --enable ATH11K_PCI

    # Broadcom WiFi (common on MacBooks and some laptops)
    $config_script --enable BRCMFMAC
    $config_script --enable BRCMFMAC_PCIE
    $config_script --enable BRCMFMAC_USB
    $config_script --enable BRCMFMAC_SDIO
    $config_script --enable BRCMSMAC

    # Realtek WiFi (common USB dongles and some laptops)
    $config_script --enable RTL8187
    $config_script --enable RTL8192CU
    $config_script --enable RTL8XXXU
    $config_script --enable RTW88
    $config_script --enable RTW88_8822BE
    $config_script --enable RTW88_8822CE
    $config_script --enable RTW88_8723DE
    $config_script --enable RTW88_8821CE
    $config_script --enable RTW88_PCI
    $config_script --enable RTW88_USB
    $config_script --enable RTW89
    $config_script --enable RTW89_8852AE
    $config_script --enable RTW89_8852BE
    $config_script --enable RTW89_8852CE
    $config_script --enable RTW89_PCI

    # MediaTek WiFi (newer laptops and USB dongles)
    $config_script --enable MT7601U
    $config_script --enable MT76x0U
    $config_script --enable MT76x2U
    $config_script --enable MT7615E
    $config_script --enable MT7663U
    $config_script --enable MT7921E
    $config_script --enable MT7921U

    # Ralink (older USB WiFi dongles, now MediaTek)
    $config_script --enable RT2X00
    $config_script --enable RT2800USB
    $config_script --enable RT2800PCI

    # Marvell WiFi (some laptops and embedded)
    $config_script --enable MWIFIEX
    $config_script --enable MWIFIEX_PCIE
    $config_script --enable MWIFIEX_USB

    # Virtio network (QEMU)
    $config_script --enable VIRTIO_NET

    # ==========================================================================
    # Sound (basic ALSA)
    # ==========================================================================
    $config_script --enable SOUND
    $config_script --enable SND
    $config_script --enable SND_HDA_INTEL
    $config_script --enable SND_HDA_CODEC_HDMI
    $config_script --enable SND_HDA_CODEC_REALTEK
    $config_script --enable SND_USB_AUDIO

    # ==========================================================================
    # Kernel debugging/development (useful during development)
    # ==========================================================================
    $config_script --enable IKCONFIG
    $config_script --enable IKCONFIG_PROC
    $config_script --enable MAGIC_SYSRQ
    $config_script --enable DEBUG_FS
    $config_script --enable PRINTK
    $config_script --enable EARLY_PRINTK

    # ==========================================================================
    # Security
    # ==========================================================================
    $config_script --enable SECCOMP
    $config_script --enable SECURITY
    $config_script --enable SECURITY_SELINUX
    $config_script --disable SECURITY_SELINUX_BOOTPARAM_VALUE
    $config_script --enable SECURITY_APPARMOR
    $config_script --set-str DEFAULT_SECURITY ""

    # ==========================================================================
    # Containers/namespaces (for future container support)
    # ==========================================================================
    $config_script --enable NAMESPACES
    $config_script --enable USER_NS
    $config_script --enable PID_NS
    $config_script --enable NET_NS
    $config_script --enable CGROUP_PIDS
    $config_script --enable MEMCG
    $config_script --enable CGROUP_DEVICE

    # ==========================================================================
    # Performance
    # ==========================================================================
    $config_script --enable PREEMPT
    $config_script --enable NO_HZ_IDLE
    $config_script --enable HIGH_RES_TIMERS
    $config_script --enable SMP
    $config_script --set-val NR_CPUS 256

    # Update config with defaults for new options
    make olddefconfig

    log_success "Kernel configuration generated"

    # Save config for reference
    mkdir -p "$CONFIG_DIR"
    cp .config "${CONFIG_DIR}/config-${KERNEL_VERSION}-raven"
    log_info "Config saved to ${CONFIG_DIR}/config-${KERNEL_VERSION}-raven"
}

# Run menuconfig for manual tweaking
run_menuconfig() {
    log_info "Launching kernel menuconfig..."
    cd "$KERNEL_BUILD_DIR"
    make menuconfig

    # Save updated config
    mkdir -p "$CONFIG_DIR"
    cp .config "${CONFIG_DIR}/config-${KERNEL_VERSION}-raven"
    log_success "Config saved after menuconfig"
}

# Build kernel
build_kernel() {
    log_info "Building kernel with ${JOBS} parallel jobs..."
    log_info "This may take 10-30 minutes depending on your hardware..."
    if [ "$ENABLE_LOGGING" = true ]; then
        log_info "Build output is being logged to: ${LOG_FILE}"
    fi

    cd "$KERNEL_BUILD_DIR"

    log_to_file ""
    log_to_file "=========================================="
    log_to_file "  Building kernel image (bzImage)"
    log_to_file "=========================================="
    log_to_file ""

    # Build kernel image with real-time output and logging
    if ! run_logged make KCFLAGS="-std=gnu11" -j"$JOBS" bzImage; then
        log_error "Kernel image build failed!"
        if [ "$ENABLE_LOGGING" = true ]; then
            log_error "Check log file for details: ${LOG_FILE}"
        fi
        exit 1
    fi

    log_to_file ""
    log_to_file "=========================================="
    log_to_file "  Building kernel modules"
    log_to_file "=========================================="
    log_to_file ""

    # Build modules with real-time output and logging
    if ! run_logged make KCFLAGS="-std=gnu11" -j"$JOBS" modules; then
        log_error "Kernel modules build failed!"
        if [ "$ENABLE_LOGGING" = true ]; then
            log_error "Check log file for details: ${LOG_FILE}"
        fi
        exit 1
    fi

    log_success "Kernel build complete!"
}

# Install kernel to output directory
install_kernel() {
    log_info "Installing kernel to ${OUTPUT_DIR}..."

    mkdir -p "${OUTPUT_DIR}/boot"
    mkdir -p "${OUTPUT_DIR}/lib/modules"

    cd "$KERNEL_BUILD_DIR"

    # Copy kernel image
    cp arch/x86/boot/bzImage "${OUTPUT_DIR}/boot/vmlinuz-${KERNEL_VERSION}-raven"

    # Copy System.map
    cp System.map "${OUTPUT_DIR}/boot/System.map-${KERNEL_VERSION}-raven"

    # Copy kernel config
    cp .config "${OUTPUT_DIR}/boot/config-${KERNEL_VERSION}-raven"

    # Install modules
    make INSTALL_MOD_PATH="${OUTPUT_DIR}" modules_install

    # Create symlinks
    cd "${OUTPUT_DIR}/boot"
    ln -sf "vmlinuz-${KERNEL_VERSION}-raven" vmlinuz-raven
    ln -sf "System.map-${KERNEL_VERSION}-raven" System.map-raven

    log_success "Kernel installed to ${OUTPUT_DIR}"

    # Show results
    echo ""
    log_info "Kernel files:"
    ls -lh "${OUTPUT_DIR}/boot/"
    echo ""
    log_info "Modules installed to: ${OUTPUT_DIR}/lib/modules/"
    du -sh "${OUTPUT_DIR}/lib/modules/"*
}

# Clean build
clean_build() {
    log_info "Cleaning kernel build..."

    if [ -d "$KERNEL_BUILD_DIR" ]; then
        cd "$KERNEL_BUILD_DIR"
        make mrproper
    fi

    rm -rf "$OUTPUT_DIR"

    log_success "Clean complete"
}

# Main execution
main() {
    echo ""
    echo "=========================================="
    echo "  RavenLinux Kernel Build Script"
    echo "  Kernel Version: ${KERNEL_VERSION}"
    echo "=========================================="
    echo ""

    if [ "$CLEAN" = true ]; then
        clean_build
        exit 0
    fi

    # Setup logging before any major operations
    setup_logging

    check_dependencies
    download_kernel

    # Check if config exists, if not generate or restore it
    if [ ! -f "${KERNEL_BUILD_DIR}/.config" ]; then
        # If saved RavenLinux config exists, use it (preserves custom settings)
        if [ -f "${CONFIG_DIR}/config-${KERNEL_VERSION}-raven" ]; then
            log_info "Restoring saved RavenLinux kernel config..."
            cp "${CONFIG_DIR}/config-${KERNEL_VERSION}-raven" "${KERNEL_BUILD_DIR}/.config"
            cd "$KERNEL_BUILD_DIR"
            make olddefconfig
            log_success "Kernel config restored from ${CONFIG_DIR}/config-${KERNEL_VERSION}-raven"
        else
            # No saved config exists, generate new one from scratch
            generate_config
        fi
    fi

    if [ "$MENUCONFIG" = true ]; then
        run_menuconfig
    fi

    if [ "$CONFIG_ONLY" = true ]; then
        log_success "Config generated. Exiting without building."
        exit 0
    fi

    build_kernel
    install_kernel

    echo ""
    echo "=========================================="
    echo "  Build Complete!"
    echo "=========================================="
    echo ""
    echo "Kernel: ${OUTPUT_DIR}/boot/vmlinuz-raven"
    echo "Modules: ${OUTPUT_DIR}/lib/modules/${KERNEL_VERSION}.0-raven/"
    if [ "$ENABLE_LOGGING" = true ] && [ -n "$LOG_FILE" ]; then
        echo "Build Log: ${LOG_FILE}"
    fi
    echo ""
    echo "Next steps:"
    echo "  1. Build initramfs: ./scripts/build-initramfs.sh"
    echo "  2. Test with QEMU: ./scripts/test-qemu.sh"
    echo ""

    # Finalize log file
    if [ "$ENABLE_LOGGING" = true ] && [ -n "$LOG_FILE" ]; then
        {
            echo ""
            echo "=========================================="
            echo "  Build Complete"
            echo "  Finished: $(date)"
            echo "=========================================="
        } >> "$LOG_FILE"
    fi
}

main "$@"
