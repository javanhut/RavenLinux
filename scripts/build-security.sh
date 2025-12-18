#!/bin/bash
# =============================================================================
# RavenLinux Security Packages Build Script
# =============================================================================
# Builds packages required for user/group management and session handling:
# - glib2       (core library with headers/pkgconfig)
# - duktape     (JavaScript engine for polkit)
# - elogind     (session/seat management)
# - polkit      (authorization framework)
# - accountsservice (user account D-Bus service)
#
# Build Order (dependency chain):
#   1. glib2 -> 2. duktape -> 3. elogind -> 4. polkit -> 5. accountsservice
#
# Usage: ./scripts/build-security.sh [PACKAGE]
#   PACKAGE: all (default), glib2, duktape, elogind, polkit, accountsservice

set -euo pipefail

# =============================================================================
# Environment Setup
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${RAVEN_ROOT:-$(dirname "$SCRIPT_DIR")}"
BUILD_DIR="${RAVEN_BUILD:-${PROJECT_ROOT}/build}"
SYSROOT_DIR="${SYSROOT_DIR:-${BUILD_DIR}/sysroot}"
SOURCES_DIR="${SOURCES_DIR:-${BUILD_DIR}/sources}"
LOGS_DIR="${LOGS_DIR:-${BUILD_DIR}/logs}"
TOOLCHAIN_DIR="${TOOLCHAIN_DIR:-${BUILD_DIR}/toolchain}"

# Package versions (from BLFS 12.4)
GLIB2_VERSION="2.86.3"
DUKTAPE_VERSION="2.7.0"
ELOGIND_VERSION="255.17"
POLKIT_VERSION="126"
ACCOUNTSSERVICE_VERSION="23.13.9"

# Build settings
JOBS="${RAVEN_JOBS:-$(nproc)}"
TARGET="${RAVEN_TARGET:-x86_64-raven-linux-musl}"

# =============================================================================
# Logging
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
    BOLD='\033[1m'
    NC='\033[0m'
    log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
    log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
    log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
    log_error() { echo -e "${RED}[ERROR]${NC} $1" >&2; }
    log_fatal() { log_error "$1"; exit 1; }
    log_step() { echo -e "${CYAN}[STEP]${NC} $1"; }
    log_section() { echo -e "\n${BOLD}========================================${NC}\n${BOLD}  $1${NC}\n${BOLD}========================================${NC}\n"; }
    run_logged() { "$@"; }
fi

# =============================================================================
# System Setup and Dependencies
# =============================================================================

# Check and install required system dependencies
check_system_dependencies() {
    log_step "Checking system dependencies..."

    local missing_deps=()

    # Required commands
    local required_cmds=(meson ninja curl tar pkg-config python3)
    for cmd in "${required_cmds[@]}"; do
        if ! command -v "$cmd" &>/dev/null; then
            missing_deps+=("$cmd")
        fi
    done

    # Required Python modules
    if ! python3 -c "import jinja2" &>/dev/null; then
        log_warn "Python jinja2 module not found"
        missing_deps+=("python-jinja (or python-jinja2)")
    fi

    if [[ ${#missing_deps[@]} -gt 0 ]]; then
        log_error "Missing system dependencies:"
        for dep in "${missing_deps[@]}"; do
            log_error "  - $dep"
        done
        log_error ""
        log_error "Please install them using your package manager, e.g.:"
        log_error "  sudo pacman -S meson ninja curl python-jinja"
        return 1
    fi

    log_success "All system dependencies found"
}

# Setup symlinks for glib2 tools so meson/pkg-config can find them
setup_glib_tools() {
    log_step "Setting up glib2 tool symlinks..."

    local tools=(glib-mkenums glib-genmarshal gdbus-codegen glib-compile-resources glib-compile-schemas)
    local need_sudo=false

    for tool in "${tools[@]}"; do
        local sysroot_tool="${SYSROOT_DIR}/usr/bin/${tool}"
        local system_tool="/usr/bin/${tool}"

        if [[ -f "${sysroot_tool}" ]]; then
            # Check if system tool exists and is not our symlink
            if [[ -L "${system_tool}" ]]; then
                local current_target
                current_target=$(readlink -f "${system_tool}" 2>/dev/null || true)
                if [[ "${current_target}" == "${sysroot_tool}" ]]; then
                    log_info "  ${tool}: symlink already correct"
                    continue
                fi
            fi

            if [[ ! -e "${system_tool}" ]] || [[ -L "${system_tool}" ]]; then
                # Try to create symlink (may need sudo)
                if ln -sf "${sysroot_tool}" "${system_tool}" 2>/dev/null; then
                    log_info "  ${tool}: symlink created"
                else
                    need_sudo=true
                    log_warn "  ${tool}: needs sudo to create symlink"
                fi
            else
                log_info "  ${tool}: system version exists, skipping"
            fi
        fi
    done

    # Setup codegen Python module directory
    local codegen_src="${SYSROOT_DIR}/usr/share/glib-2.0/codegen"
    local codegen_dst="/usr/share/glib-2.0/codegen"
    local glib_share_src="${SYSROOT_DIR}/usr/share/glib-2.0"
    local glib_share_dst="/usr/share/glib-2.0"

    if [[ -d "${codegen_src}" ]]; then
        # Remove any circular/broken symlinks inside the codegen source directory
        if [[ -L "${codegen_src}/codegen" ]]; then
            rm -f "${codegen_src}/codegen" 2>/dev/null || true
            log_info "  Removed circular symlink in codegen directory"
        fi

        mkdir -p "${glib_share_dst}" 2>/dev/null || true

        # Remove existing codegen (could be stale symlink or wrong version)
        if [[ -e "${codegen_dst}" ]] || [[ -L "${codegen_dst}" ]]; then
            rm -rf "${codegen_dst}" 2>/dev/null || true
        fi

        if ln -sf "${codegen_src}" "${codegen_dst}" 2>/dev/null; then
            log_info "  codegen module: symlink created"
            # Clear Python cache to avoid stale bytecode
            rm -rf "${codegen_src}/__pycache__" 2>/dev/null || true
        else
            need_sudo=true
            log_warn "  codegen module: needs sudo to create symlink"
        fi
    fi

    # Export PYTHONPATH to ensure gdbus-codegen finds the right modules
    export PYTHONPATH="${glib_share_src}:${PYTHONPATH:-}"

    if $need_sudo; then
        log_warn ""
        log_warn "Some symlinks could not be created. Please run with sudo or manually create:"
        for tool in "${tools[@]}"; do
            local sysroot_tool="${SYSROOT_DIR}/usr/bin/${tool}"
            if [[ -f "${sysroot_tool}" ]] && [[ ! -L "/usr/bin/${tool}" ]]; then
                log_warn "  sudo ln -sf ${sysroot_tool} /usr/bin/${tool}"
            fi
        done
        if [[ -d "${codegen_src}" ]] && [[ ! -L "${codegen_dst}" ]]; then
            log_warn "  sudo mkdir -p /usr/share/glib-2.0"
            log_warn "  sudo ln -sf ${codegen_src} ${codegen_dst}"
        fi
        return 1
    fi

    log_success "glib2 tools setup complete"
}

# =============================================================================
# Utility Functions
# =============================================================================

download_source() {
    local name="$1"
    local url="$2"
    local filename="${3:-$(basename "$url")}"

    mkdir -p "${SOURCES_DIR}"

    if [[ ! -f "${SOURCES_DIR}/${filename}" ]]; then
        log_info "Downloading ${name}..."
        curl -L -o "${SOURCES_DIR}/${filename}" "$url" || {
            log_error "Failed to download ${name}"
            return 1
        }
    else
        log_info "${name} already downloaded"
    fi
}

extract_source() {
    local archive="$1"
    local dest_name="$2"

    if [[ -d "${SOURCES_DIR}/${dest_name}" ]]; then
        log_info "${dest_name} already extracted"
        return 0
    fi

    log_info "Extracting ${archive}..."
    cd "${SOURCES_DIR}"

    case "$archive" in
        *.tar.gz|*.tgz)
            tar -xzf "${archive}"
            ;;
        *.tar.xz)
            tar -xJf "${archive}"
            ;;
        *.tar.bz2)
            tar -xjf "${archive}"
            ;;
        *)
            log_fatal "Unknown archive format: ${archive}"
            ;;
    esac

    cd "${PROJECT_ROOT}"
}

check_command() {
    local cmd="$1"
    if ! command -v "$cmd" &>/dev/null; then
        log_fatal "Required command not found: $cmd"
    fi
}

# =============================================================================
# Build: glib2
# =============================================================================

build_glib2() {
    log_section "Building GLib2 ${GLIB2_VERSION}"

    local src_dir="${SOURCES_DIR}/glib-${GLIB2_VERSION}"
    local build_dir="${src_dir}/build"
    local log_file="${LOGS_DIR}/glib2.log"

    # Check if already installed with headers
    if [[ -f "${SYSROOT_DIR}/usr/include/glib-2.0/glib.h" ]] && \
       [[ -f "${SYSROOT_DIR}/usr/lib/pkgconfig/glib-2.0.pc" ]]; then
        log_info "GLib2 already installed with headers, skipping"
        return 0
    fi

    check_command meson
    check_command ninja

    # Clean up existing glib2 files that may conflict (non-symlink libraries)
    log_step "Cleaning up existing glib2 files..."
    rm -f "${SYSROOT_DIR}/usr/lib/libglib-2.0.so"* 2>/dev/null || true
    rm -f "${SYSROOT_DIR}/usr/lib/libgio-2.0.so"* 2>/dev/null || true
    rm -f "${SYSROOT_DIR}/usr/lib/libgobject-2.0.so"* 2>/dev/null || true
    rm -f "${SYSROOT_DIR}/usr/lib/libgmodule-2.0.so"* 2>/dev/null || true
    rm -f "${SYSROOT_DIR}/usr/lib/libgthread-2.0.so"* 2>/dev/null || true
    rm -rf "${SYSROOT_DIR}/usr/include/glib-2.0" 2>/dev/null || true
    rm -rf "${SYSROOT_DIR}/usr/include/gio-unix-2.0" 2>/dev/null || true
    rm -rf "${SYSROOT_DIR}/usr/lib/glib-2.0" 2>/dev/null || true

    # Download
    download_source "glib2" \
        "https://download.gnome.org/sources/glib/2.86/glib-${GLIB2_VERSION}.tar.xz" \
        "glib-${GLIB2_VERSION}.tar.xz"

    # Extract
    extract_source "glib-${GLIB2_VERSION}.tar.xz" "glib-${GLIB2_VERSION}"

    # Build
    log_step "Configuring GLib2..."
    cd "${src_dir}"
    rm -rf build
    mkdir -p build
    cd build

    meson setup .. \
        --prefix=/usr \
        --buildtype=release \
        -D introspection=disabled \
        -D glib_debug=disabled \
        -D man-pages=disabled \
        -D sysprof=disabled \
        -D tests=false \
        2>&1 | tee "${log_file}"

    log_step "Compiling GLib2..."
    ninja -j${JOBS} 2>&1 | tee -a "${log_file}"

    log_step "Installing GLib2 to sysroot..."
    DESTDIR="${SYSROOT_DIR}" ninja install 2>&1 | tee -a "${log_file}"

    cd "${PROJECT_ROOT}"
    log_success "GLib2 ${GLIB2_VERSION} built and installed"
}

# =============================================================================
# Build: duktape
# =============================================================================

build_duktape() {
    log_section "Building Duktape ${DUKTAPE_VERSION}"

    local src_dir="${SOURCES_DIR}/duktape-${DUKTAPE_VERSION}"
    local log_file="${LOGS_DIR}/duktape.log"

    # Check if already installed
    if [[ -f "${SYSROOT_DIR}/usr/lib/libduktape.so" ]] && \
       [[ -f "${SYSROOT_DIR}/usr/include/duktape.h" ]]; then
        log_info "Duktape already installed, skipping"
        return 0
    fi

    # Download
    download_source "duktape" \
        "https://duktape.org/duktape-${DUKTAPE_VERSION}.tar.xz" \
        "duktape-${DUKTAPE_VERSION}.tar.xz"

    # Extract
    extract_source "duktape-${DUKTAPE_VERSION}.tar.xz" "duktape-${DUKTAPE_VERSION}"

    # Build
    log_step "Building Duktape..."
    cd "${src_dir}"

    # Fix optimization flag
    sed -i 's/-Os/-O2/' Makefile.sharedlibrary

    make -f Makefile.sharedlibrary INSTALL_PREFIX=/usr -j${JOBS} 2>&1 | tee "${log_file}"

    log_step "Installing Duktape to sysroot..."
    make -f Makefile.sharedlibrary INSTALL_PREFIX=/usr DESTDIR="${SYSROOT_DIR}" install 2>&1 | tee -a "${log_file}"

    # Create pkgconfig file
    mkdir -p "${SYSROOT_DIR}/usr/lib/pkgconfig"
    cat > "${SYSROOT_DIR}/usr/lib/pkgconfig/duktape.pc" << EOF
prefix=/usr
exec_prefix=\${prefix}
libdir=\${exec_prefix}/lib
includedir=\${prefix}/include

Name: duktape
Description: Embeddable JavaScript engine
Version: ${DUKTAPE_VERSION}
Libs: -L\${libdir} -lduktape -lm
Cflags: -I\${includedir}
EOF

    cd "${PROJECT_ROOT}"
    log_success "Duktape ${DUKTAPE_VERSION} built and installed"
}

# =============================================================================
# Build: elogind
# =============================================================================

build_elogind() {
    log_section "Building elogind ${ELOGIND_VERSION}"

    local src_dir="${SOURCES_DIR}/elogind-${ELOGIND_VERSION}"
    local build_dir="${src_dir}/build"
    local log_file="${LOGS_DIR}/elogind.log"

    # Check if already installed
    if [[ -f "${SYSROOT_DIR}/usr/lib/libelogind.so" ]] && \
       [[ -f "${SYSROOT_DIR}/usr/bin/loginctl" ]]; then
        log_info "elogind already installed, skipping"
        return 0
    fi

    check_command meson
    check_command ninja

    # Download
    download_source "elogind" \
        "https://github.com/elogind/elogind/archive/refs/tags/v${ELOGIND_VERSION}.tar.gz" \
        "elogind-${ELOGIND_VERSION}.tar.gz"

    # Extract
    extract_source "elogind-${ELOGIND_VERSION}.tar.gz" "elogind-${ELOGIND_VERSION}"

    # Build
    log_step "Configuring elogind..."
    cd "${src_dir}"
    rm -rf build
    mkdir -p build
    cd build

    meson setup .. \
        --prefix=/usr \
        --buildtype=release \
        -D man=false \
        -D docdir=/usr/share/doc/elogind-${ELOGIND_VERSION} \
        -D cgroup-controller=elogind \
        -D dev-kvm-mode=0660 \
        -D dbuspolicydir=/etc/dbus-1/system.d \
        -D default-kill-user-processes=false \
        2>&1 | tee "${log_file}"

    log_step "Compiling elogind..."
    ninja -j${JOBS} 2>&1 | tee -a "${log_file}"

    log_step "Installing elogind to sysroot..."
    DESTDIR="${SYSROOT_DIR}" ninja install 2>&1 | tee -a "${log_file}"

    # Create compatibility symlinks
    ln -sfv libelogind.pc "${SYSROOT_DIR}/usr/lib/pkgconfig/libsystemd.pc" 2>/dev/null || true
    ln -sfvn elogind "${SYSROOT_DIR}/usr/include/systemd" 2>/dev/null || true

    # Create PAM configuration for elogind
    log_step "Configuring PAM for elogind..."
    mkdir -p "${SYSROOT_DIR}/etc/pam.d"

    # Add elogind to system-session if it exists
    if [[ -f "${SYSROOT_DIR}/etc/pam.d/system-session" ]]; then
        if ! grep -q "pam_elogind.so" "${SYSROOT_DIR}/etc/pam.d/system-session"; then
            cat >> "${SYSROOT_DIR}/etc/pam.d/system-session" << 'EOF'

# Begin elogind addition
session  required    pam_loginuid.so
session  optional    pam_elogind.so
# End elogind addition
EOF
        fi
    fi

    # Create elogind-user PAM file
    cat > "${SYSROOT_DIR}/etc/pam.d/elogind-user" << 'EOF'
# Begin /etc/pam.d/elogind-user
account  required    pam_access.so
account  include     system-account

session  required    pam_env.so
session  required    pam_limits.so
session  required    pam_unix.so
session  required    pam_loginuid.so
session  optional    pam_elogind.so

auth     required    pam_deny.so
password required    pam_deny.so
# End /etc/pam.d/elogind-user
EOF

    cd "${PROJECT_ROOT}"
    log_success "elogind ${ELOGIND_VERSION} built and installed"
}

# =============================================================================
# Build: polkit
# =============================================================================

build_polkit() {
    log_section "Building Polkit ${POLKIT_VERSION}"

    local src_dir="${SOURCES_DIR}/polkit-${POLKIT_VERSION}"
    local build_dir="${src_dir}/build"
    local log_file="${LOGS_DIR}/polkit.log"

    # Check if already installed
    if [[ -f "${SYSROOT_DIR}/usr/lib/libpolkit-gobject-1.so" ]] && \
       [[ -f "${SYSROOT_DIR}/usr/bin/pkexec" ]]; then
        log_info "Polkit already installed, skipping"
        return 0
    fi

    # Check dependencies
    if [[ ! -f "${SYSROOT_DIR}/usr/lib/libduktape.so" ]]; then
        log_fatal "Polkit requires duktape - build duktape first"
    fi
    if [[ ! -f "${SYSROOT_DIR}/usr/lib/libelogind.so" ]]; then
        log_fatal "Polkit requires elogind - build elogind first"
    fi
    if [[ ! -f "${SYSROOT_DIR}/usr/lib/libglib-2.0.so" ]]; then
        log_fatal "Polkit requires glib2 - build glib2 first"
    fi

    # Ensure glib tools are available
    setup_glib_tools || {
        log_error "Failed to setup glib tools. Run script with sudo."
        return 1
    }

    check_command meson
    check_command ninja

    # Download
    download_source "polkit" \
        "https://github.com/polkit-org/polkit/archive/refs/tags/${POLKIT_VERSION}.tar.gz" \
        "polkit-${POLKIT_VERSION}.tar.gz"

    # Extract
    extract_source "polkit-${POLKIT_VERSION}.tar.gz" "polkit-${POLKIT_VERSION}"

    # Create polkitd user/group in sysroot
    log_step "Creating polkitd user/group..."
    if ! grep -q "^polkitd:" "${SYSROOT_DIR}/etc/group" 2>/dev/null; then
        echo "polkitd:x:27:" >> "${SYSROOT_DIR}/etc/group"
    fi
    if ! grep -q "^polkitd:" "${SYSROOT_DIR}/etc/passwd" 2>/dev/null; then
        echo "polkitd:x:27:27:PolicyKit Daemon Owner:/etc/polkit-1:/bin/false" >> "${SYSROOT_DIR}/etc/passwd"
    fi

    # Build
    log_step "Configuring Polkit..."
    cd "${src_dir}"
    rm -rf build
    mkdir -p build
    cd build

    # Set PKG_CONFIG_PATH to find our installed libs
    # Add sysroot bin to PATH for glib tools (glib-mkenums, glib-genmarshal, etc.)
    # Add sysroot lib to linker path for libelogind
    export PKG_CONFIG_PATH="${SYSROOT_DIR}/usr/lib/pkgconfig:${PKG_CONFIG_PATH:-}"
    export PATH="${SYSROOT_DIR}/usr/bin:${PATH}"
    export LIBRARY_PATH="${SYSROOT_DIR}/usr/lib:${LIBRARY_PATH:-}"
    export LD_LIBRARY_PATH="${SYSROOT_DIR}/usr/lib:${LD_LIBRARY_PATH:-}"

    # Build link arguments for meson - pass library path to linker
    local link_args="-L${SYSROOT_DIR}/usr/lib -Wl,-rpath-link,${SYSROOT_DIR}/usr/lib"

    meson setup .. \
        --prefix=/usr \
        --buildtype=release \
        -D man=false \
        -D session_tracking=elogind \
        -D systemdsystemunitdir=/tmp \
        -D tests=false \
        -D gtk_doc=false \
        -D introspection=false \
        -D os_type=lfs \
        -D c_link_args="${link_args}" \
        -D cpp_link_args="${link_args}" \
        2>&1 | tee "${log_file}"

    log_step "Compiling Polkit..."
    ninja -j${JOBS} 2>&1 | tee -a "${log_file}"

    log_step "Installing Polkit to sysroot..."
    DESTDIR="${SYSROOT_DIR}" ninja install 2>&1 | tee -a "${log_file}"

    # Clean up systemd unit files
    rm -rf "${SYSROOT_DIR}/tmp/"*.service 2>/dev/null || true

    # Create polkit rules directory
    mkdir -p "${SYSROOT_DIR}/etc/polkit-1/rules.d"
    mkdir -p "${SYSROOT_DIR}/usr/share/polkit-1/rules.d"

    cd "${PROJECT_ROOT}"
    log_success "Polkit ${POLKIT_VERSION} built and installed"
}

# =============================================================================
# Build: accountsservice
# =============================================================================

build_accountsservice() {
    log_section "Building AccountsService ${ACCOUNTSSERVICE_VERSION}"

    local src_dir="${SOURCES_DIR}/accountsservice-${ACCOUNTSSERVICE_VERSION}"
    local build_dir="${src_dir}/build"
    local log_file="${LOGS_DIR}/accountsservice.log"

    # Check if already installed
    if [[ -f "${SYSROOT_DIR}/usr/lib/libaccountsservice.so" ]]; then
        log_info "AccountsService already installed, skipping"
        return 0
    fi

    # Check dependencies
    if [[ ! -f "${SYSROOT_DIR}/usr/lib/libpolkit-gobject-1.so" ]]; then
        log_fatal "AccountsService requires polkit - build polkit first"
    fi
    if [[ ! -f "${SYSROOT_DIR}/usr/lib/libglib-2.0.so" ]]; then
        log_fatal "AccountsService requires glib2 - build glib2 first"
    fi
    if [[ ! -f "${SYSROOT_DIR}/usr/lib/libelogind.so" ]]; then
        log_fatal "AccountsService requires elogind - build elogind first"
    fi

    # Cleanup: Remove circular symlinks and stale Python cache in codegen
    log_step "Cleaning up glib codegen environment..."
    local codegen_dir="${SYSROOT_DIR}/usr/share/glib-2.0/codegen"
    if [[ -L "${codegen_dir}/codegen" ]]; then
        rm -f "${codegen_dir}/codegen"
        log_info "  Removed circular symlink in sysroot codegen"
    fi
    rm -rf "${codegen_dir}/__pycache__" 2>/dev/null || true
    rm -rf "/usr/share/glib-2.0/codegen/__pycache__" 2>/dev/null || true

    # Ensure glib tools are available
    setup_glib_tools || {
        log_error "Failed to setup glib tools. Run script with sudo."
        return 1
    }

    check_command meson
    check_command ninja

    # Download
    download_source "accountsservice" \
        "https://www.freedesktop.org/software/accountsservice/accountsservice-${ACCOUNTSSERVICE_VERSION}.tar.xz" \
        "accountsservice-${ACCOUNTSSERVICE_VERSION}.tar.xz"

    # Clean existing source if subprojects are broken or build failed previously
    if [[ -d "${src_dir}" ]]; then
        local need_clean=false

        # Check if mocklibc was patched incorrectly (multiple declarations)
        if [[ -f "${src_dir}/subprojects/mocklibc-1.0/src/netgroup-debug.c" ]]; then
            if grep -c "extern void print_indent" "${src_dir}/subprojects/mocklibc-1.0/src/netgroup-debug.c" 2>/dev/null | grep -q "^[2-9]"; then
                log_info "Found badly patched mocklibc"
                need_clean=true
            fi
        fi

        # Check if previous build failed (build dir exists but no success marker)
        if [[ -d "${src_dir}/build" ]] && [[ ! -f "${src_dir}/build/src/accounts-daemon" ]]; then
            log_info "Found incomplete previous build"
            need_clean=true
        fi

        if $need_clean; then
            log_info "Cleaning broken accountsservice source directory..."
            rm -rf "${src_dir}"
        fi
    fi

    # Extract
    extract_source "accountsservice-${ACCOUNTSSERVICE_VERSION}.tar.xz" "accountsservice-${ACCOUNTSSERVICE_VERSION}"

    # Build
    log_step "Configuring AccountsService..."
    cd "${src_dir}"

    # Apply pre-build fixes
    mv tests/dbusmock tests/dbusmock-tests 2>/dev/null || true
    sed -e '/accounts_service\.py/s/dbusmock/dbusmock-tests/' \
        -e 's/assertEquals/assertEqual/' \
        -i tests/test-libaccountsservice.py 2>/dev/null || true

    rm -rf build
    mkdir -p build
    cd build

    # Set PKG_CONFIG_PATH to find our installed libs
    # Add sysroot bin to PATH for glib tools
    # Add sysroot lib to linker path for libelogind, libpolkit
    # Add PYTHONPATH for gdbus-codegen to find codegen modules
    export PKG_CONFIG_PATH="${SYSROOT_DIR}/usr/lib/pkgconfig:${PKG_CONFIG_PATH:-}"
    export PATH="${SYSROOT_DIR}/usr/bin:${PATH}"
    export LIBRARY_PATH="${SYSROOT_DIR}/usr/lib:${LIBRARY_PATH:-}"
    export LD_LIBRARY_PATH="${SYSROOT_DIR}/usr/lib:${LD_LIBRARY_PATH:-}"
    export PYTHONPATH="${SYSROOT_DIR}/usr/share/glib-2.0:${PYTHONPATH:-}"

    # Build link arguments for meson - pass library path to linker
    local link_args="-L${SYSROOT_DIR}/usr/lib -Wl,-rpath-link,${SYSROOT_DIR}/usr/lib"

    meson setup .. \
        --prefix=/usr \
        --buildtype=release \
        -D admin_group=wheel \
        -D elogind=true \
        -D systemdsystemunitdir=no \
        -D gtk_doc=false \
        -D introspection=false \
        -D vapi=false \
        -D docbook=false \
        -D c_link_args="${link_args}" \
        -D cpp_link_args="${link_args}" \
        2>&1 | tee "${log_file}"

    # Fix mocklibc missing print_indent declaration AFTER meson setup
    # Meson downloads the subproject during setup, so we patch it before ninja
    local mocklibc_dir="${src_dir}/subprojects/mocklibc-1.0/src"
    if [[ -d "${mocklibc_dir}" ]]; then
        if [[ -f "${mocklibc_dir}/netgroup-debug.c" ]] && ! grep -q "void print_indent" "${mocklibc_dir}/netgroup-debug.c"; then
            log_info "Patching mocklibc to fix print_indent declaration..."
            # Add forward declaration after includes
            sed -i '/^#include/a\/* Forward declaration */\nextern void print_indent(FILE *stream, int indent);' "${mocklibc_dir}/netgroup-debug.c"
        fi
    fi

    log_step "Compiling AccountsService..."
    ninja -j${JOBS} 2>&1 | tee -a "${log_file}"

    log_step "Installing AccountsService to sysroot..."
    DESTDIR="${SYSROOT_DIR}" ninja install 2>&1 | tee -a "${log_file}"

    # Create directories
    mkdir -p "${SYSROOT_DIR}/var/lib/AccountsService/users"
    mkdir -p "${SYSROOT_DIR}/var/lib/AccountsService/icons"

    # Create polkit rules for admin group
    mkdir -p "${SYSROOT_DIR}/etc/polkit-1/rules.d"
    cat > "${SYSROOT_DIR}/etc/polkit-1/rules.d/40-admin.rules" << 'EOF'
polkit.addAdminRule(function(action, subject) {
   return ["unix-group:wheel", "unix-group:adm", "unix-group:sudo"];
});
EOF

    cd "${PROJECT_ROOT}"
    log_success "AccountsService ${ACCOUNTSSERVICE_VERSION} built and installed"
}

# =============================================================================
# Configure sudo/su (install binaries, PAM, sudoers)
# =============================================================================

configure_sudo() {
    log_section "Configuring sudo/su"

    local sudo_src="${BUILD_DIR}/bin/sudo"
    local su_src="${BUILD_DIR}/bin/su"
    local visudo_src="${BUILD_DIR}/bin/visudo"

    # Check if sudo-rs binaries exist
    if [[ ! -f "${sudo_src}" ]]; then
        log_warn "sudo binary not found at ${sudo_src}"
        log_info "Run the main build first to compile sudo-rs"
        return 0
    fi

    # Install sudo/su/visudo binaries to sysroot
    log_step "Installing sudo/su/visudo binaries..."
    mkdir -p "${SYSROOT_DIR}/usr/bin"

    for bin in sudo su visudo; do
        local src="${BUILD_DIR}/bin/${bin}"
        if [[ -f "${src}" ]]; then
            cp "${src}" "${SYSROOT_DIR}/usr/bin/${bin}"
            chmod 755 "${SYSROOT_DIR}/usr/bin/${bin}"
            log_info "  Installed ${bin}"
        fi
    done

    # Set setuid bit on sudo and su (required for privilege escalation)
    log_step "Setting setuid permissions..."
    chmod u+s "${SYSROOT_DIR}/usr/bin/sudo" 2>/dev/null || log_warn "Could not set setuid on sudo (run as root)"
    chmod u+s "${SYSROOT_DIR}/usr/bin/su" 2>/dev/null || log_warn "Could not set setuid on su (run as root)"

    # Create symlinks in /bin for compatibility
    mkdir -p "${SYSROOT_DIR}/bin"
    ln -sf /usr/bin/sudo "${SYSROOT_DIR}/bin/sudo" 2>/dev/null || true
    ln -sf /usr/bin/su "${SYSROOT_DIR}/bin/su" 2>/dev/null || true
    ln -sf /usr/bin/visudo "${SYSROOT_DIR}/bin/visudo" 2>/dev/null || true

    # Create sudoers.d directory
    log_step "Creating sudoers configuration..."
    mkdir -p "${SYSROOT_DIR}/etc/sudoers.d"
    chmod 750 "${SYSROOT_DIR}/etc/sudoers.d"

    # Create main sudoers file
    cat > "${SYSROOT_DIR}/etc/sudoers" << 'EOF'
## sudoers file for RavenLinux
##
## This file MUST be edited with the 'visudo' command as root.
##
## See sudoers(5) for more information on @include directives:

## Host alias specification

## User alias specification

## Cmnd alias specification

## Defaults specification
Defaults    env_reset
Defaults    mail_badpass
Defaults    secure_path="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

## Allow root to run any commands anywhere
root    ALL=(ALL:ALL) ALL

## Allow members of group wheel to execute any command
%wheel  ALL=(ALL:ALL) ALL

## Allow members of group sudo to execute any command
%sudo   ALL=(ALL:ALL) ALL

## Read drop-in files from /etc/sudoers.d
@includedir /etc/sudoers.d
EOF

    # Set proper permissions on sudoers (must be 0440)
    chmod 0440 "${SYSROOT_DIR}/etc/sudoers"

    # Create PAM configuration for sudo
    log_step "Creating PAM configuration for sudo..."
    mkdir -p "${SYSROOT_DIR}/etc/pam.d"

    cat > "${SYSROOT_DIR}/etc/pam.d/sudo" << 'EOF'
#%PAM-1.0
# Authentication - allow root to use sudo without password
auth       sufficient   pam_rootok.so
auth       required     pam_unix.so nullok try_first_pass

# Account management
account    sufficient   pam_rootok.so
account    required     pam_unix.so

# Session management
session    required     pam_unix.so

# Password management (for sudo -k, etc.)
password   required     pam_unix.so nullok sha512
EOF

    # Update su PAM config
    log_step "Updating su PAM configuration..."
    cat > "${SYSROOT_DIR}/etc/pam.d/su" << 'EOF'
#%PAM-1.0
# Authentication - allow root to su without password
auth       sufficient   pam_rootok.so
auth       required     pam_unix.so nullok try_first_pass

# Account management
account    sufficient   pam_rootok.so
account    required     pam_unix.so

# Session management
session    required     pam_unix.so

# Password management
password   required     pam_unix.so nullok sha512
EOF

    # Create su-l (su - login shell) PAM config
    cat > "${SYSROOT_DIR}/etc/pam.d/su-l" << 'EOF'
#%PAM-1.0
# su with login shell - same as su but with login session
auth       sufficient   pam_rootok.so
auth       required     pam_unix.so nullok try_first_pass
account    sufficient   pam_rootok.so
account    required     pam_unix.so
session    required     pam_unix.so
password   required     pam_unix.so nullok sha512
EOF

    # Create system-auth PAM config (common auth stack)
    if [[ ! -f "${SYSROOT_DIR}/etc/pam.d/system-auth" ]]; then
        cat > "${SYSROOT_DIR}/etc/pam.d/system-auth" << 'EOF'
#%PAM-1.0
# Common authentication stack
auth       required     pam_unix.so nullok try_first_pass
account    required     pam_unix.so
password   required     pam_unix.so nullok sha512
session    required     pam_unix.so
EOF
    fi

    # Create system-account PAM config
    if [[ ! -f "${SYSROOT_DIR}/etc/pam.d/system-account" ]]; then
        cat > "${SYSROOT_DIR}/etc/pam.d/system-account" << 'EOF'
#%PAM-1.0
account    required     pam_unix.so
EOF
    fi

    # Create system-session PAM config
    if [[ ! -f "${SYSROOT_DIR}/etc/pam.d/system-session" ]]; then
        cat > "${SYSROOT_DIR}/etc/pam.d/system-session" << 'EOF'
#%PAM-1.0
session    required     pam_limits.so
session    required     pam_unix.so
session    optional     pam_elogind.so
EOF
    fi

    log_success "sudo/su configuration complete"
    log_info ""
    log_info "Configured:"
    log_info "  - /usr/bin/sudo, /usr/bin/su, /usr/bin/visudo"
    log_info "  - /etc/sudoers (wheel and sudo groups can use sudo)"
    log_info "  - /etc/sudoers.d/ directory"
    log_info "  - /etc/pam.d/sudo, /etc/pam.d/su, /etc/pam.d/su-l"
    log_info "  - elogind session integration in PAM"
}

# =============================================================================
# Build All Security Packages
# =============================================================================

build_all() {
    log_section "Building All Security Packages"

    # Check system dependencies first
    check_system_dependencies || exit 1

    log_info "Build order:"
    log_info "  1. glib2 (rebuild for headers/pkgconfig)"
    log_info "  2. duktape (JavaScript engine)"
    log_info "  3. elogind (session management)"
    log_info "  4. polkit (authorization)"
    log_info "  5. accountsservice (user accounts)"
    log_info "  6. configure sudo/su (PAM, sudoers)"
    echo ""

    build_glib2

    # Setup glib tools after glib2 is built (needs the binaries to exist)
    setup_glib_tools || {
        log_error "Failed to setup glib tools. Run script with sudo or create symlinks manually."
        exit 1
    }

    build_duktape
    build_elogind
    build_polkit
    build_accountsservice
    configure_sudo

    log_section "Security Packages Build Complete"
    log_success "All security packages built successfully!"
    log_info ""
    log_info "Installed packages:"
    log_info "  - glib2 ${GLIB2_VERSION}"
    log_info "  - duktape ${DUKTAPE_VERSION}"
    log_info "  - elogind ${ELOGIND_VERSION}"
    log_info "  - polkit ${POLKIT_VERSION}"
    log_info "  - accountsservice ${ACCOUNTSSERVICE_VERSION}"
    log_info "  - sudo/su configured with PAM + elogind"
    log_info ""
    log_info "Next steps:"
    log_info "  1. Add elogind service to init system"
    log_info "  2. Add polkitd service to init system"
    log_info "  3. Add accounts-daemon service to init system"
    log_info "  4. Verify kernel has CONFIG_CGROUPS=y"
    log_info "  5. Add user to 'wheel' or 'sudo' group for sudo access"
}

# =============================================================================
# Show Help
# =============================================================================

show_help() {
    cat << EOF
RavenLinux Security Packages Builder

Usage: $(basename "$0") [PACKAGE]

Packages:
    all             Build all packages in order (default)
    glib2           Build GLib2 (core library)
    duktape         Build Duktape (JavaScript engine)
    elogind         Build elogind (session management)
    polkit          Build Polkit (authorization)
    accountsservice Build AccountsService (user accounts)
    sudo            Configure sudo/su (install binaries, PAM, sudoers)

Options:
    -h, --help      Show this help message

Environment Variables:
    RAVEN_JOBS      Number of parallel build jobs (default: nproc)
    RAVEN_BUILD     Build directory (default: ./build)

Examples:
    $(basename "$0")                    # Build all packages
    $(basename "$0") polkit             # Build only polkit
    $(basename "$0") sudo               # Configure sudo/su only
    RAVEN_JOBS=8 $(basename "$0") all   # Build with 8 jobs
EOF
}

# =============================================================================
# Main
# =============================================================================

main() {
    local package="${1:-all}"

    # Create necessary directories
    mkdir -p "${SOURCES_DIR}" "${LOGS_DIR}" "${SYSROOT_DIR}/usr/lib/pkgconfig"

    # Parse arguments
    case "$package" in
        -h|--help)
            show_help
            exit 0
            ;;
        all)
            init_logging "build-security" "Building all security packages" 2>/dev/null || true
            build_all
            ;;
        glib2)
            init_logging "build-security" "Building glib2" 2>/dev/null || true
            check_system_dependencies || exit 1
            build_glib2
            ;;
        duktape)
            init_logging "build-security" "Building duktape" 2>/dev/null || true
            check_system_dependencies || exit 1
            build_duktape
            ;;
        elogind)
            init_logging "build-security" "Building elogind" 2>/dev/null || true
            check_system_dependencies || exit 1
            build_elogind
            ;;
        polkit)
            init_logging "build-security" "Building polkit" 2>/dev/null || true
            check_system_dependencies || exit 1
            build_polkit
            ;;
        accountsservice)
            init_logging "build-security" "Building accountsservice" 2>/dev/null || true
            check_system_dependencies || exit 1
            build_accountsservice
            ;;
        sudo)
            init_logging "build-security" "Configuring sudo/su" 2>/dev/null || true
            configure_sudo
            ;;
        *)
            log_error "Unknown package: $package"
            show_help
            exit 1
            ;;
    esac

    finalize_logging 0 2>/dev/null || true
}

main "$@"
