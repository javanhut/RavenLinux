#!/bin/bash
# =============================================================================
# RavenLinux Stage 3: Build Packages
# =============================================================================
# Builds all RavenLinux custom packages:
# - Vem (text editor)
# - Carrion (programming language)
# - Ivaldi (version control)
# - rvn (package manager)
# - raven-installer (GUI installer)
# - raven-usb (USB creator)
# - RavenBoot (bootloader)

set -euo pipefail

# =============================================================================
# Environment Setup (with defaults for standalone execution)
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${RAVEN_ROOT:-$(dirname "$(dirname "$SCRIPT_DIR")")}"
BUILD_DIR="${RAVEN_BUILD:-${PROJECT_ROOT}/build}"
SYSROOT_DIR="${SYSROOT_DIR:-${BUILD_DIR}/sysroot}"
PACKAGES_DIR="${PACKAGES_DIR:-${BUILD_DIR}/packages}"
LOGS_DIR="${LOGS_DIR:-${BUILD_DIR}/logs}"
TOOLS_DIR="${PROJECT_ROOT}/tools"

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
    CYAN='\033[0;36m'
    NC='\033[0m'
    log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
    log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
    log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
    log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
    log_step() { echo -e "${CYAN}[STEP]${NC} $1"; }
fi

# =============================================================================
# Build Go packages (Vem, Carrion, Ivaldi)
# =============================================================================
build_go_packages() {
    log_step "Building Go packages..."

    if [[ -x "${PROJECT_ROOT}/scripts/build-packages.sh" ]]; then
        "${PROJECT_ROOT}/scripts/build-packages.sh" all 2>&1 | tee "${LOGS_DIR}/go-packages.log"
    else
        log_warn "build-packages.sh not found"
    fi

    # Copy to sysroot
    mkdir -p "${SYSROOT_DIR}/bin"
    for pkg in vem carrion ivaldi; do
        if [[ -f "${PACKAGES_DIR}/bin/${pkg}" ]]; then
            cp "${PACKAGES_DIR}/bin/${pkg}" "${SYSROOT_DIR}/bin/"
            log_info "  Installed ${pkg}"
        fi
    done

    log_success "Go packages built"
}

# =============================================================================
# Build rvn package manager (Rust)
# =============================================================================
build_rvn() {
    log_step "Building rvn package manager..."

    local rvn_dir="${TOOLS_DIR}/rvn"

    if [[ ! -d "${rvn_dir}" ]]; then
        log_warn "rvn source not found at ${rvn_dir}"
        return 0
    fi

    if ! command -v cargo &>/dev/null; then
        log_warn "Cargo not found, skipping rvn build"
        return 0
    fi

    cd "${rvn_dir}"

    if cargo build --release 2>&1 | tee "${LOGS_DIR}/rvn.log"; then
        mkdir -p "${PACKAGES_DIR}/bin" "${SYSROOT_DIR}/bin"
        cp target/release/rvn "${PACKAGES_DIR}/bin/"
        cp target/release/rvn "${SYSROOT_DIR}/bin/"
        log_success "rvn package manager built"
    else
        log_warn "Failed to build rvn"
    fi

    cd "${PROJECT_ROOT}"
}

# =============================================================================
# Hyprland compositor (copied from host in build-live-iso.sh)
# =============================================================================
# NOTE: raven-compositor has been replaced with Hyprland.
# Hyprland binary is copied from the host system during ISO build.
# See scripts/build-live-iso.sh copy_wayland_tools() function.

# =============================================================================
# Build raven-installer (Go with Gio UI)
# =============================================================================
build_installer() {
    log_step "Building raven-installer..."

    local installer_dir="${TOOLS_DIR}/raven-installer"

    if [[ ! -d "${installer_dir}" ]]; then
        log_warn "raven-installer source not found"
        return 0
    fi

    if ! command -v go &>/dev/null; then
        log_warn "Go not found, skipping installer build"
        return 0
    fi

    cd "${installer_dir}"

    # Gio UI requires CGO
    if CGO_ENABLED=1 go build -o raven-installer . 2>&1 | tee "${LOGS_DIR}/installer.log"; then
        mkdir -p "${PACKAGES_DIR}/bin" "${SYSROOT_DIR}/bin"
        cp raven-installer "${PACKAGES_DIR}/bin/"
        cp raven-installer "${SYSROOT_DIR}/bin/"
        ln -sf raven-installer "${SYSROOT_DIR}/bin/raven-install"
        log_success "raven-installer built"
    else
        log_warn "Failed to build raven-installer"
    fi

    cd "${PROJECT_ROOT}"
}

# =============================================================================
# Build raven-usb (Go with Gio UI)
# =============================================================================
build_usb_creator() {
    log_step "Building raven-usb..."

    local usb_dir="${TOOLS_DIR}/raven-usb"

    if [[ ! -d "${usb_dir}" ]]; then
        log_warn "raven-usb source not found"
        return 0
    fi

    if ! command -v go &>/dev/null; then
        log_warn "Go not found, skipping USB creator build"
        return 0
    fi

    cd "${usb_dir}"

    if CGO_ENABLED=1 go build -o raven-usb . 2>&1 | tee "${LOGS_DIR}/usb-creator.log"; then
        mkdir -p "${PACKAGES_DIR}/bin" "${SYSROOT_DIR}/bin"
        cp raven-usb "${PACKAGES_DIR}/bin/"
        cp raven-usb "${SYSROOT_DIR}/bin/"
        log_success "raven-usb built"
    else
        log_warn "Failed to build raven-usb"
    fi

    cd "${PROJECT_ROOT}"
}

# =============================================================================
# Build WiFi tools (Go)
# =============================================================================
build_wifi_tools() {
    log_step "Building WiFi tools..."

    # Build wifi TUI
    local wifi_tui_dir="${PROJECT_ROOT}/tools/raven-wifi-tui"
    if [[ -d "${wifi_tui_dir}" ]]; then
        if ! command -v go &>/dev/null; then
            log_warn "Go not found, skipping wifi TUI build"
        else
            cd "${wifi_tui_dir}"
            go mod tidy 2>/dev/null || true

            if CGO_ENABLED=0 go build -o wifi . 2>&1 | tee "${LOGS_DIR}/wifi-tui.log"; then
                mkdir -p "${PACKAGES_DIR}/bin" "${SYSROOT_DIR}/bin"
                cp wifi "${PACKAGES_DIR}/bin/"
                cp wifi "${SYSROOT_DIR}/bin/"
                log_success "wifi (TUI) built"
            else
                log_warn "Failed to build wifi TUI"
            fi
            cd "${PROJECT_ROOT}"
        fi
    else
        log_warn "WiFi TUI source not found at ${wifi_tui_dir}"
    fi

    # Build raven-wifi GUI
    local wifi_gui_dir="${PROJECT_ROOT}/tools/raven-wifi"
    if [[ -d "${wifi_gui_dir}" ]]; then
        if ! command -v go &>/dev/null; then
            log_warn "Go not found, skipping raven-wifi GUI build"
        else
            cd "${wifi_gui_dir}"
            log_info "Downloading dependencies for raven-wifi GUI..."
            go mod download 2>/dev/null || go mod tidy 2>/dev/null || true

            log_info "Compiling raven-wifi GUI with Wayland support..."
            
            # CGO flags to ensure Wayland backend is linked
            local cgo_cflags=""
            local cgo_ldflags=""

            # Add Wayland and XKB flags if available
            if pkg-config --exists wayland-client wayland-egl xkbcommon 2>/dev/null; then
                cgo_cflags="$(pkg-config --cflags wayland-client wayland-egl xkbcommon 2>/dev/null || true)"
                cgo_ldflags="$(pkg-config --libs wayland-client wayland-egl xkbcommon 2>/dev/null || true)"
                log_info "Building with Wayland support: ${cgo_ldflags}"
            else
                log_warn "Wayland libraries not found, building with X11 fallback only"
            fi

            if env CGO_ENABLED=1 \
                CGO_CFLAGS="${cgo_cflags}" \
                CGO_LDFLAGS="${cgo_ldflags}" \
                go build -ldflags="-s -w" -o raven-wifi . 2>&1 | tee "${LOGS_DIR}/wifi-gui.log"; then
                
                mkdir -p "${PACKAGES_DIR}/bin" "${SYSROOT_DIR}/bin"
                cp raven-wifi "${PACKAGES_DIR}/bin/"
                cp raven-wifi "${SYSROOT_DIR}/bin/"
                chmod +x "${PACKAGES_DIR}/bin/raven-wifi"
                chmod +x "${SYSROOT_DIR}/bin/raven-wifi"
                log_success "raven-wifi (GUI) built and installed"
            else
                log_warn "Failed to build raven-wifi GUI"
            fi
            cd "${PROJECT_ROOT}"
        fi
    else
        log_warn "WiFi GUI source not found at ${wifi_gui_dir}"
    fi
}

# =============================================================================
# Build RavenBoot bootloader (Rust UEFI)
# =============================================================================
build_bootloader() {
    log_step "Building RavenBoot bootloader..."

    local bootloader_dir="${PROJECT_ROOT}/bootloader"

    if [[ ! -d "${bootloader_dir}" ]]; then
        log_warn "Bootloader source not found"
        return 0
    fi

    if ! command -v cargo &>/dev/null; then
        log_warn "Cargo not found, skipping bootloader build"
        return 0
    fi

    # Check for UEFI target
    if ! rustup target list --installed 2>/dev/null | grep -q "x86_64-unknown-uefi"; then
        log_info "Adding UEFI target..."
        rustup target add x86_64-unknown-uefi 2>/dev/null || {
            log_warn "Failed to add UEFI target"
            return 0
        }
    fi

    cd "${bootloader_dir}"

    if cargo build --target x86_64-unknown-uefi --release 2>&1 | tee "${LOGS_DIR}/bootloader.log"; then
        mkdir -p "${PACKAGES_DIR}/boot"
        cp target/x86_64-unknown-uefi/release/raven-boot.efi "${PACKAGES_DIR}/boot/"
        log_success "RavenBoot bootloader built"
    else
        log_warn "Failed to build bootloader"
    fi

    cd "${PROJECT_ROOT}"
}

# =============================================================================
# Build Development Tools (GCC, Go, Python, Rust, etc.)
# =============================================================================
build_dev_tools() {
    log_step "Building development tools..."

    local sources_dir="${BUILD_DIR}/sources"
    local dev_build_dir="${BUILD_DIR}/dev-tools"
    mkdir -p "${sources_dir}" "${dev_build_dir}"

    # Check for pre-built binaries first
    local prebuilt_dir="${PROJECT_ROOT}/prebuilt"
    if [[ -d "${prebuilt_dir}" ]]; then
        log_info "Checking for prebuilt binaries..."
        for tool in gcc g++ go python vim nvim ssh; do
            if [[ -f "${prebuilt_dir}/bin/${tool}" ]]; then
                log_info "Using prebuilt ${tool}"
                cp "${prebuilt_dir}/bin/${tool}" "${SYSROOT_DIR}/usr/bin/"
            fi
        done
    fi

    # Build/repair GCC if missing or installed as a broken wrapper
    if [[ ! -x "${SYSROOT_DIR}/usr/bin/gcc" ]] || ! file "${SYSROOT_DIR}/usr/bin/gcc" 2>/dev/null | grep -q "ELF"; then
        build_gcc
    fi

    # Build Go if not prebuilt
    if [[ ! -f "${SYSROOT_DIR}/usr/bin/go" ]]; then
        build_golang
    fi

    # Build/repair Python if missing or installed as a broken wrapper
    if [[ ! -x "${SYSROOT_DIR}/usr/bin/python3" ]] || ! file "${SYSROOT_DIR}/usr/bin/python3" 2>/dev/null | grep -q "ELF"; then
        build_python
    fi

    # Rust is typically installed via rustup during first boot
    # but we include the package definition for rvn

    mkdir -p "${SYSROOT_DIR}/etc/raven"
    log_success "Development tools built"
}

# Build GCC from source
build_gcc() {
    log_info "Building GCC toolchain..."

    local gcc_ver="13.2.0"
    local binutils_ver="2.42"
    local gmp_ver="6.3.0"
    local mpfr_ver="4.2.1"
    local mpc_ver="1.3.1"

    local build_dir="${BUILD_DIR}/dev-tools/gcc-build"
    mkdir -p "${build_dir}"

    # Check if host has GCC (needed to bootstrap)
    if ! command -v gcc &>/dev/null; then
        log_warn "Host GCC not found - cannot build GCC from source"
        log_info "GCC will be available via rvn install gcc"
        return 0
    fi

    # For a full bootstrap, we'd download and build:
    # 1. binutils
    # 2. gmp, mpfr, mpc (GCC dependencies)
    # 3. GCC itself
    # This is a lengthy process - for now, we check for system GCC

    # Prefer copying a working host toolchain into the sysroot over wrappers.
    # Wrappers break inside the live ISO (they just exec themselves).
    local host_gcc
    host_gcc="$(command -v gcc)"
    local host_gpp
    host_gpp="$(command -v g++)"

    if [[ -x "${host_gcc}" ]] && [[ -x "${host_gpp}" ]]; then
        log_info "Copying host GCC toolchain into sysroot..."

        local target
        target="$(gcc -dumpmachine 2>/dev/null || true)"
        local version
        version="$(gcc -dumpfullversion 2>/dev/null || gcc -dumpversion 2>/dev/null || true)"

        mkdir -p "${SYSROOT_DIR}/usr/bin" "${SYSROOT_DIR}/bin"

        cp -L "${host_gcc}" "${SYSROOT_DIR}/usr/bin/gcc"
        cp -L "${host_gpp}" "${SYSROOT_DIR}/usr/bin/g++"
        chmod 755 "${SYSROOT_DIR}/usr/bin/gcc" "${SYSROOT_DIR}/usr/bin/g++"

        # Common driver symlinks
        ln -sf gcc "${SYSROOT_DIR}/usr/bin/cc"
        ln -sf g++ "${SYSROOT_DIR}/usr/bin/c++"

        # Copy binutils that GCC relies on at runtime
        for bin in as ld ar ranlib nm strip objdump objcopy readelf; do
            if command -v "${bin}" &>/dev/null; then
                cp -L "$(command -v "${bin}")" "${SYSROOT_DIR}/usr/bin/${bin}" 2>/dev/null || true
                chmod 755 "${SYSROOT_DIR}/usr/bin/${bin}" 2>/dev/null || true
                copy_binary_deps "$(command -v "${bin}")"
            fi
        done

        # Copy GCC internal programs (cc1, collect2, etc.) and support directories.
        for prog in cc1 cc1plus collect2 lto1 lto-wrapper; do
            local p
            p="$(gcc -print-prog-name="${prog}" 2>/dev/null || true)"
            if [[ -n "${p}" ]] && [[ -x "${p}" ]]; then
                local d
                d="$(dirname "${p}")"
                mkdir -p "${SYSROOT_DIR}${d}"
                cp -L "${p}" "${SYSROOT_DIR}${p}" 2>/dev/null || true
                copy_binary_deps "${p}"
            fi
        done

        if [[ -n "${target}" ]] && [[ -n "${version}" ]]; then
            for dir in "/usr/lib/gcc/${target}/${version}" "/usr/libexec/gcc/${target}/${version}" "/usr/lib/gcc/${target}" "/usr/libexec/gcc/${target}"; do
                if [[ -d "${dir}" ]]; then
                    mkdir -p "${SYSROOT_DIR}${dir}"
                    cp -a "${dir}/." "${SYSROOT_DIR}${dir}/" 2>/dev/null || true
                fi
            done
        fi

        # Copy include directories used by GCC
        for inc in "$(gcc -print-file-name=include 2>/dev/null)" "$(gcc -print-file-name=include-fixed 2>/dev/null)"; do
            if [[ -n "${inc}" ]] && [[ -d "${inc}" ]]; then
                mkdir -p "${SYSROOT_DIR}${inc}"
                cp -a "${inc}/." "${SYSROOT_DIR}${inc}/" 2>/dev/null || true
            fi
        done

        # Copy shared library deps for the GCC driver itself
        copy_binary_deps "${host_gcc}"
        copy_binary_deps "${host_gpp}"

        log_success "Host GCC toolchain installed"
    fi
}

# Build Go from source or download binary
build_golang() {
    log_info "Installing Go..."

    local go_ver="1.22.0"
    local arch="amd64"
    [[ "$(uname -m)" == "aarch64" ]] && arch="arm64"

    local go_tarball="go${go_ver}.linux-${arch}.tar.gz"
    local go_url="https://go.dev/dl/${go_tarball}"
    local cache_dir="${BUILD_DIR}/sources"

    mkdir -p "${cache_dir}"

    # Download if not cached
    if [[ ! -f "${cache_dir}/${go_tarball}" ]]; then
        log_info "Downloading Go ${go_ver}..."
        if curl -fsSL -o "${cache_dir}/${go_tarball}" "${go_url}"; then
            log_info "Downloaded Go"
        else
            log_warn "Failed to download Go - will be available via rvn install go"
            return 0
        fi
    fi

    # Extract to sysroot
    log_info "Installing Go to sysroot..."
    mkdir -p "${SYSROOT_DIR}/usr/lib"
    tar -xzf "${cache_dir}/${go_tarball}" -C "${SYSROOT_DIR}/usr/lib"

    # Create symlinks
    mkdir -p "${SYSROOT_DIR}/usr/bin"
    ln -sf ../lib/go/bin/go "${SYSROOT_DIR}/usr/bin/go"
    ln -sf ../lib/go/bin/gofmt "${SYSROOT_DIR}/usr/bin/gofmt"

    # Create profile script
    mkdir -p "${SYSROOT_DIR}/etc/profile.d"
    cat > "${SYSROOT_DIR}/etc/profile.d/go.sh" << 'GOPROFILE'
# Go environment
export GOROOT=/usr/lib/go
export GOPATH=$HOME/go
export PATH=$PATH:$GOROOT/bin:$GOPATH/bin
GOPROFILE

    log_success "Go ${go_ver} installed"
}

# Build Python from source or use system Python
build_python() {
    log_info "Installing Python..."

    local py_ver="3.12.1"

    # Check for host Python
    if command -v python3 &>/dev/null; then
        local host_py
        host_py="$(command -v python3)"
        local host_ver
        host_ver="$(python3 --version 2>&1 | awk '{print $2}')"

        log_info "Copying host Python ${host_ver} into sysroot..."

        mkdir -p "${SYSROOT_DIR}/usr/bin"
        cp -L "${host_py}" "${SYSROOT_DIR}/usr/bin/python3"
        chmod 755 "${SYSROOT_DIR}/usr/bin/python3"
        ln -sf python3 "${SYSROOT_DIR}/usr/bin/python"

        # Copy stdlib and site-packages for the host Python installation
        local -a py_paths=()
        while IFS= read -r p; do
            [[ -n "$p" ]] && py_paths+=("$p")
        done < <(python3 - <<'PY'
import sysconfig
paths = sysconfig.get_paths()
keys = ["stdlib", "platstdlib", "purelib", "platlib", "include", "platinclude"]
seen = set()
for k in keys:
    p = paths.get(k)
    if p and p not in seen:
        seen.add(p)
        print(p)
PY
)

        for p in "${py_paths[@]}"; do
            if [[ -d "${p}" ]]; then
                mkdir -p "${SYSROOT_DIR}${p}"
                cp -a "${p}/." "${SYSROOT_DIR}${p}/" 2>/dev/null || true
            fi
        done

        # Copy libpython if present
        local libdir
        libdir="$(python3 -c 'import sysconfig; print(sysconfig.get_config_var("LIBDIR") or "")' 2>/dev/null || true)"
        local ldlib
        ldlib="$(python3 -c 'import sysconfig; print(sysconfig.get_config_var("LDLIBRARY") or "")' 2>/dev/null || true)"
        if [[ -n "${libdir}" ]] && [[ -n "${ldlib}" ]] && [[ -f "${libdir}/${ldlib}" ]]; then
            mkdir -p "${SYSROOT_DIR}${libdir}"
            cp -L "${libdir}/${ldlib}" "${SYSROOT_DIR}${libdir}/" 2>/dev/null || true
            # Common symlink names
            cp -L "${libdir}/libpython"* "${SYSROOT_DIR}${libdir}/" 2>/dev/null || true
        fi

        # Copy pip if available
        if command -v pip3 &>/dev/null; then
            cp -L "$(command -v pip3)" "${SYSROOT_DIR}/usr/bin/pip3" 2>/dev/null || true
            chmod 755 "${SYSROOT_DIR}/usr/bin/pip3" 2>/dev/null || true
            ln -sf pip3 "${SYSROOT_DIR}/usr/bin/pip"
        fi

        copy_binary_deps "${host_py}"
        log_success "Python installed from host"
    else
        log_warn "Host Python not found - will be available via rvn install python"
    fi
}

# =============================================================================
# Build Editors (Vim, Neovim)
# =============================================================================
build_editors() {
    log_step "Building editors..."

    mkdir -p "${SYSROOT_DIR}/etc/xdg/nvim"
    mkdir -p "${SYSROOT_DIR}/etc/vim"

    # Build/repair Vim if not present or linked against libcanberra / has build-path RUNPATH
    if [[ -f "${SYSROOT_DIR}/usr/bin/vim" ]]; then
        if readelf -d "${SYSROOT_DIR}/usr/bin/vim" 2>/dev/null | grep -q "libcanberra\\.so" || \
           readelf -d "${SYSROOT_DIR}/usr/bin/vim" 2>/dev/null | grep -q "${SYSROOT_DIR}/usr/lib"; then
            log_warn "Vim needs rebuild (removing libcanberra dependency / build-path RUNPATH)"
            rm -f "${SYSROOT_DIR}/usr/bin/vim" "${SYSROOT_DIR}/usr/bin/xxd" "${SYSROOT_DIR}/usr/bin/vi" \
                  "${SYSROOT_DIR}/bin/vim" "${SYSROOT_DIR}/bin/vi" 2>/dev/null || true
        fi
    fi
    if [[ ! -f "${SYSROOT_DIR}/usr/bin/vim" ]]; then
        build_vim
    fi

    # Build Neovim if not present
    if [[ ! -f "${SYSROOT_DIR}/usr/bin/nvim" ]]; then
        build_neovim
    fi

    # Copy Neovim config if exists
    local nvim_config="${PROJECT_ROOT}/configs/nvim/init.lua"
    if [[ -f "${nvim_config}" ]]; then
        cp "${nvim_config}" "${SYSROOT_DIR}/etc/xdg/nvim/init.lua"
        log_info "Installed Neovim default config"
    fi

    # Create basic vimrc
    cat > "${SYSROOT_DIR}/etc/vim/vimrc" << 'EOF'
" RavenLinux default vimrc
set nocompatible
syntax on
filetype plugin indent on
set number
set relativenumber
set expandtab
set tabstop=4
set shiftwidth=4
set smartindent
set autoindent
set hlsearch
set incsearch
set ignorecase
set smartcase
set cursorline
set mouse=a
set clipboard=unnamedplus
set wildmenu
set showcmd
set laststatus=2
set ruler

" Key mappings
let mapleader = " "
nnoremap <leader>w :w<CR>
nnoremap <leader>q :q<CR>
nnoremap <C-h> <C-w>h
nnoremap <C-j> <C-w>j
nnoremap <C-k> <C-w>k
nnoremap <C-l> <C-w>l
EOF

    log_info "Installed Vim default config"
    log_success "Editors built and configured"
}

# =============================================================================
# Build Core Dependencies (ncurses, etc.)
# =============================================================================

# Build ncurses library (required for vim, neovim, shells, etc.)
build_ncurses() {
    log_info "Building ncurses..."

    local ncurses_ver="6.5"
    local cache_dir="${BUILD_DIR}/sources"
    mkdir -p "${cache_dir}"

    # Check if already built; if present but missing symbol version definitions, rebuild.
    if [[ -f "${SYSROOT_DIR}/usr/lib/libncursesw.so.6" ]]; then
        if readelf --version-info "${SYSROOT_DIR}/usr/lib/libncursesw.so.6" 2>/dev/null | grep -q "Version definition section"; then
            log_info "ncurses already installed"
            return 0
        fi
        log_warn "ncurses present but missing symbol version info; rebuilding to avoid runtime warnings"
        rm -f "${SYSROOT_DIR}/usr/lib/libncurses"* "${SYSROOT_DIR}/usr/lib/libtinfo"* 2>/dev/null || true
        rm -f "${SYSROOT_DIR}/usr/lib/libncursesw"* "${SYSROOT_DIR}/usr/lib/libtinfow"* 2>/dev/null || true
    elif [[ -f "${SYSROOT_DIR}/usr/lib/libncursesw.so" ]] || [[ -f "${SYSROOT_DIR}/usr/lib/libncurses.so" ]]; then
        log_info "ncurses already installed"
        return 0
    fi

    local ncurses_tarball="${cache_dir}/ncurses-${ncurses_ver}.tar.gz"
    local ncurses_src="${cache_dir}/ncurses-${ncurses_ver}"

    # Download ncurses
    if [[ ! -f "${ncurses_tarball}" ]]; then
        log_info "Downloading ncurses ${ncurses_ver}..."
        if ! curl -fsSL -o "${ncurses_tarball}" \
            "https://ftp.gnu.org/gnu/ncurses/ncurses-${ncurses_ver}.tar.gz"; then
            log_warn "Failed to download ncurses"
            return 1
        fi
    fi

    # Extract
    if [[ ! -d "${ncurses_src}" ]]; then
        tar -xzf "${ncurses_tarball}" -C "${cache_dir}"
    fi

    cd "${ncurses_src}"

    # Configure ncurses with wide character support
    # Note: --without-cxx-binding is needed for GCC 15+ due to NCURSES_BOOL type conflict
    ./configure \
        --prefix=/usr \
        --with-shared \
        --with-termlib \
        --enable-widec \
        --enable-pc-files \
        --with-pkg-config-libdir=/usr/lib/pkgconfig \
        --without-debug \
        --without-ada \
        --without-cxx-binding \
        --with-versioned-syms \
        --enable-symlinks \
        --with-terminfo-dirs="/usr/share/terminfo:/etc/terminfo" \
        --with-default-terminfo-dir=/usr/share/terminfo

    make -j$(nproc)
    # LD_LIBRARY_PATH is needed so the freshly-built tic can find libtinfow.so.6
    # when compiling the terminfo database during install
    LD_LIBRARY_PATH="${SYSROOT_DIR}/usr/lib" make DESTDIR="${SYSROOT_DIR}" install

    # Create non-wide symlinks for compatibility
    cd "${SYSROOT_DIR}/usr/lib"
    for lib in ncurses form panel menu; do
        ln -sf lib${lib}w.so lib${lib}.so 2>/dev/null || true
        ln -sf lib${lib}w.a lib${lib}.a 2>/dev/null || true
    done
    ln -sf libncursesw.so libcurses.so 2>/dev/null || true
    # Create libtinfo symlinks pointing to libtinfow (wide char version)
    ln -sf libtinfow.so libtinfo.so 2>/dev/null || true
    ln -sf libtinfow.so.6 libtinfo.so.6 2>/dev/null || true

    # Also link headers
    cd "${SYSROOT_DIR}/usr/include"
    ln -sf ncursesw/* . 2>/dev/null || true

    cd "${PROJECT_ROOT}"
    log_success "ncurses ${ncurses_ver} built and installed"
}

# Build libcanberra (optional, for event sounds)
build_libcanberra() {
    log_info "Building libcanberra..."

    local canberra_ver="0.30"
    local cache_dir="${BUILD_DIR}/sources"
    mkdir -p "${cache_dir}"

    # Check if already built
    if [[ -f "${SYSROOT_DIR}/usr/lib/libcanberra.so" ]]; then
        log_info "libcanberra already installed"
        return 0
    fi

    local canberra_tarball="${cache_dir}/libcanberra-${canberra_ver}.tar.xz"
    local canberra_src="${cache_dir}/libcanberra-${canberra_ver}"

    # Download libcanberra
    if [[ ! -f "${canberra_tarball}" ]]; then
        log_info "Downloading libcanberra ${canberra_ver}..."
        if ! curl -fsSL -o "${canberra_tarball}" \
            "http://0pointer.de/lennart/projects/libcanberra/libcanberra-${canberra_ver}.tar.xz"; then
            log_warn "Failed to download libcanberra - skipping (optional)"
            return 0
        fi
    fi

    # Extract
    if [[ ! -d "${canberra_src}" ]]; then
        tar -xJf "${canberra_tarball}" -C "${cache_dir}"
    fi

    cd "${canberra_src}"

    # Configure without GTK (minimal build)
    ./configure \
        --prefix=/usr \
        --disable-gtk \
        --disable-gtk3 \
        --disable-oss \
        --disable-lynx \
        --enable-null \
        --with-builtin=dso

    make -j$(nproc) || {
        log_warn "libcanberra build failed - skipping (optional)"
        cd "${PROJECT_ROOT}"
        return 0
    }
    make DESTDIR="${SYSROOT_DIR}" install

    cd "${PROJECT_ROOT}"
    log_success "libcanberra ${canberra_ver} built and installed"
}

# Build libsodium (encryption library for vim, neovim, etc.)
build_libsodium() {
    log_info "Building libsodium..."

    local sodium_ver="1.0.20"
    local cache_dir="${BUILD_DIR}/sources"
    mkdir -p "${cache_dir}"

    # Check if already built
    if [[ -f "${SYSROOT_DIR}/usr/lib/libsodium.so" ]]; then
        log_info "libsodium already installed"
        return 0
    fi

    local sodium_tarball="${cache_dir}/libsodium-${sodium_ver}.tar.gz"
    local sodium_src="${cache_dir}/libsodium-${sodium_ver}"

    # Download libsodium
    if [[ ! -f "${sodium_tarball}" ]]; then
        log_info "Downloading libsodium ${sodium_ver}..."
        if ! curl -fsSL -o "${sodium_tarball}" \
            "https://download.libsodium.org/libsodium/releases/libsodium-${sodium_ver}.tar.gz"; then
            # Try GitHub releases as fallback
            if ! curl -fsSL -o "${sodium_tarball}" \
                "https://github.com/jedisct1/libsodium/releases/download/${sodium_ver}-RELEASE/libsodium-${sodium_ver}.tar.gz"; then
                log_warn "Failed to download libsodium"
                return 1
            fi
        fi
    fi

    # Extract
    if [[ ! -d "${sodium_src}" ]]; then
        tar -xzf "${sodium_tarball}" -C "${cache_dir}"
        # Handle different directory naming
        if [[ -d "${cache_dir}/libsodium-stable" ]]; then
            mv "${cache_dir}/libsodium-stable" "${sodium_src}"
        fi
    fi

    cd "${sodium_src}"

    ./configure \
        --prefix=/usr \
        --disable-static \
        --enable-shared

    make -j$(nproc)
    make DESTDIR="${SYSROOT_DIR}" install

    cd "${PROJECT_ROOT}"
    log_success "libsodium ${sodium_ver} built and installed"
}

# Build libuv (async I/O library for neovim)
build_libuv() {
    log_info "Building libuv..."

    local libuv_ver="1.48.0"
    local cache_dir="${BUILD_DIR}/sources"
    mkdir -p "${cache_dir}"

    # Check if already built
    if [[ -f "${SYSROOT_DIR}/usr/lib/libuv.so" ]]; then
        log_info "libuv already installed"
        return 0
    fi

    local libuv_tarball="${cache_dir}/libuv-v${libuv_ver}.tar.gz"
    local libuv_src="${cache_dir}/libuv-v${libuv_ver}"

    # Download libuv
    if [[ ! -f "${libuv_tarball}" ]]; then
        log_info "Downloading libuv ${libuv_ver}..."
        if ! curl -fsSL -o "${libuv_tarball}" \
            "https://dist.libuv.org/dist/v${libuv_ver}/libuv-v${libuv_ver}.tar.gz"; then
            log_warn "Failed to download libuv"
            return 1
        fi
    fi

    # Extract
    if [[ ! -d "${libuv_src}" ]]; then
        tar -xzf "${libuv_tarball}" -C "${cache_dir}"
    fi

    cd "${libuv_src}"

    # libuv uses cmake or autotools
    if [[ -f "CMakeLists.txt" ]] && command -v cmake &>/dev/null; then
        mkdir -p build && cd build
        cmake .. -DCMAKE_INSTALL_PREFIX=/usr -DBUILD_TESTING=OFF
        make -j$(nproc)
        make DESTDIR="${SYSROOT_DIR}" install
    else
        ./autogen.sh 2>/dev/null || true
        ./configure --prefix=/usr
        make -j$(nproc)
        make DESTDIR="${SYSROOT_DIR}" install
    fi

    cd "${PROJECT_ROOT}"
    log_success "libuv ${libuv_ver} built and installed"
}

# Build readline (command line editing for bash, python, etc.)
build_readline() {
    log_info "Building readline..."

    local readline_ver="8.2"
    local cache_dir="${BUILD_DIR}/sources"
    mkdir -p "${cache_dir}"

    # Check if already built; if present but linked against a ncurses with symbol versions
    # while our sysroot ncurses lacks them, rebuild to avoid runtime warnings.
    if [[ -f "${SYSROOT_DIR}/usr/lib/libreadline.so.8" ]] && [[ -f "${SYSROOT_DIR}/usr/lib/libncursesw.so.6" ]]; then
        local ncurses_has_versions=0
        if readelf --version-info "${SYSROOT_DIR}/usr/lib/libncursesw.so.6" 2>/dev/null | grep -q "Version definition section"; then
            ncurses_has_versions=1
        fi

        if readelf --version-info "${SYSROOT_DIR}/usr/lib/libreadline.so.8" 2>/dev/null | grep -q "NCURSES"; then
            if [[ $ncurses_has_versions -eq 0 ]]; then
                log_warn "readline present but sysroot ncurses lacks symbol versions; rebuilding readline after ncurses fix"
                rm -f "${SYSROOT_DIR}/usr/lib/libreadline"* "${SYSROOT_DIR}/usr/lib/libhistory"* 2>/dev/null || true
            else
                log_info "readline already installed"
                return 0
            fi
        else
            log_info "readline already installed"
            return 0
        fi
    elif [[ -f "${SYSROOT_DIR}/usr/lib/libreadline.so" ]]; then
        log_info "readline already installed"
        return 0
    fi

    local readline_tarball="${cache_dir}/readline-${readline_ver}.tar.gz"
    local readline_src="${cache_dir}/readline-${readline_ver}"

    # Download readline
    if [[ ! -f "${readline_tarball}" ]]; then
        log_info "Downloading readline ${readline_ver}..."
        if ! curl -fsSL -o "${readline_tarball}" \
            "https://ftp.gnu.org/gnu/readline/readline-${readline_ver}.tar.gz"; then
            log_warn "Failed to download readline"
            return 1
        fi
    fi

    # Extract
    if [[ ! -d "${readline_src}" ]]; then
        tar -xzf "${readline_tarball}" -C "${cache_dir}"
    fi

    cd "${readline_src}"

    # Readline needs ncurses
    export LDFLAGS="-L${SYSROOT_DIR}/usr/lib"
    export CPPFLAGS="-I${SYSROOT_DIR}/usr/include"
    export LIBRARY_PATH="${SYSROOT_DIR}/usr/lib"

    ./configure \
        --prefix=/usr \
        --with-curses \
        --enable-shared

    make -j$(nproc) SHLIB_LIBS="-lncursesw"
    make DESTDIR="${SYSROOT_DIR}" install

    unset LDFLAGS CPPFLAGS LIBRARY_PATH

    cd "${PROJECT_ROOT}"
    log_success "readline ${readline_ver} built and installed"
}

# Build attr (extended attributes - required by acl)
build_attr() {
    log_info "Building attr..."

    local attr_ver="2.5.2"
    local cache_dir="${BUILD_DIR}/sources"
    mkdir -p "${cache_dir}"

    # Check if already built
    if [[ -f "${SYSROOT_DIR}/usr/lib/libattr.so" ]]; then
        log_info "attr already installed"
        return 0
    fi

    local attr_tarball="${cache_dir}/attr-${attr_ver}.tar.xz"
    local attr_src="${cache_dir}/attr-${attr_ver}"

    # Download attr
    if [[ ! -f "${attr_tarball}" ]]; then
        log_info "Downloading attr ${attr_ver}..."
        if ! curl -fsSL -o "${attr_tarball}" \
            "https://download.savannah.nongnu.org/releases/attr/attr-${attr_ver}.tar.xz"; then
            log_warn "Failed to download attr"
            return 1
        fi
    fi

    # Extract
    if [[ ! -d "${attr_src}" ]]; then
        tar -xJf "${attr_tarball}" -C "${cache_dir}"
    fi

    cd "${attr_src}"

    ./configure \
        --prefix=/usr \
        --disable-static \
        --sysconfdir=/etc

    make -j$(nproc)
    make DESTDIR="${SYSROOT_DIR}" install

    cd "${PROJECT_ROOT}"
    log_success "attr ${attr_ver} built and installed"
}

# Build acl (Access Control Lists - needed by vim, coreutils, etc.)
build_acl() {
    log_info "Building acl..."

    local acl_ver="2.3.2"
    local cache_dir="${BUILD_DIR}/sources"
    mkdir -p "${cache_dir}"

    # Check if already built
    if [[ -f "${SYSROOT_DIR}/usr/lib/libacl.so" ]]; then
        log_info "acl already installed"
        return 0
    fi

    # acl requires attr
    if [[ ! -f "${SYSROOT_DIR}/usr/lib/libattr.so" ]]; then
        log_info "Building attr dependency first..."
        build_attr || return 1
    fi

    local acl_tarball="${cache_dir}/acl-${acl_ver}.tar.xz"
    local acl_src="${cache_dir}/acl-${acl_ver}"

    # Download acl
    if [[ ! -f "${acl_tarball}" ]]; then
        log_info "Downloading acl ${acl_ver}..."
        if ! curl -fsSL -o "${acl_tarball}" \
            "https://download.savannah.nongnu.org/releases/acl/acl-${acl_ver}.tar.xz"; then
            log_warn "Failed to download acl"
            return 1
        fi
    fi

    # Extract
    if [[ ! -d "${acl_src}" ]]; then
        tar -xJf "${acl_tarball}" -C "${cache_dir}"
    fi

    cd "${acl_src}"

    export LDFLAGS="-L${SYSROOT_DIR}/usr/lib"
    export CPPFLAGS="-I${SYSROOT_DIR}/usr/include"

    ./configure \
        --prefix=/usr \
        --disable-static

    make -j$(nproc)
    make DESTDIR="${SYSROOT_DIR}" install

    unset LDFLAGS CPPFLAGS

    cd "${PROJECT_ROOT}"
    log_success "acl ${acl_ver} built and installed"
}

# Build gpm (General Purpose Mouse - console mouse support)
build_gpm() {
    log_info "Building gpm..."

    local gpm_ver="1.20.7"
    local cache_dir="${BUILD_DIR}/sources"
    mkdir -p "${cache_dir}"

    # Check if already built
    if [[ -f "${SYSROOT_DIR}/usr/lib/libgpm.so" ]]; then
        log_info "gpm already installed"
        return 0
    fi

    local gpm_tarball="${cache_dir}/gpm-${gpm_ver}.tar.bz2"
    # Use a writable copy of the sources; previous builds may have left root-owned trees
    local gpm_src="${cache_dir}/gpm-${gpm_ver}-src"

    # Download gpm
    if [[ ! -f "${gpm_tarball}" ]]; then
        log_info "Downloading gpm ${gpm_ver}..."
        if ! curl -fsSL -o "${gpm_tarball}" \
            "https://www.nico.schottelius.org/software/gpm/archives/gpm-${gpm_ver}.tar.bz2"; then
            # Try alternative mirror
            if ! curl -fsSL -o "${gpm_tarball}" \
                "https://github.com/telmich/gpm/archive/refs/tags/${gpm_ver}.tar.gz"; then
                log_warn "Failed to download gpm - skipping (optional)"
                return 0
            fi
            # It's a .tar.gz from github, rename
            mv "${gpm_tarball}" "${cache_dir}/gpm-${gpm_ver}.tar.gz"
            gpm_tarball="${cache_dir}/gpm-${gpm_ver}.tar.gz"
        fi
    fi

    # Extract
    if [[ ! -d "${gpm_src}" ]]; then
        rm -rf "${gpm_src}"
        mkdir -p "${gpm_src}"
        if [[ "${gpm_tarball}" == *.tar.bz2 ]]; then
            tar -xjf "${gpm_tarball}" --strip-components=1 -C "${gpm_src}"
        else
            tar -xzf "${gpm_tarball}" --strip-components=1 -C "${gpm_src}"
        fi
    fi

    cd "${gpm_src}"

    # Patch for modern glibc: add sys/sysmacros.h include for major() macro
    # This is needed because glibc 2.28+ moved major/minor from sys/types.h to sys/sysmacros.h
    if [[ -f "src/daemon/open_console.c" ]]; then
        if ! grep -q "sys/sysmacros.h" "src/daemon/open_console.c"; then
            sed -i '/#include <sys\/stat.h>/a #include <sys/sysmacros.h>' "src/daemon/open_console.c"
        fi
    elif [[ -f "daemon/open_console.c" ]]; then
        if ! grep -q "sys/sysmacros.h" "daemon/open_console.c"; then
            sed -i '/#include <sys\/stat.h>/a #include <sys/sysmacros.h>' "daemon/open_console.c"
        fi
    fi

    # Clean previous build if exists to ensure fresh configure
    make distclean 2>/dev/null || make clean 2>/dev/null || true

    # gpm requires autoreconf
    if [[ -f "autogen.sh" ]]; then
        ./autogen.sh 2>/dev/null || autoreconf -fi 2>/dev/null || true
    fi

    # --without-curses avoids Gpm_Wgetch type conflict with modern ncurses
    ./configure \
        --prefix=/usr \
        --sysconfdir=/etc \
        --without-curses

    make -j$(nproc) || {
        log_warn "gpm build failed - skipping (optional)"
        cd "${PROJECT_ROOT}"
        return 0
    }
    make DESTDIR="${SYSROOT_DIR}" install

    cd "${PROJECT_ROOT}"
    log_success "gpm ${gpm_ver} built and installed"
}

# Build zlib (compression - needed by many programs)
build_zlib() {
    log_info "Building zlib..."

    local zlib_ver="1.3.1"
    local cache_dir="${BUILD_DIR}/sources"
    mkdir -p "${cache_dir}"

    # Check if already built
    if [[ -f "${SYSROOT_DIR}/usr/lib/libz.so" ]]; then
        log_info "zlib already installed"
        return 0
    fi

    local zlib_tarball="${cache_dir}/zlib-${zlib_ver}.tar.gz"
    local zlib_src="${cache_dir}/zlib-${zlib_ver}"

    # Download zlib
    if [[ ! -f "${zlib_tarball}" ]]; then
        log_info "Downloading zlib ${zlib_ver}..."
        if ! curl -fsSL -o "${zlib_tarball}" \
            "https://zlib.net/zlib-${zlib_ver}.tar.gz"; then
            log_warn "Failed to download zlib"
            return 1
        fi
    fi

    # Extract
    if [[ ! -d "${zlib_src}" ]]; then
        tar -xzf "${zlib_tarball}" -C "${cache_dir}"
    fi

    cd "${zlib_src}"

    ./configure --prefix=/usr

    make -j$(nproc)
    make DESTDIR="${SYSROOT_DIR}" install

    cd "${PROJECT_ROOT}"
    log_success "zlib ${zlib_ver} built and installed"
}

# Copy glibc from host (libc, libm, libpthread, etc.)
copy_glibc() {
    log_info "Copying glibc libraries from host..."

    local -a GLIBC_LIBS=(
        "libc.so*"
        "libm.so*"
        "libpthread.so*"
        "libdl.so*"
        "librt.so*"
        "libresolv.so*"
        "libnss_*.so*"
        "libnsl.so*"
        "libutil.so*"
        "libcrypt.so*"
        "ld-linux-x86-64.so*"
        "ld-linux.so*"
    )

    local -a LIB_DIRS=(
        "/usr/lib"
        "/usr/lib64"
        "/usr/lib/x86_64-linux-gnu"
        "/lib"
        "/lib64"
        "/lib/x86_64-linux-gnu"
    )

    local copied=0

    for pattern in "${GLIBC_LIBS[@]}"; do
        for dir in "${LIB_DIRS[@]}"; do
            [[ -d "$dir" ]] || continue

            for lib in "$dir"/$pattern; do
                [[ -e "$lib" ]] || continue

                local dest="${SYSROOT_DIR}${lib}"
                if [[ ! -f "$dest" ]]; then
                    mkdir -p "$(dirname "$dest")"
                    cp -L "$lib" "$dest" 2>/dev/null && copied=$((copied + 1))
                fi
            done
        done
    done

    log_info "Copied ${copied} glibc libraries"
}

# Build all core dependencies
build_core_deps() {
    log_step "Building core dependencies..."

    # Copy glibc from host first (libc, libm, etc.)
    copy_glibc

    # zlib - compression (used by many programs)
    build_zlib || true

    # ncurses is required for terminal applications
    build_ncurses

    # readline for command line editing (bash, python, etc.)
    build_readline || true

    # attr and acl for file permissions
    build_attr || true
    build_acl || true

    # gpm for console mouse support (optional)
    build_gpm || true

    # libsodium for encryption (vim, etc.)
    build_libsodium || true

    # libuv for async I/O (neovim)
    build_libuv || true

    # libcanberra is optional but useful for audio events
    build_libcanberra || true

    log_success "Core dependencies built"
}

# =============================================================================
# Build Vim from source
# =============================================================================
build_vim() {
    log_info "Building Vim..."

    local vim_ver="9.1.0"
    local cache_dir="${BUILD_DIR}/sources"
    mkdir -p "${cache_dir}"

    # Check if already installed
    if [[ -f "${SYSROOT_DIR}/usr/bin/vim" ]]; then
        log_info "Vim already installed"
        return 0
    fi

    # Ensure ncurses is built first
    if [[ ! -f "${SYSROOT_DIR}/usr/lib/libncursesw.so" ]] && [[ ! -f "${SYSROOT_DIR}/usr/lib/libncurses.so" ]]; then
        log_info "Building ncurses dependency first..."
        build_ncurses
    fi

    local vim_src="${cache_dir}/vim-${vim_ver}-src"

    # Download vim source
    if [[ ! -d "${vim_src}" ]]; then
        log_info "Downloading Vim ${vim_ver} source..."
        rm -rf "${vim_src}"
        if ! git clone --depth=1 --branch v${vim_ver} https://github.com/vim/vim.git "${vim_src}" 2>&1; then
            # Try tarball as fallback
            log_info "Git clone failed, trying tarball..."
            local vim_tarball="${cache_dir}/vim-${vim_ver}.tar.gz"
            if curl -fsSL -o "${vim_tarball}" \
                "https://github.com/vim/vim/archive/refs/tags/v${vim_ver}.tar.gz"; then
                mkdir -p "${vim_src}"
                tar -xzf "${vim_tarball}" --strip-components=1 -C "${vim_src}"
            else
                log_warn "Failed to download Vim source"
                return 1
            fi
        fi
    fi

    cd "${vim_src}"

    # Set up environment to find our built libraries
    # NOTE: Do NOT set LD_LIBRARY_PATH here - it will cause the host shell to load
    # the sysroot's readline which breaks /bin/sh with "undefined symbol: rl_print_keybinding"
    # LIBRARY_PATH is the compile-time equivalent - tells gcc where to find libs without affecting runtime
    export LDFLAGS="-L${SYSROOT_DIR}/usr/lib"
    export CPPFLAGS="-I${SYSROOT_DIR}/usr/include -I${SYSROOT_DIR}/usr/include/ncursesw -I${SYSROOT_DIR}/usr/include/sodium"
    export CFLAGS="-I${SYSROOT_DIR}/usr/include -I${SYSROOT_DIR}/usr/include/ncursesw"
    export PKG_CONFIG_PATH="${SYSROOT_DIR}/usr/lib/pkgconfig"
    export LIBRARY_PATH="${SYSROOT_DIR}/usr/lib"
    export LIBS="-L${SYSROOT_DIR}/usr/lib -lncursesw -ltinfo"
    local ld_library_path="${SYSROOT_DIR}/usr/lib:${SYSROOT_DIR}/lib:${SYSROOT_DIR}/lib64"

    # Determine optional features based on what libraries are available
    local optional_flags=""

    # libsodium for encryption
    if [[ -f "${SYSROOT_DIR}/usr/lib/libsodium.so" ]]; then
        optional_flags="${optional_flags} --enable-libsodium"
        log_info "Enabling libsodium support for vim encryption"
    fi

    # ACL support
    if [[ -f "${SYSROOT_DIR}/usr/lib/libacl.so" ]]; then
        optional_flags="${optional_flags} --enable-acl"
        log_info "Enabling ACL support for vim"
    fi

    # GPM (console mouse) support
    if [[ -f "${SYSROOT_DIR}/usr/lib/libgpm.so" ]]; then
        optional_flags="${optional_flags} --enable-gpm"
        log_info "Enabling GPM (console mouse) support for vim"
    else
        optional_flags="${optional_flags} --disable-gpm"
    fi

    # Clean previous build if exists to ensure fresh configure
    make distclean 2>/dev/null || make clean 2>/dev/null || true

    # Configure vim with ncurses and optional features
    # Vim's configure runs compiled test programs. Since we link against sysroot ncurses/tinfo,
    # ensure those are discoverable at runtime without embedding build-path RUNPATH into the final binary.
    LD_LIBRARY_PATH="${ld_library_path}" ./configure \
        --prefix=/usr \
        --with-features=huge \
        --enable-multibyte \
        --disable-gui \
        --without-x \
        --disable-canberra \
        --with-tlib=ncursesw \
        --enable-cscope \
        --disable-netbeans \
        ${optional_flags}

    if LD_LIBRARY_PATH="${ld_library_path}" make -j$(nproc) && LD_LIBRARY_PATH="${ld_library_path}" make DESTDIR="${SYSROOT_DIR}" install; then
        # Create symlinks
        mkdir -p "${SYSROOT_DIR}/bin"
        ln -sf ../usr/bin/vim "${SYSROOT_DIR}/bin/vim"
        ln -sf ../usr/bin/vim "${SYSROOT_DIR}/bin/vi"
        ln -sf vim "${SYSROOT_DIR}/usr/bin/vi"

        log_success "Vim ${vim_ver} built and installed"
    else
        log_warn "Vim build failed"
        return 1
    fi

    # Unset environment
    unset LDFLAGS CPPFLAGS CFLAGS PKG_CONFIG_PATH LIBRARY_PATH LIBS

    cd "${PROJECT_ROOT}"
}

# Build Neovim from source or download release
build_neovim() {
    log_info "Building Neovim..."

    local nvim_ver="0.9.5"
    local arch="linux64"
    [[ "$(uname -m)" == "aarch64" ]] && arch="linux-arm64"

    local cache_dir="${BUILD_DIR}/sources"
    mkdir -p "${cache_dir}"

    # Download prebuilt release (much faster than building)
    local nvim_tarball="nvim-${arch}.tar.gz"
    local nvim_url="https://github.com/neovim/neovim/releases/download/v${nvim_ver}/${nvim_tarball}"

    if [[ ! -f "${cache_dir}/${nvim_tarball}" ]]; then
        log_info "Downloading Neovim ${nvim_ver}..."
        if curl -fsSL -o "${cache_dir}/${nvim_tarball}" "${nvim_url}"; then
            log_info "Downloaded Neovim"
        else
            # Try alternative URL format
            nvim_tarball="nvim-linux64.tar.gz"
            nvim_url="https://github.com/neovim/neovim/releases/download/stable/${nvim_tarball}"
            if curl -fsSL -o "${cache_dir}/${nvim_tarball}" "${nvim_url}"; then
                log_info "Downloaded Neovim (stable)"
            else
                log_warn "Failed to download Neovim"
                return 0
            fi
        fi
    fi

    # Extract to sysroot
    log_info "Installing Neovim to sysroot..."
    local nvim_extract="${cache_dir}/nvim-extract"
    mkdir -p "${nvim_extract}"
    tar -xzf "${cache_dir}/${nvim_tarball}" -C "${nvim_extract}" --strip-components=1

    # Copy to sysroot
    mkdir -p "${SYSROOT_DIR}/usr/bin" "${SYSROOT_DIR}/usr/share" "${SYSROOT_DIR}/usr/lib"
    cp "${nvim_extract}/bin/nvim" "${SYSROOT_DIR}/usr/bin/"
    chmod 755 "${SYSROOT_DIR}/usr/bin/nvim"

    # Copy bundled libraries from neovim release (if present)
    if [[ -d "${nvim_extract}/lib" ]]; then
        cp -r "${nvim_extract}/lib"/* "${SYSROOT_DIR}/usr/lib/" 2>/dev/null || true
        log_info "Copied bundled neovim libraries"
    fi

    if [[ -d "${nvim_extract}/share/nvim" ]]; then
        cp -r "${nvim_extract}/share/nvim" "${SYSROOT_DIR}/usr/share/"
    fi

    # Copy library dependencies for nvim binary
    copy_binary_deps "${SYSROOT_DIR}/usr/bin/nvim"

    # Create neovim symlink
    ln -sf nvim "${SYSROOT_DIR}/usr/bin/neovim"
    mkdir -p "${SYSROOT_DIR}/bin"
    ln -sf ../usr/bin/nvim "${SYSROOT_DIR}/bin/nvim"

    log_success "Neovim ${nvim_ver} installed"
}

# =============================================================================
# Build SSH Support (OpenSSH)
# =============================================================================
build_ssh() {
    log_step "Building SSH support..."

    mkdir -p "${SYSROOT_DIR}/etc/ssh"
    mkdir -p "${SYSROOT_DIR}/var/lib/sshd"
    chmod 700 "${SYSROOT_DIR}/var/lib/sshd"

    # Build OpenSSH if not present
    if [[ ! -f "${SYSROOT_DIR}/usr/bin/ssh" ]]; then
        build_openssh
    fi

    # Copy SSH configs
    local ssh_config_dir="${PROJECT_ROOT}/configs/ssh"
    if [[ -d "${ssh_config_dir}" ]]; then
        if [[ -f "${ssh_config_dir}/sshd_config" ]]; then
            cp "${ssh_config_dir}/sshd_config" "${SYSROOT_DIR}/etc/ssh/"
            log_info "Installed sshd_config"
        fi
        if [[ -f "${ssh_config_dir}/ssh_config" ]]; then
            cp "${ssh_config_dir}/ssh_config" "${SYSROOT_DIR}/etc/ssh/"
            log_info "Installed ssh_config"
        fi
    fi

    log_success "SSH support built and configured"
}

# Build OpenSSH from source or use system binaries
build_openssh() {
    log_info "Building OpenSSH..."

    local ssh_ver="9.6p1"
    local cache_dir="${BUILD_DIR}/sources"
    mkdir -p "${cache_dir}"

    # Check for system SSH first
    if command -v ssh &>/dev/null && command -v sshd &>/dev/null; then
        log_info "Using host OpenSSH"
        mkdir -p "${SYSROOT_DIR}/usr/bin" "${SYSROOT_DIR}/usr/lib/ssh"

        # Copy SSH binaries
        for bin in ssh scp sftp ssh-keygen ssh-keyscan ssh-add ssh-agent; do
            if [[ -x "/usr/bin/${bin}" ]]; then
                cp "/usr/bin/${bin}" "${SYSROOT_DIR}/usr/bin/"
                log_info "  Copied ${bin}"
            fi
        done

        # Copy sshd
        if [[ -x "/usr/sbin/sshd" ]]; then
            cp "/usr/sbin/sshd" "${SYSROOT_DIR}/usr/bin/"
        elif [[ -x "/usr/bin/sshd" ]]; then
            cp "/usr/bin/sshd" "${SYSROOT_DIR}/usr/bin/"
        fi

        # Copy helper programs
        for helper in sftp-server ssh-keysign; do
            for path in /usr/lib/ssh /usr/libexec/openssh /usr/lib/openssh; do
                if [[ -x "${path}/${helper}" ]]; then
                    mkdir -p "${SYSROOT_DIR}/usr/lib/ssh"
                    cp "${path}/${helper}" "${SYSROOT_DIR}/usr/lib/ssh/"
                    break
                fi
            done
        done

        log_success "OpenSSH binaries installed"
        return 0
    fi

    # Download and build from source
    local ssh_tarball="openssh-${ssh_ver}.tar.gz"
    local ssh_url="https://cdn.openbsd.org/pub/OpenBSD/OpenSSH/portable/${ssh_tarball}"

    if [[ ! -f "${cache_dir}/${ssh_tarball}" ]]; then
        log_info "Downloading OpenSSH ${ssh_ver}..."
        if curl -fsSL -o "${cache_dir}/${ssh_tarball}" "${ssh_url}"; then
            log_info "Downloaded OpenSSH"
        else
            log_warn "Failed to download OpenSSH"
            return 0
        fi
    fi

    # Extract and build
    local ssh_src="${cache_dir}/openssh-${ssh_ver}"
    if [[ ! -d "${ssh_src}" ]]; then
        tar -xzf "${cache_dir}/${ssh_tarball}" -C "${cache_dir}"
    fi

    cd "${ssh_src}"
    if ./configure --prefix=/usr --sysconfdir=/etc/ssh --with-privsep-path=/var/lib/sshd && \
       make -j$(nproc) && \
       make DESTDIR="${SYSROOT_DIR}" install; then
        log_success "OpenSSH ${ssh_ver} built and installed"
    else
        log_warn "OpenSSH build failed - will be available via rvn install openssh"
    fi
    cd "${PROJECT_ROOT}"
}

# =============================================================================
# Build Rust Toolchain
# =============================================================================
build_rust() {
    log_step "Building Rust toolchain..."

    local cache_dir="${BUILD_DIR}/sources"
    mkdir -p "${cache_dir}"

    # Check if Rust is already in sysroot
    if [[ -f "${SYSROOT_DIR}/usr/bin/rustc" ]]; then
        log_info "Rust already installed"
        return 0
    fi

    local arch="x86_64-unknown-linux-gnu"
    [[ "$(uname -m)" == "aarch64" ]] && arch="aarch64-unknown-linux-gnu"

    # Download rustup-init
    local rustup_url="https://static.rust-lang.org/rustup/dist/${arch}/rustup-init"
    local rustup_init="${cache_dir}/rustup-init"

    if [[ ! -f "${rustup_init}" ]]; then
        log_info "Downloading rustup..."
        if curl -fsSL -o "${rustup_init}" "${rustup_url}"; then
            chmod +x "${rustup_init}"
        else
            log_warn "Failed to download rustup"
            # Fallback: check for host rust
            if command -v rustc &>/dev/null; then
                log_info "Using host Rust toolchain"
                mkdir -p "${SYSROOT_DIR}/usr/bin"
                for bin in rustc cargo rustfmt clippy-driver rust-analyzer; do
                    if command -v "${bin}" &>/dev/null; then
                        local src=$(command -v "${bin}")
                        # Resolve symlinks and copy actual binary
                        src=$(readlink -f "$src")
                        cp -L "${src}" "${SYSROOT_DIR}/usr/bin/${bin}" 2>/dev/null || true
                        chmod 755 "${SYSROOT_DIR}/usr/bin/${bin}" 2>/dev/null || true
                        # Copy library dependencies
                        copy_binary_deps "${src}"
                    fi
                done
                log_success "Host Rust installed with dependencies"
                return 0
            fi
            return 0
        fi
    fi

    # Install Rust to sysroot
    log_info "Installing Rust toolchain..."
    export RUSTUP_HOME="${SYSROOT_DIR}/usr/lib/rustup"
    export CARGO_HOME="${SYSROOT_DIR}/usr/lib/cargo"
    mkdir -p "${RUSTUP_HOME}" "${CARGO_HOME}"

    "${rustup_init}" -y --no-modify-path --default-toolchain stable \
        --profile minimal -c rustfmt -c clippy 2>&1 | tee "${LOGS_DIR}/rust-install.log" || true

    # Create symlinks in /usr/bin and copy library dependencies
    mkdir -p "${SYSROOT_DIR}/usr/bin"
    for bin in rustc cargo rustfmt cargo-clippy clippy-driver rustup; do
        if [[ -f "${CARGO_HOME}/bin/${bin}" ]]; then
            ln -sf "../lib/cargo/bin/${bin}" "${SYSROOT_DIR}/usr/bin/${bin}"
            # Copy library dependencies for each rust tool
            copy_binary_deps "${CARGO_HOME}/bin/${bin}"
        fi
    done

    # Copy Rust standard library (needed for compiling)
    if [[ -d "${RUSTUP_HOME}/toolchains" ]]; then
        log_info "Copying Rust standard library..."
        # The stdlib is needed for rustc to work
    fi

    # Create profile script for Rust
    mkdir -p "${SYSROOT_DIR}/etc/profile.d"
    cat > "${SYSROOT_DIR}/etc/profile.d/rust.sh" << 'RUSTPROFILE'
# Rust environment
export RUSTUP_HOME=/usr/lib/rustup
export CARGO_HOME=/usr/lib/cargo
export PATH="$PATH:$CARGO_HOME/bin"
RUSTPROFILE

    log_success "Rust toolchain installed"
}

# =============================================================================
# Build Shells (bash, fish) with Dependencies
# =============================================================================
build_shells() {
    log_step "Building shells..."

    # Ensure bash is available (usually from base system)
    build_bash

    # Build/install fish
    build_fish

    # Set bash as default shell
    set_default_shell

    log_success "Shells built"
}

# Build fish with dependencies
build_fish() {
    log_info "Building fish..."

    local fish_ver="3.7.1"
    local cache_dir="${BUILD_DIR}/sources"
    mkdir -p "${cache_dir}"

    # Check if already installed
    if [[ -f "${SYSROOT_DIR}/usr/bin/fish" ]]; then
        log_info "fish already installed"
        return 0
    fi

    # Try to copy from host first
    if command -v fish &>/dev/null; then
        local host_fish=$(command -v fish)
        log_info "Copying host fish and dependencies..."

        mkdir -p "${SYSROOT_DIR}/usr/bin" "${SYSROOT_DIR}/bin"
        mkdir -p "${SYSROOT_DIR}/usr/share/fish"

        # Copy fish binaries
        cp "${host_fish}" "${SYSROOT_DIR}/usr/bin/fish"
        chmod 755 "${SYSROOT_DIR}/usr/bin/fish"
        ln -sf ../usr/bin/fish "${SYSROOT_DIR}/bin/fish"

        # Copy fish_indent and fish_key_reader if available
        for bin in fish_indent fish_key_reader; do
            if command -v "${bin}" &>/dev/null; then
                cp "$(command -v ${bin})" "${SYSROOT_DIR}/usr/bin/"
            fi
        done

        # Copy fish data files
        for dir in /usr/share/fish; do
            if [[ -d "${dir}" ]]; then
                cp -r "${dir}"/* "${SYSROOT_DIR}${dir}/" 2>/dev/null || true
            fi
        done

        # Copy required shared libraries
        copy_binary_deps "${host_fish}"

        log_success "fish installed from host"
        return 0
    fi

    # Check for cmake - required to build fish from source
    if ! command -v cmake &>/dev/null; then
        log_warn "cmake not found - cannot build fish from source"
        log_info "Install cmake with: pacman -S cmake (Arch) or apt install cmake (Debian)"
        return 0
    fi

    # Download prebuilt or build from source
    local fish_tarball="fish-${fish_ver}.tar.xz"
    local fish_url="https://github.com/fish-shell/fish-shell/releases/download/${fish_ver}/${fish_tarball}"

    if [[ ! -f "${cache_dir}/${fish_tarball}" ]]; then
        log_info "Downloading fish ${fish_ver}..."
        if curl -fsSL -o "${cache_dir}/${fish_tarball}" -L "${fish_url}"; then
            log_info "Downloaded fish"
        else
            log_warn "Failed to download fish - fish will not be available"
            return 0
        fi
    fi

    # Extract and build
    local fish_src="${cache_dir}/fish-${fish_ver}-src"
    if [[ ! -d "${fish_src}" ]]; then
        rm -rf "${fish_src}"
        mkdir -p "${fish_src}"
        tar -xJf "${cache_dir}/${fish_tarball}" --strip-components=1 -C "${fish_src}"
    fi

    cd "${fish_src}"
    # Clean previous build if exists
    rm -rf build 2>/dev/null || true

    # Patch CMakeLists.txt to disable Tests.cmake inclusion (causes cmake "test" target conflict)
    if [[ -f CMakeLists.txt ]]; then
        sed -i 's/include(cmake\/Tests.cmake)/#include(cmake\/Tests.cmake)  # disabled - conflicts with CTest/' CMakeLists.txt 2>/dev/null || true
    fi

    mkdir -p build && cd build
    if cmake .. -DCMAKE_INSTALL_PREFIX=/usr -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=OFF && \
       make -j$(nproc) && \
       make DESTDIR="${SYSROOT_DIR}" install; then
        ln -sf ../usr/bin/fish "${SYSROOT_DIR}/bin/fish"
        log_success "fish ${fish_ver} built and installed"
    else
        log_warn "fish build failed"
    fi
    cd "${PROJECT_ROOT}"
}

# Build bash from source
build_bash() {
    log_info "Building bash..."

    local bash_ver="5.2.21"
    local cache_dir="${BUILD_DIR}/sources"
    mkdir -p "${cache_dir}"

    # Force rebuild to ensure correct linking against our libs (ncurses, readline)
    # even if stage2 copied a host bash.
    rm -f "${SYSROOT_DIR}/bin/bash" "${SYSROOT_DIR}/usr/bin/bash" "${SYSROOT_DIR}/bin/sh"

    local bash_tarball="${cache_dir}/bash-${bash_ver}.tar.gz"
    local bash_src="${cache_dir}/bash-${bash_ver}"

    # Download bash
    if [[ ! -f "${bash_tarball}" ]]; then
        log_info "Downloading bash ${bash_ver}..."
        if ! curl -fsSL -o "${bash_tarball}" \
            "https://ftp.gnu.org/gnu/bash/bash-${bash_ver}.tar.gz"; then
            log_warn "Failed to download bash"
            return 1
        fi
    fi

    # Extract
    if [[ ! -d "${bash_src}" ]]; then
        tar -xzf "${bash_tarball}" -C "${cache_dir}"
    fi

    cd "${bash_src}"

    # Set up environment to find our built libraries (ncurses, readline)
    # -std=gnu89 required for bash's old K&R style C code to compile with modern GCC
    export LDFLAGS="-L${SYSROOT_DIR}/usr/lib -Wl,-rpath,${SYSROOT_DIR}/usr/lib"
    export CPPFLAGS="-I${SYSROOT_DIR}/usr/include -I${SYSROOT_DIR}/usr/include/ncursesw"
    export CFLAGS="-I${SYSROOT_DIR}/usr/include -I${SYSROOT_DIR}/usr/include/ncursesw -std=gnu89"

    # Configure bash
    # --without-bash-malloc: use glibc malloc (better for compatibility)
    # Use bundled readline to avoid symbol mismatch issues (rl_print_keybinding error)
    ./configure \
        --prefix=/usr \
        --bindir=/bin \
        --without-bash-malloc \
        --with-curses

    if make -j$(nproc) && make DESTDIR="${SYSROOT_DIR}" install; then
        # Create symlinks
        ln -sf bash "${SYSROOT_DIR}/bin/sh"
        # Ensure /usr/bin/bash exists (bindir=/bin puts it in /bin)
        mkdir -p "${SYSROOT_DIR}/usr/bin"
        ln -sf ../../bin/bash "${SYSROOT_DIR}/usr/bin/bash"
        ln -sf ../../bin/bash "${SYSROOT_DIR}/usr/bin/sh"

        log_success "Bash ${bash_ver} built and installed"
    else
        log_warn "Bash build failed"
        # Fallback to host copy if build fails
        if command -v bash &>/dev/null; then
            log_warn "Falling back to host bash..."
            cp "$(which bash)" "${SYSROOT_DIR}/bin/bash"
            ln -sf bash "${SYSROOT_DIR}/bin/sh"
        fi
    fi

    unset LDFLAGS CPPFLAGS CFLAGS

    cd "${PROJECT_ROOT}"
}

# Copy shared library dependencies for a binary
copy_binary_deps() {
    local binary="$1"
    local libs

    # Get list of required libraries
    libs=$(ldd "${binary}" 2>/dev/null | grep "=>" | awk '{print $3}' | grep -v "^$" || true)

    for lib in ${libs}; do
        if [[ -f "${lib}" ]]; then
            local lib_dir=$(dirname "${lib}")
            mkdir -p "${SYSROOT_DIR}${lib_dir}"
            if [[ ! -f "${SYSROOT_DIR}${lib}" ]]; then
                cp -L "${lib}" "${SYSROOT_DIR}${lib}" 2>/dev/null || true
            fi
        fi
    done

    # Also copy the dynamic linker
    local ld_linux=$(ldd "${binary}" 2>/dev/null | grep "ld-linux" | awk '{print $1}' || true)
    if [[ -n "${ld_linux}" ]] && [[ -f "${ld_linux}" ]]; then
        local ld_dir=$(dirname "${ld_linux}")
        mkdir -p "${SYSROOT_DIR}${ld_dir}"
        if [[ ! -f "${SYSROOT_DIR}${ld_linux}" ]]; then
            cp -L "${ld_linux}" "${SYSROOT_DIR}${ld_linux}" 2>/dev/null || true
        fi
    fi
}

# =============================================================================
# Install Essential Runtime Libraries
# =============================================================================
# These are libraries commonly needed by applications like vim, neovim, cargo,
# python, etc. that may be loaded via dlopen() at runtime and not detected by ldd.
install_essential_libs() {
    log_step "Installing essential runtime libraries..."

    fixup_soname_symlink() {
        local dir="$1"
        local soname="$2"

        [[ -d "$dir" ]] || return 0

        local latest
        latest="$(ls -1 "${dir}/${soname}."* 2>/dev/null | sort -V | tail -n 1 || true)"
        [[ -n "$latest" ]] || return 0

        ln -sf "$(basename "$latest")" "${dir}/${soname}" 2>/dev/null || true
    }

    fixup_readline_history_symlinks() {
        local dir="$1"
        fixup_soname_symlink "$dir" "libreadline.so.8"
        fixup_soname_symlink "$dir" "libhistory.so.8"
    }

    # Essential library patterns to search for on the host system
    # These cover terminal apps, GUI apps, audio, crypto, compression, etc.
    local -a LIB_PATTERNS=(
        # Terminal/ncurses (vim, neovim, htop, etc.)
        "libncurses*"
        "libncursesw*"
        "libtinfo*"
        "libreadline*"
        "libhistory*"

        # C/C++ runtime
        "libgcc_s*"
        "libstdc++*"
        "libatomic*"

        # Compression
        "libz.so*"
        "liblzma*"
        "libbz2*"
        "libzstd*"

        # Crypto/SSL
        "libssl*"
        "libcrypto*"
        "libgnutls*"

        # FFI and dynamic loading
        "libffi*"
        "libdl*"
        "libltdl*"
        "libtdb*"

        # Python runtime
        "libpython3*"
        "libexpat*"
        "libsqlite3*"

        # Lua runtime (neovim)
        "libluajit*"
        "liblua5*"

        # Audio (libcanberra, pulseaudio, alsa)
        "libcanberra*"
        "libpulse*"
        "libasound*"
        "libsndfile*"
        "libvorbis*"
        "libogg*"
        "libFLAC*"

        # GLib/GTK base (many apps use these)
        "libglib-2*"
        "libgobject-2*"
        "libgio-2*"
        "libgmodule-2*"
        "libgthread-2*"
        "libpcre*"
        "libpcre2*"

        # Math
        "libm.so*"
        "libgmp*"
        "libmpfr*"
        "libmpc*"
        "libisl*"

        # Threading
        "libpthread*"

        # System
        "libc.so*"
        "librt*"
        "libresolv*"
        "libnss*"
        "libnsl*"
        "libutil*"

        # Terminal emulation
        "libvterm*"
        "libtermkey*"
        "libunibilium*"
        "libmsgpack*"
        "libtree-sitter*"

        # Wayland/display (for GUI apps)
        "libwayland-client*"
        "libwayland-cursor*"
        "libwayland-egl*"
        "libxkbcommon*"

        # Input
        "libinput*"
        "libevdev*"
        "libudev*"

        # Misc runtime
        "libsystemd*"
        "libdbus*"
        "libuuid*"
        "libblkid*"
        "libmount*"
    )

    # Search directories for libraries
    local -a LIB_DIRS=(
        "/usr/lib"
        "/usr/lib64"
        "/usr/lib/x86_64-linux-gnu"
        "/lib"
        "/lib64"
        "/lib/x86_64-linux-gnu"
    )

    local copied=0
    local skipped=0

    for pattern in "${LIB_PATTERNS[@]}"; do
        for dir in "${LIB_DIRS[@]}"; do
            [[ -d "$dir" ]] || continue

            # Find matching libraries
            while IFS= read -r -d '' lib; do
                [[ -f "$lib" ]] || continue

                # Determine destination path
                local dest="${SYSROOT_DIR}${lib}"

                # Skip if already present
                if [[ -f "$dest" ]]; then
                    skipped=$((skipped + 1))
                    continue
                fi

                # Create directory and copy library
                mkdir -p "$(dirname "$dest")"
                if cp -L "$lib" "$dest" 2>/dev/null; then
                    copied=$((copied + 1))
                fi
            done < <(find "$dir" -maxdepth 1 -name "$pattern" -print0 2>/dev/null)
        done
    done

    # If both built and host libs were copied, prefer the newest minor version for SONAME links.
    # This prevents breakage like "/bin/bash: undefined symbol: rl_print_keybinding".
    fixup_readline_history_symlinks "${SYSROOT_DIR}/usr/lib"
    fixup_readline_history_symlinks "${SYSROOT_DIR}/usr/lib64"
    fixup_readline_history_symlinks "${SYSROOT_DIR}/lib"
    fixup_readline_history_symlinks "${SYSROOT_DIR}/lib64"

    log_info "Copied ${copied} libraries (${skipped} already present)"

    # Also copy essential terminfo database for terminal apps
    install_terminfo

    log_success "Essential runtime libraries installed"
}

# Install terminfo database for terminal applications
install_terminfo() {
    log_info "Installing terminfo database..."

    local -a TERMINFO_DIRS=(
        "/usr/share/terminfo"
        "/lib/terminfo"
        "/etc/terminfo"
    )

    local terminfo_dest="${SYSROOT_DIR}/usr/share/terminfo"
    mkdir -p "$terminfo_dest"

    for dir in "${TERMINFO_DIRS[@]}"; do
        if [[ -d "$dir" ]]; then
            # Copy common terminal types
            for term in xterm xterm-256color linux vt100 vt220 screen screen-256color tmux tmux-256color alacritty foot kitty rxvt rxvt-unicode; do
                local first_char="${term:0:1}"
                local src_file="$dir/$first_char/$term"

                if [[ -f "$src_file" ]]; then
                    mkdir -p "$terminfo_dest/$first_char"
                    cp "$src_file" "$terminfo_dest/$first_char/" 2>/dev/null || true
                fi
            done
            break  # Only copy from first found directory
        fi
    done

    # Set TERMINFO environment variable in profile
    mkdir -p "${SYSROOT_DIR}/etc/profile.d"
    cat > "${SYSROOT_DIR}/etc/profile.d/terminfo.sh" << 'EOF'
# Terminfo database location
export TERMINFO=/usr/share/terminfo
export TERM="${TERM:-linux}"
EOF

    log_info "Terminfo database installed"
}

# Copy all .so files for a specific library (handles versioned symlinks)
copy_lib_family() {
    local lib_name="$1"
    local -a search_dirs=("/usr/lib" "/usr/lib64" "/usr/lib/x86_64-linux-gnu" "/lib" "/lib64" "/lib/x86_64-linux-gnu")

    for dir in "${search_dirs[@]}"; do
        [[ -d "$dir" ]] || continue

        for lib in "$dir"/${lib_name}*; do
            [[ -e "$lib" ]] || continue

            local dest="${SYSROOT_DIR}${lib}"
            if [[ ! -f "$dest" ]]; then
                mkdir -p "$(dirname "$dest")"
                cp -L "$lib" "$dest" 2>/dev/null || true
            fi
        done
    done
}

# Set bash as the default shell
set_default_shell() {
    log_info "Setting bash as default shell..."

    # Create /etc/shells
    mkdir -p "${SYSROOT_DIR}/etc"
    cat > "${SYSROOT_DIR}/etc/shells" << 'SHELLS'
# Valid login shells - RavenLinux
# Default: bash
/bin/bash
/usr/bin/bash
/bin/fish
/usr/bin/fish
/bin/sh
/usr/bin/sh
SHELLS

    # Set default shell for root to bash
    if [[ -f "${SYSROOT_DIR}/etc/passwd" ]]; then
        sed -i 's|^root:[^:]*:[^:]*:[^:]*:[^:]*:[^:]*:.*$|root:x:0:0:root:/root:/bin/bash|' "${SYSROOT_DIR}/etc/passwd" 2>/dev/null || true
    else
        # Create passwd with bash as default
        cat > "${SYSROOT_DIR}/etc/passwd" << 'PASSWD'
root:x:0:0:root:/root:/bin/bash
PASSWD
    fi

    # Create /etc/default/useradd to set bash as default for new users
    mkdir -p "${SYSROOT_DIR}/etc/default"
    cat > "${SYSROOT_DIR}/etc/default/useradd" << 'USERADD'
# Default values for useradd
GROUP=100
HOME=/home
INACTIVE=-1
EXPIRE=
SHELL=/bin/bash
SKEL=/etc/skel
CREATE_MAIL_SPOOL=yes
USERADD

    log_success "bash set as default shell"
}

# =============================================================================
# Install Shell Tools and Configs
# =============================================================================
install_shell_tools() {
    log_step "Installing shell tools..."

    local shell_tools_dir="${PROJECT_ROOT}/tools/raven-shell-tools"
    mkdir -p "${SYSROOT_DIR}/usr/bin"

    # Install switch-shell
    if [[ -f "${shell_tools_dir}/switch-shell" ]]; then
        cp "${shell_tools_dir}/switch-shell" "${SYSROOT_DIR}/usr/bin/"
        chmod 755 "${SYSROOT_DIR}/usr/bin/switch-shell"
        log_info "Installed switch-shell"
    else
        log_warn "switch-shell not found at ${shell_tools_dir}/switch-shell"
    fi

    # Install shell-reload
    if [[ -f "${shell_tools_dir}/shell-reload" ]]; then
        cp "${shell_tools_dir}/shell-reload" "${SYSROOT_DIR}/usr/bin/"
        chmod 755 "${SYSROOT_DIR}/usr/bin/shell-reload"
        log_info "Installed shell-reload"
    else
        log_warn "shell-reload not found at ${shell_tools_dir}/shell-reload"
    fi

    # Copy shell configurations
    local configs_dir="${PROJECT_ROOT}/configs"

    # bash configs
    if [[ -d "${configs_dir}/bash" ]]; then
        mkdir -p "${SYSROOT_DIR}/etc/bash" "${SYSROOT_DIR}/etc"
        cp "${configs_dir}/bash/"* "${SYSROOT_DIR}/etc/bash/" 2>/dev/null || true
        cp "${configs_dir}/bash/bashrc" "${SYSROOT_DIR}/etc/bashrc" 2>/dev/null || true
        cp "${configs_dir}/bash/bash_profile" "${SYSROOT_DIR}/etc/profile" 2>/dev/null || true
        chmod 644 "${SYSROOT_DIR}/etc/bash/"* "${SYSROOT_DIR}/etc/bashrc" "${SYSROOT_DIR}/etc/profile" 2>/dev/null || true
        # User skeleton
        cp "${configs_dir}/bash/bashrc" "${SYSROOT_DIR}/etc/skel/.bashrc" 2>/dev/null || true
        cp "${configs_dir}/bash/bash_profile" "${SYSROOT_DIR}/etc/skel/.bash_profile" 2>/dev/null || true
        log_info "Installed bash configs"
    fi

    # fish configs
    if [[ -d "${configs_dir}/fish" ]]; then
        mkdir -p "${SYSROOT_DIR}/etc/fish"
        cp "${configs_dir}/fish/"* "${SYSROOT_DIR}/etc/fish/" 2>/dev/null || true
        # User skeleton for fish
        mkdir -p "${SYSROOT_DIR}/etc/skel/.config/fish"
        cp "${configs_dir}/fish/config.fish" "${SYSROOT_DIR}/etc/skel/.config/fish/" 2>/dev/null || true
        log_info "Installed fish configs"
    fi

    log_success "Shell tools and configs installed"
}

# =============================================================================
# Build File Navigation Tools (ranger, fzf)
# =============================================================================
build_file_tools() {
    log_step "Building file navigation tools..."

    # Build fzf
    build_fzf

    # Build ranger
    build_ranger

    log_success "File navigation tools built"
}

# Build fzf
build_fzf() {
    log_info "Building fzf..."

    local fzf_ver="0.46.0"
    local cache_dir="${BUILD_DIR}/sources"
    mkdir -p "${cache_dir}"

    # Check if already installed
    if [[ -f "${SYSROOT_DIR}/usr/bin/fzf" ]]; then
        log_info "fzf already installed"
        return 0
    fi

    # Download prebuilt binary (faster than building)
    local arch="amd64"
    [[ "$(uname -m)" == "aarch64" ]] && arch="arm64"

    local fzf_tarball="fzf-${fzf_ver}-linux_${arch}.tar.gz"
    local fzf_url="https://github.com/junegunn/fzf/releases/download/${fzf_ver}/${fzf_tarball}"

    if [[ ! -f "${cache_dir}/${fzf_tarball}" ]]; then
        log_info "Downloading fzf ${fzf_ver}..."
        if curl -fsSL -o "${cache_dir}/${fzf_tarball}" -L "${fzf_url}"; then
            log_info "Downloaded fzf"
        else
            log_warn "Failed to download fzf"
            return 1
        fi
    fi

    # Extract
    mkdir -p "${SYSROOT_DIR}/usr/bin"
    tar -xzf "${cache_dir}/${fzf_tarball}" -C "${SYSROOT_DIR}/usr/bin" fzf
    chmod 755 "${SYSROOT_DIR}/usr/bin/fzf"

    # Download shell integration scripts
    mkdir -p "${SYSROOT_DIR}/usr/share/fzf"
    for script in completion.bash completion.fish key-bindings.bash key-bindings.fish; do
        local script_url="https://raw.githubusercontent.com/junegunn/fzf/${fzf_ver}/shell/${script}"
        curl -fsSL -o "${SYSROOT_DIR}/usr/share/fzf/${script}" "${script_url}" 2>/dev/null || true
    done

    log_success "fzf ${fzf_ver} installed"
}

# Build ranger
build_ranger() {
    log_info "Building ranger..."

    local ranger_ver="1.9.3"
    local cache_dir="${BUILD_DIR}/sources"
    mkdir -p "${cache_dir}"

    # Check if already installed
    if [[ -f "${SYSROOT_DIR}/usr/bin/ranger" ]]; then
        log_info "ranger already installed"
        return 0
    fi

    # Ranger requires Python - check if available
    if [[ ! -f "${SYSROOT_DIR}/usr/bin/python3" ]] && ! command -v python3 &>/dev/null; then
        log_warn "Python not available - skipping ranger"
        return 1
    fi

    # Try to copy from host first
    if command -v ranger &>/dev/null; then
        local host_ranger=$(command -v ranger)
        mkdir -p "${SYSROOT_DIR}/usr/bin"
        cp "${host_ranger}" "${SYSROOT_DIR}/usr/bin/ranger"
        chmod 755 "${SYSROOT_DIR}/usr/bin/ranger"

        # Copy ranger library
        if [[ -d "/usr/lib/python3/dist-packages/ranger" ]]; then
            mkdir -p "${SYSROOT_DIR}/usr/lib/python3/dist-packages"
            cp -r "/usr/lib/python3/dist-packages/ranger" "${SYSROOT_DIR}/usr/lib/python3/dist-packages/"
        fi

        log_success "ranger installed from host"
        return 0
    fi

    # Download and install
    local ranger_tarball="ranger-${ranger_ver}.tar.gz"
    local ranger_url="https://github.com/ranger/ranger/archive/refs/tags/v${ranger_ver}.tar.gz"

    if [[ ! -f "${cache_dir}/${ranger_tarball}" ]]; then
        log_info "Downloading ranger ${ranger_ver}..."
        if curl -fsSL -o "${cache_dir}/${ranger_tarball}" -L "${ranger_url}"; then
            log_info "Downloaded ranger"
        else
            log_warn "Failed to download ranger"
            return 1
        fi
    fi

    # Extract and install
    local ranger_src="${cache_dir}/ranger-${ranger_ver}"
    if [[ ! -d "${ranger_src}" ]]; then
        tar -xzf "${cache_dir}/${ranger_tarball}" -C "${cache_dir}"
    fi

    cd "${ranger_src}"
    python3 setup.py install --prefix=/usr --root="${SYSROOT_DIR}" 2>/dev/null || \
        pip3 install --target="${SYSROOT_DIR}/usr/lib/python3/dist-packages" . 2>/dev/null || \
        log_warn "ranger installation failed"

    # Create simple launcher if install failed
    if [[ ! -f "${SYSROOT_DIR}/usr/bin/ranger" ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/bin"
        cat > "${SYSROOT_DIR}/usr/bin/ranger" << 'RANGER_LAUNCH'
#!/usr/bin/env python3
import sys
sys.path.insert(0, '/usr/lib/python3/dist-packages')
from ranger import main
main()
RANGER_LAUNCH
        chmod 755 "${SYSROOT_DIR}/usr/bin/ranger"
    fi

    cd "${PROJECT_ROOT}"
    log_success "ranger installed"
}

# =============================================================================
# Summary
# =============================================================================
print_summary() {
    echo ""
    echo -e "${CYAN}=========================================="
    echo "  Package Build Summary"
    echo "==========================================${NC}"
    echo ""

    echo -e "${CYAN}RavenLinux Tools:${NC}"
    local packages=(vem carrion ivaldi rvn raven-compositor raven-installer raven-usb wifi)
    for pkg in "${packages[@]}"; do
        if [[ -f "${SYSROOT_DIR}/bin/${pkg}" ]] || [[ -f "${SYSROOT_DIR}/usr/bin/${pkg}" ]]; then
            local bin_path="${SYSROOT_DIR}/bin/${pkg}"
            [[ ! -f "${bin_path}" ]] && bin_path="${SYSROOT_DIR}/usr/bin/${pkg}"
            local size
            size=$(du -h "${bin_path}" 2>/dev/null | cut -f1)
            echo -e "  ${GREEN}[OK]${NC} ${pkg} (${size})"
        else
            echo -e "  ${YELLOW}[--]${NC} ${pkg} (not built)"
        fi
    done

    if [[ -f "${PACKAGES_DIR}/boot/raven-boot.efi" ]]; then
        local size
        size=$(du -h "${PACKAGES_DIR}/boot/raven-boot.efi" | cut -f1)
        echo -e "  ${GREEN}[OK]${NC} raven-boot.efi (${size})"
    else
        echo -e "  ${YELLOW}[--]${NC} raven-boot.efi (not built)"
    fi

    echo ""
    echo -e "${CYAN}Shells:${NC}"
    for shell in bash fish; do
        if [[ -f "${SYSROOT_DIR}/usr/bin/${shell}" ]] || [[ -f "${SYSROOT_DIR}/bin/${shell}" ]]; then
            echo -e "  ${GREEN}[OK]${NC} ${shell}"
        else
            echo -e "  ${YELLOW}[--]${NC} ${shell}"
        fi
    done

    echo ""
    echo -e "${CYAN}Shell Tools:${NC}"
    for tool in switch-shell shell-reload; do
        if [[ -f "${SYSROOT_DIR}/usr/bin/${tool}" ]]; then
            echo -e "  ${GREEN}[OK]${NC} ${tool}"
        else
            echo -e "  ${YELLOW}[--]${NC} ${tool}"
        fi
    done

    echo ""
    echo -e "${CYAN}Development Tools:${NC}"
    for tool in gcc g++ go python3 rustc cargo; do
        if [[ -f "${SYSROOT_DIR}/usr/bin/${tool}" ]] || [[ -L "${SYSROOT_DIR}/usr/bin/${tool}" ]]; then
            echo -e "  ${GREEN}[OK]${NC} ${tool}"
        else
            echo -e "  ${YELLOW}[--]${NC} ${tool}"
        fi
    done

    echo ""
    echo -e "${CYAN}Editors:${NC}"
    for editor in vim vi nvim neovim; do
        if [[ -f "${SYSROOT_DIR}/usr/bin/${editor}" ]] || [[ -L "${SYSROOT_DIR}/usr/bin/${editor}" ]]; then
            echo -e "  ${GREEN}[OK]${NC} ${editor}"
        else
            echo -e "  ${YELLOW}[--]${NC} ${editor}"
        fi
    done

    echo ""
    echo -e "${CYAN}Networking:${NC}"
    for tool in ssh scp sshd sftp; do
        if [[ -f "${SYSROOT_DIR}/usr/bin/${tool}" ]]; then
            echo -e "  ${GREEN}[OK]${NC} ${tool}"
        else
            echo -e "  ${YELLOW}[--]${NC} ${tool}"
        fi
    done

    echo ""
    echo -e "${CYAN}File Navigation:${NC}"
    for tool in fzf ranger; do
        if [[ -f "${SYSROOT_DIR}/usr/bin/${tool}" ]]; then
            echo -e "  ${GREEN}[OK]${NC} ${tool}"
        else
            echo -e "  ${YELLOW}[--]${NC} ${tool}"
        fi
    done

    echo ""
    echo -e "${CYAN}Configuration Files:${NC}"
    [[ -f "${SYSROOT_DIR}/etc/ssh/sshd_config" ]] && echo -e "  ${GREEN}[OK]${NC} SSH config"
    [[ -f "${SYSROOT_DIR}/etc/xdg/nvim/init.lua" ]] && echo -e "  ${GREEN}[OK]${NC} Neovim config"
    [[ -f "${SYSROOT_DIR}/etc/vim/vimrc" ]] && echo -e "  ${GREEN}[OK]${NC} Vim config"
    [[ -f "${SYSROOT_DIR}/etc/shells" ]] && echo -e "  ${GREEN}[OK]${NC} /etc/shells"
    [[ -f "${SYSROOT_DIR}/etc/bashrc" ]] && echo -e "  ${GREEN}[OK]${NC} bashrc"
    [[ -f "${SYSROOT_DIR}/etc/fish/config.fish" ]] && echo -e "  ${GREEN}[OK]${NC} fish config"
    [[ -f "${SYSROOT_DIR}/root/.bashrc" ]] && echo -e "  ${GREEN}[OK]${NC} root bashrc"

    # Check default shell
    echo ""
    if grep -q "/bin/bash" "${SYSROOT_DIR}/etc/passwd" 2>/dev/null; then
        echo -e "  ${GREEN}[OK]${NC} Default shell: bash"
    else
        echo -e "  ${YELLOW}[--]${NC} Default shell: not set to bash"
    fi
    echo ""
}

# =============================================================================
# Main
# =============================================================================
main() {
    echo ""
    echo "=========================================="
    echo "  Stage 3: Building Packages"
    echo "=========================================="
    echo ""

    mkdir -p "${LOGS_DIR}" "${PACKAGES_DIR}/bin" "${PACKAGES_DIR}/boot"

    # Build core dependencies first (ncurses, libcanberra, etc.)
    # These are required by shells, editors, and many other tools
    build_core_deps

    # Build custom RavenLinux tools
    build_go_packages
    build_rvn
    # NOTE: Hyprland compositor is copied from host in build-live-iso.sh
    build_installer
    build_usb_creator
    build_wifi_tools
    build_bootloader

    # Build shells (bash, fish) - MUST come early as default shell
    build_shells

    # Build and install development tools
    build_dev_tools
    build_rust

    # Build editors
    build_editors

    # Build SSH support
    build_ssh

    # Install shell tools and configs
    install_shell_tools

    # Build file navigation tools
    build_file_tools

    # Install essential runtime libraries (ncurses, libcanberra, glib, etc.)
    # These are needed by vim, neovim, cargo, python, and other applications
    # but may not be detected by ldd because they're loaded via dlopen()
    install_essential_libs

    print_summary

    log_success "Stage 3 complete!"
    echo ""
}

# Run main (whether executed directly or sourced)
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
else
    main "$@"
fi
