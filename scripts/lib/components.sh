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
    "rvn|RavenPackageManager|rust|rvn,rvnd|.|Raven Package Manager, and the daemon that installs for wheel without sudo"
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
#
# raven-firewall is the eighth name and was missing from this constant for as
# long as it has existed in init/Cargo.toml. That is worse than it sounds,
# because this string is not a list of names to check: stage-raven.sh passes it
# to build_rust_component as the set of binaries to copy out of the target
# directory, and stage4's check_sysroot_layers reads it back to decide what the
# image must carry. A [[bin]] that is not in here is compiled on every build
# and then thrown away with the target directory -- documented in
# ARCHITECTURE.md, built, and installed nowhere. Adding it here is what makes
# the binary reach the staging directory at all; stage-raven.sh's build_raven_init
# still has to `install -m 0755` it into the sysroot from there, beside the
# seven lines that do the same for the others.
#
# Those two changes are one change and have to land together. stage4's
# check_sysroot_layers reads this constant and returns 1 for anything it
# cannot find in the sysroot, and main() treats that as fatal -- "Refusing an
# incomplete desktop ISO" -- so a name added here without the matching install
# line stops the ISO build rather than shipping an image quietly missing a
# binary. That is the trade this file exists to make, and it is the right way
# round; it is also why the missing install line cannot be deferred.
RAVEN_INIT_BINARIES="raven-init,raven-rc,raven-powerd,raven-ports,raven-timed,raven-mount,raven-fprintd,raven-firewall"

# Face unlock. Local to this repository like the init crate, and in a crate of
# its own rather than a binary of init's: it links an ONNX inference engine,
# and nothing that PID 1 builds should have to compile that. Its models are not
# in the image -- see faced/fetch-models.sh.
RAVEN_FACED_BINARIES="raven-faced"

# =============================================================================
# The base layer -- scripts/stages/stage2-native.sh
# =============================================================================
# Not a component table: these are not built from repositories and most of them
# are not ours. They are here because they are the programs /etc/raven/init.toml
# names in an `exec` and the layers below do not provide -- so nothing checked
# whether any of them was in the image, and a boot service pointing at a path
# that does not exist fails silently by design (`critical = false`).
#
# raven-dhcp is why this list exists. init.toml has run `/bin/raven-dhcp --all
# -q` as the `network` service for as long as there has been an init.toml, and
# init/src/ports.rs runs the same path when a wired link comes up after boot.
# No stage built it and no stage installed it. There was no such program, on
# any image this repository has ever produced, and every wired machine booted
# with no address and no complaint. It is a shell script in configs/ now, and
# stage2's install_raven_dhcp puts it in place.
#
# raven-udev and raven-console-font are why the list says stage2 and nothing
# else. stage4 used to install them itself -- from create_squashfs, after
# check_sysroot_layers had already looked for them -- so every build reported
# them missing and refused the ISO. Everything here must be in place before
# stage4 starts; stage2's install_raven_udev and install_raven_console_font.
#
# syslogd and klogd are deliberately absent: their services ship
# `enabled = false`, so an image without them is the intended image.
#
# raven-firmware is the one entry here that init.toml does not name in an exec
# -- it has no service on purpose, because nothing should write firmware to a
# dock unasked. It is in the list anyway, for the reason the list exists: it is
# installed by stage2's install_raven_firmware and would go missing exactly as
# quietly as raven-dhcp did, with the same "there is no such program" outcome
# for anyone who ran it.
#
# raven-snapshot is the second such entry and is here for the same reason
# raven-firmware is: no init.toml service names it, because what calls it is
# rvn, once per transaction, through the hook in the file list below. An image
# without it takes no snapshot before an upgrade, and the way anybody finds
# that out is by needing one. stage2's install_raven_snapshot puts it in place.
RAVEN_BASE_BINARIES="raven-udev,raven-console-font,agetty,dbus-daemon,raven-dhcp,dhcpcd,raven-firmware,raven-snapshot"

# Base-layer files that are not programs and go missing just as quietly. The CA
# bundle is the first: stage2's copy_ca_certificates stages it from the build
# host, and an image without it builds, boots and logs in -- and then cannot
# make one verified HTTPS connection, which on a running system is also the
# only way to get the bundle back. Absolute paths inside the sysroot.
#
# The other two are policy files, and they are here because policy that is not
# installed is indistinguishable from policy that is:
#
#   50-raven.conf      the kernel parameter policy. raven-init's sysctl stage
#                      reads /usr/lib/sysctl.d at boot and applies what it
#                      finds; for every image built before stage2 started
#                      installing this, it found nothing, so kptr_restrict,
#                      dmesg_restrict, the protected_* symlink and FIFO
#                      hardening and rp_filter all stayed at the kernel
#                      defaults while the file in the repository said
#                      otherwise. Nothing fails when it is missing, which is
#                      exactly why the check has to be here.
#   50-snapshot.toml   rvn's pre-transaction hook, in the vendor directory
#                      txhooks.rs reads first. Without it rvn upgrades a btrfs
#                      machine with no way back, silently -- the hook is the
#                      only thing that calls raven-snapshot on its own.
RAVEN_BASE_FILES="/etc/ssl/certs/ca-certificates.crt,/usr/lib/sysctl.d/50-raven.conf,/usr/share/rvn/hooks.d/50-snapshot.toml"

# =============================================================================
# The GUI layer -- scripts/stages/stage-gui.sh
# =============================================================================
# The compositor workspace. key|package|binary|description -- one cargo build
# of the RavenGUI tree produces every row.
#
# raven-open is the desktop's xdg-open, xdg-settings and xdg-mime -- one binary
# under three more names, which install_raven_open_names links. It is here
# rather than a Raven-layer row because it shares raven-desktop, the .desktop
# parser, with the compositor's launcher: one workspace, so what the launcher
# lists and what opens a link can never parse an entry differently.
declare -a GUI_COMPONENTS=(
    "huginn-comp|huginn-comp|huginn|Huginn - Wayland compositor, and the shell it draws"
    "raven-output|raven-output|raven-output|Display layout and scaling utility"
    "raven-open|raven-open|raven-open|Raven Open - opens files and links; the system's xdg-open"
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
    "SETTINGS|RavenSettingsUI|raven-settings,raven-keycast|gui/raven-settings|Raven Settings - network, sound, screens and updates, with its key overlay"
    "STORE|RavenStore|raven-store|gui/raven-store|Raven Store - the graphical front-end for rvn"
    "BATTERY|RavenBatteryManagement|raven-power|gui/raven-power|Raven Power - battery profiles and energy use"
    "GAMING|RavenGaming|raven-gaming|gui/raven-gaming|Raven Gaming - graphics drivers, game readiness and capture"
    "CONTROLS|RavenControls|raven-controls,raven-controlsd|gui/raven-controls|Raven Controls - keyboard backlight, fans and thermals, with its daemon"
    "VIEWER|RavenViewer|raven-viewer|gui/raven-viewer|Raven Viewer - the PDF and DOCX reader"
    "EAGLEEYE|EagleEye|eagleeye|gui/eagleeye|EagleEye - the image viewer"
    "PLAYER|OwlPlayer|owl-player|gui/owl-player|Owl Player - the media player"
    "CAMERA|RavenCamera|raven-camera|gui/raven-camera|Raven Camera - photos, video, screenshots and screen recording"
    "LOGIN|RavenLogin|ravend,raven-greeter,raven-lock|gui/ravenlogin|Raven Login - the display manager, its greeter and the lock screen"
    "CANVAS|RavenCanvas|ravencanvasd,ravencanvas|gui/ravencanvas|Raven Canvas - the wallpaper daemon and its CLI"
    "ROOSTBAR|RoostBar|roostbar|gui/roostbar|RoostBar - the layer-shell status bar"
)

# Applications the ISO carries for the installer to offer, and nothing else.
# Same row format as GUI_APPS, and the same <KEY>_* variables through
# raven_gui_app_vars -- but they are not part of the system: the GUI stage
# builds each into its own tree under /usr/share/raven/optional/<id>/root,
# where the live session neither runs nor advertises it, and raven-install
# copies a tree onto the disk only when somebody switches it on. So they are
# not in raven_gui_binaries either: nothing expects them in /usr/bin.
#
# The installer's id for each is the key, lowercased.
declare -a OPTIONAL_APPS=(
    "TUTORIAL|RavenTutorial|raven-tutorial|gui/raven-tutorial|Raven Tutorial - a guided first tour of the desktop"
    "ORACLE|Oracle|oracle,raven-oracle|gui/raven-oracle|Oracle - a local troubleshooting companion, command line and app"
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

# The graphical installer. Built by stage-gui.sh's stage_installer_ui from
# installer-ui/ in THIS repository rather than cloned, so it is not a GUI_APPS
# row -- exactly the same reason RAVEN_INIT_BINARIES above is a constant and
# not a row. And for the same reason it needs to be here: the check has to know
# about everything the stages install, not just everything they clone.
#
# It was missing from this file, and that is not a hypothetical. stage_installer_ui
# builds it, installs it to /usr/bin and writes its .desktop -- and stage4's
# check_sysroot_layers looked for fifteen GUI binaries, none of them this one.
# When the GTK4 toolkit was absent from the build container the installer was
# skipped along with the other five GTK applications, and the one function whose
# entire job is to notice a missing component reported the GUI layer complete.
# An ISO that cannot install itself, built green.
GUI_INSTALLER_BINARIES="raven-installer-ui"

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

# Every base-layer program /etc/raven/init.toml expects to be able to exec.
raven_base_binaries() {
    printf '%s\n' "${RAVEN_BASE_BINARIES//,/$'\n'}"
}

# Every base-layer file the image is not usable without, one per line.
raven_base_files() {
    printf '%s\n' "${RAVEN_BASE_FILES//,/$'\n'}"
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
    printf '%s\n' "${GUI_INSTALLER_BINARIES//,/$'\n'}"
}

# The repository a Raven-layer or GUI component is cloned from.
#   raven_component_url <repo>
raven_component_url() {
    printf 'https://github.com/%s/%s.git\n' "${RAVEN_GITHUB_OWNER}" "$1"
}

# Populates <KEY>_REPO, <KEY>_URL, <KEY>_BINARIES and <KEY>_MANIFEST from the
# GUI_APPS (or OPTIONAL_APPS) row for
# <KEY>, so the GUI stage names each repository once -- here -- and every
# fetch, build and install site reads it back out.
#
# Returns 1 for a key with no row, which is a programming error in the caller
# rather than a build failure: the stage aborts on it instead of quietly
# cloning "https://github.com/javanhut/.git".
raven_gui_app_vars() {
    local key="$1" spec rowkey repo binaries manifest

    for spec in "${GUI_APPS[@]}" "${OPTIONAL_APPS[@]}"; do
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
# Manifest pins are OPT-IN. By default every component tracks its default
# branch, because that is what a tree of nine first-party repositories under
# active development actually wants: a fix committed to CAW or RavenGUI reaches
# the next ISO without anyone remembering to bump a hash. The pins stayed in
# the manifests -- they still record the last commit known to build, and they
# are still what `check-manifests.sh` reads -- but nothing consults them for a
# checkout unless asked.
#
#   RAVEN_USE_MANIFEST_PINS=1     honour the [source] pins; reproducible build
#   RAVEN_IGNORE_MANIFEST_PINS=1  force branch tracking even if the above is set
#
# The cost is real and worth stating: two ISOs built from an unchanged tree a
# week apart can contain different software. That is the trade this default
# makes -- current over reproducible. Set RAVEN_USE_MANIFEST_PINS=1 for a
# release build, where it is the wrong way round.

# Whether a checkout should come from the manifest pin rather than the branch.
# Two variables rather than one so the older, negative spelling keeps working
# and still wins: a script that set it to force the tip must not start
# honouring pins because the default moved underneath it.
raven_manifest_pins_enabled() {
    [[ "${RAVEN_IGNORE_MANIFEST_PINS:-0}" == "1" ]] && return 1
    [[ "${RAVEN_USE_MANIFEST_PINS:-0}" == "1" ]]
}

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

    if raven_manifest_pins_enabled; then
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
# may continue to collect failures; stage4 rejects missing required components.
raven_fetch_repo() {
    local name="$1" url="$2" dest="$3" ref="$4" offline="$5" origin="${6:-}"
    if [[ -n "${RAVEN_SOURCE_LOCK:-}" ]]; then
        [[ -r "${RAVEN_SOURCE_LOCK}" ]] || { log_warn "Source lock is unreadable"; return 1; }
        local locked
        locked="$(awk -F '\t' -v name="${name}" '$1 == name {print $3}' "${RAVEN_SOURCE_LOCK}")"
        raven_ref_is_sha "${locked}" || { log_warn "${name}: missing or invalid source lock entry"; return 1; }
        if [[ -n "${ref}" && "${ref}" != "${locked}" ]]; then
            log_warn "${name}: source lock conflicts with requested ref ${ref}"
            return 1
        fi
        ref="${locked}"
    fi
    local parent; parent="$(dirname "${dest}")"

    if [[ "${offline}" == "1" ]]; then
        if [[ -d "${dest}/.git" ]]; then
            log_info "  offline: using existing clone of ${name}"
            raven_verify_source "${name}" "${url}" "${dest}" "${ref}"
            return $?
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
               && git remote set-url origin "${url}" \
               && git fetch --tags --depth 1 origin "${ref:-HEAD}" 2>/dev/null \
               && git reset --hard FETCH_HEAD >/dev/null 2>&1 ); then
            log_warn "  could not update ${name}; refusing stale source"
            return 1
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
            git clone --depth 1 --branch "${ref}" -q "${url}" "${dest}" || return 1
        else
            git clone --depth 1 -q "${url}" "${dest}" || return 1
        fi
    fi

    if raven_ref_is_sha "${ref}"; then
        raven_verify_source "${name}" "${url}" "${dest}" "${ref}"
    else
        # An explicit branch/tag was selected by the successful fetch/clone;
        # a local branch name can still point at the previous checkout.
        raven_verify_source "${name}" "${url}" "${dest}" HEAD
    fi
}

# Record the exact source used; never claim that a failed pin was honored.
raven_verify_source() {
    local name="$1" url="$2" dest="$3" ref="$4" head wanted
    head="$(git -C "${dest}" rev-parse HEAD)" || return 1
    if [[ -n "${ref}" ]]; then
        wanted="$(git -C "${dest}" rev-parse "${ref}^{commit}" 2>/dev/null)" || {
            log_warn "${name}: requested ref ${ref} is unavailable"
            return 1
        }
        if [[ "${wanted}" != "${head}" ]]; then
            log_warn "${name}: requested ${ref} (${wanted}), found ${head}"
            return 1
        fi
    fi
    # Cargo protocol regeneration may touch timestamps, but source edits must
    # not be silently folded into a supposedly pinned build.
    if [[ -n "$(git -C "${dest}" status --porcelain --untracked-files=no)" ]]; then
        log_warn "${name}: source checkout has tracked modifications"
        return 1
    fi
    log_info "  ${name} @ ${head}"
    local records="${SYSROOT_DIR}/usr/share/raven/build/sources"
    mkdir -p "${records}"
    printf '%s\t%s\t%s\n' "${name}" "${url}" "${head}" > "${records}/${name}.tsv"
}

# =============================================================================
# Versions
# =============================================================================
# What a package built from a component should call itself.
#
# WHY THIS EXISTS
#
# Every first-party manifest under packages/ says `version = "0.1.0"` (or
# "0.0.1", which is the same statement said quieter). That was harmless while
# nothing built packages. It stops being harmless the moment `rvn build` runs
# over these manifests, because that string is the whole of what rvn knows
# about a package: `crate::version::vercmp` decides every upgrade from it, and
# two ISOs built a month apart would produce two different `crow` packages
# both calling themselves 0.1.0-1. rvn would consider the second one already
# installed and do nothing, which is the worst available outcome -- not an
# error, just an upgrade that silently never happens.
#
# WHY NOT THE MANIFEST PIN
#
# The obvious version is the [source] commit in the manifest, and it would be
# wrong. Manifest pins are OPT-IN (see raven_manifest_pins_enabled above):
# unless RAVEN_USE_MANIFEST_PINS=1 is set, raven_resolve_ref returns `head`
# and every component is cloned at its default branch. A version built from
# the pin would therefore name a commit that, on a default build, was not the
# one compiled -- a version string that is precisely and confidently false.
# The revision here comes from the checkout's own HEAD, after the fetch, which
# is the one thing that is true whichever way the pin policy is set.
#
# WHY A DATE AND NOT A COUNT
#
# The conventional shape for a package built from a moving branch is
# `<upstream>+r<n>.g<short>`, where <n> is `git rev-list --count HEAD`. That
# number is not available here: raven_fetch_repo clones with --depth 1 on
# every path it has, so the count is 1 in every checkout this build produces.
# Making it available means unshallowing nine Raven repositories and eighteen
# GUI ones on every build, which is the whole of what --depth 1 was there to
# avoid, for a number whose only job is to sort.
#
# So <n> is the commit's own committer date instead, as UTC YYYYMMDDHHMMSS.
# It is already in the shallow clone, it needs no extra fetch, and under
# rpmvercmp it sorts exactly as the count would: the digits are one numeric
# segment, compared by value, and it only ever increases along a branch.
#
# Committer date rather than author date (%cd, not %ad) because a rebase or a
# cherry-pick preserves the author date -- two genuinely different commits can
# carry the same one, and a version must not repeat. The committer date is
# when this commit entered this history, which is the question being asked.
#
# UTC rather than local time because the same commit must produce the same
# version on a build machine in any timezone.
#
# THE SHAPE, AND WHAT EACH PART IS FOR
#
#   <upstream>+r<YYYYMMDDHHMMSS>.g<12 hex>[.dirty]
#
# The `+` is what makes the result sort above the bare upstream version under
# rpmvercmp, so a stamped package upgrades an unstamped one rather than the
# other way round. The `r` part is the ordering and the `g` part is the
# identity: two commits made in the same second sort arbitrarily against each
# other, but they are still distinguishable, and the full 40-character id is
# in the sources record raven_verify_source writes either way. The width is
# fixed at 12 rather than left to `git --short`, which auto-scales with the
# size of the repository: a width that grows as a component gains commits
# would give one commit two different version strings over time.
#
# No `-` appears anywhere in it, deliberately: rvn refuses a version
# containing one, because that character separates version from release.

# The upstream version a component's manifest declares -- the base a build
# revision is appended to, and the whole version for a component that has no
# checkout to read.
#
# Scoped to the [package] table for the same reason raven_manifest_ref is
# scoped to [source]: `version` is a plausible key in more than one table, and
# one picked up from the wrong place would be a silently misnamed package
# rather than an error.
#   raven_manifest_version <manifest-path-under-packages>
raven_manifest_version() {
    local rel="$1"
    [[ -n "${rel}" ]] || return 1

    local file="${PROJECT_ROOT:-.}/packages/${rel}/package.toml"
    [[ -f "${file}" ]] || return 1

    awk '
        /^[[:space:]]*\[/ { inpackage = ($0 ~ /^[[:space:]]*\[package\]/); next }
        !inpackage { next }
        /^[[:space:]]*version[[:space:]]*=/ {
            if (found) next
            val = $0; sub(/^[^=]*=[[:space:]]*/, "", val)
            sub(/[[:space:]]*#.*$/, "", val)
            gsub(/"/, "", val); gsub(/[[:space:]]/, "", val)
            if (val == "") next
            version = val; found = 1
        }
        # The answer is printed here and not at the match, because awk runs END
        # after an `exit` in a rule: printing there and exiting 0 would still
        # leave this block to overwrite the status with 1.
        END {
            if (found) { print version; exit 0 }
            exit 1
        }
    ' "${file}"
}

# The revision part for a checkout: r<date>.g<short>, with `.dirty` appended
# when the tree has been edited since that commit.
#
# Returns 1 rather than guessing for anything that is not a git checkout, for
# a git that cannot read it, and for a HEAD whose date or id does not come
# back in the expected shape. A caller that gets nothing here must not fall
# back to the bare manifest version: that is the collision this whole section
# exists to prevent.
#
# The `.dirty` suffix matters even though raven_verify_source already refuses
# a checkout with tracked modifications, because this function is also useful
# on a tree nobody fetched -- a developer's local clone pointed at by
# RAVEN_<KEY>_REF workflows, or a component built by hand. A version that
# names a commit the built tree does not match is the same lie as the pin.
#   raven_source_revision <checkout>
raven_source_revision() {
    local dest="$1" stamp short

    [[ -n "${dest}" && -d "${dest}/.git" ]] || return 1
    command -v git &>/dev/null || return 1

    # format-local reads the timezone out of the environment, which is why TZ
    # is set on the command rather than trusted.
    stamp="$(TZ=UTC git -C "${dest}" show -s \
        --date=format-local:%Y%m%d%H%M%S --format=%cd HEAD 2>/dev/null)" || return 1
    short="$(git -C "${dest}" rev-parse --short=12 HEAD 2>/dev/null)" || return 1

    # A malformed piece would produce an archive whose name cannot be parsed
    # back into a version, so it is an error here instead.
    [[ "${stamp}" =~ ^[0-9]{14}$ ]] || return 1
    [[ "${short}" =~ ^[0-9a-f]{7,40}$ ]] || return 1

    if [[ -n "$(git -C "${dest}" status --porcelain --untracked-files=no 2>/dev/null)" ]]; then
        printf 'r%s.g%s.dirty\n' "${stamp}" "${short}"
    else
        printf 'r%s.g%s\n' "${stamp}" "${short}"
    fi
}

# The version a package built from a component should carry: what its manifest
# calls upstream, plus the revision of the tree that was actually compiled.
#
#   raven_component_version <manifest-path-under-packages> [checkout]
#
# With a checkout, a revision that cannot be read is a failure -- see
# raven_source_revision. Without one, the manifest version is the whole
# answer, which is the right reading for an in-tree component: the seven init
# binaries, faced, the installer UI and the configs/ shell tools are built
# from this repository rather than cloned, their manifests say
# `type = "local"` in [source], and their cadence is the distribution's own
# RAVEN_VERSION. There is no component commit to name because there is no
# component repository. This repository is tracked by ivaldi rather than git,
# so there is no revision to read on this side either; if in-tree packages
# ever need one, it has to come from ivaldi and RAVEN_VERSION is the honest
# answer until then.
raven_component_version() {
    local manifest="$1" dest="${2:-}" upstream revision

    upstream="$(raven_manifest_version "${manifest}")" || return 1

    [[ -n "${dest}" ]] || { printf '%s\n' "${upstream}"; return 0; }

    revision="$(raven_source_revision "${dest}")" || return 1
    printf '%s+%s\n' "${upstream}" "${revision}"
}

# Records the version a component was packaged at, beside the commit record
# raven_verify_source writes.
#
# A separate file rather than a fourth column in sources/<name>.tsv, because
# that record has readers that count its fields: check-desktop-image.py
# rejects an image whose provenance record is not exactly three
# tab-separated fields, and scripts/test-desktop-build.py writes three. A
# fourth column would turn every build red.
#   raven_record_version <name> <version>
raven_record_version() {
    local name="$1" version="$2"
    [[ -n "${name}" && -n "${version}" ]] || return 1

    # No sysroot means no image to record into, which is a caller error
    # rather than something to write to /usr/share on the build host.
    [[ -n "${SYSROOT_DIR:-}" ]] || return 1

    local records="${SYSROOT_DIR}/usr/share/raven/build/versions"
    mkdir -p "${records}" || return 1
    printf '%s\n' "${version}" > "${records}/${name}"
}
