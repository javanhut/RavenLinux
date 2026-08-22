# =============================================================================
# RavenLinux Build Environment
# =============================================================================
# A reproducible Linux build host for RavenLinux, usable from macOS/Windows
# (or any Linux) via Docker or Podman.
#
# RavenLinux is a Linux-From-Scratch style distro: the build performs chroot,
# overlayfs mounts, loop-device setup and runs a musl cross-toolchain. None of
# that works on macOS natively, so this image provides the Linux host.
#
# IMPORTANT: the container must run --privileged (chroot/mount/loop devices).
# See scripts/docker-build.sh for the recommended invocation, or:
#
#   docker build -t ravenlinux-build .
#   docker run --rm -it --privileged \
#       -v "$PWD:/raven" -w /raven \
#       ravenlinux-build ./scripts/build.sh all
#
# Arch Linux is used as the base because RavenLinux is primarily tested there
# and check-deps.sh maps the most complete package set to pacman.
#
# The base system is a console system -- no graphical stack is built, so no
# GUI toolkits are installed here. Add what you need alongside whatever you
# add back to the distribution.
# =============================================================================

# RavenLinux only targets x86_64, and Arch Linux only publishes an x86_64 image
# (there is no arm64 variant). Pin the platform so the build works everywhere,
# including Apple Silicon (arm64) hosts, where it runs under emulation.
FROM --platform=linux/amd64 archlinux:latest

# Avoid interactive prompts; keep pacman caches out of the image layers.
ENV LANG=C.UTF-8

# -----------------------------------------------------------------------------
# Disable pacman's seccomp download sandbox
# -----------------------------------------------------------------------------
# pacman 7's download sandbox installs a seccomp syscall filter. Under qemu-user
# emulation (building this amd64 image on an arm64 host, e.g. Apple Silicon)
# seccomp() returns EINVAL, so pacman aborts with:
#   error: error restricting syscalls via seccomp: 22
#   error: switching to sandbox user 'alpm' failed!
# Uncommenting DisableSandboxSyscalls turns off only that syscall filter; the
# build still runs in a throwaway container. No-op on native x86_64 hosts.
RUN sed -i 's/^#DisableSandboxSyscalls/DisableSandboxSyscalls/' /etc/pacman.conf

# -----------------------------------------------------------------------------
# System + build dependencies
# -----------------------------------------------------------------------------
# Mirrors scripts/check-deps.sh (command deps + EXTRA_PACKAGES_ARCH). Grouped to
# match that file so the two stay easy to diff.
RUN pacman -Syu --noconfirm --needed \
        # Core build toolchain
        base-devel make gcc binutils \
        # Archive / compression
        tar gzip xz bzip2 cpio zstd unzip \
        # Download + VCS
        curl wget git \
        # File / text utilities
        findutils file patch coreutils rsync sed gawk grep diffutils which less \
        # Disk / filesystem / ISO tooling
        squashfs-tools xorriso util-linux e2fsprogs dosfstools mtools grub \
        # Build systems
        meson ninja cmake pkgconf autoconf automake libtool m4 gettext gperf \
        # Kernel build
        bc flex bison perl python python-jinja openssl \
        linux-headers libelf pahole \
        # Core libraries / dev headers used by the base system
        ncurses zlib libffi \
        # uutils-coreutils builds onig_sys with RUSTONIG_SYSTEM_LIBONIG=1
        # (the crate's bundled oniguruma fails to compile with modern GCC),
        # so it needs the system library plus oniguruma.pc.
        oniguruma \
        # Misc runtime utilities used by the build/test scripts
        kexec-tools inetutils \
    && pacman -Scc --noconfirm

# -----------------------------------------------------------------------------
# Rust via rustup
# -----------------------------------------------------------------------------
# The build needs cargo AND the rust-src component (for the musl cross target).
# rustup gives both reliably, independent of Arch's rust packaging.
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
RUN pacman -Syu --noconfirm --needed rustup \
    && rustup default stable \
    && rustup component add rust-src \
    && pacman -Scc --noconfirm

WORKDIR /raven

CMD ["/bin/bash"]
