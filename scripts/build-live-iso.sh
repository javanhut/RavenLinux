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
    mkdir -p "${LIVE_ROOT}"/usr/share/{fonts,icons,themes,backgrounds,zsh}
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

copy_sudo_rs() {
    log_step "Installing sudo-rs..."

    if [[ -f "${RAVEN_BUILD}/bin/sudo" ]]; then
        cp "${RAVEN_BUILD}/bin/sudo" "${LIVE_ROOT}/bin/sudo"
        chmod 4755 "${LIVE_ROOT}/bin/sudo" 2>/dev/null || chmod 755 "${LIVE_ROOT}/bin/sudo" || true
    else
        log_warn "sudo-rs not found at ${RAVEN_BUILD}/bin/sudo (run ./scripts/build.sh stage1)"
        return 0
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

    local have_zsh=false
    local have_bash=false

    # Copy zsh from host
    if command -v zsh &>/dev/null; then
        cp "$(which zsh)" "${LIVE_ROOT}/bin/zsh" && have_zsh=true

        # Copy zsh configuration files
        mkdir -p "${LIVE_ROOT}/usr/share/zsh"
        cp -r /usr/share/zsh/* "${LIVE_ROOT}/usr/share/zsh/" 2>/dev/null || true

        log_info "  Added zsh"
    fi

    # Copy bash from host
    if command -v bash &>/dev/null; then
        cp "$(which bash)" "${LIVE_ROOT}/bin/bash" && have_bash=true
        log_info "  Added bash"
    fi

    # Create sh symlink - prefer zsh, fall back to bash
    if [[ "$have_zsh" == true ]]; then
        ln -sf zsh "${LIVE_ROOT}/bin/sh"
        log_info "  /bin/sh -> zsh"
    elif [[ "$have_bash" == true ]]; then
        ln -sf bash "${LIVE_ROOT}/bin/sh"
        log_info "  /bin/sh -> bash"
    else
        log_warn "  WARNING: No shell available for /bin/sh!"
    fi

    log_success "Shells installed"
}

copy_raven_packages() {
    log_step "Copying RavenLinux custom packages..."

    local packages_bin="${RAVEN_BUILD}/packages/bin"

    if [[ -d "${packages_bin}" ]]; then
        for pkg in vem carrion ivaldi raven-installer rvn raven-dhcp; do
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

copy_networking_tools() {
    log_step "Copying networking tools..."

    # Copy essential networking tools from host
    local net_tools=(ip ping dhcpcd wpa_supplicant iw iwconfig ifconfig route netstat ss curl wget)

    for tool in "${net_tools[@]}"; do
        if command -v "$tool" &>/dev/null; then
            cp "$(which "$tool")" "${LIVE_ROOT}/bin/" 2>/dev/null || \
            cp "$(which "$tool")" "${LIVE_ROOT}/sbin/" 2>/dev/null || true
            log_info "  Added ${tool}"
        fi
    done

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

    # weston as fallback compositor
    if command -v weston &>/dev/null; then
        cp "$(which weston)" "${LIVE_ROOT}/bin/"
        log_info "  Added weston"

        # Copy weston runtime modules/backends (weston 14 uses libweston-$major)
        for d in /usr/lib/weston /usr/lib64/weston /usr/share/weston \
            /usr/lib/libweston-* /usr/lib64/libweston-*; do
            [[ -d "$d" ]] || continue
            mkdir -p "${LIVE_ROOT}${d}"
            cp -a "${d}/." "${LIVE_ROOT}${d}/" 2>/dev/null || true
            log_info "  Copied $(basename "$d") runtime data"
        done
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

    # Hyprland (optional)
    if command -v Hyprland &>/dev/null; then
        cp "$(which Hyprland)" "${LIVE_ROOT}/bin/"
        log_info "  Added Hyprland"
    fi
    if command -v hyprctl &>/dev/null; then
        cp "$(which hyprctl)" "${LIVE_ROOT}/bin/"
        log_info "  Added hyprctl"
    fi
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

    # Copy raven-compositor if built
    if [[ -f "${RAVEN_BUILD}/packages/bin/raven-compositor" ]]; then
        cp "${RAVEN_BUILD}/packages/bin/raven-compositor" "${LIVE_ROOT}/bin/"
        chmod +x "${LIVE_ROOT}/bin/raven-compositor"
        log_info "  Added raven-compositor"
    fi

    # Copy libseat library
    for lib in /usr/lib/libseat.so* /lib/libseat.so*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            mkdir -p "${LIVE_ROOT}/usr/lib"
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    # Copy libinput library
    for lib in /usr/lib/libinput.so* /lib/libinput.so*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            mkdir -p "${LIVE_ROOT}/usr/lib"
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
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

    # Copy libxkbcommon
    for lib in /usr/lib/libxkbcommon*.so*; do
        if [[ -f "$lib" ]] || [[ -L "$lib" ]]; then
            mkdir -p "${LIVE_ROOT}/usr/lib"
            cp -L "$lib" "${LIVE_ROOT}/usr/lib/" 2>/dev/null || true
        fi
    done

    log_success "Wayland tools installed"
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
    cat > "${LIVE_ROOT}/etc/hosts" << EOF
127.0.0.1   localhost
::1         localhost
127.0.1.1   raven-linux.localdomain raven-linux
EOF

    # /etc/passwd
    cat > "${LIVE_ROOT}/etc/passwd" << EOF
root:x:0:0:root:/root:/bin/zsh
raven:x:1000:1000:Raven User:/home/raven:/bin/zsh
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
/bin/zsh
EOF

    # /etc/sudoers (wheel group allowed by default)
    mkdir -p "${LIVE_ROOT}/etc/sudoers.d"
    cat > "${LIVE_ROOT}/etc/sudoers" << 'EOF'
Defaults env_reset
Defaults lecture=never

root ALL=(ALL:ALL) ALL
%wheel ALL=(ALL:ALL) ALL
EOF
    chmod 0440 "${LIVE_ROOT}/etc/sudoers" 2>/dev/null || true

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

# Source zsh config if using zsh
if [ -n "$ZSH_VERSION" ]; then
    [ -f /etc/zsh/zshrc ] && . /etc/zsh/zshrc
fi
EOF

    # /etc/zsh/zshrc (system-wide zsh config)
    mkdir -p "${LIVE_ROOT}/etc/zsh"
    cat > "${LIVE_ROOT}/etc/zsh/zshrc" << 'EOF'
# RavenLinux ZSH Configuration

# History
HISTFILE=~/.zsh_history
HISTSIZE=10000
SAVEHIST=10000
setopt SHARE_HISTORY
setopt HIST_IGNORE_DUPS

# Completion
autoload -Uz compinit
compinit

# Prompt
autoload -Uz promptinit
promptinit

# Custom prompt
PROMPT='[%n@raven-linux]# '

# Aliases
alias ls='ls --color=auto'
alias ll='ls -la'
alias la='ls -A'
alias l='ls -CF'
alias grep='grep --color=auto'
alias ..='cd ..'
alias ...='cd ../..'

# Keybindings (vim-like)
bindkey -v
bindkey '^R' history-incremental-search-backward

# Environment
export PATH=/bin:/sbin:/usr/bin:/usr/sbin:$HOME/.local/bin
export EDITOR=vem
export VISUAL=vem
EOF

    # Create raven user home directory
    mkdir -p "${LIVE_ROOT}/home/raven"
    cp "${LIVE_ROOT}/etc/zsh/zshrc" "${LIVE_ROOT}/home/raven/.zshrc"
    chown -R 1000:1000 "${LIVE_ROOT}/home/raven" 2>/dev/null || true

    # Root's zshrc
    cp "${LIVE_ROOT}/etc/zsh/zshrc" "${LIVE_ROOT}/root/.zshrc"

    log_success "Configuration files created"
}

create_init_system() {
    log_step "Creating init system..."

    # Create a proper init script for the live environment
    cat > "${LIVE_ROOT}/init" << 'INIT'
#!/bin/bash
# RavenLinux Live Init

export PATH=/bin:/sbin:/usr/bin:/usr/sbin

echo "Starting Raven Linux Live..."

# Mount essential filesystems
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev 2>/dev/null || true
mkdir -p /dev/pts /dev/shm
mount -t devpts devpts /dev/pts
mount -t tmpfs tmpfs /dev/shm
mount -t tmpfs tmpfs /tmp
mount -t tmpfs tmpfs /run

# Set hostname
hostname raven-linux

# Start udevd if available
if [ -x /sbin/udevd ]; then
    /sbin/udevd --daemon
    udevadm trigger
    udevadm settle
fi

# Configure networking (try DHCP on ethernet)
for sysiface in /sys/class/net/e*; do
    [ -d "$sysiface" ] || continue
    iface="$(basename "$sysiface")"

    if command -v raven-dhcp &>/dev/null; then
        raven-dhcp -q -i "$iface" 2>/dev/null || true
        continue
    fi

    if command -v dhcpcd &>/dev/null; then
        dhcpcd "$iface" 2>/dev/null || true
        continue
    fi

    if command -v udhcpc &>/dev/null; then
        udhcpc -i "$iface" -n -q 2>/dev/null || true
        continue
    fi
done

# Clear screen and show welcome
clear 2>/dev/null || printf '\033[2J\033[H'
printf '\033[1;36m'
cat << 'BANNER'

  ╔═══════════════════════════════════════════════════════════════════════════╗
  ║                                                                           ║
  ║    ██████╗  █████╗ ██╗   ██╗███████╗███╗   ██╗    ██╗     ██╗███╗   ██╗   ║
  ║    ██╔══██╗██╔══██╗██║   ██║██╔════╝████╗  ██║    ██║     ██║████╗  ██║   ║
  ║    ██████╔╝███████║██║   ██║█████╗  ██╔██╗ ██║    ██║     ██║██╔██╗ ██║   ║
  ║    ██╔══██╗██╔══██║╚██╗ ██╔╝██╔══╝  ██║╚██╗██║    ██║     ██║██║╚██╗██║   ║
  ║    ██║  ██║██║  ██║ ╚████╔╝ ███████╗██║ ╚████║    ███████╗██║██║ ╚████║   ║
  ║    ╚═╝  ╚═╝╚═╝  ╚═╝  ╚═══╝  ╚══════╝╚═╝  ╚═══╝    ╚══════╝╚═╝╚═╝  ╚═══╝   ║
  ║                                                                           ║
  ║                    A Developer-Focused Linux Distribution                 ║
  ║                                                                           ║
  ╚═══════════════════════════════════════════════════════════════════════════╝

BANNER
printf '\033[0m'
printf '\033[1;33m'
echo "                              Version 2025.12"
printf '\033[0m'
echo ""
printf '\033[1;37m'
echo "  ┌─────────────────────────────────────────────────────────────────────────┐"
echo "  │  BUILT-IN TOOLS:                                                        │"
echo "  │    vem        - Text editor           wifi       - WiFi manager         │"
echo "  │    carrion    - Programming language  rvn        - Package manager      │"
echo "  │    ivaldi     - Version control       raven-install - System installer  │"
echo "  └─────────────────────────────────────────────────────────────────────────┘"
printf '\033[0m'
echo ""
printf '\033[0;32m'
echo "  Type 'poweroff' to shutdown, 'reboot' to restart"
printf '\033[0m'
echo ""

# Start login shell
if [ -x /bin/zsh ]; then
    exec /bin/zsh -l
else
    exec /bin/bash -l
fi
INIT
    chmod +x "${LIVE_ROOT}/init"

    log_success "Init system created"
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

    run_logged mksquashfs "${LIVE_ROOT}" "${ISO_DIR}/iso-root/raven/filesystem.squashfs" \
        -comp zstd -Xcompression-level 15 \
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
    linux /boot/vmlinuz rdinit=/init quiet loglevel=3 raven.graphics=wayland raven.wayland=weston
    initrd /boot/initramfs.img
}

menuentry "Raven Linux Live (Wayland - Raven Compositor WIP)" --class raven {
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
    echo "    - Zsh (default shell)"
    echo "    - Vem (text editor)"
    echo "    - Carrion (programming language)"
    echo "    - Ivaldi (version control)"
    echo "    - rvn (package manager)"
    echo ""
    echo "  To test in QEMU (UEFI):"
    echo "    qemu-system-x86_64 -cdrom ${ISO_OUTPUT} -m 4G \\"
    echo "      -device virtio-vga-gl -display gtk,gl=on \\"
    echo "      -serial stdio \\"
    echo "      -bios /usr/share/edk2-ovmf/x64/OVMF_CODE.4m.fd -enable-kvm"
    echo ""
    echo "  To test in QEMU (BIOS):"
    echo "    qemu-system-x86_64 -cdrom ${ISO_OUTPUT} -m 4G -enable-kvm"
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
    copy_initramfs
    copy_coreutils
    copy_sudo_rs
    install_whoami
    copy_shells
    copy_raven_packages
    copy_package_manager
    copy_networking_tools
    copy_wayland_tools
    copy_ca_certificates
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
