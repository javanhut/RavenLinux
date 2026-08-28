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
    # raven-install partitions the target disk with sfdisk and clears the old
    # signatures with wipefs. stage2 copies both into the sysroot, so a build
    # host without them produces an ISO that cannot install itself.
    "sfdisk:util-linux:-:-:-:-:-:Script-driven partition table editor"
    "wipefs:util-linux:-:-:-:-:-:Filesystem signature eraser"
    "mkfs.ext4:e2fsprogs:-:-:-:-:-:Create ext4 filesystem"
    "mkfs.fat:dosfstools:-:-:-:-:-:Create FAT filesystem"
    "mcopy:mtools:-:-:-:-:-:Copy files to FAT images"
    "mmd:mtools:-:-:-:-:-:Create directories in FAT images"
    
    # Bootloader
    "grub-mkstandalone:grub:grub-efi-amd64-bin:grub2-efi-x64:grub2:grub:grub-efi:GRUB EFI image builder"

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
    # stage2 copies setfont into the sysroot; raven-console-font needs it to
    # load the PSF console font at boot.
    "setfont:kbd:console-setup:kbd:kbd:kbd:kbd:Console font loader"
)

# Optional tools. Not needed to BUILD RavenLinux -- only to boot and test what
# was built -- so these are reported separately and never counted as missing
# build dependencies. Putting them in DEPENDENCIES would paint every build host
# red for tools its build does not use.
# Format matches DEPENDENCIES: cmd:arch:debian:fedora:suse:void:alpine:description
OPTIONAL_DEPENDENCIES=(
    "qemu-system-x86_64:qemu-base:qemu-system-x86:qemu-system-x86:qemu-x86:qemu:qemu-system-x86_64:Boot the built ISO (imlazy qemu)"
    # Only needed for "raven-install --efi-nvram". The installer's default path
    # writes the fallback bootloader at \EFI\BOOT\BOOTX64.EFI, which boots
    # without touching NVRAM at all.
    "efibootmgr:efibootmgr:-:-:-:-:-:Register a UEFI NVRAM boot entry (raven-install --efi-nvram)"
    # stage4 rasterises the shipped TTF into a PSF console font with this. It is
    # optional because the build is fail-soft about it: without freetype-py the
    # ISO still boots, on the kernel's built-in 8x16 font.
    "freetype:python-freetype-py:python3-freetype:python3-freetype:python3-freetype:python3-freetype:py3-freetype:Rasterise the console font (stage4)"
)

# Optional package groups with no command of their own to probe. A QEMU built
# without a UI backend still provides qemu-system-x86_64, so `imlazy qemu-desktop`
# -- and therefore any test of the Huginn session -- fails at run time with only
# a "no graphical display backend" message to go on.
OPTIONAL_PACKAGES_ARCH="qemu-ui-gtk qemu-ui-opengl edk2-ovmf"
OPTIONAL_PACKAGES_DEBIAN="qemu-system-gui ovmf"
OPTIONAL_PACKAGES_FEDORA="qemu-ui-gtk edk2-ovmf"
OPTIONAL_PACKAGES_SUSE="qemu-ui-gtk qemu-ovmf-x86_64"
OPTIONAL_PACKAGES_VOID="qemu edk2-ovmf"
OPTIONAL_PACKAGES_ALPINE="qemu-system-x86_64 ovmf"

# Additional package groups (not command-based)
# Format: "distro:packages"
# oniguruma is needed because uutils-coreutils builds onig_sys with
# RUSTONIG_SYSTEM_LIBONIG=1 (the crate's bundled copy fails to compile with
# modern GCC), so it needs the system library plus oniguruma.pc.
# Mirrored in the Dockerfile.
# The GUI stage builds huginn, which unlike every Raven-layer component links C
# libraries: smithay binds libdrm/libgbm/libinput/libseat/libudev and Mesa
# supplies EGL. Missing them is not fatal -- stage-gui.sh checks for them and
# skips itself, producing a console-only ISO -- so they are listed with the
# rest rather than treated as a hard requirement.
#
# The last group in each list is different in kind: the icon and cursor themes
# and the fonts are not linked or compiled against anything. They are *copied
# from this host into the image*, by stage2's copy_system_utils() and by
# stage_gui_data() in stage-gui.sh. They are listed here because their absence
# is invisible at build time and produces a desktop that starts and is visibly
# broken:
#
#   adwaita cursors      what /usr/share/icons/default inherits. Without it the
#                        compositor draws no pointer over its own surfaces.
#   breeze + hicolor     what dock and launcher Icon= names resolve against.
#   dejavu + noto emoji  the only proportional and emoji faces on the image;
#                        the repo ships JetBrains Mono, which is monospace.
#
# Package names differ more here than for the libraries above, so treat the
# unfamiliar ones as descriptions to map rather than as gospel -- this check is
# advisory, and the container in the Dockerfile is the supported build path.
EXTRA_PACKAGES_ARCH="base-devel linux-headers libelf pahole python-jinja meson ninja oniguruma libdrm libinput mesa libxkbcommon wayland alsa-lib libwacom libevdev mtdev seatd parted gptfdisk efibootmgr kbd python-freetype-py adwaita-cursors breeze-icons hicolor-icon-theme ttf-dejavu noto-fonts-emoji"
EXTRA_PACKAGES_DEBIAN="build-essential linux-headers-generic libelf-dev python3-jinja2 libonig-dev libdrm-dev libinput-dev libseat-dev libgbm-dev libegl-dev libxkbcommon-dev libwayland-dev libwacom-dev libevdev-dev libmtdev-dev seatd adwaita-icon-theme breeze-icon-theme hicolor-icon-theme fonts-dejavu fonts-noto-color-emoji"
EXTRA_PACKAGES_FEDORA="kernel-devel elfutils-libelf-devel python3-jinja2 oniguruma-devel libdrm-devel libinput-devel libseat-devel mesa-libgbm-devel mesa-libEGL-devel libxkbcommon-devel wayland-devel libwacom-devel libevdev-devel mtdev-devel seatd adwaita-cursor-theme breeze-icon-theme hicolor-icon-theme dejavu-fonts-all google-noto-emoji-color-fonts"
EXTRA_PACKAGES_SUSE="kernel-devel libelf-devel python3-Jinja2 oniguruma-devel libdrm-devel libinput-devel libseat-devel Mesa-libgbm-devel Mesa-libEGL-devel libxkbcommon-devel wayland-devel libwacom-devel libevdev-devel mtdev-devel seatd adwaita-icon-theme breeze5-icons hicolor-icon-theme dejavu-fonts noto-coloremoji-fonts"
EXTRA_PACKAGES_VOID="base-devel linux-headers elfutils-devel python3-Jinja2 oniguruma-devel libdrm-devel libinput-devel seatd-devel MesaLib-devel libxkbcommon-devel wayland-devel libwacom-devel libevdev-devel mtdev-devel seatd adwaita-icon-theme breeze-icons hicolor-icon-theme dejavu-fonts-ttf noto-fonts-emoji"
EXTRA_PACKAGES_ALPINE="build-base linux-headers elfutils-dev py3-jinja2 oniguruma-dev libdrm-dev libinput-dev libseat-dev mesa-dev libxkbcommon-dev wayland-dev libwacom-dev libevdev-dev mtdev-dev seatd adwaita-icon-theme breeze-icons hicolor-icon-theme ttf-dejavu font-noto-emoji"

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
            # Prefer rustup's component list. It is authoritative for the
            # rustup-managed toolchain this build uses, and unlike
            # `rustc --print sysroot` it does not crash under qemu-user
            # emulation (running the amd64 image on an arm64 host segfaults
            # rustc, which previously made this check report a false MISSING).
            if command -v rustup &>/dev/null; then
                if rustup component list --installed 2>/dev/null | grep -q '^rust-src'; then
                    return 0
                fi
            fi
            # Fall back to known on-disk locations. RUSTUP_HOME may be a custom
            # path (the build image uses /usr/local/rustup), so search its
            # toolchains too rather than assuming ~/.rustup.
            local rust_sysroot
            rust_sysroot="$(rustc --print sysroot 2>/dev/null || true)"
            if [[ -n "$rust_sysroot" && -d "${rust_sysroot}/lib/rustlib/src/rust/library" ]]; then
                return 0
            fi
            if [[ -d "/usr/lib/rustlib/src/rust/library" ]]; then
                return 0
            fi
            if compgen -G "${RUSTUP_HOME:-$HOME/.rustup}/toolchains/*/lib/rustlib/src/rust/library" >/dev/null 2>&1; then
                return 0
            fi
            return 1
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

# Reports optional tooling without affecting the exit status.
check_optional_dependencies() {
    local distro="$1"

    [[ "$QUIET" == "true" ]] && return 0

    log_section "Optional (testing only -- not needed to build)"

    local dep_entry cmd desc pkg
    for dep_entry in "${OPTIONAL_DEPENDENCIES[@]}"; do
        IFS=':' read -r cmd _ _ _ _ _ _ desc <<< "$dep_entry"
        if check_command "$cmd"; then
            echo -e "  ${GREEN}[OK]${NC} $cmd - $desc"
        else
            pkg=$(get_package_name "$dep_entry" "$distro")
            echo -e "  ${YELLOW}[--]${NC} $cmd - $desc${pkg:+  (install: $pkg)}"
        fi
    done

    # The display backend has no command of its own: a headless QEMU still
    # ships qemu-system-x86_64, so this is the only place it can be surfaced
    # before `imlazy qemu-desktop` fails at run time.
    local optional_pkgs=""
    case "$distro" in
        arch)   optional_pkgs="$OPTIONAL_PACKAGES_ARCH" ;;
        debian) optional_pkgs="$OPTIONAL_PACKAGES_DEBIAN" ;;
        fedora) optional_pkgs="$OPTIONAL_PACKAGES_FEDORA" ;;
        suse)   optional_pkgs="$OPTIONAL_PACKAGES_SUSE" ;;
        void)   optional_pkgs="$OPTIONAL_PACKAGES_VOID" ;;
        alpine) optional_pkgs="$OPTIONAL_PACKAGES_ALPINE" ;;
    esac

    if [[ -n "$optional_pkgs" ]]; then
        echo ""
        echo -e "  For ${BOLD}imlazy qemu-desktop${NC} (the Huginn Wayland session), QEMU also needs a"
        echo -e "  display backend and UEFI firmware, which most distributions package apart:"
        echo -e "      ${CYAN}${optional_pkgs}${NC}"
    fi

    echo ""
    return 0
}

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
        check_optional_dependencies "$distro"
        exit 0
    fi

    check_optional_dependencies "$distro"
    
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
