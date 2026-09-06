#!/bin/bash
# =============================================================================
# RavenLinux dev install: build the repo's own pieces on a running Raven
# system and put them straight into that system.
# =============================================================================
# The ISO build (imlazy build / iso) is for cutting a new image. This is for
# the loop in between: you are booted into Raven, you edit something in this
# repo, you want the machine you are sitting at to run it. Nothing here goes
# through the container, the musl cross target, or the sysroot -- the running
# host is glibc and the crates build natively in seconds.
#
# Everything installed here is copied from the same repo paths the stage
# scripts copy from, so what you test live is what the next ISO ships.
#
# Usage:
#   scripts/dev-install.sh [target ...] [options]
#
# Targets (default: init installer tools):
#   init        build init/ natively; install raven-init, raven-rc,
#               raven-powerd, raven-ports, raven-timed to /usr/bin
#               (stage-raven.sh:build_raven_init)
#   installer-ui  build installer-ui/ natively; install raven-installer-ui and
#               its launcher entry, icon and metainfo. Not in the default set:
#               it is a GTK4 build, and it is only worth waiting for when the
#               graphical installer is what changed.
#   installer   scripts/installer/* -> /usr/bin, configs/installer/profiles
#               -> /etc/raven/install-profiles (stage4-iso.sh:install_installer)
#   tools       configs/raven-console-font, configs/raven-udev,
#               etc/raven/raven-shell -> /usr/bin
#   configs     etc/raven/{init,power,time}.toml -> /etc/raven,
#               configs/raven/services/*.toml -> /etc/raven/init.d,
#               configs/raven/session.d/* -> /etc/raven/session.d.
#               Diff-only unless --force-configs: the live init.toml carries
#               machine-local edits (hostname, agetty args) that a blind copy
#               would erase.
#   all         every target above
#
# Options:
#   -n, --dry-run        build, then report what would change; install nothing
#   --force-configs      actually overwrite config files under /etc/raven
#   --no-restart         install but do not restart services or reload init
#   --allow-non-raven    skip the "am I on Raven?" check (for testing the script)
#
# Service handling after install: a changed raven-powerd is restarted through
# raven-rc; changed configs trigger `raven-rc reload`; a changed raven-init is
# swapped in with `raven-rc reexec` -- PID 1 execs the new binary in place and
# adopts the running services, so no reboot.
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RAVEN_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
export RAVEN_ROOT
RAVEN_NO_LOG=1
# shellcheck source=lib/logging.sh
source "${SCRIPT_DIR}/lib/logging.sh"

DRY_RUN=0
FORCE_CONFIGS=0
NO_RESTART=0
ALLOW_NON_RAVEN=0
TARGETS=()

for arg in "$@"; do
    case "$arg" in
        -n|--dry-run)      DRY_RUN=1 ;;
        --force-configs)   FORCE_CONFIGS=1 ;;
        --no-restart)      NO_RESTART=1 ;;
        --allow-non-raven) ALLOW_NON_RAVEN=1 ;;
        -h|--help)         sed -n '2,/^# ====.*$/{/^# ====/d;s/^# \{0,1\}//p}' "$0"; exit 0 ;;
        init|installer|installer-ui|tools|configs) TARGETS+=("$arg") ;;
        all)               TARGETS+=(init installer installer-ui tools configs) ;;
        *) log_error "unknown argument: $arg"; exit 2 ;;
    esac
done
(( ${#TARGETS[@]} )) || TARGETS=(init installer tools)

# -----------------------------------------------------------------------------
# Are we on the system we are about to write into?
# -----------------------------------------------------------------------------
on_raven() {
    local id=""
    [[ -r /etc/os-release ]] && id="$(. /etc/os-release && echo "${ID:-}")"
    [[ "$id" == "raven" ]] && return 0
    [[ "$(readlink /proc/1/exe 2>/dev/null)" == */raven-init ]]
}
if ! on_raven && (( ALLOW_NON_RAVEN == 0 )); then
    log_error "This does not look like a running RavenLinux system (ID=raven or raven-init as PID 1)."
    log_error "dev-install writes into the live root. For an ISO use 'imlazy build'; to override, pass --allow-non-raven."
    exit 1
fi

# -----------------------------------------------------------------------------
# Privilege: build as the user, install through sudo
#
# The normal invocation is deliberately *not* prefixed with sudo; install_file
# elevates only the copies and service-control calls that need it. Be forgiving
# when the whole script is run through sudo anyway: rustup selects toolchains
# from the effective user's home, and root commonly has no default even though
# the invoking account does. In that case, drop back to SUDO_USER for builds.
# -----------------------------------------------------------------------------
SUDO=()
BUILD_AS=()
if (( EUID != 0 )); then
    SUDO=(sudo)
    if (( DRY_RUN == 0 )) && ! sudo -n true 2>/dev/null; then
        log_info "sudo is needed to write into /usr/bin and /etc/raven"
        sudo -v
    fi
elif [[ -n "${SUDO_USER:-}" && "${SUDO_USER}" != "root" ]]; then
    BUILD_AS=(sudo -H -u "${SUDO_USER}" --)
fi

# -----------------------------------------------------------------------------
# Install helper: copies only when content differs, records what changed
# -----------------------------------------------------------------------------
CHANGED=()     # destinations that were (or would be) replaced
SKIPPED_CFG=() # configs that differ but were not written (no --force-configs)

# install_file <src> <dest> <mode>
install_file() {
    local src="$1" dest="$2" mode="$3"
    if [[ ! -e "$src" ]]; then
        log_warn "  missing in repo, skipped: ${src#"${RAVEN_ROOT}"/}"
        return 0
    fi
    if [[ -e "$dest" ]] && cmp -s "$src" "$dest"; then
        [[ "${RAVEN_LOG_VERBOSE:-0}" == "1" ]] && log_info "  unchanged: $dest"
        return 0
    fi
    CHANGED+=("$dest")
    if (( DRY_RUN )); then
        log_info "  would update: $dest"
        [[ -e "$dest" ]] && diff -u "$dest" "$src" | head -n 40 | sed 's/^/      /' || true
        return 0
    fi
    "${SUDO[@]}" install -D -m "$mode" "$src" "$dest"
    log_success "  updated: $dest"
}

# install_config <src> <dest> [mode]: like install_file but never overwrites
# without --force-configs; shows the diff instead.
install_config() {
    local src="$1" dest="$2" mode="${3:-0644}"
    if [[ -e "$dest" ]] && ! cmp -s "$src" "$dest" && (( FORCE_CONFIGS == 0 )); then
        SKIPPED_CFG+=("$dest")
        log_warn "  differs (not written, use --force-configs): $dest"
        diff -u "$dest" "$src" | head -n 40 | sed 's/^/      /' || true
        return 0
    fi
    install_file "$src" "$dest" "$mode"
}

# -----------------------------------------------------------------------------
# Targets
# -----------------------------------------------------------------------------
do_init() {
    log_section "init crate (raven-init, raven-rc, raven-powerd, raven-ports, raven-timed)"
    local src="${RAVEN_ROOT}/init"
    ( cd "$src" && "${BUILD_AS[@]}" cargo build --release --locked ) || {
        log_error "cargo build failed; nothing installed"
        return 1
    }
    local out="${src}/target/release"
    for b in raven-init raven-rc raven-powerd raven-ports raven-timed; do
        install_file "${out}/${b}" "/usr/bin/${b}" 0755
    done
}

do_installer() {
    log_section "installer"
    local s="${RAVEN_ROOT}/scripts/installer"
    install_file "${s}/raven-install"        /usr/bin/raven-install        0755
    install_file "${s}/raven-postinstall"    /usr/bin/raven-postinstall    0755
    install_file "${s}/raven-desktopinstall" /usr/bin/raven-desktopinstall 0755
    local p
    for p in "${RAVEN_ROOT}"/configs/installer/profiles/*.packages; do
        [[ -e "$p" ]] || continue
        install_file "$p" "/etc/raven/install-profiles/$(basename "$p")" 0644
    done
}

# The graphical installer. Separate from `installer` because it is a cargo
# build against GTK4 and the shell scripts above are a copy -- somebody fixing
# a prompt in raven-install should not wait for libadwaita to link.
#
# Not in the default target list for the same reason.
do_installer_ui() {
    log_section "installer-ui"
    local src="${RAVEN_ROOT}/installer-ui"

    if [[ ! -f "${src}/Cargo.toml" ]]; then
        log_warn "installer-ui/ is not in this tree; skipped"
        return 0
    fi
    local mod missing=()
    for mod in gtk4 libadwaita-1 glib-2.0 gio-2.0; do
        pkg-config --exists "$mod" 2>/dev/null || missing+=("$mod")
    done
    if (( ${#missing[@]} )); then
        log_error "missing build dependencies: ${missing[*]}"
        return 1
    fi

    ( cd "$src" && "${BUILD_AS[@]}" cargo build --release --locked ) || {
        log_error "cargo build failed; nothing installed"
        return 1
    }
    install_file "${src}/target/release/raven-installer-ui" /usr/bin/raven-installer-ui 0755
    install_file "${src}/data/com.raveninstaller.Raven.desktop" \
        /usr/share/applications/com.raveninstaller.Raven.desktop 0644
    install_file "${src}/data/icons/hicolor/scalable/apps/com.raveninstaller.Raven.svg" \
        /usr/share/icons/hicolor/scalable/apps/com.raveninstaller.Raven.svg 0644
    install_file "${src}/data/com.raveninstaller.Raven.metainfo.xml" \
        /usr/share/metainfo/com.raveninstaller.Raven.metainfo.xml 0644
}

do_tools() {
    log_section "repo-sourced tools"
    install_file "${RAVEN_ROOT}/configs/raven-console-font" /usr/bin/raven-console-font 0755
    install_file "${RAVEN_ROOT}/configs/raven-udev"         /usr/bin/raven-udev         0755
    install_file "${RAVEN_ROOT}/etc/raven/raven-shell"      /usr/bin/raven-shell        0755
    local rule
    for rule in "${RAVEN_ROOT}"/configs/udev/*.rules; do
        [[ -e "$rule" ]] || continue
        install_file "$rule" "/usr/lib/udev/rules.d/$(basename "$rule")" 0644
    done
}

do_configs() {
    log_section "configs under /etc/raven"
    install_config "${RAVEN_ROOT}/etc/raven/init.toml"  /etc/raven/init.toml
    install_config "${RAVEN_ROOT}/etc/raven/power.toml" /etc/raven/power.toml
    install_config "${RAVEN_ROOT}/etc/raven/time.toml"  /etc/raven/time.toml
    local f
    for f in "${RAVEN_ROOT}"/configs/raven/services/*.toml; do
        [[ -e "$f" ]] || continue
        install_config "$f" "/etc/raven/init.d/$(basename "$f")"
    done
    for f in "${RAVEN_ROOT}"/configs/raven/session.d/*; do
        [[ -e "$f" ]] || continue
        install_config "$f" "/etc/raven/session.d/$(basename "$f")" 0755
    done
}

for t in "${TARGETS[@]}"; do
    # installer-ui -> do_installer_ui. A target name is what someone types on
    # the command line; a function name cannot carry a hyphen comfortably.
    "do_${t//-/_}"
done

# -----------------------------------------------------------------------------
# Make the changes live
# -----------------------------------------------------------------------------
changed() { local d; for d in "${CHANGED[@]:-}"; do [[ "$d" == "$1" ]] && return 0; done; return 1; }

log_section "summary"
if (( ${#CHANGED[@]} == 0 )); then
    log_info "nothing differed from the running system"
else
    printf '  %s\n' "${CHANGED[@]}"
fi
(( ${#SKIPPED_CFG[@]} )) && log_warn "${#SKIPPED_CFG[@]} config file(s) differ but were left alone (re-run with --force-configs)"

if (( DRY_RUN )); then
    log_info "dry run: nothing was written"
    exit 0
fi

if (( NO_RESTART == 0 )); then
    if changed /usr/bin/raven-powerd; then
        log_step "restarting powerd"
        "${SUDO[@]}" raven-rc restart powerd || log_warn "raven-rc restart powerd failed"
    fi
    if changed /usr/bin/raven-ports; then
        log_step "restarting ports"
        "${SUDO[@]}" raven-rc restart ports || log_warn "raven-rc restart ports failed"
    fi
    if changed /usr/bin/raven-timed; then
        log_step "restarting timed"
        # On a machine that predates the service, restart has nothing to
        # restart until the drop-in is in place -- that is the configs target.
        "${SUDO[@]}" raven-rc restart timed \
            || log_warn "raven-rc restart timed failed (new here? run: imlazy dev configs, then sudo raven-rc start timed)"
    fi
    config_changed=0
    for d in "${CHANGED[@]:-}"; do [[ "$d" == /etc/raven/* ]] && config_changed=1; done
    if (( config_changed )); then
        log_step "reloading init configuration"
        "${SUDO[@]}" raven-rc reload || log_warn "raven-rc reload failed"
    fi
fi

if changed /usr/bin/raven-init; then
    if (( NO_RESTART )); then
        log_warn "raven-init replaced on disk; run 'sudo raven-rc reexec' to swap PID 1"
    else
        log_step "re-executing PID 1 with the new raven-init"
        # The running init may predate the verb; then the only way is a reboot.
        if ! "${SUDO[@]}" raven-rc reexec; then
            log_warn "raven-rc reexec failed; the running init may be too old for it -- reboot to pick up the new raven-init"
        fi
    fi
fi
if changed /usr/bin/raven-shell; then
    log_info "raven-shell updated; open a new login/terminal to pick it up"
fi
if changed /usr/bin/raven-udev || changed /usr/bin/raven-console-font; then
    log_info "boot-time service scripts updated; they run again on next boot (or: sudo raven-rc restart <name>)"
fi
