#!/bin/bash
# =============================================================================
# RavenLinux Stage 3: Base Packages
# =============================================================================
# Builds the packages that sit on top of the stage 2 sysroot but are not part
# of the cross-built base itself:
#
#   - core libraries the shells and OpenSSH link against (zlib, ncurses,
#     readline, attr, acl)
#   - the shells (bash, fish) and their configuration
#   - OpenSSH client + server
#   - RavenBoot, the UEFI bootloader (bootloader/)
#   - the runtime libraries and terminfo entries a console system needs
#
# Anything beyond that -- editors, language toolchains, a desktop -- is
# deliberately out of scope. Add it here (or in its own stage) as you grow the
# distribution back out.
#
# Package definitions live in packages/core and packages/base.

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
# Core libraries
# =============================================================================
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

    # zlib - compression (OpenSSH, and much of the base)
    build_zlib || true

    # ncurses is required for terminal applications
    build_ncurses

    # readline for command line editing (bash)
    build_readline || true

    # attr and acl for file permissions
    build_attr || true
    build_acl || true

    log_success "Core dependencies built"
}

# =============================================================================
# SSH
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
# Build RavenBoot (UEFI bootloader)
# =============================================================================
# Produces packages/boot/raven-boot.efi, which stage4 installs as the primary
# UEFI bootloader (falling back to GRUB when it is missing).
build_bootloader() {
    log_step "Building RavenBoot (UEFI bootloader)..."

    local bootloader_dir="${PROJECT_ROOT}/bootloader"

    if [[ ! -d "${bootloader_dir}" ]]; then
        log_warn "bootloader/ not found, skipping RavenBoot"
        return 0
    fi

    if ! command -v cargo &>/dev/null; then
        log_warn "Cargo not found, skipping RavenBoot build"
        return 0
    fi

    # The UEFI target is not installed by default.
    if command -v rustup &>/dev/null; then
        if ! rustup target list --installed 2>/dev/null | grep -qx "x86_64-unknown-uefi"; then
            if ! rustup target add x86_64-unknown-uefi; then
                log_warn "Failed to add the x86_64-unknown-uefi target, skipping RavenBoot"
                return 0
            fi
        fi
    fi

    mkdir -p "${PACKAGES_DIR}/boot"

    if (cd "${bootloader_dir}" && cargo build --release --target x86_64-unknown-uefi); then
        local efi="${bootloader_dir}/target/x86_64-unknown-uefi/release/raven-boot.efi"
        if [[ -f "${efi}" ]]; then
            cp "${efi}" "${PACKAGES_DIR}/boot/raven-boot.efi"
            log_success "RavenBoot built -> ${PACKAGES_DIR}/boot/raven-boot.efi"
        else
            log_warn "RavenBoot built but raven-boot.efi not found at ${efi}"
        fi
    else
        log_warn "RavenBoot build failed; stage4 will fall back to GRUB for UEFI"
    fi

    cd "${PROJECT_ROOT}"
}

# =============================================================================
# Shells
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

        mkdir -p "${SYSROOT_DIR}/usr/bin"
        mkdir -p "${SYSROOT_DIR}/usr/share/fish"

        # Copy fish binaries. No /bin/fish link: /bin is a symlink onto
        # /usr/bin, so `ln -sf ../usr/bin/fish ${SYSROOT}/bin/fish` would
        # unlink the binary and leave a dangler pointing at /usr/usr/bin/fish.
        cp "${host_fish}" "${SYSROOT_DIR}/usr/bin/fish"
        chmod 755 "${SYSROOT_DIR}/usr/bin/fish"

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
        # --prefix=/usr already put it in /usr/bin, which IS /bin post-merge.
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
    rm -f "${SYSROOT_DIR}/usr/bin/bash" "${SYSROOT_DIR}/usr/bin/sh"

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
    # --bindir=/usr/bin, not /bin. With bindir=/bin the install landed in the
    # merge symlink and then needed two "repair" links --
    #   ln -sf ../../bin/bash "${SYSROOT_DIR}/usr/bin/bash"
    #   ln -sf ../../bin/bash "${SYSROOT_DIR}/usr/bin/sh"
    # -- each of which is a self-reference once /bin is a symlink, i.e. ELOOP
    # on /usr/bin/bash and /usr/bin/sh. That is the single worst failure in the
    # merge: /init and 18 other scripts have a #!/bin/sh or #!/bin/bash
    # shebang, so the ISO would build clean and boot with no shell at all.
    ./configure \
        --prefix=/usr \
        --bindir=/usr/bin \
        --without-bash-malloc \
        --with-curses

    if make -j$(nproc) && make DESTDIR="${SYSROOT_DIR}" install; then
        # Same-directory alias, the only shape that is safe here.
        ln -sf bash "${SYSROOT_DIR}/usr/bin/sh"

        log_success "Bash ${bash_ver} built and installed"
    else
        log_warn "Bash build failed"
        # Fallback to host copy if build fails
        if command -v bash &>/dev/null; then
            log_warn "Falling back to host bash..."
            cp "$(which bash)" "${SYSROOT_DIR}/usr/bin/bash"
            ln -sf bash "${SYSROOT_DIR}/usr/bin/sh"
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
        # Terminal/ncurses
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

        # Crypto/SSL (OpenSSH)
        "libssl*"
        "libcrypto*"

        # FFI and dynamic loading
        "libffi*"
        "libdl*"
        "libltdl*"

        # Math
        "libm.so*"

        # Threading
        "libpthread*"

        # System
        "libc.so*"
        "librt*"
        "libresolv*"
        "libnss*"
        "libnsl*"
        "libutil*"

        # Block devices / mount (util-linux tools in the base)
        "libudev*"
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
    # /usr/lib64, /lib and /lib64 are all symlinks onto /usr/lib now, so one
    # call covers what used to be four passes over the same directory.
    fixup_readline_history_symlinks "${SYSROOT_DIR}/usr/lib"

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
    log_step "Installing shell configuration..."

    mkdir -p "${SYSROOT_DIR}/usr/bin" "${SYSROOT_DIR}/etc/skel"

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

    log_success "Shell configuration installed"
}

# =============================================================================
# Summary
# =============================================================================
print_summary() {
    echo ""
    echo "=========================================="
    echo "  Stage 3 Summary"
    echo "=========================================="
    echo ""

    echo "Shells:"
    for shell in bash fish; do
        if [[ -f "${SYSROOT_DIR}/usr/bin/${shell}" ]]; then
            echo "  [OK] ${shell}"
        else
            echo "  [--] ${shell}"
        fi
    done

    echo ""
    echo "Networking:"
    for tool in ssh scp sshd sftp; do
        if [[ -f "${SYSROOT_DIR}/usr/bin/${tool}" ]]; then
            echo "  [OK] ${tool}"
        else
            echo "  [--] ${tool}"
        fi
    done

    echo ""
    echo "Bootloader:"
    if [[ -f "${PACKAGES_DIR}/boot/raven-boot.efi" ]]; then
        echo "  [OK] raven-boot.efi ($(du -h "${PACKAGES_DIR}/boot/raven-boot.efi" | cut -f1))"
    else
        echo "  [--] raven-boot.efi (not built)"
    fi

    echo ""
    echo "Configuration:"
    [[ -f "${SYSROOT_DIR}/etc/ssh/sshd_config" ]] && echo "  [OK] SSH config"
    [[ -f "${SYSROOT_DIR}/etc/shells" ]]          && echo "  [OK] /etc/shells"
    [[ -f "${SYSROOT_DIR}/etc/bashrc" ]]          && echo "  [OK] bashrc"
    [[ -f "${SYSROOT_DIR}/etc/fish/config.fish" ]] && echo "  [OK] fish config"

    echo ""
    if grep -q "/bin/bash" "${SYSROOT_DIR}/etc/passwd" 2>/dev/null; then
        echo "  [OK] Default shell: bash"
    else
        echo "  [--] Default shell: not set to bash"
    fi
    echo ""
}

# =============================================================================
# Main
# =============================================================================
main() {
    echo ""
    echo "=========================================="
    echo "  Stage 3: Base Packages"
    echo "=========================================="
    echo ""

    mkdir -p "${LOGS_DIR}" "${PACKAGES_DIR}/bin" "${PACKAGES_DIR}/boot"

    # Libraries the shells and OpenSSH link against
    build_core_deps

    # Shells (bash, fish) and the default-shell wiring
    build_shells

    # OpenSSH client + server
    build_ssh

    # RavenBoot UEFI bootloader -> packages/boot/raven-boot.efi (used by stage4)
    build_bootloader

    # Shell configuration from configs/
    install_shell_tools

    # Runtime libraries loaded via dlopen() that ldd will not report,
    # plus the terminfo database
    install_essential_libs

    print_summary

    log_success "Stage 3 complete!"
    echo ""
}

# Run main (whether executed directly or sourced)
main "$@"
