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
# This image carries the build dependencies of every layer the ISO ships, the
# desktop included: the compositor's C libraries and the GTK4 toolkit its
# applications are written against. It said the opposite for a long time -- "no
# graphical stack is built, so no GUI toolkits are installed here" -- and the
# stages took it at its word, skipping six applications per build.
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
        # Console font. kbd supplies setfont, which stage2 copies into the
        # sysroot; python-freetype-py is what stage4 rasterises the shipped
        # JetBrains Mono Nerd Font TTF into PSF with. Without the latter the
        # build still succeeds and the console falls back to the kernel font.
        kbd python-freetype-py \
        # Device firmware. The kernel builds iwlwifi, ath9k/10k/11k/12k,
        # rtw88/89, mt7921 and brcmfmac in (=y), but a wireless driver without
        # its blob just fails to probe and the interface never appears -- which
        # looks exactly like a missing driver. stage2's copy_firmware() takes
        # these from the build host, and without this package that host is this
        # container, which had no /lib/firmware at all.
        linux-firmware \
        # NTFS, for the installer. raven-install --alongside shrinks the
        # partition the other OS lives on to make room, and on any machine that
        # ships with Windows that partition is NTFS -- ntfsresize is the whole
        # of how that is done, and without it "install alongside Windows" is a
        # button that cannot work.
        #
        # Both packages, and that is not belt and braces: ntfs-3g is now only
        # the FUSE driver -- `pacman -Si ntfs-3g` lists ntfsprogs as an
        # *optional* dependency -- and ntfsresize, ntfsfix and mkfs.ntfs all
        # live in ntfsprogs. Installing ntfs-3g alone gets a container that can
        # mount NTFS and cannot resize it, which is the wrong half. The driver
        # is worth having too: it is what lets the installed system read the
        # Windows partition it just made room next to.
        ntfs-3g ntfsprogs \
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
        libdrm libinput mesa libxkbcommon wayland alsa-lib \
        # libinput classifies devices through libwacom, which pulls in lua.
        # Named explicitly because the closure is not obvious from the above.
        libwacom libevdev mtdev \
        # seatd, not libseat: on Arch there is no libseat package -- the seatd
        # package owns both the daemon and /usr/lib/libseat.so.
        seatd \
        # The X server huginn drives for X11 clients. stage-gui.sh copies this
        # binary and its libraries into the sysroot; without it in the image the
        # stage warns and moves on, and the ISO ships with no X11 support at all.
        xorg-xwayland \
        # Full live-desktop audio, Bluetooth, and authorization runtime.
        # ell is only an optdepend of bluez-utils, but its btpclient links
        # against libell and stage-desktop-runtime.py refuses any binary with
        # an unresolved library rather than shipping something that cannot run.
        pipewire pipewire-audio pipewire-pulse wireplumber bluez bluez-utils ell \
        alsa-ucm-conf alsa-topology-conf sof-firmware polkit \
        # The GTK4 stack -- SIX of the image's applications, and every one of
        # its graphical *applications* as opposed to its shell, is a GTK4 +
        # libadwaita client: Files, Settings, Store, Power, Controls and the
        # graphical installer. Each of their stage_* functions in stage-gui.sh
        # opens with the same `pkg-config --exists gtk4 libadwaita-1 glib-2.0
        # gio-2.0` guard and, per this stage's fail-soft rule, warns and
        # returns 0 when it does not hold.
        #
        # This container had none of them, and the result was not a warning
        # anyone saw: install_desktop_entries writes an entry only for a binary
        # it can see, so all six were skipped together and the ISO booted to a
        # launcher holding exactly two things -- Terminal, and Crow inside it.
        # A desktop with no file manager, no settings and no installer looks
        # like a compositor bug rather than six absent build dependencies.
        #
        # glib2-devel is not what satisfies the guard -- glib-2.0.pc,
        # gio-2.0.pc and glib-compile-schemas are all owned by glib2, which
        # arrives as a dependency of gtk4. It is here for the development
        # tooling around them (gdbus-codegen, glib-mkenums) that a -sys crate
        # may shell out to, and because stage_gtk_runtime names it. Verified
        # with pacman -Qo rather than assumed: an earlier version of this
        # comment had the .pc files in the wrong package.
        gtk4 libadwaita glib2-devel \
        # The other half of a GTK application: the data it reads at run time,
        # which stage_gtk_runtime() copies out of this container and into the
        # sysroot. None of it is linked, so none of it is found by ldd, and
        # each piece fails separately and quietly on the image:
        #
        #   gsettings-desktop-schemas  org.gnome.desktop.interface. Without it
        #                              every g_settings_new() is a fatal GLib
        #                              error and the applications abort on
        #                              startup, having built perfectly.
        #   shared-mime-info           update-mime-database and the source XML.
        #                              Without it every file is
        #                              application/octet-stream.
        #   desktop-file-utils         update-desktop-database, which is what
        #                              makes the MIME half of the launcher's
        #                              entries resolve.
        #   glycin + glycin-gtk4       image decoding. Since gdk-pixbuf 2.44
        #                              the loaders are out-of-process: `glycin`
        #                              owns /usr/lib/glycin-loaders, which is
        #                              the directory stage_gtk_runtime() looks
        #                              for, and glycin-gtk4 is the toolkit's
        #                              side of it. gtk4 depends on neither.
        #   bubblewrap                 the sandbox glycin refuses to decode
        #                              outside of, so GTK draws no image at all
        #                              without it.
        #   librsvg                    the SVG loader. The application icons
        #                              staged into hicolor are SVG.
        #   dconf                      the GSettings backend. Without it
        #                              settings apply for the life of the
        #                              process and are forgotten on exit.
        gsettings-desktop-schemas shared-mime-info desktop-file-utils \
        glycin glycin-gtk4 bubblewrap librsvg dconf \
        # The terminal needs no packages of its own. stage-gui.sh builds
        # RavenTerminal from source with `go build -tags wayland`, whose GLFW
        # compiles from vendored C against wayland-client/cursor/egl and
        # xkbcommon, and whose go-gl bindings run `pkg-config --cflags -- gl`
        # for libGL. Every one of those is already here: `wayland`,
        # `libxkbcommon`, and `mesa` (which brings libglvnd, and with it gl.pc).
        # Its xdg-shell and viewporter protocol sources are pre-generated in
        # tree, so no wayland-scanner or wayland-protocols is needed either.
        #
        # Written out rather than left implicit because "the terminal has no
        # build deps of its own" is surprising, and because the next person to
        # trim this list needs to know that dropping `wayland`, `libxkbcommon`
        # or `mesa` costs the terminal as well as the compositor.
        #
        # Cursor theme. huginn loads the default pointer through
        # xcursor::CursorTheme::load(XCURSOR_THEME or "default"), and
        # /usr/share/icons/default/index.theme says `Inherits=Adwaita`. With no
        # Adwaita on the host that lookup returns None and there is no visible
        # pointer anywhere over the compositor's own surfaces -- the dock, the
        # launcher, the background. This package is what that inherit resolves
        # to. It is the cursors alone; the icon theme below is a separate one.
        adwaita-cursors \
        # Application icons for the dock and launcher. hicolor is the spec's
        # base theme and carries only what applications install for themselves,
        # so generic Icon= names resolve to nothing against it -- RavenGUI's
        # theme.rs measured 10 of 36 applications with no icon under hicolor
        # and 1 under breeze-dark, which is why ICON_THEME names the latter.
        # hicolor-icon-theme ships the index.theme every other theme falls back
        # through, so it is not optional even when it holds no icons itself.
        breeze-icons hicolor-icon-theme \
        # Fonts. stage4 rasterises the shipped JetBrains Mono Nerd Font into a
        # console PSF and copies the TTFs, and that was the *only* font on the
        # image: the shell drew every label in a monospace face, and anything
        # outside Latin/Greek/Cyrillic drew blank. DejaVu adds the proportional
        # sans and serif families plus far wider coverage, and the emoji font
        # is what stops an emoji in a window title from being a blank box.
        # stage2's copy_system_utils() copies /usr/share/fonts wholesale, so
        # installing them here is what puts them on the image.
        #
        # CJK is deliberately absent: noto-fonts-cjk is ~120MB against DejaVu's
        # 7, and huginn has no way to type it either -- there is no
        # text-input-v3 yet, so an IME cannot attach. Add it here when that
        # changes; the font stack needs no code change to pick it up.
        ttf-dejavu noto-fonts-emoji \
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
