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
# =============================================================================

FROM archlinux:latest

# Avoid interactive prompts; keep pacman caches out of the image layers.
ENV LANG=C.UTF-8

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
        squashfs-tools xorriso util-linux e2fsprogs dosfstools mtools \
        # Build systems
        meson ninja cmake pkgconf autoconf automake libtool m4 gettext gperf \
        # Kernel build
        bc flex bison perl python python-jinja openssl \
        linux-headers libelf pahole \
        # Go toolchain (1.23+)
        go \
        # Libraries / dev headers
        ncurses zlib libffi \
        # Wayland / compositor build deps
        wayland wayland-protocols libxkbcommon pixman libdrm mesa libinput \
        seatd swaybg xorg-xwayland pango cairo gdk-pixbuf2 \
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

# The build's permission-fix step expects to be able to chown; running as root
# inside the privileged container keeps chroot/mount working. The script handles
# resulting root-owned build artifacts.
WORKDIR /raven

# Default to a no-op shell; pass a build command at `docker run` time, e.g.
#   ./scripts/build.sh all
CMD ["/bin/bash"]
