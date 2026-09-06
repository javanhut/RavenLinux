# RavenLinux Architecture

## Overview

RavenLinux is an independent Linux distribution built from scratch. This
document describes the base system — the layer everything else gets built on
top of.

The base is deliberately narrow. It boots, gives you a shell, talks to the
network, and can be rebuilt from source in one command. On top of it sits the
Raven layer — the shell, package managers, version control, editor, task runner
and language RavenLinux provides for itself rather than inheriting.

Above that sits the graphical layer: the Huginn compositor, which draws the
desktop itself, the terminal and file manager it opens, the wallpaper behind
them, and the login screen in front of the lot.

What is still intentionally left out: hosted Rust/Go toolchains.

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
| `raven-lock` | RavenLogin | Rust | Session lock screen: the login screen's twin, on `ext-session-lock-v1` |
| `raven-terminal` | RavenTerminal | Go + cgo | Terminal emulator (OpenGL 4.1 via GLFW, Wayland backend) |
| `ravenfilemanager` | RavenFileManager | Rust | File manager (GTK4, libadwaita) — the image's only GTK client |
| `ravencanvasd`, `ravencanvas` | RavenCanvas | Rust | The wallpaper: a wlr-layer-shell client, and its control CLI |
| `roostbar` | RoostBar | Rust | Layer-shell status bar, started through the global session.d drop-in |
| `ravend`, `raven-greeter` | RavenLogin | Rust | The login daemon, which reads `/etc/shadow`, and the login screen, which does not |
| `raven-installer-ui` | `installer-ui/` (this repo) | Rust | The graphical installer (GTK4, libadwaita): a front-end for `raven-install`, which does the installing |

What they drag in spans two orders of magnitude, and the stage has a written
list of none of it. `huginn` links seventeen shared libraries — libdrm, libgbm,
libinput, libseat, libudev and the chain behind them — with Mesa's EGL/GLES
drivers dlopened at run time. `ravenfilemanager` links a hundred and thirty-four:
GTK4, libadwaita, pango, cairo, harfbuzz, GStreamer, appstream, krb5, gnutls and
the desktop stack behind them. `ravend` and both RavenCanvas binaries link libc
and next to nothing else. `stage_gui_libraries()` resolves the closure with `ldd`
against the binaries it has just built and stages whatever stage2 did not already
provide — because a list written here would be right on the day it was written
and silently wrong the first time a dependency changed. libinput alone reaches
libwacom, which reaches lua.

`ldd` is necessary and not sufficient, which is why `stage_gtk_runtime()` exists.
A toolkit reads *data* at run time that no linker mentions: the compiled
GSettings schema cache, without which every GTK application aborts at startup;
`gsettings-desktop-schemas`, without which libadwaita cannot tell light from
dark; `mime.cache`, without which every file is `application/octet-stream`; and
`bwrap`, without which glycin decodes no images and the toolkit displays none.
Each fails quietly and separately. The caches are regenerated against the sysroot
rather than copied, since one built on the host encodes host paths.

RavenCanvas is the one in that table that could have been built anywhere. Both
its binaries link libc, libm and libgcc_s and nothing else — its Wayland client
is wayland-rs's pure-Rust backend rather than libwayland — so it satisfies the
Raven layer's invariant outright. (`ravend` links no more than that either, but
it ships with a greeter that draws, and a workspace builds against one target at
a time.) It is built here regardless, because everything it needs from the image
— `/usr/share/wallpaper/set` and the session launcher that starts it — is
written by this stage and by no other, and the Raven layer runs first.

Only RavenGUI's two binaries and `raven-terminal` can fail the stage — the first
because there is then nothing to log into, the last because huginn names it in
two compiled-in places and a desktop that cannot open a terminal can start no
process at all. The other three are things a desktop can be missing rather than
parts of one, so every failure path in `stage_filemanager()`, `stage_canvas()`
and `stage_login()` warns and returns 0, and `FILEMANAGER_SKIP`, `CANVAS_SKIP`
and `LOGIN_SKIP` leave each of them out on purpose.

RavenTerminal is deliberately not part of the *Raven* layer, for the same reason
huginn is not: it is a GPU-accelerated terminal binding OpenGL and GLFW through
cgo, so it cannot be the static musl binary every component of that layer is. It
belongs here instead, with the display server it needs — and not as an optional
extra, because huginn names `raven-terminal` in two compiled-in places and a
desktop that cannot open a terminal can start no process at all.

The stage builds it with `-tags wayland` rather than letting its Makefile detect
a backend from `$XDG_SESSION_TYPE`, which is never set in a container and would
silently select X11.

### Init System (`init/`)

`raven-init` runs as PID 1. It reads `/etc/raven/init.toml`, mounts the virtual
filesystems, and hands off to `raven-rc` for service supervision.

| File | Role |
|------|------|
| `init/src/main.rs` | PID 1 entry point, early boot |
| `init/src/config.rs` | `init.toml` parsing |
| `init/src/service.rs` | Service definition and lifecycle |
| `init/src/rc.rs` | Service manager (`raven-rc`) |
| `init/src/power.rs` | Suspend to RAM, and the `/run/raven-power/state` marker |
| `init/src/powerd.rs` | `raven-powerd`: the power button, the sleep button, the lid, and the `/run/raven-power/ctl` socket |
| `init/src/profile.rs` | `raven-powerd`'s CPU/platform power profile: governor, energy-performance preference, ACPI platform profile and PCIe ASPM, switched between the `[profile]` presets in `power.toml` as the supply changes |

#### Sleep

There is no logind here, so the split is drawn by hand. `raven-powerd` decides
*whether* to sleep -- it reads the evdev nodes for the power button, the sleep
button and the lid, and looks up what they mean in `/etc/raven/power.toml` --
and PID 1 does the sleeping, because the write to `/sys/power/state` is
privileged, must not race another one, and does not return until the machine is
awake. The daemon asks over the same control socket `raven-rc` uses, so
`raven-rc suspend` and closing the lid take the identical path.

The desktop is the third thing that asks, and it asks the daemon, not init.
`raven-powerd` listens on `/run/raven-power/ctl`, group `video` and mode 0660,
for one of `suspend`, `poweroff` or `reboot`, and routes the word through the
same code a lid close takes -- cooldown, then the request to init, with the
same fallbacks when init is old or absent. That socket is the desktop's logind
stand-in: it is what Huginn's quick settings write to. It does not contradict
the rule, stated where Huginn watches the sleep marker, that an unprivileged
session gets no socket into PID 1. Init's own socket stays 0600 and root-only;
what the session holds is a socket into the daemon that is already the policy
gatekeeper and already the only thing that asks init to sleep, and what it can
say to it is one of three verbs. The desktop gets a verb, never PID 1. `video`
because the session already holds that group for the DRM device, so the right
to sleep the machine follows the right to draw on it without a group invented
for the purpose.

Waking is nobody's code: the power button and the lid are ACPI wakeup sources,
and `raven-powerd` arms them at start so that a machine which sleeps can also
be woken. Keyboards are deliberately not armed -- a laptop keyboard claims to
have a power key, and an armed i8042 is a classic cause of a machine that
resumes a second after it suspends.

Either side of the sleep, init writes one word to `/run/raven-power/state` and
runs `/etc/raven/sleep.d/*` with `pre` or `post`. The marker is what tells
Huginn to re-take the display and repaint: it held DRM master straight through
the suspend, and without logind's `PrepareForSleep` a file in a tmpfs is the
signal.

#### Locking on resume

The same marker is what locks the machine. Huginn blanks the session *first* and
starts `raven-lock` second, then re-takes the display -- so the first frame
composited after a resume is the lock screen and never the desktop. Doing it the
other way round would show the session for as long as a process takes to exec
and connect, which on a laptop is exactly the moment somebody has just opened
the lid.

The blank going up before any client exists is the reason huginn arms a ten
second timer alongside it: if `raven-lock` never claims the session, the blank
comes back down. A machine whose lock screen is missing should show its desktop,
not a black display with no way past it.

There is no setting for this. A laptop that sleeps on a lid close and wakes
showing the desktop has a lock screen in name only.

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
  (small hand-off)          (raven-init)
       │                        │
       └───────────┬────────────┘
                   ▼
          raven-init starts services
          and the selected session/getty
```

The two paths share everything after `switch_root`, including raven-init as PID
1 and the service graph. The live root enters it through a tiny `/init`
hand-off; an installed root enters through `/sbin/init`. `raven-install` copies
the squashfs, removes the hand-off, and writes a `boot.cfg` carrying
`root=UUID=…`.

`raven.live` on the command line forces the live path even when a `root=` is
present.

### Console Font

The Linux virtual terminal draws from a PSF bitmap font, not a scalable one, so
the JetBrains Mono Nerd Font TTFs in `fonts/` are of no use to tty1 on their
own. stage4 rasterises the regular face into
`/usr/share/kbd/consolefonts/raven-<W>x<H>.psfu` at four cell sizes with
`scripts/make-console-font.py`, and the one-shot `console-font` service loads it
through raven-init on both live and installed systems.

The cell size is chosen from the framebuffer width rather than fixed, because
the kernel's built-in 8x16 font is unreadable on a HiDPI laptop panel and
oversized on a VGA console. Box-drawing and Powerline glyphs are stretched to
the cell edge where the font's advance falls short of it, so drawn lines join
across cells instead of coming out dashed.

The generator is fail-soft: a build host without `freetype-py` produces a
working ISO whose console runs on the kernel font.

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

#### The graphical installer (`installer-ui/`)

`raven-installer-ui` is a second way to drive that script and not a second
implementation of it. It partitions nothing: it asks the questions the wizard
asks, writes the answers to a file, and runs `raven-install`. Three options on
that script are the whole interface, and they are documented as `PROTOCOL` at
the top of it:

| Option | What it is for |
|--------|----------------|
| `--probe` | Report the preflight results, the disks and what is on them, the source tree, the timezones and the profiles as `key=value` on stdout. Nothing is written, and a failed check is reported as a value rather than an exit code. |
| `--answers FILE --non-interactive` | Take the wizard's answers from a file instead of a person. Validated by `apply_answers`, which uses the same validators the prompts do and refuses rather than corrects. |
| `--progress-fd N` | Write each status line a second time to descriptor N as one record per line: `phase`, `pct`, `ok`, `warn`, `fail`, `info`, `dirty`, `done`. |

The probe is what keeps the two front-ends in parity. It runs the *real*
preflight and the *real* source lookup — `soft_die` is the whole of the
difference — so the disks, filesystems and profiles the window offers are the
ones the script would offer, including the ones it refuses. A machine
`raven-install` will not install onto is a machine the window will not offer to
install onto, in the script's own words, without the window having been taught
what those words are.

It is in this tree rather than a repository of its own, unlike every other GUI
component, because it is the other half of a file in `scripts/installer/` and a
version skew between them is a wizard that cannot drive the installer it is
looking at. `scripts/installer/test-raven-install.sh` fails when the two
disagree about the answer keys, the phase ids or the protocol version.

#### Reaching root, and who the live desktop is

The live ISO's desktop runs as **root**, and only the live one. Its boot
entries — the GRUB entry in `stage4-iso.sh` and RavenBoot's compiled-in
defaults in `bootloader/src/config.rs` — carry `raven.user=root`, which is the
override `raven-init` already reads. Without it the session falls to
`user::first_regular()`, which on this image is the `raven` placeholder: an
account that exists so the image has a non-root user, not because anyone is
meant to be it.

It is live-only by construction rather than by a runtime check. Those two
entries are what boot the ISO; the entries `install_bootloader` writes to an
installed disk carry `root=UUID=` and no `raven.user`, so an installed machine
uses the account the installer created. `test-raven-install.sh` fails if
`raven.user` ever appears in what `raven-install` writes.

On an installed system the window therefore runs as an ordinary user, and
getting to root is `sudo` — there is no polkit agent on this image. Only the
call that installs is elevated, and the password, asked for before the wizard
rather than in front of the install, fills sudo's timestamp and is not kept.
`privesc::detect` asks `sudo -n -l -- <installer>` rather than `sudo -n true`,
because the live image's grant is scoped to that one command and a blanket
question would be answered "password required" on the image built not to need
one.

That grant is `/etc/sudoers.d/10-raven-live-installer`, written by stage4 and
deleted by `remove_live_credentials()` during `configure_system`, beside where
the tty1 autologin is replaced with a real login prompt. It is not a hole on
the live image — tty1 there runs with `--skip-login` and is already an
unauthenticated root shell — and it is the reason the graphical installer still
works if someone boots the ISO with a different `raven.user=`.

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
| **GUI** | The desktop: huginn, raven-terminal, ravenfilemanager, ravencanvasd, roostbar, ravend, raven-lock, the application menu, and the shared libraries, GTK runtime, icon themes and cursor theme they need |
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
- `packages/gui/` — ravengui (huginn), ravenfilemanager,
  ravenlogin, ravencanvas. raven-terminal is built by the same stage from its
  own repository and has no manifest here yet

## Extending the System

The base is designed to be grown, not modified:

- **A package** → a `package.toml` under `packages/`, plus a build function in
  stage 3
- **A Raven tool** → one row in the `RAVEN_COMPONENTS` table in
  `scripts/stages/stage-raven.sh`, plus a `package.toml` under `packages/raven/`
- **A new software class** (desktop, toolchain, editor suite) → its own stage
  script and a case in `build.sh`, rather than an ever-growing stage 3
- **A host build dependency** → `check-deps.sh` *and* the `Dockerfile`
- **Boot behavior** → `create_managed_live_init()`, `setup_ravenboot()`, and
  `setup_grub()` in `scripts/stages/stage4-iso.sh`
- **How a disk boots** → the init script generated by
  `scripts/build-initramfs.sh` (`raven_root_from_cmdline`,
  `raven_mount_disk_root`)
- **Installation** → `scripts/installer/raven-install`. A question the wizard
  asks needs a key in its `ANSWER_KEYS` and `apply_answers`, and a row in
  `installer-ui/src/pages.rs`; the parity tests fail until both are there
- **The console font** → `scripts/make-console-font.py` (glyph set),
  `install_console_font()` in stage4 (cell sizes), `configs/raven-console-font`
  (selection at boot)
- **What the desktop can launch** → `install_desktop_entries()` in stage-gui
  (the `.desktop` files), and RavenGUI's `dock::PINNED` and `theme::TERMINAL`
  (the two compiled-in names the entries have to match)
- **The desktop's cursor, icons and fonts** → `stage_gui_data()` in stage-gui
  and the icon-theme list in stage2's `copy_system_utils()`; the packages
  themselves come from the `Dockerfile`
- **The boot menu** → `bootloader/src/` (RavenBoot's menu is compiled in)
- **Service management** → `init/src/service.rs` and `init/src/rc.rs`
