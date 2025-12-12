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
        su sudo passwd login chpasswd useradd usermod groupadd
        # Archiving
        tar gzip gunzip bzip2 xz zstd unzip zip
        # Editors (fallback)
        vi nano
        # Terminal utilities
        clear reset stty tput tset
        # Locale and timezone
        locale localedef localectl timedatectl hwclock date
        # Systemd tools (if available, for compatibility)
        journalctl systemctl
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
                log_info "  Skipping ${util} (destination is a symlink)"
                continue
            fi

            cp "$src" "$dest" 2>/dev/null && log_info "  Added ${util}" || true
        fi
    done

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
    echo "raven" > "${SYSROOT_DIR}/etc/hostname"

    # /etc/hosts
    cat > "${SYSROOT_DIR}/etc/hosts" << 'EOF'
127.0.0.1   localhost
::1         localhost
127.0.1.1   raven.localdomain raven
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

PROMPT='%F{cyan}[raven%f:%F{blue}%~%f]%# '

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
