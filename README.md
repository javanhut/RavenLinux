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

On top of that base sits the **Raven layer** — the software RavenLinux provides
for itself rather than inheriting:

| Tool | What it is |
|------|------------|
| `ravenshell` | [Raven Shell](https://github.com/javanhut/RavenShell) — the default login shell and scripting language |
| `rvn` | [Raven Package Manager](https://github.com/javanhut/RavenPackageManager) |
| `poxy` | [Poxy](https://github.com/javanhut/Poxy), a universal package manager |
| `ivaldi` | [Ivaldi](https://github.com/javanhut/Ivaldi), the version control system |
| `crow` | [Crow](https://github.com/javanhut/CrowTextEditor), the text editor |
| `imlazy` | [ImLazy](https://github.com/javanhut/ImLazy), the task runner |
| `oxigen` | [OxigenLang](https://github.com/javanhut/OxigenLang), the interpreted language |
| `caw`, `cawd` | [CAW](https://github.com/javanhut/CAW), the wireless stack — netlink and WPA in-process, no supplicant |

Every one of those is built as a **static** binary — Go with `CGO_ENABLED=0`,
Rust against `x86_64-unknown-linux-musl` — so the Raven layer adds no runtime
link dependency to the base sysroot.

And a graphical layer, built separately because it cannot be static:

| Tool | What it is |
|------|------------|
| `huginn` | [RavenGUI](https://github.com/javanhut/RavenGUI)'s Wayland compositor, which also draws the desktop — dock, launcher, notifications |
| `muninn-lock` | the session lock screen, a separate process so a compositor bug cannot unlock the screen |
| `raven-terminal` | [RavenTerminal](https://github.com/javanhut/RavenTerminal), the terminal the desktop opens — on the dock and on `Super`+`Shift`+`T` |
| `ravenfilemanager` | [RavenFileManager](https://github.com/javanhut/RavenFileManager), the file manager — the other icon on the dock, and the image's only GTK client |
| `ravencanvasd`, `ravencanvas` | [RavenCanvas](https://github.com/javanhut/RavenCanvas), the wallpaper — a layer-shell client, started by the session script before the compositor it draws behind |
| `ravend`, `raven-greeter` | [RavenLogin](https://github.com/javanhut/RavenLogin), the login screen — and the root daemon behind it, which is not the process that draws |

Boot the `Raven Desktop (Huginn)` entry, or add `raven.graphics=wayland` to the
kernel cmdline, and raven-init starts the session instead of a getty — or, if
`ravend` is on the image, the password prompt in front of it.

Only the first three are load-bearing. Without huginn there is nothing to log
into, and without `raven-terminal` nothing to launch — huginn names it in two
compiled-in places — so both failing fails the stage. The rest are things a
desktop can be missing: `FILEMANAGER_SKIP=1`, `CANVAS_SKIP=1` and `LOGIN_SKIP=1`
each produce an image that still boots to a working session.

RavenCanvas is the one of those that is a separate process by choice rather than
necessity. huginn draws its own dock, launcher and notifications inside its
render loop, because anything that must feel instant and must never fail does
not get to be a process that can die — and a wallpaper is exactly the case that
rule is not about, since huginn paints its own background underneath and the
worst its death can do is leave a plain desktop.

RavenFileManager is the first thing on the image that is simply an application,
and the only GTK client on it. That is most of what makes it interesting to
build: it links a hundred and thirty-four shared libraries against huginn's
seventeen, and four of the things it needs are ones `ldd` cannot see — a
compiled `gschemas.compiled`, without which every GTK application aborts at
startup on a schema lookup; `gsettings-desktop-schemas`, without which
libadwaita silently stays light; `mime.cache`, without which every file is
`application/octet-stream`; and `bwrap`, without which the toolkit displays no
images at all, because gdk-pixbuf's loaders moved into glycin and it decodes
inside a sandbox or not at all. The stage rebuilds those caches against the
sysroot rather than copying the build host's, which encode host paths.

RavenLogin is the password prompt, and the split between its two binaries is the
point of it. `ravend` runs as root and reads `/etc/shadow`; `raven-greeter` runs
as its own unprivileged account, draws, and can ask `ravend` exactly one question
over a `0600` socket that `ravend` checks `SO_PEERCRED` on anyway. Drawing a
login screen means parsing fonts, rasterizing glyphs and decoding images — close
enough to a list of everything that has ever been remotely exploitable in a
login screen — and the password hashes are not in that process. Nothing on the
image records the choice to use it: `raven-init` looks for the `ravend` binary
and starts it in place of the autologin session when it finds one, so installing
it is the whole of turning the prompt on and `LOGIN_SKIP=1` is the whole of
leaving it off.

RavenTerminal is built by the same stage as the compositor, from its own
repository, with the **Wayland** GLFW backend rather than X11 — its Makefile
picks a backend from `$XDG_SESSION_TYPE`, which is never set in a container, so
the stage passes `-tags wayland` explicitly instead of getting an XWayland
dependency by accident. It renders through OpenGL 4.1 and carries its own Nerd
Fonts inside the binary, so it needs nothing from `/usr/share/fonts`.

It takes `-e` to run something other than a shell, which is what lets a terminal
program have a launcher entry at all: `crow` is in the menu as
`Exec=raven-terminal -e crow %F`. Only the first tab runs the command; tabs
opened afterwards are ordinary shells.

The entry has to name the terminal explicitly rather than set `Terminal=true`,
because huginn parses that key and then ignores it — `Entry::argv` goes straight
to `Command::spawn` either way, so such an entry would exec a bare TUI with no
controlling terminal, open no window, and report no error.

`caw`, `rvn`, `ivaldi`, `imlazy` and `poxy` still have no entries, and `-e` does
not change that: each is a subcommand CLI that prints usage and exits when run
bare, so a menu entry would be a window that flashes and closes. They are run by
typing their names, which is what they were built for.

What's deliberately absent: hosted Rust/Go toolchains. That's the thing still to
build back.

## Building

The build runs inside a container so it works from macOS, Windows, or Linux. It
needs Docker or Podman, and the container runs `--privileged` because the build
uses chroot, overlayfs mounts, and loop devices.

RavenLinux uses [ImLazy](https://github.com/javanhut/ImLazy) as its build
interface. `lazy.toml` is the canonical build graph; the small Makefile only
forwards old commands to ImLazy.

```bash
imlazy image    # build the toolchain image (cached after the first run)
imlazy build    # build everything -> ./build/ and raven-<ver>-x86_64.iso
imlazy raven    # build just the Raven layer into an existing sysroot
imlazy gui      # build just the compositor and shell
imlazy shell    # drop into the build environment interactively
imlazy list     # list every command with a description
imlazy -n qemu  # dry run: print what a command would execute
```

**On a host where Podman runs rootless**, add `RAVEN_NO_DEVNODES=1`. The
initramfs builder calls `mknod`, which a user namespace refuses whatever the
capability set says, and stage1 dies with `mknod: Operation not permitted`:

```bash
RAVEN_NO_DEVNODES=1 imlazy build
```

That is safe here because the kernel config sets `CONFIG_DEVTMPFS_MOUNT=y`, so
`/dev` is populated before init runs -- but the resulting ISO is not identical
to a rootful build, so rule that difference out first if it misbehaves at boot.

Common overrides:

```bash
imlazy build jobs=8          # 8 parallel compile jobs
imlazy build engine=podman   # force Podman instead of Docker
imlazy iso arch=x86_64       # target architecture
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
| `raven` | `scripts/stages/stage-raven.sh` | The Raven layer: ravenshell, rvn, poxy, ivaldi, crow, imlazy, oxigen, caw |
| `gui` | `scripts/stages/stage-gui.sh` | The desktop: huginn, muninn-lock, raven-terminal, ravenfilemanager, ravencanvasd, ravend, the application menu, and the shared libraries, GTK runtime, icon themes and cursor theme they need |
| `stage4` | `scripts/stages/stage4-iso.sh` | Squashfs root, RavenBoot/GRUB setup, EFI image, bootable ISO |

The Raven layer is unnumbered on purpose. Stages 0–4 build a base system that
has to stand on its own; the Raven layer sits on top of stage3 and must run
before stage4, because stage4 squashes the sysroot into the ISO and anything
installed afterwards would not ship.

Run one stage at a time with `imlazy stage2` or `./scripts/build.sh stage2`.
`imlazy initramfs` rebuilds only the initramfs, which stage1 would otherwise
rebuild behind the kernel — useful because its init script, the thing that
decides between the live squashfs and a `root=` on disk, changes far more often
than the kernel does.

The Raven layer is fail-soft: a component that will not clone or compile is
logged and skipped, and the ISO still builds. Narrow it while iterating:

```bash
./scripts/build.sh raven                  # all eight components
RAVEN_ONLY=crow,ivaldi imlazy raven         # just these two
RAVEN_ONLY=caw imlazy raven                 # caw ships two binaries, caw and cawd
RAVEN_SKIP=oxigen imlazy raven              # everything but this one
RAVEN_OFFLINE=1 imlazy raven                # reuse existing clones, no network
RAVEN_IVALDI_REF=v0.1.2 imlazy raven        # pin one component to a git ref
RAVEN_KEEP_BASH_DEFAULT=1 imlazy raven      # install ravenshell, keep bash default
RAVEN_PACMAN_FROM_HOST=1 imlazy raven       # give rvn the host's pacman.conf
```

The GUI stage is fail-soft the same way, and skips itself when the build host
lacks the libraries huginn links. The terminal is fail-soft *within* it, so a
build host that can compile the compositor but not the terminal still produces a
desktop — one that cannot launch anything, which the stage summary says outright
rather than leaving to be discovered at boot:

```bash
./scripts/build.sh gui                    # huginn, muninn-lock, raven-terminal
GUI_SKIP=1 imlazy build                     # console-only ISO
GUI_REF=v0.1.0 imlazy gui                   # pin RavenGUI to a git ref
GUI_OFFLINE=1 imlazy gui                    # reuse the existing clones
TERMINAL_SKIP=1 imlazy gui                  # compositor only, no terminal
TERMINAL_REF=v0.2.0 imlazy gui              # pin RavenTerminal to a git ref
FILEMANAGER_SKIP=1 imlazy gui               # no file manager; the desktop works
LOGIN_SKIP=1 imlazy gui                     # no password prompt; autologin
CANVAS_SKIP=1 imlazy gui                    # no wallpaper daemon; flat background
CANVAS_REF=v0.1.0 imlazy gui                # pin RavenCanvas to a git ref
```

Each of those four also takes an `_OFFLINE=1`, which reuses that component's
existing clone without falling back to the network.

The stage ends with a **Desktop:** summary reporting the terminal, the file
manager, the wallpaper, the application menu, the cursor theme, the icon theme
and the fonts. Each of those was missing at once at one point, none of them
failed the build, and the result was a green ISO whose desktop booted to a dock
with nothing on it but the launcher, a launcher enumerating nothing, no mouse
pointer and a single monospace font.

A fresh image ships an empty `/usr/share/wallpaper/set`, so the desktop draws
RavenCanvas's built-in gradient until somebody sets a picture — which the
summary says out loud, because a gradient otherwise looks like a wallpaper that
failed:

```bash
sudo raven-set-wallpaper /path/to/image   # the machine: desktop and login screen
ravencanvas set scene aurora --persist    # this user only
```

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

`scripts/test-qemu.sh` boots the most recent ISO it can find. It picks up KVM
when `/dev/kvm` is available and falls back to software emulation with a warning
when it is not -- that fallback boots in minutes rather than seconds, which is
worth knowing before a slow boot reads as a hang.

```bash
imlazy qemu           # serial console
imlazy qemu-uefi      # boot through OVMF firmware
imlazy qemu-desktop   # with a virtio GPU, for the Wayland session
imlazy smoke          # boot for 180s unattended, then exit
```

Quit QEMU with **Ctrl-a x**.

Pick **`Raven Linux (Serial)`** in the boot menu to watch the kernel over the
serial line. The default entry logs to `tty0`, so on `-nographic` the output
stops after the bootloader banner and looks like a hang.

For the Wayland session, pick **`Raven Desktop (Huginn)`** (under
`Raven Linux (Graphical) >` on the RavenBoot/UEFI path). `qemu-desktop` passes
`virtio-vga-gl`, because Huginn's udev backend has to take DRM master on a real
device and QEMU's default emulated VGA provides none. A display backend is a
separate package on most distributions:

```bash
sudo pacman -S qemu-ui-gtk qemu-ui-opengl   # Arch
sudo apt install qemu-system-gui            # Debian
```

`./scripts/check-deps.sh` lists these under "Optional (testing only)". Writing
to a USB stick is unchanged:

```bash
sudo dd if=raven-*.iso of=/dev/sdX bs=4M status=progress
```

The ISO boots to a root shell on tty1, and is hybrid: BIOS boots through GRUB,
UEFI through RavenBoot. `cawd` is supervised by raven-init, so `caw scan` and
`caw connect <ssid>` work from that shell -- though saved profiles live on a
tmpfs overlay and do not survive a reboot of a live image.

## Installing

The live image installs itself. Boot it, and at the shell:

```bash
raven-install
```

It asks for a disk, a hostname, a user account and password, a timezone and a
locale, prints what it is about to do, and waits for you to type `YES`. Then it
partitions, copies the system across, and installs the bootloader.

```bash
raven-install --dry-run              # every check and the full plan, no writes
raven-install --disk /dev/nvme0n1    # skip the disk question
raven-install --swap none            # no swap partition
raven-install --efi-nvram            # also register a UEFI NVRAM boot entry
raven-install --profile minimal      # smallest install; add packages later
raven-install --profile desktop      # record the desktop package template
```

The layout is GPT, and **the target disk is erased completely** — there is no
dual-boot mode yet, and no manual partitioning:

The base installation stays small. After the installed system has networking,
run `sudo raven-postinstall` to preview and apply the selected package profile,
or `sudo raven-postinstall --profile developer`. Profiles are editable package
lists in `/etc/raven/install-profiles`, following archinstall's separation of
disk installation from a reusable system profile.

For a desktop, `sudo raven-desktopinstall` is the better command. It installs
the same packages in named sets — `session`, `toolkit`, `media`, `fonts`,
`portals`, `audio`, `firmware` — one `rvn` call each rather than one call for
everything, so a set whose names have gone stale upstream costs you that set
and is reported by name instead of failing the whole install.

More importantly, it does the half that installing packages does not cover.
Four toolkit caches are derived state that no package on this image rebuilds:

| Cache | What its absence looks like |
|---|---|
| `gschemas.compiled` | **every GTK application aborts on startup** — GSettings reads only the compiled cache, never the XML |
| `mime.cache` | every file reads as `application/octet-stream`: one icon, no "Open With" |
| `mimeinfo.cache` | "Open Containing Folder" resolves to nothing |
| `icon-theme.cache` | stale icons, or slow theme lookups |

They are missing on a freshly staged image, because the ISO stages those trees
as plain files copied off a build host — and they go stale again on a package
upgrade, so this is worth running on a machine that has been up for months and
not only after an install.

```
raven-desktopinstall                 # every set but devel and extras
raven-desktopinstall --list          # what each set contains
raven-desktopinstall -n toolkit      # dry run, one set, no root needed
raven-desktopinstall --caches-only   # rebuild the caches, install nothing
```

| Partition | Size | Filesystem | Mount |
|-----------|------|------------|-------|
| 1 | 512M | FAT32 | `/boot/efi` |
| 2 | RAM-sized, optional | swap | — |
| 3 | the rest | ext4 | `/` |

RavenBoot is installed to the ESP twice: at `\EFI\raven\raven-boot.efi`, and
at `\EFI\BOOT\BOOTX64.EFI`. The second is the removable-media fallback path,
and it is what makes the disk boot on firmware that will not take an NVRAM entry
from us — which is most laptop firmware. `--efi-nvram` adds a named entry on top
of that, and needs `efibootmgr` in the image.

Two firmware settings decide whether this works at all:

- **Secure Boot must be off.** RavenBoot is unsigned; with Secure Boot on the
  firmware refuses to load it and you get no error, just the next boot entry.
  The installer checks and warns.
- **RAID/Intel RST storage mode is supported** — the kernel carries VMD, so
  the NVMe drive is visible either way. If the installer reports no disks on
  an older image, switching the controller to AHCI mode is the workaround.

On an ASUS ROG machine Secure Boot is under F2 → F7 (Advanced Mode) → Security.

The installed system boots to a login prompt on tty1 rather than the live
image's root shell. If something goes wrong, the RavenBoot menu carries a
**RavenLinux (rescue shell)** entry that boots the installed root with
`init=/bin/bash` and nothing else running.

## Fonts

**JetBrains Mono Nerd Font Mono** is the identity typeface, shipped in this
repository and turned by the build into the two formats a Linux system actually
needs. Two more families come from the build host, because a desktop cannot be
drawn in a monospace face alone:

| Where | Format | Built by |
|-------|--------|----------|
| tty1, the console | JetBrains Mono as a PSF2 bitmap at 8x16, 10x20, 12x24 and 16x32 | `scripts/make-console-font.py`, run by stage4 |
| the Wayland session | JetBrains Mono's `.ttf`, through fontconfig | copied to `/usr/share/fonts` by stage4 |
| the shell's own text | **DejaVu Sans/Serif** — the proportional faces the dock, launcher and notifications draw in | `ttf-dejavu` on the build host, copied by stage2 |
| emoji, anywhere | **Noto Color Emoji** | `noto-fonts-emoji` on the build host, copied by stage2 |

Only the first is vendored. The other two are host packages named in the
`Dockerfile`, and stage2's `copy_system_utils()` copies `/usr/share/fonts`
wholesale, so installing them there is what puts them on the image. stage4 warns
if no proportional face made it: huginn draws its labels through cosmic-text,
which takes whatever it finds, so a monospace-only image produces a desktop
where every label is monospace and anything outside Latin, Greek and Cyrillic
draws as nothing at all.

CJK is deliberately absent — `noto-fonts-cjk` is around 120MB against DejaVu's
7, and huginn has no way to type it either, since there is no `text-input-v3`
for an input method to attach to. Add the package to the `Dockerfile` when that
changes; nothing in the font handling needs a code change to pick it up.

The virtual terminal cannot use a TTF, so stage4 rasterises the same typeface
into `/usr/share/kbd/consolefonts/` and `raven-console-font` loads one at boot,
choosing the cell size from the framebuffer width — 16x32 on a 2560px panel down
to 8x16 on a VGA console. Without it the console falls back to the kernel's
built-in 8x16 VGA font: no Nerd Font glyphs, box drawing that does not join up,
and text about two millimetres tall on a laptop screen.

Override it on the kernel command line, or permanently:

```bash
raven.font=12x24                              # a specific cell size
raven.font=none                               # keep the kernel's own font
echo 'FONT=12x24' > /etc/raven/console-font.conf
setfont /usr/share/kbd/consolefonts/raven-12x24.psfu   # right now, no reboot
```

The console holds 512 glyphs, so the PSF carries a curated subset — ASCII,
Latin-1, all of box drawing and block elements, and around sixty Nerd Font
icons. `GLYPH_SET` in `scripts/make-console-font.py` is the list; see
[fonts/README.md](fonts/README.md) for the details and for replacing the family.

Generating the console font needs `python-freetype-py` on the build host. The
build is fail-soft without it and produces a working ISO on the kernel font.

Host-side tests, which need no ISO and no container:

```bash
imlazy test
```

You can also run the built rootfs as a container image, which is much faster
than booting a VM when you only want to poke at the userland:

```bash
imlazy rootfs
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
│   ├── installer/            # raven-install, the disk installer and wizard
│   ├── make-console-font.py  # rasterises the shipped TTF into a PSF console font
│   └── stages/               # the five build stages, plus stage-raven.sh
├── init/                     # raven-init and raven-rc (Rust)
├── bootloader/               # RavenBoot, the UEFI bootloader (Rust)
├── packages/
│   ├── core/                 # musl, linux, openssl, openssh, sudo-rs, uutils
│   ├── base/                 # bash, fish
│   ├── raven/                # ravenshell, rvn, poxy, ivaldi, crow, imlazy, oxigen, caw
│   └── gui/                  # ravengui: huginn, muninn, muninn-lock
├── configs/                  # shell, SSH, kernel, fontconfig configuration
├── etc/                      # files installed into the rootfs /etc
├── fonts/                    # JetBrains Mono Nerd Font (console and desktop)
└── docs/                     # build and kernel notes
```

## Adding Things Back

The build is set up so that growing the distribution means adding, not
rewiring:

- **A package**: add a `package.toml` under `packages/` and a build function in
  `scripts/stages/stage3-packages.sh` (the existing `build_openssh` is a good
  template for an autotools package).
- **A Raven tool**: add one row to the `RAVEN_COMPONENTS` table at the top of
  `scripts/stages/stage-raven.sh` and a `package.toml` under `packages/raven/`.
  The table row carries everything the build needs — repository, language,
  binary name, and build target — so no new build function is required.
- **A new class of software** (a desktop, a toolchain, an editor suite): give it
  its own stage script under `scripts/stages/` and a case in `build.sh`, rather
  than growing stage 3 indefinitely.
- **Host build dependencies**: add them to both `scripts/check-deps.sh` and the
  `Dockerfile` — the two are kept deliberately parallel so they're easy to diff.
- **Boot behavior**: both live and installed roots run `raven-init`; stage4
  installs the small `/init` hand-off. The GRUB menu is in `setup_grub()` and
  RavenBoot staging in `setup_ravenboot()` in the same file. RavenBoot's own
  menu is compiled in — see `bootloader/src/`.
- **How an installed disk boots**: the initramfs init script, generated by
  `scripts/build-initramfs.sh`. It reads `root=` from the kernel command line
  and mounts that partition; with no `root=` it falls back to hunting for the
  live squashfs. `imlazy initramfs` rebuilds just that, without a stage1 rerun.
- **Installation**: `scripts/installer/raven-install`, copied into the sysroot
  by `install_installer()` in stage4 so it ships inside the squashfs.
- **The console font**: `GLYPH_SET` in `scripts/make-console-font.py` for which
  glyphs it carries, `install_console_font()` in stage4 for which cell sizes get
  built, and `configs/raven-console-font` for how one is chosen at boot.

## License

See [LICENSE](LICENSE).
