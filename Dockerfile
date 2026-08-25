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
        # Partitioning and UEFI tooling for raven-install. util-linux above
        # already carries sfdisk/wipefs/partx/blockdev/losetup/findmnt/mkswap;
        # these three are the ones it does not, and stage2 copies each of them
        # into the sysroot, so a host without them ships an ISO that cannot
        # install itself.
        parted gptfdisk efibootmgr \
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
        # Device firmware. The kernel builds iwlwifi, ath9k/10k/11k/12k,
        # rtw88/89, mt7921 and brcmfmac in (=y), but a wireless driver without
        # its blob just fails to probe and the interface never appears -- which
        # looks exactly like a missing driver. stage2's copy_firmware() takes
        # these from the build host, and without this package that host is this
        # container, which had no /lib/firmware at all.
        linux-firmware \
        # regulatory.db + its signature. The kernel is built with
        # CFG80211_REQUIRE_SIGNED_REGDB, so without these cfg80211 falls back to
        # the built-in world domain: the card associates, but loses channels and
        # transmit power. Ships separately from linux-firmware on Arch.
        wireless-regdb \
        # Go toolchain -- the Raven stage builds ravenshell, poxy and imlazy
        # with CGO_ENABLED=0, so no Go cgo headers are needed.
        go \
        # Compositor stack -- the GUI stage builds huginn against these. Unlike
        # every Raven-layer component, huginn links C libraries: smithay binds
        # libdrm/libgbm/libinput/libseat/libudev, and Mesa provides EGL and the
        # DRI drivers it dlopens. Without them stage-gui.sh skips itself and
        # the ISO ships console-only.
        #
        # These are also the libraries stage-gui.sh copies into the sysroot, so
        # this list is what the shipped system ends up carrying.
        libdrm libinput mesa libxkbcommon wayland \
        # libinput classifies devices through libwacom, which pulls in lua.
        # Named explicitly because the closure is not obvious from the above.
        libwacom libevdev mtdev \
        # seatd, not libseat: on Arch there is no libseat package -- the seatd
        # package owns both the daemon and /usr/lib/libseat.so.
        seatd \
    && pacman -Scc --noconfirm

# -----------------------------------------------------------------------------
# Rust via rustup
# -----------------------------------------------------------------------------
# The build needs cargo AND the rust-src component (for the musl cross target).
# rustup gives both reliably, independent of Arch's rust packaging.
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
# Two extra targets are pre-installed so the build works offline and does not
# pay for a target download mid-build:
#   x86_64-unknown-uefi        RavenBoot (stage3)
#   x86_64-unknown-linux-musl  the Raven toolchain's static Rust binaries
RUN pacman -Syu --noconfirm --needed rustup \
    && rustup default stable \
    && rustup component add rust-src \
    && rustup target add x86_64-unknown-uefi \
    && rustup target add x86_64-unknown-linux-musl \
    && pacman -Scc --noconfirm

WORKDIR /raven

CMD ["/bin/bash"]
