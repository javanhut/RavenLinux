#!/bin/bash
# =============================================================================
# RavenLinux Stage 4: Generate ISO Image
# =============================================================================
# Creates a bootable ISO image with:
# - RavenBoot UEFI bootloader (primary)
# - GRUB fallback for BIOS systems
# - Squashfs compressed root filesystem
# - Live boot support

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
LOGS_DIR="${LOGS_DIR:-${BUILD_DIR}/logs}"

# Version info
RAVEN_VERSION="${RAVEN_VERSION:-2025.12}"
RAVEN_ARCH="${RAVEN_ARCH:-x86_64}"
ISO_LABEL="RAVEN_LIVE"
ISO_OUTPUT="${PROJECT_ROOT}/raven-${RAVEN_VERSION}-${RAVEN_ARCH}.iso"

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
    CYAN='\033[0;36m'
    NC='\033[0m'
    log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
    log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
    log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
    log_error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }
    log_step() { echo -e "${CYAN}[STEP]${NC} $1"; }
fi

source "${PROJECT_ROOT}/scripts/lib/hyprland-config.sh"

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
# Create live init script in sysroot
# =============================================================================
create_live_init() {
    log_step "Creating live init system..."

    mkdir -p "${SYSROOT_DIR}"/{bin,sbin,etc}

    # Provide a default raven-init config in the ISO so tools can reference it.
    mkdir -p "${SYSROOT_DIR}/etc/raven"
    if [[ -f "${PROJECT_ROOT}/etc/raven/init.toml" ]]; then
        cp "${PROJECT_ROOT}/etc/raven/init.toml" "${SYSROOT_DIR}/etc/raven/init.toml" 2>/dev/null || true
    elif [[ -f "${PROJECT_ROOT}/init/config/init.toml" ]]; then
        cp "${PROJECT_ROOT}/init/config/init.toml" "${SYSROOT_DIR}/etc/raven/init.toml" 2>/dev/null || true
    fi
    if [[ ! -f "${SYSROOT_DIR}/etc/raven/init.toml" ]]; then
        cat > "${SYSROOT_DIR}/etc/raven/init.toml" << 'EOF'
# RavenLinux Init Configuration
# /etc/raven/init.toml

[system]
hostname = "raven-linux"
default_runlevel = "default"
shutdown_timeout = 10
load_modules = true
enable_udev = true
enable_network = true
log_level = "info"

[[services]]
name = "getty-tty1"
description = "Getty login on tty1"
exec = "/sbin/agetty"
args = ["--noclear", "--autologin", "root", "tty1", "linux"]
restart = true
enabled = true
critical = false

[[services]]
name = "getty-ttyS0"
description = "Serial console getty on ttyS0"
exec = "/sbin/agetty"
args = ["--noclear", "--autologin", "root", "-L", "115200", "ttyS0", "vt102"]
restart = true
enabled = false
critical = false

[[services]]
name = "dbus"
description = "D-Bus system message bus"
exec = "/usr/bin/dbus-daemon"
args = ["--system", "--nofork", "--nopidfile"]
restart = true
enabled = true
critical = false

[[services]]
name = "iwd"
description = "iNet Wireless Daemon"
exec = "/usr/libexec/iwd"
args = []
restart = true
enabled = true
critical = false
EOF
    fi
    chmod 0644 "${SYSROOT_DIR}/etc/raven/init.toml" 2>/dev/null || true

    # If earlier build stages weren't run, create minimal auth/NSS files so sudo/login work.
    local default_shell="/bin/sh"
    if [[ -x "${SYSROOT_DIR}/bin/bash" ]]; then
        default_shell="/bin/bash"
    fi

    if [[ ! -f "${SYSROOT_DIR}/etc/nsswitch.conf" ]]; then
        cat > "${SYSROOT_DIR}/etc/nsswitch.conf" << 'EOF'
passwd: files
group: files
shadow: files

hosts: files dns
networks: files

protocols: files
services: files
ethers: files
rpc: files
EOF
    fi

    if [[ ! -f "${SYSROOT_DIR}/etc/passwd" ]]; then
        cat > "${SYSROOT_DIR}/etc/passwd" << EOF
root:x:0:0:root:/root:${default_shell}
raven:x:1000:1000:Raven User:/home/raven:${default_shell}
nobody:x:65534:65534:Nobody:/:/bin/false
EOF
    fi

    if [[ ! -f "${SYSROOT_DIR}/etc/group" ]]; then
        cat > "${SYSROOT_DIR}/etc/group" << 'EOF'
root:x:0:
wheel:x:10:raven
audio:x:11:raven
video:x:12:raven
input:x:13:raven
users:x:100:raven
raven:x:1000:
nobody:x:65534:
EOF
    fi

    if [[ ! -f "${SYSROOT_DIR}/etc/shadow" ]]; then
        cat > "${SYSROOT_DIR}/etc/shadow" << 'EOF'
root::0:0:99999:7:::
raven::0:0:99999:7:::
nobody:!:0:0:99999:7:::
EOF
        chmod 600 "${SYSROOT_DIR}/etc/shadow" 2>/dev/null || true
    fi

    if [[ ! -f "${SYSROOT_DIR}/etc/pam.d/sudo" ]]; then
        mkdir -p "${SYSROOT_DIR}/etc/pam.d" "${SYSROOT_DIR}/etc/security" "${SYSROOT_DIR}/etc/security/limits.d"
        cat > "${SYSROOT_DIR}/etc/pam.d/sudo" << 'EOF'
#%PAM-1.0
auth       sufficient   pam_rootok.so
auth       required     pam_unix.so nullok try_first_pass
account    sufficient   pam_rootok.so
account    required     pam_unix.so
session    required     pam_unix.so
password   required     pam_unix.so nullok sha512
EOF
        cat > "${SYSROOT_DIR}/etc/pam.d/su" << 'EOF'
#%PAM-1.0
auth       sufficient   pam_rootok.so
auth       required     pam_unix.so nullok try_first_pass
account    sufficient   pam_rootok.so
account    required     pam_unix.so
session    required     pam_unix.so
password   required     pam_unix.so nullok sha512
EOF
        cat > "${SYSROOT_DIR}/etc/pam.d/login" << 'EOF'
#%PAM-1.0
auth       required     pam_unix.so nullok try_first_pass
account    required     pam_unix.so
session    required     pam_unix.so
password   required     pam_unix.so nullok sha512
EOF
        cat > "${SYSROOT_DIR}/etc/pam.d/passwd" << 'EOF'
#%PAM-1.0
password   required     pam_unix.so nullok sha512
EOF
        cat > "${SYSROOT_DIR}/etc/security/limits.conf" << 'EOF'
# /etc/security/limits.conf
# Minimal defaults (RavenLinux). Add custom limits in /etc/security/limits.d/.
EOF
    fi

    cat > "${SYSROOT_DIR}/init" << 'INIT'
#!/bin/bash
# RavenLinux Live Init

export PATH=/bin:/sbin:/usr/bin:/usr/sbin
export HOME=/root
export TERM=linux
export LANG=en_US.UTF-8
export PS1='[\u@raven-linux]# '

# Mount essential filesystems if not already mounted
mountpoint -q /proc || mount -t proc proc /proc
mountpoint -q /sys || mount -t sysfs sysfs /sys
mountpoint -q /dev || mount -t devtmpfs devtmpfs /dev 2>/dev/null || mount -t tmpfs tmpfs /dev
mkdir -p /dev/pts /dev/shm /tmp /run
mountpoint -q /dev/pts || mount -t devpts devpts /dev/pts
mountpoint -q /dev/shm || mount -t tmpfs tmpfs /dev/shm
mountpoint -q /tmp || mount -t tmpfs tmpfs /tmp
mountpoint -q /run || mount -t tmpfs tmpfs /run

# Fix common permission/ownership issues that break PAM/sudo in live images.
fix_auth_perms() {
    command -v chown >/dev/null 2>&1 || return 0
    command -v chmod >/dev/null 2>&1 || return 0

    # PAM rejects insecure shadow files (wrong owner/mode), and sudo requires setuid root.
    if [ -e /etc/shadow ]; then
        chown 0:0 /etc/shadow 2>/dev/null || true
        chmod 600 /etc/shadow 2>/dev/null || true
    fi

    for f in /etc/passwd /etc/group; do
        [ -e "$f" ] || continue
        chown 0:0 "$f" 2>/dev/null || true
        chmod 644 "$f" 2>/dev/null || true
    done

    for b in /bin/sudo /bin/su; do
        [ -e "$b" ] || continue
        chown 0:0 "$b" 2>/dev/null || true
        chmod 4755 "$b" 2>/dev/null || true
    done
}
fix_auth_perms || true

# Create /dev/log syslog socket (required by PAM/sudo for audit logging)
# Without this, sudo fails with "PAM error: Authentication service cannot retrieve authentication info"
start_syslog_socket() {
    [ -S /dev/log ] && return 0
    if command -v python3 >/dev/null 2>&1; then
        python3 -c '
import socket, os
try:
    os.unlink("/dev/log")
except: pass
s = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
s.bind("/dev/log")
os.chmod("/dev/log", 0o666)
while True:
    try: s.recv(4096)
    except: pass
' >/dev/null 2>&1 &
    fi
}
start_syslog_socket

# Set hostname (use /proc method as fallback if hostname binary is missing)
if command -v hostname >/dev/null 2>&1; then
    hostname raven-linux 2>/dev/null || true
else
    echo raven-linux > /proc/sys/kernel/hostname 2>/dev/null || true
fi

# Start udevd if available (helps libinput/Xorg enumerate devices)
if [ -x /sbin/udevd ]; then
    /sbin/udevd --daemon 2>/dev/null || true
elif [ -x /usr/lib/systemd/systemd-udevd ]; then
    /usr/lib/systemd/systemd-udevd --daemon 2>/dev/null || true
fi

if command -v udevadm >/dev/null 2>&1; then
    udevadm trigger --action=add 2>/dev/null || udevadm trigger 2>/dev/null || true
    udevadm settle 2>/dev/null || true
fi

# Start D-Bus system bus (needed by iwd/iwctl and many GUI apps)
if [ ! -S /run/dbus/system_bus_socket ] && command -v dbus-daemon >/dev/null 2>&1; then
    mkdir -p /run/dbus
    if command -v dbus-uuidgen >/dev/null 2>&1; then
        dbus-uuidgen --ensure=/etc/machine-id >/dev/null 2>&1 || true
    fi
    dbus-daemon --system --fork --nopidfile >/dev/null 2>&1 || true
fi

# Start iwd if available (WiFi daemon)
if ! pgrep -x iwd >/dev/null 2>&1; then
    if [ -x /usr/libexec/iwd ]; then
        /usr/libexec/iwd >/dev/null 2>&1 &
    elif command -v iwd >/dev/null 2>&1; then
        iwd >/dev/null 2>&1 &
    fi
fi

# Bring up wired networking automatically (WiFi still needs a connect step)
if command -v raven-dhcp >/dev/null 2>&1; then
    raven-dhcp --all -q >/dev/null 2>&1 || true
elif command -v dhcpcd >/dev/null 2>&1; then
    dhcpcd -q >/dev/null 2>&1 || true
elif command -v udhcpc >/dev/null 2>&1; then
    udhcpc -q -f >/dev/null 2>&1 || true
fi

# Try to load common GPU drivers (helps VMs where the driver is modular).
if command -v modprobe >/dev/null 2>&1; then
    modprobe -a virtio_gpu vmwgfx vboxvideo qxl bochs cirrus_qemu i915 amdgpu nouveau simpledrm 2>/dev/null || true
fi

# Suppress kernel messages
dmesg -n 1 2>/dev/null || true

# Clear screen and show banner
clear 2>/dev/null || printf '\033[2J\033[H'
printf '\033[1;36m'
cat << 'BANNER'

  ╔════════════════════════════════════════════════════════════════════════════════════════════╗
  ║                                                                                            ║
  ║    ██████╗  █████╗ ██╗   ██╗███████╗███╗   ██╗    ██╗     ██╗███╗   ██╗██╗   ██╗██╗  ██╗   ║
  ║    ██╔══██╗██╔══██╗██║   ██║██╔════╝████╗  ██║    ██║     ██║████╗  ██║██║   ██║╚██╗██╔╝   ║
  ║    ██████╔╝███████║██║   ██║█████╗  ██╔██╗ ██║    ██║     ██║██╔██╗ ██║██║   ██║ ╚███╔╝    ║
  ║    ██╔══██╗██╔══██║╚██╗ ██╔╝██╔══╝  ██║╚██╗██║    ██║     ██║██║╚██╗██║██║   ██║ ██╔██╗    ║
  ║    ██║  ██║██║  ██║ ╚████╔╝ ███████╗██║ ╚████║    ███████╗██║██║ ╚████║╚██████╔╝██╔╝ ██╗   ║
  ║    ╚═╝  ╚═╝╚═╝  ╚═╝  ╚═══╝  ╚══════╝╚═╝  ╚═══╝    ╚══════╝╚═╝╚═╝  ╚═══╝ ╚═════╝ ╚═╝  ╚═╝   ║
  ║                                                                                            ║
  ║                          A Developer-Focused Linux Distribution                            ║
  ║                                                                                            ║
  ╚════════════════════════════════════════════════════════════════════════════════════════════╝

BANNER
printf '\033[0m'
printf '\033[1;33m'
echo "                                       Version 2025.12"
printf '\033[0m'
echo ""
printf '\033[1;37m'
echo "  ┌────────────────────────────────────────────────────────────────────────────────────────┐"
echo "  │  BUILT-IN TOOLS:                                                                       │"
echo "  │    vem        - Text editor              wifi       - WiFi manager                     │"
echo "  │    carrion    - Programming language     rvn        - Package manager                  │"
echo "  │    ivaldi     - Version control          raven-install - System installer              │"
echo "  └────────────────────────────────────────────────────────────────────────────────────────┘"
printf '\033[0m'
echo ""
printf '\033[0;32m'
echo "  Type 'poweroff' to shutdown, 'reboot' to restart"
printf '\033[0m'
echo ""

cmdline="$(cat /proc/cmdline 2>/dev/null || true)"

start_shell_loop() {
    cd /root

    # Find first available TTY
    local tty_dev="/dev/tty1"
    if echo "$cmdline" | grep -qE '(^| )raven\.console=serial($| )'; then
        tty_dev="/dev/ttyS0"
    fi
    if [ ! -c "$tty_dev" ]; then
        tty_dev="/dev/console"
    fi

    # Switch to tty1 if openvt is available (only makes sense for real VTs)
    if [ "$tty_dev" = "/dev/tty1" ] && command -v openvt >/dev/null 2>&1; then
        while true; do
            if [ -x /bin/bash ]; then
                # Use -- to separate openvt options from command arguments
                openvt -c 1 -w -s -f -- /bin/bash --login || true
            elif [ -x /bin/sh ]; then
                openvt -c 1 -w -s -f -- /bin/sh -l || true
            else
                echo "No shell available! Sleeping..."
                sleep 10
            fi
            echo "Shell exited. Restarting..."
            sleep 1
        done
    else
        # Fallback: redirect to TTY device directly
        # Close inherited fds and reopen on the TTY
        if [ "$tty_dev" = "/dev/ttyS0" ] && command -v stty >/dev/null 2>&1; then
            stty -F "$tty_dev" 115200 cs8 -cstopb -parenb 2>/dev/null || true
        fi
        exec 0<>"$tty_dev" 1>&0 2>&0
        while true; do
            if [ "$tty_dev" = "/dev/ttyS0" ] && [ -x /sbin/agetty ]; then
                /sbin/agetty --noclear --autologin root -L 115200 ttyS0 vt102 || true
            elif [ -x /bin/bash ]; then
                /bin/bash --login -i
            elif [ -x /bin/sh ]; then
                /bin/sh -l
            else
                echo "No shell available! Sleeping..."
                sleep 10
            fi
            echo ""
            echo "Shell exited. Restarting..."
            sleep 1
        done
    fi
}

if echo "$cmdline" | grep -qE '(^| )raven\.graphics=wayland($| )'; then
    echo ""
    echo "Starting Wayland graphics..."

    if [ ! -d /dev/dri ]; then
        echo "No /dev/dri found; DRM/KMS not available. Skipping Wayland."
        echo "Hint: QEMU needs a KMS-capable GPU (e.g. -device virtio-vga-gl -display gtk,gl=on)."
        echo "Hint: VirtualBox should use 'VMSVGA' graphics controller."
        dmesg 2>/dev/null | grep -iE 'drm|kms|gpu|i915|amdgpu|nouveau|virtio|vmwgfx|vbox|qxl|bochs|cirrus|simpledrm|framebuffer' | tail -n 200 || true
    elif [ -x /bin/raven-wayland-session ]; then
        if command -v openvt >/dev/null 2>&1; then
            if openvt -c 1 -s -f -- /bin/raven-wayland-session; then
                :
            else
                echo "Wayland session exited; falling back to shell."
            fi
        elif /bin/raven-wayland-session; then
            :
        else
            echo "Wayland session exited; falling back to shell."
        fi
    else
        echo "raven-wayland-session not found; falling back to shell."
    fi
fi

if echo "$cmdline" | grep -qE '(^| )raven\.graphics=x11($| )'; then
    echo ""
    echo "Starting X11 graphics..."

    if [ ! -d /dev/dri ]; then
        echo "No /dev/dri found; DRM/KMS not available. Skipping X11."
        echo "Hint: QEMU needs a KMS-capable GPU (e.g. -device virtio-vga-gl -display gtk,gl=on)."
        echo "Hint: VirtualBox should use 'VMSVGA' graphics controller."
        dmesg 2>/dev/null | grep -iE 'drm|kms|gpu|i915|amdgpu|nouveau|virtio|vmwgfx|vbox|qxl|bochs|cirrus|simpledrm|framebuffer' | tail -n 200 || true
    elif [ -x /bin/raven-x11-session ]; then
        if command -v openvt >/dev/null 2>&1; then
            if openvt -c 1 -s -f -- /bin/raven-x11-session; then
                :
            else
                echo "X11 session exited; falling back to shell."
            fi
        elif /bin/raven-x11-session; then
            :
        else
            echo "X11 session exited; falling back to shell."
        fi
    else
        echo "raven-x11-session not found; falling back to shell."
    fi
fi

start_shell_loop
INIT
    chmod +x "${SYSROOT_DIR}/init"

    # Also create /sbin/init symlink
    mkdir -p "${SYSROOT_DIR}/sbin"
    ln -sf /init "${SYSROOT_DIR}/sbin/init" 2>/dev/null || true

    log_success "Live init created"
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

    log_success "Boot files copied"
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

    mkdir -p "${SYSROOT_DIR}/lib/modules"
    rm -rf "${SYSROOT_DIR}/lib/modules/${release}" 2>/dev/null || true
    cp -a "${modules_root}/${release}" "${SYSROOT_DIR}/lib/modules/" 2>/dev/null || true

    if [[ -d "${SYSROOT_DIR}/lib/modules/${release}" ]]; then
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
# Build and install Raven desktop components (shell, menu, desktop, settings)
# =============================================================================
build_desktop_components() {
    log_step "Building Raven desktop components..."

    if ! command -v go &>/dev/null; then
        log_warn "Go not found, skipping desktop component build"
        return 0
    fi

    local desktop_dir="${PROJECT_ROOT}/desktop"
    mkdir -p "${SYSROOT_DIR}/bin"
    mkdir -p "${SYSROOT_DIR}/root/.config/hypr"
    mkdir -p "${SYSROOT_DIR}/root/.config/raven/scripts"

    # Build raven-shell (panel/taskbar)
    if [[ -d "${desktop_dir}/raven-shell" ]]; then
        log_info "  Building raven-shell..."
        cd "${desktop_dir}/raven-shell"
        if CGO_ENABLED=1 go build -o raven-shell . 2>&1; then
            cp raven-shell "${SYSROOT_DIR}/bin/"
            chmod +x "${SYSROOT_DIR}/bin/raven-shell"
            log_info "  Installed raven-shell"
        else
            log_warn "  Failed to build raven-shell"
        fi
        cd "${PROJECT_ROOT}"
    fi

    # Build raven-desktop (background/icons)
    if [[ -d "${desktop_dir}/raven-desktop" ]]; then
        log_info "  Building raven-desktop..."
        cd "${desktop_dir}/raven-desktop"
        if CGO_ENABLED=1 go build -o raven-desktop . 2>&1; then
            cp raven-desktop "${SYSROOT_DIR}/bin/"
            chmod +x "${SYSROOT_DIR}/bin/raven-desktop"
            log_info "  Installed raven-desktop"
        else
            log_warn "  Failed to build raven-desktop"
        fi
        cd "${PROJECT_ROOT}"
    fi

    # Build raven-menu (application launcher)
    if [[ -d "${desktop_dir}/raven-menu" ]]; then
        log_info "  Building raven-menu..."
        cd "${desktop_dir}/raven-menu"
        if CGO_ENABLED=1 go build -o raven-menu . 2>&1; then
            cp raven-menu "${SYSROOT_DIR}/bin/"
            chmod +x "${SYSROOT_DIR}/bin/raven-menu"
            log_info "  Installed raven-menu"
        else
            log_warn "  Failed to build raven-menu"
        fi
        cd "${PROJECT_ROOT}"
    fi

    # Build raven-settings-menu (settings application)
    if [[ -d "${desktop_dir}/raven-settings-menu" ]]; then
        log_info "  Building raven-settings-menu..."
        cd "${desktop_dir}/raven-settings-menu"
        if CGO_ENABLED=1 go build -o raven-settings-menu . 2>&1; then
            cp raven-settings-menu "${SYSROOT_DIR}/bin/"
            chmod +x "${SYSROOT_DIR}/bin/raven-settings-menu"
            log_info "  Installed raven-settings-menu"
        else
            log_warn "  Failed to build raven-settings-menu"
        fi
        cd "${PROJECT_ROOT}"
    fi

    # Install Hyprland configuration
    install_hyprland_config \
        "${SYSROOT_DIR}/root/.config/hypr/hyprland.conf" \
        "${SYSROOT_DIR}/etc/hypr/hyprland.conf"
    log_info "  Installed Hyprland config"

    # Install Raven scripts
    if [[ -d "${desktop_dir}/config/raven/scripts" ]]; then
        cp "${desktop_dir}/config/raven/scripts"/*.sh "${SYSROOT_DIR}/root/.config/raven/scripts/" 2>/dev/null || true
        chmod +x "${SYSROOT_DIR}/root/.config/raven/scripts"/*.sh 2>/dev/null || true
        log_info "  Installed Raven scripts"
    fi

    # Install default Raven settings
    if [[ ! -f "${SYSROOT_DIR}/root/.config/raven/settings.json" ]]; then
        cat > "${SYSROOT_DIR}/root/.config/raven/settings.json" << 'EOF'
{
  "theme": "dark",
  "accent_color": "#009688",
  "font_size": 14,
  "icon_theme": "Papirus-Dark",
  "cursor_theme": "Adwaita",
  "panel_opacity": 0.95,
  "enable_animations": true,
  "wallpaper_path": "",
  "wallpaper_mode": "fill",
  "show_desktop_icons": false,
  "panel_position": "top",
  "panel_height": 38,
  "show_clock": true,
  "clock_format": "24h",
  "show_workspaces": true,
  "border_width": 2,
  "gap_size": 8,
  "focus_follows_mouse": false,
  "titlebar_buttons": "close,minimize,maximize",
  "keyboard_layout": "us",
  "mouse_speed": 0.5,
  "touchpad_natural_scroll": true,
  "touchpad_tap_to_click": true,
  "screen_timeout": 300,
  "suspend_timeout": 900,
  "lid_close_action": "suspend",
  "master_volume": 80,
  "mute_on_lock": false
}
EOF
        log_info "  Created default Raven settings"
    fi

    log_success "Raven desktop components built and installed"
}

# =============================================================================
# Copy Hyprland compositor and dependencies from host
# =============================================================================
copy_wayland_compositor() {
    log_step "Copying Wayland compositor from host..."

    # Copy Hyprland if available
    if command -v Hyprland &>/dev/null; then
        local hyprland_bin
        hyprland_bin="$(which Hyprland)"
        cp "$hyprland_bin" "${SYSROOT_DIR}/bin/"
        chmod +x "${SYSROOT_DIR}/bin/Hyprland"
        log_info "  Copied Hyprland"

        # Copy Hyprland libraries
        ldd "$hyprland_bin" 2>/dev/null | grep -o '/[^ ]*' | while read -r lib; do
            [[ -z "$lib" || ! -f "$lib" ]] && continue
            dest="${SYSROOT_DIR}${lib}"
            if [[ ! -f "$dest" ]]; then
                mkdir -p "$(dirname "$dest")"
                cp -L "$lib" "$dest" 2>/dev/null || true
            fi
        done || true

        # Ensure Hyprland config exists (build-time template)
        if [[ ! -f "${SYSROOT_DIR}/etc/hypr/hyprland.conf" ]]; then
            install_hyprland_config \
                "${SYSROOT_DIR}/etc/hypr/hyprland.conf" \
                "${SYSROOT_DIR}/root/.config/hypr/hyprland.conf"
            log_info "  Installed Hyprland config"
        fi

        # Copy hyprctl if available
        if command -v hyprctl &>/dev/null; then
            cp "$(which hyprctl)" "${SYSROOT_DIR}/bin/"
            log_info "  Copied hyprctl"
        fi

        # Copy Hyprland GUI utilities (if installed)
        for guiutil in hyprland-welcome hyprland-update-screen hyprland-donate-screen hyprland-dialog hyprland-run; do
            if command -v "${guiutil}" &>/dev/null; then
                cp "$(which "${guiutil}")" "${SYSROOT_DIR}/bin/" 2>/dev/null || true
                log_info "  Copied ${guiutil}"
            fi
        done

        # Copy Hyprland data files
        if [[ -d /usr/share/hyprland ]]; then
            mkdir -p "${SYSROOT_DIR}/usr/share/hyprland"
            cp -a /usr/share/hyprland/. "${SYSROOT_DIR}/usr/share/hyprland/" 2>/dev/null || true
            log_info "  Copied /usr/share/hyprland"
        fi

        # Copy wayland-protocols
        if [[ -d /usr/share/wayland-protocols ]]; then
            mkdir -p "${SYSROOT_DIR}/usr/share/wayland-protocols"
            cp -a /usr/share/wayland-protocols/. "${SYSROOT_DIR}/usr/share/wayland-protocols/" 2>/dev/null || true
            log_info "  Copied wayland-protocols"
        fi

        # Copy XKB keyboard layouts
        if [[ -d /usr/share/X11/xkb ]]; then
            mkdir -p "${SYSROOT_DIR}/usr/share/X11/xkb"
            cp -a /usr/share/X11/xkb/. "${SYSROOT_DIR}/usr/share/X11/xkb/" 2>/dev/null || true
            log_info "  Copied XKB layouts"
        fi

        log_success "Hyprland compositor copied"
    else
        log_warn "Hyprland not found on host - install with: sudo pacman -S hyprland"
    fi

    # Copy seatd if available
    if command -v seatd &>/dev/null; then
        cp "$(which seatd)" "${SYSROOT_DIR}/bin/" 2>/dev/null || true
        log_info "  Copied seatd"
    fi

    # Copy Xwayland if available
    if command -v Xwayland &>/dev/null; then
        local xwayland_bin
        xwayland_bin="$(which Xwayland)"
        if [[ ! -f "${SYSROOT_DIR}/bin/Xwayland" ]]; then
            cp "$xwayland_bin" "${SYSROOT_DIR}/bin/"
            log_info "  Copied Xwayland"
            # Copy Xwayland libraries
            ldd "$xwayland_bin" 2>/dev/null | grep -o '/[^ ]*' | while read -r lib; do
                [[ -z "$lib" || ! -f "$lib" ]] && continue
                dest="${SYSROOT_DIR}${lib}"
                if [[ ! -f "$dest" ]]; then
                    mkdir -p "$(dirname "$dest")"
                    cp -L "$lib" "$dest" 2>/dev/null || true
                fi
            done || true
        fi
    fi
}

# =============================================================================
# Install packages to sysroot
# =============================================================================
install_packages_to_sysroot() {
    log_step "Installing packages to sysroot..."

    mkdir -p "${SYSROOT_DIR}/bin"

    # Copy all built packages from packages/bin
    if [[ -d "${PACKAGES_DIR}/bin" ]]; then
        for pkg in "${PACKAGES_DIR}/bin"/*; do
            [[ -f "$pkg" ]] || continue
            local name
            name="$(basename "$pkg")"
            cp "$pkg" "${SYSROOT_DIR}/bin/"
            chmod +x "${SYSROOT_DIR}/bin/${name}"
            log_info "  Installed ${name}"
        done
    fi

    # Create raven-install symlink for the installer
    if [[ -f "${SYSROOT_DIR}/bin/raven-installer" ]]; then
        ln -sf raven-installer "${SYSROOT_DIR}/bin/raven-install"
        log_info "  Created raven-install symlink"
    fi

    # Copy desktop entries if present
    if [[ -d "${PROJECT_ROOT}/configs/desktop" ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/share/applications"
        cp "${PROJECT_ROOT}/configs/desktop"/*.desktop "${SYSROOT_DIR}/usr/share/applications/" 2>/dev/null || true
    fi

    # Copy session helper scripts
    if [[ -f "${PROJECT_ROOT}/configs/raven-wayland-session" ]]; then
        cp "${PROJECT_ROOT}/configs/raven-wayland-session" "${SYSROOT_DIR}/bin/raven-wayland-session" 2>/dev/null || true
        chmod +x "${SYSROOT_DIR}/bin/raven-wayland-session" 2>/dev/null || true
    fi
    if [[ -f "${PROJECT_ROOT}/configs/raven-x11-session" ]]; then
        cp "${PROJECT_ROOT}/configs/raven-x11-session" "${SYSROOT_DIR}/bin/raven-x11-session" 2>/dev/null || true
        chmod +x "${SYSROOT_DIR}/bin/raven-x11-session" 2>/dev/null || true
    fi
    if command -v swaybg &>/dev/null; then
        cp "$(which swaybg)" "${SYSROOT_DIR}/bin/" 2>/dev/null || true
        log_info "  Copied swaybg"
    else
        log_warn "  swaybg not found on host; wallpapers may not render"
    fi
    if [[ -d "${PROJECT_ROOT}/desktop/config/raven/backgrounds" ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/share/backgrounds"
        cp "${PROJECT_ROOT}/desktop/config/raven/backgrounds"/* "${SYSROOT_DIR}/usr/share/backgrounds/" 2>/dev/null || true
        log_info "  Copied Raven wallpapers"
    fi
    # Fontconfig + fonts (needed for terminal/shell; missing config causes warnings).
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

    # Cursor themes (needed for proper cursor display in Wayland compositors).
    if [[ -d "/usr/share/icons" ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/share/icons"
        for theme in default breeze_cursors Adwaita hicolor; do
            if [[ -d "/usr/share/icons/${theme}" ]]; then
                mkdir -p "${SYSROOT_DIR}/usr/share/icons/${theme}"
                cp -a "/usr/share/icons/${theme}/." "${SYSROOT_DIR}/usr/share/icons/${theme}/" 2>/dev/null || true
                log_info "  Copied /usr/share/icons/${theme}"
            fi
        done
    fi

    # Ensure shared library dependencies for newly installed binaries are present.
    log_info "Copying runtime libraries for sysroot binaries..."
    for bin in "${SYSROOT_DIR}"/bin/* "${SYSROOT_DIR}"/sbin/*; do
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
    find "${SYSROOT_DIR}/bin" "${SYSROOT_DIR}/sbin" "${SYSROOT_DIR}/usr/bin" "${SYSROOT_DIR}/usr/sbin" \
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

    # Add live init if not present
    [[ -f "${SYSROOT_DIR}/init" ]] || create_live_init

    # Install packages to sysroot before creating squashfs
    install_packages_to_sysroot

    # Clean up to reduce size
    cleanup_sysroot

    local pseudo="${LOGS_DIR}/squashfs.pseudo"
    : > "${pseudo}"
    [[ -e "${SYSROOT_DIR}/bin/sudo" ]] && echo "bin/sudo m 4755 0 0" >> "${pseudo}"
    [[ -e "${SYSROOT_DIR}/bin/su" ]] && echo "bin/su m 4755 0 0" >> "${pseudo}"
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

        # RavenBoot now has built-in submenu support with sensible defaults.
        # The bootloader will use its compiled-in menu structure which includes:
        # - Raven Linux (terminal)
        # - Raven Linux (Graphical) > submenu with compositor options
        # - Raven Linux (Recovery)
        # - System > submenu with UEFI Shell, Reboot, Shutdown
        #
        # No boot.cfg needed - the defaults are baked into the bootloader.
        # If you want to customize, create boot.cfg with flat entries (submenus not yet
        # supported in config file parsing).
        log_info "  Using built-in boot menu with submenu support"

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

# Default: Graphical mode with Raven Compositor
menuentry "Raven Linux" --class raven {
    linux /boot/vmlinuz rdinit=/init quiet loglevel=3 raven.graphics=wayland raven.wayland=raven console=tty0
    initrd /boot/initramfs.img
}

# Serial console mode (for VMs, headless, debugging)
menuentry "Raven Linux (Serial)" --class raven {
    linux /boot/vmlinuz rdinit=/init quiet loglevel=3 console=ttyS0,115200 console=tty0
    initrd /boot/initramfs.img
}

# Graphical options submenu
submenu "Raven Linux (Graphical) >" --class raven {
    menuentry "Raven Compositor" --class raven {
        linux /boot/vmlinuz rdinit=/init quiet loglevel=3 raven.graphics=wayland raven.wayland=raven console=tty0
        initrd /boot/initramfs.img
    }

    menuentry "< Back" --class raven {
        configfile /boot/grub/grub.cfg
    }
}

# System options submenu
submenu "System >" --class raven {
    menuentry "Recovery Mode" --class raven {
        linux /boot/vmlinuz rdinit=/init single console=ttyS0,115200 console=tty0
        initrd /boot/initramfs.img
    }

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

    # Create EFI bootloader if RavenBoot wasn't available
    if [[ ! -f "${ISO_ROOT}/EFI/BOOT/BOOTX64.EFI" ]]; then
        if command -v grub-mkstandalone &>/dev/null; then
            grub-mkstandalone \
                --format=x86_64-efi \
                --output="${ISO_ROOT}/EFI/BOOT/BOOTX64.EFI" \
                --locales="" \
                --fonts="" \
                "boot/grub/grub.cfg=${ISO_ROOT}/boot/grub/grub.cfg" 2>/dev/null || \
                log_warn "Failed to create GRUB EFI"
        fi
    fi

    log_success "GRUB configured"
}

# =============================================================================
# Create EFI boot image
# =============================================================================
create_efi_image() {
    log_step "Creating EFI boot image..."

    local efi_img="${ISO_ROOT}/boot/efiboot.img"

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
# Generate ISO
# =============================================================================
generate_iso() {
    log_step "Generating ISO image..."

    # Try full hybrid ISO first
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
        -eltorito-alt-boot \
        -e boot/efiboot.img \
        -no-emul-boot \
        -isohybrid-gpt-basdat \
        -graft-points \
        "${ISO_ROOT}" \
        /boot/grub/i386-pc=/usr/lib/grub/i386-pc \
        2>&1 | tee "${LOGS_DIR}/xorriso.log"; then
        log_success "Hybrid ISO created"
    else
        # Fallback to simpler EFI-only ISO
        log_warn "Hybrid ISO failed, creating EFI-only ISO..."
        xorriso -as mkisofs \
            -iso-level 3 \
            -R -J -joliet-long \
            -volid "${ISO_LABEL}" \
            -output "${ISO_OUTPUT}" \
            -eltorito-alt-boot \
            -e boot/efiboot.img \
            -no-emul-boot \
            -isohybrid-gpt-basdat \
            "${ISO_ROOT}" 2>&1 | tee "${LOGS_DIR}/xorriso.log"
    fi

    # Generate checksums
    sha256sum "${ISO_OUTPUT}" > "${ISO_OUTPUT}.sha256"
    md5sum "${ISO_OUTPUT}" > "${ISO_OUTPUT}.md5"

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
        echo "  Bootloader: RavenBoot (UEFI)"
    else
        echo "  Bootloader: GRUB (UEFI)"
    fi

    echo ""
    echo "  Test in QEMU (UEFI):"
    echo "    qemu-system-x86_64 -cdrom ${ISO_OUTPUT} -m 4G \\"
    echo "      -device virtio-vga-gl -display gtk,gl=on \\"
    echo "      -serial stdio \\"
    echo "      -bios /usr/share/edk2-ovmf/x64/OVMF_CODE.4m.fd -enable-kvm"
    echo ""
    echo "  Test in QEMU (BIOS):"
    echo "    qemu-system-x86_64 -cdrom ${ISO_OUTPUT} -m 4G -enable-kvm"
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
    setup_iso_structure
    create_live_init
    copy_boot_files
    copy_kernel_modules
    build_desktop_components
    copy_wayland_compositor
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
