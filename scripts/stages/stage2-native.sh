#!/bin/bash
# =============================================================================
# RavenLinux Stage 2: Native System Build
# =============================================================================
# Copies host tools and libraries needed for a functional live system
# In a full LFS-style build, this would rebuild everything natively
# For now, we copy essential tools from the host system

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
BUILD_DIR="${PROJECT_ROOT}/build"
SYSROOT_DIR="${BUILD_DIR}/sysroot"
LOGS_DIR="${BUILD_DIR}/logs"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# =============================================================================
# Copy shells from host
# =============================================================================
copy_shells() {
    log_info "Copying shells..."

    # Copy zsh
    if command -v zsh &>/dev/null; then
        cp "$(which zsh)" "${SYSROOT_DIR}/bin/zsh"
        mkdir -p "${SYSROOT_DIR}/usr/share/zsh"
        cp -r /usr/share/zsh/* "${SYSROOT_DIR}/usr/share/zsh/" 2>/dev/null || true
        log_info "  Added zsh"
    fi

    # Copy bash
    if command -v bash &>/dev/null; then
        cp "$(which bash)" "${SYSROOT_DIR}/bin/bash"
        log_info "  Added bash"
    fi

    # Create sh symlink
    ln -sf zsh "${SYSROOT_DIR}/bin/sh" 2>/dev/null || ln -sf bash "${SYSROOT_DIR}/bin/sh"

    log_success "Shells installed"
}

# =============================================================================
# Copy essential system utilities from host
# =============================================================================
copy_system_utils() {
    log_info "Copying system utilities..."

    local utils=(
        # Process management
        ps kill killall pkill pgrep top htop
        # File operations
        find grep sed awk xargs file less more
        # Disk utilities
        mount umount fdisk parted mkfs.ext4 mkfs.vfat fsck blkid lsblk
        # System info
        dmesg lspci lsusb free uptime
        # User management
        su sudo passwd login
        # Archiving
        tar gzip gunzip bzip2 xz zstd
        # Editors (fallback)
        vi nano
        # Misc
        clear reset stty
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
        dhcpcd dhclient
        wpa_supplicant wpa_cli
        iw iwconfig iwlist
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

        # Get library dependencies
        timeout 2 ldd "$bin" 2>/dev/null | grep -o '/[^ ]*' | while read -r lib; do
            [[ -z "$lib" || ! -f "$lib" ]] && continue
            local dest="${SYSROOT_DIR}${lib}"
            if [[ ! -f "$dest" ]]; then
                mkdir -p "$(dirname "$dest")"
                cp -L "$lib" "$dest" 2>/dev/null && ((lib_count++)) || true
            fi
        done
    done

    # Copy dynamic linker
    for ld in /lib64/ld-linux-x86-64.so.2 /lib/ld-linux-x86-64.so.2 /lib/ld-musl-x86_64.so.1; do
        if [[ -f "$ld" ]]; then
            mkdir -p "${SYSROOT_DIR}$(dirname "$ld")"
            cp -L "$ld" "${SYSROOT_DIR}${ld}" 2>/dev/null || true
        fi
    done

    # Create lib64 symlink if needed
    if [[ -d "${SYSROOT_DIR}/lib" && ! -e "${SYSROOT_DIR}/lib64" ]]; then
        ln -sf lib "${SYSROOT_DIR}/lib64"
    fi

    log_success "Libraries copied"
}

# =============================================================================
# Create essential config files
# =============================================================================
create_configs() {
    log_info "Creating configuration files..."

    # /etc/os-release
    cat > "${SYSROOT_DIR}/etc/os-release" << 'EOF'
NAME="Raven Linux"
PRETTY_NAME="Raven Linux 2025.12"
ID=raven
BUILD_ID=rolling
VERSION_ID=2025.12
VERSION="2025.12 (Rolling)"
ANSI_COLOR="38;2;23;147;209"
HOME_URL="https://ravenlinux.org"
LOGO=raven-logo
EOF

    # /etc/hostname
    echo "raven" > "${SYSROOT_DIR}/etc/hostname"

    # /etc/hosts
    cat > "${SYSROOT_DIR}/etc/hosts" << 'EOF'
127.0.0.1   localhost
::1         localhost
127.0.1.1   raven.localdomain raven
EOF

    # /etc/passwd
    cat > "${SYSROOT_DIR}/etc/passwd" << 'EOF'
root:x:0:0:root:/root:/bin/zsh
raven:x:1000:1000:Raven User:/home/raven:/bin/zsh
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
export LANG=en_US.UTF-8
export EDITOR=vem
export VISUAL=vem
export RAVEN_LINUX=1

[ -f /etc/zsh/zshrc ] && . /etc/zsh/zshrc
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
    create_configs

    echo ""
    log_success "Stage 2 complete!"
    echo ""
}

# Run if executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
