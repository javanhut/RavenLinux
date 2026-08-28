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

`cawd` runs from `/etc/raven/init.toml` and is **the only wireless daemon in
the image** — iwd is neither built nor shipped, because two nl80211 daemons on
one wiphy fight over the interface.
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
| **wired** | `raven-terminal` | [javanhut/RavenTerminal](https://github.com/javanhut/RavenTerminal) |

RavenTerminal is GPU-accelerated and links OpenGL and GLFW through cgo, so it
needs a display server and a graphics stack. The console base has none of that,
which is why it sat unwired while the Raven layer was all that existed. It is
built by `scripts/stages/stage-gui.sh` — the stage that builds the display
server it was waiting for.

It is not optional there. Huginn names `raven-terminal` in two compiled-in
places — `theme::TERMINAL`, what `Super`+`Shift`+`T` spawns, and `dock::PINNED`,
which it shares with the file manager below — so a desktop without it can start
no process at all.

It does *not* boot to a dock holding a dead icon, which this section claimed for
a while and `dock::items` has never done. A pinned name matching no `.desktop`
entry is skipped rather than drawn: the loop pushes an item only where
`position` finds an entry, for both pinned names alike. So `TERMINAL_SKIP=1`
yields a dock with the launcher and the file manager on it, exactly as
`FILEMANAGER_SKIP=1` yields one with the launcher and the terminal — and the
failure is silent at the dock and total everywhere else, which is the argument
for the stage summary saying so rather than the dock showing it.

Two details of how the stage builds it are load-bearing:

- **`-tags wayland`, stated rather than detected.** Its Makefile chooses a
  backend with `BACKEND ?= auto`, which reads `$XDG_SESSION_TYPE` and falls back
  to X11 when unset — and it is always unset in a container. Left alone, the
  build would quietly produce the X11 backend, and the terminal would run
  through XWayland on a Wayland-only machine, or not at all on an image where
  Xwayland was never staged.
- **`CGO_ENABLED=1`, set rather than inherited.** The Raven stage exports
  `CGO_ENABLED=0` for its static binaries, and a leaked `0` turns this into a
  link failure against `-lwayland-client` that names nothing about cgo.

Its GLFW is vendored in-tree with the xdg-shell and viewporter protocol sources
pre-generated, so the build needs no wayland-scanner and no wayland-protocols —
only `wayland`, `libxkbcommon` and `mesa`, all of which the Dockerfile already
installs for huginn. Its Nerd Fonts are embedded in the binary, so it needs
nothing from `/usr/share/fonts` either.

## Compositor and Desktop Shell

| Status | Binary | Repo |
|--------|--------|------|
| **wired** | `huginn` | [javanhut/RavenGUI](https://github.com/javanhut/RavenGUI) |

Built by `scripts/stages/stage-gui.sh` — **its own stage**, not the Raven
layer. Build it with `imlazy gui`; it runs between `raven` and `stage4`.

`huginn` is a Wayland compositor on Smithay, and it draws the desktop itself —
dock, launcher, overview, notifications — inside its own render loop rather than
hosting a shell client. Anything that must feel instant and must never fail does
not get to be a separate process that can miss a frame or die. The lock screen
is the one thing that goes the other way, and for the opposite reason:
`ext-session-lock-v1` only guarantees a locked screen if a bug elsewhere cannot
take the locker down with it, which requires it not to share an address space
with the rest of the shell. It is `raven-lock`, and it ships with RavenLogin
rather than here — it is the login screen's twin, drawn by the same code.

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

raven-init needed one change, and it is the one worth knowing about. Booting
with `raven.graphics=wayland` already made it disable the tty1 getty, ensure
`seatd`, set `LIBSEAT_BACKEND=seatd`, and exec `/bin/raven-wayland-session`
with `RAVEN_WAYLAND_COMPOSITOR` taken from `raven.wayland=<name>`. That
launcher simply did not exist — init fell through to a `/bin/raven-compositor`
that nothing built.

What init did *not* do was give the session an owner. Services inherit PID 1's
credentials, so the compositor and everything launched from its dock ran as
uid 0, and the `video`/`render`/`input` groups the installer sets up meant
nothing because root ignores them. `ServiceConfig` now takes a `user` field:
init resolves `raven.user=<name>`, or failing that the lowest-uid regular
account, creates `/run/user/<uid>` owned by them, and drops supplementary
groups, gid and uid — in that order — between `fork` and `exec`. A name that
cannot be resolved fails the start rather than falling back to root.

The GUI stage installs the launcher: it sets up `XDG_RUNTIME_DIR` and
`LIBSEAT_BACKEND` and then `exec`s the compositor, so huginn runs directly under
init — its exit status is the session's, and a signal from init reaches it
rather than a wrapper that would have to forward it. There is nothing to start
after it, because the compositor draws the shell.

Still outstanding upstream: privilege gating on `raven_shell_v1` — every client
can currently read workspace state and switch workspaces — and SIGTERM handling
in the compositor. The shell is still software-rendered; the iced renderer is
deliberately deferred, and the protocol plumbing does not change when it
lands.

This was the prerequisite for RavenTerminal rather than a peer of it, and the
dependency now runs the other way too: the same stage builds both, because
huginn names `raven-terminal` in two compiled-in places and a desktop that
cannot open a terminal cannot start anything at all. Neither ships usefully
without the other.

## File Manager

| Status | Binary | Repo |
|--------|--------|------|
| **wired** | `ravenfilemanager` | [javanhut/RavenFileManager](https://github.com/javanhut/RavenFileManager) |

Built by `scripts/stages/stage-gui.sh`, from its own repository, alongside
huginn and RavenTerminal. `imlazy gui` builds it; `FILEMANAGER_SKIP=1` leaves it
out.

It is the first thing on this image that is an *application*. huginn and
`raven-terminal` are the desktop — without either there is nothing to log into,
or nothing to launch — so their failures fail the stage. An image without a file
manager is a working desktop missing a program, so every failure path in
`stage_filemanager()` warns and returns 0.

It is also the first GTK client, and that is the part worth knowing about.

**A hundred and thirty-four shared libraries.** huginn links seventeen and that
was enough to keep it out of the Raven layer, whose invariant is a static binary
that adds nothing to the sysroot's link graph. This links GTK4, libadwaita,
pango, cairo, harfbuzz, GStreamer, appstream, krb5 and gnutls, and the desktop
stack behind them. `stage_gui_libraries()` resolves the set with `ldd` like
everything else here, so no list of them is written down anywhere.

**And four things `ldd` cannot see.** This is why `stage_gtk_runtime()` exists.
The sysroot had a complete graphics stack and none of the *data* a toolkit reads
at run time, and each piece fails quietly and separately:

| Missing | Symptom |
|---|---|
| `gschemas.compiled` | **the application aborts on startup.** GSettings reads only the compiled cache, never the XML, so `g_settings_new()` is a fatal GLib error naming a schema |
| `gsettings-desktop-schemas` | libadwaita cannot read `org.gnome.desktop.interface`, silently stays light, and looks like an app that ignores the system theme |
| `/usr/share/mime/mime.cache` | every file is `application/octet-stream`: one icon, no "Open With" |
| `bwrap` | **no images anywhere in the toolkit.** Since gdk-pixbuf 2.44 the loaders moved out of process into glycin, which decodes inside a bubblewrap sandbox and refuses to decode without one |

The caches are regenerated against the sysroot rather than copied — a cache
built on the host encodes host paths — and all three tools take the directory to
work on as an argument, so the stage needs no chroot to run them.

On an installed system the same set is `raven-desktopinstall`'s `toolkit` set,
and rebuilding those caches is the reason that script does not stop when `rvn`
does.

`huginn` pins it. `dock::PINNED` names `com.ravenfilemanager.Raven` — by
*desktop file stem*, which is why it appears there under a reverse-DNS name and
`raven-terminal` does not: GTK requires an application's entry to be named for
its application id. A pinned name matching no entry is skipped rather than drawn
dead, so `FILEMANAGER_SKIP=1` yields a one-icon dock rather than an icon that
launches nothing.

`install_desktop_entries()` writes its entry outside the `raven-terminal` guard,
unlike crow's: it opens its own window and does not need a terminal to exist.
The entry adds the `StartupWMClass` upstream's `data/*.desktop` omits — the
match works without it, because the file stem and the app_id happen to agree,
but `dock::owns` checks `StartupWMClass` first and a coincidence of two names is
not something to rely on. It also claims `inode/directory` in a `mimeapps.list`;
nothing on the image claimed it before, so "Open Containing Folder" from any
application resolved to nothing.

## Login

| Status | Binary | Repo |
|--------|--------|------|
| **wired** | `ravend`, `raven-greeter` | [javanhut/RavenLogin](https://github.com/javanhut/RavenLogin) |

The login screen, and the daemon behind it. Built by `stage_login()` in
`stage-gui.sh`.

`raven-init` used to resolve `raven.user=<name>`, or failing that the lowest-uid
regular account, and start the session as them with no password prompt anywhere
in the path — right for an image you are bringing up, wrong for a machine you
use. RavenLogin is that prompt, and it is now what an image with
`raven.graphics=wayland` boots to. The autologin session is what is left when
`ravend` is not installed.

The split is the design: `ravend` runs as root and reads `/etc/shadow`;
`raven-greeter` runs as its own `raven-greeter` account, draws, and can ask one
question over a `0600` Unix socket that `ravend` checks `SO_PEERCRED` on anyway.
Drawing a login screen means parsing fonts, rasterizing glyphs and decoding
images — near enough a list of everything that has ever been remotely
exploitable in a login screen — and the password hashes are not in that process.

Wiring it needed four things, and this is where each of them ended up:

- **a stage that builds both binaries.** `stage_login()` in `stage-gui.sh`,
  after the session launcher, on the pattern the terminal and the file manager
  already use: its own repository, its own cargo invocation, every failure path
  a warning. `LOGIN_SKIP=1` leaves it out.
- **a `raven-greeter` account and group.** Created by `stage_login()` rather
  than in stage2 alongside `caw`'s. stage2's account table is Arch's canonical
  numbering, copied because `rvn` extracts tar payloads that record ownership
  numerically; `raven-greeter` is ours and not Arch's, and putting our own id
  in that table would blur what the table is for. It takes uid 972, or the next
  free id below it, and joins video, render, input and seat.
- **an `init.toml` service for `ravend`, and an init that hands off to it.**
  Neither is written to `init.toml`. `apply_kernel_cmdline_overrides()` looks
  for the `ravend` binary and, finding it, ensures that service *instead of*
  `wayland-session` and returns — so installing the binary is the whole of
  turning the login screen on, and no file on the image records the choice.
  `wait_for_seat()` counts it too: the greeter's compositor needs a seat before
  anybody has logged in.
- **what happens when `ravend` fails to start.** It restarts, and nothing falls
  through to the passwordless session — that path is not reached at all on an
  image that has `ravend`. It is not `critical`, so a boot is not panicked over
  it, and the console gettys are still there to fix a machine whose greeter
  will not come up.

`stage_login()` creates `/usr/share/wallpaper` and `/usr/share/wallpaper/set`
empty. Drop an image in as `set/wallpaper.<ext>` and both huginn and the greeter
draw it — or install RavenCanvas below, which is what makes that a command
rather than a file operation.

## Wallpaper

| Status | Binary | Repo |
|--------|--------|------|
| **wired** | `ravencanvasd`, `ravencanvas` | [javanhut/RavenCanvas](https://github.com/javanhut/RavenCanvas) |

Built by `stage_canvas()` in `stage-gui.sh`, from its own repository, alongside
huginn, the terminal, the file manager and the login screen. `imlazy gui` builds
it; `CANVAS_SKIP=1` leaves it out, and an image without it shows huginn's own
flat background.

`ravencanvasd` is an ordinary unprivileged Wayland client that puts one
`wlr-layer-shell` surface on the background layer of each output and draws a
picture on it. `ravencanvas` is the CLI that reconfigures it over a control
socket without a restart; `raven-set-wallpaper` — upstream's
`scripts/set-wallpaper.sh`, renamed on the way in as its own installer renames
it — sets the *machine's* wallpaper, which is a different thing and the more
useful of the two.

Four modes: a flat colour, one PNG or JPEG fitted five ways, every image in a
watched directory in turn with a crossfade, or one of four computed scenes with
no file on disk at all.

**It is a separate process on purpose, and it is the one component of this
desktop that is.** huginn draws its dock, launcher, overview and notifications
inside its own render loop, because the design rule is that anything which must
feel instant and must never fail does not get to be a process that can miss a
frame or die. A wallpaper is exactly the case that rule is not about — the file
may be missing, corrupt, or on a disk that is not mounted yet — and huginn
already paints its own background under everything, so the worst this process's
death can do is leave a plain desktop. RavenGUI's `docs/protocols.md` says so
directly: *"Panels, the dock and the wallpaper are wlr-layer-shell surfaces. Do
not duplicate those here."*

Three things about how the stage builds and wires it are worth knowing.

**It is not in the Raven layer, and it would qualify.** Every component under
`packages/raven/` is a static binary that adds nothing to the sysroot's runtime
link graph, and this meets that: `ldd` on either binary reports libc, libm and
libgcc_s and no more. Its Wayland client is wayland-rs's pure-Rust backend, so
there is no libwayland-client in the link graph at all, and `--as-needed` drops
the libxkbcommon a client that never touches the keyboard never calls into. It
is built here anyway because linkage is not the only thing a stage owns.
Everything this daemon needs from the image is written by `stage-gui.sh` and by
nothing else — `/usr/share/wallpaper/set`, which is also what the greeter reads,
and `raven-wayland-session`, which is where a session client gets started from.
The Raven stage runs first, so a wallpaper staged there would be wired into a
launcher this stage then overwrites whole.

**It is started from the session script, not from `init.toml`.** Every other
daemon on this image is a service there; this one cannot be. Init's services run
as root before anybody has logged in, and this is an unprivileged client that
has to run as the session's own user and share its runtime directory.
`install_session_launcher()` backgrounds it immediately before that script's
final `exec` — the same shape the session bus above it uses, for a stronger
version of the same reason: the daemon is a client of the compositor the script
is about to *become*, so there is no line after the `exec` on which to start it.
`connect()` retries for ten seconds, and finds the socket by searching
`$XDG_RUNTIME_DIR` rather than reading a `$WAYLAND_DISPLAY` that nothing has set
yet — the compositor binds the first free number, so a stale lock from a
previous session puts it on `wayland-1`. The line is guarded with `command -v`,
so `CANVAS_SKIP=1` needs no second version of the launcher.

**The shipped `/etc/raven/canvas.toml` has its `[background]` commented out, and
that is load-bearing.** With no `[background]` anywhere the daemon draws
`/usr/share/wallpaper/set/wallpaper.<ext>` — the same file the greeter draws the
login screen on, which is what makes the two show one picture. Writing a
`[background]` into that file overrides the machine's wallpaper for every user
and breaks the contract; `~/.config/raven/canvas.toml` overrides it for one
user, which is what `ravencanvas set --persist` writes and is the supported way
to differ. So:

```bash
sudo raven-set-wallpaper /path/to/image   # the machine: desktop and login screen
ravencanvas set scene aurora --persist    # this user only
raven-set-wallpaper status                # what is set, and what overrides it
```

The daemon watches `set/`, so the desktop changes within a moment and there is
nothing to restart; the greeter and huginn read the file once, at their own
start, so the login screen follows at the next one.

Nothing is shipped into the library. An empty `set/` is a working image that
draws the built-in gradient, and a photograph committed to a distribution
repository is a licence question nobody asked for — which the GUI stage's
**Desktop:** summary now says out loud, because a desktop showing that gradient
otherwise looks like a wallpaper that failed.

Costs nothing when it is not doing anything, and that is a design property
rather than a hope. A still wallpaper is drawn once and then blocks on its four
descriptors with no timer armed; an animated one stops on its own when it cannot
be seen, because a compositor sends no frame callback for a surface it is not
going to draw and nothing here renders until the previous callback arrives.
