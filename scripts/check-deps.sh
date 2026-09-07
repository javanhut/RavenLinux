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
# The GTK4 group at the end of each list is what the desktop's *applications*
# are built against, as opposed to the compositor: Files, Settings, Store,
# Power, Controls and the graphical installer are all GTK4 + libadwaita, and
# each one's stage_* function checks for gtk4/libadwaita-1/glib-2.0/gio-2.0
# with pkg-config and skips itself when they are absent. All six skip together,
# because they all fail the same check -- and install_desktop_entries then
# writes no entry for any of them, so the image boots to a launcher with
# Terminal and Crow in it and nothing else. That was the state of every ISO
# built in the container until these were added to it.
#
# The trailing runtime packages in the group are not compiled against anything.
# stage_gtk_runtime() copies them from this host into the image: the GSettings
# schemas without which every GTK application aborts on startup, the MIME
# database, glycin's out-of-process loaders and the bwrap sandbox they refuse
# to decode without, and dconf, without which settings are forgotten on exit.
#
# Package names differ more here than for the libraries above, so treat the
# unfamiliar ones as descriptions to map rather than as gospel -- this check is
# advisory, and the container in the Dockerfile is the supported build path.
EXTRA_PACKAGES_ARCH="base-devel linux-headers libelf pahole python-jinja meson ninja oniguruma libdrm libinput mesa libxkbcommon wayland alsa-lib libwacom libevdev mtdev seatd parted gptfdisk efibootmgr kbd python-freetype-py adwaita-cursors breeze-icons hicolor-icon-theme ttf-dejavu noto-fonts-emoji gtk4 libadwaita glib2-devel gsettings-desktop-schemas shared-mime-info desktop-file-utils glycin glycin-gtk4 bubblewrap librsvg dconf"
EXTRA_PACKAGES_DEBIAN="build-essential linux-headers-generic libelf-dev python3-jinja2 libonig-dev libdrm-dev libinput-dev libseat-dev libgbm-dev libegl-dev libxkbcommon-dev libwayland-dev libwacom-dev libevdev-dev libmtdev-dev seatd adwaita-icon-theme breeze-icon-theme hicolor-icon-theme fonts-dejavu fonts-noto-color-emoji libgtk-4-dev libadwaita-1-dev libglib2.0-dev gsettings-desktop-schemas shared-mime-info desktop-file-utils glycin-loaders bubblewrap librsvg2-common dconf-gsettings-backend"
EXTRA_PACKAGES_FEDORA="kernel-devel elfutils-libelf-devel python3-jinja2 oniguruma-devel libdrm-devel libinput-devel libseat-devel mesa-libgbm-devel mesa-libEGL-devel libxkbcommon-devel wayland-devel libwacom-devel libevdev-devel mtdev-devel seatd adwaita-cursor-theme breeze-icon-theme hicolor-icon-theme dejavu-fonts-all google-noto-emoji-color-fonts gtk4-devel libadwaita-devel glib2-devel gsettings-desktop-schemas shared-mime-info desktop-file-utils glycin-loaders bubblewrap librsvg2 dconf"
EXTRA_PACKAGES_SUSE="kernel-devel libelf-devel python3-Jinja2 oniguruma-devel libdrm-devel libinput-devel libseat-devel Mesa-libgbm-devel Mesa-libEGL-devel libxkbcommon-devel wayland-devel libwacom-devel libevdev-devel mtdev-devel seatd adwaita-icon-theme breeze5-icons hicolor-icon-theme dejavu-fonts noto-coloremoji-fonts gtk4-devel libadwaita-devel glib2-devel gsettings-desktop-schemas shared-mime-info desktop-file-utils bubblewrap rsvg-view dconf"
EXTRA_PACKAGES_VOID="base-devel linux-headers elfutils-devel python3-Jinja2 oniguruma-devel libdrm-devel libinput-devel seatd-devel MesaLib-devel libxkbcommon-devel wayland-devel libwacom-devel libevdev-devel mtdev-devel seatd adwaita-icon-theme breeze-icons hicolor-icon-theme dejavu-fonts-ttf noto-fonts-emoji gtk4-devel libadwaita-devel glib-devel gsettings-desktop-schemas shared-mime-info desktop-file-utils bubblewrap librsvg dconf"
EXTRA_PACKAGES_ALPINE="build-base linux-headers elfutils-dev py3-jinja2 oniguruma-dev libdrm-dev libinput-dev libseat-dev mesa-dev libxkbcommon-dev wayland-dev libwacom-dev libevdev-dev mtdev-dev seatd adwaita-icon-theme breeze-icons hicolor-icon-theme ttf-dejavu font-noto-emoji gtk4.0-dev libadwaita-dev glib-dev gsettings-desktop-schemas shared-mime-info desktop-file-utils bubblewrap librsvg dconf"

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
    - RavenLinux (rvn)
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

# Maps /etc/os-release to one of the distribution ids the tables below are
# keyed on. Three things were wrong with the way this used to work.
#
# RAVEN WAS NOT IN THE LIST. RavenLinux sets ID=raven, which matched nothing,
# so the one distribution this repository builds could not run its own
# dependency checker: "Could not detect Linux distribution", on the machine
# whose whole purpose is to build it.
#
# ID_LIKE WAS IGNORED. That field is the standard os-release answer to exactly
# this question -- Raven's says `ID_LIKE=arch` -- and consulting it is what
# stops every future derivative from needing an edit here.
#
# THE COMMAND PROBES BELOW WERE DEAD CODE. They were the `elif` arms of the
# `if [[ -f /etc/os-release ]]`, so they ran only when that file was absent --
# and it never is: every modern Linux ships one. An unrecognised ID therefore
# answered "unknown" with pacman sitting right there on the PATH. The
# os-release branch now falls through to them on no match instead of answering
# for them.
detect_distro() {
    local id="" id_like="" candidate

    if [[ -f /etc/os-release ]]; then
        # Subshell-free but scoped: this function is always called through
        # $(...), so the variables os-release sets do not escape the caller.
        # shellcheck disable=SC1091
        source /etc/os-release
        id="${ID:-}"
        id_like="${ID_LIKE:-}"
    fi

    # ID first, then each word of ID_LIKE in order. ID_LIKE is a
    # space-separated list ordered from closest relative outward
    # ("ID_LIKE=ubuntu debian"), so first match wins.
    for candidate in "$id" $id_like; do
        case "$candidate" in
            # RavenLinux is its own entry rather than an alias of arch: it
            # shares Arch's package *names* -- rvn reads pacman.conf and
            # resolves against the same repositories -- but not Arch's package
            # manager. See distro_package_family below for that split.
            raven)
                echo "raven"
                return 0
                ;;
            arch|artix|manjaro|endeavouros|garuda)
                echo "arch"
                return 0
                ;;
            debian|ubuntu|linuxmint|pop|elementary|zorin|kali)
                echo "debian"
                return 0
                ;;
            fedora|rhel|centos|rocky|alma|nobara)
                echo "fedora"
                return 0
                ;;
            opensuse*|suse|sles)
                echo "suse"
                return 0
                ;;
            void)
                echo "void"
                return 0
                ;;
            alpine)
                echo "alpine"
                return 0
                ;;
        esac
    done

    # No usable ID or ID_LIKE. Fall through to what is actually installed,
    # which is reached now that the os-release branch no longer answers
    # "unknown" on its own.
    if command -v rvn &>/dev/null; then
        echo "raven"
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

# Which set of package *names* a distribution uses, as opposed to which package
# manager installs them. The two are the same everywhere except on Raven: rvn
# parses /etc/pacman.conf and resolves against the same repositories Arch does,
# so every name in the arch column and in EXTRA_PACKAGES_ARCH is correct there
# verbatim. Mapping the family is what lets that be true without a seventh copy
# of six long package lists that would immediately start drifting from the
# arch ones they are character-for-character identical to.
distro_package_family() {
    case "$1" in
        raven) echo "arch" ;;
        *)     echo "$1" ;;
    esac
}

get_package_manager() {
    local distro="$1"
    case "$distro" in
        raven)  echo "rvn" ;;
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
        # rvn has its own verbs rather than pacman's flags, and it refreshes
        # the databases on its own unless told not to -- which is why, unlike
        # every other entry here, it needs no separate sync step in
        # install_packages below.
        raven)  echo "sudo rvn install --yes" ;;
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
    
    # The column is chosen by package family, not by distribution: Raven reads
    # the arch column because rvn resolves Arch's names.
    distro="$(distro_package_family "$distro")"

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
    local distro
    distro="$(distro_package_family "$1")"
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

# =============================================================================
# Installed-package queries
# =============================================================================
# The extra packages are not commands, so check_command cannot see them. That
# is the whole reason they are a separate list -- and until now it meant they
# were never checked at all: install_packages appended EXTRA_PACKAGES_<DISTRO>
# to the install list wholesale, so a host with every one of them already
# present was still told to install forty packages, and a host missing the ones
# that matter was told nothing in particular.
#
# It is worth being precise about what that cost. The GTK4 group lives in these
# lists, and a build host without it silently drops six applications from the
# ISO -- Files, Settings, Store, Power, Controls and the graphical installer --
# because every one of their stage_* functions in stage-gui.sh probes
# pkg-config, warns, and returns 0. The image boots to a launcher with two
# entries in it. This checker is the last place that can say so before a build
# that takes hours, and it was the one list it never looked at.
#
# The query is per distribution and runs ONCE per invocation, not once per
# package: rvn list, pacman -Qq and rpm -qa each cost roughly as much for six
# hundred packages as for one, and forty invocations of any of them is forty
# times the work for the same answer.
declare -A INSTALLED_PACKAGES=()
INSTALLED_PACKAGES_LOADED=""

# Emits the names of every installed package, one per line, bare -- no version,
# no repository prefix, no status column. Each distribution needs its own
# incantation to get there:
list_installed_packages() {
    case "$1" in
        # rvn prints "repo/name version" and appends " (dependency)" for
        # packages nothing asked for by name. Both the prefix and the suffix
        # have to come off; --explicit would drop the dependencies entirely,
        # which is wrong here -- a package pulled in as a dependency is still
        # installed, and installing it again is still a no-op.
        raven)
            rvn list 2>/dev/null | awk '{ sub(/^[^\/]*\//, "", $1); print $1 }'
            ;;
        arch)
            pacman -Qq 2>/dev/null
            ;;
        # -W alone lists packages that are merely *known*, including ones
        # removed but still holding their configuration, so the status has to
        # be filtered on rather than assumed.
        debian)
            dpkg-query -W -f='${db:Status-Status} ${Package}\n' 2>/dev/null \
                | awk '$1 == "installed" { print $2 }'
            ;;
        fedora|suse)
            rpm -qa --qf '%{NAME}\n' 2>/dev/null
            ;;
        # "ii bash-5.2.21_1 GNU Bourne Again Shell" -- the name and version are
        # one field joined by a hyphen, and the name itself may contain
        # hyphens, so it is the LAST hyphen that separates them.
        void)
            xbps-query -l 2>/dev/null | awk '{ sub(/-[^-]*$/, "", $2); print $2 }'
            ;;
        alpine)
            apk info 2>/dev/null
            ;;
    esac
}

# Fills the cache on first use. A distribution whose query prints nothing --
# because the tool is absent, or because this is a distribution the case above
# does not cover -- leaves the cache empty, and package_installed below reports
# "unknown" rather than guessing "missing": offering to install forty packages
# that are already there is the failure this function exists to end, and doing
# it because a query failed would just be the same failure with extra steps.
load_installed_packages() {
    local distro="$1" pkg

    [[ -n "$INSTALLED_PACKAGES_LOADED" ]] && return 0
    INSTALLED_PACKAGES_LOADED=1

    while IFS= read -r pkg; do
        [[ -n "$pkg" ]] && INSTALLED_PACKAGES["$pkg"]=1
    done < <(list_installed_packages "$distro")

    return 0
}

# 0 installed, 1 not installed, 2 no database to ask.
package_installed() {
    local distro="$1" pkg="$2"

    load_installed_packages "$distro"
    (( ${#INSTALLED_PACKAGES[@]} == 0 )) && return 2
    [[ -n "${INSTALLED_PACKAGES[$pkg]:-}" ]]
}

# Reports the extra packages and records the missing ones in
# MISSING_EXTRA_PACKAGES for the install offer.
#
# Deliberately does NOT affect the exit status, and deliberately contributes
# nothing to -q output. scripts/build.sh treats any -q output as a reason to
# stop and any non-zero exit as fatal, and these packages are not that: a
# missing font or icon theme is a degraded image, not a failed build, and the
# lists themselves carry names that are advisory on the less-tested
# distributions. What it will not do is let them stay invisible.
check_extra_packages() {
    local distro="$1"
    local extra
    extra="$(get_extra_packages "$distro")"

    MISSING_EXTRA_PACKAGES=()
    [[ -z "$extra" ]] && return 0

    local -a present=() missing=()
    local pkg rc

    for pkg in $extra; do
        package_installed "$distro" "$pkg" && rc=0 || rc=$?
        case "$rc" in
            0) present+=("$pkg") ;;
            1) missing+=("$pkg") ;;
            2)
                # No database. Say so once and stop pretending to check.
                if [[ "$QUIET" != "true" ]]; then
                    log_section "Extra Packages (build-host libraries, themes and fonts)"
                    log_warn "Cannot query installed packages on this system"
                    log_info "  Verify these by hand: ${extra}"
                    echo ""
                fi
                return 0
                ;;
        esac
    done

    # A package the database does not know about, whose command is sitting on
    # the PATH anyway, is its own category and must not be treated as missing.
    # RavenLinux is full of them: stage2 copies meson, ninja, parted and
    # efibootmgr into the sysroot as *files*, so an installed Raven system has
    # the tools with no package owning them -- rvn and pacman agree they are
    # not installed, and both are right.
    #
    # They are split out rather than merged into either list because getting it
    # wrong in either direction is worse than saying it. Called missing, they
    # pad the install command with packages the build does not need; and that
    # command would then FAIL rather than no-op, because installing a package
    # over unowned files on disk is a file conflict, not an upgrade. Called
    # present, a genuinely absent development package that happens to share a
    # name with a binary on the PATH would go unreported.
    local -a unowned=() absent=()
    for pkg in "${missing[@]}"; do
        if command -v "$pkg" &>/dev/null; then
            unowned+=("$pkg")
        else
            absent+=("$pkg")
        fi
    done

    MISSING_EXTRA_PACKAGES=("${absent[@]}")
    missing=("${absent[@]}")

    [[ "$QUIET" == "true" ]] && return 0

    log_section "Extra Packages (build-host libraries, themes and fonts)"

    if (( ${#missing[@]} == 0 && ${#unowned[@]} == 0 )); then
        log_success "All ${#present[@]} extra packages are installed"
        echo ""
        return 0
    fi

    echo -e "  ${GREEN}[OK]${NC} ${#present[@]} installed"
    echo ""

    local pkg
    for pkg in "${missing[@]}"; do
        echo -e "  ${YELLOW}[--]${NC} $pkg"
    done
    (( ${#missing[@]} > 0 )) && echo ""

    if (( ${#unowned[@]} > 0 )); then
        for pkg in "${unowned[@]}"; do
            echo -e "  ${GREEN}[~~]${NC} $pkg - on the PATH, but no package owns it"
        done
        echo ""
        log_info "  [~~] is fine: the build calls these by name and will find them."
        log_info "  They are left out of the command below on purpose -- installing"
        log_info "  a package over unowned files on disk is a file conflict."
        echo ""
    fi

    if (( ${#missing[@]} == 0 )); then
        return 0
    fi

    # The GTK4 group is the one whose absence has a specific, expensive and
    # entirely silent consequence, so it gets named rather than left as six
    # more lines in a list of forty.
    local -a gtk_missing=()
    for pkg in "${missing[@]}"; do
        case "$pkg" in
            gtk4*|libgtk-4*|libadwaita*|*adwaita-devel*) gtk_missing+=("$pkg") ;;
        esac
    done
    if (( ${#gtk_missing[@]} > 0 )); then
        log_warn "  Missing the GTK4 toolkit (${gtk_missing[*]})."
        log_warn "  Files, Settings, Store, Power, Controls and the graphical"
        log_warn "  installer are all GTK4 clients and will ALL be skipped by"
        log_warn "  the GUI stage, leaving an ISO whose launcher holds only the"
        log_warn "  terminal and Crow. The build will not fail; it will just"
        log_warn "  produce that image."
        echo ""
    fi

    local install_cmd
    install_cmd="$(get_install_command "$distro")"
    if [[ -n "$install_cmd" ]]; then
        echo "  Install them with:"
        echo ""
        echo -e "    ${CYAN}${install_cmd} ${missing[*]}${NC}"
        echo ""
    fi

    return 0
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
    
    # Add the extra packages that are actually absent. check_extra_packages
    # has already queried the package database and left them here; appending
    # get_extra_packages wholesale, as this used to, offered to reinstall
    # everything on a host that was already complete.
    if (( ${#MISSING_EXTRA_PACKAGES[@]} > 0 )); then
        packages+=("${MISSING_EXTRA_PACKAGES[@]}")
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
    
    # Update package database first for some distros. Raven is deliberately
    # absent: rvn refreshes the databases itself before resolving (that is what
    # its --no-sync opts out of), and it is not an alias for pacman -- adding it
    # to the arch arm would run pacman -Sy on a system that has no pacman.
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
    case "$(distro_package_family "$distro")" in
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
        log_info "Supported: RavenLinux, Arch, Debian/Ubuntu, Fedora/RHEL, openSUSE, Void, Alpine"
        exit 1
    fi
    
    log_info "Detected distribution: $distro"
    
    # Initialize missing packages arrays
    declare -a MISSING_PACKAGES=()
    declare -a MISSING_EXTRA_PACKAGES=()
    
    # Check dependencies. The extras are checked on both paths: they are the
    # list the GTK4 toolkit lives in, and a host with every command present and
    # no GTK4 is exactly the host that silently ships a two-application ISO.
    # Reporting "all dependencies are installed" over that was the false green
    # that let it happen.
    if check_dependencies "$distro"; then
        check_extra_packages "$distro"
        check_optional_dependencies "$distro"

        # Still exit 0: scripts/build.sh reads any -q output as a reason to
        # stop, and a missing font or icon theme is a degraded image rather
        # than a failed build. The report above is the point, not the status.
        if (( ${#MISSING_EXTRA_PACKAGES[@]} > 0 )); then
            log_warn "Every command dependency is present, but ${#MISSING_EXTRA_PACKAGES[@]} extra package(s) are not."
            log_info "  The build will run. See the section above for what it will leave out."
        fi
        exit 0
    fi

    check_extra_packages "$distro"
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
