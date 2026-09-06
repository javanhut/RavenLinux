#!/bin/bash
# =============================================================================
# RavenLinux component tables
# =============================================================================
# The single declaration site for every repository the image is built from, and
# for the binaries each one installs into /usr/bin.
#
# WHY THIS IS A LIBRARY AND NOT THREE COPIES
#
# Three files need this list and they need to agree: stage-raven.sh builds the
# static layer, stage-gui.sh builds the graphical one, and stage4-iso.sh checks
# that what the other two were supposed to produce is actually in the sysroot.
# When the check carried its own hardcoded copy it drifted, exactly the way the
# GUI stage's own comment predicted a hardcoded copy would: `imlazy` was in
# RAVEN_COMPONENTS and absent from the check, `raven-ports` was built and never
# looked for, and nine of the ten GUI repositories had no presence check at all.
# A component could vanish from an image and every stage still reported success.
#
# This is the same arrangement scripts/lib/usrmerge.sh already has with
# check-manifests.sh, and for the same reason: the thing that does the work and
# the thing that checks the work read one table, so they cannot disagree.
#
# ADDING A COMPONENT
#
# Add one row here. The stage that builds it picks it up, and stage4 starts
# checking for it, with no other edit.
# =============================================================================

# Guard against double-sourcing: build.sh sources every stage into one shell,
# so this file is read several times per build and the tables are `declare -a`.
[[ -n "${RAVEN_COMPONENTS_SH_LOADED:-}" ]] && return 0
RAVEN_COMPONENTS_SH_LOADED=1

# Every component repository lives under this account. Kept here so a fork
# changes one string rather than one per component.
RAVEN_GITHUB_OWNER="${RAVEN_GITHUB_OWNER:-javanhut}"

# =============================================================================
# The Raven layer -- scripts/stages/stage-raven.sh
# =============================================================================
# key|repo|lang|binaries|build_targets|description
#
#   key           short name; also the RAVEN_<KEY>_REF env suffix and the
#                 packages/raven/<key> directory
#   repo          github.com/${RAVEN_GITHUB_OWNER}/<repo>
#   lang          go | rust
#   binaries      the installed name(s) in /usr/bin. Comma-separated for a
#                 component that ships more than one -- caw is a CLI plus the
#                 daemon it drives, and neither is useful without the other.
#   build_targets go:   the package path(s) to build, positionally paired with
#                       `binaries`
#                 rust: the workspace package(s) to pass to -p, or "." for a
#                       plain crate
#
# The graphical components are intentionally absent from this table. RavenGUI
# (huginn) links libudev, libdrm, libseat and libinput through smithay, so it
# cannot be a static musl binary the way everything here is. Those live in
# GUI_COMPONENTS and GUI_APPS below.
declare -a RAVEN_COMPONENTS=(
    "ravenshell|RavenShell|go|ravenshell|.|Raven Shell - interactive shell and scripting language"
    "rvn|RavenPackageManager|rust|rvn|.|Raven Package Manager"
    "poxy|Poxy|go|poxy|./cmd|Poxy - universal package manager"
    "ivaldi|Ivaldi|rust|ivaldi|.|Ivaldi - version control system"
    "crow|CrowTextEditor|rust|crow|.|Crow - text editor"
    "imlazy|ImLazy|go|imlazy|.|ImLazy - task runner"
    "oxigen|OxigenLang|rust|oxigen|oxigen|OxigenLang - interpreted language"
    "caw|CAW|rust|caw,cawd|caw,cawd|CAW - wireless and network utility, with its daemon"
)

# The init crate. Local to this repository rather than fetched, so it is not a
# row above -- but it is built by the same stage and it is just as required, so
# the check has to know about it. raven-rc dispatches on argv[0], so
# poweroff/reboot/halt/shutdown are symlinks to it and not separate binaries.
RAVEN_INIT_BINARIES="raven-init,raven-rc,raven-powerd,raven-ports,raven-timed"

# =============================================================================
# The GUI layer -- scripts/stages/stage-gui.sh
# =============================================================================
# The compositor workspace. key|package|binary|description -- one cargo build
# of the RavenGUI tree produces every row.
declare -a GUI_COMPONENTS=(
    "huginn-comp|huginn-comp|huginn|Huginn - Wayland compositor, and the shell it draws"
)

# The rest of the desktop: one row per repository the GUI stage clones and
# builds on its own. key|repo|binaries|description
#
#   key       the <KEY>_REPO / <KEY>_URL / <KEY>_REF / <KEY>_OFFLINE prefix
#   repo      github.com/${RAVEN_GITHUB_OWNER}/<repo>
#   binaries  what lands in /usr/bin, comma-separated. Every one of these is
#             all-or-nothing in its stage: a component that produced only some
#             of its binaries installs none of them.
#
# GUI_REPO (RavenGUI itself) is not here -- it is the workspace above, fetched
# by fetch_gui_source rather than by the generic per-application path.
declare -a GUI_APPS=(
    "TERMINAL|RavenTerminal|raven-terminal|Raven Terminal - the Wayland terminal emulator"
    "FILEMANAGER|RavenFileManager|ravenfilemanager|Raven Files - the GTK4 file manager"
    "SETTINGS|RavenSettingsUI|raven-settings|Raven Settings - network, sound, screens and updates"
    "STORE|RavenStore|raven-store|Raven Store - the graphical front-end for rvn"
    "BATTERY|RavenBatteryManagement|raven-power|Raven Power - battery profiles and energy use"
    "CONTROLS|RavenControls|raven-controls,raven-controlsd|Raven Controls - keyboard backlight, fans and thermals, with its daemon"
    "LOGIN|RavenLogin|ravend,raven-greeter,raven-lock|Raven Login - the display manager, its greeter and the lock screen"
    "CANVAS|RavenCanvas|ravencanvasd,ravencanvas|Raven Canvas - the wallpaper daemon and its CLI"
    "ROOSTBAR|RoostBar|roostbar|RoostBar - the layer-shell status bar"
)

# Written by install_session_launcher, not built from any repository. It is
# what starts a session's clients, so an image with the compositor and without
# this one boots to a compositor drawing nothing.
GUI_SESSION_BINARIES="raven-wayland-session"

# =============================================================================
# Accessors
# =============================================================================
# Everything below reads the tables above. Nothing else should re-derive them.

# Splits a comma-separated field into the named array.
#   raven_split_list <array-name> <a,b,c>
raven_split_list() {
    local -n _out="$1"
    IFS=',' read -r -a _out <<< "$2"
}

# Every binary the Raven layer installs, one per line, in table order.
raven_layer_binaries() {
    local spec binaries
    for spec in "${RAVEN_COMPONENTS[@]}"; do
        IFS='|' read -r _ _ _ binaries _ _ <<< "${spec}"
        printf '%s\n' "${binaries//,/$'\n'}"
    done
    printf '%s\n' "${RAVEN_INIT_BINARIES//,/$'\n'}"
}

# Every binary the GUI layer installs, one per line, in table order.
raven_gui_binaries() {
    local spec binary binaries
    for spec in "${GUI_COMPONENTS[@]}"; do
        IFS='|' read -r _ _ binary _ <<< "${spec}"
        printf '%s\n' "${binary}"
    done
    for spec in "${GUI_APPS[@]}"; do
        IFS='|' read -r _ _ binaries _ <<< "${spec}"
        printf '%s\n' "${binaries//,/$'\n'}"
    done
    printf '%s\n' "${GUI_SESSION_BINARIES//,/$'\n'}"
}

# The repository a Raven-layer or GUI component is cloned from.
#   raven_component_url <repo>
raven_component_url() {
    printf 'https://github.com/%s/%s.git\n' "${RAVEN_GITHUB_OWNER}" "$1"
}

# Populates <KEY>_REPO, <KEY>_URL and <KEY>_BINARIES from the GUI_APPS row for
# <KEY>, so the GUI stage names each repository once -- here -- and every
# fetch, build and install site reads it back out.
#
# Returns 1 for a key with no row, which is a programming error in the caller
# rather than a build failure: the stage aborts on it instead of quietly
# cloning "https://github.com/javanhut/.git".
raven_gui_app_vars() {
    local key="$1" spec rowkey repo binaries

    for spec in "${GUI_APPS[@]}"; do
        IFS='|' read -r rowkey repo binaries _ <<< "${spec}"
        [[ "${rowkey}" == "${key}" ]] || continue
        printf -v "${key}_REPO" '%s' "${repo}"
        printf -v "${key}_URL" '%s' "$(raven_component_url "${repo}")"
        printf -v "${key}_BINARIES" '%s' "${binaries}"
        return 0
    done

    printf 'FATAL: no GUI_APPS row for key "%s" in %s\n' \
        "${key}" "${BASH_SOURCE[0]}" >&2
    return 1
}
