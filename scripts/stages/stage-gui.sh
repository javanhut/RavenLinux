#!/bin/bash
# =============================================================================
# RavenLinux GUI Stage: Compositor and Desktop Shell
# =============================================================================
# Builds the desktop: RavenGUI's Huginn, the Wayland compositor and the shell it
# draws, plus RavenTerminal, from its own
# repository, because huginn names `raven-terminal` in two compiled-in places
# and a desktop that cannot open a terminal cannot start anything at all, and
# RavenFileManager, from a third. See REPOSFORRAVEN.md for what each of them is.
#
# The three are not equal. huginn and raven-terminal are the desktop: without
# either there is nothing to log into or nothing to launch, so their failure
# paths mark the whole stage failed. RavenFileManager is an application, and
# an image without it is a working desktop missing a program -- so every one of
# its failure paths warns and returns 0. It is also the only GTK client here,
# which is why stage_gtk_runtime() exists at all.
#
# Two more repositories are built here and documented at their constants below:
# RavenLogin, the password prompt this image boots to, and RavenCanvas, the
# wallpaper daemon. Both are optional in the file manager's sense -- every
# failure path warns and returns 0 -- and neither can fail the stage.
#
# THERE IS NO SEPARATE SHELL PROCESS
#
# The dock, launcher, overview and notifications are drawn by the compositor
# itself, inside its render loop, so nothing that must feel instant can miss a
# frame or die on its own. The lock screen is the one thing that goes the other
# way, for the opposite reason: ext-session-lock-v1 keeps the screen locked when
# the locking client dies, which is worth nothing if that client shares an
# address space with the rest of the shell. It is `raven-lock`, and it comes
# from RavenLogin rather than from here -- it is the login screen's twin, drawn
# by the same code and checking passwords against the same daemon.
#
# Like the Raven stage this is unnumbered, and for the same reason: stages 0-4
# build a console system that has to stand on its own. It runs after raven and
# before stage4, because stage4 squashes the sysroot into the ISO.
#
#   stage0 -> stage1 -> stage2 -> stage3 -> raven -> gui -> stage4
#
# WHY THIS IS NOT PART OF THE RAVEN STAGE
#
# Every component in stage-raven.sh is a static binary -- Go with CGO_ENABLED=0,
# Rust against musl -- so the Raven layer adds nothing to the sysroot's runtime
# link graph and any part of it can be dropped without consequence. Huginn
# cannot be built that way and never will be: smithay reaches the hardware
# through libdrm, libgbm, libinput, libseat and libudev, and Mesa's EGL/GLES
# drivers are dlopened at runtime. It links seventeen shared libraries.
#
# Mixing it into the Raven layer would quietly end the property that makes that
# layer droppable, so it gets a stage that owns its own linkage instead. The
# base already carries most of a graphics stack -- stage2's copy_libraries()
# stages Mesa, EGL, GBM and the DRI drivers -- and this stage adds whatever the
# built binaries turn out to need on top of it.
#
# HOW IT MEETS THE REST OF THE SYSTEM
#
# raven-init already has the Wayland slot: booting with raven.graphics=wayland
# makes it disable the tty1 getty, ensure seatd, pick the session account, create
# its /run/user/<uid> owned by that user, and start
# /bin/raven-wayland-session with RAVEN_WAYLAND_COMPOSITOR set from
# raven.wayland=<name>. That launcher did not exist until this stage installed
# it.
#
# Init did need changing once, and this is it: the session used to run as root,
# because init starts every service as itself and nothing said otherwise. It
# now resolves an account -- raven.user=<name>, else the lowest-uid regular
# user -- and drops to it, which is why the launcher below no longer drives
# udev itself and no longer assumes /run/user/0.
#
# Environment:
#   GUI_SKIP=1                 skip this stage entirely
#   GUI_SESSION_ONLY=1         update only the session launcher and entry;
#                              do not fetch or compile any GUI component
#   GUI_OFFLINE=1              never touch the network; use the existing clones
#   GUI_REF=<git-ref>          build a particular RavenGUI ref
#   GUI_TARGET=<rust-target>   override the host target (rarely wanted)
#   TERMINAL_SKIP=1            skip RavenTerminal; the desktop ships unusable
#   TERMINAL_OFFLINE=1         as GUI_OFFLINE, for the terminal alone
#   TERMINAL_REF=<git-ref>     build a particular RavenTerminal ref
#   FILEMANAGER_SKIP=1         skip RavenFileManager; the desktop keeps working
#   FILEMANAGER_OFFLINE=1      as GUI_OFFLINE, for the file manager alone
#   FILEMANAGER_REF=<git-ref>  build a particular RavenFileManager ref
#   LOGIN_SKIP=1               skip RavenLogin; the image autologins
#   LOGIN_OFFLINE=1            as GUI_OFFLINE, for the login screen alone
#   LOGIN_REF=<git-ref>        build a particular RavenLogin ref
#   CANVAS_SKIP=1              skip RavenCanvas; the desktop draws huginn's own
#                              flat background instead of a wallpaper
#   CANVAS_OFFLINE=1           as GUI_OFFLINE, for the wallpaper daemon alone
#   CANVAS_REF=<git-ref>       build a particular RavenCanvas ref
#   ROOSTBAR_SKIP=1            skip the status bar
#   ROOSTBAR_OFFLINE=1         reuse an existing RoostBar clone
#   ROOSTBAR_REF=<git-ref>     build a particular RoostBar ref
# =============================================================================

set -euo pipefail

# =============================================================================
# Environment Setup (with defaults for standalone execution)
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${RAVEN_ROOT:-$(dirname "$(dirname "$SCRIPT_DIR")")}"
BUILD_DIR="${RAVEN_BUILD:-${PROJECT_ROOT}/build}"
SYSROOT_DIR="${SYSROOT_DIR:-${BUILD_DIR}/sysroot}"
PACKAGES_DIR="${PACKAGES_DIR:-${BUILD_DIR}/packages}"
SOURCES_DIR="${SOURCES_DIR:-${BUILD_DIR}/sources}"
LOGS_DIR="${LOGS_DIR:-${BUILD_DIR}/logs}"
RAVEN_JOBS="${RAVEN_JOBS:-$(nproc)}"

GUI_SRC_DIR="${SOURCES_DIR}/gui"
GUI_STAGE_DIR="${PACKAGES_DIR}/gui"

GUI_REPO="RavenGUI"
GUI_URL="https://github.com/javanhut/${GUI_REPO}.git"

# The terminal. A second repository rather than a component of RavenGUI: it is
# an ordinary Wayland client that happens to be the one this desktop cannot do
# without, and it shares no code with the compositor.
TERMINAL_REPO="RavenTerminal"
TERMINAL_URL="https://github.com/javanhut/${TERMINAL_REPO}.git"

# The file manager. A third repository, and the first application on this image
# that is neither the compositor nor something the compositor cannot start
# without: the desktop is complete without it, which is why every failure path
# below warns and returns rather than failing the stage.
#
# Its application id is also its .desktop stem, its icon name and its D-Bus
# name, so it is written once here and used for all four.
FILEMANAGER_REPO="RavenFileManager"
FILEMANAGER_URL="https://github.com/javanhut/${FILEMANAGER_REPO}.git"
FILEMANAGER_BIN="ravenfilemanager"
FILEMANAGER_APPID="com.ravenfilemanager.Raven"

# The login screen. A fourth repository, and the last thing between the boot and
# somebody's session: ravend reads /etc/shadow, draws a password prompt through
# a greeter it starts as an unprivileged account, and starts the session itself.
#
# Failure here is a warning and not a failed stage, but it is not the same kind
# of optional as the file manager: an image without it boots to the autologin
# session raven-init has always had, which is a working desktop with no password
# on it. init decides between the two by looking for the binary, so not building
# this is exactly what "no login screen" means. See stage_login().
LOGIN_REPO="RavenLogin"
LOGIN_URL="https://github.com/javanhut/${LOGIN_REPO}.git"
# The account its greeter runs as. Nothing else on the machine runs as this.
LOGIN_GREETER_USER="raven-greeter"
LOGIN_GREETER_HOME="/var/lib/raven-greeter"
# A fixed id, so an image built twice has the same numbers in it and a
# filesystem moved between builds does not change owner. 972 is below the 1000
# regular-account floor and outside the block Arch's sysusers hands out. If
# something already holds it the next free id down is used instead.
LOGIN_GREETER_UID="${LOGIN_GREETER_UID:-972}"

# The wallpaper. A fifth repository: `ravencanvasd`, a wlr-layer-shell client
# that puts one background-layer surface on each output, and `ravencanvas`, the
# CLI that talks to its control socket.
#
# WHY IT IS HERE AND NOT IN THE RAVEN LAYER
#
# It would qualify. Both binaries link libc, libm and libgcc_s and nothing
# else -- its Wayland client is wayland-rs's pure-Rust backend, so there is no
# libwayland-client in the link graph and `--as-needed` drops the libxkbcommon
# a keyboard-less client never calls into. That is the Raven layer's whole
# invariant, met.
#
# It is here anyway, because linkage is not the only thing that stage owns.
# Everything this daemon needs from the image is written by this stage and by
# nothing else: /usr/share/wallpaper/set, which is also what the greeter reads,
# and raven-wayland-session, which is where a session client gets started from.
# The Raven stage runs first, so a wallpaper staged there would be wired into a
# launcher that this stage then overwrites wholesale.
#
# It is also optional in the file manager's sense rather than the terminal's:
# huginn paints its own background under everything, so an image without this
# is a desktop with a flat colour behind it. Every failure path below warns and
# returns 0.
CANVAS_REPO="RavenCanvas"
CANVAS_URL="https://github.com/javanhut/${CANVAS_REPO}.git"

# RoostBar is a layer-shell status bar started through the machine-wide
# session.d mechanism. It is kept separate from huginn so a failed optional
# module can never take down the compositor.
ROOSTBAR_REPO="RoostBar"
ROOSTBAR_URL="https://github.com/javanhut/${ROOSTBAR_REPO}.git"
ROOSTBAR_BIN="roostbar"

# Huginn links glibc, unlike everything in the Raven layer. The host target is
# the right one: the sysroot carries the host's glibc and its dynamic linker,
# which is how the Xorg and Mesa binaries stage2 copies already work.
GUI_TARGET="${GUI_TARGET:-$(rustc -vV 2>/dev/null | awk '/^host:/{print $2}')}"

# key|package|binary|description
GUI_COMPONENTS=(
    "huginn-comp|huginn-comp|huginn|Huginn - Wayland compositor, and the shell it draws"
)

# Every binary this stage ships, derived from the table above rather than
# written out again: the two lists drifted apart once already, and a hardcoded
# copy is wrong the moment a component is added or dropped.
declare -a GUI_ALL_BINARIES=()
for _spec in "${GUI_COMPONENTS[@]}"; do
    IFS='|' read -r _ _ _binary _ <<< "${_spec}"
    GUI_ALL_BINARIES+=("${_binary}")
done
unset _spec _binary

declare -a GUI_BUILT=()
declare -a GUI_FAILED=()

# =============================================================================
# Logging (use shared library or define fallbacks)
# =============================================================================
if [[ -f "${PROJECT_ROOT}/scripts/lib/logging.sh" ]]; then
    # shellcheck disable=SC1091
    source "${PROJECT_ROOT}/scripts/lib/logging.sh"
else
    log_info()    { echo "[INFO] $*"; }
    log_warn()    { echo "[WARN] $*" >&2; }
    log_error()   { echo "[ERROR] $*" >&2; }
    log_success() { echo "[SUCCESS] $*"; }
    log_step()    { echo ""; echo "==> $*"; }
fi

# =============================================================================
# Toolchain check
# =============================================================================
# Unlike the Raven stage there is no musl target to probe. What this needs is
# the development side of the libraries smithay binds: a build host without
# libdrm or libinput headers fails at link time, not at run time.
have_gui_toolchain() {
    command -v cargo &>/dev/null || return 1
    command -v pkg-config &>/dev/null || return 1

    local -a missing=()
    local lib

    # How a dependency has to be checked is decided by how its crate finds it,
    # not by what feels thorough. Getting that backwards makes this check
    # disagree with the build, which is the one thing it must never do: too
    # strict and the ISO loses its compositor on a host that could have built
    # it, too lax and the "skip cleanly" path never runs and the build dies at
    # link time instead.
    #
    # pkg-config group -- these reach the library through a build script that
    # probes pkg-config, so a .pc file is the requirement and the .so alone is
    # not enough. Module names are pkg-config's, which is not always the
    # soname: Mesa's GBM ships gbm.pc, not libgbm.pc.
    #
    # libseat belongs here, and used to be exempted. libseat-sys's build.rs is
    # `pkg_config::probe_library("libseat").unwrap()` on both of its cfg paths,
    # with no fallback -- so a host with libseat.so and no libseat.pc passed
    # this check and then failed the build with
    #
    #   called `Result::unwrap()` on an `Err` value: pkg-config exited with
    #   status code 1 ... Package 'libseat' was not found
    #
    # which is exactly the failure this function exists to turn into a clean
    # skip. The old comment said libseat "has no .pc on every distribution";
    # that is true, and the consequence is that those hosts cannot build the
    # compositor at all, not that the check should wave them through.
    for lib in libdrm libinput libudev gbm egl libseat; do
        pkg-config --exists "${lib}" 2>/dev/null || missing+=("${lib}")
    done

    # Link-name group -- the xkbcommon crate is 0.8.0, which carries no
    # build.rs at all and reaches the library through `#[link(name =
    # "xkbcommon")]`. That resolves as a plain -lxkbcommon at link time, so
    # what it needs is the libxkbcommon.so development symlink and a .pc file
    # is irrelevant. It was not checked at all before, so a host missing it got
    # no warning here and a bare linker error several minutes into the build.
    local found dir
    found=0
    for dir in /usr/lib /usr/lib64 /usr/lib/x86_64-linux-gnu; do
        [[ -e "${dir}/libxkbcommon.so" ]] && { found=1; break; }
    done
    (( found == 0 )) && missing+=("libxkbcommon")

    if (( ${#missing[@]} > 0 )); then
        log_warn "Missing build dependencies: ${missing[*]}"
        log_info "  install them with: pacman -S --needed libdrm libinput systemd mesa seatd libxkbcommon"
        return 1
    fi
    return 0
}

# =============================================================================
# Source fetching
# =============================================================================
# Clone or update one repository into `dest`.
#
# Generalised from what was fetch_gui_source when the terminal became a second
# repository this stage builds. One implementation rather than two near-copies:
# the offline handling, the "update failed, use what is there" fallback and the
# branch-then-default clone retry are all easy to get subtly different, and a
# terminal that silently built from a stale checkout would look like a code bug.
#
#   fetch_repo <name> <url> <dest> [ref] [offline]
fetch_repo() {
    local name="$1" url="$2" dest="$3" ref="${4:-}" offline="${5:-0}"
    local parent
    parent="$(dirname "${dest}")"

    if [[ "${offline}" == "1" ]]; then
        if [[ -d "${dest}/.git" ]]; then
            log_info "  offline: using existing clone of ${name}"
            return 0
        fi
        log_warn "  offline: no clone of ${name} in ${parent}"
        return 1
    fi

    command -v git &>/dev/null || { log_warn "  git not found"; return 1; }
    mkdir -p "${parent}"

    if [[ -d "${dest}/.git" ]]; then
        log_info "  updating ${name}..."
        if ! (cd "${dest}" && git fetch --tags --depth 1 origin "${ref:-HEAD}" 2>/dev/null \
              && git reset --hard FETCH_HEAD >/dev/null 2>&1); then
            log_warn "  could not update ${name}, using the existing checkout"
        fi
    else
        log_info "  cloning ${name}..."
        rm -rf "${dest}"
        if [[ -n "${ref}" ]]; then
            git clone --depth 1 --branch "${ref}" -q "${url}" "${dest}" 2>/dev/null \
                || git clone --depth 1 -q "${url}" "${dest}" || return 1
        else
            git clone --depth 1 -q "${url}" "${dest}" || return 1
        fi
    fi

    local rev
    rev="$(cd "${dest}" && git rev-parse --short HEAD 2>/dev/null || echo unknown)"
    log_info "  ${name} @ ${rev}"
    return 0
}

fetch_gui_source() {
    fetch_repo "${GUI_REPO}" "${GUI_URL}" "${GUI_SRC_DIR}/${GUI_REPO}" \
        "${GUI_REF:-}" "${GUI_OFFLINE:-0}"
}

# =============================================================================
# Build
# =============================================================================
# One cargo invocation for both binaries: they share a workspace, a dependency
# graph and a target directory, and the committed Cargo.lock was resolved
# against the set.
build_gui_workspace() {
    local src="$1"

    local -a cargo_args=(build --release --target "${GUI_TARGET}")
    [[ -f "${src}/Cargo.lock" ]] && cargo_args+=(--locked)

    local spec key package binary desc
    for spec in "${GUI_COMPONENTS[@]}"; do
        IFS='|' read -r key package binary desc <<< "${spec}"
        cargo_args+=(-p "${package}")
    done

    (
        cd "${src}"
        cargo "${cargo_args[@]}" -j "${RAVEN_JOBS}"
    )
}

# =============================================================================
# Library staging
# =============================================================================
# Copy every shared object the built binaries resolve to that the sysroot does
# not already have.
#
# Resolved with ldd rather than a hand-written list, because the list is not
# knowable by reading the manifests: libinput alone drags in libwacom, which
# drags in lua. A list would be correct on the day it was written and silently
# wrong the first time a dependency changed.
stage_gui_libraries() {
    local -a binaries=("$@")

    command -v ldd &>/dev/null || {
        log_warn "  ldd not found, cannot resolve shared libraries"
        return 0
    }

    mkdir -p "${SYSROOT_DIR}/usr/lib"

    local copied=0 present=0
    local line lib base
    while read -r lib; do
        [[ -n "${lib}" && -f "${lib}" ]] || continue
        base="$(basename "${lib}")"

        # The dynamic loader is stage2's: it belongs at /lib64 and /lib, where
        # the ELF interpreter path points, and a copy in /usr/lib would be
        # dead weight at best.
        case "${base}" in
            ld-linux-*|ld-musl-*) continue ;;
        esac

        # Already staged by stage2/stage3? Leave it alone: those copies are
        # the ones the rest of the system was linked against.
        if [[ -e "${SYSROOT_DIR}/usr/lib/${base}" ]]; then
            present=$((present + 1))
            continue
        fi

        # -L to dereference: the sysroot wants the real object, not a symlink
        # into a host path that will not exist at run time.
        if cp -L "${lib}" "${SYSROOT_DIR}/usr/lib/${base}" 2>/dev/null; then
            copied=$((copied + 1))
            log_info "    + ${base}"
        fi
    done < <(
        for b in "${binaries[@]}"; do
            ldd "${b}" 2>/dev/null | awk '/=> \//{print $3}'
        done | sort -u
    )

    log_info "  ${copied} library(ies) staged, ${present} already present"
}

# =============================================================================
# XWayland
# =============================================================================
# The X server that lets X11-only applications run under a Wayland compositor.
# Huginn implements the window-manager half (smithay's XwmHandler); this stages
# the server binary it drives.
#
# Both halves are required and neither is useful alone. Without the binary the
# compositor logs "XWayland unavailable" once at startup and runs Wayland-only;
# without the compositor support the binary is never executed at all. That is
# why this lives in the GUI stage next to the compositor rather than with the
# general packages: they ship or fail together.
#
# stage2 already stages the two things XWayland needs beyond its libraries --
# /usr/bin/xkbcomp, which it forks to compile keymaps, and /usr/share/X11/xkb,
# the data xkbcomp reads. See the comment at stage2-native.sh:334.
stage_xwayland() {
    local src
    # `|| true` is load-bearing under `set -euo pipefail`: when Xwayland is not
    # installed, `command -v` exits 1, the assignment takes that status, and the
    # whole stage dies before reaching the check below. The graceful path is
    # unreachable without it -- which is exactly how this first failed, with
    # "[STEP] Staging XWayland..." and nothing after it.
    src="$(command -v Xwayland 2>/dev/null || true)"

    if [[ -z "${src}" ]]; then
        log_warn "  Xwayland not found on the build host; X11 apps will not run"
        log_warn "  install it with: pacman -S --needed xorg-xwayland"
        return 0
    fi

    install -m 0755 "${src}" "${SYSROOT_DIR}/usr/bin/Xwayland"
    log_success "  Xwayland staged ($(du -h "${src}" | cut -f1))"

    # Its shared libraries, through the same resolver the compositor's use --
    # which skips anything stage2/stage3 already placed, so this adds only the
    # X-specific ones (libxcvt, libxshmfence, libei and friends).
    stage_gui_libraries "${src}"

    # Belt and braces: xkbcomp is stage2's job, but a GUI-only rebuild
    # (`imlazy gui` against an older sysroot) would not have run it, and the
    # failure mode is a keyboard that does nothing in every X11 window.
    if [[ ! -x "${SYSROOT_DIR}/usr/bin/xkbcomp" && -x /usr/bin/xkbcomp ]]; then
        install -m 0755 /usr/bin/xkbcomp "${SYSROOT_DIR}/usr/bin/xkbcomp"
        log_info "    + xkbcomp (keymap compiler for Xwayland)"
    fi
}

# =============================================================================
# =============================================================================
# The terminal
# =============================================================================
# Huginn names its terminal in exactly two places, both compiled in and neither
# overridable: `theme::TERMINAL` is what the Super+Shift+T binding spawns, and
# `dock::PINNED` names it first of the two applications pinned to the dock out
# of the box. Both say `raven-terminal`.
#
# Nothing built it. RavenTerminal was listed in REPOSFORRAVEN.md as "not wired"
# and in ARCHITECTURE.md as waiting on a display server that did not exist, and
# this stage built only the compositor -- so the desktop shipped with a
# launcher enumerating an empty /usr/share/applications, a dock that drew
# nothing but its own launcher button, and no terminal anywhere on the image. A
# running desktop could start no process at all.
#
# The dock stayed *quiet* about it, which is the part worth remembering:
# `dock::items` pushes an item only where `position` finds a matching entry, so
# a pinned name resolving to nothing is skipped rather than drawn dead. There
# was no broken icon to click and no error anywhere -- just a desktop that
# could not start a program. Hence the stage summary at the bottom of this
# file.
#
# The display server it was waiting for is the one this stage builds, so this is
# where it goes -- REPOSFORRAVEN.md said as much before it was true.
#
# WHY IT CANNOT LIVE IN THE RAVEN STAGE
#
# Every component in stage-raven.sh is CGO_ENABLED=0 and static. RavenTerminal
# is Go too, but it renders through OpenGL 4.1 bound by cgo (go-gl/gl) with GLFW
# compiled from C source in-tree, so it links libwayland-client, libwayland-egl,
# libxkbcommon and libGL. That is the same reason huginn is here, and the same
# reason neither can be dropped from the sysroot's link graph for free.
#
# WHY -tags wayland IS PASSED RATHER THAN LEFT TO ITS MAKEFILE
#
# RavenTerminal's Makefile picks its backend with `BACKEND ?= auto`, which reads
# $XDG_SESSION_TYPE and falls back to x11 when it is unset. It is always unset
# in a container build, so the Makefile would silently produce the X11 backend
# and the terminal would run under XWayland on a machine whose compositor is
# Wayland -- working, but through a translation layer, and broken outright on an
# image where stage_xwayland found no Xwayland to stage. The target is known
# here, so it is stated: `go build -tags wayland`, which is what selects
# -D_GLFW_WAYLAND and links wayland-client/cursor/egl instead of the X11 set.
#
# The xdg-shell and viewporter protocol sources are pre-generated and vendored
# in third_party/glfw, so this needs no wayland-scanner and no wayland-protocols
# on the build host -- only the libraries and headers the Dockerfile already
# installs for huginn.
#
# Environment:
#   TERMINAL_SKIP=1       skip the terminal; the desktop ships unusable
#   TERMINAL_OFFLINE=1    never touch the network; use the existing clone
#   TERMINAL_REF=<ref>    build a particular ref instead of the default
stage_terminal() {
    if [[ "${TERMINAL_SKIP:-0}" == "1" ]]; then
        log_warn "  TERMINAL_SKIP=1: the desktop will have no way to launch anything"
        return 0
    fi

    command -v go &>/dev/null || {
        log_warn "  go not found on the build host; RavenTerminal will not be built"
        log_warn "  and the desktop will have no terminal. Install it with:"
        log_warn "    pacman -S --needed go"
        return 0
    }

    # Probed before building for the same reason have_gui_toolchain exists: cgo
    # reports a missing header as
    #
    #   ./glfw/src/wl_platform.h:27:10: fatal error: wayland-client.h:
    #   No such file or directory
    #
    # a hundred lines into a compile, which reads like a broken vendored GLFW
    # rather than a build host missing a package. `gl` is in the list because
    # go-gl/gl's generated bindings run `pkg-config --cflags -- gl`, so libGL's
    # .pc file is a build requirement and not only a link-time one -- on Arch it
    # comes from libglvnd, which arrives as a dependency of mesa.
    local -a missing=()
    local mod
    for mod in wayland-client wayland-cursor wayland-egl xkbcommon gl; do
        pkg-config --exists "${mod}" 2>/dev/null || missing+=("${mod}")
    done
    if (( ${#missing[@]} > 0 )); then
        log_warn "  missing build dependencies for the terminal: ${missing[*]}"
        log_warn "  install them with: pacman -S --needed wayland libxkbcommon mesa"
        log_warn "  the desktop will have no terminal and can launch nothing"
        return 0
    fi

    local dest="${GUI_SRC_DIR}/${TERMINAL_REPO}"
    if ! fetch_repo "${TERMINAL_REPO}" "${TERMINAL_URL}" "${dest}" \
            "${TERMINAL_REF:-}" "${TERMINAL_OFFLINE:-${GUI_OFFLINE:-0}}"; then
        log_warn "  RavenTerminal source unavailable; the desktop will have no terminal"
        return 0
    fi

    # cgo is the whole point here, so it is set rather than inherited: the Raven
    # stage exports CGO_ENABLED=0 for its static builds, and a leaked 0 turns
    # this into a link failure against -lwayland-client rather than anything
    # that names cgo.
    log_info "  building RavenTerminal (wayland backend)..."
    if ! (
        cd "${dest}"
        CGO_ENABLED=1 go build -tags wayland -o raven-terminal ./src
    ); then
        log_warn "  RavenTerminal build failed; the desktop will have no terminal"
        return 0
    fi

    local out="${dest}/raven-terminal"
    if [[ ! -x "${out}" ]]; then
        log_warn "  RavenTerminal produced no binary; the desktop will have no terminal"
        return 0
    fi

    install -m 0755 "${out}" "${SYSROOT_DIR}/usr/bin/raven-terminal"
    log_success "  raven-terminal installed ($(du -h "${out}" | cut -f1))"

    # Through the same resolver as the compositor's libraries, so this adds only
    # what stage2/stage3 did not already place.
    stage_gui_libraries "${out}"

    # Its icon, so the dock and launcher have something to draw. The binary
    # embeds this SVG for its own window icon; the desktop needs it on disk,
    # under the name the .desktop file's Icon= key asks for. hicolor rather than
    # breeze-dark because this is an application shipping its own icon, which is
    # exactly what hicolor is the theme for.
    local icon="${dest}/src/assets/raven_terminal_icon.svg"
    if [[ -f "${icon}" ]]; then
        install -Dm 0644 "${icon}" \
            "${SYSROOT_DIR}/usr/share/icons/hicolor/scalable/apps/raven-terminal.svg"
        log_info "    + raven-terminal.svg (hicolor/scalable)"
    else
        log_warn "    no icon in the checkout; the dock entry will draw blank"
    fi

    # No config is written. RavenTerminal reads ~/.config/raven-terminal at
    # runtime and creates it when absent, its fonts are embedded in the binary
    # (JetBrains Mono, FiraCode, Hack and Ubuntu Mono Nerd Fonts), and it
    # advertises TERM=xterm-256color, which copy_terminfo() in stage2 already
    # stages. There is nothing left for the image to provide.
}

# =============================================================================
# The GTK runtime
# =============================================================================
# Everything a GTK4/libadwaita application needs that is not a shared library,
# and therefore everything `ldd` cannot find for us.
#
# This exists because RavenFileManager is the first GTK application on the
# image. huginn needed none of it -- it draws with smithay and Mesa directly --
# so the sysroot has a complete graphics stack and none of the *data* a toolkit
# reads at run time. Each item below fails quietly and separately, which is the
# whole problem with them:
#
#   - No `gschemas.compiled` and every `g_settings_new()` is a fatal GLib
#     error: "Settings schema 'org.gtk.gtk4.Settings.FileChooser' is not
#     installed". The XML alone does nothing -- GSettings reads only the
#     compiled cache -- and the build host has the same gap today, which is why
#     `gsettings list-schemas` on a running Raven desktop prints "No schemas
#     installed".
#   - No `/usr/share/mime/mime.cache` and `g_file_info_get_content_type()`
#     answers application/octet-stream for every file. A file manager that
#     cannot tell a directory from a JPEG still runs; it just has one icon and
#     no "Open With".
#   - No glycin loaders and GTK4 decodes no images at all. Since gdk-pixbuf
#     2.44 the loaders moved out of process into glycin, which sandboxes each
#     decoder with bubblewrap -- so `bwrap` is a runtime dependency of drawing
#     a PNG, which is not a sentence anyone expects to be true.
#   - No GIO modules and TLS is unavailable to anything using GIO streams.
#
# Everything here is copied from the build host, like stage2's libraries, and
# the caches are regenerated against the sysroot rather than copied: a cache
# built on the host encodes host paths, and the three tools all take the
# directory to work on as an argument, so no chroot is needed.
stage_gtk_runtime() {
    local staged=0

    # GSettings schemas. gsettings-desktop-schemas is in here too and is not
    # optional: libadwaita's AdwStyleManager reads org.gnome.desktop.interface
    # to decide light or dark, and resolves to light with a warning when the
    # schema is absent -- an application that ignores the system theme looks
    # like an application bug.
    local schemas="/usr/share/glib-2.0/schemas"
    if [[ -d "${schemas}" ]]; then
        mkdir -p "${SYSROOT_DIR}${schemas}"
        cp -a "${schemas}/." "${SYSROOT_DIR}${schemas}/" 2>/dev/null || true
        if command -v glib-compile-schemas &>/dev/null; then
            if glib-compile-schemas "${SYSROOT_DIR}${schemas}" 2>/dev/null; then
                log_info "  compiled GSettings schemas ($(find "${SYSROOT_DIR}${schemas}" -name '*.gschema.xml' | wc -l) schema(s))"
                staged=$((staged + 1))
            else
                log_warn "  glib-compile-schemas failed; GTK applications will abort on startup"
            fi
        else
            log_warn "  glib-compile-schemas not on the build host: the schemas are"
            log_warn "  staged but not compiled, which is the same as not staged"
            log_warn "  install it with: pacman -S --needed glib2-devel"
        fi
    else
        log_warn "  no GSettings schemas on the build host; GTK applications will abort"
    fi

    # The MIME database. Copied and rebuilt rather than copied with its cache,
    # because mime.cache is a memory-mapped binary whose layout is tied to the
    # tool that wrote it.
    if [[ -d /usr/share/mime ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/share/mime"
        cp -a /usr/share/mime/. "${SYSROOT_DIR}/usr/share/mime/" 2>/dev/null || true
        if command -v update-mime-database &>/dev/null; then
            update-mime-database "${SYSROOT_DIR}/usr/share/mime" 2>/dev/null || true
            log_info "  staged the MIME database"
            staged=$((staged + 1))
        fi
    fi

    # Image decoding. glycin-loaders are separate executables the toolkit
    # spawns, so they carry their own link graph -- resolved through the same
    # ldd path as everything else rather than assumed to be a subset of the
    # application's.
    if [[ -d /usr/lib/glycin-loaders ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/lib/glycin-loaders"
        cp -a /usr/lib/glycin-loaders/. "${SYSROOT_DIR}/usr/lib/glycin-loaders/" 2>/dev/null || true

        local -a loaders=()
        while IFS= read -r loader; do
            loaders+=("${loader}")
        done < <(find /usr/lib/glycin-loaders -type f -perm -u+x 2>/dev/null)
        (( ${#loaders[@]} > 0 )) && stage_gui_libraries "${loaders[@]}"

        # The sandbox. glycin refuses to decode outside one, so a missing bwrap
        # is a toolkit that draws no images rather than a toolkit that draws
        # them unsandboxed.
        local bwrap
        bwrap="$(command -v bwrap 2>/dev/null || true)"
        if [[ -n "${bwrap}" ]]; then
            # 0755, and deliberately not setuid. bubblewrap has two build
            # modes: setuid-root for kernels without unprivileged user
            # namespaces, and unprivileged for kernels with them. The host's is
            # 0755, so it is the unprivileged build -- and installing that one
            # 4755 would not make it work on a kernel that needs the other, it
            # would only add a setuid-root binary to the image for no reason.
            # RavenLinux's kernel enables user namespaces; if that ever changes
            # this is the line that has to change with it.
            install -m 0755 "${bwrap}" "${SYSROOT_DIR}/usr/bin/bwrap"
            stage_gui_libraries "${bwrap}"
            log_info "  staged glycin loaders and bwrap"
            staged=$((staged + 1))
        else
            log_warn "  bwrap not on the build host: glycin sandboxes every decode and"
            log_warn "  refuses to run without it, so GTK will display no images"
            log_warn "  install it with: pacman -S --needed bubblewrap"
        fi
    fi

    # gdk-pixbuf's own loaders, for the gdk-pixbuf 2.42-era layout. Absent on a
    # host with gdk-pixbuf 2.44+, where the loaders moved to glycin above --
    # hence the guard rather than a warning. The cache is regenerated with the
    # sysroot prefix stripped, because it stores absolute module paths and the
    # ones the host tool writes would name the build machine.
    local pixbuf="/usr/lib/gdk-pixbuf-2.0/2.10.0"
    if [[ -d "${pixbuf}/loaders" ]]; then
        mkdir -p "${SYSROOT_DIR}${pixbuf}"
        cp -a "${pixbuf}/." "${SYSROOT_DIR}${pixbuf}/" 2>/dev/null || true
        if command -v gdk-pixbuf-query-loaders &>/dev/null; then
            GDK_PIXBUF_MODULEDIR="${SYSROOT_DIR}${pixbuf}/loaders" \
                gdk-pixbuf-query-loaders 2>/dev/null \
                | sed "s|${SYSROOT_DIR}||g" \
                > "${SYSROOT_DIR}${pixbuf}/loaders.cache" || true
            log_info "  staged gdk-pixbuf loaders"
            staged=$((staged + 1))
        fi
    fi

    # GIO modules: the TLS backend, the proxy resolvers, and the dconf GSettings
    # backend. Nothing here is fatal on its own -- GSettings falls back to the
    # in-memory backend when dconf is unreachable, which means settings apply
    # for the life of the process and are forgotten on exit.
    if [[ -d /usr/lib/gio/modules ]]; then
        mkdir -p "${SYSROOT_DIR}/usr/lib/gio/modules"
        cp -a /usr/lib/gio/modules/. "${SYSROOT_DIR}/usr/lib/gio/modules/" 2>/dev/null || true
        local -a giomods=()
        while IFS= read -r m; do giomods+=("${m}"); done \
            < <(find /usr/lib/gio/modules -name '*.so' 2>/dev/null)
        (( ${#giomods[@]} > 0 )) && stage_gui_libraries "${giomods[@]}"
        log_info "  staged GIO modules"
        staged=$((staged + 1))
    fi

    log_info "  ${staged} GTK runtime component(s) staged"
}

# =============================================================================
# RavenFileManager
# =============================================================================
# The desktop's file manager: a GTK4/libadwaita application from its own
# repository, built here for the same reason the terminal is -- it is a Wayland
# client of the compositor this stage produces, and it links a graphics stack
# that only exists from this stage onward.
#
# WHY IT IS NOT IN THE RAVEN LAYER, AND NOT LIKE THE OTHER TWO EITHER
#
# The Raven layer's invariant is a static binary that adds nothing to the
# sysroot's link graph. huginn broke that with seventeen shared libraries and
# got this stage. RavenFileManager links **a hundred and thirty-four**: GTK4,
# libadwaita, pango, cairo, harfbuzz, GStreamer, appstream, krb5, gnutls and
# the rest of the desktop stack behind them.
#
# That number is the reason every failure path here returns 0. huginn and
# raven-terminal are the desktop -- without either there is nothing to log into
# and nothing to launch. The file manager is an application: an image that
# builds without it is a working desktop that is missing a program, and that is
# not worth failing an ISO over.
#
# WHY PREFIX=/usr AND NOT ITS MAKEFILE'S DEFAULT
#
# Its Makefile installs to /usr/local, which is where it currently sits on a
# development machine and is correct there. The image is not that: /usr/local
# on the ISO is an empty tree the package manager does not own, and although
# raven-wayland-session exports XDG_DATA_DIRS=/usr/local/share:/usr/share -- so
# the launcher would find the entry either way -- the binary and its data belong
# with everything else the build produced. `make install` is not used at all:
# it would run update-desktop-database and gtk-update-icon-cache against the
# build host's live tree, and the file list is short enough to state.
#
# Environment:
#   FILEMANAGER_SKIP=1      skip it; the desktop is complete without it
#   FILEMANAGER_OFFLINE=1   never touch the network; use the existing clone
#   FILEMANAGER_REF=<ref>   build a particular ref instead of the default
stage_filemanager() {
    if [[ "${FILEMANAGER_SKIP:-0}" == "1" ]]; then
        log_info "  FILEMANAGER_SKIP=1: no file manager on the image"
        return 0
    fi

    command -v cargo &>/dev/null || {
        log_warn "  cargo not found; RavenFileManager will not be built"
        return 0
    }

    # Probed rather than discovered in the middle of a link, for the same
    # reason the terminal's are: gtk4-rs reports a missing gtk4.pc as a
    # system-deps panic in a build script, which names neither GTK nor the
    # package to install.
    local -a missing=()
    local mod
    for mod in gtk4 libadwaita-1 glib-2.0 gio-2.0; do
        pkg-config --exists "${mod}" 2>/dev/null || missing+=("${mod}")
    done
    if (( ${#missing[@]} > 0 )); then
        log_warn "  missing build dependencies for the file manager: ${missing[*]}"
        log_warn "  install them with: pacman -S --needed gtk4 libadwaita"
        log_warn "  the desktop will ship without a file manager"
        return 0
    fi

    local dest="${GUI_SRC_DIR}/${FILEMANAGER_REPO}"
    if ! fetch_repo "${FILEMANAGER_REPO}" "${FILEMANAGER_URL}" "${dest}" \
            "${FILEMANAGER_REF:-}" "${FILEMANAGER_OFFLINE:-${GUI_OFFLINE:-0}}"; then
        log_warn "  RavenFileManager source unavailable; no file manager on the image"
        return 0
    fi

    # The host target, like huginn and for the same reason: this links glibc,
    # and the sysroot carries the host's glibc and dynamic linker. CGO_ENABLED
    # is unset here rather than inherited -- the Raven stage exports 0 for its
    # static Go builds and it means nothing to cargo, but leaving it exported
    # into a build that shells out to a C compiler has bitten this stage once
    # already.
    log_info "  building RavenFileManager for ${GUI_TARGET}..."
    local -a cargo_args=(build --release --target "${GUI_TARGET}")
    [[ -f "${dest}/Cargo.lock" ]] && cargo_args+=(--locked)
    if ! (
        cd "${dest}"
        unset CGO_ENABLED
        cargo "${cargo_args[@]}" -j "${RAVEN_JOBS}"
    ); then
        log_warn "  RavenFileManager build failed; no file manager on the image"
        return 0
    fi

    local out="${dest}/target/${GUI_TARGET}/release/${FILEMANAGER_BIN}"
    if [[ ! -x "${out}" ]]; then
        log_warn "  RavenFileManager produced no binary; no file manager on the image"
        return 0
    fi

    install -Dm 0755 "${out}" "${SYSROOT_DIR}/usr/bin/${FILEMANAGER_BIN}"
    log_success "  ${FILEMANAGER_BIN} installed ($(du -h "${out}" | cut -f1))"

    stage_gui_libraries "${out}"

    # Its data. The paths mirror `make install` with PREFIX=/usr; the desktop
    # entry is deliberately not among them -- install_desktop_entries() writes
    # every entry on this image, so that what the launcher shows is decided in
    # one place.
    local appdata="${SYSROOT_DIR}/usr/share"
    install -Dm 0644 "${dest}/data/icons/hicolor/scalable/apps/${FILEMANAGER_APPID}.svg" \
        "${appdata}/icons/hicolor/scalable/apps/${FILEMANAGER_APPID}.svg" 2>/dev/null \
        && log_info "    + ${FILEMANAGER_APPID}.svg (hicolor/scalable)" \
        || log_warn "    no icon in the checkout; the dock entry will draw blank"

    install -Dm 0644 "${dest}/data/${FILEMANAGER_APPID}.metainfo.xml" \
        "${appdata}/metainfo/${FILEMANAGER_APPID}.metainfo.xml" 2>/dev/null || true

    # Its default config, shipped as reference and not as configuration.
    #
    # Worth being exact about, because the path looks load-bearing and is not.
    # `AppConfig::config_dir()` in raven-core resolves $XDG_CONFIG_HOME/raven,
    # or ~/.config/raven, and there is no system fallback below it -- and what
    # it reads there is `config.toml`, not `default.toml`. So nothing on a
    # running system ever opens this file. It is here for the same reason
    # upstream's `make install` places it: it is the documented default set,
    # and the answer to "what can I put in ~/.config/raven/config.toml".
    #
    # data/resources/style.css is deliberately not among them: raven-ui
    # include_str!s it at ../../../data/resources/style.css, so the stylesheet
    # is compiled into the binary and a copy on disk would be a second one that
    # can disagree with what is actually being drawn.
    local f
    for f in default keybindings actions; do
        install -Dm 0644 "${dest}/config/${f}.toml" \
            "${appdata}/${FILEMANAGER_BIN}/config/${f}.toml" 2>/dev/null || true
    done
    log_info "    + default config under /usr/share/${FILEMANAGER_BIN}/config (reference)"

    # Only now, and only on the success path: none of it is worth carrying on
    # an image with nothing that reads it, and it is not a small amount --
    # against an otherwise empty sysroot the schemas, the MIME database, the
    # glycin loaders and their libraries come to a little over 100MB.
    log_step "Staging the GTK runtime..."
    stage_gtk_runtime
}

# =============================================================================
# The application menu
# =============================================================================
# `launcher::scan_applications()` reads $XDG_DATA_DIRS/applications, and
# `dock::items` resolves its pinned names against the same list.
# scripts/lib/skeleton.sh creates /usr/share/applications and nothing ever wrote
# a file into it, so both came up empty: the launcher enumerated zero
# applications and the dock's one pinned entry matched nothing.
#
# WHY THESE ARE WRITTEN HERE RATHER THAN SHIPPED BY EACH PACKAGE
#
# They describe how the *desktop* starts things, which is a fact about the
# desktop and not about crow. crow is a terminal program: its entry is correct
# only because there is a terminal to wrap it in, and that is true only from
# this stage onward. Writing them where the terminal is staged keeps the two
# from drifting apart.
#
# WHY TUI PROGRAMS ARE WRAPPED EXPLICITLY AND NOT MARKED Terminal=true
#
# `raven_desktop::Entry` parses `Terminal=` into a field that nothing reads:
# `Huginn::launch` hands `Entry::argv` straight to `Command::spawn` either way.
# A `Terminal=true` entry would therefore exec the bare TUI with no controlling
# terminal, which does not open a window and does not report an error -- it
# just silently does nothing. Wrapping in `raven-terminal -e` is what actually
# runs today. Setting the key as well, so the entry stays honest if huginn
# learns to read it, would then wrap it twice.
#
# ONLY ENTRIES THAT DO SOMETHING
#
# Deliberately not here: caw, rvn, ivaldi, imlazy and poxy. Every one is a
# subcommand CLI that prints usage and exits when run bare, so a menu entry for
# it is a window that flashes and closes. `raven-terminal -e` does not change
# that -- the problem is the shape of those programs, not the terminal's
# inability to run them. They are reached by typing their names, which is what
# they were designed for. A launcher listing things that appear to do nothing
# when clicked is worse than a shorter launcher.
# =============================================================================
# The login screen
# =============================================================================
# RavenLogin: `ravend`, which runs as root and reads /etc/shadow, and
# `raven-greeter`, which draws and can ask ravend exactly one question over a
# Unix socket. The split is the point of that repository -- the process that
# parses fonts and decodes images is not the process that can read password
# hashes -- and this stage keeps it by creating the unprivileged account the
# greeter needs before installing anything.
#
# Nothing here edits init.toml or the kernel cmdline. raven-init looks for
# `ravend` on the image and starts it in place of the autologin session when it
# finds one, so installing the binary is the whole of turning the login screen
# on, and LOGIN_SKIP=1 is the whole of leaving it off.
stage_login() {
    if [[ "${LOGIN_SKIP:-0}" == "1" ]]; then
        log_warn "  LOGIN_SKIP=1: the image will autologin with no password prompt"
        return 0
    fi

    local dest="${GUI_SRC_DIR}/${LOGIN_REPO}"
    if ! fetch_repo "${LOGIN_REPO}" "${LOGIN_URL}" "${dest}" \
            "${LOGIN_REF:-}" "${LOGIN_OFFLINE:-${GUI_OFFLINE:-0}}"; then
        log_warn "  RavenLogin source unavailable; the image will autologin"
        return 0
    fi

    log_info "  building RavenLogin for ${GUI_TARGET}..."
    local -a cargo_args=(
        build --release --target "${GUI_TARGET}"
        -p ravend -p raven-greeter -p raven-lock
    )
    [[ -f "${dest}/Cargo.lock" ]] && cargo_args+=(--locked)
    if ! ( cd "${dest}" && cargo "${cargo_args[@]}" -j "${RAVEN_JOBS}" ); then
        log_warn "  RavenLogin build failed; the image will autologin"
        return 0
    fi

    # All or none. ravend without a greeter is a daemon that starts a
    # compositor and then cannot draw a prompt in it, which presents as a
    # machine that boots to a blank screen -- strictly worse than the autologin
    # session it would have replaced. raven-lock is in the same list for a
    # narrower reason: huginn blanks the session on resume and then starts it,
    # and a missing lock screen turns that into ten seconds of black display
    # before the compositor gives up and un-blanks.
    local built="${dest}/target/${GUI_TARGET}/release"
    local binary
    for binary in ravend raven-greeter raven-lock; do
        if [[ ! -x "${built}/${binary}" ]]; then
            log_warn "  ${binary} was not produced by the build; the image will autologin"
            return 0
        fi
    done

    login_greeter_account || {
        log_warn "  cannot create the greeter's account; not installing the login screen"
        return 0
    }

    for binary in ravend raven-greeter raven-lock; do
        install -m 0755 "${built}/${binary}" "${SYSROOT_DIR}/usr/bin/${binary}"
        log_success "  ${binary} installed ($(du -h "${built}/${binary}" | cut -f1))"
    done

    # Never overwritten: every value in it is a default ravend already carries,
    # so a file that is already there is one somebody edited on purpose.
    if [[ -f "${SYSROOT_DIR}/etc/raven/login.toml" ]]; then
        log_info "  /etc/raven/login.toml exists; leaving it alone"
    elif [[ -f "${dest}/config/login.toml" ]]; then
        install -Dm 0644 "${dest}/config/login.toml" "${SYSROOT_DIR}/etc/raven/login.toml"
        log_info "  + /etc/raven/login.toml"
    fi

    # The two drawing clients need their libraries resolved; ravend links
    # nothing but libc, which is the property the whole repository is arranged
    # around.
    stage_gui_libraries "${built}/raven-greeter"
    stage_gui_libraries "${built}/raven-lock"
    install_wallpaper_dirs
}

# The account the greeter runs as.
#
# Written into the sysroot's own files rather than with useradd, which would
# create the account on the build host. A system account: nologin shell, locked
# password, and membership of the groups that let its compositor work at all --
# video and render for the GPU, input for the keyboard, seat for seatd. A
# greeter missing `video` starts, draws nothing, and looks like a driver bug.
login_greeter_account() {
    local passwd="${SYSROOT_DIR}/etc/passwd"
    local group="${SYSROOT_DIR}/etc/group"
    local shadow="${SYSROOT_DIR}/etc/shadow"
    [[ -f "${passwd}" && -f "${group}" ]] || { log_warn "  no /etc/passwd in the sysroot"; return 1; }

    if grep -q "^${LOGIN_GREETER_USER}:" "${passwd}"; then
        log_info "  account ${LOGIN_GREETER_USER} already in the sysroot"
    else
        # The configured id unless somebody holds it, then the next free one
        # down. Searching downwards stays inside the system range whatever
        # happens; searching up would eventually collide with a real user.
        local uid="${LOGIN_GREETER_UID}"
        while awk -F: -v id="${uid}" '$3 == id { found = 1 } END { exit !found }' "${passwd}"; do
            uid=$(( uid - 1 ))
            if (( uid < 900 )); then
                log_warn "  no free system uid below ${LOGIN_GREETER_UID}"
                return 1
            fi
        done
        [[ "${uid}" == "${LOGIN_GREETER_UID}" ]] \
            || log_info "  uid ${LOGIN_GREETER_UID} is taken; using ${uid}"

        printf '%s:x:%s:\n' "${LOGIN_GREETER_USER}" "${uid}" >> "${group}"
        printf '%s:x:%s:%s:Raven login screen:%s:/usr/bin/nologin\n' \
            "${LOGIN_GREETER_USER}" "${uid}" "${uid}" "${LOGIN_GREETER_HOME}" >> "${passwd}"
        # '!' and not '': an empty field is a password of no characters, and
        # RavenLogin's own policy calls that an account that can be logged in
        # to. Locked is what is meant.
        [[ -f "${shadow}" ]] && printf '%s:!:19000:0:99999:7:::\n' "${LOGIN_GREETER_USER}" >> "${shadow}"
        log_info "  + ${LOGIN_GREETER_USER} (uid ${uid}, nologin, locked)"
    fi

    local g
    for g in video render input seat; do
        if ! grep -q "^${g}:" "${group}"; then
            log_warn "  group '${g}' is not in the sysroot; the greeter may not reach the"
            log_warn "  GPU, the keyboard or the seat. Check this first if it draws nothing."
            continue
        fi
        # Append to the member list, and only once: this stage runs again on
        # every rebuild into the same sysroot.
        awk -F: -v user="${LOGIN_GREETER_USER}" -v want="${g}" '
            BEGIN { OFS = ":" }
            $1 == want {
                n = split($4, members, ",")
                for (i = 1; i <= n; i++) if (members[i] == user) { print; next }
                $4 = ($4 == "" ? user : $4 "," user)
            }
            { print }
        ' "${group}" > "${group}.new" && mv "${group}.new" "${group}"
    done
    log_info "  ${LOGIN_GREETER_USER} is in video, render, input, seat (where they exist)"

    install -d -m 0755 "${SYSROOT_DIR}${LOGIN_GREETER_HOME}"
    return 0
}

# =============================================================================
# The wallpaper
# =============================================================================
# RavenCanvas: `ravencanvasd`, a wlr-layer-shell client that draws the desktop
# background, `ravencanvas`, the CLI that reconfigures it over a control socket
# without a restart, and `raven-set-wallpaper`, which puts a picture in the
# machine's library and points set/ at it.
#
# Nothing here decides what the wallpaper *is*. install_wallpaper_dirs() leaves
# the library empty and the shipped canvas.toml has its `[background]`
# commented out on purpose, so a fresh image draws the daemon's built-in
# gradient and one `raven-set-wallpaper` on the running machine is the whole of
# changing it -- on the desktop and on the login screen at once, because both
# read that one directory.
#
# It is started from raven-wayland-session, not from init.toml. See the block
# install_session_launcher() writes for why, and note that this function does
# not write it: the launcher is one file written whole by that function, and
# the line it carries is guarded with `command -v` so that CANVAS_SKIP=1 needs
# no second version of it.
stage_canvas() {
    if [[ "${CANVAS_SKIP:-0}" == "1" ]]; then
        log_info "  CANVAS_SKIP=1: no wallpaper daemon; huginn's flat background"
        return 0
    fi

    command -v cargo &>/dev/null || {
        log_warn "  cargo not found; RavenCanvas will not be built"
        return 0
    }

    local dest="${GUI_SRC_DIR}/${CANVAS_REPO}"
    if ! fetch_repo "${CANVAS_REPO}" "${CANVAS_URL}" "${dest}" \
            "${CANVAS_REF:-}" "${CANVAS_OFFLINE:-${GUI_OFFLINE:-0}}"; then
        log_warn "  RavenCanvas source unavailable; no wallpaper daemon"
        return 0
    fi

    # The host target, like everything else this stage builds. This one would
    # take the musl target too -- see the note at CANVAS_REPO -- but the image
    # it ships into carries glibc for huginn's sake anyway, so a second target
    # here would buy nothing and cost a second toolchain check.
    log_info "  building RavenCanvas for ${GUI_TARGET}..."
    local -a cargo_args=(build --release --target "${GUI_TARGET}" -p ravencanvasd -p ravencanvas)
    [[ -f "${dest}/Cargo.lock" ]] && cargo_args+=(--locked)
    if ! ( cd "${dest}" && cargo "${cargo_args[@]}" -j "${RAVEN_JOBS}" ); then
        log_warn "  RavenCanvas build failed; no wallpaper daemon"
        return 0
    fi

    # Both or neither, on the pattern ravend and its greeter set. The CLI
    # without the daemon is a program whose every subcommand fails to connect,
    # and the daemon without the CLI is a wallpaper no user can change for
    # themselves -- `raven-set-wallpaper` sets the machine's, which is not the
    # same thing.
    local built="${dest}/target/${GUI_TARGET}/release"
    local binary
    for binary in ravencanvasd ravencanvas; do
        if [[ ! -x "${built}/${binary}" ]]; then
            log_warn "  ${binary} was not produced by the build; no wallpaper daemon"
            return 0
        fi
    done

    for binary in ravencanvasd ravencanvas; do
        install -m 0755 "${built}/${binary}" "${SYSROOT_DIR}/usr/bin/${binary}"
        log_success "  ${binary} installed ($(du -h "${built}/${binary}" | cut -f1))"
    done

    # Renamed on the way in, exactly as the upstream installer renames it. This
    # is the name canvas.toml, the daemon's README and RavenLogin's own
    # documentation all use; a `set-wallpaper.sh` in /usr/bin says nothing
    # about whose wallpaper it sets.
    if [[ -f "${dest}/scripts/set-wallpaper.sh" ]]; then
        install -m 0755 "${dest}/scripts/set-wallpaper.sh" \
            "${SYSROOT_DIR}/usr/bin/raven-set-wallpaper"
        log_info "  + /usr/bin/raven-set-wallpaper"
    else
        log_warn "  no set-wallpaper.sh in the checkout; the wallpaper must be set by hand"
    fi

    # Never overwritten, for the reason login.toml is not: every value in it is
    # a default the daemon already carries, so a file that is already there is
    # one somebody edited on purpose.
    #
    # What it ships with is the load-bearing part. `[background]` is commented
    # out, and that is not an omission: a `[background]` here overrides the
    # machine's wallpaper for every user on the machine, which is exactly the
    # contract with the login screen that install_wallpaper_dirs() exists to
    # keep. A file that is present but does not parse is not fatal here, unlike
    # ravend's -- the daemon warns and leaves what is on screen.
    if [[ -f "${SYSROOT_DIR}/etc/raven/canvas.toml" ]]; then
        log_info "  /etc/raven/canvas.toml exists; leaving it alone"
    elif [[ -f "${dest}/config/canvas.toml" ]]; then
        install -Dm 0644 "${dest}/config/canvas.toml" "${SYSROOT_DIR}/etc/raven/canvas.toml"
        log_info "  + /etc/raven/canvas.toml"
    else
        log_warn "  no config/canvas.toml in the checkout; the daemon's own defaults apply"
    fi

    # Resolved anyway, though it is expected to find nothing new: both binaries
    # link libc, libm and libgcc_s and no more. Running it is how that stays
    # true -- a dependency added upstream would otherwise be a wallpaper daemon
    # that installs cleanly and cannot start.
    stage_gui_libraries "${built}/ravencanvasd" "${built}/ravencanvas"

    # Called here as well as in stage_login(), and deliberately: either half can
    # be skipped, and the directories are the contract between them rather than
    # either one's property. Both upstream installers do the same.
    install_wallpaper_dirs
}

# =============================================================================
# RoostBar
# =============================================================================
stage_roostbar() {
    if [[ "${ROOSTBAR_SKIP:-0}" == "1" ]]; then
        log_info "  ROOSTBAR_SKIP=1: no status bar on the image"
        return 0
    fi

    command -v cargo &>/dev/null || {
        log_warn "  cargo not found; RoostBar will not be built"
        return 0
    }
    if ! pkg-config --exists alsa 2>/dev/null; then
        log_warn "  alsa-lib development files missing; RoostBar will not be built"
        log_warn "  install them with: pacman -S --needed alsa-lib"
        return 0
    fi

    local dest="${GUI_SRC_DIR}/${ROOSTBAR_REPO}"
    if ! fetch_repo "${ROOSTBAR_REPO}" "${ROOSTBAR_URL}" "${dest}" \
            "${ROOSTBAR_REF:-}" "${ROOSTBAR_OFFLINE:-${GUI_OFFLINE:-0}}"; then
        log_warn "  RoostBar source unavailable; no status bar on the image"
        return 0
    fi

    log_info "  building RoostBar for ${GUI_TARGET}..."
    local -a cargo_args=(build --release --target "${GUI_TARGET}")
    [[ -f "${dest}/Cargo.lock" ]] && cargo_args+=(--locked)
    if ! ( cd "${dest}" && cargo "${cargo_args[@]}" -j "${RAVEN_JOBS}" ); then
        log_warn "  RoostBar build failed; no status bar on the image"
        return 0
    fi

    local out="${dest}/target/${GUI_TARGET}/release/${ROOSTBAR_BIN}"
    if [[ ! -x "${out}" ]]; then
        log_warn "  RoostBar produced no binary; no status bar on the image"
        return 0
    fi

    install -Dm 0755 "${out}" "${SYSROOT_DIR}/usr/bin/${ROOSTBAR_BIN}"
    install -Dm 0755 "${PROJECT_ROOT}/configs/raven/session.d/50-roostbar" \
        "${SYSROOT_DIR}/etc/raven/session.d/50-roostbar"
    if [[ -f "${dest}/config.example.toml" ]]; then
        install -Dm 0644 "${dest}/config.example.toml" \
            "${SYSROOT_DIR}/usr/share/doc/roostbar/config.example.toml"
    fi
    stage_gui_libraries "${out}"
    log_success "  ${ROOSTBAR_BIN} installed and enabled for every graphical session"
}

# The machine's wallpaper library, and the one entry that says which image is
# on.
#
# Both are created empty and nothing is shipped into them: an image with no
# wallpaper draws the compositor's flat background and the greeter's backdrop,
# which is a desktop, and a photograph committed to a distribution repository
# is a licence question nobody asked for.
#
# The directories exist anyway because every reader is compiled to look here:
# huginn draws /usr/share/wallpaper/set/wallpaper.<ext> behind the desktop,
# raven-greeter draws the same file behind the login prompt, and ravencanvasd
# draws it over huginn's on the background layer -- which is what makes the
# three look like one machine. A directory that is already there is also the
# difference between "drop a file in" and "work out where it goes".
#
# ravencanvasd is the only one of the three that watches it, so on a running
# desktop the picture changes within a moment of `raven-set-wallpaper`; the
# other two read it once, at their own start.
install_wallpaper_dirs() {
    install -d -m 0755 "${SYSROOT_DIR}/usr/share/wallpaper"
    install -d -m 0755 "${SYSROOT_DIR}/usr/share/wallpaper/set"
    log_info "  + /usr/share/wallpaper and set/ (empty; drop an image in to use one)"
}

install_desktop_entries() {
    local dir="${SYSROOT_DIR}/usr/share/applications"
    mkdir -p "${dir}"

    local written=0

    # StartupWMClass matches the app_id RavenTerminal sets --
    # `glfw.WindowHintString(glfw.X11ClassName, "raven-terminal")` in
    # src/window/window.go, which its vendored GLFW patch also feeds to
    # xdg_toplevel_set_app_id on Wayland. That is how the dock knows a running
    # window belongs to this entry: `dock::owns` checks StartupWMClass first,
    # then falls back to the file stem, which happens to match here too.
    if [[ -x "${SYSROOT_DIR}/usr/bin/raven-terminal" ]]; then
        cat > "${dir}/raven-terminal.desktop" << 'ENTRY'
[Desktop Entry]
Type=Application
Name=Terminal
GenericName=Terminal Emulator
Comment=Run commands in a shell
Exec=raven-terminal
Icon=raven-terminal
Categories=System;TerminalEmulator;
Keywords=shell;prompt;command;console;
StartupWMClass=raven-terminal
Terminal=false
ENTRY
        chmod 0644 "${dir}/raven-terminal.desktop"
        written=$((written + 1))
        log_info "    + raven-terminal.desktop"

        # crow is a modal terminal editor, so its entry is launchable only
        # because the terminal above exists and takes `-e` -- hence the nesting.
        # %F is how a file dropped on the launcher reaches it; huginn's
        # `Entry::argv` drops the code entirely when there is no target, so the
        # bare case runs `raven-terminal -e crow` and opens an empty buffer.
        if [[ -x "${SYSROOT_DIR}/usr/bin/crow" ]]; then
            cat > "${dir}/crow.desktop" << 'ENTRY'
[Desktop Entry]
Type=Application
Name=Crow
GenericName=Text Editor
Comment=Selection-first modal text editor
Exec=raven-terminal -e crow %F
Icon=accessories-text-editor
Categories=Utility;TextEditor;Development;
Keywords=editor;text;code;modal;
MimeType=text/plain;
StartupWMClass=raven-terminal
Terminal=false
ENTRY
            chmod 0644 "${dir}/crow.desktop"
            written=$((written + 1))
            log_info "    + crow.desktop"
        fi
    else
        log_warn "  no terminal built; no entry for it or for crow"
        log_warn "  (both are terminal programs and neither can be launched without it)"
    fi

    # Outside that branch on purpose. The file manager is a Wayland client that
    # opens its own window, so unlike crow it does not need the terminal to
    # exist -- an image built with TERMINAL_SKIP=1 still gets a launcher with
    # something in it.
    #
    # Written here rather than installed from the repository's own
    # data/*.desktop, even though that file exists and is nearly identical, so
    # that every entry on this image is decided in one place. The one
    # difference that matters is StartupWMClass: upstream omits it, and the
    # dock then has to fall back to matching the file stem against the app_id.
    # That happens to work -- GTK4 sets the app_id from the application id, and
    # the .desktop is named for it -- but it works by coincidence of two names
    # agreeing, and dock::owns checks StartupWMClass first when it is present.
    if [[ -x "${SYSROOT_DIR}/usr/bin/${FILEMANAGER_BIN}" ]]; then
        cat > "${dir}/${FILEMANAGER_APPID}.desktop" << ENTRY
[Desktop Entry]
Type=Application
Name=Files
GenericName=File Manager
Comment=Browse and manage files
Exec=${FILEMANAGER_BIN} %U
Icon=${FILEMANAGER_APPID}
Categories=System;FileTools;FileManager;GTK;
Keywords=files;folders;explorer;browser;manager;
MimeType=inode/directory;
StartupWMClass=${FILEMANAGER_APPID}
StartupNotify=true
Terminal=false
ENTRY
        chmod 0644 "${dir}/${FILEMANAGER_APPID}.desktop"
        written=$((written + 1))
        log_info "    + ${FILEMANAGER_APPID}.desktop"

        # What opens a directory. Nothing on the image claimed inode/directory
        # before, so "Open Containing Folder" from any application resolved to
        # nothing at all.
        cat > "${dir}/mimeapps.list" << DEFAULTS
[Default Applications]
inode/directory=${FILEMANAGER_APPID}.desktop
DEFAULTS
        chmod 0644 "${dir}/mimeapps.list"
        log_info "    + mimeapps.list (inode/directory)"
    fi

    # The launcher reads this directory; update-desktop-database is what makes
    # the *MIME* half of it work, and nothing else on the image writes it.
    if (( written > 0 )) && command -v update-desktop-database &>/dev/null; then
        update-desktop-database -q "${dir}" 2>/dev/null || true
    fi

    if (( written == 0 )); then
        log_warn "  no application entries written: the launcher will enumerate nothing"
    fi

    log_info "  ${written} application entry(ies) installed"
}

# libinput classifies devices from data files, and libwacom from its own
# database. Without them a touchpad is a generic pointer and a tablet is
# nothing at all -- the compositor runs, the hardware just behaves oddly.
#
# The icon themes are the other half of the shell's appearance and fail just as
# quietly. `pointer::Cursor::load` resolves the default pointer through
# /usr/share/icons/default, whose index.theme is two lines that say
# `Inherits=Adwaita`; with no Adwaita to inherit it returns None, and the
# compositor draws no pointer at all over its own dock, launcher and
# background. Clients that set their own cursor still show one, which is what
# makes it read as a rendering bug rather than a missing package.
#
# breeze/breeze-dark are what the dock and launcher resolve `Icon=` names
# against, and hicolor is the base every theme falls back through. stage2
# stages all of these too; they are repeated here so that a GUI-only rebuild
# (`imlazy gui` against an older sysroot) does not produce a desktop with no
# cursor, in the same spirit as the xkbcomp fallback in stage_xwayland.
stage_gui_data() {
    local dir
    for dir in /usr/share/libinput /usr/share/libwacom /usr/share/X11/xkb; do
        if [[ -d "${dir}" ]]; then
            mkdir -p "${SYSROOT_DIR}${dir}"
            cp -a "${dir}/." "${SYSROOT_DIR}${dir}/" 2>/dev/null || true
            log_info "  staged ${dir}"
        fi
    done

    local theme
    for theme in default Adwaita hicolor breeze breeze-dark; do
        dir="/usr/share/icons/${theme}"
        [[ -d "${dir}" ]] || continue
        if [[ -d "${SYSROOT_DIR}${dir}" ]]; then
            continue
        fi
        mkdir -p "${SYSROOT_DIR}${dir}"
        cp -a "${dir}/." "${SYSROOT_DIR}${dir}/" 2>/dev/null || true
        log_info "  staged icon theme ${theme}"
    done

    # Said plainly, because the symptom -- a desktop with no mouse pointer --
    # does not point at its cause on its own.
    if [[ ! -d "${SYSROOT_DIR}/usr/share/icons/Adwaita/cursors" ]]; then
        log_warn "  no cursor theme staged: the compositor will draw no pointer"
        log_warn "  over its own surfaces. Install adwaita-cursors on the build host."
    fi
}

# =============================================================================
# Session launcher
# =============================================================================
# raven-init starts this when the kernel cmdline says raven.graphics=wayland;
# until now it did not exist and init fell through to a /bin/raven-compositor
# that nothing builds. It sets the session environment up and hands over to the
# compositor. The compositor draws the shell itself; what a session starts
# besides it is the wallpaper daemon and whatever is in session.d, both
# backgrounded ahead of the exec because nothing runs after it.
install_session_launcher() {
    local dest="${SYSROOT_DIR}/usr/bin/raven-wayland-session"

    mkdir -p "${SYSROOT_DIR}/usr/bin"
    cat > "${dest}" << 'LAUNCHER'
#!/bin/sh
# RavenLinux Wayland session: Huginn.
#
# Started by raven-init when the kernel cmdline carries raven.graphics=wayland.
# RAVEN_WAYLAND_COMPOSITOR comes from raven.wayland=<name> and names the
# compositor binary; huginn is the default and the only one that ships.
set -eu

COMPOSITOR="${RAVEN_WAYLAND_COMPOSITOR:-huginn}"
case "${COMPOSITOR}" in
    raven|raven-compositor|"") COMPOSITOR=huginn ;;
esac

# XDG_RUNTIME_DIR comes from raven-init, which creates it owned by whoever the
# session runs as before starting this. The fallback is for a hand-run session
# and derives the uid rather than assuming 0 -- it used to say /run/user/0
# outright, which was correct only while the whole desktop ran as root.
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
export LIBSEAT_BACKEND="${LIBSEAT_BACKEND:-seatd}"
# Tolerated rather than checked: under `set -e` a failed mkdir would end the
# session, and the directory normally exists already because init made it. A
# session user cannot create it themselves -- /run/user is root-owned -- so
# failing here would be failing at something that is not this script's job.
mkdir -p "${XDG_RUNTIME_DIR}" 2>/dev/null || true
chmod 0700 "${XDG_RUNTIME_DIR}" 2>/dev/null || true

# Where .desktop files and icon themes are looked for. huginn defaults to this
# exact value when the variable is unset, so this changes nothing for the
# launcher -- it is set because every toolkit an application might be built
# with reads it too, and inherits whatever the session exports.
export XDG_DATA_DIRS="${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"

# Names this desktop, so that an application shipping OnlyShowIn/NotShowIn is
# filtered against something rather than against an empty string. Matches
# DesktopNames in the huginn.desktop session entry; the two must agree.
export XDG_CURRENT_DESKTOP="${XDG_CURRENT_DESKTOP:-Huginn:Raven}"

# The pointer. huginn falls back to the theme literally named "default", which
# is a two-line index.theme that says `Inherits=Adwaita` -- naming Adwaita
# outright drops that indirection, and is one less file whose absence produces
# a desktop with no visible cursor. Clients read the same two variables, so
# this is also what keeps their pointers the same size as the compositor's.
export XCURSOR_THEME="${XCURSOR_THEME:-Adwaita}"
export XCURSOR_SIZE="${XCURSOR_SIZE:-24}"

command -v "${COMPOSITOR}" >/dev/null 2>&1 || {
    echo "raven-wayland-session: ${COMPOSITOR} is not installed" >&2
    exit 1
}

# The GPU drivers are modules, and raven-init does not wait for one service
# before starting the next -- so without this the compositor races the udev
# coldplug, wins, and takes the only DRM device that exists that early:
# simpledrm, the EFI framebuffer. When the real driver loads seconds later the
# kernel revokes simpledrm, the compositor's GPU disappears out from under it,
# and it shuts down -- a blank panel that looks like a render bug and is a
# start-order bug. raven-udev is idempotent: whoever runs second settles and
# moves on.
# Only root can drive udev, and since the session dropped to a regular user
# this is now init's job -- it runs raven-udev as the wayland-session's
# pre_exec, which happens before the privilege drop. The call is kept here,
# guarded, for a session started by hand from a root shell; as a normal user
# raven-udev would fail, and under `set -e` that failure would end the session
# before the compositor ever started.
if [ "$(id -u)" = 0 ] && [ -x /usr/sbin/raven-udev ]; then
    /usr/sbin/raven-udev || true
fi

# Belt and braces for slow GPUs: if a real card's module is loaded but its DRM
# node has not appeared yet, give it a moment rather than grabbing simpledrm.
i=0
while [ $i -lt 50 ]; do
    for card in /sys/class/drm/card*; do
        [ -e "${card}/device/driver" ] || continue
        case "$(basename "$(readlink "${card}/device/driver")")" in
            simple-framebuffer|simpledrm) ;;
            *) break 2 ;;
        esac
    done
    i=$((i + 1))
    sleep 0.1
done

# A session bus, because nothing else starts one. /etc/raven/init.toml has the
# system bus and stops there, so DBUS_SESSION_BUS_ADDRESS arrives unset and
# every GTK, Qt and Chromium client falls back to libdbus autolaunch -- which
# wants X11, and fails with "Could not parse server address: Unknown address
# type". The log noise is the least of it: a browser that cannot reach a Secret
# Service keeps its saved passwords in plaintext, no XDG portal is reachable,
# and nothing can read the battery.
#
# Backgrounded rather than wrapping the compositor in dbus-run-session, which
# would read better and be wrong: that wrapper waits on its child instead of
# execing it and does not forward signals, so with raven-init signalling the
# one pid it tracks the wrapper would die and leave the compositor orphaned,
# still holding DRM master. Backgrounding keeps the exec below, and with it the
# compositor as the process init supervises.
#
# The socket is the well-known $XDG_RUNTIME_DIR/bus, so a client that probes
# the path rather than reading the variable finds the same bus. /run is a fresh
# tmpfs each boot, so a stale socket can only be this boot's earlier session --
# it is cleared only when nothing answers on it, never while a bus is live.
if command -v dbus-daemon >/dev/null 2>&1; then
    export DBUS_SESSION_BUS_ADDRESS="unix:path=${XDG_RUNTIME_DIR}/bus"
    if ! dbus-send --session --dest=org.freedesktop.DBus --type=method_call \
            --print-reply / org.freedesktop.DBus.Peer.Ping >/dev/null 2>&1; then
        rm -f "${XDG_RUNTIME_DIR}/bus"
        dbus-daemon --session --address="${DBUS_SESSION_BUS_ADDRESS}" \
            --nofork --nopidfile --syslog-only &
    fi
fi

# PipeWire is a per-user service, just like the session bus above. Its Arch
# packages ship systemd user units, but Raven does not run systemd (or another
# user service manager), so installing the binaries alone leaves no media
# server for applications or for huginn's `wpctl` volume control to reach.
# WirePlumber must start after PipeWire: it discovers the ALSA devices, chooses
# the default sink and applies the routing policy.
#
# Keep pid files in the per-user runtime directory so an init-driven restart of
# the compositor reuses the audio session instead of starting a second policy
# manager. PipeWire is also probed directly because somebody may have started
# it by hand without Raven's pid file.
audio_runtime="${XDG_RUNTIME_DIR}/raven-audio"
mkdir -p "${audio_runtime}"

pid_is_alive() {
    [ -r "$1" ] || return 1
    IFS= read -r audio_pid < "$1" || return 1
    case "${audio_pid}" in
        ''|*[!0-9]*) return 1 ;;
    esac
    kill -0 "${audio_pid}" 2>/dev/null
}

if command -v pipewire >/dev/null 2>&1; then
    if ! command -v pw-cli >/dev/null 2>&1 || ! pw-cli info 0 >/dev/null 2>&1; then
        if ! pid_is_alive "${audio_runtime}/pipewire.pid"; then
            pipewire </dev/null &
            echo $! > "${audio_runtime}/pipewire.pid"
        fi
    fi

    # Do not start the policy manager before the media server has bound its
    # socket. Usually this is one pass; the bound keeps a failed daemon from
    # delaying the desktop indefinitely.
    i=0
    while [ $i -lt 50 ] && [ ! -S "${XDG_RUNTIME_DIR}/pipewire-0" ]; do
        i=$((i + 1))
        sleep 0.1
    done

    if [ -S "${XDG_RUNTIME_DIR}/pipewire-0" ] \
            && command -v wireplumber >/dev/null 2>&1 \
            && ! pid_is_alive "${audio_runtime}/wireplumber.pid"; then
        wireplumber </dev/null &
        echo $! > "${audio_runtime}/wireplumber.pid"
    fi

    # Huginn detects its mixer once, during construction. Wait until
    # WirePlumber has selected a real output so that a normal startup cannot
    # permanently turn its volume slider into the disconnected in-memory stub.
    if command -v wpctl >/dev/null 2>&1; then
        i=0
        while [ $i -lt 50 ] \
                && ! wpctl get-volume '@DEFAULT_AUDIO_SINK@' >/dev/null 2>&1; do
            i=$((i + 1))
            sleep 0.1
        done
    fi
fi

# The wallpaper. Backgrounded ahead of the exec below, exactly like the session
# bus above it and for a stronger version of the same reason: ravencanvasd is a
# layer-shell client of the compositor this script is about to *become*, so
# there is no line after the exec on which to start it. It starts first and
# waits -- connect() retries for ten seconds, and finds the socket by searching
# $XDG_RUNTIME_DIR rather than reading a $WAYLAND_DISPLAY that nothing has set
# yet, because the compositor binds the first free number and a stale lock puts
# it on wayland-1.
#
# Guarded rather than written conditionally: this launcher is one file that
# stage-gui.sh writes whole, and `command -v` is the whole of the difference
# between an image built with the wallpaper daemon and one built with
# CANVAS_SKIP=1.
#
# Not a service in /etc/raven/init.toml, where every other daemon on this image
# lives. Init's services are system services started as root before anybody has
# logged in; this is an ordinary unprivileged client that has to run as the
# session's own user and share its runtime directory.
#
# Its death costs a picture and nothing else -- huginn paints its own
# background under everything on the background layer -- which is why nothing
# here checks that it came up.
if command -v ravencanvasd >/dev/null 2>&1; then
    ravencanvasd &
fi

# Per-user session programs -- the nearest thing this system has to a user
# service manager. raven-init's services run as root, once per boot, before
# any login, with no compositor and no per-user runtime directory; a status
# bar, a notification daemon or a clipboard manager is none of those things.
# It belongs to a session: started as its user, after its compositor, and
# gone when it ends. The wallpaper daemon above is one such program with a
# hand-written line; this is the general form, so the next one is a file
# rather than an edit to this script.
#
#     /etc/raven/session.d/            every user on the machine
#     ~/.config/raven/session.d/       this user. A file of the same name
#                                      overrides the machine-wide one, and a
#                                      non-executable one masks it.
#
# Each executable is started once the compositor has bound its socket, with
# WAYLAND_DISPLAY set to it, in name order -- a numeric prefix is the
# convention. setsid puts each in its own session and process group, so one
# that hangs, exits or is killed touches neither the others nor the
# compositor; stdio goes to /dev/null so a chatty one cannot block on a pipe
# nobody reads. A program that needs the socket before this finds it can
# still wait on its own, as ravencanvasd does.
#
# The wait is a background subshell for the same reason ravencanvasd is
# backgrounded: the exec below is the end of this script, and there is no
# line after it. Fifteen seconds is generous -- huginn binds its socket well
# inside one -- and on a compositor that never comes up the subshell exits
# quietly, because by then the session has failed for a louder reason.
# >>> session.d >>>
(
    i=0
    while [ $i -lt 150 ]; do
        for s in "${XDG_RUNTIME_DIR}"/wayland-*; do
            case "$s" in *.lock) continue ;; esac
            [ -S "$s" ] && { WAYLAND_DISPLAY="$(basename "$s")"; export WAYLAND_DISPLAY; break 2; }
        done
        i=$((i + 1))
        sleep 0.1
    done
    [ -n "${WAYLAND_DISPLAY:-}" ] || exit 0
    user_d="${XDG_CONFIG_HOME:-$HOME/.config}/raven/session.d"
    for f in /etc/raven/session.d/* "$user_d"/*; do
        [ -e "$f" ] || continue
        name="$(basename "$f")"
        case "$f" in /etc/raven/session.d/*) [ -e "$user_d/$name" ] && continue ;; esac
        [ -x "$f" ] || continue
        setsid "$f" </dev/null >/dev/null 2>&1 &
    done
) &
# <<< session.d <<<

# exec, not background-and-wait: with no second process to supervise there is
# nothing for this script to do afterwards, and execing puts the compositor
# directly under init -- so its exit status is the session's, and a signal from
# init reaches it rather than a shell that would have to forward it.
#
# --backend udev: this is a TTY, not a nested session. Huginn autodetects from
# an inherited WAYLAND_DISPLAY, which is exactly what a login session lacks,
# but saying so beats depending on the absence of a variable.
exec "${COMPOSITOR}" --backend udev
LAUNCHER

    chmod 0755 "${dest}"
    log_success "  raven-wayland-session installed"
}

# =============================================================================
# Session entry
# =============================================================================
# stage2 creates /usr/share/wayland-sessions but nothing ever wrote into it, so
# the directory looked wired up and enumerated zero sessions. Raven boots the
# compositor from the kernel cmdline (raven.graphics=wayland), not from a
# display manager, so nothing reads this today -- but a greeter added later
# reads exactly this directory, and an empty one is indistinguishable from a
# system with no graphical session at all.
#
# Exec is the launcher, not `huginn` directly: the launcher is what waits for a
# real DRM node instead of grabbing simpledrm, and a greeter that ran the
# compositor straight would hit the exact race install_session_launcher exists
# to avoid.
install_session_entry() {
    local dir="${SYSROOT_DIR}/usr/share/wayland-sessions"

    mkdir -p "${dir}"
    cat > "${dir}/huginn.desktop" << 'ENTRY'
[Desktop Entry]
Name=Raven Desktop (Huginn)
Comment=Wayland compositor and desktop shell
Exec=/usr/bin/raven-wayland-session
TryExec=/usr/bin/huginn
Type=Application
DesktopNames=Huginn;Raven
Keywords=wayland;compositor;desktop;
ENTRY
    chmod 0644 "${dir}/huginn.desktop"
    log_success "  huginn.desktop installed"
}

# =============================================================================
# Install
# =============================================================================
# /usr/bin only -- /bin is a symlink onto it. The old
# `ln -sf ../usr/bin/${binary} ${SYSROOT}/bin/${binary}` unlinked the binary it
# had just installed and left a dangling link in its place, which for this
# stage meant huginn silently missing from the ISO.
install_gui_binary() {
    local binary="$1" src="$2"

    mkdir -p "${SYSROOT_DIR}/usr/bin"
    install -m 0755 "${src}" "${SYSROOT_DIR}/usr/bin/${binary}"
}

# =============================================================================
# Summary
# =============================================================================
print_gui_summary() {
    echo ""
    echo "=========================================="
    echo "  GUI Stage Summary"
    echo "=========================================="
    echo ""

    echo "Components:"
    local spec key package binary desc path
    for spec in "${GUI_COMPONENTS[@]}"; do
        IFS='|' read -r key package binary desc <<< "${spec}"
        path="${SYSROOT_DIR}/usr/bin/${binary}"
        if [[ -f "${path}" ]]; then
            printf "  [OK] %-14s %-6s %s\n" "${binary}" "$(du -h "${path}" | cut -f1)" "${desc}"
        else
            printf "  [--] %-14s %-6s %s\n" "${binary}" "" "${desc}"
        fi
    done

    echo ""
    if [[ -x "${SYSROOT_DIR}/usr/bin/ravend" ]]; then
        echo "Login:"
        printf "  [OK] %-14s %-6s %s\n" "ravend" \
            "$(du -h "${SYSROOT_DIR}/usr/bin/ravend" | cut -f1)" \
            "boots to a password prompt; raven-init starts this, not the session"
    else
        echo "Login:"
        printf "  [--] %-14s %-6s %s\n" "ravend" "" \
            "no login screen: this image autologins with no password"
    fi

    echo ""
    echo "Session:"
    if [[ -x "${SYSROOT_DIR}/usr/bin/raven-wayland-session" ]]; then
        echo "  [OK] raven-wayland-session (boot with raven.graphics=wayland)"
    else
        echo "  [--] raven-wayland-session"
    fi

    # What the compositor needs from the rest of the image to be usable, as
    # opposed to merely running. Each of these was missing at once, and none of
    # them failed the build or printed anything: the ISO was green and the
    # desktop booted to a dock with nothing on it but the launcher, a launcher
    # enumerating nothing, no mouse pointer and one monospace font. A summary
    # that says so is the cheapest place to notice -- and the only one, since
    # dock::items draws no placeholder for a pinned application it cannot
    # resolve.
    echo ""
    echo "Desktop:"

    if [[ -x "${SYSROOT_DIR}/usr/bin/raven-terminal" ]]; then
        echo "  [OK] terminal            /usr/bin/raven-terminal"
    else
        echo "  [!!] terminal            MISSING - nothing can be launched at all"
    fi

    # [--] rather than [!!]: the desktop is complete without a file manager,
    # which is exactly why its absence needs saying out loud -- nothing else in
    # this build fails when it is not there.
    if [[ -x "${SYSROOT_DIR}/usr/bin/${FILEMANAGER_BIN}" ]]; then
        echo "  [OK] file manager        /usr/bin/${FILEMANAGER_BIN}"

        # Checked separately from the binary, because this is the failure that
        # produces a program which installs cleanly and then dies on launch
        # with a GLib error naming a schema rather than a missing file.
        if [[ -f "${SYSROOT_DIR}/usr/share/glib-2.0/schemas/gschemas.compiled" ]]; then
            echo "  [OK] GSettings schemas   gschemas.compiled"
        else
            echo "  [!!] GSettings schemas   MISSING - every GTK application aborts on startup"
        fi
        if [[ -x "${SYSROOT_DIR}/usr/bin/bwrap" ]]; then
            echo "  [OK] image decoding      glycin + bwrap"
        else
            echo "  [--] image decoding      no bwrap - GTK will display no images"
        fi
    else
        echo "  [--] file manager        not built"
    fi

    # [--] and not [!!]: huginn paints its own background under everything, so
    # an image without this is a desktop with a flat colour behind it rather
    # than a broken one. Worth a line all the same -- nothing else in the build
    # notices, and "the wallpaper I set does not appear" is otherwise chased
    # from the wrong end.
    if [[ -x "${SYSROOT_DIR}/usr/bin/ravencanvasd" ]]; then
        echo "  [OK] wallpaper           /usr/bin/ravencanvasd"
        if [[ -x "${SYSROOT_DIR}/usr/bin/raven-set-wallpaper" ]]; then
            echo "  [OK] wallpaper control   raven-set-wallpaper, ravencanvas"
        else
            echo "  [--] wallpaper control   no raven-set-wallpaper - set it by hand in set/"
        fi
        # The empty library is the expected state of a fresh image and not a
        # fault: no photograph is committed to this repository, and one is a
        # licence question nobody asked for. Said out loud because a desktop
        # showing the built-in gradient looks like a wallpaper that failed.
        local set_dir="${SYSROOT_DIR}/usr/share/wallpaper/set"
        if [[ -n "$(find "${set_dir}" -maxdepth 1 -name 'wallpaper.*' 2>/dev/null)" ]]; then
            echo "  [OK] wallpaper set       $(basename "$(find "${set_dir}" -maxdepth 1 -name 'wallpaper.*' | sort | head -1)")"
        else
            echo "  [--] wallpaper set       none - the built-in gradient; raven-set-wallpaper <image>"
        fi
    else
        echo "  [--] wallpaper           not built - huginn's flat background"
    fi

    if [[ -x "${SYSROOT_DIR}/usr/bin/${ROOSTBAR_BIN}" \
            && -x "${SYSROOT_DIR}/etc/raven/session.d/50-roostbar" ]]; then
        echo "  [OK] status bar          /usr/bin/${ROOSTBAR_BIN} (session.d enabled)"
    else
        echo "  [--] status bar          not built"
    fi

    local entries=0
    if [[ -d "${SYSROOT_DIR}/usr/share/applications" ]]; then
        entries="$(find "${SYSROOT_DIR}/usr/share/applications" -name '*.desktop' 2>/dev/null | wc -l)"
    fi
    if (( entries > 0 )); then
        echo "  [OK] application menu    ${entries} entry(ies)"
    else
        echo "  [!!] application menu    EMPTY - the launcher will enumerate nothing"
    fi

    if [[ -d "${SYSROOT_DIR}/usr/share/icons/Adwaita/cursors" ]]; then
        echo "  [OK] cursor theme        Adwaita"
    else
        echo "  [!!] cursor theme        MISSING - no pointer over the shell"
    fi

    local icon_files=0
    if [[ -d "${SYSROOT_DIR}/usr/share/icons/breeze-dark" ]]; then
        icon_files="$(find "${SYSROOT_DIR}/usr/share/icons/breeze-dark" -type f 2>/dev/null | wc -l)"
    fi
    if (( icon_files > 0 )); then
        echo "  [OK] icon theme          breeze-dark (${icon_files} files)"
    else
        echo "  [--] icon theme          missing - dock and launcher icons blank"
    fi

    # Counted by family rather than by file: four faces of one monospace family
    # is what the image shipped, and it looks like plenty until you notice the
    # shell has no proportional face and nothing outside Latin draws.
    local font_files=0
    if [[ -d "${SYSROOT_DIR}/usr/share/fonts" ]]; then
        font_files="$(find "${SYSROOT_DIR}/usr/share/fonts" \
            \( -iname '*.ttf' -o -iname '*.otf' -o -iname '*.ttc' \) 2>/dev/null | wc -l)"
    fi
    if [[ -n "$(find "${SYSROOT_DIR}/usr/share/fonts" -iname 'DejaVuSans.ttf' 2>/dev/null)" ]]; then
        echo "  [OK] fonts               ${font_files} face(s), proportional present"
    elif (( font_files > 0 )); then
        echo "  [--] fonts               ${font_files} face(s), MONOSPACE ONLY"
    else
        echo "  [!!] fonts               NONE - the shell will draw no text"
    fi

    if (( ${#GUI_FAILED[@]} > 0 )); then
        echo ""
        echo "Not built: ${GUI_FAILED[*]}"
        echo "  (the ISO still builds; the console system does not need these)"
    fi
    echo ""
}

# =============================================================================
# Main
# =============================================================================
main() {
    echo ""
    echo "=========================================="
    echo "  GUI Stage: Compositor and Shell"
    echo "=========================================="
    echo ""

    if [[ "${GUI_SKIP:-0}" == "1" ]]; then
        log_info "GUI_SKIP=1, skipping the GUI stage"
        return 0
    fi

    mkdir -p "${LOGS_DIR}" "${GUI_SRC_DIR}" "${GUI_STAGE_DIR}"

    if [[ ! -d "${SYSROOT_DIR}" ]]; then
        log_error "Sysroot not found at ${SYSROOT_DIR}"
        log_error "Run stage2 and stage3 first."
        return 1
    fi

    # Shell/session changes should not rebuild a Rust compositor. This path is
    # intentionally before the target and toolchain checks: all it needs is an
    # existing sysroot, and `imlazy session` is meant to take seconds.
    if [[ "${GUI_SESSION_ONLY:-0}" == "1" ]]; then
        log_step "Updating graphical session wiring..."
        install_session_launcher
        install_session_entry
        log_success "Graphical session wiring updated; no GUI binaries rebuilt"
        echo ""
        return 0
    fi

    if [[ -z "${GUI_TARGET}" ]]; then
        log_warn "Cannot determine the host Rust target, skipping the GUI stage"
        return 0
    fi

    # Fail-soft, exactly like the Raven stage: a console system that boots is
    # worth more than a graphical one that does not build.
    if ! have_gui_toolchain; then
        log_warn "GUI build dependencies unavailable, skipping the GUI stage"
        return 0
    fi

    log_step "RavenGUI (${GUI_ALL_BINARIES[*]})"

    if ! fetch_gui_source; then
        log_warn "  source unavailable, skipping the GUI stage"
        return 0
    fi

    local src="${GUI_SRC_DIR}/${GUI_REPO}"

    log_info "  building for ${GUI_TARGET}..."
    if ! build_gui_workspace "${src}"; then
        log_warn "  build failed, skipping the GUI stage"
        GUI_FAILED=("${GUI_ALL_BINARIES[@]}")
        print_gui_summary
        return 0
    fi

    # All or nothing. Half a graphical session is worse than none: a console
    # at least tells you what went wrong.
    local -a built_paths=()
    local spec key package binary desc out
    for spec in "${GUI_COMPONENTS[@]}"; do
        IFS='|' read -r key package binary desc <<< "${spec}"
        out="${src}/target/${GUI_TARGET}/release/${binary}"
        if [[ ! -f "${out}" ]]; then
            log_warn "  ${binary}: not produced by the build"
            GUI_FAILED=("${GUI_ALL_BINARIES[@]}")
            print_gui_summary
            return 0
        fi
        built_paths+=("${out}")
    done

    for spec in "${GUI_COMPONENTS[@]}"; do
        IFS='|' read -r key package binary desc <<< "${spec}"
        out="${src}/target/${GUI_TARGET}/release/${binary}"
        cp "${out}" "${GUI_STAGE_DIR}/${binary}"
        install_gui_binary "${binary}" "${GUI_STAGE_DIR}/${binary}"
        GUI_BUILT+=("${binary}")
        log_success "  ${binary} installed ($(du -h "${out}" | cut -f1))"
    done

    log_step "Staging shared libraries..."
    stage_gui_libraries "${built_paths[@]}"
    stage_gui_data

    log_step "Staging XWayland..."
    stage_xwayland

    # Before the entries: install_desktop_entries writes only what it can see
    # a binary for, so a terminal staged after it would not be described.
    log_step "Staging the terminal..."
    stage_terminal

    # Same ordering rule, same reason: install_desktop_entries writes only what
    # it can see a binary for.
    log_step "Staging the file manager..."
    stage_filemanager

    log_step "Installing application entries..."
    install_desktop_entries

    log_step "Installing the session launcher..."
    install_session_launcher
    install_session_entry

    # With the session rather than with the applications: the launcher just
    # written is what starts it, and the line that does so is guarded on the
    # binary this installs. Either order works -- the guard is evaluated at
    # boot, not here -- and this one keeps the two next to each other.
    log_step "Staging the wallpaper daemon..."
    stage_canvas

    log_step "Staging the status bar..."
    stage_roostbar

    # Last, and after the session launcher on purpose: ravend starts that
    # launcher once somebody has authenticated, so installing the login screen
    # before the thing it hands over to would leave a window -- however
    # theoretical in a build script -- where the image has a password prompt
    # and nothing behind it.
    log_step "Staging the login screen..."
    stage_login

    print_gui_summary

    log_success "GUI stage complete!"
    echo ""
}

main "$@"
