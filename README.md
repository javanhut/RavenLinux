# RavenLinux

A minimal Linux distribution built from scratch, meant to be rebuilt on.

```
  _____                         _      _
 |  __ \                       | |    (_)
 | |__) |__ ___   _____ _ __   | |     _ _ __  _   ___  __
 |  _  // _` \ \ / / _ \ '_ \  | |    | | '_ \| | | \ \/ /
 | | \ \ (_| |\ V /  __/ | | | | |____| | | | | |_| |>  <
 |_|  \_\__,_| \_/ \___|_| |_| |______|_|_| |_|\__,_/_/\_\
```

## Overview

RavenLinux is an independent, Linux-From-Scratch style distribution. This is the
base system and nothing more: a musl userland, a custom init, a shell, and a
bootable ISO. It is deliberately small so that everything above it can be
designed and built to taste.

What's here:

- **musl libc** base, cross-built from source with a purpose-built toolchain
- **Custom init** (`raven-init`) and service manager (`raven-rc`), written in Rust
- **Custom bootloader** (RavenBoot), a UEFI loader written in Rust, with GRUB as
  the BIOS fallback
- **uutils coreutils** (Rust) for the core userland
- **bash** and **fish** shells, **OpenSSH** client and server
- **A five-stage build** that runs in a container, so it works from any host

What's deliberately absent: a desktop environment, a package manager, a text
editor, language toolchains, an installer. Those are the things to build back.

## Building

The build runs inside a container so it works from macOS, Windows, or Linux. It
needs Docker or Podman, and the container runs `--privileged` because the build
uses chroot, overlayfs mounts, and loop devices.

```bash
make image      # build the toolchain image (cached after the first run)
make build      # build everything -> ./build/ and raven-<ver>-x86_64.iso
make shell      # drop into the build environment interactively
make help       # list every target
```

Common overrides:

```bash
make build JOBS=8          # 8 parallel compile jobs
make build ENGINE=podman   # force Podman instead of Docker
make iso ARCH=x86_64       # target architecture
```

To build on a Linux host directly, without the container:

```bash
./scripts/check-deps.sh    # check and install host build dependencies
./scripts/build.sh all
```

Note that RavenLinux targets x86_64 only. On an arm64 host (e.g. Apple Silicon)
the amd64 build image runs under emulation, and `rustc` crashes under
qemu-user — use Docker Desktop or colima with Rosetta, or build on x86_64.

## Build Stages

| Stage | Script | What it does |
|-------|--------|--------------|
| `stage0` | `scripts/stages/stage0-toolchain.sh` | musl cross-compilation toolchain (binutils, gcc, musl) |
| `stage1` | `scripts/stages/stage1-base.sh` | Base system cross-built with that toolchain; kernel and initramfs |
| `stage2` | `scripts/stages/stage2-native.sh` | Native rebuild of the sysroot: shells, system utilities, networking, PAM/NSS, libraries, locale and timezone data |
| `stage3` | `scripts/stages/stage3-packages.sh` | Base packages: core libraries (zlib, ncurses, readline, attr, acl), shells, OpenSSH, RavenBoot |
| `stage4` | `scripts/stages/stage4-iso.sh` | Squashfs root, RavenBoot/GRUB setup, EFI image, bootable ISO |

Run one stage at a time with `make stage2` or `./scripts/build.sh stage2`.

Artifacts land in `./build/`:

```
build/
├── toolchain/   # stage0 cross toolchain
├── sysroot/     # the RavenLinux root filesystem
├── packages/    # built binaries staged for the sysroot
├── sources/     # downloaded and cloned sources
├── iso/         # ISO workspace
└── logs/        # per-stage build logs
```

## Testing the ISO

```bash
# UEFI
qemu-system-x86_64 -cdrom raven-*.iso -m 2G \
  -nographic -serial mon:stdio \
  -bios /usr/share/edk2-ovmf/x64/OVMF_CODE.4m.fd -enable-kvm

# BIOS
qemu-system-x86_64 -cdrom raven-*.iso -m 2G -enable-kvm

# Write to USB
sudo dd if=raven-*.iso of=/dev/sdX bs=4M status=progress
```

The ISO boots to a root shell on tty1. Under UEFI it boots via RavenBoot; under
BIOS (or if RavenBoot didn't build) GRUB takes over, offering a serial-console
entry for headless VMs and a recovery entry.

You can also run the built rootfs as a container image, which is much faster
than booting a VM when you only want to poke at the userland:

```bash
make rootfs
docker run --rm -it --platform linux/amd64 ravenlinux
```

## Repository Layout

```
.
├── Makefile                  # containerized build entry point
├── Dockerfile                # the Linux build host image
├── scripts/
│   ├── build.sh              # build orchestration
│   ├── check-deps.sh         # host dependency check/install
│   ├── docker-build.sh       # container engine wrapper
│   ├── build-kernel.sh       # kernel build
│   ├── build-initramfs.sh    # initramfs build
│   ├── build-uutils.sh       # uutils coreutils build
│   ├── export-rootfs.sh      # package the sysroot as a container image
│   ├── lib/logging.sh        # shared logging
│   └── stages/               # the five build stages
├── init/                     # raven-init and raven-rc (Rust)
├── bootloader/               # RavenBoot, the UEFI bootloader (Rust)
├── packages/
│   ├── core/                 # musl, linux, openssl, openssh, sudo-rs, uutils
│   └── base/                 # bash, fish
├── configs/                  # shell, SSH, kernel, fontconfig configuration
├── etc/                      # files installed into the rootfs /etc
├── fonts/                    # console font (JetBrains Mono Nerd Font)
└── docs/                     # build and kernel notes
```

## Adding Things Back

The build is set up so that growing the distribution means adding, not
rewiring:

- **A package**: add a `package.toml` under `packages/` and a build function in
  `scripts/stages/stage3-packages.sh` (the existing `build_openssh` is a good
  template for an autotools package).
- **A new class of software** (a desktop, a toolchain, an editor suite): give it
  its own stage script under `scripts/stages/` and a case in `build.sh`, rather
  than growing stage 3 indefinitely.
- **Host build dependencies**: add them to both `scripts/check-deps.sh` and the
  `Dockerfile` — the two are kept deliberately parallel so they're easy to diff.
- **Boot behavior**: the live init is a heredoc inside
  `create_live_init()` in `scripts/stages/stage4-iso.sh`; the GRUB menu is in
  `setup_grub()` and RavenBoot staging in `setup_ravenboot()` in the same file.
  RavenBoot's own menu is compiled in — see `bootloader/src/`.

## License

See [LICENSE](LICENSE).
