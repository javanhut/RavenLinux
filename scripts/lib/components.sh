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
# builds on its own. key|repo|binaries|manifest|description
#
#   key       the <KEY>_REPO / <KEY>_URL / <KEY>_REF / <KEY>_OFFLINE prefix
#   repo      github.com/${RAVEN_GITHUB_OWNER}/<repo>
#   binaries  what lands in /usr/bin, comma-separated. Every one of these is
#             all-or-nothing in its stage: a component that produced only some
#             of its binaries installs none of them.
#   manifest  the packages/<manifest>/package.toml whose [source] commit pins
#             this repository, or empty for one with no manifest yet. The
#             Raven layer needs no such column: its manifest directory is its
#             key, so packages/raven/<key> is derived rather than restated.
#
# GUI_REPO (RavenGUI itself) is not here -- it is the workspace above, fetched
# by fetch_gui_source rather than by the generic per-application path.
declare -a GUI_APPS=(
    "TERMINAL|RavenTerminal|raven-terminal||Raven Terminal - the Wayland terminal emulator"
    "FILEMANAGER|RavenFileManager|ravenfilemanager|gui/ravenfilemanager|Raven Files - the GTK4 file manager"
    "SETTINGS|RavenSettingsUI|raven-settings|gui/raven-settings|Raven Settings - network, sound, screens and updates"
    "STORE|RavenStore|raven-store|gui/raven-store|Raven Store - the graphical front-end for rvn"
    "BATTERY|RavenBatteryManagement|raven-power|gui/raven-power|Raven Power - battery profiles and energy use"
    "CONTROLS|RavenControls|raven-controls,raven-controlsd|gui/raven-controls|Raven Controls - keyboard backlight, fans and thermals, with its daemon"
    "LOGIN|RavenLogin|ravend,raven-greeter,raven-lock|gui/ravenlogin|Raven Login - the display manager, its greeter and the lock screen"
    "CANVAS|RavenCanvas|ravencanvasd,ravencanvas|gui/ravencanvas|Raven Canvas - the wallpaper daemon and its CLI"
    "ROOSTBAR|RoostBar|roostbar|gui/roostbar|RoostBar - the layer-shell status bar"
)

# RavenTerminal has no row under packages/ -- the manifest field above is empty
# for it, and it is the one GUI repository that cannot be pinned until one is
# written. fetch reports it as unpinned rather than pretending otherwise.

# The compositor's own manifest, for the same treatment.
GUI_MANIFEST="gui/ravengui"

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
        IFS='|' read -r _ _ binaries _ _ <<< "${spec}"
        printf '%s\n' "${binaries//,/$'\n'}"
    done
    printf '%s\n' "${GUI_SESSION_BINARIES//,/$'\n'}"
}

# The repository a Raven-layer or GUI component is cloned from.
#   raven_component_url <repo>
raven_component_url() {
    printf 'https://github.com/%s/%s.git\n' "${RAVEN_GITHUB_OWNER}" "$1"
}

# Populates <KEY>_REPO, <KEY>_URL, <KEY>_BINARIES and <KEY>_MANIFEST from the
# GUI_APPS row for
# <KEY>, so the GUI stage names each repository once -- here -- and every
# fetch, build and install site reads it back out.
#
# Returns 1 for a key with no row, which is a programming error in the caller
# rather than a build failure: the stage aborts on it instead of quietly
# cloning "https://github.com/javanhut/.git".
raven_gui_app_vars() {
    local key="$1" spec rowkey repo binaries manifest

    for spec in "${GUI_APPS[@]}"; do
        IFS='|' read -r rowkey repo binaries manifest _ <<< "${spec}"
        [[ "${rowkey}" == "${key}" ]] || continue
        printf -v "${key}_REPO" '%s' "${repo}"
        printf -v "${key}_URL" '%s' "$(raven_component_url "${repo}")"
        printf -v "${key}_BINARIES" '%s' "${binaries}"
        printf -v "${key}_MANIFEST" '%s' "${manifest}"
        return 0
    done

    printf 'FATAL: no GUI_APPS row for key "%s" in %s\n' \
        "${key}" "${BASH_SOURCE[0]}" >&2
    return 1
}

# =============================================================================
# Pins
# =============================================================================
# A component is fetched at whatever ref resolves here. There are three sources
# and they are tried in this order:
#
#   1. <KEY>_REF / RAVEN_<KEY>_REF in the environment -- a one-off override
#   2. the [source] commit (or tag) in the component's packages/ manifest
#   3. nothing: the default branch, whatever it is today
#
# WHY 2 EXISTS
#
# It did not, for a long time, and the manifests said otherwise. Eight
# packages/raven manifests carried a `commit = "..."` line -- rvn's is even
# introduced by the comment "The commit pin below is what makes it
# reproducible" -- and no code anywhere read them. Every build cloned the
# default branch, so two ISOs built a week apart from an unchanged tree
# contained different software, and the hashes in the manifests described a
# state nothing had ever built. packages/gui/ravengui's own comment had already
# noticed this about itself.
#
# Reading the manifests is what makes those lines true. A manifest with no pin
# still means "track the branch" -- that is the honest reading of an absent
# field, and it is what the unpinned GUI manifests currently say.
#
# Set RAVEN_IGNORE_MANIFEST_PINS=1 to skip step 2 for every component, which is
# how you build the tip of everything without editing nine files.

# Prints the [source] commit -- or tag, if there is no commit -- from a
# component's manifest. Returns 1 when there is no manifest or no pin in it.
#
# Scoped to the [source] table deliberately: `commit` is a plausible key
# elsewhere in a manifest, and a pin picked up from the wrong table would be a
# silently wrong checkout rather than an error.
#   raven_manifest_ref <manifest-path-under-packages>
raven_manifest_ref() {
    local rel="$1"
    [[ -n "${rel}" ]] || return 1

    local file="${PROJECT_ROOT:-.}/packages/${rel}/package.toml"
    [[ -f "${file}" ]] || return 1

    awk '
        /^[[:space:]]*\[/ { insource = ($0 ~ /^[[:space:]]*\[source\]/); next }
        !insource { next }
        /^[[:space:]]*(commit|tag)[[:space:]]*=/ {
            key = $0; sub(/[[:space:]]*=.*$/, "", key); gsub(/[[:space:]]/, "", key)
            val = $0; sub(/^[^=]*=[[:space:]]*/, "", val)
            sub(/[[:space:]]*#.*$/, "", val)
            gsub(/"/, "", val); gsub(/[[:space:]]/, "", val)
            if (val == "") next
            if (key == "commit") commit = val; else tag = val
        }
        END {
            if (commit != "") { print commit; exit 0 }
            if (tag    != "") { print tag;    exit 0 }
            exit 1
        }
    ' "${file}"
}

# True for a ref git can only reach by fetching it as an object: a full commit
# id. `git clone --branch` takes tags and branches and rejects these, which is
# why the fetchers below branch on it.
raven_ref_is_sha() {
    [[ "$1" =~ ^[0-9a-fA-F]{40}$ ]]
}

# Resolves the ref for one component and says where it came from.
#
# Prints "<origin><TAB><ref>", origin being env | manifest | head. Origin comes
# FIRST because the ref is empty for an unpinned component, and `read` with a
# tab IFS strips a leading tab as whitespace -- emitting the ref first would
# shift `head` into the ref field and pin every unpinned component to a branch
# named "head". Origin is never empty, so this order cannot be misread.
#   raven_resolve_ref <env-ref> <manifest-path>
raven_resolve_ref() {
    local env_ref="$1" manifest="$2" ref

    if [[ -n "${env_ref}" ]]; then
        printf 'env\t%s\n' "${env_ref}"
        return 0
    fi

    if [[ "${RAVEN_IGNORE_MANIFEST_PINS:-0}" != "1" ]]; then
        if ref="$(raven_manifest_ref "${manifest}")" && [[ -n "${ref}" ]]; then
            printf 'manifest\t%s\n' "${ref}"
            return 0
        fi
    fi

    printf 'head\t\n'
}

# =============================================================================
# Fetch
# =============================================================================
# One implementation for both stages. They had a copy each, and stage-gui.sh's
# copy carries the comment explaining why that was a bad idea -- "the
# branch-then-default clone retry are all easy to get subtly different". They
# then diverged anyway, because only one of them could fetch a commit id.
#
#   raven_fetch_repo <name> <url> <dest> <ref> <offline> [origin-label]
#
# Returns 0 when <dest> holds a usable checkout, 1 when it does not. Callers
# treat 1 as "skip this component", never as a reason to fail the build.
raven_fetch_repo() {
    local name="$1" url="$2" dest="$3" ref="$4" offline="$5" origin="${6:-}"
    local parent; parent="$(dirname "${dest}")"

    if [[ "${offline}" == "1" ]]; then
        if [[ -d "${dest}/.git" ]]; then
            log_info "  offline: using existing clone of ${name}"
            return 0
        fi
        log_warn "  offline: no clone of ${name} in ${parent}"
        return 1
    fi

    if ! command -v git &>/dev/null; then
        log_warn "  git not found, cannot fetch ${name}"
        return 1
    fi

    mkdir -p "${parent}"

    case "${origin}" in
        manifest) log_info "  ${name} pinned at ${ref} (manifest)" ;;
        env)      log_info "  ${name} pinned at ${ref} (environment)" ;;
        head)     log_info "  ${name} is unpinned; tracking the default branch" ;;
    esac

    # A short hex string is almost certainly a truncated commit id, and git
    # cannot fetch one: it would fall through to --branch, fail, and clone the
    # default branch instead -- a silently unpinned component. Say so.
    if [[ ! "${ref}" =~ ^[0-9a-fA-F]{40}$ && "${ref}" =~ ^[0-9a-fA-F]{7,39}$ ]]; then
        log_warn "  ${name}: '${ref}' looks like a short commit id; git needs all 40"
    fi

    if [[ -d "${dest}/.git" ]]; then
        log_info "  updating ${name}..."
        # Unshallow-safe: --depth on fetch keeps shallow clones shallow. A
        # commit id works here where it does not for --branch, because this is
        # a fetch of an object rather than a ref by name.
        if ! ( cd "${dest}" \
               && git fetch --tags --depth 1 origin "${ref:-HEAD}" 2>/dev/null \
               && git reset --hard FETCH_HEAD >/dev/null 2>&1 ); then
            log_warn "  could not update ${name}, using the existing checkout"
        fi
    else
        log_info "  cloning ${name}..."
        rm -rf "${dest}"
        if raven_ref_is_sha "${ref}"; then
            # `git clone --branch` rejects a commit id, so the pinned-by-hash
            # case is init + fetch-that-object + checkout. Still depth 1.
            if ! ( mkdir -p "${dest}" \
                   && cd "${dest}" \
                   && git init -q \
                   && git remote add origin "${url}" \
                   && git fetch --depth 1 -q origin "${ref}" \
                   && git checkout -q FETCH_HEAD ); then
                log_warn "  ${name}: could not fetch commit ${ref}"
                rm -rf "${dest}"
                return 1
            fi
        elif [[ -n "${ref}" ]]; then
            git clone --depth 1 --branch "${ref}" -q "${url}" "${dest}" 2>/dev/null \
                || git clone --depth 1 -q "${url}" "${dest}" || return 1
        else
            git clone --depth 1 -q "${url}" "${dest}" || return 1
        fi
    fi

    local rev
    rev="$(cd "${dest}" && git rev-parse --short HEAD 2>/dev/null || echo unknown)"
    log_info "  ${name} @ ${rev}"

    # A pin that did not take is worth more than a log line: the build is not
    # the build that was asked for. It is still not fatal -- the component
    # compiles and the ISO is usable -- so this warns rather than returning 1.
    if raven_ref_is_sha "${ref}"; then
        local head
        head="$(cd "${dest}" && git rev-parse HEAD 2>/dev/null || echo unknown)"
        if [[ "${head,,}" != "${ref,,}" ]]; then
            log_warn "  ${name}: pinned at ${ref} but the checkout is ${head}"
        fi
    fi

    return 0
}
