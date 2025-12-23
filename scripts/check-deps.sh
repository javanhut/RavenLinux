#!/bin/bash
# =============================================================================
# RavenLinux Dependency Checker
# =============================================================================
# Checks for all required build dependencies and offers to install them.
# Supports: Arch Linux, Debian/Ubuntu, Fedora/RHEL, openSUSE, Void, Alpine
#
# Usage: ./scripts/check-deps.sh [OPTIONS]
#
# Options:
#   -y, --yes       Auto-install without prompting
#   -q, --quiet     Only show missing dependencies
#   -h, --help      Show this help message

set -euo pipefail

# =============================================================================
# Configuration
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RAVEN_ROOT="$(dirname "$SCRIPT_DIR")"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

# Options
AUTO_INSTALL=false
QUIET=false

# =============================================================================
# Required Dependencies
# =============================================================================

# Format: "command:arch_pkg:debian_pkg:fedora_pkg:suse_pkg:void_pkg:alpine_pkg:description"
# Use "-" if package name is same as command, use "" if not available
DEPENDENCIES=(
    # Core build tools
    "make:-:-:-:-:-:-:Build automation tool"
    "gcc:-:-:-:-:-:-:GNU C Compiler"
    "g++:gcc:g++:gcc-c++:gcc-c++:gcc:g++:GNU C++ Compiler"
    "ld:binutils:-:-:-:-:-:GNU linker"
    "ar:binutils:-:-:-:-:-:GNU archiver"
    "as:binutils:-:-:-:-:-:GNU assembler"
    "ranlib:binutils:-:-:-:-:-:Archive indexer"
    "strip:binutils:-:-:-:-:-:Strip symbols from binaries"
    
    # Archive/compression tools
    "tar:-:-:-:-:-:-:Tape archive utility"
    "gzip:-:-:-:-:-:-:GNU zip compression"
    "xz:-:xz-utils:-:-:-:-:XZ compression"
    "bzip2:-:-:-:-:-:-:Bzip2 compression"
    "cpio:-:-:-:-:-:-:Copy in/out archive tool"
    "zstd:-:-:-:-:-:-:Zstandard compression"
    "unzip:-:-:-:-:-:-:Unzip utility"
    
    # Download tools
    "curl:-:-:-:-:-:-:URL transfer tool"
    "wget:-:-:-:-:-:-:Network downloader"
    
    # Version control
    "git:-:-:-:-:-:-:Version control system"
    
    # File utilities
    "find:findutils:-:-:-:-:-:Find files utility"
    "file:-:-:-:-:-:-:File type detection"
    "patch:-:-:-:-:-:-:Apply patches to files"
    "install:coreutils:-:-:-:-:-:Install files utility"
    "rsync:-:-:-:-:-:-:Fast file copy utility"
    
    # Text processing
    "sed:-:-:-:-:-:-:Stream editor"
    "awk:gawk:-:-:-:-:-:Pattern scanning tool"
    "grep:-:-:-:-:-:-:Pattern matching"
    "diff:diffutils:-:-:-:-:-:File comparison"
    
    # Disk/filesystem tools
    "mksquashfs:squashfs-tools:-:-:-:-:-:Create squashfs images"
    "xorriso:-:-:-:-:-:-:ISO image creation"
    "losetup:util-linux:-:-:-:-:-:Loop device setup"
    "blkid:util-linux:-:-:-:-:-:Block device identification"
    "mount:util-linux:-:-:-:-:-:Mount filesystems"
    "fdisk:util-linux:-:-:-:-:-:Partition table manipulator"
    "mkfs.ext4:e2fsprogs:-:-:-:-:-:Create ext4 filesystem"
    "mkfs.fat:dosfstools:-:-:-:-:-:Create FAT filesystem"
    "mcopy:mtools:-:-:-:-:-:Copy files to FAT images"
    "mmd:mtools:-:-:-:-:-:Create directories in FAT images"
    
    # Wayland compositor and display server
    "Hyprland:hyprland:hyprland:hyprland:hyprland:hyprland:hyprland:Hyprland Wayland compositor"
    "hyprland-welcome:hyprland-guiutils:-:-:-:-:-:Hyprland GUI utilities"
    "Xwayland:xorg-xwayland:xwayland:xorg-x11-server-Xwayland:xwayland:xorg-server-xwayland:xwayland:XWayland X11 compatibility"
    "seatd:seatd:seatd:seatd:seatd:seatd:seatd:Seat management daemon"
    
    # Build systems
    "cargo:rust:-:rust-cargo:cargo:rust:rust:Rust package manager"
    "rust-src:rust-src:rustc-src:rust-src:rust-src:rust-src:rust-src:Rust source for cross-compilation"
    "go:go:golang-go:golang:go:go:go:Go programming language"
    "meson:-:-:-:-:-:-:Meson build system"
    "ninja:-:ninja-build:-:ninja:ninja:samurai:Ninja build tool"
    "cmake:-:-:-:-:-:-:CMake build system"
    "pkg-config:pkgconf:-:-:pkgconf:-:-:Package config tool"
    "autoconf:-:-:-:-:-:-:Autoconf build tool"
    "automake:-:-:-:-:-:-:Automake build tool"
    "libtool:-:-:-:-:-:-:Libtool library tool"
    "m4:-:-:-:-:-:-:M4 macro processor"
    "gettext:-:-:-:-:-:-:Internationalization tools"
    "gperf:-:-:-:-:-:-:Perfect hash function generator"
    
    # Kernel build
    "bc:-:-:-:-:-:-:Arbitrary precision calculator"
    "flex:-:-:-:-:-:-:Fast lexical analyzer"
    "bison:-:-:-:-:-:-:Parser generator"
    "perl:-:-:-:-:-:-:Perl interpreter"
    "python3:python:-:-:-:-:-:Python 3 interpreter"
    "openssl:-:-:-:-:-:-:OpenSSL toolkit"
    
    # Python modules (checked via python import)
    "jinja2:python-jinja:python3-jinja2:python3-jinja2:python3-Jinja2:python3-Jinja2:py3-jinja2:Python Jinja2 templating"
    
    # Libraries (development headers)
    "ncurses:ncurses:libncurses-dev:ncurses-devel:ncurses-devel:ncurses-devel:ncurses-dev:NCurses library"
    "ssl:openssl:libssl-dev:openssl-devel:libopenssl-devel:openssl-devel:openssl-dev:OpenSSL development files"
    "zlib:zlib:zlib1g-dev:zlib-devel:zlib-devel:zlib-devel:zlib-dev:Zlib compression library"
    "libffi:libffi:libffi-dev:libffi-devel:libffi-devel:libffi-devel:libffi-dev:Foreign function interface library"
    
    # Graphics/Wayland build dependencies
    "wayland-scanner:wayland:libwayland-dev:wayland-devel:wayland-devel:wayland-devel:wayland-dev:Wayland scanner tool"
    
    # EFI/bootloader
    "objcopy:binutils:-:-:-:-:-:Object copy utility"
    
    # Misc utilities
    "tee:coreutils:-:-:-:-:-:Tee utility"
    "timeout:coreutils:-:-:-:-:-:Timeout utility"
    "nproc:coreutils:-:-:-:-:-:CPU count utility"
    "ldd:glibc:libc-bin:glibc-common:glibc:glibc:libc-utils:Library dependency lister"
    "which:-:-:-:-:-:-:Locate commands"
    "hostname:inetutils:-:hostname:hostname:inetutils:inetutils:Hostname utility"
    "less:-:-:-:-:-:-:File pager"
    "kexec:kexec-tools:-:kexec-tools:kexec-tools:kexec-tools:kexec-tools:Kexec reboot utility"
)

# Additional package groups (not command-based)
# Format: "distro:packages"
EXTRA_PACKAGES_ARCH="base-devel linux-headers libelf pahole python-jinja meson ninja wayland-protocols libxkbcommon pixman libdrm mesa libinput seatd pango cairo gdk-pixbuf2"
EXTRA_PACKAGES_DEBIAN="build-essential linux-headers-generic libelf-dev python3-jinja2 libwayland-dev wayland-protocols libxkbcommon-dev libpixman-1-dev libdrm-dev libmesa-dev libinput-dev libseat-dev libpango1.0-dev libcairo2-dev libgdk-pixbuf2.0-dev"
EXTRA_PACKAGES_FEDORA="kernel-devel elfutils-libelf-devel python3-jinja2 wayland-devel wayland-protocols-devel libxkbcommon-devel pixman-devel libdrm-devel mesa-libGL-devel libinput-devel libseat-devel pango-devel cairo-devel gdk-pixbuf2-devel"
EXTRA_PACKAGES_SUSE="kernel-devel libelf-devel python3-Jinja2 wayland-devel wayland-protocols-devel libxkbcommon-devel pixman-devel libdrm-devel Mesa-libGL-devel libinput-devel libseat-devel pango-devel cairo-devel gdk-pixbuf-devel"
EXTRA_PACKAGES_VOID="base-devel linux-headers elfutils-devel python3-Jinja2 wayland-devel wayland-protocols libxkbcommon-devel pixman-devel libdrm-devel mesa-devel libinput-devel seatd-devel pango-devel cairo-devel gdk-pixbuf-devel"
EXTRA_PACKAGES_ALPINE="build-base linux-headers elfutils-dev py3-jinja2 wayland-dev wayland-protocols libxkbcommon-dev pixman-dev libdrm-dev mesa-dev libinput-dev seatd-dev pango-dev cairo-dev gdk-pixbuf-dev"

# =============================================================================
# Functions
# =============================================================================

show_help() {
    cat << EOF
RavenLinux Dependency Checker

Usage: $(basename "$0") [OPTIONS]

Options:
    -y, --yes       Auto-install missing dependencies without prompting
    -q, --quiet     Only show missing dependencies (no status messages)
    -h, --help      Show this help message

Supported Distributions:
    - Arch Linux (pacman)
    - Debian/Ubuntu (apt)
    - Fedora/RHEL/CentOS (dnf/yum)
    - openSUSE (zypper)
    - Void Linux (xbps)
    - Alpine Linux (apk)

Examples:
    $(basename "$0")              # Check and prompt to install
    $(basename "$0") -y           # Auto-install missing deps
    $(basename "$0") -q           # Just list missing deps
EOF
}

log_info() {
    [[ "$QUIET" == "true" ]] && return
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    [[ "$QUIET" == "true" ]] && return
    echo -e "${GREEN}[OK]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_section() {
    [[ "$QUIET" == "true" ]] && return
    echo ""
    echo -e "${BOLD}=== $1 ===${NC}"
    echo ""
}

detect_distro() {
    if [[ -f /etc/os-release ]]; then
        source /etc/os-release
        case "$ID" in
            arch|artix|manjaro|endeavouros|garuda)
                echo "arch"
                ;;
            debian|ubuntu|linuxmint|pop|elementary|zorin|kali)
                echo "debian"
                ;;
            fedora|rhel|centos|rocky|alma|nobara)
                echo "fedora"
                ;;
            opensuse*|suse|sles)
                echo "suse"
                ;;
            void)
                echo "void"
                ;;
            alpine)
                echo "alpine"
                ;;
            *)
                echo "unknown"
                ;;
        esac
    elif command -v pacman &>/dev/null; then
        echo "arch"
    elif command -v apt &>/dev/null; then
        echo "debian"
    elif command -v dnf &>/dev/null || command -v yum &>/dev/null; then
        echo "fedora"
    elif command -v zypper &>/dev/null; then
        echo "suse"
    elif command -v xbps-install &>/dev/null; then
        echo "void"
    elif command -v apk &>/dev/null; then
        echo "alpine"
    else
        echo "unknown"
    fi
}

get_package_manager() {
    local distro="$1"
    case "$distro" in
        arch)   echo "pacman" ;;
        debian) echo "apt" ;;
        fedora) echo "dnf" ;;
        suse)   echo "zypper" ;;
        void)   echo "xbps-install" ;;
        alpine) echo "apk" ;;
        *)      echo "" ;;
    esac
}

get_install_command() {
    local distro="$1"
    case "$distro" in
        arch)   echo "sudo pacman -S --needed --noconfirm" ;;
        debian) echo "sudo apt-get install -y" ;;
        fedora) echo "sudo dnf install -y" ;;
        suse)   echo "sudo zypper install -y" ;;
        void)   echo "sudo xbps-install -y" ;;
        alpine) echo "sudo apk add" ;;
        *)      echo "" ;;
    esac
}

get_package_name() {
    local dep_entry="$1"
    local distro="$2"
    
    IFS=':' read -r cmd arch_pkg debian_pkg fedora_pkg suse_pkg void_pkg alpine_pkg desc <<< "$dep_entry"
    
    local pkg=""
    case "$distro" in
        arch)   pkg="$arch_pkg" ;;
        debian) pkg="$debian_pkg" ;;
        fedora) pkg="$fedora_pkg" ;;
        suse)   pkg="$suse_pkg" ;;
        void)   pkg="$void_pkg" ;;
        alpine) pkg="$alpine_pkg" ;;
    esac
    
    # "-" means use command name as package name
    if [[ "$pkg" == "-" ]]; then
        pkg="$cmd"
    fi
    
    echo "$pkg"
}

check_command() {
    local cmd="$1"
    
    # Special cases for library/header/module checks
    case "$cmd" in
        # Python modules
        jinja2)
            python3 -c "import jinja2" &>/dev/null
            return $?
            ;;
        
        # Library header checks
        ncurses)
            [[ -f /usr/include/ncurses.h ]] || [[ -f /usr/include/ncursesw/ncurses.h ]]
            return $?
            ;;
        ssl)
            [[ -f /usr/include/openssl/ssl.h ]] || pkg-config --exists openssl &>/dev/null
            return $?
            ;;
        zlib)
            [[ -f /usr/include/zlib.h ]] || pkg-config --exists zlib &>/dev/null
            return $?
            ;;
        libffi)
            [[ -f /usr/include/ffi.h ]] || pkg-config --exists libffi &>/dev/null
            return $?
            ;;
        wayland-scanner)
            command -v wayland-scanner &>/dev/null || pkg-config --exists wayland-scanner &>/dev/null
            return $?
            ;;
        
        # Rust source (needed for UEFI cross-compilation)
        rust-src)
            # Check common locations for rust-src
            local rust_sysroot
            rust_sysroot="$(rustc --print sysroot 2>/dev/null)" || return 1
            [[ -d "${rust_sysroot}/lib/rustlib/src/rust/library" ]] || \
            [[ -d "/usr/lib/rustlib/src/rust/library" ]] || \
            [[ -d "${HOME}/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library" ]]
            return $?
            ;;
        
        # Standard command check
        *)
            command -v "$cmd" &>/dev/null
            ;;
    esac
}

check_dependencies() {
    local distro="$1"
    local -a missing_cmds=()
    local -a missing_pkgs=()
    local -a found_cmds=()
    
    log_section "Checking Build Dependencies"
    
    for dep_entry in "${DEPENDENCIES[@]}"; do
        IFS=':' read -r cmd arch_pkg debian_pkg fedora_pkg suse_pkg void_pkg alpine_pkg desc <<< "$dep_entry"
        
        if check_command "$cmd"; then
            found_cmds+=("$cmd")
            [[ "$QUIET" != "true" ]] && echo -e "  ${GREEN}[OK]${NC} $cmd - $desc"
        else
            missing_cmds+=("$cmd")
            local pkg
            pkg=$(get_package_name "$dep_entry" "$distro")
            if [[ -n "$pkg" ]]; then
                missing_pkgs+=("$pkg")
            fi
            echo -e "  ${RED}[MISSING]${NC} $cmd - $desc"
        fi
    done
    
    echo ""
    
    if [[ ${#missing_cmds[@]} -eq 0 ]]; then
        log_success "All ${#found_cmds[@]} dependencies are installed!"
        return 0
    else
        log_warn "Missing ${#missing_cmds[@]} dependencies"
        
        # Remove duplicates from missing packages
        local -a unique_pkgs=()
        declare -A seen
        for pkg in "${missing_pkgs[@]}"; do
            if [[ -n "$pkg" && -z "${seen[$pkg]:-}" ]]; then
                seen[$pkg]=1
                unique_pkgs+=("$pkg")
            fi
        done
        
        # Store for later use
        MISSING_PACKAGES=("${unique_pkgs[@]}")
        return 1
    fi
}

get_extra_packages() {
    local distro="$1"
    case "$distro" in
        arch)   echo "$EXTRA_PACKAGES_ARCH" ;;
        debian) echo "$EXTRA_PACKAGES_DEBIAN" ;;
        fedora) echo "$EXTRA_PACKAGES_FEDORA" ;;
        suse)   echo "$EXTRA_PACKAGES_SUSE" ;;
        void)   echo "$EXTRA_PACKAGES_VOID" ;;
        alpine) echo "$EXTRA_PACKAGES_ALPINE" ;;
        *)      echo "" ;;
    esac
}

install_packages() {
    local distro="$1"
    shift
    local packages=("$@")
    
    if [[ ${#packages[@]} -eq 0 ]]; then
        log_info "No packages to install"
        return 0
    fi
    
    local install_cmd
    install_cmd=$(get_install_command "$distro")
    
    if [[ -z "$install_cmd" ]]; then
        log_error "Unknown distribution, cannot install packages automatically"
        log_info "Please install these packages manually: ${packages[*]}"
        return 1
    fi
    
    # Add extra packages for development
    local extra
    extra=$(get_extra_packages "$distro")
    if [[ -n "$extra" ]]; then
        packages+=($extra)
    fi
    
    # Remove duplicates
    local -a unique_pkgs=()
    declare -A seen
    for pkg in "${packages[@]}"; do
        if [[ -n "$pkg" && -z "${seen[$pkg]:-}" ]]; then
            seen[$pkg]=1
            unique_pkgs+=("$pkg")
        fi
    done
    
    log_section "Installing Packages"
    
    echo "The following packages will be installed:"
    echo ""
    for pkg in "${unique_pkgs[@]}"; do
        echo "  - $pkg"
    done
    echo ""
    
    if [[ "$AUTO_INSTALL" != "true" ]]; then
        read -p "Do you want to install these packages? [y/N] " -n 1 -r
        echo ""
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            log_info "Installation cancelled"
            return 1
        fi
    fi
    
    log_info "Running: $install_cmd ${unique_pkgs[*]}"
    echo ""
    
    # Update package database first for some distros
    case "$distro" in
        debian)
            sudo apt-get update
            ;;
        arch)
            sudo pacman -Sy
            ;;
    esac
    
    if $install_cmd "${unique_pkgs[@]}"; then
        log_success "Packages installed successfully"
        return 0
    else
        log_error "Failed to install some packages"
        return 1
    fi
}

print_summary() {
    local distro="$1"
    local pkg_manager
    pkg_manager=$(get_package_manager "$distro")
    
    log_section "Summary"
    
    echo "  Distribution: $distro"
    echo "  Package Manager: $pkg_manager"
    echo ""
    
    if [[ ${#MISSING_PACKAGES[@]} -gt 0 ]]; then
        echo "  To install missing dependencies manually:"
        echo ""
        local install_cmd
        install_cmd=$(get_install_command "$distro")
        echo "    $install_cmd ${MISSING_PACKAGES[*]}"
        echo ""
    fi
}

# =============================================================================
# Main
# =============================================================================

main() {
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -y|--yes)
                AUTO_INSTALL=true
                shift
                ;;
            -q|--quiet)
                QUIET=true
                shift
                ;;
            -h|--help)
                show_help
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                show_help
                exit 1
                ;;
        esac
    done
    
    [[ "$QUIET" != "true" ]] && echo ""
    [[ "$QUIET" != "true" ]] && echo -e "${BOLD}${CYAN}RavenLinux Dependency Checker${NC}"
    [[ "$QUIET" != "true" ]] && echo ""
    
    # Detect distribution
    local distro
    distro=$(detect_distro)
    
    if [[ "$distro" == "unknown" ]]; then
        log_error "Could not detect Linux distribution"
        log_info "Supported: Arch, Debian/Ubuntu, Fedora/RHEL, openSUSE, Void, Alpine"
        exit 1
    fi
    
    log_info "Detected distribution: $distro"
    
    # Initialize missing packages array
    declare -a MISSING_PACKAGES=()
    
    # Check dependencies
    if check_dependencies "$distro"; then
        exit 0
    fi
    
    # Offer to install missing packages
    if [[ ${#MISSING_PACKAGES[@]} -gt 0 ]]; then
        print_summary "$distro"
        
        if [[ "$QUIET" == "true" ]]; then
            # Just list missing packages
            echo "${MISSING_PACKAGES[*]}"
            exit 1
        fi
        
        if install_packages "$distro" "${MISSING_PACKAGES[@]}"; then
            echo ""
            log_success "All dependencies should now be installed"
            log_info "You can now run: ./scripts/build.sh"
        else
            exit 1
        fi
    fi
}

main "$@"
