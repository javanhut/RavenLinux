#!/bin/bash
# =============================================================================
# RavenLinux Stage 2: Native System Build
# =============================================================================
# Copies host tools and libraries needed for a functional live system
# In a full LFS-style build, this would rebuild everything natively
# For now, we copy essential tools from the host system

set -euo pipefail

# =============================================================================
# Environment Setup (with defaults for standalone execution)
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${RAVEN_ROOT:-$(dirname "$(dirname "$SCRIPT_DIR")")}"
BUILD_DIR="${RAVEN_BUILD:-${PROJECT_ROOT}/build}"
SYSROOT_DIR="${SYSROOT_DIR:-${BUILD_DIR}/sysroot}"
LOGS_DIR="${LOGS_DIR:-${BUILD_DIR}/logs}"

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
# Copy shells from host
# =============================================================================
copy_shells() {
    log_info "Copying shells..."

    local have_zsh=false
    local have_bash=false

    # Copy zsh
    if command -v zsh &>/dev/null; then
        cp "$(which zsh)" "${SYSROOT_DIR}/bin/zsh" && have_zsh=true
        mkdir -p "${SYSROOT_DIR}/usr/share/zsh"
        cp -r /usr/share/zsh/* "${SYSROOT_DIR}/usr/share/zsh/" 2>/dev/null || true
        log_info "  Added zsh"
    fi

    # Copy bash
    if command -v bash &>/dev/null; then
        cp "$(which bash)" "${SYSROOT_DIR}/bin/bash" && have_bash=true
        log_info "  Added bash"
    fi

    # Create sh symlink
    if $have_zsh; then
        ln -sf zsh "${SYSROOT_DIR}/bin/sh"
    elif $have_bash; then
        ln -sf bash "${SYSROOT_DIR}/bin/sh"
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

    local coreutils_bin="${SYSROOT_DIR}/bin/coreutils"
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
        # Disk utilities
        mount umount fdisk parted mkfs.ext4 mkfs.vfat fsck blkid lsblk
        # System info
        dmesg lspci lsusb free uptime uname hostname hostnamectl
        # User management
        passwd login chpasswd useradd usermod groupadd
        # Archiving
        tar gzip gunzip bzip2 xz zstd unzip zip
        # Editors (fallback)
        vi nano
        # Terminal utilities
        clear reset stty tput tset
        # Virtual terminal helpers (Wayland KMS compositors often need a VT)
        openvt chvt
        # Locale and timezone
        locale localedef localectl timedatectl hwclock date
        # D-Bus (needed by iwd/iwctl and many GUI apps)
        dbus-daemon dbus-launch dbus-send dbus-uuidgen
        # Seat management (Wayland compositors)
        seatd
        # Kernel module utilities (needed for auto-loading GPU/input drivers)
        modprobe insmod rmmod lsmod depmod modinfo kmod
        # Reference compositor (optional)
        weston
        # Alternative compositor (optional)
        Hyprland hyprctl
        # X11/Xwayland (optional, for legacy app support)
        Xorg Xwayland xinit startx xterm xclock xsetroot twm
        # Systemd tools (if available, for compatibility)
        journalctl systemctl udevadm
    )

    for util in "${utils[@]}"; do
        if command -v "$util" &>/dev/null; then
            local src
            src="$(which "$util" 2>/dev/null)" || continue
            [[ -f "$src" ]] || continue

            # Determine destination
            local dest="${SYSROOT_DIR}/bin/${util}"
            if [[ "$src" == */sbin/* ]]; then
                dest="${SYSROOT_DIR}/sbin/${util}"
            fi

            # Special handling for uname - save as uname.real for wrapper script
            if [[ "$util" == "uname" ]]; then
                dest="${SYSROOT_DIR}/bin/uname.real"
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

    # Prefer sudo-rs (built in stage1) over host sudo/su
    for bin in sudo su visudo; do
        if [[ -f "${BUILD_DIR}/bin/${bin}" ]]; then
            cp "${BUILD_DIR}/bin/${bin}" "${SYSROOT_DIR}/bin/${bin}" 2>/dev/null || true
        fi
    done
    if [[ -f "${SYSROOT_DIR}/bin/sudo" ]]; then
        chmod 4755 "${SYSROOT_DIR}/bin/sudo" 2>/dev/null || chmod 755 "${SYSROOT_DIR}/bin/sudo" || true
    fi
    if [[ -f "${SYSROOT_DIR}/bin/su" ]]; then
        chmod 4755 "${SYSROOT_DIR}/bin/su" 2>/dev/null || chmod 755 "${SYSROOT_DIR}/bin/su" || true
    fi
    if [[ -f "${SYSROOT_DIR}/bin/visudo" ]]; then
        chmod 755 "${SYSROOT_DIR}/bin/visudo" 2>/dev/null || true
    fi

    # Raven Wayland session launcher (optional, used when booting with raven.graphics=wayland)
    if [[ -f "${PROJECT_ROOT}/configs/raven-wayland-session" ]]; then
        cp "${PROJECT_ROOT}/configs/raven-wayland-session" "${SYSROOT_DIR}/bin/raven-wayland-session" 2>/dev/null || true
        chmod +x "${SYSROOT_DIR}/bin/raven-wayland-session" 2>/dev/null || true
        log_info "  Added raven-wayland-session"
    fi

    # If weston is available on the build host, copy its runtime data/plugins.
    if [[ -x "${SYSROOT_DIR}/bin/weston" ]]; then
        for d in /usr/lib/weston /usr/lib64/weston /usr/share/weston \
            /usr/lib/libweston-* /usr/lib64/libweston-*; do
            if [[ -d "$d" ]]; then
                mkdir -p "${SYSROOT_DIR}${d}"
                cp -a "${d}/." "${SYSROOT_DIR}${d}/" 2>/dev/null || true
                log_info "  Copied $(basename "$d") runtime data"
            fi
        done
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
            mkdir -p "${SYSROOT_DIR}/sbin"
            cp -L "${udevd}" "${SYSROOT_DIR}/sbin/udevd" 2>/dev/null || true
            chmod +x "${SYSROOT_DIR}/sbin/udevd" 2>/dev/null || true
            log_info "  Copied ${udevd} -> /sbin/udevd"
            break
        fi
    done

    # Provide /sbin/udevd symlink if we only have systemd-udevd.
    if [[ ! -e "${SYSROOT_DIR}/sbin/udevd" ]] && [[ -e "${SYSROOT_DIR}/usr/lib/systemd/systemd-udevd" ]]; then
        mkdir -p "${SYSROOT_DIR}/sbin"
        ln -sf /usr/lib/systemd/systemd-udevd "${SYSROOT_DIR}/sbin/udevd" 2>/dev/null || true
    fi

    # Some udev/module loaders expect modprobe in /sbin.
    if [[ -x "${SYSROOT_DIR}/bin/modprobe" ]] && [[ ! -e "${SYSROOT_DIR}/sbin/modprobe" ]]; then
        mkdir -p "${SYSROOT_DIR}/sbin"
        ln -sf /bin/modprobe "${SYSROOT_DIR}/sbin/modprobe" 2>/dev/null || true
    fi

    log_success "System utilities installed"
}

# =============================================================================
# Copy networking tools
# =============================================================================
copy_networking() {
    log_info "Copying networking tools..."

    local net_tools=(
        ip ping ping6 ss netstat route
        dhcpcd dhclient udhcpc
        iwd iwctl iwmon                    # iwd (preferred WiFi backend)
        wpa_supplicant wpa_cli wpa_passphrase  # wpa_supplicant (fallback)
        iw iwconfig iwlist rfkill
        ifconfig
        curl wget
        nc ncat
        host dig nslookup
        traceroute tracepath
    )

    for tool in "${net_tools[@]}"; do
        if command -v "$tool" &>/dev/null; then
            local src
            src="$(which "$tool" 2>/dev/null)" || continue
            [[ -f "$src" ]] || continue

            local dest="${SYSROOT_DIR}/bin/${tool}"
            if [[ "$src" == */sbin/* ]]; then
                dest="${SYSROOT_DIR}/sbin/${tool}"
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
# Copy required libraries for all binaries
# =============================================================================
copy_libraries() {
    log_info "Copying required libraries..."

    local lib_count=0

    for bin in "${SYSROOT_DIR}"/bin/* "${SYSROOT_DIR}"/sbin/*; do
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

    # Setup lib directories - use real directories, not symlinks
    mkdir -p "${SYSROOT_DIR}/lib"
    mkdir -p "${SYSROOT_DIR}/lib64"
    mkdir -p "${SYSROOT_DIR}/usr/lib"
    mkdir -p "${SYSROOT_DIR}/usr/lib64"

    # Copy dynamic linker to /lib64/ - this is where glibc binaries expect it
    log_info "Copying dynamic linker..."
    for ld in /lib64/ld-linux-x86-64.so.2 /lib/ld-linux-x86-64.so.2 /lib/ld-musl-x86_64.so.1 /usr/lib/ld-linux-x86-64.so.2; do
        if [[ -f "$ld" ]] || [[ -L "$ld" ]]; then
            local ld_name
            ld_name="$(basename "$ld")"
            # Copy to both /lib64 and /lib for maximum compatibility
            cp -L "$ld" "${SYSROOT_DIR}/lib64/${ld_name}" 2>/dev/null && log_info "  Copied ${ld_name} to /lib64/" || true
            cp -L "$ld" "${SYSROOT_DIR}/lib/${ld_name}" 2>/dev/null || true
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
        # EGL
        libEGL.so libEGL.so.1
        # GLX
        libglapi.so libglapi.so.0
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

    # Hyprland runtime data (if installed on host)
    log_info "Copying Hyprland runtime data..."
    for hypr_dir in /usr/share/hyprland /usr/share/hypr; do
        if [[ -d "${hypr_dir}" ]]; then
            mkdir -p "${SYSROOT_DIR}${hypr_dir}"
            cp -a "${hypr_dir}/." "${SYSROOT_DIR}${hypr_dir}/" 2>/dev/null || true
            log_info "  Copied ${hypr_dir}"
        fi
    done
    if [[ -f "/usr/share/wayland-sessions/hyprland.desktop" ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/share/wayland-sessions"
        cp -a "/usr/share/wayland-sessions/hyprland.desktop" "${SYSROOT_DIR}/usr/share/wayland-sessions/" 2>/dev/null || true
        log_info "  Copied hyprland.desktop"
    fi

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
        "${SYSROOT_DIR}/usr/lib64/xorg/modules" \
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

    log_success "Libraries copied"
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

    # Copy compiled locale archive if available
    if [[ -f /usr/lib/locale/locale-archive ]]; then
        cp /usr/lib/locale/locale-archive "${SYSROOT_DIR}/usr/lib/locale/" 2>/dev/null || true
    fi

    # Copy individual compiled locales
    for locale_dir in /usr/lib/locale/en_US* /usr/lib/locale/C.* /usr/lib/locale/POSIX; do
        if [[ -d "$locale_dir" ]]; then
            cp -r "$locale_dir" "${SYSROOT_DIR}/usr/lib/locale/" 2>/dev/null || true
        fi
    done

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

    # Create locale.conf
    cat > "${SYSROOT_DIR}/etc/locale.conf" << 'EOF'
LANG=en_US.UTF-8
LC_ALL=en_US.UTF-8
EOF

    # Create a minimal /etc/default/locale
    mkdir -p "${SYSROOT_DIR}/etc/default"
    cat > "${SYSROOT_DIR}/etc/default/locale" << 'EOF'
LANG=en_US.UTF-8
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
# Create essential config files
# =============================================================================
create_configs() {
    log_info "Creating configuration files..."

    local default_shell="/bin/sh"
    if [[ -x "${SYSROOT_DIR}/bin/zsh" ]]; then
        default_shell="/bin/zsh"
    elif [[ -x "${SYSROOT_DIR}/bin/bash" ]]; then
        default_shell="/bin/bash"
    elif [[ -x "${SYSROOT_DIR}/bin/sh" ]]; then
        default_shell="/bin/sh"
    fi

    # /etc/os-release
    cat > "${SYSROOT_DIR}/etc/os-release" << 'EOF'
NAME="Raven Linux"
PRETTY_NAME="Raven Linux 2025.12"
ID=raven
ID_LIKE=arch
BUILD_ID=rolling
VERSION_ID=2025.12
VERSION="2025.12 (Rolling)"
ANSI_COLOR="38;2;23;147;209"
HOME_URL="https://github.com/javanhut/RavenLinux"
DOCUMENTATION_URL="https://github.com/javanhut/RavenLinux"
LOGO=raven-logo
EOF

    # Create uname wrapper to show raven-linux
    # Remove existing symlink first (stage1 creates /bin/uname -> coreutils)
    rm -f "${SYSROOT_DIR}/bin/uname"
    cat > "${SYSROOT_DIR}/bin/uname" << 'UNAMESCRIPT'
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
    chmod +x "${SYSROOT_DIR}/bin/uname"

    # /etc/hostname
    echo "raven-linux" > "${SYSROOT_DIR}/etc/hostname"

    # /etc/hosts
    cat > "${SYSROOT_DIR}/etc/hosts" << 'EOF'
127.0.0.1   localhost
::1         localhost
127.0.1.1   raven-linux.localdomain raven-linux
EOF

    # /etc/passwd
    cat > "${SYSROOT_DIR}/etc/passwd" << EOF
root:x:0:0:root:/root:${default_shell}
raven:x:1000:1000:Raven User:/home/raven:${default_shell}
nobody:x:65534:65534:Nobody:/:/bin/false
EOF

    # /etc/group
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

    # /etc/shadow (empty passwords for live)
    cat > "${SYSROOT_DIR}/etc/shadow" << 'EOF'
root::0:0:99999:7:::
raven::0:0:99999:7:::
nobody:!:0:0:99999:7:::
EOF
    chmod 600 "${SYSROOT_DIR}/etc/shadow"

    # /etc/shells
    cat > "${SYSROOT_DIR}/etc/shells" << 'EOF'
/bin/sh
/bin/bash
/bin/zsh
EOF

    # /etc/sudoers (wheel group allowed by default)
    mkdir -p "${SYSROOT_DIR}/etc/sudoers.d"
    cat > "${SYSROOT_DIR}/etc/sudoers" << 'EOF'
Defaults env_reset
Defaults lecture=never

root ALL=(ALL:ALL) ALL
%wheel ALL=(ALL:ALL) ALL
EOF
    chmod 0440 "${SYSROOT_DIR}/etc/sudoers" 2>/dev/null || true

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

    # /bin/whoami (standalone, does not depend on uutils multicall behavior)
    rm -f "${SYSROOT_DIR}/bin/whoami" 2>/dev/null || true
    cat > "${SYSROOT_DIR}/bin/whoami" << 'EOF'
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
    chmod 755 "${SYSROOT_DIR}/bin/whoami" 2>/dev/null || true

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

    # D-Bus configuration (needed by iwctl/iwd and many GUI apps)
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

    # ZSH config
    mkdir -p "${SYSROOT_DIR}/etc/zsh"
    cat > "${SYSROOT_DIR}/etc/zsh/zshrc" << 'EOF'
# RavenLinux ZSH Configuration
HISTFILE=~/.zsh_history
HISTSIZE=10000
SAVEHIST=10000
setopt SHARE_HISTORY HIST_IGNORE_DUPS

autoload -Uz compinit && compinit
autoload -Uz promptinit && promptinit

    PROMPT='[%n@raven-linux]# '

alias ls='ls --color=auto'
alias ll='ls -la'
alias la='ls -A'
alias grep='grep --color=auto'
alias ..='cd ..'

bindkey -v
bindkey '^R' history-incremental-search-backward

export PATH=/bin:/sbin:/usr/bin:/usr/sbin:$HOME/.local/bin
export EDITOR=vem
EOF

    cp "${SYSROOT_DIR}/etc/zsh/zshrc" "${SYSROOT_DIR}/home/raven/.zshrc"
    cp "${SYSROOT_DIR}/etc/zsh/zshrc" "${SYSROOT_DIR}/root/.zshrc"

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
    mkdir -p "${SYSROOT_DIR}"/{bin,sbin,lib,lib64,usr/{bin,sbin,lib,share},etc,home,root}

    copy_shells
    copy_system_utils
    copy_networking
    copy_ca_certificates
    copy_libraries
    copy_terminfo
    copy_locale_data
    copy_timezone_data
    create_configs

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
