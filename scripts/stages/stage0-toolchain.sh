#!/bin/bash
# Stage 0: Build Cross-Compilation Toolchain
# This builds binutils, GCC, and musl libc for cross-compiling RavenLinux

set -euo pipefail

# Package versions
BINUTILS_VERSION="2.42"
GCC_VERSION="14.2.0"
MUSL_VERSION="1.2.5"
LINUX_VERSION="6.11"
GMP_VERSION="6.3.0"
MPFR_VERSION="4.2.1"
MPC_VERSION="1.3.1"

# URLs
BINUTILS_URL="https://ftp.gnu.org/gnu/binutils/binutils-${BINUTILS_VERSION}.tar.xz"
GCC_URL="https://ftp.gnu.org/gnu/gcc/gcc-${GCC_VERSION}/gcc-${GCC_VERSION}.tar.xz"
MUSL_URL="https://musl.libc.org/releases/musl-${MUSL_VERSION}.tar.gz"
LINUX_URL="https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-${LINUX_VERSION}.tar.xz"
GMP_URL="https://ftp.gnu.org/gnu/gmp/gmp-${GMP_VERSION}.tar.xz"
MPFR_URL="https://ftp.gnu.org/gnu/mpfr/mpfr-${MPFR_VERSION}.tar.xz"
MPC_URL="https://ftp.gnu.org/gnu/mpc/mpc-${MPC_VERSION}.tar.gz"

# Export toolchain paths
export PATH="${TOOLCHAIN_DIR}/bin:${PATH}"

download_toolchain_sources() {
    log_info "Downloading toolchain sources..."

    download_source "binutils" "$BINUTILS_URL"
    download_source "gcc" "$GCC_URL"
    download_source "musl" "$MUSL_URL"
    download_source "linux" "$LINUX_URL"
    download_source "gmp" "$GMP_URL"
    download_source "mpfr" "$MPFR_URL"
    download_source "mpc" "$MPC_URL"

    log_success "All toolchain sources downloaded"
}

build_binutils() {
    log_info "Building binutils ${BINUTILS_VERSION}..."

    local build_dir="${SOURCES_DIR}/binutils-build"
    rm -rf "$build_dir"
    mkdir -p "$build_dir"

    extract_source "binutils-${BINUTILS_VERSION}.tar.xz"

    cd "$build_dir"
    "${SOURCES_DIR}/binutils-${BINUTILS_VERSION}/configure" \
        --prefix="${TOOLCHAIN_DIR}" \
        --target="${RAVEN_TARGET}" \
        --with-sysroot="${SYSROOT_DIR}" \
        --disable-nls \
        --disable-werror \
        --disable-multilib \
        2>&1 | tee "${LOGS_DIR}/binutils-configure.log"

    make -j"${RAVEN_JOBS}" 2>&1 | tee "${LOGS_DIR}/binutils-build.log"
    make install 2>&1 | tee "${LOGS_DIR}/binutils-install.log"

    cd "${RAVEN_ROOT}"
    rm -rf "$build_dir"

    log_success "binutils built successfully"
}

install_linux_headers() {
    log_info "Installing Linux headers ${LINUX_VERSION}..."

    extract_source "linux-${LINUX_VERSION}.tar.xz"

    cd "${SOURCES_DIR}/linux-${LINUX_VERSION}"
    make mrproper
    make ARCH="${RAVEN_ARCH}" headers
    find usr/include -type f ! -name '*.h' -delete
    cp -rv usr/include "${SYSROOT_DIR}/usr/"

    cd "${RAVEN_ROOT}"

    log_success "Linux headers installed"
}

build_gcc_stage1() {
    log_info "Building GCC ${GCC_VERSION} (Stage 1 - C only)..."

    local build_dir="${SOURCES_DIR}/gcc-build-stage1"
    rm -rf "$build_dir"
    mkdir -p "$build_dir"

    extract_source "gcc-${GCC_VERSION}.tar.xz"

    # Extract and link GCC dependencies
    extract_source "gmp-${GMP_VERSION}.tar.xz"
    extract_source "mpfr-${MPFR_VERSION}.tar.xz"
    extract_source "mpc-${MPC_VERSION}.tar.gz"

    mv "${SOURCES_DIR}/gmp-${GMP_VERSION}" "${SOURCES_DIR}/gcc-${GCC_VERSION}/gmp"
    mv "${SOURCES_DIR}/mpfr-${MPFR_VERSION}" "${SOURCES_DIR}/gcc-${GCC_VERSION}/mpfr"
    mv "${SOURCES_DIR}/mpc-${MPC_VERSION}" "${SOURCES_DIR}/gcc-${GCC_VERSION}/mpc"

    cd "$build_dir"
    "${SOURCES_DIR}/gcc-${GCC_VERSION}/configure" \
        --prefix="${TOOLCHAIN_DIR}" \
        --target="${RAVEN_TARGET}" \
        --with-sysroot="${SYSROOT_DIR}" \
        --with-newlib \
        --without-headers \
        --disable-nls \
        --disable-shared \
        --disable-multilib \
        --disable-threads \
        --disable-libatomic \
        --disable-libgomp \
        --disable-libquadmath \
        --disable-libssp \
        --disable-libvtv \
        --disable-libstdcxx \
        --enable-languages=c \
        2>&1 | tee "${LOGS_DIR}/gcc-stage1-configure.log"

    make -j"${RAVEN_JOBS}" all-gcc all-target-libgcc 2>&1 | tee "${LOGS_DIR}/gcc-stage1-build.log"
    make install-gcc install-target-libgcc 2>&1 | tee "${LOGS_DIR}/gcc-stage1-install.log"

    cd "${RAVEN_ROOT}"
    rm -rf "$build_dir"

    log_success "GCC Stage 1 built successfully"
}

build_musl() {
    log_info "Building musl ${MUSL_VERSION}..."

    extract_source "musl-${MUSL_VERSION}.tar.gz"

    cd "${SOURCES_DIR}/musl-${MUSL_VERSION}"

    CC="${RAVEN_TARGET}-gcc" \
    CROSS_COMPILE="${RAVEN_TARGET}-" \
    ./configure \
        --prefix=/usr \
        --target="${RAVEN_TARGET}" \
        2>&1 | tee "${LOGS_DIR}/musl-configure.log"

    make -j"${RAVEN_JOBS}" 2>&1 | tee "${LOGS_DIR}/musl-build.log"
    make DESTDIR="${SYSROOT_DIR}" install 2>&1 | tee "${LOGS_DIR}/musl-install.log"

    # Create necessary symlinks
    ln -sf libc.so "${SYSROOT_DIR}/usr/lib/ld-musl-${RAVEN_ARCH}.so.1"
    mkdir -p "${SYSROOT_DIR}/lib"
    ln -sf ../usr/lib/ld-musl-${RAVEN_ARCH}.so.1 "${SYSROOT_DIR}/lib/ld-musl-${RAVEN_ARCH}.so.1"

    cd "${RAVEN_ROOT}"

    log_success "musl built successfully"
}

build_gcc_stage2() {
    log_info "Building GCC ${GCC_VERSION} (Stage 2 - Full)..."

    local build_dir="${SOURCES_DIR}/gcc-build-stage2"
    rm -rf "$build_dir"
    mkdir -p "$build_dir"

    cd "$build_dir"
    "${SOURCES_DIR}/gcc-${GCC_VERSION}/configure" \
        --prefix="${TOOLCHAIN_DIR}" \
        --target="${RAVEN_TARGET}" \
        --with-sysroot="${SYSROOT_DIR}" \
        --disable-nls \
        --disable-multilib \
        --enable-languages=c,c++ \
        --enable-default-pie \
        --enable-default-ssp \
        2>&1 | tee "${LOGS_DIR}/gcc-stage2-configure.log"

    make -j"${RAVEN_JOBS}" 2>&1 | tee "${LOGS_DIR}/gcc-stage2-build.log"
    make install 2>&1 | tee "${LOGS_DIR}/gcc-stage2-install.log"

    cd "${RAVEN_ROOT}"
    rm -rf "$build_dir"

    log_success "GCC Stage 2 built successfully"
}

# Main Stage 0 execution
download_toolchain_sources
build_binutils
install_linux_headers
build_gcc_stage1
build_musl
build_gcc_stage2

log_success "=== Stage 0 Complete: Cross Toolchain Ready ==="
log_info "Toolchain installed to: ${TOOLCHAIN_DIR}"
log_info "Sysroot installed to: ${SYSROOT_DIR}"
