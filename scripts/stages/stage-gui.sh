#!/bin/bash
# =============================================================================
# RavenLinux GUI Stage: Compositor and Desktop Shell
# =============================================================================
# Builds RavenGUI -- Huginn, the Wayland compositor, and Muninn, the desktop
# shell it hosts. See REPOSFORRAVEN.md for what they are.
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
# makes it disable the tty1 getty, ensure seatd, create /run/user/0, and start
# /bin/raven-wayland-session with RAVEN_WAYLAND_COMPOSITOR set from
# raven.wayland=<name>. That launcher did not exist until this stage installed
# it. Nothing in init needed changing.
#
# Environment:
#   GUI_SKIP=1                 skip this stage entirely
#   GUI_OFFLINE=1              never touch the network; use the existing clone
#   GUI_REF=<git-ref>          build a particular ref instead of the default
#   GUI_TARGET=<rust-target>   override the host target (rarely wanted)
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

# Huginn links glibc, unlike everything in the Raven layer. The host target is
# the right one: the sysroot carries the host's glibc and its dynamic linker,
# which is how the Xorg and Mesa binaries stage2 copies already work.
GUI_TARGET="${GUI_TARGET:-$(rustc -vV 2>/dev/null | awk '/^host:/{print $2}')}"

# key|package|binary|description
GUI_COMPONENTS=(
    "huginn-comp|huginn-comp|huginn|Huginn - Wayland compositor"
    "muninn|muninn|muninn|Muninn - desktop shell: panel, launcher, notifications"
    "muninn-lock|muninn-lock|muninn-lock|Muninn Lock - session lock screen"
)

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
    # Module names as pkg-config knows them, which is not always the soname:
    # Mesa's GBM ships gbm.pc, not libgbm.pc.
    for lib in libdrm libinput libudev gbm egl; do
        pkg-config --exists "${lib}" 2>/dev/null || missing+=("${lib}")
    done

    # libseat has no .pc on every distribution; look for the library itself.
    if ! pkg-config --exists libseat 2>/dev/null; then
        local found=0
        for dir in /usr/lib /usr/lib64 /usr/lib/x86_64-linux-gnu; do
            [[ -e "${dir}/libseat.so" ]] && found=1 && break
        done
        (( found == 0 )) && missing+=("libseat")
    fi

    if (( ${#missing[@]} > 0 )); then
        log_warn "Missing build dependencies: ${missing[*]}"
        log_info "  install them with: pacman -S --needed libdrm libinput systemd mesa seatd"
        return 1
    fi
    return 0
}

# =============================================================================
# Source fetching
# =============================================================================
fetch_gui_source() {
    local dest="${GUI_SRC_DIR}/${GUI_REPO}"
    local ref="${GUI_REF:-}"

    if [[ "${GUI_OFFLINE:-0}" == "1" ]]; then
        if [[ -d "${dest}/.git" ]]; then
            log_info "  offline: using existing clone of ${GUI_REPO}"
            return 0
        fi
        log_warn "  offline: no clone of ${GUI_REPO} in ${GUI_SRC_DIR}"
        return 1
    fi

    command -v git &>/dev/null || { log_warn "  git not found"; return 1; }
    mkdir -p "${GUI_SRC_DIR}"

    if [[ -d "${dest}/.git" ]]; then
        log_info "  updating ${GUI_REPO}..."
        if ! (cd "${dest}" && git fetch --tags --depth 1 origin "${ref:-HEAD}" 2>/dev/null \
              && git reset --hard FETCH_HEAD >/dev/null 2>&1); then
            log_warn "  could not update ${GUI_REPO}, using the existing checkout"
        fi
    else
        log_info "  cloning ${GUI_REPO}..."
        rm -rf "${dest}"
        if [[ -n "${ref}" ]]; then
            git clone --depth 1 --branch "${ref}" -q "${GUI_URL}" "${dest}" 2>/dev/null \
                || git clone --depth 1 -q "${GUI_URL}" "${dest}" || return 1
        else
            git clone --depth 1 -q "${GUI_URL}" "${dest}" || return 1
        fi
    fi

    local rev
    rev="$(cd "${dest}" && git rev-parse --short HEAD 2>/dev/null || echo unknown)"
    log_info "  ${GUI_REPO} @ ${rev}"
    return 0
}

# =============================================================================
# Build
# =============================================================================
# One cargo invocation for all three binaries: they share a workspace, a
# dependency graph and a target directory, and the committed Cargo.lock was
# resolved against the set.
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
        if [[ -e "${SYSROOT_DIR}/usr/lib/${base}" || -e "${SYSROOT_DIR}/lib/${base}" ]]; then
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

# libinput classifies devices from data files, and libwacom from its own
# database. Without them a touchpad is a generic pointer and a tablet is
# nothing at all -- the compositor runs, the hardware just behaves oddly.
stage_gui_data() {
    local dir
    for dir in /usr/share/libinput /usr/share/libwacom /usr/share/X11/xkb; do
        if [[ -d "${dir}" ]]; then
            mkdir -p "${SYSROOT_DIR}${dir}"
            cp -a "${dir}/." "${SYSROOT_DIR}${dir}/" 2>/dev/null || true
            log_info "  staged ${dir}"
        fi
    done
}

# =============================================================================
# Session launcher
# =============================================================================
# raven-init starts this when the kernel cmdline says raven.graphics=wayland;
# until now it did not exist and init fell through to a /bin/raven-compositor
# that nothing builds. It starts the compositor, waits for its socket, then
# starts the shell as an ordinary client -- which is the whole architecture:
# if muninn dies the session survives, so the launcher restarts it rather than
# taking the compositor down.
install_session_launcher() {
    local dest="${SYSROOT_DIR}/bin/raven-wayland-session"

    mkdir -p "${SYSROOT_DIR}/bin"
    cat > "${dest}" << 'LAUNCHER'
#!/bin/sh
# RavenLinux Wayland session: Huginn, then Muninn.
#
# Started by raven-init when the kernel cmdline carries raven.graphics=wayland.
# RAVEN_WAYLAND_COMPOSITOR comes from raven.wayland=<name> and names the
# compositor binary; huginn is the default and the only one that ships.
set -eu

COMPOSITOR="${RAVEN_WAYLAND_COMPOSITOR:-huginn}"
case "${COMPOSITOR}" in
    raven|raven-compositor|"") COMPOSITOR=huginn ;;
esac

export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/0}"
export LIBSEAT_BACKEND="${LIBSEAT_BACKEND:-seatd}"
mkdir -p "${XDG_RUNTIME_DIR}"
chmod 0700 "${XDG_RUNTIME_DIR}"

command -v "${COMPOSITOR}" >/dev/null 2>&1 || {
    echo "raven-wayland-session: ${COMPOSITOR} is not installed" >&2
    exit 1
}

# --backend udev: this is a TTY, not a nested session. Huginn autodetects from
# an inherited WAYLAND_DISPLAY, which is exactly what a login session lacks,
# but saying so beats depending on the absence of a variable.
"${COMPOSITOR}" --backend udev &
COMPOSITOR_PID=$!

# Wait for the compositor's socket rather than sleeping: the shell cannot
# connect before it exists, and how long that takes depends on the GPU.
i=0
while [ $i -lt 100 ]; do
    for sock in "${XDG_RUNTIME_DIR}"/wayland-* "${XDG_RUNTIME_DIR}"/huginn-*; do
        if [ -S "${sock}" ]; then
            WAYLAND_DISPLAY="$(basename "${sock}")"
            export WAYLAND_DISPLAY
            break 2
        fi
    done
    kill -0 "${COMPOSITOR_PID}" 2>/dev/null || {
        echo "raven-wayland-session: ${COMPOSITOR} exited during startup" >&2
        exit 1
    }
    i=$((i + 1))
    sleep 0.1
done

if [ -z "${WAYLAND_DISPLAY:-}" ]; then
    echo "raven-wayland-session: no compositor socket appeared" >&2
    kill "${COMPOSITOR_PID}" 2>/dev/null || true
    exit 1
fi

# The shell is a separate process on purpose: a crash here costs the panel and
# not the session, so it is restarted in place while the compositor keeps
# running. Give up after a few tries rather than spinning forever.
if command -v muninn >/dev/null 2>&1; then
    (
        tries=0
        while [ $tries -lt 5 ]; do
            muninn || true
            kill -0 "${COMPOSITOR_PID}" 2>/dev/null || exit 0
            tries=$((tries + 1))
            echo "raven-wayland-session: muninn exited, restarting (${tries}/5)" >&2
            sleep 1
        done
    ) &
fi

wait "${COMPOSITOR_PID}"
LAUNCHER

    chmod 0755 "${dest}"
    log_success "  raven-wayland-session installed"
}

# =============================================================================
# Install
# =============================================================================
install_gui_binary() {
    local binary="$1" src="$2"

    mkdir -p "${SYSROOT_DIR}/usr/bin" "${SYSROOT_DIR}/bin"
    install -m 0755 "${src}" "${SYSROOT_DIR}/usr/bin/${binary}"
    ln -sf "../usr/bin/${binary}" "${SYSROOT_DIR}/bin/${binary}"
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
    echo "Session:"
    if [[ -x "${SYSROOT_DIR}/bin/raven-wayland-session" ]]; then
        echo "  [OK] raven-wayland-session (boot with raven.graphics=wayland)"
    else
        echo "  [--] raven-wayland-session"
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

    log_step "RavenGUI (huginn, muninn, muninn-lock)"

    if ! fetch_gui_source; then
        log_warn "  source unavailable, skipping the GUI stage"
        return 0
    fi

    local src="${GUI_SRC_DIR}/${GUI_REPO}"

    log_info "  building for ${GUI_TARGET}..."
    if ! build_gui_workspace "${src}"; then
        log_warn "  build failed, skipping the GUI stage"
        GUI_FAILED=(huginn muninn muninn-lock)
        print_gui_summary
        return 0
    fi

    # All or nothing. A compositor with no shell is a blank screen, which is
    # worse than a console: at least a console tells you what went wrong.
    local -a built_paths=()
    local spec key package binary desc out
    for spec in "${GUI_COMPONENTS[@]}"; do
        IFS='|' read -r key package binary desc <<< "${spec}"
        out="${src}/target/${GUI_TARGET}/release/${binary}"
        if [[ ! -f "${out}" ]]; then
            log_warn "  ${binary}: not produced by the build"
            GUI_FAILED=(huginn muninn muninn-lock)
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

    log_step "Installing the session launcher..."
    install_session_launcher

    print_gui_summary

    log_success "GUI stage complete!"
    echo ""
}

main "$@"
