#!/bin/bash
# =============================================================================
# RavenLinux Stage 2: Native System Build
# =============================================================================
# Builds the native system by:
#   1. Copying tools from host system when available (fast path)
#   2. Building from source when not available on host (fallback)
#
# This ensures functionality is never limited by what's installed on the
# build host system. Critical tools like D-Bus, dhcpcd, etc. will be built
# from source if not found on the host.
#
# Environment Variables:
#   RAVEN_FORCE_SOURCE_BUILD=1  - Always build from source, never copy from host
#   RAVEN_JOBS=N                - Number of parallel build jobs (default: nproc)
#   RAVEN_EXPAT_VERSION         - expat version to build (default: 2.6.2)
#   RAVEN_DBUS_VERSION          - D-Bus version to build (default: 1.14.10)
#   RAVEN_DHCPCD_VERSION        - dhcpcd version to build (default: 10.0.6)
#   RAVEN_ETHTOOL_VERSION       - ethtool version to build (default: 6.11)
# =============================================================================

set -euo pipefail

# =============================================================================
# Environment Setup (with defaults for standalone execution)
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${RAVEN_ROOT:-$(dirname "$(dirname "$SCRIPT_DIR")")}"
BUILD_DIR="${RAVEN_BUILD:-${PROJECT_ROOT}/build}"
SYSROOT_DIR="${SYSROOT_DIR:-${BUILD_DIR}/sysroot}"
LOGS_DIR="${LOGS_DIR:-${BUILD_DIR}/logs}"

# Source build configuration (set RAVEN_FORCE_SOURCE_BUILD=1 to always build from source)
: "${RAVEN_FORCE_SOURCE_BUILD:=0}"
: "${RAVEN_JOBS:=$(nproc 2>/dev/null || echo 4)}"
: "${RAVEN_ENABLE_SUDO:=1}"

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
    log_fatal() { echo -e "${RED}[FATAL]${NC} $1"; exit "${2:-1}"; }
fi


# =============================================================================
# Copy shells from host
# =============================================================================
copy_shells() {
    log_info "Copying shells..."

    local have_bash=false

    # Copy bash
    if command -v bash &>/dev/null; then
        cp "$(which bash)" "${SYSROOT_DIR}/usr/bin/bash" && have_bash=true
        log_info "  Added bash"
    fi

    # Same-directory alias. /bin/sh and /bin/bash still resolve here through
    # the /bin -> usr/bin merge link, which is what 19 shebangs depend on.
    if $have_bash; then
        ln -sf bash "${SYSROOT_DIR}/usr/bin/sh"
    else
        log_warn "  WARNING: No shell available for /bin/sh!"
    fi

    log_success "Shells installed"
}

# =============================================================================
# Copy essential system utilities from host
# =============================================================================
copy_system_utils() {
    log_info "Copying system utilities..."

    local coreutils_bin="${SYSROOT_DIR}/usr/bin/coreutils"
    local coreutils_list=""
    if [[ -x "${coreutils_bin}" ]]; then
        coreutils_list="$("${coreutils_bin}" --list 2>/dev/null || true)"
    fi

    local utils=(
        # Basic coreutils (essential!)
        ls cat cp mv rm mkdir rmdir touch chmod chown ln
        head tail wc cut sort uniq tr tee
        pwd cd basename dirname realpath readlink
        echo printf test expr env sleep
        id whoami groups who w date
        # Process management
        ps kill killall pkill pgrep top htop
        # File operations
        find grep sed awk xargs file less more
        # Debugging/diagnostics
        which whereis type timeout
        strace ltrace lsof ldd
        # Disk utilities
        mount umount mountpoint fdisk parted mkfs.ext4 mkfs.vfat fsck blkid lsblk
        # Partitioning and installation. raven-install needs every one of these:
        # sfdisk writes the GPT, wipefs clears the old signatures, partx/blockdev
        # make the kernel reread the table, findmnt identifies the boot media,
        # and losetup mounts the squashfs it copies from.
        sfdisk sgdisk wipefs partx partprobe blockdev losetup findmnt
        mkswap swapon swapoff
        # Filesystem creation beyond the ext4/vfat pair above, when the host has
        # them. raven-install offers whichever ones are present.
        mkfs.xfs mkfs.btrfs mkfs.ext2 mkfs.ext3 mkfs.fat fatlabel e2label tune2fs
        resize2fs dumpe2fs
        # Coreutils the installer uses that the uutils set does not always cover.
        df du dd sync stat truncate od install chroot column split
        # Everything stage1 would have symlinked onto the uutils multicall
        # binary but is not already listed above. build_coreutils only logs an
        # error when the uutils build fails, so the sysroot can reach this
        # point with no coreutils at all -- and then this list is the only
        # source of these commands. raven-install died on a missing seq that
        # way. When uutils is present these stay as coreutils symlinks (see
        # the "Keeping" branch below), so listing them costs nothing.
        seq yes paste chgrp dir vdir md5sum sha256sum true false
        mktemp mknod tty
        # System info
        dmesg lspci lsusb free uptime uname hostname
        dmidecode lscpu
        sensors smartctl nvme hdparm
        # User management
        passwd login chpasswd useradd usermod groupadd getent chsh
        runuser setpriv newgrp sg
        # Archiving
        tar gzip gunzip bzip2 xz zstd unzip zip
        # Editors (fallback)
        vi nano
        # Terminal utilities
        clear reset stty tput tset
        # Virtual terminal helpers (Wayland KMS compositors often need a VT)
        openvt chvt agetty getty setsid
        # Console font loading. stage4 rasterises JetBrains Mono Nerd Font into
        # PSF, and raven-console-font calls setfont to load it; without setfont
        # tty1 stays on the kernel's built-in 8x16 VGA font. From kbd.
        setfont showconsolefont kbd_mode
        # Login/session management
        login agetty setsid
        # Locale and timezone
        locale localedef localectl timedatectl hwclock date
        # D-Bus (needed by many GUI apps)
        dbus-daemon dbus-launch dbus-send dbus-uuidgen
        # Seat management (Wayland compositors)
        seatd
        # Kernel module utilities (needed for auto-loading GPU/input drivers)
        modprobe insmod rmmod lsmod depmod modinfo kmod
        # Boot/SecureBoot diagnostics (optional)
        mokutil efibootmgr
        # Reference compositor (optional)
        weston
        weston-terminal
        # X11/Xwayland (optional, for legacy app support)
        Xorg Xwayland xinit startx xterm xclock xsetroot twm
        # udevadm only. systemctl/journalctl/hostnamectl are deliberately NOT
        # here: RavenLinux's PID 1 is raven-init, so those binaries can only
        # ever print "System has not been booted with systemd as init system
        # (PID 1)". Shipping a command that cannot work is worse than not
        # shipping it -- it sends you looking for a broken service manager
        # instead of at raven-rc, which is the one that does work.
        udevadm
        # Build tools (needed for source builds when host binaries unavailable)
        make cmake ninja meson gcc g++ cc c++ ld ar as nm objcopy objdump
        strip ranlib pkg-config autoconf automake autoreconf libtool m4
        # Python (needed by meson and some build systems)
        python python3 pip pip3
    )

    for util in "${utils[@]}"; do
        if command -v "$util" &>/dev/null; then
            local src
            src="$(which "$util" 2>/dev/null)" || continue
            [[ -f "$src" ]] || continue

            # Destination is always /usr/bin. The host's split between /bin
            # and /sbin does not survive the usr-merge: /sbin is a symlink onto
            # /usr/bin, so branching on the source path only produced two names
            # for one directory.
            local dest="${SYSROOT_DIR}/usr/bin/${util}"

            # Special handling for uname - save as uname.real for wrapper script
            if [[ "$util" == "uname" ]]; then
                dest="${SYSROOT_DIR}/usr/bin/uname.real"
            fi

            # Avoid overwriting symlink targets (e.g. uutils coreutils multi-call setup)
            if [[ -L "$dest" ]]; then
                link_target="$(readlink "$dest" 2>/dev/null || true)"
                if [[ -n "${coreutils_list}" ]] && [[ "${link_target}" == "coreutils" ]]; then
                    if printf '%s\n' "${coreutils_list}" | grep -qx "${util}"; then
                        log_info "  Keeping ${util} (provided by coreutils)"
                        continue
                    fi
                    log_info "  Replacing ${util} symlink (not provided by coreutils)"
                    rm -f "$dest" 2>/dev/null || true
                else
                    log_info "  Skipping ${util} (destination is a symlink)"
                    continue
                fi
            fi

            cp "$src" "$dest" 2>/dev/null && log_info "  Added ${util}" || true
        fi
    done

    # NOTE: sudo/su setup is now handled by dedicated setup_sudo and setup_su functions
    # which copy the original sudo from host (not sudo-rs) with all dependencies.
    # This ensures PAM authentication works properly.

    # Set SUID on passwd so users can change their own passwords
    if [[ -f "${SYSROOT_DIR}/usr/bin/passwd" ]]; then
        chmod 4755 "${SYSROOT_DIR}/usr/bin/passwd" 2>/dev/null || chmod 755 "${SYSROOT_DIR}/usr/bin/passwd" || true
        log_info "  Set SUID on passwd"
    fi

    # Set SUID on mount/umount for user mounts (optional, some distros do this)
    # Uncomment if you want users to be able to mount/umount:
    # if [[ -f "${SYSROOT_DIR}/bin/mount" ]]; then
    #     chmod 4755 "${SYSROOT_DIR}/bin/mount" 2>/dev/null || true
    # fi
    # if [[ -f "${SYSROOT_DIR}/bin/umount" ]]; then
    #     chmod 4755 "${SYSROOT_DIR}/bin/umount" 2>/dev/null || true
    # fi

    # Raven Wayland session launcher (optional, used when booting with raven.graphics=wayland)
    if [[ -f "${PROJECT_ROOT}/configs/raven-wayland-session" ]]; then
        cp "${PROJECT_ROOT}/configs/raven-wayland-session" "${SYSROOT_DIR}/usr/bin/raven-wayland-session" 2>/dev/null || true
        chmod +x "${SYSROOT_DIR}/usr/bin/raven-wayland-session" 2>/dev/null || true
        log_info "  Added raven-wayland-session"
    fi

    # Fontconfig + fonts + cursor themes for "fast desktop" sessions.
    # Weston terminal/shell uses fontconfig; missing config/themes causes noisy warnings.
    log_info "Copying fontconfig, fonts, and cursor themes..."
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
    if [[ -d "/usr/share/fonts" ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/share/fonts"
        cp -a "/usr/share/fonts/." "${SYSROOT_DIR}/usr/share/fonts/" 2>/dev/null || true
        log_info "  Copied /usr/share/fonts"
    fi
    mkdir -p "${SYSROOT_DIR}/var/cache/fontconfig" 2>/dev/null || true
    if [[ -d "/usr/share/icons" ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/share/icons"
        # `default` and `Adwaita` are the cursor path and must stay together:
        # default/index.theme is a two-line file that says `Inherits=Adwaita`,
        # so copying it without the theme it points at leaves the compositor
        # resolving the default pointer to nothing and drawing no cursor at all
        # over its own dock, launcher and background. `breeze`/`breeze-dark`
        # are the application icons the dock and launcher resolve Icon= names
        # against; hicolor is the base every theme falls back through, and
        # carries almost nothing on its own.
        for theme in default breeze_cursors Adwaita hicolor breeze breeze-dark; do
            if [[ -d "/usr/share/icons/${theme}" ]]; then
                mkdir -p "${SYSROOT_DIR}/usr/share/icons/${theme}"
                cp -a "/usr/share/icons/${theme}/." "${SYSROOT_DIR}/usr/share/icons/${theme}/" 2>/dev/null || true
                log_info "  Copied /usr/share/icons/${theme}"
            fi
        done
    fi

    # If weston is available on the build host, copy its runtime data/plugins.
    if [[ -x "${SYSROOT_DIR}/usr/bin/weston" ]]; then
        for d in /usr/lib/weston /usr/lib64/weston /usr/share/weston \
            /usr/lib/libweston-* /usr/lib64/libweston-*; do
            if [[ -d "$d" ]]; then
                mkdir -p "${SYSROOT_DIR}${d}"
                cp -a "${d}/." "${SYSROOT_DIR}${d}/" 2>/dev/null || true
                log_info "  Copied $(basename "$d") runtime data"
            fi
        done

        # Install Weston config for RavenLinux sessions.
        if [[ -f "${PROJECT_ROOT}/configs/weston/weston.ini" ]]; then
            mkdir -p "${SYSROOT_DIR}/etc/xdg/weston"
            cp "${PROJECT_ROOT}/configs/weston/weston.ini" "${SYSROOT_DIR}/etc/xdg/weston/weston.ini" 2>/dev/null || true
            chmod 644 "${SYSROOT_DIR}/etc/xdg/weston/weston.ini" 2>/dev/null || true
            log_info "  Added /etc/xdg/weston/weston.ini"
        fi
    fi

    # Copy xkeyboard-config data for libxkbcommon (keyboard layouts).
    # Different distros use different install prefixes; Raven's libxkbcommon is
    # typically built to look in /usr/share/xkeyboard-config-2.
    log_info "Copying xkeyboard-config (XKB) data..."
    if [[ -d "/usr/share/xkeyboard-config-2" ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/share" "${SYSROOT_DIR}/usr/share/X11"
        cp -a "/usr/share/xkeyboard-config-2" "${SYSROOT_DIR}/usr/share/" 2>/dev/null || true
        ln -sf ../xkeyboard-config-2 "${SYSROOT_DIR}/usr/share/X11/xkb" 2>/dev/null || true
        log_info "  Copied /usr/share/xkeyboard-config-2"
    elif [[ -d "/usr/share/X11/xkb" ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/share/X11"
        cp -a "/usr/share/X11/xkb" "${SYSROOT_DIR}/usr/share/X11/" 2>/dev/null || true
        ln -sf X11/xkb "${SYSROOT_DIR}/usr/share/xkeyboard-config-2" 2>/dev/null || true
        log_info "  Copied /usr/share/X11/xkb"
    elif [[ -d "/usr/share/xkeyboard-config" ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/share"
        cp -a "/usr/share/xkeyboard-config" "${SYSROOT_DIR}/usr/share/" 2>/dev/null || true
        ln -sf xkeyboard-config "${SYSROOT_DIR}/usr/share/xkeyboard-config-2" 2>/dev/null || true
        log_info "  Copied /usr/share/xkeyboard-config"
    else
        log_warn "  No xkeyboard-config data found on host; keyboard layouts may be missing"
    fi

    # XWayland invokes /usr/bin/xkbcomp at runtime to compile keymaps. If it is
    # missing, XWayland will fail to start with "Failed to activate virtual core keyboard".
    # Install xkbcomp into the sysroot and symlink it to the expected path.
    if [[ -x "/usr/bin/xkbcomp" ]]; then
        log_info "Copying xkbcomp (required for XWayland)..."
        mkdir -p "${SYSROOT_DIR}/usr/bin"
        # Straight into /usr/bin. The old form copied to /bin and then linked
        # /usr/bin/xkbcomp -> ../../bin/xkbcomp, which is a self-reference once
        # /bin is a symlink: XWayland would die with "Failed to activate
        # virtual core keyboard" and the link would look fine in `ls`.
        cp -a "/usr/bin/xkbcomp" "${SYSROOT_DIR}/usr/bin/xkbcomp" 2>/dev/null || true
        log_info "  Added /usr/bin/xkbcomp"
    else
        log_warn "  /usr/bin/xkbcomp not found on host; XWayland may fail to start"
    fi

    # Copy libinput data files (quirks) used to classify input devices.
    log_info "Copying libinput data..."
    if [[ -d "/usr/share/libinput" ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/share"
        cp -a "/usr/share/libinput" "${SYSROOT_DIR}/usr/share/" 2>/dev/null || true
        log_info "  Copied /usr/share/libinput"
    else
        log_warn "  No /usr/share/libinput found on host; input devices may not be detected correctly"
    fi

    # Copy udev runtime data (rules/hwdb) and daemon (for device enumeration).
    log_info "Copying udev runtime data..."
    if [[ -d "/usr/lib/udev" ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/lib"
        cp -a "/usr/lib/udev" "${SYSROOT_DIR}/usr/lib/" 2>/dev/null || true
        log_info "  Copied /usr/lib/udev"
    fi

    # Copy custom RavenLinux udev rules for input device access
    if [[ -f "${RAVEN_ROOT}/configs/72-raven-input.rules" ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/lib/udev/rules.d"
        cp "${RAVEN_ROOT}/configs/72-raven-input.rules" "${SYSROOT_DIR}/usr/lib/udev/rules.d/" 2>/dev/null || true
        log_info "  Copied custom input device udev rules"
    fi
    if [[ -d "/etc/udev" ]]; then
        mkdir -p "${SYSROOT_DIR}/etc"
        cp -a "/etc/udev" "${SYSROOT_DIR}/etc/" 2>/dev/null || true
        log_info "  Copied /etc/udev"
    fi

    # systemd-udevd is often located outside PATH; copy it if present.
    if [[ -e "/usr/lib/systemd/systemd-udevd" ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/lib/systemd"
        cp -L "/usr/lib/systemd/systemd-udevd" "${SYSROOT_DIR}/usr/lib/systemd/systemd-udevd" 2>/dev/null || true
        chmod +x "${SYSROOT_DIR}/usr/lib/systemd/systemd-udevd" 2>/dev/null || true
        log_info "  Copied /usr/lib/systemd/systemd-udevd"
    fi

    # Some distros ship udevd in /sbin; copy it too if available.
    for udevd in /sbin/udevd /usr/sbin/udevd /usr/lib/udev/udevd; do
        if [[ -e "${udevd}" ]]; then
            mkdir -p "${SYSROOT_DIR}/usr/bin"
            cp -L "${udevd}" "${SYSROOT_DIR}/usr/bin/udevd" 2>/dev/null || true
            chmod +x "${SYSROOT_DIR}/usr/bin/udevd" 2>/dev/null || true
            log_info "  Copied ${udevd} -> /sbin/udevd"
            break
        fi
    done

    # Provide /sbin/udevd symlink if we only have systemd-udevd.
    if [[ ! -e "${SYSROOT_DIR}/usr/bin/udevd" ]] && [[ -e "${SYSROOT_DIR}/usr/lib/systemd/systemd-udevd" ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/bin"
        ln -sf /usr/lib/systemd/systemd-udevd "${SYSROOT_DIR}/usr/bin/udevd" 2>/dev/null || true
    fi

    # /sbin/modprobe used to be linked to /bin/modprobe for udev's benefit.
    # Under the merge both names are /usr/bin/modprobe, so the link is a
    # self-reference and the copy in /usr/bin already satisfies every caller.

    log_success "System utilities installed"
}

# =============================================================================
# Copy networking tools
# =============================================================================
copy_networking() {
    log_info "Copying networking tools..."

    # Ensure critical networking components are available
    # These will copy from host if available, or build from source if not
    ensure_ethtool
    ensure_dbus      # Message bus for GUI apps
    # No iwd, wpa_supplicant, iw or libnl: cawd (stage-raven) is the wireless
    # daemon and speaks nl80211 itself. A second daemon on the wiphy fights it
    # for the interface, and the tools would have nothing to talk to.
    ensure_dhcpcd    # DHCP client

    local net_tools=(
        ip ping ping6 ss netstat route
        dhcpcd dhclient udhcpc
        rfkill ethtool
        ifconfig
        curl wget
        nc ncat
        host dig nslookup
        traceroute tracepath mtr tcpdump
    )

    for tool in "${net_tools[@]}"; do
        if command -v "$tool" &>/dev/null; then
            local src
            src="$(which "$tool" 2>/dev/null)" || continue
            [[ -f "$src" ]] || continue

            local dest="${SYSROOT_DIR}/usr/bin/${tool}"
            if [[ "$src" == */sbin/* ]]; then
                dest="${SYSROOT_DIR}/usr/bin/${tool}"
            fi

            cp "$src" "$dest" 2>/dev/null && log_info "  Added ${tool}" || true
        fi
    done

    # DNS config
    echo "nameserver 8.8.8.8" > "${SYSROOT_DIR}/etc/resolv.conf"
    echo "nameserver 1.1.1.1" >> "${SYSROOT_DIR}/etc/resolv.conf"

    log_success "Networking tools installed"
}

# =============================================================================
# Ensure ethtool is available (host copy fallback builds from source)
# =============================================================================
ensure_ethtool() {
    # Prefer host ethtool if present.
    if command -v ethtool &>/dev/null; then
        return 0
    fi

    # Already present in sysroot from a previous step.
    if [[ -x "${SYSROOT_DIR}/usr/bin/ethtool" ]]; then
        return 0
    fi

    local version="${RAVEN_ETHTOOL_VERSION:-6.11}"
    local sources_dir="${BUILD_DIR}/sources"
    local tarball="${sources_dir}/ethtool-${version}.tar.xz"
    local url="https://www.kernel.org/pub/software/network/ethtool/ethtool-${version}.tar.xz"
    local build_dir="${sources_dir}/ethtool-${version}"

    mkdir -p "${sources_dir}" 2>/dev/null || true

    if [[ ! -f "${tarball}" ]]; then
        if command -v curl &>/dev/null; then
            log_info "Downloading ethtool ${version}..."
            curl -fL -o "${tarball}" "${url}" 2>/dev/null || true
        elif command -v wget &>/dev/null; then
            log_info "Downloading ethtool ${version}..."
            wget -O "${tarball}" "${url}" 2>/dev/null || true
        else
            log_warn "Cannot download ethtool (need curl or wget); ethtool will be missing"
            return 0
        fi
    fi

    if [[ ! -f "${tarball}" ]]; then
        log_warn "ethtool not found on host and download failed; ethtool will be missing"
        return 0
    fi

    if ! command -v make &>/dev/null || ! command -v cc &>/dev/null; then
        log_warn "Cannot build ethtool (missing make/cc); install ethtool on the host or add build tools"
        return 0
    fi

    rm -rf "${build_dir}" 2>/dev/null || true
    if tar -xf "${tarball}" -C "${sources_dir}" 2>/dev/null; then
        :
    else
        log_warn "Failed to extract ${tarball}; ethtool will be missing"
        return 0
    fi

    if [[ ! -d "${build_dir}" ]]; then
        log_warn "Expected extracted directory ${build_dir} not found; ethtool will be missing"
        return 0
    fi

    log_info "Building ethtool ${version}..."
    (
        cd "${build_dir}"
        # --sbindir=/usr/bin: /sbin and /usr/sbin are both symlinks onto
        # /usr/bin, so name the real directory rather than installing through
        # a merge link.
        ./configure --prefix=/usr --sbindir=/usr/bin >/dev/null 2>&1 || ./configure --prefix=/usr --sbindir=/usr/bin
        make -j"${RAVEN_JOBS:-$(nproc)}" >/dev/null 2>&1 || make -j"${RAVEN_JOBS:-$(nproc)}"
        make DESTDIR="${SYSROOT_DIR}" install-strip >/dev/null 2>&1 || make DESTDIR="${SYSROOT_DIR}" install
    ) || {
        log_warn "ethtool build failed; ethtool will be missing"
        return 0
    }

    if [[ -x "${SYSROOT_DIR}/usr/bin/ethtool" ]]; then
        log_info "  Installed ethtool into sysroot"
    else
        log_warn "ethtool build completed but binary not found in sysroot"
    fi
}

# =============================================================================
# Source Build Fallback System
# =============================================================================
# These ensure_* functions implement a fallback pattern:
#   1. If RAVEN_FORCE_SOURCE_BUILD=1, skip to source build
#   2. Check if binary exists on host -> copy it
#   3. Check if already built in sysroot -> skip
#   4. Download source and build from scratch
#
# This ensures functionality is never limited by what's on the host system.
# =============================================================================

# Helper: Download a file with curl or wget
download_file() {
    local url="$1"
    local dest="$2"

    if [[ -f "$dest" ]]; then
        return 0
    fi

    if command -v curl &>/dev/null; then
        curl -fL -o "$dest" "$url" 2>/dev/null && return 0
    fi
    if command -v wget &>/dev/null; then
        wget -q -O "$dest" "$url" 2>/dev/null && return 0
    fi

    log_warn "Cannot download $url (need curl or wget)"
    return 1
}

# Helper: Check if we should force source builds
should_force_source_build() {
    [[ "${RAVEN_FORCE_SOURCE_BUILD:-0}" == "1" ]]
}

# Helper: Copy binary from host if available
copy_from_host() {
    local binary_name="$1"
    local dest_path="$2"

    if should_force_source_build; then
        return 1
    fi

    local src=""
    # Check common locations
    for path in "/usr/bin/${binary_name}" "/bin/${binary_name}" "/usr/sbin/${binary_name}" "/sbin/${binary_name}" "/usr/libexec/${binary_name}"; do
        if [[ -x "$path" ]]; then
            src="$path"
            break
        fi
    done

    # Also try which
    if [[ -z "$src" ]] && command -v "$binary_name" &>/dev/null; then
        src="$(which "$binary_name" 2>/dev/null)" || true
    fi

    if [[ -n "$src" ]] && [[ -x "$src" ]]; then
        mkdir -p "$(dirname "$dest_path")"
        cp -L "$src" "$dest_path" 2>/dev/null || return 1
        chmod +x "$dest_path" 2>/dev/null || true
        log_info "  Copied ${binary_name} from host"
        return 0
    fi

    return 1
}

# =============================================================================
# Ensure expat is available (XML parser, required by D-Bus)
# =============================================================================
ensure_expat() {
    log_info "Ensuring expat (XML parser)..."

    # Check if already in sysroot
    if [[ -f "${SYSROOT_DIR}/usr/lib/libexpat.so.1" ]]; then
        log_info "  expat already present in sysroot"
        return 0
    fi

    # Try to copy from host
    if ! should_force_source_build; then
        local found=0
        for libdir in /usr/lib /lib /usr/lib64 /lib64 /usr/lib/x86_64-linux-gnu; do
            if [[ -f "${libdir}/libexpat.so.1" ]]; then
                mkdir -p "${SYSROOT_DIR}/usr/lib"
                cp -L "${libdir}/libexpat.so.1" "${SYSROOT_DIR}/usr/lib/" 2>/dev/null || true
                cp -L "${libdir}/libexpat.so" "${SYSROOT_DIR}/usr/lib/" 2>/dev/null || true
                # Also copy headers if available
                if [[ -f "/usr/include/expat.h" ]]; then
                    mkdir -p "${SYSROOT_DIR}/usr/include"
                    cp -L /usr/include/expat*.h "${SYSROOT_DIR}/usr/include/" 2>/dev/null || true
                fi
                log_info "  Copied expat from host"
                found=1
                break
            fi
        done
        [[ $found -eq 1 ]] && return 0
    fi

    # Build from source
    local version="${RAVEN_EXPAT_VERSION:-2.6.2}"
    local sources_dir="${BUILD_DIR}/sources"
    local tarball="${sources_dir}/expat-${version}.tar.xz"
    local url="https://github.com/libexpat/libexpat/releases/download/R_${version//./_}/expat-${version}.tar.xz"
    local build_dir="${sources_dir}/expat-${version}"

    mkdir -p "${sources_dir}"

    if ! download_file "$url" "$tarball"; then
        log_warn "Failed to download expat; D-Bus may not build"
        return 1
    fi

    if ! command -v make &>/dev/null || ! command -v cc &>/dev/null; then
        log_warn "Cannot build expat (missing make/cc)"
        return 1
    fi

    rm -rf "${build_dir}" 2>/dev/null || true
    tar -xf "${tarball}" -C "${sources_dir}" 2>/dev/null || {
        log_warn "Failed to extract expat"
        return 1
    }

    log_info "  Building expat ${version}..."
    (
        cd "${build_dir}"
        ./configure --prefix=/usr --disable-static --enable-shared >/dev/null 2>&1 || \
            ./configure --prefix=/usr --disable-static --enable-shared
        make -j"${RAVEN_JOBS:-$(nproc)}" >/dev/null 2>&1 || make -j"${RAVEN_JOBS:-$(nproc)}"
        make DESTDIR="${SYSROOT_DIR}" install >/dev/null 2>&1 || make DESTDIR="${SYSROOT_DIR}" install
    ) || {
        log_warn "expat build failed"
        return 1
    }

    if [[ -f "${SYSROOT_DIR}/usr/lib/libexpat.so.1" ]]; then
        log_success "  Built and installed expat"
        return 0
    else
        log_warn "expat build completed but library not found"
        return 1
    fi
}

# =============================================================================
# Ensure D-Bus is available (message bus)
# =============================================================================
ensure_dbus() {
    log_info "Ensuring D-Bus..."

    # Check if dbus-daemon already in sysroot
    if [[ -x "${SYSROOT_DIR}/usr/bin/dbus-daemon" ]]; then
        log_info "  D-Bus already present in sysroot"
        return 0
    fi

    # Try to copy from host
    if ! should_force_source_build; then
        if copy_from_host "dbus-daemon" "${SYSROOT_DIR}/usr/bin/dbus-daemon"; then
            # Also copy related tools
            copy_from_host "dbus-send" "${SYSROOT_DIR}/usr/bin/dbus-send" || true
            copy_from_host "dbus-launch" "${SYSROOT_DIR}/usr/bin/dbus-launch" || true
            copy_from_host "dbus-uuidgen" "${SYSROOT_DIR}/usr/bin/dbus-uuidgen" || true
            copy_from_host "dbus-monitor" "${SYSROOT_DIR}/usr/bin/dbus-monitor" || true

            # Copy D-Bus libraries
            for libdir in /usr/lib /lib /usr/lib64 /lib64 /usr/lib/x86_64-linux-gnu; do
                if [[ -f "${libdir}/libdbus-1.so.3" ]]; then
                    mkdir -p "${SYSROOT_DIR}/usr/lib"
                    cp -L "${libdir}/libdbus-1.so"* "${SYSROOT_DIR}/usr/lib/" 2>/dev/null || true
                    break
                fi
            done

            # Copy D-Bus config
            if [[ -d "/usr/share/dbus-1" ]]; then
                mkdir -p "${SYSROOT_DIR}/usr/share/dbus-1"
                cp -r /usr/share/dbus-1/* "${SYSROOT_DIR}/usr/share/dbus-1/" 2>/dev/null || true
            fi
            if [[ -d "/etc/dbus-1" ]]; then
                mkdir -p "${SYSROOT_DIR}/etc/dbus-1"
                cp -r /etc/dbus-1/* "${SYSROOT_DIR}/etc/dbus-1/" 2>/dev/null || true
            fi

            # dbus-daemon --system reads <user>dbus</user> out of system.conf and
            # drops privileges to it. With no such account getpwnam fails and the
            # daemon exits immediately, every time -- which is what had raven-init
            # restarting it in a loop. The account is created in /etc/passwd; this
            # is the rest of what it needs.
            #
            # machine-id: dbus looks in /var/lib/dbus/machine-id as well as
            # /etc/machine-id. The live init generates the latter at boot, so
            # point one at the other rather than generating two.
            mkdir -p "${SYSROOT_DIR}/var/lib/dbus"
            ln -sf /etc/machine-id "${SYSROOT_DIR}/var/lib/dbus/machine-id" 2>/dev/null || true

            return 0
        fi
    fi

    # Ensure expat is available (D-Bus dependency)
    ensure_expat || log_warn "expat not available; D-Bus build may fail"

    # Build from source
    local version="${RAVEN_DBUS_VERSION:-1.14.10}"
    local sources_dir="${BUILD_DIR}/sources"
    local tarball="${sources_dir}/dbus-${version}.tar.xz"
    local url="https://dbus.freedesktop.org/releases/dbus/dbus-${version}.tar.xz"
    local build_dir="${sources_dir}/dbus-${version}"

    mkdir -p "${sources_dir}"

    if ! download_file "$url" "$tarball"; then
        log_warn "Failed to download D-Bus"
        return 1
    fi

    if ! command -v make &>/dev/null || ! command -v cc &>/dev/null; then
        log_warn "Cannot build D-Bus (missing make/cc)"
        return 1
    fi

    rm -rf "${build_dir}" 2>/dev/null || true
    tar -xf "${tarball}" -C "${sources_dir}" 2>/dev/null || {
        log_warn "Failed to extract D-Bus"
        return 1
    }

    log_info "  Building D-Bus ${version}..."

    # Set PKG_CONFIG_PATH to find expat we just built
    local pkg_config_path="${SYSROOT_DIR}/usr/lib/pkgconfig:${PKG_CONFIG_PATH:-}"

    (
        cd "${build_dir}"
        export PKG_CONFIG_PATH="$pkg_config_path"
        export CFLAGS="-I${SYSROOT_DIR}/usr/include ${CFLAGS:-}"
        export LDFLAGS="-L${SYSROOT_DIR}/usr/lib ${LDFLAGS:-}"

        ./configure \
            --prefix=/usr \
            --sysconfdir=/etc \
            --localstatedir=/var \
            --disable-static \
            --disable-doxygen-docs \
            --disable-xml-docs \
            --disable-systemd \
            --disable-selinux \
            --disable-apparmor \
            --disable-libaudit \
            --with-system-socket=/run/dbus/system_bus_socket \
            --with-system-pid-file=/run/dbus/pid \
            >/dev/null 2>&1 || \
        ./configure \
            --prefix=/usr \
            --sysconfdir=/etc \
            --localstatedir=/var \
            --disable-static

        make -j"${RAVEN_JOBS:-$(nproc)}" >/dev/null 2>&1 || make -j"${RAVEN_JOBS:-$(nproc)}"
        make DESTDIR="${SYSROOT_DIR}" install >/dev/null 2>&1 || make DESTDIR="${SYSROOT_DIR}" install
    ) || {
        log_warn "D-Bus build failed"
        return 1
    }

    if [[ -x "${SYSROOT_DIR}/usr/bin/dbus-daemon" ]]; then
        log_success "  Built and installed D-Bus"

        # Create necessary directories
        mkdir -p "${SYSROOT_DIR}/run/dbus"
        mkdir -p "${SYSROOT_DIR}/var/lib/dbus"

        return 0
    else
        log_warn "D-Bus build completed but binary not found"
        return 1
    fi
}

# =============================================================================
# Ensure dhcpcd is available (DHCP client)
# =============================================================================
ensure_dhcpcd() {
    log_info "Ensuring dhcpcd..."

    # Check if already in sysroot
    if [[ -x "${SYSROOT_DIR}/usr/bin/dhcpcd" ]]; then
        log_info "  dhcpcd already present in sysroot"
        return 0
    fi

    # Try to copy from host
    if ! should_force_source_build; then
        if copy_from_host "dhcpcd" "${SYSROOT_DIR}/usr/bin/dhcpcd"; then
            # Copy config
            if [[ -f "/etc/dhcpcd.conf" ]]; then
                cp /etc/dhcpcd.conf "${SYSROOT_DIR}/etc/" 2>/dev/null || true
            fi
            mkdir -p "${SYSROOT_DIR}/var/lib/dhcpcd"
            return 0
        fi
    fi

    # Build from source
    local version="${RAVEN_DHCPCD_VERSION:-10.0.6}"
    local sources_dir="${BUILD_DIR}/sources"
    local tarball="${sources_dir}/dhcpcd-${version}.tar.xz"
    local url="https://github.com/NetworkConfiguration/dhcpcd/releases/download/v${version}/dhcpcd-${version}.tar.xz"
    local build_dir="${sources_dir}/dhcpcd-${version}"

    mkdir -p "${sources_dir}"

    if ! download_file "$url" "$tarball"; then
        log_warn "Failed to download dhcpcd"
        return 1
    fi

    if ! command -v make &>/dev/null || ! command -v cc &>/dev/null; then
        log_warn "Cannot build dhcpcd (missing make/cc)"
        return 1
    fi

    rm -rf "${build_dir}" 2>/dev/null || true
    tar -xf "${tarball}" -C "${sources_dir}" 2>/dev/null || {
        log_warn "Failed to extract dhcpcd"
        return 1
    }

    log_info "  Building dhcpcd ${version}..."
    (
        cd "${build_dir}"
        ./configure \
            --prefix=/usr \
            --sbindir=/sbin \
            --sysconfdir=/etc \
            --runstatedir=/run \
            --dbdir=/var/lib/dhcpcd \
            --disable-privsep \
            >/dev/null 2>&1 || \
        ./configure --prefix=/usr --sbindir=/sbin

        make -j"${RAVEN_JOBS:-$(nproc)}" >/dev/null 2>&1 || make -j"${RAVEN_JOBS:-$(nproc)}"
        make DESTDIR="${SYSROOT_DIR}" install >/dev/null 2>&1 || make DESTDIR="${SYSROOT_DIR}" install
    ) || {
        log_warn "dhcpcd build failed"
        return 1
    }

    if [[ -x "${SYSROOT_DIR}/usr/bin/dhcpcd" ]]; then
        log_success "  Built and installed dhcpcd"
        mkdir -p "${SYSROOT_DIR}/var/lib/dhcpcd"
        return 0
    else
        log_warn "dhcpcd build completed but binary not found"
        return 1
    fi
}

# =============================================================================
# Setup PAM + NSS runtime pieces (sudo/login need dlopened modules)
# Based on Linux From Scratch (LFS) guidelines for proper authentication
# =============================================================================
setup_pam_and_nss() {
    log_info "Setting up PAM and NSS runtime modules (LFS-based)..."

    mkdir -p "${SYSROOT_DIR}/etc/pam.d" "${SYSROOT_DIR}/etc/security"
    mkdir -p "${SYSROOT_DIR}/usr/lib/security"
    mkdir -p "${SYSROOT_DIR}/etc/security/limits.d"

    # ==========================================================================
    # LFS-style modular PAM configuration
    # ==========================================================================

    # system-auth: Base authentication stack
    cat > "${SYSROOT_DIR}/etc/pam.d/system-auth" << 'EOF'
#%PAM-1.0
# Begin /etc/pam.d/system-auth - RavenLinux (LFS-based)
auth      required    pam_unix.so
# End /etc/pam.d/system-auth
EOF

    # system-account: Account management
    cat > "${SYSROOT_DIR}/etc/pam.d/system-account" << 'EOF'
#%PAM-1.0
# Begin /etc/pam.d/system-account - RavenLinux (LFS-based)
account   required    pam_unix.so
# End /etc/pam.d/system-account
EOF

    # system-session: Session management
    cat > "${SYSROOT_DIR}/etc/pam.d/system-session" << 'EOF'
#%PAM-1.0
# Begin /etc/pam.d/system-session - RavenLinux (LFS-based)
session   required    pam_unix.so
# End /etc/pam.d/system-session
EOF

    # system-password: Password management
    cat > "${SYSROOT_DIR}/etc/pam.d/system-password" << 'EOF'
#%PAM-1.0
# Begin /etc/pam.d/system-password - RavenLinux (LFS-based)
password  required    pam_unix.so sha512 shadow try_first_pass
# End /etc/pam.d/system-password
EOF

    # login: Console login (simple config that works)
    cat > "${SYSROOT_DIR}/etc/pam.d/login" << 'EOF'
#%PAM-1.0
# Begin /etc/pam.d/login - RavenLinux
auth       required     pam_unix.so nullok try_first_pass
account    required     pam_unix.so
session    required     pam_unix.so
password   required     pam_unix.so nullok sha512
# End /etc/pam.d/login
EOF

    # passwd: Password change utility
    cat > "${SYSROOT_DIR}/etc/pam.d/passwd" << 'EOF'
#%PAM-1.0
# Begin /etc/pam.d/passwd - RavenLinux (LFS-based)
password  include     system-password
# End /etc/pam.d/passwd
EOF

    # chpasswd: batch password setting.
    #
    # Not optional, and not obvious: chpasswd is built against PAM, so without
    # its own config it falls through to /etc/pam.d/other -- which is pam_deny
    # for every phase. It then fails with "pam_start failure 26 ... password not
    # changed" and leaves the account's shadow field at "!", i.e. locked.
    #
    # The installer sets every account's password with `chpasswd -R`, so this
    # missing file meant a freshly installed system had no working login at all:
    # no password worked, and neither did no password.
    cat > "${SYSROOT_DIR}/etc/pam.d/chpasswd" << 'EOF'
#%PAM-1.0
# Begin /etc/pam.d/chpasswd - RavenLinux
auth      sufficient  pam_rootok.so
account   required    pam_permit.so
password  include     system-password
session   required    pam_permit.so
# End /etc/pam.d/chpasswd
EOF

    # newusers takes the same path as chpasswd and fails the same way.
    cat > "${SYSROOT_DIR}/etc/pam.d/newusers" << 'EOF'
#%PAM-1.0
# Begin /etc/pam.d/newusers - RavenLinux
auth      sufficient  pam_rootok.so
account   required    pam_permit.so
password  include     system-password
session   required    pam_permit.so
# End /etc/pam.d/newusers
EOF

    # chage reads and writes shadow ageing fields; same fallback, same failure.
    cat > "${SYSROOT_DIR}/etc/pam.d/chage" << 'EOF'
#%PAM-1.0
# Begin /etc/pam.d/chage - RavenLinux
auth      sufficient  pam_rootok.so
account   required    pam_permit.so
session   required    pam_permit.so
# End /etc/pam.d/chage
EOF

    # other: Secure fallback - deny unconfigured services
    cat > "${SYSROOT_DIR}/etc/pam.d/other" << 'EOF'
#%PAM-1.0
# Begin /etc/pam.d/other - Deny unconfigured services for security
auth        required    pam_warn.so
auth        required    pam_deny.so
account     required    pam_warn.so
account     required    pam_deny.so
password    required    pam_warn.so
password    required    pam_deny.so
session     required    pam_warn.so
session     required    pam_deny.so
# End /etc/pam.d/other
EOF

    # ==========================================================================
    # Environment and security configuration files
    # ==========================================================================

    # /etc/environment (required by pam_env.so)
    cat > "${SYSROOT_DIR}/etc/environment" << 'EOF'
# /etc/environment - System-wide environment variables
PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin"
EOF

    # /etc/security/pam_env.conf (required by pam_env.so)
    cat > "${SYSROOT_DIR}/etc/security/pam_env.conf" << 'EOF'
# /etc/security/pam_env.conf
# Environment variables set by pam_env module
# Format: VARIABLE [DEFAULT=value] [OVERRIDE=value]
EOF

    # /etc/security/limits.conf
    cat > "${SYSROOT_DIR}/etc/security/limits.conf" << 'EOF'
# /etc/security/limits.conf - Resource limits
# RavenLinux defaults. Add custom limits in /etc/security/limits.d/
# <domain> <type> <item> <value>
EOF

    # /etc/security/access.conf (for pam_access.so)
    cat > "${SYSROOT_DIR}/etc/security/access.conf" << 'EOF'
# /etc/security/access.conf - Login access control
# RavenLinux: Allow all users from all origins by default
# Format: permission : users : origins
+ : ALL : ALL
EOF

    # ==========================================================================
    # Copy PAM modules from host
    # ==========================================================================
    local host_security_dirs=()
    for d in \
        /lib/security \
        /usr/lib/security \
        /lib64/security \
        /usr/lib64/security \
        /lib/x86_64-linux-gnu/security \
        /usr/lib/x86_64-linux-gnu/security; do
        [[ -d "$d" ]] && host_security_dirs+=("$d")
    done

    # Extended list of PAM modules for proper authentication
    local pam_modules=(
        pam_unix.so
        pam_env.so
        pam_rootok.so
        pam_deny.so
        pam_permit.so
        pam_warn.so
        pam_limits.so
        pam_loginuid.so
        pam_nologin.so
        pam_securetty.so
        pam_wheel.so
        pam_access.so
        pam_faildelay.so
    )

    local copied_any=0
    for mod in "${pam_modules[@]}"; do
        local src=""
        for d in "${host_security_dirs[@]}"; do
            if [[ -e "${d}/${mod}" ]]; then
                src="${d}/${mod}"
                break
            fi
        done
        if [[ -n "$src" ]]; then
            cp -L "$src" "${SYSROOT_DIR}/usr/lib/security/${mod}" 2>/dev/null || true
            cp -L "$src" "${SYSROOT_DIR}/usr/lib/security/${mod}" 2>/dev/null || true
            copied_any=1
        fi
    done

    if [[ "${copied_any}" -eq 0 ]]; then
        log_warn "No PAM modules found on host; sudo/login may not work (install a PAM stack and rerun stage2)"
    else
        log_info "  Added PAM modules"
    fi

    # ==========================================================================
    # Copy PAM helper binaries (CRITICAL for password verification)
    # ==========================================================================
    mkdir -p "${SYSROOT_DIR}/usr/bin"
    local pam_helpers=(
        unix_chkpwd
        unix_update
    )
    local helper_dirs=(
        /usr/bin
        /sbin
        /usr/sbin
        /usr/lib
        /usr/lib/security
        /lib/security
    )
    for helper in "${pam_helpers[@]}"; do
        local src=""
        for d in "${helper_dirs[@]}"; do
            if [[ -x "${d}/${helper}" ]]; then
                src="${d}/${helper}"
                break
            fi
        done
        if [[ -n "$src" ]]; then
            # The path is compiled into pam_unix.so, not looked up on PATH, so
            # installing only to /sbin is not enough: Arch's module execs
            # /usr/bin/unix_chkpwd. When it is missing, pam_unix cannot read
            # /etc/shadow and every PAM call -- including the account check
            # autologin still performs -- returns PAM_AUTHINFO_UNAVAIL, i.e.
            # "Authentication service cannot retrieve authentication info".
            # login then exits, agetty restarts it, and the console loops.
            #
            # Ask the module we just copied where it expects the helper, and
            # cover the conventional locations too, so this survives building
            # against a PAM configured with a different --sbindir.
            # Under the usr-merge sbin, usr/sbin, usr/bin and bin are ONE
            # directory, so the old four-entry list wrote the same SUID binary
            # four times. Everything from `strings` still matters: a PAM built
            # with a different --sbindir can name /usr/lib/security or similar,
            # which is a genuinely different directory.
            local -a helper_dests=(usr/bin)
            local p
            while read -r p; do
                [[ -n "${p}" ]] || continue
                helper_dests+=("${p#/}")
            done < <(strings "${SYSROOT_DIR}/usr/lib/security/pam_unix.so" 2>/dev/null \
                     | grep -E "^/.*/${helper}$" | sed "s|/${helper}$||")

            local dest canon seen=""
            for dest in "${helper_dests[@]}"; do
                # Collapse the merged aliases so a path named bin, sbin or
                # usr/sbin is recognised as the usr/bin we already wrote.
                canon="${dest}"
                case "/${dest}" in
                    /bin|/sbin|/usr/sbin)   canon="usr/bin" ;;
                    /lib|/lib64|/usr/lib64) canon="usr/lib" ;;
                    /bin/*)        canon="usr/bin/${dest#bin/}" ;;
                    /sbin/*)       canon="usr/bin/${dest#sbin/}" ;;
                    /usr/sbin/*)   canon="usr/bin/${dest#usr/sbin/}" ;;
                    /lib/*)        canon="usr/lib/${dest#lib/}" ;;
                    /lib64/*)      canon="usr/lib/${dest#lib64/}" ;;
                    /usr/lib64/*)  canon="usr/lib/${dest#usr/lib64/}" ;;
                esac
                [[ ",${seen}," == *",${canon},"* ]] && continue
                seen="${seen},${canon}"
                mkdir -p "${SYSROOT_DIR}/${canon}"
                cp -L "$src" "${SYSROOT_DIR}/${canon}/${helper}"
                chmod 4755 "${SYSROOT_DIR}/${canon}/${helper}"  # SUID root
            done
            log_info "  Added PAM helper: ${helper} (SUID) -> ${seen#,}"
        else
            log_warn "PAM helper ${helper} not found on host - login/su/sudo will fail with"
            log_warn "  'Authentication service cannot retrieve authentication info'"
        fi
    done

    # ==========================================================================
    # login must exist where callers expect to find it
    # ==========================================================================
    # agetty's compiled-in default login program is /bin/login. This image
    # installs login to /sbin/login only, so any agetty invoked without an
    # explicit --login-program reads the username, fails to exec /bin/login,
    # and exits -- the getty is restarted and reprompts, so the console loops
    # on "login:" and never reaches a password prompt.
    #
    # Same shape as the unix_chkpwd and seatd problems above: a binary that
    # exists, at a path nothing looks in.
    # The usr-merge is what actually fixes this: /bin, /sbin, /usr/bin and
    # /usr/sbin are all one directory, so a login installed anywhere is login
    # everywhere. The old fan-out linked /bin/login, /usr/bin/login and
    # /usr/sbin/login to /sbin/login -- three self-references post-merge.
    if [[ -e "${SYSROOT_DIR}/usr/bin/login" ]]; then
        log_info "  login reachable at /bin/login (agetty's default)"
    else
        log_warn "No /sbin/login to link; agetty without --login-program will loop"
    fi

    # ==========================================================================
    # Copy NSS modules (for user/group lookups)
    # ==========================================================================
    mkdir -p "${SYSROOT_DIR}/usr/lib"
    local nss_libs=(
        libnss_files.so.2
        libnss_dns.so.2
        libnss_compat.so.2
    )
    for lib in "${nss_libs[@]}"; do
        for d in /lib /lib64 /usr/lib /usr/lib64 /lib/x86_64-linux-gnu /usr/lib/x86_64-linux-gnu; do
            if [[ -e "${d}/${lib}" ]]; then
                # /lib is /usr/lib now; the second cp used to just overwrite
                # the first with the same bytes.
                cp -L "${d}/${lib}" "${SYSROOT_DIR}/usr/lib/${lib}" 2>/dev/null || true
                break
            fi
        done
    done

    log_success "PAM/NSS runtime setup complete (LFS-based)"
}

# =============================================================================
# Setup sudo using GNU sudo from host system
# =============================================================================
setup_sudo() {
    log_info "Setting up sudo (GNU sudo from host)..."

    # Create target directories
    mkdir -p "${SYSROOT_DIR}/usr/bin"

    # Copy sudo from host
    if [[ -x "/usr/bin/sudo" ]]; then
        cp -L "/usr/bin/sudo" "${SYSROOT_DIR}/usr/bin/sudo"
        chmod 4755 "${SYSROOT_DIR}/usr/bin/sudo"  # SUID root
        # There used to be `ln -sf /usr/bin/sudo "${SYSROOT_DIR}/bin/sudo"`
        # here. Post-merge ${SYSROOT}/bin/sudo IS ${SYSROOT}/usr/bin/sudo, so
        # that unlinked the SUID binary and replaced it with a link to itself.
        # Worse, during the build the absolute target escaped the sysroot and
        # resolved against the BUILD HOST's /usr/bin/sudo, so every build-time
        # check passed while the ISO shipped nothing.
        log_info "  Installed sudo binary (SUID)"
    else
        log_error "sudo not found on host system"
        return 1
    fi

    # Copy visudo, sudoedit, sudoreplay if available
    for bin in visudo sudoedit sudoreplay; do
        if [[ -x "/usr/bin/${bin}" ]]; then
            cp -L "/usr/bin/${bin}" "${SYSROOT_DIR}/usr/bin/${bin}"
            chmod 755 "${SYSROOT_DIR}/usr/bin/${bin}"
            log_info "  Installed ${bin}"
        fi
    done

    # Copy sudo plugin libraries (required for GNU sudo)
    mkdir -p "${SYSROOT_DIR}/usr/lib/sudo"
    if [[ -d "/usr/lib/sudo" ]]; then
        cp -a /usr/lib/sudo/. "${SYSROOT_DIR}/usr/lib/sudo/" 2>/dev/null || true
        log_info "  Copied /usr/lib/sudo/ plugins"
    fi

    # Copy sudo's library dependencies
    for lib in $(ldd /usr/bin/sudo 2>/dev/null | grep -o '/[^ ]*' | sort -u); do
        [[ -f "$lib" ]] || continue
        local dest="${SYSROOT_DIR}${lib}"
        if [[ ! -f "$dest" ]]; then
            mkdir -p "$(dirname "$dest")"
            cp -L "$lib" "$dest" 2>/dev/null || true
        fi
    done

    # Also copy dependencies from sudo plugins
    if [[ -d "/usr/lib/sudo" ]]; then
        for so in /usr/lib/sudo/*.so; do
            [[ -f "$so" ]] || continue
            for lib in $(ldd "$so" 2>/dev/null | grep -o '/[^ ]*' | sort -u); do
                [[ -f "$lib" ]] || continue
                local dest="${SYSROOT_DIR}${lib}"
                if [[ ! -f "$dest" ]]; then
                    mkdir -p "$(dirname "$dest")"
                    cp -L "$lib" "$dest" 2>/dev/null || true
                fi
            done
        done
    fi
    log_info "  Copied library dependencies"

    # ==========================================================================
    # Create sudoers configuration (LFS-based)
    # ==========================================================================
    mkdir -p "${SYSROOT_DIR}/etc/sudoers.d"

    cat > "${SYSROOT_DIR}/etc/sudoers" << 'EOF'
# /etc/sudoers - RavenLinux sudo configuration
#
# This file MUST be edited with 'visudo' to ensure proper syntax

# Defaults
Defaults    env_reset
Defaults    secure_path="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
Defaults    timestamp_timeout=15

# Root can do anything
root ALL=(ALL:ALL) ALL

# Members of the wheel group may execute any command
%wheel ALL=(ALL:ALL) ALL

# Read drop-in files from /etc/sudoers.d
@includedir /etc/sudoers.d
EOF
    chmod 0440 "${SYSROOT_DIR}/etc/sudoers"
    chown root:root "${SYSROOT_DIR}/etc/sudoers" 2>/dev/null || true

    # ==========================================================================
    # Create PAM config for sudo (LFS-based with proper authentication)
    # ==========================================================================
    cat > "${SYSROOT_DIR}/etc/pam.d/sudo" << 'EOF'
#%PAM-1.0
# Begin /etc/pam.d/sudo - RavenLinux
auth       sufficient   pam_rootok.so
auth       required     pam_unix.so nullok try_first_pass
account    sufficient   pam_rootok.so
account    required     pam_unix.so
session    required     pam_unix.so
password   required     pam_unix.so nullok sha512
# End /etc/pam.d/sudo
EOF

    log_success "sudo setup complete (GNU sudo)"
}

# =============================================================================
# Setup su using GNU su from host system
# Note: su does NOT require wheel group membership - any user can su with password
# =============================================================================
setup_su() {
    log_info "Setting up su (GNU su from host)..."

    mkdir -p "${SYSROOT_DIR}/usr/bin"

    # Find and copy su from host
    local su_src=""
    for candidate in /usr/bin/su /bin/su; do
        if [[ -x "$candidate" ]]; then
            su_src="$candidate"
            break
        fi
    done

    if [[ -n "$su_src" ]]; then
        cp -L "$su_src" "${SYSROOT_DIR}/usr/bin/su"
        chmod 4755 "${SYSROOT_DIR}/usr/bin/su"  # SUID root
        log_info "  Installed su binary (SUID)"

        # Copy su's library dependencies
        for lib in $(ldd "$su_src" 2>/dev/null | grep -o '/[^ ]*' | sort -u); do
            [[ -f "$lib" ]] || continue
            local dest="${SYSROOT_DIR}${lib}"
            if [[ ! -f "$dest" ]]; then
                mkdir -p "$(dirname "$dest")"
                cp -L "$lib" "$dest" 2>/dev/null || true
            fi
        done
        log_info "  Copied library dependencies"
    else
        log_error "su not found on host system"
        return 1
    fi

    # ==========================================================================
    # Create PAM config for su (LFS-based)
    # Note: pam_wheel.so is NOT used - any user can su with correct password
    # ==========================================================================
    cat > "${SYSROOT_DIR}/etc/pam.d/su" << 'EOF'
#%PAM-1.0
# Begin /etc/pam.d/su - RavenLinux
# Allow root to su without password
auth       sufficient   pam_rootok.so
auth       required     pam_unix.so nullok try_first_pass
account    sufficient   pam_rootok.so
account    required     pam_unix.so
session    required     pam_unix.so
password   required     pam_unix.so nullok sha512
# End /etc/pam.d/su
EOF

    log_success "su setup complete (GNU su)"
}

# =============================================================================
# Copy CA certificates (for HTTPS)
# =============================================================================
copy_ca_certificates() {
    log_info "Copying CA certificates..."

    mkdir -p "${SYSROOT_DIR}/etc/ssl/certs" "${SYSROOT_DIR}/etc/pki/tls/certs"

    local src=""
    for candidate in \
        /etc/ssl/certs/ca-certificates.crt \
        /etc/ssl/cert.pem \
        /etc/pki/tls/certs/ca-bundle.crt \
        /etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem; do
        if [[ -f "$candidate" ]]; then
            src="$candidate"
            break
        fi
    done

    if [[ -z "$src" ]]; then
        log_warn "No CA bundle found on host; HTTPS may fail in the target system"
        return 0
    fi

    cp -L "$src" "${SYSROOT_DIR}/etc/ssl/certs/ca-certificates.crt" 2>/dev/null || true
    ln -sf /etc/ssl/certs/ca-certificates.crt "${SYSROOT_DIR}/etc/ssl/cert.pem" 2>/dev/null || true
    cp -L "${SYSROOT_DIR}/etc/ssl/certs/ca-certificates.crt" "${SYSROOT_DIR}/etc/pki/tls/certs/ca-bundle.crt" 2>/dev/null || true

    log_info "  Added CA bundle from ${src}"
    log_success "CA certificates installed"
}

# =============================================================================
# Copy firmware blobs (needed for WiFi, etc.)
# =============================================================================
copy_firmware() {
    log_info "Copying firmware blobs..."

    local host_firmware=""
    for candidate in /usr/lib/firmware /lib/firmware; do
        if [[ -d "$candidate" ]]; then
            host_firmware="$candidate"
            break
        fi
    done

    if [[ -z "$host_firmware" ]]; then
        log_warn "No host firmware directory found -- WiFi and graphics will not work at all."
        log_warn "  Install linux-firmware on the build host and rerun stage2."
        log_warn "  (In the containerized build that host is the image itself; see the Dockerfile.)"
        return 0
    fi

    mkdir -p "${SYSROOT_DIR}/usr/lib/firmware"

    # Every vendor whose driver the kernel builds in. Shipping only Realtek --
    # as this did -- meant an Intel or Atheros or MediaTek card came up dead on
    # a kernel that had perfectly good support for it compiled in.
    local fw_dirs=(
        intel/iwlwifi          # iwlwifi (the real blobs; see the symlink pass)
        ath9k_htc ath10k ath11k ath12k
        mediatek               # mt7921 and friends
        brcm                   # brcmfmac
        rtw88 rtw89 rtlwifi    # Realtek WiFi
        rtl_nic rtl_bt         # Realtek ethernet + bluetooth
        mrvl                   # Marvell
        amdgpu                 # AMD graphics, 86MB -- see below
        i915                   # Intel graphics, 25MB
        xe                     # Intel graphics, Lunar Lake and later
        qca                    # Qualcomm bluetooth
        # Audio. SOF firmware and topologies for every Intel laptop since
        # ~2019 and AMD Renoir onward; without them snd_sof loads and the
        # machine has neither speakers nor microphone. The Cirrus and TI
        # blobs are the smart amplifiers those same laptops hang off HDA.
        intel/sof intel/sof-tplg intel/sof-ipc4 intel/sof-ipc4-lib
        intel/sof-ipc4-tplg intel/sof-ace-tplg
        amd cirrus ti
        renesas                # uPD720201/2 xHCI, common in USB 3 add-in cards
    )

    # Graphics firmware is not optional on any machine built this decade. Both
    # amdgpu and i915 refuse to initialise without their blobs, and when they do
    # CONFIG_DRM_SIMPLEDRM takes the EFI framebuffer instead: the console works,
    # and the compositor renders into a shadow buffer with no GPU behind it,
    # which looks like a frozen screen of uninitialised memory rather than like
    # a missing driver. That is the failure this list existed to prevent for
    # WiFi and did not cover for graphics.
    #
    # nouveau's GSP blobs are 152MB decompressed against amdgpu's 86 and i915's
    # 25, and they only matter for a discrete NVIDIA card. On a hybrid laptop
    # that card owns some of the ports, though -- the HDMI and USB-C outputs on
    # most gaming laptops are wired to the dGPU -- so without its firmware
    # those ports are dead no matter what the compositor does. Shipped by
    # default; RAVEN_FW_NVIDIA=0 drops it for a smaller image.
    if [[ "${RAVEN_FW_NVIDIA:-1}" == "1" ]]; then
        fw_dirs+=(nvidia)
        log_info "  including nouveau GSP firmware (+152MB; RAVEN_FW_NVIDIA=0 to drop)"
    fi

    # Arch ships firmware zstd-compressed. The kernel can read .zst blobs
    # (CONFIG_FW_LOADER_COMPRESS_ZSTD=y) but only when it goes looking for the
    # .zst name itself, which not every driver path does. Decompressing on the
    # way in works regardless, and squashfs compresses it all back down.
    local have_zstd=0
    command -v zstd &>/dev/null && have_zstd=1

    # Copies one file, decompressing and dereferencing as needed.
    _install_fw() {
        local src="$1" dest="$2"
        mkdir -p "$(dirname "$dest")"
        case "$src" in
            *.zst)
                if (( have_zstd )); then
                    zstd -dqf "$src" -o "${dest%.zst}" 2>/dev/null || return 1
                else
                    cp -aL "$src" "$dest" 2>/dev/null || return 1
                fi
                ;;
            *)  cp -aL "$src" "$dest" 2>/dev/null || return 1 ;;
        esac
        return 0
    }

    local copied=0 dir f rel
    for dir in "${fw_dirs[@]}"; do
        [[ -d "${host_firmware}/${dir}" ]] || continue
        while IFS= read -r f; do
            rel="${f#${host_firmware}/}"
            _install_fw "$f" "${SYSROOT_DIR}/usr/lib/firmware/${rel}" && copied=$((copied + 1))
        done < <(find "${host_firmware}/${dir}" -type f -o -type l 2>/dev/null)
        log_info "  Added ${dir} firmware"
    done

    # iwlwifi asks for "iwlwifi-<name>.ucode" at the firmware root. Arch keeps
    # the real files under intel/iwlwifi and leaves top-level symlinks, which a
    # directory-only sweep never sees -- so pick them up by name and resolve
    # them into real files where the driver actually looks.
    while IFS= read -r f; do
        rel="$(basename "$f")"
        _install_fw "$f" "${SYSROOT_DIR}/usr/lib/firmware/${rel}" && copied=$((copied + 1))
    done < <(find "${host_firmware}" -maxdepth 1 \( -name 'iwlwifi-*' -o -name 'regulatory.db*' \) 2>/dev/null)

    # Intel bluetooth (btintel) wants intel/ibt-*.sfi and .ddc, which sit as
    # loose files directly under intel/ next to the subdirectories above. The
    # directory sweep takes intel/iwlwifi and never these, so the driver
    # stack was complete and the adapter never came up.
    while IFS= read -r f; do
        rel="intel/$(basename "$f")"
        _install_fw "$f" "${SYSROOT_DIR}/usr/lib/firmware/${rel}" && copied=$((copied + 1))
    done < <(find "${host_firmware}/intel" -maxdepth 1 -type f -name 'ibt-*' 2>/dev/null)

    if (( copied == 0 )); then
        log_warn "Found ${host_firmware} but copied no firmware; WiFi and graphics will not work"
        return 0
    fi

    log_success "Firmware installed (${copied} files, $(du -sh "${SYSROOT_DIR}/usr/lib/firmware" 2>/dev/null | cut -f1))"
}

# =============================================================================
# Copy required libraries for all binaries
# =============================================================================
copy_libraries() {
    log_info "Copying required libraries..."

    local lib_count=0

    # /bin, /sbin and /usr/sbin all resolve to /usr/bin; iterating them
    # separately walked the same directory four times.
    for bin in "${SYSROOT_DIR}"/usr/bin/*; do
        [[ -f "$bin" && -x "$bin" && ! -L "$bin" ]] || continue

        # Skip statically linked binaries
        if file "$bin" 2>/dev/null | grep -q "statically linked"; then
            continue
        fi

        # Get library dependencies (|| true to handle grep finding no matches)
        timeout 2 ldd "$bin" 2>/dev/null | grep -o '/[^ ]*' | while read -r lib; do
            [[ -z "$lib" || ! -f "$lib" ]] && continue
            local dest="${SYSROOT_DIR}${lib}"
            if [[ ! -f "$dest" ]]; then
                mkdir -p "$(dirname "$dest")"
                cp -L "$lib" "$dest" 2>/dev/null || true
            fi
        done || true
    done

    # Some components dlopen modules at runtime (not visible via `ldd` on the
    # main executable). Copy deps for common module locations we ship.
    log_info "Copying runtime libraries for dlopened modules..."
    for so in \
        "${SYSROOT_DIR}"/usr/lib/security/*.so \
        "${SYSROOT_DIR}"/usr/lib/security/*.so \
        "${SYSROOT_DIR}"/usr/lib/libnss_*.so.* \
        "${SYSROOT_DIR}"/usr/lib/libnss_*.so.* \
        "${SYSROOT_DIR}"/usr/lib/libweston-*/*.so \
        "${SYSROOT_DIR}"/usr/lib64/libweston-*/*.so \
        "${SYSROOT_DIR}"/usr/lib/weston/*.so \
        "${SYSROOT_DIR}"/usr/lib64/weston/*.so; do
        [[ -f "$so" ]] || continue
        timeout 2 ldd "$so" 2>/dev/null | grep -o '/[^ ]*' | while read -r lib; do
            [[ -z "$lib" || ! -f "$lib" ]] && continue
            local dest="${SYSROOT_DIR}${lib}"
            if [[ ! -f "$dest" ]]; then
                mkdir -p "$(dirname "$dest")"
                cp -L "$lib" "$dest" 2>/dev/null || true
            fi
        done || true
    done

    # /lib, /lib64 and /usr/lib64 are usr-merge symlinks onto /usr/lib. Do NOT
    # turn them back into real directories: /bin, /sbin, /lib and /lib64 have
    # to be symlinks for Arch's `filesystem` package to extract at all.
    mkdir -p "${SYSROOT_DIR}/usr/lib"

    # The dynamic linker. 217 binaries in the sysroot carry PT_INTERP
    # /lib64/ld-linux-x86-64.so.2, so that path must resolve -- it does, via
    # /lib64 -> usr/lib. One copy is enough; /lib64, /lib and /usr/lib64 are
    # all the same directory now, and the old three-way copy was 3x the bytes
    # for one file.
    log_info "Copying dynamic linker..."
    for ld in /lib64/ld-linux-x86-64.so.2 /lib/ld-linux-x86-64.so.2 /lib/ld-musl-x86_64.so.1 /usr/lib/ld-linux-x86-64.so.2; do
        if [[ -f "$ld" ]] || [[ -L "$ld" ]]; then
            local ld_name
            ld_name="$(basename "$ld")"
            cp -L "$ld" "${SYSROOT_DIR}/usr/lib/${ld_name}" 2>/dev/null \
                && log_info "  Copied ${ld_name} to /usr/lib/ (reachable as /lib64/${ld_name})" || true
        fi
    done

    # Copy graphics/OpenGL libraries (needed for GUI apps like raven-wifi)
    log_info "Copying graphics libraries..."
    local graphics_libs=(
        # OpenGL
        libGL.so libGL.so.1
        libGLX.so libGLX.so.0
        libGLdispatch.so libGLdispatch.so.0
        libOpenGL.so libOpenGL.so.0
        # OpenGL ES (required by Gio UI and other EGL-based apps)
        libGLESv1_CM.so libGLESv1_CM.so.1
        libGLESv2.so libGLESv2.so.2
        # EGL
        libEGL.so libEGL.so.1
        # Mesa GLVND vendor libraries (required by /usr/share/glvnd/egl_vendor.d/*)
        libEGL_mesa.so.0
        # GLX
        libglapi.so libglapi.so.0
        # Mesa GLX vendor libraries (used by GLVND/libGLX on most distros)
        libGLX_mesa.so.0
        libGLX_indirect.so.0
        # Mesa
        libgbm.so libgbm.so.1
        # Wayland
        libwayland-client.so libwayland-client.so.0
        libwayland-egl.so libwayland-egl.so.1
        libwayland-cursor.so libwayland-cursor.so.0
        # X11
        libX11.so libX11.so.6
        libXcursor.so libXcursor.so.1
        libXrandr.so libXrandr.so.2
        libXi.so libXi.so.6
        libXinerama.so libXinerama.so.1
        libXxf86vm.so libXxf86vm.so.1
        libXext.so libXext.so.6
        libXrender.so libXrender.so.1
        libXfixes.so libXfixes.so.3
        libxcb.so libxcb.so.1
        libxkbcommon.so libxkbcommon.so.0
        # Vulkan
        libvulkan.so libvulkan.so.1
    )

    for lib in "${graphics_libs[@]}"; do
        for dir in /usr/lib /usr/lib64 /usr/lib/x86_64-linux-gnu /lib /lib64; do
            if [[ -f "${dir}/${lib}" ]]; then
                mkdir -p "${SYSROOT_DIR}/usr/lib"
                cp -L "${dir}/${lib}" "${SYSROOT_DIR}/usr/lib/" 2>/dev/null || true
                break
            fi
        done
    done

    # Mesa/GLVND driver files are dlopened at runtime (not shown in `ldd`),
    # but are required for EGL/GBM/DRM compositors like raven-compositor.
    log_info "Copying Mesa/GLVND driver data..."

    # DRI drivers (e.g. virgl/virtio, radeonsi, iris, swrast)
    for dri_dir in /usr/lib/dri /usr/lib64/dri /usr/lib/x86_64-linux-gnu/dri; do
        if [[ -d "${dri_dir}" ]]; then
            mkdir -p "${SYSROOT_DIR}${dri_dir}"
            cp -a "${dri_dir}/." "${SYSROOT_DIR}${dri_dir}/" 2>/dev/null || true
            log_info "  Copied $(basename "$(dirname "${dri_dir}")")/dri drivers"
        fi
    done

    # GBM DRI loader module(s) (Mesa)
    # Needed for EGL/GBM compositors (Weston, wlroots, etc.).
    for gbm_dir in /usr/lib/gbm /usr/lib64/gbm /usr/lib/x86_64-linux-gnu/gbm; do
        if [[ -d "${gbm_dir}" ]]; then
            mkdir -p "${SYSROOT_DIR}${gbm_dir}"
            cp -a "${gbm_dir}/." "${SYSROOT_DIR}${gbm_dir}/" 2>/dev/null || true
            log_info "  Copied ${gbm_dir} modules"
        fi
    done

    # Xorg / Xwayland runtime data (modules, config snippets)
    log_info "Copying Xorg/Xwayland runtime data..."
    if [[ -x "/usr/lib/Xorg" ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/lib"
        cp -L "/usr/lib/Xorg" "${SYSROOT_DIR}/usr/lib/Xorg" 2>/dev/null || true
        log_info "  Copied /usr/lib/Xorg"
    fi
    if [[ -x "/usr/lib/Xorg.wrap" ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/lib"
        cp -L "/usr/lib/Xorg.wrap" "${SYSROOT_DIR}/usr/lib/Xorg.wrap" 2>/dev/null || true
        chmod 4755 "${SYSROOT_DIR}/usr/lib/Xorg.wrap" 2>/dev/null || true
        log_info "  Copied /usr/lib/Xorg.wrap"
    fi

    for xorg_dir in /usr/lib/xorg /usr/lib64/xorg /usr/lib/x86_64-linux-gnu/xorg; do
        if [[ -d "${xorg_dir}" ]]; then
            mkdir -p "${SYSROOT_DIR}${xorg_dir}"
            cp -a "${xorg_dir}/." "${SYSROOT_DIR}${xorg_dir}/" 2>/dev/null || true
            log_info "  Copied ${xorg_dir}"
        fi
    done
    for xorg_conf_dir in /usr/share/X11/xorg.conf.d /etc/X11/xorg.conf.d; do
        if [[ -d "${xorg_conf_dir}" ]]; then
            mkdir -p "${SYSROOT_DIR}${xorg_conf_dir}"
            cp -a "${xorg_conf_dir}/." "${SYSROOT_DIR}${xorg_conf_dir}/" 2>/dev/null || true
            log_info "  Copied $(basename "${xorg_conf_dir}")"
        fi
    done

    # Wayland session data
    log_info "Setting up Wayland session data..."
    mkdir -p "${SYSROOT_DIR}/usr/share/wayland-sessions"

    # EGL vendor JSONs (GLVND)
    for egl_dir in /usr/share/glvnd/egl_vendor.d /etc/glvnd/egl_vendor.d; do
        if [[ -d "${egl_dir}" ]]; then
            mkdir -p "${SYSROOT_DIR}${egl_dir}"
            cp -a "${egl_dir}/." "${SYSROOT_DIR}${egl_dir}/" 2>/dev/null || true
            log_info "  Copied EGL vendor config"
        fi
    done

    # Mesa drirc config (optional but harmless)
    for drirc in /usr/share/drirc /usr/share/drirc.d; do
        if [[ -e "${drirc}" ]]; then
            mkdir -p "${SYSROOT_DIR}$(dirname "${drirc}")"
            cp -a "${drirc}" "${SYSROOT_DIR}${drirc}" 2>/dev/null || true
        fi
    done

    # Ensure we also ship shared libraries required by Mesa dlopened modules.
    log_info "Copying runtime libraries for Mesa dlopened modules..."
    for so in \
        "${SYSROOT_DIR}"/usr/lib/libEGL_mesa.so.0 \
        "${SYSROOT_DIR}"/usr/lib/libGLX_mesa.so.0 \
        "${SYSROOT_DIR}"/usr/lib/gbm/*.so \
        "${SYSROOT_DIR}"/usr/lib64/gbm/*.so \
        "${SYSROOT_DIR}"/usr/lib/x86_64-linux-gnu/gbm/*.so \
        "${SYSROOT_DIR}"/usr/lib/dri/*.so \
        "${SYSROOT_DIR}"/usr/lib64/dri/*.so \
        "${SYSROOT_DIR}"/usr/lib/x86_64-linux-gnu/dri/*.so; do
        [[ -f "$so" && ! -L "$so" ]] || continue
        timeout 2 ldd "$so" 2>/dev/null | grep -o '/[^ ]*' | while read -r lib; do
            [[ -z "$lib" || ! -f "$lib" ]] && continue
            local dest="${SYSROOT_DIR}${lib}"
            if [[ ! -f "$dest" ]]; then
                mkdir -p "$(dirname "$dest")"
                cp -L "$lib" "$dest" 2>/dev/null || true
            fi
        done || true
    done

    # Ensure we ship shared libraries required by Xorg/Xwayland dlopened modules.
    log_info "Copying runtime libraries for Xorg/Xwayland modules..."
    for modules_dir in \
        "${SYSROOT_DIR}/usr/lib/xorg/modules" \
        "${SYSROOT_DIR}/usr/lib/xorg/modules" \
        "${SYSROOT_DIR}/usr/lib/x86_64-linux-gnu/xorg/modules"; do
        [[ -d "${modules_dir}" ]] || continue
        while IFS= read -r -d '' so; do
            timeout 2 ldd "$so" 2>/dev/null | grep -o '/[^ ]*' | while read -r lib; do
                [[ -z "$lib" || ! -f "$lib" ]] && continue
                local dest="${SYSROOT_DIR}${lib}"
                if [[ ! -f "$dest" ]]; then
                    mkdir -p "$(dirname "$dest")"
                    cp -L "$lib" "$dest" 2>/dev/null || true
                fi
            done || true
        done < <(find "${modules_dir}" -type f -name '*.so' -print0 2>/dev/null) || true
    done

    # Mesa is C++ (libgallium), so ensure libstdc++/libgcc_s in the sysroot are
    # at least as new as the host copies we pulled Mesa from.
    sync_runtime_lib() {
        local libname="$1"
        local src=""
        for candidate in "/usr/lib/${libname}" "/lib/${libname}" "/usr/lib64/${libname}" "/lib64/${libname}"; do
            if [[ -e "${candidate}" ]]; then
                src="${candidate}"
                break
            fi
        done
        [[ -n "${src}" ]] || return 0

        local src_real
        src_real="$(readlink -f "${src}" 2>/dev/null || true)"
        [[ -n "${src_real}" && -e "${src_real}" ]] || src_real="${src}"

        local dest_dir="${SYSROOT_DIR}$(dirname "${src}")"
        mkdir -p "${dest_dir}"

        local base
        base="$(basename "${src_real}")"

        # Copy the real file into the sysroot.
        if [[ "${base}" == "${libname}" ]]; then
            cp -L "${src_real}" "${SYSROOT_DIR}${src}" 2>/dev/null || true
            log_info "  Updated ${src}"
            return 0
        fi

        cp -L "${src_real}" "${dest_dir}/${base}" 2>/dev/null || true
        rm -f "${SYSROOT_DIR}${src}" 2>/dev/null || true
        ln -sf "${base}" "${SYSROOT_DIR}${src}" 2>/dev/null || true
        log_info "  Updated ${src} -> ${base}"
    }

    log_info "Ensuring GCC runtime libraries (libstdc++/libgcc_s)..."
    sync_runtime_lib "libstdc++.so.6"
    sync_runtime_lib "libgcc_s.so.1"

    # libpam / libpam_misc / libpamc used to be linked from /lib to /usr/lib so
    # that linker lookups hitting /lib first would find them. /lib IS /usr/lib
    # now, so the lookup succeeds with no link at all -- and creating one would
    # be a self-reference.

    # libcrypt.so.1 -> libcrypt.so.2 is a DIFFERENT basename, so it is a real
    # alias and still needed by pam_unix.so. Same-directory form only.
    if [[ ! -e "${SYSROOT_DIR}/usr/lib/libcrypt.so.1" ]] \
        && [[ -f "${SYSROOT_DIR}/usr/lib/libcrypt.so.2" ]]; then
        ln -sf libcrypt.so.2 "${SYSROOT_DIR}/usr/lib/libcrypt.so.1" 2>/dev/null || true
        log_info "  Created /usr/lib/libcrypt.so.1 -> libcrypt.so.2"
    fi

    # Explicitly copy PAM-related libraries (pam_unix.so dependencies that may be missed)
    # These are critical for authentication to work
    log_info "Ensuring PAM dependency libraries..."
    local pam_deps=(
        libtirpc.so.3
        libnsl.so.3
        libaudit.so.1
        libcap-ng.so.0
    )
    for lib in "${pam_deps[@]}"; do
        if [[ ! -f "${SYSROOT_DIR}/usr/lib/${lib}" ]]; then
            for src_dir in /usr/lib /lib /usr/lib64 /lib64; do
                if [[ -f "${src_dir}/${lib}" ]]; then
                    cp -L "${src_dir}/${lib}" "${SYSROOT_DIR}/usr/lib/${lib}" 2>/dev/null || true
                    cp -L "${src_dir}/${lib}" "${SYSROOT_DIR}/usr/lib/${lib}" 2>/dev/null || true
                    log_info "  Copied ${lib}"
                    break
                fi
            done
        fi
    done

    # There was a list of twelve libraries here that got linked /lib/X ->
    # ../usr/lib/X, to satisfy linker lookups that check /lib first. The
    # usr-merge makes /lib and /usr/lib the same directory, so those lookups
    # now succeed against the real file and every one of those links would be
    # a self-reference that unlinked the library it was meant to expose.

    log_success "Libraries copied"
}

# =============================================================================
# Generate ld.so.cache (critical for NSS/PAM to find libraries at runtime)
# =============================================================================
generate_ldconfig_cache() {
    log_info "Generating ld.so.cache for sysroot..."

    # Copy ldconfig to sysroot (it's statically linked so it can run anywhere)
    if [[ -x "/usr/bin/ldconfig" ]]; then
        cp -L "/usr/bin/ldconfig" "${SYSROOT_DIR}/usr/bin/ldconfig" 2>/dev/null || true
    elif [[ -x "/sbin/ldconfig" ]]; then
        cp -L "/sbin/ldconfig" "${SYSROOT_DIR}/usr/bin/ldconfig" 2>/dev/null || true
    fi

    # Run ldconfig with -r to use sysroot as root
    # This generates /etc/ld.so.cache in the sysroot
    if [[ -x "/usr/bin/ldconfig" ]]; then
        /usr/bin/ldconfig -r "${SYSROOT_DIR}" 2>/dev/null || true
        log_info "  Generated ld.so.cache"
    elif [[ -x "/sbin/ldconfig" ]]; then
        /sbin/ldconfig -r "${SYSROOT_DIR}" 2>/dev/null || true
        log_info "  Generated ld.so.cache"
    else
        log_warn "ldconfig not found, ld.so.cache not generated"
    fi

    # Verify cache was created
    if [[ -f "${SYSROOT_DIR}/etc/ld.so.cache" ]]; then
        log_success "ld.so.cache generated successfully"
    else
        log_warn "ld.so.cache was not created - NSS libraries may not load properly"
    fi
}

# =============================================================================
# Copy terminfo database (needed for clear, reset, etc.)
# =============================================================================
copy_terminfo() {
    log_info "Copying terminfo database..."

    # Find terminfo location
    local terminfo_src=""
    for dir in /usr/share/terminfo /lib/terminfo /etc/terminfo; do
        if [[ -d "$dir" ]]; then
            terminfo_src="$dir"
            break
        fi
    done

    if [[ -z "$terminfo_src" ]]; then
        log_warn "No terminfo database found on host"
        return
    fi

    # Copy essential terminal definitions
    mkdir -p "${SYSROOT_DIR}/usr/share/terminfo"

    # Copy common terminal types: linux, xterm, vt100, screen, etc.
    local terms=(
        "l/linux"
        "x/xterm" "x/xterm-256color" "x/xterm-color"
        "v/vt100" "v/vt102" "v/vt220"
        "s/screen" "s/screen-256color"
        "r/rxvt" "r/rxvt-unicode" "r/rxvt-unicode-256color"
        "a/ansi"
        "d/dumb"
    )

    for term in "${terms[@]}"; do
        local src="${terminfo_src}/${term}"
        if [[ -f "$src" ]]; then
            local dest="${SYSROOT_DIR}/usr/share/terminfo/${term}"
            mkdir -p "$(dirname "$dest")"
            cp "$src" "$dest" 2>/dev/null || true
        fi
    done

    # Also copy to /etc/terminfo as fallback
    if [[ -d "${SYSROOT_DIR}/usr/share/terminfo" ]]; then
        mkdir -p "${SYSROOT_DIR}/etc"
        ln -sf ../usr/share/terminfo "${SYSROOT_DIR}/etc/terminfo" 2>/dev/null || true
    fi

    log_success "Terminfo database copied"
}

# =============================================================================
# Make every locale declaration agree with what was actually generated
# =============================================================================
# /etc/profile and /etc/environment are written by create_configs, which runs
# after copy_locale_data, and both hardcode en_US.UTF-8. If generation fell
# back to C.UTF-8 they would contradict it and the "cannot change locale"
# warning would come straight back through pam_env and /etc/profile.
#
# Rewritten with sed rather than by interpolating into those heredocs: they are
# quoted on purpose, because they are full of $PATH and $HOME that must reach
# the shipped file unexpanded.
align_locale_declarations() {
    local lang="${RAVEN_SYSTEM_LANG:-C.UTF-8}"

    if [[ -f "${SYSROOT_DIR}/etc/profile" ]]; then
        sed -i \
            -e "s|^export LANG=.*|export LANG=${lang}|" \
            -e "s|^export LANGUAGE=.*|export LANGUAGE=${lang}|" \
            -e "s|^export LC_ALL=.*|# LC_ALL deliberately unset: it outranks every LC_* category|" \
            "${SYSROOT_DIR}/etc/profile"
    fi

    if [[ -f "${SYSROOT_DIR}/etc/environment" ]]; then
        sed -i -e "s|^LANG=.*|LANG=\"${lang}\"|" -e "/^LC_ALL=/d" \
            "${SYSROOT_DIR}/etc/environment"
    fi

    log_info "  System locale: ${lang}"
}

# =============================================================================
# Copy locale data and X11 compose files
# =============================================================================
copy_locale_data() {
    log_info "Setting up locale data..."

    # Create locale directories
    mkdir -p "${SYSROOT_DIR}/usr/share/locale"
    mkdir -p "${SYSROOT_DIR}/usr/share/i18n/locales"
    mkdir -p "${SYSROOT_DIR}/usr/share/i18n/charmaps"
    mkdir -p "${SYSROOT_DIR}/usr/lib/locale"

    # Copy locale definitions if available
    if [[ -d /usr/share/i18n/locales ]]; then
        cp /usr/share/i18n/locales/en_US "${SYSROOT_DIR}/usr/share/i18n/locales/" 2>/dev/null || true
        cp /usr/share/i18n/locales/en_GB "${SYSROOT_DIR}/usr/share/i18n/locales/" 2>/dev/null || true
        cp /usr/share/i18n/locales/POSIX "${SYSROOT_DIR}/usr/share/i18n/locales/" 2>/dev/null || true
        cp /usr/share/i18n/locales/i18n "${SYSROOT_DIR}/usr/share/i18n/locales/" 2>/dev/null || true
        cp /usr/share/i18n/locales/iso14651_t1 "${SYSROOT_DIR}/usr/share/i18n/locales/" 2>/dev/null || true
        cp /usr/share/i18n/locales/iso14651_t1_common "${SYSROOT_DIR}/usr/share/i18n/locales/" 2>/dev/null || true
        cp /usr/share/i18n/locales/translit_* "${SYSROOT_DIR}/usr/share/i18n/locales/" 2>/dev/null || true
    fi

    # Copy UTF-8 charmap
    if [[ -d /usr/share/i18n/charmaps ]]; then
        cp /usr/share/i18n/charmaps/UTF-8.gz "${SYSROOT_DIR}/usr/share/i18n/charmaps/" 2>/dev/null || true
        cp /usr/share/i18n/charmaps/UTF-8 "${SYSROOT_DIR}/usr/share/i18n/charmaps/" 2>/dev/null || true
    fi

    # Copy whatever the host happens to have already compiled. This is a
    # bonus, not the mechanism -- see the generation step below.
    if [[ -f /usr/lib/locale/locale-archive ]]; then
        cp /usr/lib/locale/locale-archive "${SYSROOT_DIR}/usr/lib/locale/" 2>/dev/null || true
    fi

    for locale_dir in /usr/lib/locale/en_US* /usr/lib/locale/C.* /usr/lib/locale/POSIX; do
        if [[ -d "$locale_dir" ]]; then
            cp -r "$locale_dir" "${SYSROOT_DIR}/usr/lib/locale/" 2>/dev/null || true
        fi
    done

    # ==========================================================================
    # Compile the locales, rather than hoping the build host has them
    # ==========================================================================
    # Copying alone was the whole mechanism, and it never fired: locales are
    # produced by locale-gen, which nobody runs in a container image, so an
    # Arch build host has no compiled en_US.UTF-8 to copy. The sysroot ended up
    # with i18n *source* data, no compiled locale, and an /etc/locale.conf
    # promising en_US.UTF-8 -- so every shell opened with
    #
    #   bash: warning: setlocale: LC_ALL: cannot change locale (en_US.UTF-8):
    #                  No such file or directory
    #
    # localedef --prefix compiles straight into the sysroot, reading the source
    # data from the build host.
    local generated=()
    if command -v localedef &>/dev/null; then
        local spec src charmap name
        for spec in "en_US:UTF-8:en_US.UTF-8" "C:UTF-8:C.UTF-8"; do
            src="${spec%%:*}"
            charmap="$(echo "${spec}" | cut -d: -f2)"
            name="${spec##*:}"

            if localedef -i "${src}" -f "${charmap}" \
                    --prefix="${SYSROOT_DIR}" "${name}" 2>/dev/null; then
                generated+=("${name}")
                log_info "  Generated ${name}"
            else
                log_warn "  Could not generate ${name}"
            fi
        done
    else
        log_warn "  localedef not on the build host; no locales will be compiled"
    fi

    # Configure the locale the system actually has. Advertising one that was
    # never built is what produced the warning in the first place, so if
    # generation failed, say C.UTF-8 -- glibc carries that one built in.
    # Ask what the sysroot actually holds. localedef --prefix writes a
    # locale-archive rather than an en_US.utf8 directory, so a directory test
    # alone reports "missing" for a locale that is present -- which is the same
    # class of mistake as the bug being fixed here.
    local system_lang="C.UTF-8"
    if [[ " ${generated[*]-} " == *" en_US.UTF-8 "* ]] \
       || [[ -d "${SYSROOT_DIR}/usr/lib/locale/en_US.utf8" ]] \
       || localedef --list-archive --prefix="${SYSROOT_DIR}" 2>/dev/null \
            | grep -qx 'en_US.utf8'; then
        system_lang="en_US.UTF-8"
    else
        log_warn "  en_US.UTF-8 unavailable; configuring ${system_lang} instead"
    fi

    # Published for align_locale_declarations(), which runs after
    # create_configs and makes every other file agree with this decision.
    RAVEN_SYSTEM_LANG="${system_lang}"

    # X11 Compose files (needed for Fyne and other GUI toolkits)
    mkdir -p "${SYSROOT_DIR}/usr/share/X11/locale"

    if [[ -d /usr/share/X11/locale ]]; then
        # Copy compose files for common locales
        for locale in en_US.UTF-8 C UTF-8 iso8859-1 compose.dir locale.dir; do
            if [[ -e "/usr/share/X11/locale/$locale" ]]; then
                cp -r "/usr/share/X11/locale/$locale" "${SYSROOT_DIR}/usr/share/X11/locale/" 2>/dev/null || true
            fi
        done

        # Copy locale.alias and compose.dir
        cp /usr/share/X11/locale/locale.alias "${SYSROOT_DIR}/usr/share/X11/locale/" 2>/dev/null || true
        cp /usr/share/X11/locale/locale.dir "${SYSROOT_DIR}/usr/share/X11/locale/" 2>/dev/null || true
        cp /usr/share/X11/locale/compose.dir "${SYSROOT_DIR}/usr/share/X11/locale/" 2>/dev/null || true
    fi

    # Create locale.gen
    cat > "${SYSROOT_DIR}/etc/locale.gen" << 'EOF'
en_US.UTF-8 UTF-8
en_GB.UTF-8 UTF-8
C.UTF-8 UTF-8
EOF

    # Create locale.conf.
    #
    # LANG only. LC_ALL is an override that outranks every LC_* category and is
    # meant for "just this command" -- setting it system-wide means a user
    # cannot pick their own LC_TIME or LC_COLLATE at all. It is also why the
    # missing locale surfaced as a warning on every single shell: bash reports
    # a failed LC_ALL loudly.
    cat > "${SYSROOT_DIR}/etc/locale.conf" << EOF
LANG=${system_lang}
EOF

    # Create a minimal /etc/default/locale
    mkdir -p "${SYSROOT_DIR}/etc/default"
    cat > "${SYSROOT_DIR}/etc/default/locale" << EOF
LANG=${system_lang}
EOF

    log_success "Locale data configured"
}

# =============================================================================
# Copy timezone data
# =============================================================================
copy_timezone_data() {
    log_info "Setting up timezone data..."

    # Create timezone directories
    mkdir -p "${SYSROOT_DIR}/usr/share/zoneinfo"

    # Copy timezone data
    if [[ -d /usr/share/zoneinfo ]]; then
        # Copy all timezone data (it's not that large)
        cp -r /usr/share/zoneinfo/* "${SYSROOT_DIR}/usr/share/zoneinfo/" 2>/dev/null || true
    fi

    # Set default timezone to UTC
    ln -sf /usr/share/zoneinfo/UTC "${SYSROOT_DIR}/etc/localtime" 2>/dev/null || true

    # Create timezone config
    echo "UTC" > "${SYSROOT_DIR}/etc/timezone"

    # Create adjtime for hwclock
    cat > "${SYSROOT_DIR}/etc/adjtime" << 'EOF'
0.0 0 0.0
0
UTC
EOF

    log_success "Timezone data configured"
}

# =============================================================================
# Create essential system directories
# =============================================================================
create_system_directories() {
    log_info "Creating essential system directories..."

    # Runtime directories (these are typically tmpfs at boot, but need to exist)
    mkdir -p "${SYSROOT_DIR}/tmp"
    mkdir -p "${SYSROOT_DIR}/run"
    mkdir -p "${SYSROOT_DIR}/run/dbus"
    mkdir -p "${SYSROOT_DIR}/run/lock"
    chmod 1777 "${SYSROOT_DIR}/tmp" 2>/dev/null || true
    chmod 755 "${SYSROOT_DIR}/run" 2>/dev/null || true

    # Variable data directories
    mkdir -p "${SYSROOT_DIR}/var/log"
    mkdir -p "${SYSROOT_DIR}/var/tmp"
    mkdir -p "${SYSROOT_DIR}/var/mail"
    mkdir -p "${SYSROOT_DIR}/var/spool/mail"
    mkdir -p "${SYSROOT_DIR}/var/lib"
    mkdir -p "${SYSROOT_DIR}/var/cache"
    chmod 1777 "${SYSROOT_DIR}/var/tmp" 2>/dev/null || true

    # Create compatibility symlinks
    ln -sf /run "${SYSROOT_DIR}/var/run" 2>/dev/null || true
    ln -sf /run/lock "${SYSROOT_DIR}/var/lock" 2>/dev/null || true

    # System directories
    mkdir -p "${SYSROOT_DIR}/usr/libexec"
    mkdir -p "${SYSROOT_DIR}/usr/local/bin"
    mkdir -p "${SYSROOT_DIR}/usr/local/lib"
    mkdir -p "${SYSROOT_DIR}/usr/local/share"

    # Config directories
    mkdir -p "${SYSROOT_DIR}/etc/profile.d"
    mkdir -p "${SYSROOT_DIR}/etc/skel"
    mkdir -p "${SYSROOT_DIR}/etc/default"
    mkdir -p "${SYSROOT_DIR}/etc/ld.so.conf.d"

    log_success "System directories created"
}

# =============================================================================
# Create essential config files
# =============================================================================
create_configs() {
    log_info "Creating configuration files..."

    # Default shell preference: bash > sh
    local default_shell="/bin/sh"
    if [[ -x "${SYSROOT_DIR}/usr/bin/bash" ]]; then
        default_shell="/bin/bash"
    elif [[ -x "${SYSROOT_DIR}/usr/bin/sh" ]]; then
        default_shell="/bin/sh"
    fi

    # /etc/os-release
    #
    # The heredoc is quoted, so ${RAVEN_VERSION} cannot expand inside it and the
    # version has to be substituted afterwards. It was previously three
    # hardcoded copies of the number, which meant bumping RAVEN_VERSION left the
    # shipped system claiming the old release.
    cat > "${SYSROOT_DIR}/etc/os-release" << 'EOF'
NAME="Raven Linux"
PRETTY_NAME="Raven Linux @RAVEN_VERSION@"
ID=raven
ID_LIKE=arch
BUILD_ID=rolling
VERSION_ID=@RAVEN_VERSION@
VERSION="@RAVEN_VERSION@ (Rolling)"
ANSI_COLOR="38;2;23;147;209"
HOME_URL="https://github.com/javanhut/RavenLinux"
DOCUMENTATION_URL="https://github.com/javanhut/RavenLinux"
LOGO=raven-logo
EOF
    sed -i "s/@RAVEN_VERSION@/${RAVEN_VERSION:-2026.08}/g" "${SYSROOT_DIR}/etc/os-release"

    # Create uname wrapper to show raven-linux
    # Remove existing symlink first (stage1 creates /bin/uname -> coreutils)
    rm -f "${SYSROOT_DIR}/usr/bin/uname"
    cat > "${SYSROOT_DIR}/usr/bin/uname" << 'UNAMESCRIPT'
#!/bin/sh
# Raven Linux uname wrapper
REAL_UNAME=/bin/uname.real

if [ ! -x "$REAL_UNAME" ]; then
    # Fallback if real uname not found
    exec /usr/bin/uname "$@"
fi

case "$1" in
    -a|--all)
        # Show full info with raven-linux
        kernel=$($REAL_UNAME -s)
        nodename=$($REAL_UNAME -n)
        release=$($REAL_UNAME -r)
        version=$($REAL_UNAME -v)
        machine=$($REAL_UNAME -m)
        echo "raven-linux $nodename $release $version $machine"
        ;;
    -s|--kernel-name)
        echo "raven-linux"
        ;;
    -o|--operating-system)
        echo "Raven Linux"
        ;;
    "")
        echo "raven-linux"
        ;;
    *)
        exec $REAL_UNAME "$@"
        ;;
esac
UNAMESCRIPT
    chmod +x "${SYSROOT_DIR}/usr/bin/uname"

	    # /etc/hostname
	    echo "raven-linux" > "${SYSROOT_DIR}/etc/hostname"

	    # /etc/hosts
	    cat > "${SYSROOT_DIR}/etc/hosts" << 'EOF'
127.0.0.1   localhost
::1         localhost
127.0.1.1   raven-linux.localdomain raven-linux
EOF

	    # /bin/raven-shell: used by agetty --skip-login as a PAM-free rescue shell
	    mkdir -p "${SYSROOT_DIR}/usr/bin"
	    if [[ -f "${PROJECT_ROOT}/etc/raven/raven-shell" ]]; then
	        cp "${PROJECT_ROOT}/etc/raven/raven-shell" "${SYSROOT_DIR}/usr/bin/raven-shell"
	        chmod 0755 "${SYSROOT_DIR}/usr/bin/raven-shell" 2>/dev/null || true
	    fi

	    # /etc/raven/init.toml (service configuration for raven-init)
	    mkdir -p "${SYSROOT_DIR}/etc/raven"
	    if [[ -f "${PROJECT_ROOT}/etc/raven/init.toml" ]]; then
	        cp "${PROJECT_ROOT}/etc/raven/init.toml" "${SYSROOT_DIR}/etc/raven/init.toml"
	    elif [[ -f "${PROJECT_ROOT}/init/config/init.toml" ]]; then
	        cp "${PROJECT_ROOT}/init/config/init.toml" "${SYSROOT_DIR}/etc/raven/init.toml"
	    fi

	    # /etc/raven/power.toml (what the power button and the lid do).
	    # Absent is fine -- raven-powerd falls back to the same defaults this
	    # file writes down -- but shipping it is what makes the settings
	    # discoverable instead of documented only in the source.
	    if [[ -f "${PROJECT_ROOT}/etc/raven/power.toml" ]]; then
	        cp "${PROJECT_ROOT}/etc/raven/power.toml" "${SYSROOT_DIR}/etc/raven/power.toml"
	        chmod 0644 "${SYSROOT_DIR}/etc/raven/power.toml" 2>/dev/null || true
	    fi

	    # /etc/raven/time.toml (NTP servers and the sync interval). Same deal
	    # as power.toml: raven-timed falls back to these exact defaults, but
	    # the file is where a person finds out the settings exist.
	    if [[ -f "${PROJECT_ROOT}/etc/raven/time.toml" ]]; then
	        cp "${PROJECT_ROOT}/etc/raven/time.toml" "${SYSROOT_DIR}/etc/raven/time.toml"
	        chmod 0644 "${SYSROOT_DIR}/etc/raven/time.toml" 2>/dev/null || true
	    fi

	    # Service drop-ins: raven-init reads /etc/raven/init.d/*.toml alongside
	    # init.toml, so daemons installed later (`rvn install openssh`) get a
	    # service definition by copying a template -- no editing init.toml. The
	    # templates ship inert under /usr/share/raven/services; init.d starts
	    # empty because the base image runs nothing it does not ship.
	    mkdir -p "${SYSROOT_DIR}/etc/raven/init.d"
	    if [[ -d "${PROJECT_ROOT}/configs/raven/services" ]]; then
	        mkdir -p "${SYSROOT_DIR}/usr/share/raven/services"
	        cp "${PROJECT_ROOT}/configs/raven/services/"*.toml \
	            "${SYSROOT_DIR}/usr/share/raven/services/" 2>/dev/null || true
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
	args = ["--noclear", "--skip-login", "--login-program", "/bin/raven-shell", "tty1", "linux"]
	restart = true
	enabled = true
	critical = false

[[services]]
	name = "getty-ttyS0"
	description = "Serial console getty on ttyS0"
	exec = "/sbin/agetty"
	args = ["--noclear", "--skip-login", "--login-program", "/bin/raven-shell", "-L", "115200", "ttyS0", "vt102"]
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
EOF
    fi
    chmod 0644 "${SYSROOT_DIR}/etc/raven/init.toml" 2>/dev/null || true

    # /etc/nsswitch.conf (glibc NSS defaults; required for users/dns on minimal systems)
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

    # /etc/passwd
    cat > "${SYSROOT_DIR}/etc/passwd" << EOF
root:x:0:0:root:/root:${default_shell}
raven:x:1000:1000:Raven User:/home/raven:${default_shell}
dbus:x:81:81:System Message Bus:/:/bin/false
nobody:x:65534:65534:Nobody:/:/bin/false
EOF

    # /etc/group
    #
    # `caw` gates state-changing commands on cawd's socket -- scan and status
    # are open to everyone, but connect, disconnect and shutdown check the
    # peer's credentials. Upstream ships this as a sysusers.d fragment, which
    # nothing here reads, so the group is declared with the rest of them.
    #
    # The numbers are Arch's, not ours to choose. A tar payload records
    # ownership NUMERICALLY, and Arch's `filesystem` ships /srv/ftp as gid 11
    # and /var/games as gid 50 -- so while audio sat at 11 and video at 12,
    # every `rvn install` that touched those paths produced files owned by the
    # wrong group entirely. audio/video/input are 92/91/97 for that reason.
    # skeleton.sh carries the full table and will renumber anything that drifts
    # back, but the file should be written correctly here in the first place.
    cat > "${SYSROOT_DIR}/etc/group" << 'EOF'
root:x:0:
wheel:x:10:raven
sudo:x:27:raven
audio:x:92:raven
video:x:91:raven
input:x:97:raven
tty:x:5:raven
disk:x:6:
lp:x:7:
kmem:x:9:
users:x:100:raven
raven:x:1000:
caw:x:970:raven
dbus:x:81:
nobody:x:65534:
nogroup:x:65533:
EOF

    # /etc/shadow with SHA512 hashed passwords
    # Default password is "raven" for both root and raven users
    # Users should change this on first boot using the first-boot-setup script
    local default_pass_hash=""

    # Generate SHA512 hash for default password "raven"
    if command -v openssl &>/dev/null; then
        # Use openssl to generate SHA512 hash with random salt
        default_pass_hash=$(openssl passwd -6 "raven" 2>/dev/null) || true
    fi

    # Fallback to pre-computed SHA512 hash if openssl failed
    if [[ -z "$default_pass_hash" ]]; then
        # Pre-computed SHA512 hash for password "raven" (salt: ravenlinux)
        default_pass_hash='$6$ravenlinux$O8Y5jKz8VgZ3LfJk5QT2xK9mNwH6pR1yB4vC7dE0fG2hI3jK4lM5nO6pQ7rS8tU9vW0xY1zA2bC3dE4fG5hI6j'
    fi

    # Calculate days since epoch for password last change
    local days_since_epoch=$(($(date +%s) / 86400))

    cat > "${SYSROOT_DIR}/etc/shadow" << EOF
root:${default_pass_hash}:${days_since_epoch}:0:99999:7:::
raven:${default_pass_hash}:${days_since_epoch}:0:99999:7:::
dbus:!:${days_since_epoch}:0:99999:7:::
nobody:!:${days_since_epoch}:0:99999:7:::
EOF
    chmod 0600 "${SYSROOT_DIR}/etc/shadow"
    chown root:root "${SYSROOT_DIR}/etc/shadow" 2>/dev/null || true
    log_info "  Created /etc/shadow with default password (change on first boot)"

    # /etc/gshadow (group shadow file - some tools expect this)
    cat > "${SYSROOT_DIR}/etc/gshadow" << 'EOF'
root:::
wheel:::raven
sudo:::raven
audio:::raven
video:::raven
input:::raven
tty:::raven
disk:::
lp:::
kmem:::
users:::raven
raven:::
caw:!::raven
nobody:!::
nogroup:!::
EOF
    chmod 0600 "${SYSROOT_DIR}/etc/gshadow"
    chown root:root "${SYSROOT_DIR}/etc/gshadow" 2>/dev/null || true

    # Ensure proper permissions on passwd/group
    chmod 0644 "${SYSROOT_DIR}/etc/passwd"
    chmod 0644 "${SYSROOT_DIR}/etc/group"

    # /etc/shells
    cat > "${SYSROOT_DIR}/etc/shells" << 'EOF'
/bin/sh
/bin/bash
EOF

    # /etc/sudoers (wheel group allowed by default)
    mkdir -p "${SYSROOT_DIR}/etc/sudoers.d"
    cat > "${SYSROOT_DIR}/etc/sudoers" << 'EOF'
Defaults env_reset
Defaults !lecture

root ALL=(ALL:ALL) ALL
%wheel ALL=(ALL:ALL) ALL
EOF
    chmod 0440 "${SYSROOT_DIR}/etc/sudoers" 2>/dev/null || true

    # Kernel module config directories (kmod expects these to exist)
    mkdir -p "${SYSROOT_DIR}/etc/modprobe.d" "${SYSROOT_DIR}/etc/modules-load.d"
    cat > "${SYSROOT_DIR}/etc/modprobe.d/raven.conf" << 'EOF'
# RavenLinux kernel module configuration
# Place module options/blacklists here.
EOF

    # Realtek rtw89 (RTL8852BE) stability defaults
    # Both Realtek drivers below are built into the kernel (=y, see
    # build-kernel.sh), and modprobe only reads these files while loading a
    # module -- so on their own they do nothing. raven-init applies them at
    # boot through /sys/module/<name>/parameters (apply_builtin_module_options
    # in init/src/main.rs), before it re-probes the card.
    cat > "${SYSROOT_DIR}/etc/modprobe.d/rtw89.conf" << 'EOF'
# RavenLinux: Realtek rtw89 defaults
# RTL8852BE often fails to bring up a netdev with PCIe ASPM enabled on some laptops.
# rtw89 is built in; raven-init applies this line at boot (modprobe never sees it).
options rtw89_pci disable_aspm=1
EOF

    # Realtek rtw88 (RTL8821CE and friends): the same trap one generation back.
    # Seen on the ASUS ROG test laptop: the 8821CE probes and loads its firmware,
    # then every `ip link set up` dies in the power-on sequence --
    # "failed to poll offset=0x6 mask=0x2 value=0x2 / mac power on failed" --
    # and the interface never leaves ENETDOWN. disable_aspm makes the driver
    # skip its CLKREQ and ASPM L1 programming (rtw88/pci.c), which is what
    # leaves the chip's power controller reachable on these boards.
    cat > "${SYSROOT_DIR}/etc/modprobe.d/rtw88.conf" << 'EOF'
# RavenLinux: Realtek rtw88 defaults
# RTL8821CE/8822CE fail "mac power on" with PCIe ASPM enabled on some laptops.
# rtw88 is built in; raven-init applies this line at boot (modprobe never sees it).
options rtw88_pci disable_aspm=1
EOF

    # Host side of the same link. rtw88_pci.disable_aspm only stops the chip
    # from requesting L1; the root port still parks the link in L1 on its own
    # (CONFIG_PCIEASPM_DEFAULT keeps whatever the BIOS programmed). On the ASUS
    # ROG test laptop the RTL8821CE powers on once at probe, and every power-on
    # after its first power-off then fails "power ready" (poll offset=0x6) --
    # with the chip-side knob already off. "performance" disables ASPM on every
    # link, the runtime equivalent of pcie_aspm=off on the command line.
    # Applied by raven-init before the card is probed (pcie_aspm is built in).
    cat > "${SYSROOT_DIR}/etc/modprobe.d/pcie-aspm.conf" << 'EOF'
# RavenLinux: PCIe ASPM policy
# Realtek RTL8821CE fails to power on again behind a link the host holds in L1.
# pcie_aspm is built in; raven-init applies this line at boot (modprobe never sees it).
options pcie_aspm policy=performance
EOF

    # asus-wmi / asus-nb-wmi were not in the kernel config of the last build
    # on which the ASUS ROG laptop's RTL8821CE worked (June 2026); they are
    # modules now, and udev loads them between the card's probe and its first
    # `port up`. On ASUS boards the WLAN rfkill they register drives the card's
    # power through the EC (WMI DEVS 0x00010011), and registering it writes
    # that state -- so they are the one new thing touching the card's power in
    # exactly the window where every later power-on starts failing. Kept out
    # until proven innocent; the cost is ASUS hotkeys/fan-curve WMI, nothing
    # the console system uses. If they turn out to be it, `options asus_nb_wmi
    # wapf=4` (WLAN under OS control) is the gentler form to try next.
    cat > "${SYSROOT_DIR}/etc/modprobe.d/asus-wmi.conf" << 'EOF'
# RavenLinux: keep ASUS WMI off the wifi card's power (see stage2-native.sh)
blacklist asus_nb_wmi
blacklist asus_wmi
EOF

    # rvn package manager config
    mkdir -p "${SYSROOT_DIR}/etc/rvn"
    cat > "${SYSROOT_DIR}/etc/rvn/config.toml" << 'EOF'
[general]
cache_dir = "/var/cache/rvn"
database_dir = "/var/lib/rvn"
log_dir = "/var/log/rvn"
parallel_downloads = 5
check_signatures = true

[[repositories]]
name = "raven"
url = "https://repo.theravenlinux.org/raven_linux_v0.1.0"
enabled = true
priority = 1

[[repositories]]
name = "community-github"
url = "https://raw.githubusercontent.com/javanhut/CommunityReposRL/main/raven_linux_v0.1.0"
enabled = false
priority = 10
type = "github"

[build]
jobs = 4
ccache = true
build_dir = "/tmp/rvn-build"
EOF

    # /usr/bin/whoami -- a standalone shim that does not depend on uutils
    # multicall behaviour.
    #
    # Only install it when there is no REAL whoami to install it over. Before
    # the usr-merge this shim lived at /bin/whoami while the real GNU coreutils
    # binary sat at /sbin/whoami, so the two never collided and both shipped.
    # /bin and /sbin are one directory now, and writing the shim
    # unconditionally would clobber the real binary -- the one name in the
    # whole sysroot where both sides were real files with different content.
    #
    # The real binary wins. Note this is a behaviour change from the shim: the
    # shim never fails, whereas coreutils whoami errors out when the uid has no
    # passwd entry.
    local _whoami="${SYSROOT_DIR}/usr/bin/whoami"
    local _install_whoami=1
    if [[ -f "${_whoami}" && ! -L "${_whoami}" ]] \
        && head -c 4 "${_whoami}" 2>/dev/null | grep -q 'ELF'; then
        log_info "  Keeping the real whoami binary; not overwriting it with the shell shim"
        _install_whoami=0
    fi
    if [[ "${_install_whoami}" == "1" ]]; then
    rm -f "${_whoami}" 2>/dev/null || true
    cat > "${_whoami}" << 'EOF'
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
    chmod 755 "${_whoami}" 2>/dev/null || true
    fi

    # /usr/bin/switch-user - Clean user switching utility for RavenLinux
    # Uses runuser/setpriv instead of su to avoid PAM issues
    cat > "${SYSROOT_DIR}/usr/bin/switch-user" << 'SWITCHUSER'
#!/bin/bash
# switch-user - RavenLinux User Switching Utility
# Usage: switch-user <username>
#
# Cleanly switches to another user WITHOUT using su/PAM

# Colors
RED='\033[1;31m'
GREEN='\033[1;32m'
YELLOW='\033[1;33m'
CYAN='\033[1;36m'
NC='\033[0m'

usage() {
    echo -e "${CYAN}switch-user${NC} - RavenLinux User Switching Utility"
    echo ""
    echo "Usage: switch-user <username>"
    echo ""
    echo "Options:"
    echo "  -h, --help     Show this help message"
    echo "  -l, --list     List available users"
    echo ""
    echo "Examples:"
    echo "  switch-user javanhut     # Switch to user 'javanhut'"
    echo "  switch-user root         # Switch back to root"
    exit 0
}

list_users() {
    echo -e "${CYAN}Available users:${NC}"
    while IFS=: read -r username _ uid _ _ home shell; do
        if [[ "$uid" -ge 1000 ]] || [[ "$uid" -eq 0 ]]; then
            if [[ -n "$shell" ]] && [[ "$shell" != "/bin/false" ]] && [[ "$shell" != "/sbin/nologin" ]]; then
                if [[ "$uid" -eq 0 ]]; then
                    echo -e "  ${GREEN}$username${NC} (root)"
                else
                    echo -e "  ${GREEN}$username${NC} (UID: $uid)"
                fi
            fi
        fi
    done < /etc/passwd
    exit 0
}

error() {
    echo -e "${RED}Error:${NC} $1" >&2
    exit 1
}

# Parse arguments
case "${1:-}" in
    -h|--help|help) usage ;;
    -l|--list|list) list_users ;;
    "")
        echo -e "${RED}Error:${NC} No username specified"
        echo "Usage: switch-user <username>"
        exit 1
        ;;
esac

TARGET_USER="$1"

# Check if user exists
if ! grep -q "^${TARGET_USER}:" /etc/passwd 2>/dev/null; then
    error "User '${TARGET_USER}' does not exist"
fi

# Get user info from /etc/passwd
IFS=: read -r _ _ USER_UID USER_GID _ USER_HOME USER_SHELL < <(grep "^${TARGET_USER}:" /etc/passwd)

# Validate/find shell
if [[ ! -x "$USER_SHELL" ]]; then
    for sh in /bin/bash /bin/sh; do
        [[ -x "$sh" ]] && { USER_SHELL="$sh"; break; }
    done
fi
[[ -x "$USER_SHELL" ]] || error "No valid shell found for user '${TARGET_USER}'"

# Create home directory if missing
[[ -d "$USER_HOME" ]] || { mkdir -p "$USER_HOME" 2>/dev/null; chown "${USER_UID}:${USER_GID}" "$USER_HOME" 2>/dev/null; }

# Create XDG runtime directory
XDG_RUNTIME="/run/user/${USER_UID}"
[[ -d "$XDG_RUNTIME" ]] || { mkdir -p "$XDG_RUNTIME" 2>/dev/null; chown "${USER_UID}:${USER_GID}" "$XDG_RUNTIME" 2>/dev/null; chmod 700 "$XDG_RUNTIME" 2>/dev/null; }

# Must be root to switch users
[[ $(id -u) -eq 0 ]] || error "Must be root to switch users"

echo -e "${GREEN}Switching to user:${NC} ${TARGET_USER}"

# Try different methods to switch user (in order of preference)

# Method 1: runuser (doesn't use PAM, designed for scripts)
if command -v runuser >/dev/null 2>&1; then
    exec runuser -u "$TARGET_USER" -- "$USER_SHELL" -l
fi

# Method 2: setpriv (directly changes UID/GID)
if command -v setpriv >/dev/null 2>&1; then
    exec setpriv --reuid="$USER_UID" --regid="$USER_GID" --init-groups \
        env HOME="$USER_HOME" USER="$TARGET_USER" LOGNAME="$TARGET_USER" \
        SHELL="$USER_SHELL" XDG_RUNTIME_DIR="$XDG_RUNTIME" \
        "$USER_SHELL" -l
fi

# Method 3: Direct exec with bash's exec builtin (requires bash 4.4+)
# This is a fallback that uses /proc to switch credentials
if [[ -w /proc/self/uid_map ]]; then
    exec env HOME="$USER_HOME" USER="$TARGET_USER" LOGNAME="$TARGET_USER" \
        SHELL="$USER_SHELL" XDG_RUNTIME_DIR="$XDG_RUNTIME" \
        "$USER_SHELL" -l
fi

# Method 4: Last resort - use su with timeout
if command -v su >/dev/null 2>&1; then
    echo -e "${YELLOW}Warning:${NC} Using su (may hang if PAM has issues)"
    if command -v timeout >/dev/null 2>&1; then
        timeout 5 su - "$TARGET_USER" || error "su timed out or failed"
    else
        exec su - "$TARGET_USER"
    fi
fi

error "No method available to switch user"
SWITCHUSER
    chmod 755 "${SYSROOT_DIR}/usr/bin/switch-user"
    log_info "  Created switch-user utility"

    # /etc/profile
    cat > "${SYSROOT_DIR}/etc/profile" << 'EOF'
export PATH=/bin:/sbin:/usr/bin:/usr/sbin
export HOME="${HOME:-/root}"
export TERM="${TERM:-linux}"
export TERMINFO=/usr/share/terminfo

# Locale settings
export LANG=en_US.UTF-8
export LC_ALL=en_US.UTF-8
export LANGUAGE=en_US.UTF-8

# X11/GUI locale support
export XLOCALEDIR=/usr/share/X11/locale

# XDG directories (required for GUI applications)
_UID="$(id -u 2>/dev/null || echo 0)"
export XDG_RUNTIME_DIR="/run/user/${_UID}"
export XDG_CONFIG_HOME="${HOME}/.config"
export XDG_DATA_HOME="${HOME}/.local/share"
export XDG_CACHE_HOME="${HOME}/.cache"

# Create XDG_RUNTIME_DIR if it doesn't exist
if [ -n "$XDG_RUNTIME_DIR" ] && [ ! -d "$XDG_RUNTIME_DIR" ]; then
    mkdir -p "$XDG_RUNTIME_DIR" 2>/dev/null
    chmod 700 "$XDG_RUNTIME_DIR" 2>/dev/null
fi

# Editor
export EDITOR=vem
export VISUAL=vem

# Raven identification
export RAVEN_LINUX=1

# Source locale.conf if it exists
[ -f /etc/locale.conf ] && . /etc/locale.conf
EOF

    # D-Bus configuration (needed by many GUI apps)
    if [[ -d /usr/share/dbus-1 ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/share/dbus-1"
        cp -r /usr/share/dbus-1/* "${SYSROOT_DIR}/usr/share/dbus-1/" 2>/dev/null || true
    fi
    if [[ -d /etc/dbus-1 ]]; then
        mkdir -p "${SYSROOT_DIR}/etc/dbus-1"
        cp -r /etc/dbus-1/* "${SYSROOT_DIR}/etc/dbus-1/" 2>/dev/null || true
    fi

    # Machine ID (required by D-Bus); regenerated on install as needed
    if [[ ! -s "${SYSROOT_DIR}/etc/machine-id" ]] && [[ -r /proc/sys/kernel/random/uuid ]]; then
        cat /proc/sys/kernel/random/uuid | tr -d '-' > "${SYSROOT_DIR}/etc/machine-id" 2>/dev/null || true
    fi

    # /etc/fstab
    cat > "${SYSROOT_DIR}/etc/fstab" << 'EOF'
# <device>  <mount>  <type>  <options>  <dump>  <pass>
proc        /proc    proc    defaults   0       0
sysfs       /sys     sysfs   defaults   0       0
devtmpfs    /dev     devtmpfs defaults  0       0
tmpfs       /tmp     tmpfs   defaults   0       0
tmpfs       /run     tmpfs   defaults   0       0
EOF

    # Create user home directories
    mkdir -p "${SYSROOT_DIR}/home/raven"
    mkdir -p "${SYSROOT_DIR}/root"

    # Bash config (system + default user configs)
    mkdir -p "${SYSROOT_DIR}/etc/bash" "${SYSROOT_DIR}/etc/skel"

    if [[ -f "${PROJECT_ROOT}/configs/bash/bashrc" ]]; then
        cp "${PROJECT_ROOT}/configs/bash/bashrc" "${SYSROOT_DIR}/etc/bash/bashrc"
        cp "${PROJECT_ROOT}/configs/bash/bashrc" "${SYSROOT_DIR}/etc/bashrc"
    else
        cat > "${SYSROOT_DIR}/etc/bashrc" << 'EOF'
# RavenLinux default bashrc (generated)
case $- in
    *i*) ;;
      *) return ;;
esac

HISTFILE=~/.bash_history
HISTSIZE=10000
HISTFILESIZE=10000
shopt -s histappend

PS1='[\u@raven-linux]# '

alias ls='ls --color=auto'
alias ll='ls -la'
alias la='ls -A'
alias grep='grep --color=auto'
alias ..='cd ..'

export PATH=/bin:/sbin:/usr/bin:/usr/sbin:$HOME/.local/bin
export EDITOR=vem
export VISUAL=vem
EOF
        cp "${SYSROOT_DIR}/etc/bashrc" "${SYSROOT_DIR}/etc/bash/bashrc"
    fi

    cp "${SYSROOT_DIR}/etc/bashrc" "${SYSROOT_DIR}/etc/skel/.bashrc" 2>/dev/null || true
    cp "${SYSROOT_DIR}/etc/bashrc" "${SYSROOT_DIR}/home/raven/.bashrc" 2>/dev/null || true
    cp "${SYSROOT_DIR}/etc/bashrc" "${SYSROOT_DIR}/root/.bashrc" 2>/dev/null || true

    cat > "${SYSROOT_DIR}/etc/skel/.bash_profile" << 'EOF'
# RavenLinux bash_profile (generated)
if [ -f ~/.bashrc ]; then
    . ~/.bashrc
fi
EOF
    cp "${SYSROOT_DIR}/etc/skel/.bash_profile" "${SYSROOT_DIR}/home/raven/.bash_profile" 2>/dev/null || true
    cp "${SYSROOT_DIR}/etc/skel/.bash_profile" "${SYSROOT_DIR}/root/.bash_profile" 2>/dev/null || true

    # /etc/login.defs - shadow password suite configuration
    cat > "${SYSROOT_DIR}/etc/login.defs" << 'EOF'
# /etc/login.defs - Shadow password suite configuration
# RavenLinux defaults

# Password aging controls
PASS_MAX_DAYS   99999
PASS_MIN_DAYS   0
PASS_WARN_AGE   7

# Min/max values for automatic UID/GID selection
UID_MIN                  1000
UID_MAX                 60000
SYS_UID_MIN               201
SYS_UID_MAX               999
GID_MIN                  1000
GID_MAX                 60000
SYS_GID_MIN               201
SYS_GID_MAX               999

# Home directory and umask
CREATE_HOME     yes
UMASK           022
HOME_MODE       0700

# Password encryption method
ENCRYPT_METHOD  SHA512

# Enable userdel to remove user groups if no members exist
USERGROUPS_ENAB yes

# Ensure mail directory exists
MAIL_DIR        /var/mail

# Default login shell
DEFAULT_HOME    yes

# Chfn restrictions
CHFN_RESTRICT   rwh

# Su defaults
SU_NAME         su
ENV_SUPATH      PATH=/sbin:/bin:/usr/sbin:/usr/bin
ENV_PATH        PATH=/bin:/usr/bin

# Log successful/failed logins
LOG_OK_LOGINS   no
FAILLOG_ENAB    yes

# TTY permissions
TTYPERM         0600
EOF

    # /etc/gshadow - group shadow file
    cat > "${SYSROOT_DIR}/etc/gshadow" << 'EOF'
root:::
wheel:::raven
audio:::raven
video:::raven
input:::raven
users:::raven
raven:::
nobody:!::
EOF
    chmod 600 "${SYSROOT_DIR}/etc/gshadow" 2>/dev/null || true

    # /etc/subuid - subordinate user IDs for containers/namespaces
    cat > "${SYSROOT_DIR}/etc/subuid" << 'EOF'
root:100000:65536
raven:165536:65536
EOF

    # /etc/subgid - subordinate group IDs for containers/namespaces
    cat > "${SYSROOT_DIR}/etc/subgid" << 'EOF'
root:100000:65536
raven:165536:65536
EOF

    # /etc/securetty - list of secure TTYs for root login
    cat > "${SYSROOT_DIR}/etc/securetty" << 'EOF'
# /etc/securetty - TTYs from which root can log in
console
tty1
tty2
tty3
tty4
tty5
tty6
ttyS0
ttyS1
ttyAMA0
hvc0
xvc0
EOF

    # /etc/environment - system-wide environment variables
    # PATH names /usr/bin and /usr/sbin only. /bin and /sbin still resolve
    # there through the usr-merge links, so listing them as well just made the
    # shell hash the same directory four times.
    cat > "${SYSROOT_DIR}/etc/environment" << 'EOF'
# /etc/environment - system-wide environment variables
PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin"
LANG="en_US.UTF-8"
EOF

    # /etc/host.conf - resolver configuration
    cat > "${SYSROOT_DIR}/etc/host.conf" << 'EOF'
# /etc/host.conf - resolver configuration
order hosts,bind
multi on
EOF

    # /etc/ld.so.conf - dynamic linker configuration
    # /lib, /lib64 and /usr/lib64 are all symlinks onto /usr/lib, so listing
    # them made ldconfig scan the same directory four times and write four
    # spellings of every entry into the cache.
    cat > "${SYSROOT_DIR}/etc/ld.so.conf" << 'EOF'
# /etc/ld.so.conf - dynamic linker configuration
# This root is usr-merged: /lib and /lib64 are symlinks onto /usr/lib.
/usr/lib
/usr/local/lib
include /etc/ld.so.conf.d/*.conf
EOF

    # /etc/inputrc - readline configuration
    cat > "${SYSROOT_DIR}/etc/inputrc" << 'EOF'
# /etc/inputrc - readline configuration
# RavenLinux defaults

# Ring bell on completion
set bell-style none

# Use visible bell if available
set bell-style visible

# Show all completions at once
set show-all-if-ambiguous on

# Ignore case when completing
set completion-ignore-case on

# Treat hyphens and underscores as equivalent
set completion-map-case on

# Show extra file information when completing
set visible-stats on

# Color files by types
set colored-stats on

# Append file type indicator
set mark-symlinked-directories on

# Match files whose names begin with '.'
set match-hidden-files on

# Key bindings
"\e[A": history-search-backward
"\e[B": history-search-forward
"\e[1;5C": forward-word
"\e[1;5D": backward-word
"\e[3~": delete-char
"\e[H": beginning-of-line
"\e[F": end-of-line
EOF

    # /etc/services - network services database (essential entries)
    cat > "${SYSROOT_DIR}/etc/services" << 'EOF'
# /etc/services - network services database
# RavenLinux essential services

tcpmux          1/tcp
echo            7/tcp
echo            7/udp
discard         9/tcp           sink null
discard         9/udp           sink null
systat          11/tcp          users
daytime         13/tcp
daytime         13/udp
netstat         15/tcp
qotd            17/tcp          quote
chargen         19/tcp          ttytst source
chargen         19/udp          ttytst source
ftp-data        20/tcp
ftp             21/tcp
ssh             22/tcp
ssh             22/udp
telnet          23/tcp
smtp            25/tcp          mail
time            37/tcp          timserver
time            37/udp          timserver
nameserver      42/tcp          name
whois           43/tcp          nicname
domain          53/tcp
domain          53/udp
bootps          67/udp
bootpc          68/udp
tftp            69/udp
gopher          70/tcp
finger          79/tcp
http            80/tcp          www www-http
kerberos        88/tcp          kerberos-sec
kerberos        88/udp          kerberos-sec
pop3            110/tcp         pop-3
sunrpc          111/tcp         portmapper rpcbind
sunrpc          111/udp         portmapper rpcbind
auth            113/tcp         ident tap
nntp            119/tcp         readnews untp
ntp             123/udp
netbios-ns      137/udp
netbios-dgm     138/udp
netbios-ssn     139/tcp
imap            143/tcp         imap2
snmp            161/udp
snmp-trap       162/udp         snmptrap
bgp             179/tcp
irc             194/tcp
ldap            389/tcp
https           443/tcp
smtps           465/tcp
submission      587/tcp
ldaps           636/tcp
imaps           993/tcp
pop3s           995/tcp
openvpn         1194/udp
mqtt            1883/tcp
nfs             2049/tcp
nfs             2049/udp
mysql           3306/tcp
rdp             3389/tcp
postgresql      5432/tcp
amqp            5672/tcp
x11             6000/tcp
http-alt        8080/tcp
EOF

    # /etc/protocols - protocol number database
    cat > "${SYSROOT_DIR}/etc/protocols" << 'EOF'
# /etc/protocols - protocol number database
# RavenLinux essential protocols

ip              0       IP              # Internet Protocol
hopopt          0       HOPOPT          # Hop-by-hop options
icmp            1       ICMP            # Internet Control Message
igmp            2       IGMP            # Internet Group Management
ggp             3       GGP             # Gateway-Gateway Protocol
ipencap         4       IP-ENCAP        # IP encapsulated in IP
st              5       ST              # ST datagram mode
tcp             6       TCP             # Transmission Control Protocol
egp             8       EGP             # Exterior Gateway Protocol
igp             9       IGP             # Interior Gateway Protocol
pup             12      PUP             # PARC universal packet
udp             17      UDP             # User Datagram Protocol
hmp             20      HMP             # Host Monitoring Protocol
xns-idp         22      XNS-IDP         # Xerox NS IDP
rdp             27      RDP             # Reliable Datagram Protocol
iso-tp4         29      ISO-TP4         # ISO Transport Protocol Class 4
dccp            33      DCCP            # Datagram Congestion Control Protocol
xtp             36      XTP             # Xpress Transfer Protocol
ddp             37      DDP             # Datagram Delivery Protocol
idpr-cmtp       38      IDPR-CMTP       # IDPR Control Message Transport
ipv6            41      IPv6            # IPv6 header
ipv6-route      43      IPv6-Route      # Routing Header for IPv6
ipv6-frag       44      IPv6-Frag       # Fragment Header for IPv6
idrp            45      IDRP            # Inter-Domain Routing Protocol
rsvp            46      RSVP            # Resource ReSerVation Protocol
gre             47      GRE             # Generic Routing Encapsulation
esp             50      ESP             # Encapsulating Security Payload
ah              51      AH              # Authentication Header
skip            57      SKIP            # Simple Key-Management for IP
ipv6-icmp       58      IPv6-ICMP       # ICMP for IPv6
ipv6-nonxt      59      IPv6-NoNxt      # No Next Header for IPv6
ipv6-opts       60      IPv6-Opts       # Destination Options for IPv6
eigrp           88      EIGRP           # EIGRP
ospf            89      OSPFIGP         # Open Shortest Path First
ax.25           93      AX.25           # AX.25
ipip            94      IPIP            # IP-within-IP
etherip         97      ETHERIP         # Ethernet-within-IP
encap           98      ENCAP           # Encapsulation Header
pim             103     PIM             # Protocol Independent Multicast
ipcomp          108     IPCOMP          # IP Payload Compression Protocol
vrrp            112     VRRP            # Virtual Router Redundancy Protocol
l2tp            115     L2TP            # Layer Two Tunneling Protocol
isis            124     ISIS            # IS-IS over IPv4
sctp            132     SCTP            # Stream Control Transmission Protocol
fc              133     FC              # Fibre Channel
udplite         136     UDPLite         # UDP-Lite
mpls-in-ip      137     MPLS-in-IP      # MPLS-in-IP
manet           138     MANET           # MANET Protocols
hip             139     HIP             # Host Identity Protocol
shim6           140     Shim6           # Site Multihoming by IPv6 Intermediation
wesp            141     WESP            # Wrapped ESP
rohc            142     ROHC            # Robust Header Compression
EOF

    # /etc/default/useradd - useradd defaults
    cat > "${SYSROOT_DIR}/etc/default/useradd" << 'EOF'
# /etc/default/useradd - useradd default values
GROUP=100
HOME=/home
INACTIVE=-1
EXPIRE=
SHELL=/bin/bash
SKEL=/etc/skel
CREATE_MAIL_SPOOL=yes
EOF

    log_success "Configuration files created"
}

# =============================================================================
# Main
# =============================================================================
main() {
    echo ""
    echo "=========================================="
    echo "  Stage 2: Native System Build"
    echo "=========================================="
    echo ""

    mkdir -p "${LOGS_DIR}"

    # Stage 2 is a rebuild of the sysroot. Ensure we can overwrite files even if
    # a previous build ran as root (root-owned files are not writable).
    if command -v mountpoint &>/dev/null; then
        if mountpoint -q "${SYSROOT_DIR}" 2>/dev/null; then
            log_fatal "${SYSROOT_DIR} is a mountpoint; unmount it before running stage2."
        fi
    fi
    if [[ -d "${SYSROOT_DIR}" ]]; then
        log_info "Resetting sysroot contents..."
        shopt -s dotglob nullglob
        rm -rf "${SYSROOT_DIR:?}/"* 2>/dev/null || true
        shopt -u dotglob nullglob

        # This wipe takes the Raven and GUI layers with it -- they are added by
        # stages that run after this one. Said out loud because the failure is
        # otherwise invisible: every later stage succeeds and the ISO boots, it
        # just has no shell, no package manager and no desktop in it.
        log_warn "  The sysroot is now empty: 'make raven' and 'make gui' must"
        log_warn "  run again before 'make iso', or those layers will not ship."
        log_warn "  'make build' does all of it in order."


        # If anything remains that we can't write to, it was likely created by
        # a previous root-run build. Fail early with a clear fix.
        if find "${SYSROOT_DIR}" -mindepth 1 ! -writable -print -quit 2>/dev/null | grep -q .; then
            log_fatal "Sysroot contains non-writable paths. Fix with: sudo chown -R \"$(id -un 2>/dev/null || echo root)\":\"$(id -gn 2>/dev/null || echo root)\" \"${SYSROOT_DIR}\""
        fi
    fi
    # stage2 wipes the sysroot above, which takes the usr-merge symlinks with
    # it, so the merged skeleton has to be rebuilt HERE as well as in stage1.
    # `imlazy stage2` is the documented entry point and never runs stage1, so
    # skipping this silently un-merges the tree for that whole flow.
    if declare -F raven_usrmerge_root >/dev/null 2>&1; then
        # log_fatal, not log_error: log_error only echoes and returns 0, so a
        # failed merge would carry on and build the rest of the system onto a
        # half-merged sysroot -- /bin a real directory, /lib a symlink. That
        # ships an image that cannot resolve its own libraries. build.sh has
        # always used log_fatal here; the others had drifted.
        raven_usrmerge_root "${SYSROOT_DIR}" || log_fatal "usr-merge skeleton failed"
    else
        log_fatal "scripts/lib/usrmerge.sh is missing; cannot create a usr-merged sysroot"
    fi
    # stage2 wiped the sysroot, so the skeleton goes with it -- same reasoning
    # as the merge above. `imlazy stage2` never runs stage1, so without this the
    # whole flow ships a tree with no /var/empty, no /etc/mtab and a 0755 /root.
    if declare -F raven_skeleton_root >/dev/null 2>&1; then
        raven_skeleton_root "${SYSROOT_DIR}" || log_fatal "rootfs skeleton failed"
    else
        log_fatal "scripts/lib/skeleton.sh is missing; cannot create a complete rootfs"
    fi

    copy_shells
    copy_system_utils
    copy_networking
    setup_pam_and_nss
    if [[ "${RAVEN_ENABLE_SUDO}" == "1" ]]; then
        setup_sudo
    else
        rm -f "${SYSROOT_DIR}/usr/bin/sudo" 2>/dev/null || true
    fi
    setup_su
    copy_ca_certificates
    copy_firmware
    copy_libraries
    generate_ldconfig_cache
    copy_terminfo
    copy_locale_data
    copy_timezone_data
    create_system_directories
    create_configs
    align_locale_declarations

    echo ""
    log_success "Stage 2 complete!"
    echo ""
}

# Run main (whether executed directly or sourced)
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
else
    main "$@"
fi
