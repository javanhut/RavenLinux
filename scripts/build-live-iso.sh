#!/bin/bash
# =============================================================================
# RavenLinux Live ISO Builder
# =============================================================================
# Creates a complete live bootable ISO with all RavenLinux components
#
# Usage: ./scripts/build-live-iso.sh [options]
#   --skip-kernel     Skip kernel build (use existing)
#   --skip-packages   Skip package builds
#   --minimal         Build minimal ISO without desktop
#   --no-log          Disable file logging

set -euo pipefail

# =============================================================================
# Configuration
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
export RAVEN_ROOT="$PROJECT_ROOT"
export RAVEN_BUILD="${PROJECT_ROOT}/build"
ISO_DIR="${RAVEN_BUILD}/iso"
LIVE_ROOT="${ISO_DIR}/live-root"
SQUASHFS_DIR="${ISO_DIR}/squashfs"

# Version info
RAVEN_VERSION="2025.12"
RAVEN_ARCH="x86_64"
ISO_LABEL="RAVEN_LIVE"
ISO_OUTPUT="${PROJECT_ROOT}/raven-${RAVEN_VERSION}-${RAVEN_ARCH}.iso"

# Options
SKIP_KERNEL=false
SKIP_PACKAGES=false
MINIMAL=false

# Source shared logging library
source "${SCRIPT_DIR}/lib/logging.sh"
source "${PROJECT_ROOT}/scripts/lib/hyprland-config.sh"

# =============================================================================
# Argument Parsing
# =============================================================================

while [[ $# -gt 0 ]]; do
    case $1 in
        --skip-kernel) SKIP_KERNEL=true; shift ;;
        --skip-packages) SKIP_PACKAGES=true; shift ;;
        --minimal) MINIMAL=true; shift ;;
        --no-log) export RAVEN_NO_LOG=1; shift ;;
        *) log_fatal "Unknown option: $1" ;;
    esac
done

# =============================================================================
# Functions
# =============================================================================

check_dependencies() {
    log_step "Checking build dependencies..."

    local missing=()

    for cmd in mksquashfs xorriso grub-mkstandalone; do
        if ! command -v "$cmd" &>/dev/null; then
            missing+=("$cmd")
        fi
    done

    if [ ${#missing[@]} -ne 0 ]; then
        log_error "Missing dependencies: ${missing[*]}"
        log_fatal "Install with: sudo pacman -S squashfs-tools libisoburn grub"
    fi

    log_success "All dependencies found"
}

setup_live_root() {
    log_step "Setting up live root filesystem..."

    rm -rf "${LIVE_ROOT}"
    mkdir -p "${LIVE_ROOT}"/{bin,sbin,lib,lib64,usr/{bin,sbin,lib,lib64,share},etc,var,tmp,root,home,dev,proc,sys,run,mnt,opt}
    mkdir -p "${LIVE_ROOT}"/usr/share/{fonts,icons,themes,backgrounds}
    mkdir -p "${LIVE_ROOT}"/etc/{skel,xdg,rvn}
    mkdir -p "${LIVE_ROOT}"/var/{log,cache,lib,tmp}

    log_success "Live root structure created"
}

copy_kernel() {
    log_step "Copying kernel..."

    mkdir -p "${LIVE_ROOT}/boot"

    if [[ -f "${RAVEN_BUILD}/kernel/boot/vmlinuz-raven" ]]; then
        run_logged cp "${RAVEN_BUILD}/kernel/boot/vmlinuz-raven" "${LIVE_ROOT}/boot/vmlinuz"
        log_success "Kernel copied"
    else
        log_fatal "Kernel not found. Run ./scripts/build-kernel.sh first"
    fi
}

copy_kernel_modules() {
    log_step "Copying kernel modules..."

    local modules_root="${RAVEN_BUILD}/kernel/lib/modules"
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

    mkdir -p "${LIVE_ROOT}/lib/modules"
    rm -rf "${LIVE_ROOT}/lib/modules/${release}" 2>/dev/null || true
    cp -a "${modules_root}/${release}" "${LIVE_ROOT}/lib/modules/" 2>/dev/null || true

    if [[ -d "${LIVE_ROOT}/lib/modules/${release}" ]]; then
        log_info "  Copied /lib/modules/${release}"

        if command -v depmod &>/dev/null; then
            depmod -b "${LIVE_ROOT}" "${release}" 2>/dev/null || log_warn "depmod failed for ${release}"
        else
            log_warn "depmod not found on host; kernel module auto-loading may not work"
        fi

        log_success "Kernel modules copied"
    else
        log_warn "Failed to copy kernel modules into live root"
    fi
}

copy_firmware() {
    log_step "Copying firmware blobs..."

    local host_firmware=""
    for candidate in /usr/lib/firmware /lib/firmware; do
        if [[ -d "$candidate" ]]; then
            host_firmware="$candidate"
            break
        fi
    done

    if [[ -z "$host_firmware" ]]; then
        log_warn "No host firmware directory found; WiFi may not work"
        return 0
    fi

    mkdir -p "${LIVE_ROOT}/lib/firmware"

    local copied_any=0
    for dir in rtw89 rtw88 rtlwifi rtl_nic rtl_bt; do
        if [[ -d "${host_firmware}/${dir}" ]]; then
            mkdir -p "${LIVE_ROOT}/lib/firmware/${dir}"
            cp -a "${host_firmware}/${dir}/." "${LIVE_ROOT}/lib/firmware/${dir}/" 2>/dev/null || true
            log_info "  Added ${dir} firmware"
            copied_any=1
        fi
    done

    if [[ "${copied_any}" -eq 0 ]]; then
        log_warn "No Realtek firmware found under ${host_firmware}; install linux-firmware on the host and rebuild"
        return 0
    fi

    log_success "Firmware installed"
}

copy_initramfs() {
    log_step "Copying initramfs..."

    if [[ -f "${RAVEN_BUILD}/initramfs-raven.img" ]]; then
        run_logged cp "${RAVEN_BUILD}/initramfs-raven.img" "${LIVE_ROOT}/boot/initramfs.img"
        log_success "Initramfs copied"
    else
        log_warn "Initramfs not found, will create minimal one"
    fi
}

copy_coreutils() {
    log_step "Copying coreutils..."

    local utils=(
        cat cp mv rm ln mkdir rmdir touch chmod chown chgrp
        ls dir vdir head tail cut paste sort uniq wc tr tee
        echo printf yes df du stat sync id whoami groups
        uname hostname date sleep basename dirname realpath
        readlink pwd md5sum sha256sum test true false env
        seq dd install mktemp mknod tty xargs find grep less
    )

    if [[ -f "${RAVEN_BUILD}/bin/coreutils" ]]; then
        # Use uutils-coreutils if available (multi-call binary)
        cp "${RAVEN_BUILD}/bin/coreutils" "${LIVE_ROOT}/bin/coreutils"

        for util in "${utils[@]}"; do
            ln -sf coreutils "${LIVE_ROOT}/bin/${util}"
        done

        log_success "Coreutils installed (uutils)"
    else
        # Fallback: copy individual utilities from host
        log_warn "uutils-coreutils not found, copying host utilities"

        for util in "${utils[@]}"; do
            if command -v "$util" &>/dev/null; then
                local src
                src="$(which "$util" 2>/dev/null)" || continue
                [[ -f "$src" ]] || continue
                cp "$src" "${LIVE_ROOT}/bin/${util}" 2>/dev/null || true
            fi
        done

        log_success "Coreutils installed (host)"
    fi
}

copy_diagnostics_tools() {
    log_step "Copying diagnostic tools..."

    local tools=(
        dmesg
        lsmod modprobe depmod modinfo
        lspci lsusb dmidecode
        lscpu
        sensors smartctl nvme hdparm
        strace lsof
        mokutil efibootmgr
        udevadm
    )

    for tool in "${tools[@]}"; do
        if command -v "$tool" &>/dev/null; then
            cp "$(which "$tool")" "${LIVE_ROOT}/bin/" 2>/dev/null || \
            cp "$(which "$tool")" "${LIVE_ROOT}/sbin/" 2>/dev/null || true
            log_info "  Added ${tool}"
        fi
    done

    # udev runtime data + daemon (needed for device enumeration + module auto-load)
    if [[ -d "/usr/lib/udev" ]]; then
        mkdir -p "${LIVE_ROOT}/usr/lib"
        cp -a "/usr/lib/udev" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        log_info "  Copied /usr/lib/udev"
    fi

    # Copy custom RavenLinux udev rules for input device access
    if [[ -f "${RAVEN_ROOT}/configs/72-raven-input.rules" ]]; then
        mkdir -p "${LIVE_ROOT}/usr/lib/udev/rules.d"
        cp "${RAVEN_ROOT}/configs/72-raven-input.rules" "${LIVE_ROOT}/usr/lib/udev/rules.d/" 2>/dev/null || true
        log_info "  Copied custom input device udev rules"
    fi
    if [[ -d "/etc/udev" ]]; then
        mkdir -p "${LIVE_ROOT}/etc"
        cp -a "/etc/udev" "${LIVE_ROOT}/etc/" 2>/dev/null || true
        log_info "  Copied /etc/udev"
    fi
    if [[ -e "/usr/lib/systemd/systemd-udevd" ]]; then
        mkdir -p "${LIVE_ROOT}/usr/lib/systemd"
        cp -L "/usr/lib/systemd/systemd-udevd" "${LIVE_ROOT}/usr/lib/systemd/systemd-udevd" 2>/dev/null || true
        chmod +x "${LIVE_ROOT}/usr/lib/systemd/systemd-udevd" 2>/dev/null || true
        log_info "  Copied /usr/lib/systemd/systemd-udevd"
    fi
    for udevd in /sbin/udevd /usr/sbin/udevd /usr/lib/udev/udevd; do
        if [[ -e "${udevd}" ]]; then
            mkdir -p "${LIVE_ROOT}/sbin"
            cp -L "${udevd}" "${LIVE_ROOT}/sbin/udevd" 2>/dev/null || true
            chmod +x "${LIVE_ROOT}/sbin/udevd" 2>/dev/null || true
            log_info "  Copied ${udevd} -> /sbin/udevd"
            break
        fi
    done
    if [[ ! -e "${LIVE_ROOT}/sbin/udevd" ]] && [[ -e "${LIVE_ROOT}/usr/lib/systemd/systemd-udevd" ]]; then
        mkdir -p "${LIVE_ROOT}/sbin"
        ln -sf /usr/lib/systemd/systemd-udevd "${LIVE_ROOT}/sbin/udevd" 2>/dev/null || true
    fi

    # Some module loaders expect modprobe in /sbin.
    if [[ -x "${LIVE_ROOT}/bin/modprobe" ]] && [[ ! -e "${LIVE_ROOT}/sbin/modprobe" ]]; then
        mkdir -p "${LIVE_ROOT}/sbin"
        ln -sf /bin/modprobe "${LIVE_ROOT}/sbin/modprobe" 2>/dev/null || true
    fi

    log_success "Diagnostic tools installed"
}

copy_sudo_rs() {
    log_step "Installing sudo-rs..."

    # Do not ship sudo by default (avoid stale SUID artifacts across rebuilds).
    # Set RAVEN_ENABLE_SUDO=1 to include it.
    rm -f "${LIVE_ROOT}/bin/sudo" "${LIVE_ROOT}/usr/bin/sudo" 2>/dev/null || true
    if [[ "${RAVEN_ENABLE_SUDO:-0}" == "1" ]]; then
        if [[ -f "${RAVEN_BUILD}/bin/sudo" ]]; then
            cp "${RAVEN_BUILD}/bin/sudo" "${LIVE_ROOT}/bin/sudo"
            chmod 4755 "${LIVE_ROOT}/bin/sudo" 2>/dev/null || chmod 755 "${LIVE_ROOT}/bin/sudo" || true
        else
            log_warn "sudo-rs not found at ${RAVEN_BUILD}/bin/sudo (run ./scripts/build.sh stage1)"
            return 0
        fi
    fi

    if [[ -f "${RAVEN_BUILD}/bin/su" ]]; then
        cp "${RAVEN_BUILD}/bin/su" "${LIVE_ROOT}/bin/su"
        chmod 4755 "${LIVE_ROOT}/bin/su" 2>/dev/null || chmod 755 "${LIVE_ROOT}/bin/su" || true
    fi

    if [[ -f "${RAVEN_BUILD}/bin/visudo" ]]; then
        cp "${RAVEN_BUILD}/bin/visudo" "${LIVE_ROOT}/bin/visudo"
        chmod 755 "${LIVE_ROOT}/bin/visudo" 2>/dev/null || true
    fi

    log_success "sudo-rs installed"
}

install_whoami() {
    log_step "Installing whoami..."

    rm -f "${LIVE_ROOT}/bin/whoami" 2>/dev/null || true
    cat > "${LIVE_ROOT}/bin/whoami" << 'EOF'
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
    chmod 755 "${LIVE_ROOT}/bin/whoami" 2>/dev/null || true
    log_success "whoami installed"
}

copy_shells() {
    log_step "Copying shells..."

    local have_bash=false

    # Copy bash from host
    if command -v bash &>/dev/null; then
        cp "$(which bash)" "${LIVE_ROOT}/bin/bash" && have_bash=true
        log_info "  Added bash"
    fi

    # Create sh symlink - prefer bash
    if [[ "$have_bash" == true ]]; then
        ln -sf bash "${LIVE_ROOT}/bin/sh"
        log_info "  /bin/sh -> bash"
    else
        log_warn "  WARNING: bash not found for /bin/sh!"
    fi

    log_success "Shells installed"
}

copy_raven_packages() {
    log_step "Copying RavenLinux custom packages..."

    local packages_bin="${RAVEN_BUILD}/packages/bin"

    if [[ -d "${packages_bin}" ]]; then
        for pkg in vem carrion ivaldi raven-installer rvn raven-dhcp raven-powerctl reboot poweroff halt; do
            if [[ -f "${packages_bin}/${pkg}" ]]; then
                cp "${packages_bin}/${pkg}" "${LIVE_ROOT}/bin/${pkg}"
                log_info "  Added ${pkg}"
            fi
        done
    fi

    # Create symlink for installer command
    if [[ -f "${LIVE_ROOT}/bin/raven-installer" ]]; then
        ln -sf raven-installer "${LIVE_ROOT}/bin/raven-install"
    fi

    log_success "Custom packages installed"
}

copy_package_manager() {
    log_step "Building and copying package manager (rvn)..."

    local rvn_dir="${PROJECT_ROOT}/tools/rvn"

    if [[ -d "${rvn_dir}" ]]; then
        cd "${rvn_dir}"

        # Build rvn
        if run_logged cargo build --release 2>/dev/null; then
            cp target/release/rvn "${LIVE_ROOT}/bin/rvn"
            ln -sf rvn "${LIVE_ROOT}/bin/run" 2>/dev/null || true
            log_success "Package manager (rvn) installed"
        else
            log_warn "Failed to build rvn, skipping"
        fi

        cd "${PROJECT_ROOT}"
    fi
}

ensure_ethtool_live() {
    if command -v ethtool &>/dev/null; then
        return 0
    fi

    for candidate in \
        "${RAVEN_BUILD}/sysroot/sbin/ethtool" \
        "${RAVEN_BUILD}/sysroot/bin/ethtool" \
        "${RAVEN_BUILD}/sysroot/usr/sbin/ethtool" \
        "${RAVEN_BUILD}/sysroot/usr/bin/ethtool"; do
        if [[ -x "${candidate}" ]]; then
            mkdir -p "${LIVE_ROOT}/sbin"
            cp -L "${candidate}" "${LIVE_ROOT}/sbin/ethtool" 2>/dev/null || true
            log_info "  Added ethtool (from sysroot)"
            return 0
        fi
    done

    local version="${RAVEN_ETHTOOL_VERSION:-6.11}"
    local sources_dir="${RAVEN_BUILD}/sources"
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
            log_warn "ethtool missing and cannot download it (need curl or wget)"
            return 0
        fi
    fi

    if [[ ! -f "${tarball}" ]]; then
        log_warn "ethtool missing and download failed"
        return 0
    fi

    if ! command -v make &>/dev/null || ! command -v cc &>/dev/null; then
        log_warn "Cannot build ethtool (missing make/cc); ethtool will be missing"
        return 0
    fi

    rm -rf "${build_dir}" 2>/dev/null || true
    tar -xf "${tarball}" -C "${sources_dir}" 2>/dev/null || {
        log_warn "Failed to extract ${tarball}; ethtool will be missing"
        return 0
    }

    if [[ ! -d "${build_dir}" ]]; then
        log_warn "Expected extracted directory ${build_dir} not found; ethtool will be missing"
        return 0
    fi

    log_info "Building ethtool ${version}..."
    (
        cd "${build_dir}"
        ./configure --prefix=/usr --sbindir=/sbin >/dev/null 2>&1 || ./configure --prefix=/usr --sbindir=/sbin
        make -j"${RAVEN_JOBS:-$(nproc)}" >/dev/null 2>&1 || make -j"${RAVEN_JOBS:-$(nproc)}"
        make DESTDIR="${LIVE_ROOT}" install-strip >/dev/null 2>&1 || make DESTDIR="${LIVE_ROOT}" install
    ) || {
        log_warn "ethtool build failed; ethtool will be missing"
        return 0
    }

    if [[ -x "${LIVE_ROOT}/sbin/ethtool" ]] || [[ -x "${LIVE_ROOT}/bin/ethtool" ]]; then
        log_info "  Added ethtool (built during ISO build)"
    fi
}

copy_networking_tools() {
    log_step "Copying networking tools..."

    # Copy essential networking tools from host
    local net_tools=(ip ping ping6 dhcpcd wpa_supplicant iw iwconfig iwlist rfkill ethtool ifconfig route netstat ss curl wget mtr traceroute tracepath tcpdump)

    for tool in "${net_tools[@]}"; do
        if command -v "$tool" &>/dev/null; then
            cp "$(which "$tool")" "${LIVE_ROOT}/bin/" 2>/dev/null || \
            cp "$(which "$tool")" "${LIVE_ROOT}/sbin/" 2>/dev/null || true
            log_info "  Added ${tool}"
        fi
    done

    # Ensure ethtool exists even if the build host doesn't ship it.
    ensure_ethtool_live

    # Copy DNS resolver config
    echo "nameserver 8.8.8.8" > "${LIVE_ROOT}/etc/resolv.conf"
    echo "nameserver 1.1.1.1" >> "${LIVE_ROOT}/etc/resolv.conf"

    log_success "Networking tools installed"
}

copy_wayland_tools() {
    log_step "Copying Wayland/graphics tools..."

    # seatd is required for proper DRM master management
    if command -v seatd &>/dev/null; then
        cp "$(which seatd)" "${LIVE_ROOT}/bin/"
        log_info "  Added seatd"
    else
        log_warn "seatd not found - install with: sudo pacman -S seatd"
    fi

    # X11/Xwayland support (for legacy apps under Wayland, or optional Xorg)
    if command -v Xwayland &>/dev/null; then
        cp "$(which Xwayland)" "${LIVE_ROOT}/bin/"
        log_info "  Added Xwayland"
    fi
    if command -v Xorg &>/dev/null; then
        cp "$(which Xorg)" "${LIVE_ROOT}/bin/"
        log_info "  Added Xorg"
        # Xorg wrapper expects /usr/lib/Xorg or /usr/lib/Xorg.wrap
        if [[ -x "/usr/lib/Xorg" ]]; then
            mkdir -p "${LIVE_ROOT}/usr/lib"
            cp -L "/usr/lib/Xorg" "${LIVE_ROOT}/usr/lib/Xorg" 2>/dev/null || true
        fi
        if [[ -x "/usr/lib/Xorg.wrap" ]]; then
            mkdir -p "${LIVE_ROOT}/usr/lib"
            cp -L "/usr/lib/Xorg.wrap" "${LIVE_ROOT}/usr/lib/Xorg.wrap" 2>/dev/null || true
            chmod 4755 "${LIVE_ROOT}/usr/lib/Xorg.wrap" 2>/dev/null || true
        fi
    fi
    for xorg_dir in /usr/lib/xorg /usr/lib64/xorg /usr/lib/x86_64-linux-gnu/xorg; do
        [[ -d "${xorg_dir}" ]] || continue
        mkdir -p "${LIVE_ROOT}${xorg_dir}"
        cp -a "${xorg_dir}/." "${LIVE_ROOT}${xorg_dir}/" 2>/dev/null || true
        log_info "  Copied ${xorg_dir}"
    done
    for xorg_conf_dir in /usr/share/X11/xorg.conf.d /etc/X11/xorg.conf.d; do
        [[ -d "${xorg_conf_dir}" ]] || continue
        mkdir -p "${LIVE_ROOT}${xorg_conf_dir}"
        cp -a "${xorg_conf_dir}/." "${LIVE_ROOT}${xorg_conf_dir}/" 2>/dev/null || true
        log_info "  Copied $(basename "${xorg_conf_dir}")"
    done

    # Optional X11 helpers/clients for a usable Xorg session.
    for tool in xinit startx xterm xclock xsetroot twm; do
        if command -v "${tool}" &>/dev/null; then
            cp "$(which "${tool}")" "${LIVE_ROOT}/bin/" 2>/dev/null || true
            log_info "  Added ${tool}"
        fi
    done

    # Hyprland (primary compositor - required)
    if command -v Hyprland &>/dev/null; then
        cp "$(which Hyprland)" "${LIVE_ROOT}/bin/"
        log_info "  Added Hyprland"

        # Copy hyprctl
        if command -v hyprctl &>/dev/null; then
            cp "$(which hyprctl)" "${LIVE_ROOT}/bin/"
            log_info "  Added hyprctl"
        fi

        # Copy Hyprland GUI utilities (if installed)
        for guiutil in hyprland-welcome hyprland-update-screen hyprland-donate-screen hyprland-dialog hyprland-run; do
            if command -v "${guiutil}" &>/dev/null; then
                cp "$(which "${guiutil}")" "${LIVE_ROOT}/bin/" 2>/dev/null || true
                log_info "  Added ${guiutil}"
            fi
        done

        # Copy Hyprland ecosystem libraries
        for lib in libaquamarine libhyprcursor libhyprgraphics libhyprlang libhyprutils; do
            for libpath in /usr/lib/${lib}.so*; do
                if [[ -f "$libpath" ]] || [[ -L "$libpath" ]]; then
                    cp -L "$libpath" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
                fi
            done
        done
        log_info "  Added Hyprland ecosystem libraries"

        # Copy additional dependencies
        for lib in libdisplay-info libliftoff libre2 libtomlplusplus; do
            for libpath in /usr/lib/${lib}.so*; do
                if [[ -f "$libpath" ]] || [[ -L "$libpath" ]]; then
                    cp -L "$libpath" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
                fi
            done
        done
    else
        log_error "Hyprland not found on host system!"
        log_error "Install with: sudo pacman -S hyprland"
        exit 1
    fi

    # Copy Hyprland data directories
    for hypr_dir in /usr/share/hyprland /usr/share/hypr; do
        [[ -d "${hypr_dir}" ]] || continue
        mkdir -p "${LIVE_ROOT}${hypr_dir}"
        cp -a "${hypr_dir}/." "${LIVE_ROOT}${hypr_dir}/" 2>/dev/null || true
        log_info "  Copied ${hypr_dir}"
    done
    if [[ -f "/usr/share/wayland-sessions/hyprland.desktop" ]]; then
        mkdir -p "${LIVE_ROOT}/usr/share/wayland-sessions"
        cp -a "/usr/share/wayland-sessions/hyprland.desktop" "${LIVE_ROOT}/usr/share/wayland-sessions/" 2>/dev/null || true
        log_info "  Copied hyprland.desktop"
    fi

    install_hyprland_config \
        "${LIVE_ROOT}/etc/hypr/hyprland.conf" \
        "${LIVE_ROOT}/etc/skel/.config/hypr/hyprland.conf" \
        "${LIVE_ROOT}/root/.config/hypr/hyprland.conf"
    log_info "  Added Raven hyprland.conf"

    # Copy Raven scripts and default settings
    mkdir -p "${LIVE_ROOT}/root/.config/raven/scripts"
    if [[ -d "${PROJECT_ROOT}/desktop/config/raven/scripts" ]]; then
        cp "${PROJECT_ROOT}/desktop/config/raven/scripts"/*.sh "${LIVE_ROOT}/root/.config/raven/scripts/" 2>/dev/null || true
        chmod +x "${LIVE_ROOT}/root/.config/raven/scripts"/*.sh 2>/dev/null || true
        log_info "  Added Raven scripts"
    fi
    if command -v swaybg &>/dev/null; then
        cp "$(which swaybg)" "${LIVE_ROOT}/bin/" 2>/dev/null || true
        log_info "  Added swaybg"
    else
        log_warn "  swaybg not found on host; wallpapers may not render"
    fi
    if [[ -d "${PROJECT_ROOT}/desktop/config/raven/backgrounds" ]]; then
        mkdir -p "${LIVE_ROOT}/usr/share/backgrounds"
        cp "${PROJECT_ROOT}/desktop/config/raven/backgrounds"/* "${LIVE_ROOT}/usr/share/backgrounds/" 2>/dev/null || true
        log_info "  Added Raven wallpapers"
    fi

    # Create default Raven settings
    if [[ ! -f "${LIVE_ROOT}/root/.config/raven/settings.json" ]]; then
        cat > "${LIVE_ROOT}/root/.config/raven/settings.json" << 'SETTINGS'
{
  "theme": "dark",
  "accent_color": "#009688",
  "panel_position": "top",
  "panel_height": 38,
  "wallpaper_path": "",
  "wallpaper_mode": "fill",
  "border_width": 2,
  "gap_size": 8
}
SETTINGS
        log_info "  Added default Raven settings"
    fi

    # openvt for starting compositor on VT
    if command -v openvt &>/dev/null; then
        cp "$(which openvt)" "${LIVE_ROOT}/bin/"
        log_info "  Added openvt"
    fi

    # Copy wayland session script
    if [[ -f "${PROJECT_ROOT}/configs/raven-wayland-session" ]]; then
        cp "${PROJECT_ROOT}/configs/raven-wayland-session" "${LIVE_ROOT}/bin/"
        chmod +x "${LIVE_ROOT}/bin/raven-wayland-session"
        log_info "  Added raven-wayland-session"
    fi

    # NOTE: raven-compositor removed - using Hyprland instead (copied above)

    # Copy libseat library
    for lib in /usr/lib/libseat.so* /lib/libseat.so*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            mkdir -p "${LIVE_ROOT}/usr/lib"
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    # Copy libinput library and all its dependencies
    for lib in /usr/lib/libinput.so* /lib/libinput.so*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            mkdir -p "${LIVE_ROOT}/usr/lib"
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    # Copy libevdev (required by libinput for event device handling)
    for lib in /usr/lib/libevdev.so* /lib/libevdev.so*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    # Copy libmtdev (multitouch device library, required by libinput)
    for lib in /usr/lib/libmtdev.so* /lib/libmtdev.so*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    # Copy libudev (required by libinput for device discovery)
    for lib in /usr/lib/libudev.so* /lib/libudev.so*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    # Copy libwacom (tablet support, optional but libinput links against it)
    for lib in /usr/lib/libwacom.so* /lib/libwacom.so*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    # Copy libgudev (GObject wrapper for udev, used by libinput)
    for lib in /usr/lib/libgudev*.so* /lib/libgudev*.so*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    # Copy libinput quirks (device-specific settings for QEMU, laptops, etc.)
    if [[ -d "/usr/share/libinput" ]]; then
        mkdir -p "${LIVE_ROOT}/usr/share"
        cp -r /usr/share/libinput "${LIVE_ROOT}/usr/share/" 2>/dev/null || true
        log_info "  Added libinput quirks"
    fi

    # Copy libinput udev helpers
    for helper in libinput-device-group libinput-fuzz-extract libinput-fuzz-to-zero; do
        if [[ -x "/usr/lib/udev/${helper}" ]]; then
            mkdir -p "${LIVE_ROOT}/usr/lib/udev"
            cp "/usr/lib/udev/${helper}" "${LIVE_ROOT}/usr/lib/udev/" 2>/dev/null || true
        fi
    done

    # Copy EGL/Mesa libraries for GPU rendering
    for lib in /usr/lib/libEGL.so* /usr/lib/libGLESv2.so* /usr/lib/libgbm.so* /usr/lib/libdrm.so* /usr/lib/libwayland-*.so*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            mkdir -p "${LIVE_ROOT}/usr/lib"
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    # Copy Mesa DRI drivers
    if [[ -d "/usr/lib/dri" ]]; then
        mkdir -p "${LIVE_ROOT}/usr/lib/dri"
        cp /usr/lib/dri/*.so "${LIVE_ROOT}/usr/lib/dri/" 2>/dev/null || true
        log_info "  Added DRI drivers"
    fi

    # Copy Mesa GBM loader module(s) (needed for EGL/GBM compositors)
    for gbm_dir in /usr/lib/gbm /usr/lib64/gbm /usr/lib/x86_64-linux-gnu/gbm; do
        [[ -d "${gbm_dir}" ]] || continue
        mkdir -p "${LIVE_ROOT}${gbm_dir}"
        cp -a "${gbm_dir}/." "${LIVE_ROOT}${gbm_dir}/" 2>/dev/null || true
        log_info "  Added GBM modules ($(basename "${gbm_dir}"))"
    done

    # Create video group for seatd
    if ! grep -q "^video:" "${LIVE_ROOT}/etc/group" 2>/dev/null; then
        echo "video:x:12:raven,root" >> "${LIVE_ROOT}/etc/group"
    fi

    # Create seat group
    if ! grep -q "^seat:" "${LIVE_ROOT}/etc/group" 2>/dev/null; then
        echo "seat:x:13:raven,root" >> "${LIVE_ROOT}/etc/group"
    fi

    # Create input group for input devices (keyboard, mouse)
    if ! grep -q "^input:" "${LIVE_ROOT}/etc/group" 2>/dev/null; then
        echo "input:x:97:raven,root" >> "${LIVE_ROOT}/etc/group"
    fi

    # Create udev rules for input device access
    mkdir -p "${LIVE_ROOT}/usr/lib/udev/rules.d"
    cat > "${LIVE_ROOT}/usr/lib/udev/rules.d/70-input.rules" << 'UDEV_INPUT_EOF'
# Input device permissions for Wayland compositors
KERNEL=="event[0-9]*", SUBSYSTEM=="input", MODE="0660", GROUP="input"
KERNEL=="mouse[0-9]*", SUBSYSTEM=="input", MODE="0660", GROUP="input"
KERNEL=="mice", SUBSYSTEM=="input", MODE="0660", GROUP="input"
KERNEL=="js[0-9]*", SUBSYSTEM=="input", MODE="0660", GROUP="input"

# Allow seat access to input devices
SUBSYSTEM=="input", TAG+="seat", TAG+="uaccess"
UDEV_INPUT_EOF
    log_info "  Created input device udev rules"

    # Create udev rules for DRM/GPU access
    cat > "${LIVE_ROOT}/usr/lib/udev/rules.d/70-drm.rules" << 'UDEV_DRM_EOF'
# DRM device permissions for Wayland compositors
KERNEL=="card[0-9]*", SUBSYSTEM=="drm", MODE="0660", GROUP="video"
KERNEL=="renderD[0-9]*", SUBSYSTEM=="drm", MODE="0666"
UDEV_DRM_EOF
    log_info "  Created DRM device udev rules"

    # Copy xkeyboard-config for keyboard layouts
    # xkbcommon looks for /usr/share/xkeyboard-config-2 or /usr/share/X11/xkb
    if [[ -d "/usr/share/xkeyboard-config-2" ]]; then
        mkdir -p "${LIVE_ROOT}/usr/share"
        cp -r /usr/share/xkeyboard-config-2 "${LIVE_ROOT}/usr/share/" 2>/dev/null || true
        mkdir -p "${LIVE_ROOT}/usr/share/X11"
        ln -sf ../xkeyboard-config-2 "${LIVE_ROOT}/usr/share/X11/xkb" 2>/dev/null || true
        log_info "  Added xkeyboard-config-2"
    elif [[ -d "/usr/share/X11/xkb" ]]; then
        mkdir -p "${LIVE_ROOT}/usr/share/X11"
        cp -r /usr/share/X11/xkb "${LIVE_ROOT}/usr/share/X11/"
        # Also create symlink that xkbcommon expects
        ln -sf X11/xkb "${LIVE_ROOT}/usr/share/xkeyboard-config-2" 2>/dev/null || true
        log_info "  Added xkeyboard-config"
    fi

    # Also check alternative location
    if [[ -d "/usr/share/xkeyboard-config" ]]; then
        mkdir -p "${LIVE_ROOT}/usr/share"
        cp -r /usr/share/xkeyboard-config "${LIVE_ROOT}/usr/share/"
        ln -sf xkeyboard-config "${LIVE_ROOT}/usr/share/xkeyboard-config-2" 2>/dev/null || true
        log_info "  Added xkeyboard-config (alt location)"
    fi

    # XWayland invokes /usr/bin/xkbcomp at runtime to compile keymaps.
    # If missing, XWayland will fail to start with keyboard initialization errors.
    if [[ -x "/usr/bin/xkbcomp" ]]; then
        mkdir -p "${LIVE_ROOT}/bin" "${LIVE_ROOT}/usr/bin"
        cp -a "/usr/bin/xkbcomp" "${LIVE_ROOT}/bin/xkbcomp" 2>/dev/null || true
        ln -sf ../../bin/xkbcomp "${LIVE_ROOT}/usr/bin/xkbcomp" 2>/dev/null || true

        # Copy runtime libs for xkbcomp (live root does not run stage2's copy_libraries).
        timeout 2 ldd /usr/bin/xkbcomp 2>/dev/null | grep -o '/[^ ]*' | while read -r lib; do
            [[ -z "$lib" || ! -f "$lib" ]] && continue
            mkdir -p "${LIVE_ROOT}$(dirname "$lib")"
            cp -L "$lib" "${LIVE_ROOT}${lib}" 2>/dev/null || true
        done || true

        log_info "  Added xkbcomp for XWayland"
    else
        log_warn "  /usr/bin/xkbcomp not found on host; XWayland may fail to start"
    fi

    # Copy libxkbcommon
    for lib in /usr/lib/libxkbcommon*.so*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            mkdir -p "${LIVE_ROOT}/usr/lib"
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    log_success "Wayland tools installed"
}

# =============================================================================
# Build and install Raven desktop components
# =============================================================================
build_raven_desktop() {
    log_step "Building Raven desktop components..."

    if ! command -v cargo &>/dev/null; then
        log_warn "Cargo not found, skipping desktop component build"
        return 0
    fi

    local desktop_dir="${PROJECT_ROOT}/desktop"
    local shell_dir="${desktop_dir}/raven-shell"

    # Build raven-shell (unified Rust desktop - includes panel, desktop, menu, settings, etc.)
    if [[ -d "${shell_dir}" ]]; then
        log_info "  Building raven-shell (Rust workspace)..."
        cd "${shell_dir}"
        if cargo build --release 2>&1; then
            # Copy main binaries
            if [[ -f "target/release/raven-shell" ]]; then
                mkdir -p "${LIVE_ROOT}/usr/bin"
                cp target/release/raven-shell "${LIVE_ROOT}/usr/bin/"
                chmod +x "${LIVE_ROOT}/usr/bin/raven-shell"
                # Also create symlink in /bin for compatibility
                mkdir -p "${LIVE_ROOT}/bin"
                ln -sf /usr/bin/raven-shell "${LIVE_ROOT}/bin/raven-shell" 2>/dev/null || true
                log_info "  Installed raven-shell"
            fi
            if [[ -f "target/release/raven-ctl" ]]; then
                cp target/release/raven-ctl "${LIVE_ROOT}/usr/bin/"
                chmod +x "${LIVE_ROOT}/usr/bin/raven-ctl"
                log_info "  Installed raven-ctl"
            fi
        else
            log_warn "  Failed to build raven-shell"
        fi
        cd "${PROJECT_ROOT}"
    else
        log_warn "  raven-shell directory not found at ${shell_dir}"
    fi

    # Copy GTK4 layer-shell library (required for panels/docks on Wayland)
    log_info "  Copying GTK4 layer-shell library..."
    for lib in /usr/lib/libgtk4-layer-shell* /usr/lib64/libgtk4-layer-shell*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            mkdir -p "${LIVE_ROOT}/usr/lib"
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    # Copy GTK4 libraries and dependencies
    log_info "  Copying GTK4 libraries..."
    for lib in /usr/lib/libgtk-4* /usr/lib64/libgtk-4*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            mkdir -p "${LIVE_ROOT}/usr/lib"
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    # Copy GDK-Pixbuf loaders for image rendering
    if [[ -d /usr/lib/gdk-pixbuf-2.0 ]]; then
        mkdir -p "${LIVE_ROOT}/usr/lib"
        cp -r /usr/lib/gdk-pixbuf-2.0 "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
    fi

    # Copy GTK4 modules and settings
    if [[ -d /usr/lib/gtk-4.0 ]]; then
        mkdir -p "${LIVE_ROOT}/usr/lib"
        cp -r /usr/lib/gtk-4.0 "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
    fi

    # Copy Pango modules (for text rendering)
    for lib in /usr/lib/libpango* /usr/lib64/libpango*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    # Copy Cairo libraries (for drawing)
    for lib in /usr/lib/libcairo* /usr/lib64/libcairo*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    # Copy GLib/GObject/GIO libraries
    for lib in /usr/lib/libglib-2.0* /usr/lib/libgobject-2.0* /usr/lib/libgio-2.0* /usr/lib/libgmodule-2.0*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    # Copy Graphene library (used by GTK4)
    for lib in /usr/lib/libgraphene* /usr/lib64/libgraphene*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    # Copy GIO modules for various functionality
    if [[ -d /usr/lib/gio ]]; then
        mkdir -p "${LIVE_ROOT}/usr/lib"
        cp -r /usr/lib/gio "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
    fi

    # Copy Epoxy library (OpenGL dispatch for GTK4)
    for lib in /usr/lib/libepoxy* /usr/lib64/libepoxy*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    # Copy HarfBuzz libraries (text shaping)
    for lib in /usr/lib/libharfbuzz* /usr/lib64/libharfbuzz*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    # Copy Fribidi library (bidirectional text)
    for lib in /usr/lib/libfribidi* /usr/lib64/libfribidi*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    # Copy Fontconfig libraries
    for lib in /usr/lib/libfontconfig* /usr/lib64/libfontconfig*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    # Copy Pixman library (pixel manipulation)
    for lib in /usr/lib/libpixman* /usr/lib64/libpixman*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    # Copy all dependencies for built Raven binaries
    log_info "  Resolving Raven binary dependencies..."
    for bin in raven-shell raven-ctl; do
        if [[ -f "${LIVE_ROOT}/usr/bin/${bin}" ]]; then
            timeout 2 ldd "${LIVE_ROOT}/usr/bin/${bin}" 2>/dev/null | grep -o '/[^ ]*' | while read -r lib; do
                [[ -z "$lib" || ! -f "$lib" ]] && continue
                local dest="${LIVE_ROOT}${lib}"
                if [[ ! -f "$dest" ]]; then
                    mkdir -p "$(dirname "$dest")"
                    cp -L "$lib" "$dest" 2>/dev/null || true
                fi
            done || true
        fi
    done

    # Install Hyprland config for the live user
    mkdir -p "${LIVE_ROOT}/root/.config/raven"
    install_hyprland_config "${LIVE_ROOT}/root/.config/hypr/hyprland.conf"
    log_info "  Installed Hyprland config"

    # Create default settings.json
    if [[ ! -f "${LIVE_ROOT}/root/.config/raven/settings.json" ]]; then
        cat > "${LIVE_ROOT}/root/.config/raven/settings.json" << 'SETTINGS_EOF'
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
  "show_workspaces": true
}
SETTINGS_EOF
        log_info "  Created default Raven settings"
    fi

    log_success "Raven desktop components built and installed"
}

# =============================================================================
# Copy Desktop Services (PipeWire, Polkit, XDG Desktop Portal)
# =============================================================================
copy_desktop_services() {
    log_step "Copying desktop services (pipewire, polkit, portals)..."

    # -------------------------------------------------------------------------
    # PipeWire - Audio/Video server
    # -------------------------------------------------------------------------
    log_info "Installing PipeWire..."

    # PipeWire binaries
    for bin in pipewire pipewire-pulse wireplumber pw-cli pw-cat pw-dump pw-top; do
        if command -v "$bin" &>/dev/null; then
            cp "$(which "$bin")" "${LIVE_ROOT}/usr/bin/" 2>/dev/null || true
            log_info "  Added $bin"
        fi
    done

    # PipeWire libraries
    for lib in libpipewire-0.3.so* libwireplumber-0.5.so* libspa-0.2.so*; do
        for libpath in /usr/lib/${lib}; do
            if [[ -f "$libpath" ]] || [[ -L "$libpath" ]]; then
                cp -L "$libpath" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
            fi
        done
    done

    # PipeWire SPA plugins (audio backends, format converters)
    if [[ -d /usr/lib/spa-0.2 ]]; then
        mkdir -p "${LIVE_ROOT}/usr/lib/spa-0.2"
        cp -a /usr/lib/spa-0.2/. "${LIVE_ROOT}/usr/lib/spa-0.2/" 2>/dev/null || true
        log_info "  Added SPA plugins"
    fi

    # PipeWire modules
    if [[ -d /usr/lib/pipewire-0.3 ]]; then
        mkdir -p "${LIVE_ROOT}/usr/lib/pipewire-0.3"
        cp -a /usr/lib/pipewire-0.3/. "${LIVE_ROOT}/usr/lib/pipewire-0.3/" 2>/dev/null || true
        log_info "  Added PipeWire modules"
    fi

    # WirePlumber modules
    if [[ -d /usr/lib/wireplumber-0.5 ]]; then
        mkdir -p "${LIVE_ROOT}/usr/lib/wireplumber-0.5"
        cp -a /usr/lib/wireplumber-0.5/. "${LIVE_ROOT}/usr/lib/wireplumber-0.5/" 2>/dev/null || true
        log_info "  Added WirePlumber modules"
    fi

    # PipeWire configuration
    if [[ -d /usr/share/pipewire ]]; then
        mkdir -p "${LIVE_ROOT}/usr/share/pipewire"
        cp -a /usr/share/pipewire/. "${LIVE_ROOT}/usr/share/pipewire/" 2>/dev/null || true
        log_info "  Added PipeWire config"
    fi
    if [[ -d /etc/pipewire ]]; then
        mkdir -p "${LIVE_ROOT}/etc/pipewire"
        cp -a /etc/pipewire/. "${LIVE_ROOT}/etc/pipewire/" 2>/dev/null || true
    fi

    # WirePlumber configuration
    if [[ -d /usr/share/wireplumber ]]; then
        mkdir -p "${LIVE_ROOT}/usr/share/wireplumber"
        cp -a /usr/share/wireplumber/. "${LIVE_ROOT}/usr/share/wireplumber/" 2>/dev/null || true
        log_info "  Added WirePlumber config"
    fi
    if [[ -d /etc/wireplumber ]]; then
        mkdir -p "${LIVE_ROOT}/etc/wireplumber"
        cp -a /etc/wireplumber/. "${LIVE_ROOT}/etc/wireplumber/" 2>/dev/null || true
    fi

    # -------------------------------------------------------------------------
    # Polkit - Privilege escalation
    # -------------------------------------------------------------------------
    log_info "Installing Polkit..."

    # Polkit daemon
    if [[ -f /usr/lib/polkit-1/polkitd ]]; then
        mkdir -p "${LIVE_ROOT}/usr/lib/polkit-1"
        cp /usr/lib/polkit-1/polkitd "${LIVE_ROOT}/usr/lib/polkit-1/" 2>/dev/null || true
        chmod +x "${LIVE_ROOT}/usr/lib/polkit-1/polkitd"
        log_info "  Added polkitd"
    fi

    # Polkit binaries
    for bin in pkaction pkcheck pkexec pkttyagent; do
        if command -v "$bin" &>/dev/null; then
            cp "$(which "$bin")" "${LIVE_ROOT}/usr/bin/" 2>/dev/null || true
        fi
    done

    # Polkit libraries
    for lib in libpolkit-gobject-1.so* libpolkit-agent-1.so*; do
        for libpath in /usr/lib/${lib}; do
            if [[ -f "$libpath" ]] || [[ -L "$libpath" ]]; then
                cp -L "$libpath" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
            fi
        done
    done

    # Polkit configuration and rules
    if [[ -d /usr/share/polkit-1 ]]; then
        mkdir -p "${LIVE_ROOT}/usr/share/polkit-1"
        cp -a /usr/share/polkit-1/. "${LIVE_ROOT}/usr/share/polkit-1/" 2>/dev/null || true
        log_info "  Added Polkit rules"
    fi
    if [[ -d /etc/polkit-1 ]]; then
        mkdir -p "${LIVE_ROOT}/etc/polkit-1"
        cp -a /etc/polkit-1/. "${LIVE_ROOT}/etc/polkit-1/" 2>/dev/null || true
    fi

    # Polkit D-Bus service file
    if [[ -f /usr/share/dbus-1/system-services/org.freedesktop.PolicyKit1.service ]]; then
        mkdir -p "${LIVE_ROOT}/usr/share/dbus-1/system-services"
        cp /usr/share/dbus-1/system-services/org.freedesktop.PolicyKit1.service \
           "${LIVE_ROOT}/usr/share/dbus-1/system-services/" 2>/dev/null || true
    fi

    # Create polkitd user (if not exists in passwd)
    if ! grep -q "^polkitd:" "${LIVE_ROOT}/etc/passwd" 2>/dev/null; then
        echo "polkitd:x:27:27:PolicyKit Daemon:/:/sbin/nologin" >> "${LIVE_ROOT}/etc/passwd"
        echo "polkitd:x:27:" >> "${LIVE_ROOT}/etc/group"
    fi

    # Create polkit directories
    mkdir -p "${LIVE_ROOT}/var/lib/polkit-1" 2>/dev/null || true

    # -------------------------------------------------------------------------
    # XDG Desktop Portal + Hyprland backend
    # -------------------------------------------------------------------------
    log_info "Installing XDG Desktop Portal..."

    # Portal binaries (different distros put these in different places)
    for bin_path in /usr/libexec/xdg-desktop-portal /usr/lib/xdg-desktop-portal; do
        if [[ -f "$bin_path" ]]; then
            mkdir -p "${LIVE_ROOT}/usr/libexec"
            cp "$bin_path" "${LIVE_ROOT}/usr/libexec/" 2>/dev/null || true
            chmod +x "${LIVE_ROOT}/usr/libexec/xdg-desktop-portal"
            log_info "  Added xdg-desktop-portal"
            break
        fi
    done

    # Hyprland portal backend
    for bin_path in /usr/libexec/xdg-desktop-portal-hyprland /usr/lib/xdg-desktop-portal-hyprland; do
        if [[ -f "$bin_path" ]]; then
            mkdir -p "${LIVE_ROOT}/usr/libexec"
            cp "$bin_path" "${LIVE_ROOT}/usr/libexec/" 2>/dev/null || true
            chmod +x "${LIVE_ROOT}/usr/libexec/xdg-desktop-portal-hyprland"
            log_info "  Added xdg-desktop-portal-hyprland"
            break
        fi
    done

    # Portal configuration
    if [[ -d /usr/share/xdg-desktop-portal ]]; then
        mkdir -p "${LIVE_ROOT}/usr/share/xdg-desktop-portal"
        cp -a /usr/share/xdg-desktop-portal/. "${LIVE_ROOT}/usr/share/xdg-desktop-portal/" 2>/dev/null || true
        log_info "  Added portal config"
    fi

    # Hyprland portal config
    if [[ -d /usr/share/xdg-desktop-portal-hyprland ]]; then
        mkdir -p "${LIVE_ROOT}/usr/share/xdg-desktop-portal-hyprland"
        cp -a /usr/share/xdg-desktop-portal-hyprland/. "${LIVE_ROOT}/usr/share/xdg-desktop-portal-hyprland/" 2>/dev/null || true
    fi

    # Portal D-Bus service files
    for svc in org.freedesktop.portal.Desktop.service org.freedesktop.impl.portal.desktop.hyprland.service; do
        if [[ -f "/usr/share/dbus-1/services/${svc}" ]]; then
            mkdir -p "${LIVE_ROOT}/usr/share/dbus-1/services"
            cp "/usr/share/dbus-1/services/${svc}" "${LIVE_ROOT}/usr/share/dbus-1/services/" 2>/dev/null || true
        fi
    done

    # Create XDG portal config for Hyprland
    mkdir -p "${LIVE_ROOT}/etc/xdg/xdg-desktop-portal"
    cat > "${LIVE_ROOT}/etc/xdg/xdg-desktop-portal/hyprland-portals.conf" << 'EOF'
[preferred]
default=hyprland;gtk
org.freedesktop.impl.portal.Screenshot=hyprland
org.freedesktop.impl.portal.ScreenCast=hyprland
EOF

    log_success "Desktop services installed"
}

copy_ca_certificates() {
    log_step "Copying CA certificates (HTTPS trust store)..."

    mkdir -p "${LIVE_ROOT}/etc/ssl/certs" "${LIVE_ROOT}/etc/pki/tls/certs"

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
        log_warn "No CA bundle found on host; HTTPS may fail in the live environment"
        return 0
    fi

    cp -L "$src" "${LIVE_ROOT}/etc/ssl/certs/ca-certificates.crt" 2>/dev/null || true
    ln -sf /etc/ssl/certs/ca-certificates.crt "${LIVE_ROOT}/etc/ssl/cert.pem" 2>/dev/null || true
    cp -L "${LIVE_ROOT}/etc/ssl/certs/ca-certificates.crt" "${LIVE_ROOT}/etc/pki/tls/certs/ca-bundle.crt" 2>/dev/null || true

    log_info "  Added CA bundle from ${src}"
    log_success "CA certificates installed"
}

setup_pam_and_nss() {
    log_step "Setting up PAM and NSS runtime modules..."

    mkdir -p "${LIVE_ROOT}/etc/pam.d" "${LIVE_ROOT}/etc/security"
    mkdir -p "${LIVE_ROOT}/lib/security" "${LIVE_ROOT}/usr/lib/security"

    cat > "${LIVE_ROOT}/etc/pam.d/sudo" << 'EOF'
#%PAM-1.0
auth       sufficient   pam_rootok.so
auth       required     pam_unix.so nullok try_first_pass
account    sufficient   pam_rootok.so
account    required     pam_unix.so
session    required     pam_unix.so
password   required     pam_unix.so nullok sha512
EOF

    cat > "${LIVE_ROOT}/etc/pam.d/su" << 'EOF'
#%PAM-1.0
auth       sufficient   pam_rootok.so
auth       required     pam_unix.so nullok try_first_pass
account    sufficient   pam_rootok.so
account    required     pam_unix.so
session    required     pam_unix.so
password   required     pam_unix.so nullok sha512
EOF

    cat > "${LIVE_ROOT}/etc/pam.d/login" << 'EOF'
#%PAM-1.0
auth       required     pam_unix.so nullok try_first_pass
account    required     pam_unix.so
session    required     pam_unix.so
password   required     pam_unix.so nullok sha512
EOF

    cat > "${LIVE_ROOT}/etc/pam.d/passwd" << 'EOF'
#%PAM-1.0
password   required     pam_unix.so nullok sha512
EOF

    cat > "${LIVE_ROOT}/etc/security/limits.conf" << 'EOF'
# /etc/security/limits.conf
# Minimal defaults (RavenLinux). Add custom limits in /etc/security/limits.d/.
EOF
    mkdir -p "${LIVE_ROOT}/etc/security/limits.d"

    # pam_env.so expects these files. Some distros treat missing files as errors.
    if [[ ! -f "${LIVE_ROOT}/etc/environment" ]]; then
        cat > "${LIVE_ROOT}/etc/environment" << 'EOF'
# /etc/environment
# System-wide environment variables
PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
EOF
    fi

    if [[ ! -f "${LIVE_ROOT}/etc/security/pam_env.conf" ]]; then
        cat > "${LIVE_ROOT}/etc/security/pam_env.conf" << 'EOF'
# /etc/security/pam_env.conf
# Environment variables set by pam_env module
# Format: VARIABLE [DEFAULT=value] [OVERRIDE=value]
EOF
    fi

    # Fallback for PAM-aware programs that don't ship their own config.
    if [[ ! -f "${LIVE_ROOT}/etc/pam.d/other" ]]; then
        cat > "${LIVE_ROOT}/etc/pam.d/other" << 'EOF'
#%PAM-1.0
auth        required     pam_deny.so
account     required     pam_deny.so
password    required     pam_deny.so
session     required     pam_deny.so
EOF
    fi

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

    local pam_modules=(
        pam_unix.so
        pam_env.so
        pam_deny.so
        pam_permit.so
        pam_warn.so
        pam_limits.so
        pam_loginuid.so
        pam_rootok.so
        pam_nologin.so
        pam_securetty.so
        pam_wheel.so
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
        [[ -n "$src" ]] || continue
        cp -L "$src" "${LIVE_ROOT}/lib/security/${mod}" 2>/dev/null || true
        cp -L "$src" "${LIVE_ROOT}/usr/lib/security/${mod}" 2>/dev/null || true
        copied_any=1
    done

    if [[ "${copied_any}" -eq 0 ]]; then
        log_warn "No PAM modules found on host; sudo/login may not work (install a PAM stack and rebuild)"
    else
        log_info "  Added PAM modules"
    fi

    # PAM helper binaries (unix_chkpwd is critical for password verification).
    mkdir -p "${LIVE_ROOT}/sbin" "${LIVE_ROOT}/usr/sbin"
    local pam_helpers=(unix_chkpwd unix_update)
    local helper_dirs=(/usr/sbin /sbin /usr/bin /usr/lib /usr/lib/security /lib/security)
    for helper in "${pam_helpers[@]}"; do
        local src=""
        for d in "${helper_dirs[@]}"; do
            if [[ -x "${d}/${helper}" ]]; then
                src="${d}/${helper}"
                break
            fi
        done
        [[ -n "$src" ]] || continue
        cp -L "$src" "${LIVE_ROOT}/sbin/${helper}" 2>/dev/null || true
        chmod 4755 "${LIVE_ROOT}/sbin/${helper}" 2>/dev/null || true
        log_info "  Added PAM helper: ${helper} (SUID)"
    done

    # NSS modules (dlopened by glibc)
    mkdir -p "${LIVE_ROOT}/lib" "${LIVE_ROOT}/usr/lib"
    local nss_libs=(libnss_files.so.2 libnss_dns.so.2 libnss_compat.so.2)
    for lib in "${nss_libs[@]}"; do
        for d in /lib /lib64 /usr/lib /usr/lib64 /lib/x86_64-linux-gnu /usr/lib/x86_64-linux-gnu; do
            if [[ -e "${d}/${lib}" ]]; then
                cp -L "${d}/${lib}" "${LIVE_ROOT}/lib/${lib}" 2>/dev/null || true
                cp -L "${d}/${lib}" "${LIVE_ROOT}/usr/lib/${lib}" 2>/dev/null || true
                break
            fi
        done
    done

    log_success "PAM/NSS setup complete"
}

copy_libraries() {
    log_step "Copying required libraries..."

    # Find and copy required libraries for all binaries
    for bin in "${LIVE_ROOT}"/bin/* "${LIVE_ROOT}"/sbin/*; do
        [[ -f "$bin" && -x "$bin" && ! -L "$bin" ]] || continue

        # Skip statically linked binaries
        if file "$bin" | grep -q "statically linked"; then
            continue
        fi

        timeout 2 ldd "$bin" 2>/dev/null | grep -o '/[^ ]*' | while read -r lib; do
            [[ -z "$lib" || ! -f "$lib" ]] && continue
            local dest="${LIVE_ROOT}${lib}"
            if [[ ! -f "$dest" ]]; then
                mkdir -p "$(dirname "$dest")"
                cp -L "$lib" "$dest" 2>/dev/null || true
            fi
        done
    done

    # Copy deps for dlopened modules (e.g. Weston backends, Mesa GBM/DRI modules).
    for so in \
        "${LIVE_ROOT}"/lib/security/*.so \
        "${LIVE_ROOT}"/usr/lib/security/*.so \
        "${LIVE_ROOT}"/lib/libnss_*.so.* \
        "${LIVE_ROOT}"/usr/lib/libnss_*.so.* \
        "${LIVE_ROOT}"/usr/lib/libweston-*/*.so \
        "${LIVE_ROOT}"/usr/lib64/libweston-*/*.so \
        "${LIVE_ROOT}"/usr/lib/weston/*.so \
        "${LIVE_ROOT}"/usr/lib64/weston/*.so \
        "${LIVE_ROOT}"/usr/lib/gbm/*.so \
        "${LIVE_ROOT}"/usr/lib64/gbm/*.so \
        "${LIVE_ROOT}"/usr/lib/x86_64-linux-gnu/gbm/*.so \
        "${LIVE_ROOT}"/usr/lib/dri/*.so \
        "${LIVE_ROOT}"/usr/lib64/dri/*.so \
        "${LIVE_ROOT}"/usr/lib/x86_64-linux-gnu/dri/*.so; do
        [[ -f "$so" && ! -L "$so" ]] || continue
        timeout 2 ldd "$so" 2>/dev/null | grep -o '/[^ ]*' | while read -r lib; do
            [[ -z "$lib" || ! -f "$lib" ]] && continue
            local dest="${LIVE_ROOT}${lib}"
            if [[ ! -f "$dest" ]]; then
                mkdir -p "$(dirname "$dest")"
                cp -L "$lib" "$dest" 2>/dev/null || true
            fi
        done || true
    done

    # Xorg/Xwayland modules are also dlopened at runtime.
    for modules_dir in \
        "${LIVE_ROOT}/usr/lib/xorg/modules" \
        "${LIVE_ROOT}/usr/lib64/xorg/modules" \
        "${LIVE_ROOT}/usr/lib/x86_64-linux-gnu/xorg/modules"; do
        [[ -d "${modules_dir}" ]] || continue
        while IFS= read -r -d '' so; do
            timeout 2 ldd "$so" 2>/dev/null | grep -o '/[^ ]*' | while read -r lib; do
                [[ -z "$lib" || ! -f "$lib" ]] && continue
                local dest="${LIVE_ROOT}${lib}"
                if [[ ! -f "$dest" ]]; then
                    mkdir -p "$(dirname "$dest")"
                    cp -L "$lib" "$dest" 2>/dev/null || true
                fi
            done || true
        done < <(find "${modules_dir}" -type f -name '*.so' -print0 2>/dev/null) || true
    done

    # Copy dynamic linker - CRITICAL for all dynamically linked binaries
    # Binaries expect /lib64/ld-linux-x86-64.so.2 - we MUST have it there
    log_info "Copying dynamic linker..."

    # Ensure /lib64 is a real directory with the linker in it
    # Remove symlink if it exists and create real directory
    if [[ -L "${LIVE_ROOT}/lib64" ]]; then
        rm -f "${LIVE_ROOT}/lib64"
    fi
    mkdir -p "${LIVE_ROOT}/lib64"

    # Copy the dynamic linker directly to /lib64/
    local linker_found=false
    for ld in /lib64/ld-linux-x86-64.so.2 /lib/ld-linux-x86-64.so.2 /usr/lib/ld-linux-x86-64.so.2; do
        if [[ -f "$ld" ]] || [[ -L "$ld" ]]; then
            cp -L "$ld" "${LIVE_ROOT}/lib64/ld-linux-x86-64.so.2" 2>/dev/null
            # Also copy to /lib/ for compatibility
            cp -L "$ld" "${LIVE_ROOT}/lib/ld-linux-x86-64.so.2" 2>/dev/null
            linker_found=true
            break
        fi
    done

    # Verify the linker exists
    if [[ -f "${LIVE_ROOT}/lib64/ld-linux-x86-64.so.2" ]]; then
        log_info "  Dynamic linker installed at /lib64/ld-linux-x86-64.so.2"
        ls -la "${LIVE_ROOT}/lib64/ld-linux-x86-64.so.2"
    else
        log_warn "  WARNING: Dynamic linker not found! Binaries will fail!"
    fi

    log_success "Libraries copied"
}

create_config_files() {
    log_step "Creating configuration files..."

    # /etc/os-release
    cat > "${LIVE_ROOT}/etc/os-release" << EOF
NAME="Raven Linux"
PRETTY_NAME="Raven Linux ${RAVEN_VERSION}"
ID=raven
BUILD_ID=rolling
VERSION_ID=${RAVEN_VERSION}
VERSION="${RAVEN_VERSION} (Rolling)"
ANSI_COLOR="38;2;23;147;209"
HOME_URL="https://ravenlinux.org"
DOCUMENTATION_URL="https://docs.ravenlinux.org"
SUPPORT_URL="https://github.com/ravenlinux/ravenlinux/discussions"
BUG_REPORT_URL="https://github.com/ravenlinux/ravenlinux/issues"
LOGO=raven-logo
EOF

    # /etc/hostname
    echo "raven-linux" > "${LIVE_ROOT}/etc/hostname"

	    # /etc/hosts
	    cat > "${LIVE_ROOT}/etc/hosts" <<- EOF
	127.0.0.1   localhost
	::1         localhost
	127.0.1.1   raven-linux.localdomain raven-linux
	EOF

	    # /bin/raven-rescue: used by agetty --skip-login as a PAM-free rescue shell
	    mkdir -p "${LIVE_ROOT}/bin"
	    if [[ -f "${PROJECT_ROOT}/etc/raven/raven-rescue" ]]; then
	        cp "${PROJECT_ROOT}/etc/raven/raven-rescue" "${LIVE_ROOT}/bin/raven-rescue" 2>/dev/null || true
	    else
	        cat > "${LIVE_ROOT}/bin/raven-rescue" << 'EOF'
#!/bin/sh
# When agetty is used with --skip-login, there may be no login(1) to set up a
# sane environment. Ensure basic defaults so the shell can run external commands.
export PATH="${PATH:-/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin}"
export HOME="${HOME:-/root}"
export USER="${USER:-root}"
export LOGNAME="${LOGNAME:-root}"
export TERM="${TERM:-linux}"
export PAGER="${PAGER:-cat}"

if [ -x /bin/bash ]; then
    exec /bin/bash -l -i
fi
exec /bin/sh -i
EOF
	    fi
	    chmod 0755 "${LIVE_ROOT}/bin/raven-rescue" 2>/dev/null || true

	    # /etc/raven/init.toml (service configuration for raven-init)
	    mkdir -p "${LIVE_ROOT}/etc/raven"
	    if [[ -f "${PROJECT_ROOT}/etc/raven/init.toml" ]]; then
	        cp "${PROJECT_ROOT}/etc/raven/init.toml" "${LIVE_ROOT}/etc/raven/init.toml"
    elif [[ -f "${PROJECT_ROOT}/init/config/init.toml" ]]; then
        cp "${PROJECT_ROOT}/init/config/init.toml" "${LIVE_ROOT}/etc/raven/init.toml"
    fi
    if [[ ! -f "${LIVE_ROOT}/etc/raven/init.toml" ]]; then
        cat > "${LIVE_ROOT}/etc/raven/init.toml" << 'EOF'
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
	exec = "/bin/agetty"
	args = ["--noclear", "--skip-login", "--login-program", "/bin/raven-rescue", "tty1", "linux"]
	restart = true
	enabled = true
	critical = false

[[services]]
	name = "getty-ttyS0"
	description = "Serial console getty on ttyS0"
	exec = "/bin/agetty"
	args = ["--noclear", "--skip-login", "--login-program", "/bin/raven-rescue", "-L", "115200", "ttyS0", "vt102"]
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
    chmod 0644 "${LIVE_ROOT}/etc/raven/init.toml" 2>/dev/null || true

    # /etc/nsswitch.conf (glibc NSS defaults; required for users/dns on minimal systems)
    cat > "${LIVE_ROOT}/etc/nsswitch.conf" << 'EOF'
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
    cat > "${LIVE_ROOT}/etc/passwd" << EOF
root:x:0:0:root:/root:/bin/bash
raven:x:1000:1000:Raven User:/home/raven:/bin/bash
nobody:x:65534:65534:Nobody:/:/bin/false
EOF

    # /etc/group
    cat > "${LIVE_ROOT}/etc/group" << EOF
root:x:0:
wheel:x:10:raven
audio:x:11:raven
video:x:12:raven
users:x:100:raven
raven:x:1000:
nobody:x:65534:
EOF

    # /etc/shadow (empty passwords for live environment)
    cat > "${LIVE_ROOT}/etc/shadow" << EOF
root::0:0:99999:7:::
raven::0:0:99999:7:::
nobody:!:0:0:99999:7:::
EOF
    chmod 600 "${LIVE_ROOT}/etc/shadow"

    # /etc/shells
    cat > "${LIVE_ROOT}/etc/shells" << EOF
/bin/sh
/bin/bash
EOF

    # /etc/sudoers (wheel group allowed by default)
    mkdir -p "${LIVE_ROOT}/etc/sudoers.d"
    cat > "${LIVE_ROOT}/etc/sudoers" << 'EOF'
Defaults env_reset
Defaults !lecture

root ALL=(ALL:ALL) ALL
%wheel ALL=(ALL:ALL) ALL
EOF
    chmod 0440 "${LIVE_ROOT}/etc/sudoers" 2>/dev/null || true

    # Kernel module config directories (kmod expects these to exist)
    mkdir -p "${LIVE_ROOT}/etc/modprobe.d" "${LIVE_ROOT}/etc/modules-load.d"
    cat > "${LIVE_ROOT}/etc/modprobe.d/raven.conf" << 'EOF'
# RavenLinux kernel module configuration
# Place module options/blacklists here.
EOF

    # Realtek rtw89 (RTL8852BE) stability defaults
    cat > "${LIVE_ROOT}/etc/modprobe.d/rtw89.conf" << 'EOF'
# RavenLinux: Realtek rtw89 defaults
# RTL8852BE often fails to bring up a netdev with PCIe ASPM enabled on some laptops.
options rtw89_pci disable_aspm=1
EOF

    # rvn package manager config
    mkdir -p "${LIVE_ROOT}/etc/rvn"
    cat > "${LIVE_ROOT}/etc/rvn/config.toml" << 'EOF'
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

    # /etc/profile
    cat > "${LIVE_ROOT}/etc/profile" << 'EOF'
export PATH=/bin:/sbin:/usr/bin:/usr/sbin
export HOME="${HOME:-/root}"
export TERM="${TERM:-linux}"
export LANG=en_US.UTF-8
export EDITOR=vem
export VISUAL=vem
export RAVEN_LINUX=1

# Source bashrc for interactive bash shells
if [ -n "$BASH_VERSION" ] && [ -f /etc/bashrc ]; then
    case $- in
        *i*) . /etc/bashrc ;;
    esac
fi
EOF

    # /etc/bashrc (system-wide bash config)
    mkdir -p "${LIVE_ROOT}/etc/bash"
    if [[ -f "${PROJECT_ROOT}/configs/bash/bashrc" ]]; then
        cp "${PROJECT_ROOT}/configs/bash/bashrc" "${LIVE_ROOT}/etc/bash/bashrc"
        cp "${PROJECT_ROOT}/configs/bash/bashrc" "${LIVE_ROOT}/etc/bashrc"
    else
        cat > "${LIVE_ROOT}/etc/bashrc" << 'EOF'
# RavenLinux default bashrc (generated)
case $- in
    *i*) ;;
      *) return ;;
esac

HISTFILE=~/.bash_history
HISTSIZE=10000
HISTFILESIZE=10000
HISTCONTROL=ignoreboth:erasedups
shopt -s histappend

PS1='[\u@raven-linux]# '

alias ls='ls --color=auto'
alias ll='ls -la'
alias la='ls -A'
alias l='ls -CF'
alias grep='grep --color=auto'
alias ..='cd ..'
alias ...='cd ../..'

export PATH="$HOME/.local/bin:$PATH"
export EDITOR=vem
export VISUAL=vem
EOF
        cp "${LIVE_ROOT}/etc/bashrc" "${LIVE_ROOT}/etc/bash/bashrc"
    fi

    # Create raven user home directory
    mkdir -p "${LIVE_ROOT}/home/raven"
    cp "${LIVE_ROOT}/etc/bashrc" "${LIVE_ROOT}/home/raven/.bashrc"
    cat > "${LIVE_ROOT}/home/raven/.bash_profile" << 'EOF'
if [ -f ~/.bashrc ]; then
    . ~/.bashrc
fi
EOF
    chown -R 1000:1000 "${LIVE_ROOT}/home/raven" 2>/dev/null || true

    # Root's bashrc
    cp "${LIVE_ROOT}/etc/bashrc" "${LIVE_ROOT}/root/.bashrc"
    cp "${LIVE_ROOT}/home/raven/.bash_profile" "${LIVE_ROOT}/root/.bash_profile" 2>/dev/null || true

    log_success "Configuration files created"
}

create_init_system() {
    log_step "Creating init system..."

    # Use the properly designed init from sysroot (has shell loop to prevent PID 1 exit)
    if [[ -f "${RAVEN_BUILD}/sysroot/init" ]]; then
        cp "${RAVEN_BUILD}/sysroot/init" "${LIVE_ROOT}/init"
        chmod +x "${LIVE_ROOT}/init"
        log_success "Init system installed from sysroot"
    else
        log_fatal "Init script not found at ${RAVEN_BUILD}/sysroot/init"
    fi
}

create_installer_stub() {
    log_step "Creating installer stub..."

    cat > "${LIVE_ROOT}/bin/raven-install" << 'INSTALLER'
#!/bin/bash
# RavenLinux Installer (Text-based stub)
# Full GUI installer will be implemented separately

echo ""
echo "=========================================="
echo "  RavenLinux Installer"
echo "=========================================="
echo ""
echo "This is a placeholder for the full installer."
echo "The GUI installer is under development."
echo ""
echo "For manual installation:"
echo "  1. Partition your disk with fdisk/parted"
echo "  2. Format partitions (mkfs.ext4, mkfs.vfat)"
echo "  3. Mount target to /mnt"
echo "  4. Copy live system: cp -a /* /mnt/"
echo "  5. Install bootloader"
echo "  6. Configure fstab"
echo ""
echo "Press any key to return..."
read -n 1
INSTALLER
    chmod +x "${LIVE_ROOT}/bin/raven-install"

    log_success "Installer stub created"
}

setup_iso_structure() {
    log_step "Setting up ISO structure..."

    rm -rf "${ISO_DIR}/iso-root"
    mkdir -p "${ISO_DIR}/iso-root"/{boot/grub,EFI/BOOT,raven}

    log_success "ISO structure created"
}

create_squashfs() {
    log_step "Creating squashfs filesystem..."

    local pseudo="${ISO_DIR}/squashfs.pseudo"
    : > "${pseudo}"
    [[ -e "${LIVE_ROOT}/bin/su" ]] && echo "bin/su m 4755 0 0" >> "${pseudo}"
    [[ -e "${LIVE_ROOT}/etc/shadow" ]] && echo "etc/shadow m 600 0 0" >> "${pseudo}"

    run_logged mksquashfs "${LIVE_ROOT}" "${ISO_DIR}/iso-root/raven/filesystem.squashfs" \
        -comp zstd -Xcompression-level 15 \
        -pf "${pseudo}" -pseudo-override \
        -e bin/sudo usr/bin/sudo \
        -b 1M -no-duplicates -quiet

    log_success "Squashfs created"
}

setup_raven_bootloader() {
    log_step "Setting up RavenBoot bootloader..."

    # Copy kernel and initramfs to ISO
    cp "${LIVE_ROOT}/boot/vmlinuz" "${ISO_DIR}/iso-root/boot/vmlinuz"
    cp "${LIVE_ROOT}/boot/initramfs.img" "${ISO_DIR}/iso-root/boot/initramfs.img" 2>/dev/null || \
        cp "${RAVEN_BUILD}/initramfs-raven.img" "${ISO_DIR}/iso-root/boot/initramfs.img"

    # Create GRUB config (fallback until RavenBoot is complete)
    cat > "${ISO_DIR}/iso-root/boot/grub/grub.cfg" << EOF
set default=0
set timeout=5

insmod all_video
insmod gfxterm
terminal_output gfxterm
set gfxmode=auto
set gfxpayload=keep

# RavenLinux theme colors
set color_normal=cyan/black
set color_highlight=white/blue

menuentry "Raven Linux Live" --class raven {
    linux /boot/vmlinuz rdinit=/init quiet loglevel=3
    initrd /boot/initramfs.img
}

menuentry "Raven Linux Live (Wayland)" --class raven {
    linux /boot/vmlinuz rdinit=/init quiet loglevel=3 raven.graphics=wayland raven.wayland=raven
    initrd /boot/initramfs.img
}

menuentry "Raven Linux Live (Verbose)" --class raven {
    linux /boot/vmlinuz rdinit=/init
    initrd /boot/initramfs.img
}

menuentry "Raven Linux Install" --class raven {
    linux /boot/vmlinuz rdinit=/init raven.installer
    initrd /boot/initramfs.img
}

menuentry "Reboot" --class restart {
    reboot
}

menuentry "Shutdown" --class shutdown {
    halt
}
EOF

    # Create EFI bootloader
    if command -v grub-mkstandalone &>/dev/null; then
        run_logged grub-mkstandalone \
            --format=x86_64-efi \
            --output="${ISO_DIR}/iso-root/EFI/BOOT/BOOTX64.EFI" \
            --locales="" \
            --fonts="" \
            "boot/grub/grub.cfg=${ISO_DIR}/iso-root/boot/grub/grub.cfg" 2>/dev/null || \
            log_warn "Failed to create EFI bootloader"
    fi

    # Create EFI boot image for ISO
    mkdir -p "${ISO_DIR}/iso-root/EFI/BOOT"
    if [[ -f "${ISO_DIR}/iso-root/EFI/BOOT/BOOTX64.EFI" ]]; then
        dd if=/dev/zero of="${ISO_DIR}/iso-root/boot/efiboot.img" bs=1M count=10 2>/dev/null
        mkfs.vfat "${ISO_DIR}/iso-root/boot/efiboot.img" 2>/dev/null || true
        mmd -i "${ISO_DIR}/iso-root/boot/efiboot.img" ::/EFI 2>/dev/null || true
        mmd -i "${ISO_DIR}/iso-root/boot/efiboot.img" ::/EFI/BOOT 2>/dev/null || true
        mcopy -i "${ISO_DIR}/iso-root/boot/efiboot.img" "${ISO_DIR}/iso-root/EFI/BOOT/BOOTX64.EFI" ::/EFI/BOOT/ 2>/dev/null || true
    fi

    log_success "Bootloader configured"
}

generate_iso() {
    log_step "Generating ISO image..."

    # Create ISO with xorriso
    run_logged xorriso -as mkisofs \
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
        "${ISO_DIR}/iso-root" \
        /boot/grub/i386-pc=/usr/lib/grub/i386-pc \
        2>/dev/null || {
            # Fallback to simpler method
            log_warn "Full ISO failed, trying simpler method..."
            run_logged xorriso -as mkisofs \
                -R -J \
                -volid "${ISO_LABEL}" \
                -output "${ISO_OUTPUT}" \
                "${ISO_DIR}/iso-root"
        }

    # Generate checksums
    sha256sum "${ISO_OUTPUT}" > "${ISO_OUTPUT}.sha256"
    md5sum "${ISO_OUTPUT}" > "${ISO_OUTPUT}.md5"

    log_success "ISO generated: ${ISO_OUTPUT}"
}

print_summary() {
    local iso_size
    iso_size=$(du -h "${ISO_OUTPUT}" 2>/dev/null | cut -f1 || echo "unknown")

    log_section "RavenLinux Live ISO Build Complete"

    echo "  ISO:      ${ISO_OUTPUT}"
    echo "  Size:     ${iso_size}"
    echo "  Version:  ${RAVEN_VERSION}"
    echo "  Arch:     ${RAVEN_ARCH}"
    echo ""
    echo "  Included:"
    echo "    - Linux Kernel 6.17.11"
    echo "    - Bash (default shell)"
    echo "    - Vem (text editor)"
    echo "    - Carrion (programming language)"
    echo "    - Ivaldi (version control)"
    echo "    - rvn (package manager)"
    echo ""
    echo "  To test in QEMU (UEFI):"
    echo "    qemu-system-x86_64 -cdrom ${ISO_OUTPUT} -m 4G \\"
    echo "      -device virtio-vga-gl -display gtk,gl=on \\"
    echo "      -device usb-ehci -device usb-tablet \\"
    echo "      -serial stdio \\"
    echo "      -bios /usr/share/edk2-ovmf/x64/OVMF_CODE.4m.fd -enable-kvm"
    echo ""
    echo "  To test in QEMU (BIOS):"
    echo "    qemu-system-x86_64 -cdrom ${ISO_OUTPUT} -m 4G \\"
    echo "      -device usb-ehci -device usb-tablet -enable-kvm"
    echo ""
    echo "  To write to USB:"
    echo "    sudo dd if=${ISO_OUTPUT} of=/dev/sdX bs=4M status=progress"
    echo ""
    if is_logging_enabled; then
        echo "  Build Log: $(get_log_file)"
        echo ""
    fi
}

# =============================================================================
# Main execution
# =============================================================================

main() {
    # Initialize logging
    init_logging "build-live-iso" "RavenLinux Live ISO Builder"
    enable_logging_trap

    log_section "RavenLinux Live ISO Builder"

    echo "  Version:  ${RAVEN_VERSION}"
    echo "  Arch:     ${RAVEN_ARCH}"
    echo "  Options:"
    echo "    Skip Kernel:   ${SKIP_KERNEL}"
    echo "    Skip Packages: ${SKIP_PACKAGES}"
    echo "    Minimal:       ${MINIMAL}"
    if is_logging_enabled; then
        echo "  Log File: $(get_log_file)"
    fi
    echo ""

    check_dependencies
    setup_live_root
    copy_kernel
    copy_kernel_modules
    copy_initramfs
    copy_coreutils
    copy_sudo_rs
    install_whoami
    copy_shells
    copy_raven_packages
    copy_package_manager
    copy_diagnostics_tools
    copy_networking_tools
    copy_wayland_tools
    build_raven_desktop
    copy_desktop_services
    copy_ca_certificates
    copy_firmware
    setup_pam_and_nss
    copy_libraries
    create_config_files
    create_init_system
    create_installer_stub
    setup_iso_structure
    create_squashfs
    setup_raven_bootloader
    generate_iso
    print_summary

    finalize_logging 0
}

main "$@"
