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
args = ["--noclear", "--autologin", "root", "--login-program", "/sbin/login", "tty1", "linux"]
restart = true
enabled = true
critical = false

[[services]]
name = "getty-ttyS0"
description = "Serial console getty on ttyS0"
exec = "/sbin/agetty"
args = ["--noclear", "--autologin", "root", "--login-program", "/sbin/login", "-L", "115200", "ttyS0", "vt102"]
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
caw:x:970:raven
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
export PS1='[\u@raven-linux]# '

# Take the locale from what the build actually compiled, rather than naming one
# here. Hardcoding en_US.UTF-8 in four places while the build shipped none is
# what had every shell opening with "cannot change locale".
if [ -r /etc/locale.conf ]; then
    . /etc/locale.conf
fi
export LANG="${LANG:-C.UTF-8}"

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

# Start udevd. Not optional for a graphical session: libinput enumerates
# input devices through libudev, so an empty udev database means a compositor
# with no keyboard and no touchpad -- and the failure is completely silent from
# the compositor's side.
#
# Errors are shown rather than sent to /dev/null. Hiding them is why that class
# of failure had no evidence to work from.
udevd_bin=""
for candidate in /sbin/udevd /usr/lib/systemd/systemd-udevd /usr/bin/udevd; do
    [ -x "$candidate" ] && { udevd_bin="$candidate"; break; }
done

if [ -n "$udevd_bin" ]; then
    if "$udevd_bin" --daemon; then
        echo "udevd started ($udevd_bin)"
    else
        echo "warning: $udevd_bin failed to start; input devices will not be"
        echo "         enumerated and a graphical session will have no input."
    fi
else
    echo "warning: no udevd found; input devices will not be enumerated."
fi

# Devices that already exist when udevd starts generate no uevents of their
# own, so without this trigger the database stays empty for exactly the
# hardware that was present at boot -- the keyboard and touchpad included.
if command -v udevadm >/dev/null 2>&1; then
    udevadm trigger --action=add 2>/dev/null || udevadm trigger 2>/dev/null || true
    udevadm settle --timeout=10 2>/dev/null || true
    echo "udev: $(udevadm info --export-db 2>/dev/null | grep -c '^P: /devices') devices enumerated"
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
echo "                                       Version @RAVEN_VERSION@"
printf '\033[0m'
echo ""
printf '\033[1;37m'
echo "  ┌────────────────────────────────────────────────────────────────────────────────────────┐"
echo "  │  This is the RavenLinux base system: a musl userland, bash, and OpenSSH.                │"
echo "  └────────────────────────────────────────────────────────────────────────────────────────┘"
printf '\033[0m'
echo ""
printf '\033[1;36m'
echo "  Run 'raven-install' to install RavenLinux onto this machine's disk."
printf '\033[0m'
printf '\033[0;32m'
echo "  Type 'poweroff' to shutdown, 'reboot' to restart"
printf '\033[0m'
echo ""

cmdline="$(cat /proc/cmdline 2>/dev/null || true)"

# The live image's PID 1 is this script, not raven-init, so the cmdline
# handling raven-init does on an installed system has to be repeated here.
# Without this, booting the "Raven Desktop (Huginn)" entry lands on a console
# with raven.graphics=wayland silently ignored.
# cawd is enabled in /etc/raven/init.toml, but that file is raven-init's and
# raven-init is not PID 1 here -- this script is. Starting it by hand is what
# makes the wireless stack actually usable from the live image: without it,
# `caw scan` finds no daemon and autoconnect never runs.
#
# There is no clean-stop counterpart: raven-init calls `caw shutdown` through
# its stop_exec hook, and this script has no shutdown path at all, so a live
# reboot leaves the station on the air until the AP times it out.
start_caw_daemon() {
    [ -x /bin/cawd ] || [ -x /usr/bin/cawd ] || return 0
    pgrep -x cawd >/dev/null 2>&1 && return 0

    echo "Starting the CAW wireless daemon..."
    cawd >/dev/null 2>&1 &

    # cawd creates /run/caw itself; wait briefly so the first `caw` command
    # finds a socket rather than racing it.
    i=0
    while [ $i -lt 20 ] && [ ! -S /run/caw/caw.sock ]; do
        i=$((i + 1))
        sleep 0.1
    done
}

start_wayland_session() {
    echo "$cmdline" | grep -qE '(^| )raven\.graphics=wayland($| )' || return 1
    [ -x /bin/raven-wayland-session ] || {
        echo "raven.graphics=wayland requested, but no compositor is installed."
        return 1
    }

    # raven.wayland=<name> picks the compositor; the launcher defaults to
    # huginn and maps the older "raven" onto it.
    compositor="$(echo "$cmdline" | sed -n 's/.*[[:space:]]raven\.wayland=\([^[:space:]]*\).*/\1/p')"
    [ -n "$compositor" ] && export RAVEN_WAYLAND_COMPOSITOR="$compositor"

    # seatd owns the seat; libseat talks to it rather than to logind, which
    # does not exist here. Started in the background because it is a daemon.
    #
    # Looked up on PATH rather than probed at two fixed paths: seatd installs
    # to /sbin here, so testing only /bin and /usr/bin meant the daemon was
    # silently never started. libseat then found no socket to connect to and
    # huginn died with "Failed to open session: No such file or directory",
    # which reads like a seat/permissions problem rather than a missing daemon.
    seatd_bin="$(command -v seatd 2>/dev/null)"
    if [ -n "$seatd_bin" ]; then
        if ! pgrep -x seatd >/dev/null 2>&1; then
            "$seatd_bin" -g video >/dev/null 2>&1 &

            # Wait for the socket rather than guessing at a fixed sleep: the
            # socket appearing is the thing the compositor actually needs.
            i=0
            while [ ! -S /run/seatd.sock ] && [ "$i" -lt 50 ]; do
                i=$((i + 1))
                sleep 0.1
            done
        fi

        if [ -S /run/seatd.sock ]; then
            echo "seatd is up on /run/seatd.sock"
        else
            echo "warning: seatd started but /run/seatd.sock never appeared;"
            echo "         the compositor will not be able to acquire a seat."
        fi
    else
        echo "warning: seatd not found on PATH -- the compositor cannot acquire"
        echo "         a seat and the Wayland session will fall back to a console."
    fi

    export XDG_RUNTIME_DIR=/run/user/0
    export LIBSEAT_BACKEND=seatd
    mkdir -p "$XDG_RUNTIME_DIR"
    chmod 0700 "$XDG_RUNTIME_DIR"

    echo "Starting the Wayland session..."
    # Not exec: if the compositor dies we fall through to a console rather
    # than leaving the machine with no PID 1 and nothing on screen.
    /bin/raven-wayland-session
    echo "Wayland session exited; falling back to a console."
    return 1
}

# Root's login shell, as recorded in /etc/passwd. stage-raven.sh points this
# at ravenshell once ravenshell actually built; when it did not, the entry is
# still bash. Reading it here is what keeps the two in step -- hardcoding
# /bin/bash meant a successfully built ravenshell was never the shell you got
# on tty1. Falls back to bash, then sh, if the recorded shell is not executable.
login_shell() {
    local sh
    sh="$(sed -n 's/^root:[^:]*:[^:]*:[^:]*:[^:]*:[^:]*:\(.*\)$/\1/p' /etc/passwd 2>/dev/null | head -n1)"
    for candidate in "$sh" /bin/bash /bin/sh; do
        [ -n "$candidate" ] && [ -x "$candidate" ] && { echo "$candidate"; return 0; }
    done
    return 1
}

start_shell_loop() {
    cd /root

    shell="$(login_shell)" || shell=""

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
            if [ -n "$shell" ]; then
                # Use -- to separate openvt options from command arguments
                openvt -c 1 -w -s -f -- "$shell" -l || true
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

        # Number of consecutive agetty/login attempts that ended immediately.
        # A broken PAM stack (a missing unix_chkpwd, say) makes login exit at
        # once with "Authentication service cannot retrieve authentication
        # info", and retrying it forever leaves an unusable console with no way
        # in. After a few of those, give up on login and run the shell directly
        # -- this is a live image that autologs in as root regardless, so
        # nothing is being bypassed that was protecting anything.
        use_login=1
        login_failures=0

        while true; do
            if [ "$use_login" = 1 ] && [ "$tty_dev" = "/dev/ttyS0" ] && [ -x /sbin/agetty ]; then
                start=$(cut -d. -f1 /proc/uptime 2>/dev/null || echo 0)
                /sbin/agetty --noclear --autologin root --login-program /sbin/login -L 115200 ttyS0 vt102 || true
                end=$(cut -d. -f1 /proc/uptime 2>/dev/null || echo 0)

                if [ "$((end - start))" -lt 2 ]; then
                    login_failures=$((login_failures + 1))
                else
                    login_failures=0
                fi

                if [ "$login_failures" -ge 3 ]; then
                    use_login=0
                    echo ""
                    echo "login is failing immediately -- check the PAM stack (/etc/pam.d/login,"
                    echo "/lib/security, and the unix_chkpwd helper). Starting a shell directly."
                fi
            elif [ -n "$shell" ]; then
                "$shell" -l
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

start_caw_daemon
start_wayland_session
start_shell_loop
INIT

    # The heredoc above is quoted -- it has to be, or every $var in the init
    # script would expand at build time -- so the banner's version is a
    # placeholder filled in here. It used to be the literal number, which drifted
    # the moment RAVEN_VERSION changed anywhere else.
    sed -i "s/@RAVEN_VERSION@/${RAVEN_VERSION}/g" "${SYSROOT_DIR}/init"

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
# /bin/raven-wayland-session, which runs huginn and then muninn as its client.
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
    #   live ISO         -- PID 1 is the live-init shell script. raven-rc would
    #                       write /run/raven-init.cmd, succeed, and be read by
    #                       nobody, so the command would silently do nothing.
    #                       Ask the kernel directly instead (CONFIG_MAGIC_SYSRQ).
    #
    # Checking /proc/1/comm at runtime rather than guessing at build time is
    # what lets the same image do the right thing once installed to disk.
    local name key
    for spec in reboot:b poweroff:o halt:o shutdown:o; do
        name="${spec%%:*}"
        key="${spec##*:}"

        cat > "${SYSROOT_DIR}/bin/${name}" << EOF
#!/bin/sh
# RavenLinux ${name}. There is no systemd here; raven-init is PID 1 on an
# installed system and this script is PID 1 on the live image.
if [ -x /bin/raven-rc ] && grep -qs raven-init /proc/1/comm 2>/dev/null; then
    exec /bin/raven-rc ${name}
fi

sync
[ -w /proc/sys/kernel/sysrq ] && echo 1 > /proc/sys/kernel/sysrq
echo ${key} > /proc/sysrq-trigger
EOF
        chmod 0755 "${SYSROOT_DIR}/bin/${name}"
        mkdir -p "${SYSROOT_DIR}/usr/bin"
        ln -sf "../../bin/${name}" "${SYSROOT_DIR}/usr/bin/${name}" 2>/dev/null || true
    done

    log_success "Shutdown commands installed (reboot, poweroff, halt, shutdown)"
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

    if [[ ! -f "${src}" ]]; then
        log_warn "scripts/installer/raven-install not found; the ISO will not be able to install itself"
        return 0
    fi

    mkdir -p "${SYSROOT_DIR}/usr/sbin" "${SYSROOT_DIR}/sbin"
    cp "${src}" "${SYSROOT_DIR}/usr/sbin/raven-install"
    chmod 0755 "${SYSROOT_DIR}/usr/sbin/raven-install"

    # /sbin is where the rest of the system administration commands live in this
    # sysroot, and it is on root's PATH; /usr/sbin is not always.
    ln -sf ../usr/sbin/raven-install "${SYSROOT_DIR}/sbin/raven-install" 2>/dev/null || true

    log_success "raven-install installed to /usr/sbin/raven-install"
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
    setup_iso_structure
    create_live_init
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
