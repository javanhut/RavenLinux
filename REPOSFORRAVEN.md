# GitHub Repos for RavenLinux

The software RavenLinux provides for itself. Everything marked **wired** is
built and installed by `scripts/stages/stage-raven.sh` and has a definition
under `packages/raven/`.

Build it with `imlazy raven` (or `./scripts/build.sh raven`). See the Build Stages
section of [README.md](README.md) for the per-component env knobs.

## Shell — not bash, not fish

| Status | Binary | Repo |
|--------|--------|------|
| **wired** | `ravenshell` | [javanhut/RavenShell](https://github.com/javanhut/RavenShell) |

The default login shell. `/etc/raven/raven-shell` prefers it, falls back to
bash, and finally to `/bin/sh`. bash and fish stay installed and listed in
`/etc/shells` — the fallback path has to work on a base build that skipped the
Raven layer.

## Package Managers

| Status | Binary | Repo |
|--------|--------|------|
| **wired** | `rvn` | [javanhut/RavenPackageManager](https://github.com/javanhut/RavenPackageManager) — the default |
| **wired** | `poxy` | [javanhut/Poxy](https://github.com/javanhut/Poxy) — universal |

Neither consumes the `packages/*/package.toml` definitions yet. Those files
currently document what the system contains; teaching `rvn` to read them
directly is the obvious next step.

## Wireless

| Status | Binary | Repo |
|--------|--------|------|
| **wired** | `caw`, `cawd` | [javanhut/CAW](https://github.com/javanhut/CAW) |

Corvus Access Wifi. Speaks netlink to the kernel directly and implements WPA
itself, so it replaces `iw`, `iwctl`, `wpa_supplicant` and `dhcpcd` outright and
never shells out. Pure Rust on `rustix`'s `linux_raw` backend — no libc, no C
crates, `#![forbid(unsafe_code)]` throughout — which makes it the cleanest
static-musl build in the layer.

Two binaries: `caw` is the CLI, `cawd` the daemon that holds the EAPOL socket
so a connection survives the AP's hourly group-key rotation. Neither is useful
alone, so the Raven stage installs both or neither.

`cawd` runs from `/etc/raven/init.toml` and **replaces `iwd`, which is now
disabled there** — both drive the same wiphy over nl80211 and cannot coexist.
The upstream `dist/cawd.service` and `dist/caw.sysusers` are systemd artifacts
and go uninstalled; the `caw` group is created in stage2 instead. `raven-dhcp`
still runs for wired interfaces.

Autoconnect works as of `b58a394`. `cawd` scans on start and rejoins the
strongest saved network by itself, so a headless box comes back after a reboot
without anyone at the console — which matters here, because RavenLinux ships
sshd enabled and a wifi-only machine that cannot rejoin on its own is a machine
you have to walk over to.

It is on for credentialed networks only. A PSK or SAE profile proves the AP is
itself — an impostor broadcasting the SSID cannot finish the handshake — but an
open network authenticates nothing, so `Profile::new` leaves `autoconnect` off
for `Credential::None` and turning it on is a deliberate edit of the profile.
`min_security` refuses a join weaker than the one first recorded, which is what
stops a saved WPA2 network being answered by an open impostor.

Testing it from the live ISO has a wrinkle that is not a bug. The root is a
read-only squashfs under a **tmpfs** overlay, so `/var/lib/caw/profiles` is
writable but not persistent: `caw connect` saves a profile, autoconnect rejoins
on it for the rest of the session, and a reboot starts from an empty store with
nothing to rejoin. Autoconnect across reboots needs an installed system, not a
live boot — so a reboot test on the ISO showing an idle radio is the expected
result, not a regression.

One integration gap is ours, not CAW's. `caw shutdown` is now the graceful stop
— it deauthenticates and removes the socket — and upstream's systemd unit wires
it as `ExecStop=`. **raven-init has no equivalent**: `shutdown_services` sends
SIGTERM and then SIGKILL, and `/etc/raven/shutdown.d` scripts only run *after*
that, so there is no hook that fires while `cawd` is still alive. Until
raven-init learns a per-service stop command, a reboot still drops the station
without a deauth and the AP holds it until the inactivity timeout.

WPA2/3-Enterprise stays off: its TLS stack pulls in a C crypto provider, which
would end the static build.

## Version Control

| Status | Binary | Repo |
|--------|--------|------|
| **wired** | `ivaldi` | [javanhut/Ivaldi](https://github.com/javanhut/Ivaldi) |

Already this repository's own VCS.

## Text Editor

| Status | Binary | Repo |
|--------|--------|------|
| **wired** | `crow` | [javanhut/CrowTextEditor](https://github.com/javanhut/CrowTextEditor) |

## Task Runner

| Status | Binary | Repo |
|--------|--------|------|
| **wired** | `imlazy` | [javanhut/ImLazy](https://github.com/javanhut/ImLazy) |

## Languages

| Status | Binary | Repo |
|--------|--------|------|
| **wired** | `oxigen` | [javanhut/OxigenLang](https://github.com/javanhut/OxigenLang) |
| not wired | — | Rust toolchain |
| not wired | — | Go toolchain |

`oxigen` ships as a static interpreter, so it needs nothing else installed.

Rust and Go are a different problem. They are currently **build host**
dependencies — the Dockerfile installs both, and the Raven layer uses them to
cross-compile every component to a static binary. Shipping the compilers
*inside* the distribution is a separate bootstrap job (a musl-hosted rustc plus
cargo, and a Go toolchain built for musl), large enough to deserve its own stage
rather than a row in the component table.

## GUI

| Status | Binary | Repo |
|--------|--------|------|
| not wired | `raven-terminal` | [javanhut/RavenTerminal](https://github.com/javanhut/RavenTerminal) |

RavenTerminal is GPU-accelerated and links OpenGL and GLFW through cgo, so it
needs a display server and a graphics stack. The console base has none of that.
It belongs with whatever graphical layer gets built back — at which point it
wants its own stage, not a slot in the Raven layer. That layer now exists:
RavenGUI below provides the display server, and `stage-gui.sh` is where
RavenTerminal would go.

## Compositor and Desktop Shell

| Status | Binary | Repo |
|--------|--------|------|
| **wired** | `huginn`, `muninn-lock` | [javanhut/RavenGUI](https://github.com/javanhut/RavenGUI) |

Built by `scripts/stages/stage-gui.sh` — **its own stage**, not the Raven
layer. Build it with `imlazy gui`; it runs between `raven` and `stage4`.

`huginn` is a Wayland compositor on Smithay, and it draws the desktop itself —
dock, launcher, overview, notifications — inside its own render loop rather than
hosting a shell client. Anything that must feel instant and must never fail does
not get to be a separate process that can miss a frame or die. `muninn-lock` is
the one exception, and it has an independent reason: `ext-session-lock-v1` only
guarantees a locked screen if a bug elsewhere cannot take the locker down with
it, which requires it not to share an address space with the rest of the shell.

It gets its own stage because it cannot satisfy the Raven layer's one
invariant. Every component under `packages/raven/` is a static binary that adds
nothing to the sysroot's runtime link graph; `huginn` links **seventeen** shared
libraries — libdrm, libgbm, libinput, libseat, libudev, libxkbcommon and the
libinput dependency chain behind them, which reaches as far as libwacom and
lua — and Mesa's EGL/GLES drivers are dlopened on top of that. Putting it in the
Raven layer would quietly end the property that makes that layer droppable.

The base was closer to ready than it looked. `stage2-native.sh:copy_libraries()`
already stages Mesa, EGL, GBM, the DRI/GBM loader modules and Xorg/Xwayland from
the build host, and `copy_system_utils()` brings in `seatd`. Eleven libraries
were missing; the GUI stage resolves them with `ldd` against the binaries it
just built rather than from a hand-written list, because the list is not
knowable from the manifests and would rot the first time a dependency changed.

raven-init needed no changes at all. Booting with `raven.graphics=wayland`
already made it disable the tty1 getty, ensure `seatd`, create `/run/user/0`
with `LIBSEAT_BACKEND=seatd`, and exec `/bin/raven-wayland-session` with
`RAVEN_WAYLAND_COMPOSITOR` taken from `raven.wayland=<name>`. That launcher
simply did not exist — init fell through to a `/bin/raven-compositor` that
nothing built. The GUI stage installs it: it sets up `XDG_RUNTIME_DIR` and
`LIBSEAT_BACKEND` and then `exec`s the compositor, so huginn runs directly under
init — its exit status is the session's, and a signal from init reaches it
rather than a wrapper that would have to forward it. There is nothing to start
after it, because the compositor draws the shell.

Still outstanding upstream: privilege gating on `raven_shell_v1` — every client
can currently read workspace state and switch workspaces — and SIGTERM handling
in the compositor. The shell is still software-rendered; the iced renderer is
deliberately deferred, and the protocol plumbing does not change when it
lands.

This is the prerequisite for RavenTerminal rather than a peer of it: it is the
display server RavenTerminal has been waiting for.
