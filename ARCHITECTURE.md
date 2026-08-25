# RavenLinux Architecture

## Overview

RavenLinux is an independent Linux distribution built from scratch. This
document describes the base system — the layer everything else gets built on
top of.

The base is deliberately narrow. It boots, gives you a shell, talks to the
network, and can be rebuilt from source in one command. On top of it sits the
Raven layer — the shell, package managers, version control, editor, task runner
and language RavenLinux provides for itself rather than inheriting.

What is still intentionally left out: a desktop, a graphical terminal, and
hosted Rust/Go toolchains.

## Design Principles

1. **Reproducible from source** — every stage is a script, not a state of mind
2. **Small enough to hold in your head** — no component you can't read in an afternoon
3. **Composable** — new software is a new stage or a new package, never a rewiring
4. **Host-independent** — the build runs in a container, so it works anywhere

## System Components

### Base System

- **Kernel**: Linux (LTS or latest stable), built from source with a Raven config
- **C Library**: musl libc
- **Bootloader**: RavenBoot (Rust, UEFI), with GRUB as the BIOS fallback
- **Init System**: `raven-init` (Rust) with `raven-rc` as service manager
- **Core Utilities**: uutils coreutils (Rust)
- **Shells**: ravenshell (default), bash, fish
- **Networking**: OpenSSH client and server
- **Privilege escalation**: sudo-rs

### The Raven Layer

The software RavenLinux provides for itself, built on top of the base system by
`scripts/stages/stage-raven.sh`:

| Binary | Source | Language | Role |
|--------|--------|----------|------|
| `ravenshell` | RavenShell | Go | Default login shell and scripting language |
| `rvn` | RavenPackageManager | Rust | Package manager |
| `poxy` | Poxy | Go | Universal package manager |
| `ivaldi` | Ivaldi | Rust | Version control |
| `crow` | CrowTextEditor | Rust | Text editor |
| `imlazy` | ImLazy | Go | Task runner |
| `oxigen` | OxigenLang | Rust | Interpreted language |
| `caw`, `cawd` | CAW | Rust | Wireless: nl80211, WPA and DHCP in-process |

All are statically linked — Go with `CGO_ENABLED=0`, Rust against
`x86_64-unknown-linux-musl` — so they add nothing to the sysroot's runtime link
graph and can be dropped in or left out freely.

`scripts/stages/stage-gui.sh` is separate precisely because it cannot hold that
line:

| Binary | Source | Language | Role |
|--------|--------|----------|------|
| `huginn` | RavenGUI | Rust | Wayland compositor (Smithay, udev/DRM backend) |
| `muninn` | RavenGUI | Rust | Desktop shell: panel, launcher, notifications |
| `muninn-lock` | RavenGUI | Rust | Session lock screen |

These link glibc and seventeen shared libraries — libdrm, libgbm, libinput,
libseat, libudev and the chain behind them — with Mesa's EGL/GLES drivers
dlopened at run time. The stage resolves that closure with `ldd` against the
binaries it just built and stages whatever stage2 did not already provide.

RavenTerminal is deliberately not part of this layer. It is a GPU-accelerated
terminal that needs OpenGL, GLFW and a display server, none of which exist in
the console base; it belongs with whatever graphical stack gets built back.

### Init System (`init/`)

`raven-init` runs as PID 1. It reads `/etc/raven/init.toml`, mounts the virtual
filesystems, and hands off to `raven-rc` for service supervision.

| File | Role |
|------|------|
| `init/src/main.rs` | PID 1 entry point, early boot |
| `init/src/config.rs` | `init.toml` parsing |
| `init/src/service.rs` | Service definition and lifecycle |
| `init/src/rc.rs` | Service manager (`raven-rc`) |

### Bootloader (`bootloader/`)

RavenBoot is a UEFI bootloader written in Rust, built for the
`x86_64-unknown-uefi` target. It ships as `EFI/BOOT/BOOTX64.EFI` on the ISO and
carries a compiled-in boot menu, so no config file is required on the ESP. GRUB
remains on the image as the BIOS path, and takes over UEFI too if RavenBoot
wasn't built.

### Boot Path

```
firmware
    │
    ├── UEFI ──▶ RavenBoot (EFI/BOOT/BOOTX64.EFI)
    │              └── falls back to GRUB when absent
    │
    └── BIOS ──▶ GRUB ── reads /boot/grub/grub.cfg
                   │
                   ▼
            vmlinuz + initramfs.img
                   │
                   ▼
    /init  ── initramfs init: reads root= from the kernel command line
                   │
       ┌───────────┴────────────┐
       │                        │
  no root=                  root=UUID=…
       │                        │
       ▼                        ▼
  live path                 disk path
  find the squashfs on      resolve the UUID, mount that
  the boot media, stack     partition on /mnt/root
  a tmpfs overlay on it
       │                        │
       ▼                        ▼
  switch_root /init         switch_root /sbin/init
  (the live boot script)    (raven-init)
       │                        │
       ▼                        ▼
  /etc/raven/raven-shell    raven-init mounts /etc/fstab,
  ravenshell > bash > sh    then starts services and a getty
       │                        │
       ▼                        ▼
  root shell on tty1        login prompt on tty1
```

The two paths share everything after `switch_root`; only the root filesystem
and the choice of PID 1 differ. `raven-install` is what turns one into the
other: it copies the squashfs onto a partition, repoints `/sbin/init` at
`raven-init`, removes the live boot script, and writes a `boot.cfg` carrying
`root=UUID=…`.

`raven.live` on the command line forces the live path even when a `root=` is
present.

### Installer (`scripts/installer/`)

`raven-install` installs the running live image onto a disk. It ships in the
sysroot at `/usr/sbin/raven-install`, so it is on the ISO and on every system
installed from it — installing from an already-installed machine onto a second
disk is the same code path, not a second one.

It partitions with `sfdisk` (GPT: ESP, optional swap, root), copies the
squashfs onto the root partition, writes `/etc/fstab`, creates the user
account, and installs RavenBoot to the ESP at both `\EFI\raven\` and the
removable-media fallback `\EFI\BOOT\BOOTX64.EFI`. The fallback path is what
makes an installed disk boot on firmware that will not accept an NVRAM entry
from us, which is most laptop firmware.

It is a shell script rather than a Rust binary on purpose: it shells out to
`sfdisk`, `mkfs` and `blkid` for everything that matters, and being a script
means it can be read and patched on the machine that is refusing to install.

The login shell is resolved by `/etc/raven/raven-shell`, which prefers
`ravenshell`, falls back to `bash`, and finally to `/bin/sh`. stage3 sets bash
as root's default so a base build without the Raven layer still boots to a
working shell; the Raven layer takes that over once `ravenshell` is installed.

## Directory Structure (target rootfs)

```
/
├── bin/          -> /usr/bin (symlink)
├── boot/         # Kernel, initramfs, bootloader
├── dev/          # Device files
├── etc/          # System configuration
│   └── raven/    # RavenLinux-specific configs (init.toml, first-boot-setup)
├── home/         # User home directories
├── lib/          -> /usr/lib (symlink)
├── lib64/        -> /usr/lib (symlink)
├── mnt/          # Mount points
├── opt/          # Optional/third-party software
├── proc/         # Process information
├── root/         # Root user home
├── run/          # Runtime data
├── sbin/         -> /usr/bin (symlink)
├── sys/          # Kernel/system information
├── tmp/          # Temporary files
├── usr/
│   ├── bin/      # All executables
│   ├── include/  # Header files
│   ├── lib/      # Libraries
│   ├── share/    # Architecture-independent data
│   └── src/      # Source code (optional)
└── var/          # Variable data
    ├── cache/
    ├── lib/
    └── log/      # System logs
```

## Build System

### Stages

| Stage | Purpose |
|-------|---------|
| **Stage 0** | Cross-compile the musl toolchain (binutils, gcc, musl) |
| **Stage 1** | Build the base system with the stage 0 toolchain; kernel and initramfs |
| **Stage 2** | Rebuild the sysroot natively: shells, system utilities, networking, PAM/NSS, libraries, locale and timezone data |
| **Stage 3** | Base packages: core libraries, shells, OpenSSH, RavenBoot |
| **Raven** | The Raven layer: ravenshell, rvn, poxy, ivaldi, crow, imlazy, oxigen, caw |
| **GUI** | Compositor and shell: huginn, muninn, muninn-lock, and the shared libraries they link |
| **Stage 4** | Squashfs root, RavenBoot/GRUB setup, EFI image, bootable ISO |

The Raven layer carries no stage number. Stages 0–4 are the base system and
must work without it; it slots between stage3 and stage4 because stage4 squashes
the sysroot into the ISO, so anything installed after it would not ship. It is
also fail-soft: a component that will not build is skipped with a warning rather
than failing the run.

Each stage is a standalone script under `scripts/stages/` that can be run on its
own; `scripts/build.sh` sequences them and owns the shared environment
(`RAVEN_ROOT`, `RAVEN_BUILD`, `SYSROOT_DIR`, and friends).

### Where State Lives

```
build/
├── toolchain/   # stage0 output
├── sysroot/     # the rootfs under construction (stages 1-3 write here)
├── packages/    # built binaries staged for install into the sysroot
├── sources/     # downloaded tarballs and cloned repos
├── iso/         # stage4 ISO workspace
└── logs/        # one log per stage run
```

Stage 2 resets the sysroot before it runs, so stages 2-4 are re-runnable
without a full rebuild from stage 0.

### Host Dependencies

`scripts/check-deps.sh` is the source of truth: it maps every required command
to a package name across Arch, Debian/Ubuntu, Fedora/RHEL, openSUSE, Void, and
Alpine, and can install the missing ones. The `Dockerfile` mirrors that list for
the Arch-based build image — keep the two in sync when adding a dependency.

## Package Format

Package definitions live under `packages/` as `package.toml`:

```toml
[package]
name = "example"
version = "1.0.0"
description = "Example package"
license = "MIT"
homepage = "https://example.com"

[build]
system = "meson"  # or "cmake", "make", "cargo", etc.
configure = []
build = []
install = []

[dependencies]
runtime = ["libc", "libfoo"]
build = ["meson", "ninja"]

[source]
url = "https://example.com/example-1.0.0.tar.gz"
sha256 = "..."
```

The definitions currently document what the base system contains; the build
functions in `scripts/stages/stage3-packages.sh` are what actually compile
them. Wiring a package manager to consume these definitions directly is one of
the natural next things to build.

Sets:

- `packages/core/` — musl, linux, openssl, openssh, libssh, sudo-rs, uutils-coreutils
- `packages/base/` — bash, fish
- `packages/raven/` — ravenshell, rvn, poxy, ivaldi, crow, imlazy, oxigen, caw
- `packages/gui/` — ravengui: huginn, muninn, muninn-lock

## Extending the System

The base is designed to be grown, not modified:

- **A package** → a `package.toml` under `packages/`, plus a build function in
  stage 3
- **A Raven tool** → one row in the `RAVEN_COMPONENTS` table in
  `scripts/stages/stage-raven.sh`, plus a `package.toml` under `packages/raven/`
- **A new software class** (desktop, toolchain, editor suite) → its own stage
  script and a case in `build.sh`, rather than an ever-growing stage 3
- **A host build dependency** → `check-deps.sh` *and* the `Dockerfile`
- **Boot behavior** → `create_live_init()`, `setup_ravenboot()`, and
  `setup_grub()` in `scripts/stages/stage4-iso.sh`
- **How a disk boots** → the init script generated by
  `scripts/build-initramfs.sh` (`raven_root_from_cmdline`,
  `raven_mount_disk_root`)
- **Installation** → `scripts/installer/raven-install`
- **The boot menu** → `bootloader/src/` (RavenBoot's menu is compiled in)
- **Service management** → `init/src/service.rs` and `init/src/rc.rs`
