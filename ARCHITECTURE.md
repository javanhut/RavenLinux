# RavenLinux Architecture

## Overview

RavenLinux is an independent Linux distribution built from scratch. This
document describes the base system — the layer everything else gets built on
top of.

The base is deliberately narrow. It boots, gives you a shell, talks to the
network, and can be rebuilt from source in one command. Everything beyond that
(package management, a desktop, toolchains, an installer) is intentionally left
out so it can be designed rather than inherited.

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
- **Shells**: bash (default), fish
- **Networking**: OpenSSH client and server
- **Privilege escalation**: sudo-rs

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
    /init  ── live init: mounts the squashfs root, then execs a shell on tty1
                   │
                   ▼
            bash (login shell, root)
```

On an installed system the handoff is to `raven-init` instead of the live init
script; the live path exists so the ISO is useful without an installer.

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
| **Stage 4** | Squashfs root, RavenBoot/GRUB setup, EFI image, bootable ISO |

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

## Extending the System

The base is designed to be grown, not modified:

- **A package** → a `package.toml` under `packages/`, plus a build function in
  stage 3
- **A new software class** (desktop, toolchain, editor suite) → its own stage
  script and a case in `build.sh`, rather than an ever-growing stage 3
- **A host build dependency** → `check-deps.sh` *and* the `Dockerfile`
- **Boot behavior** → `create_live_init()`, `setup_ravenboot()`, and
  `setup_grub()` in `scripts/stages/stage4-iso.sh`
- **The boot menu** → `bootloader/src/` (RavenBoot's menu is compiled in)
- **Service management** → `init/src/service.rs` and `init/src/rc.rs`
