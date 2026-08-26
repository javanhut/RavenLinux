#!/bin/bash
# =============================================================================
# RavenLinux - apply the rootfs skeleton to an existing tree
# =============================================================================
# The build applies the skeleton on its own (see the call sites in build.sh,
# stage1-base.sh, stage2-native.sh, stage4-iso.sh and dev-env.sh), so this
# script exists for the case the build does not cover: a sysroot that already
# exists and predates the skeleton, where a full rebuild is not worth the wall
# clock. It creates what is absent and corrects modes that have drifted.
#
# Usage: ./scripts/apply-skeleton.sh [OPTIONS] [ROOTFS]
#
# Options:
#   -n, --dry-run   Report what would change, change nothing
#   -h, --help      Show this help message
#
# ROOTFS defaults to ${SYSROOT_DIR:-build/sysroot}.
#
# A built sysroot is owned by root, so this needs root to write to it. Rather
# than fail with a wall of EACCES, it re-executes itself under sudo when the
# target is not writable -- and does nothing of the sort when it already has
# the access it needs, which is the case inside the build container.
#
# Exits non-zero if the skeleton could not be applied, or if the layout check
# still fails afterwards.
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${RAVEN_ROOT:-$(dirname "$SCRIPT_DIR")}"

DRY_RUN=false
ROOTFS_ARG=""
askpass=""
candidate=""

show_help() {
    sed -n '2,26p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        -n|--dry-run) DRY_RUN=true; shift ;;
        -h|--help)    show_help; exit 0 ;;
        -*)           echo "Unknown option: $1" >&2; show_help; exit 1 ;;
        *)
            if [[ -n "$ROOTFS_ARG" ]]; then
                echo "Too many arguments: $1" >&2
                exit 1
            fi
            ROOTFS_ARG="$1"; shift
            ;;
    esac
done

TARGET="${ROOTFS_ARG:-${SYSROOT_DIR:-${PROJECT_ROOT}/build/sysroot}}"

if [[ ! -d "$TARGET" ]]; then
    echo "Not a directory: ${TARGET}" >&2
    echo "Build a sysroot first (imlazy stage1), or name one explicitly." >&2
    exit 1
fi

# Absolute, so the re-exec under sudo does not depend on the working directory
# surviving the transition.
TARGET="$(cd "$TARGET" && pwd -P)"

# -----------------------------------------------------------------------------
# Escalate only if we actually have to.
# -----------------------------------------------------------------------------
# Writability of the root directory is the cheap proxy: a build sysroot is
# root-owned throughout, and inside the build container this script already
# runs as uid 0, where invoking sudo would be pointless and often absent.
if [[ "$DRY_RUN" != "true" && ! -w "$TARGET" ]] && [[ "$(id -u)" -ne 0 ]]; then
    if ! command -v sudo >/dev/null 2>&1; then
        echo "${TARGET} is not writable and sudo is not installed." >&2
        echo "Re-run as root." >&2
        exit 1
    fi
    # -E so RAVEN_ROOT and SYSROOT_DIR survive. The resolved target is passed
    # rather than the original argument, since sudo may not preserve the cwd.
    #
    # With no controlling terminal, plain `sudo` cannot read a password and
    # dies with "a terminal is required" -- which is what happens when this
    # runs from a tool harness or a CI step rather than a shell. sudo can use a
    # graphical askpass helper instead, but only if one is named AND there is a
    # display to draw it on; a helper with no display fails just as opaquely.
    if [[ -t 0 ]]; then
        echo "==> ${TARGET} is root-owned; re-running under sudo"
        exec sudo -E "${BASH_SOURCE[0]}" "$TARGET"
    fi

    askpass="${SUDO_ASKPASS:-}"
    if [[ -z "$askpass" ]]; then
        for candidate in /usr/bin/ksshaskpass /usr/bin/ssh-askpass \
                         /usr/lib/ssh/ssh-askpass /usr/libexec/openssh/gnome-ssh-askpass; do
            [[ -x "$candidate" ]] && { askpass="$candidate"; break; }
        done
    fi

    if [[ -n "$askpass" && ( -n "${DISPLAY:-}" || -n "${WAYLAND_DISPLAY:-}" ) ]]; then
        echo "==> ${TARGET} is root-owned and there is no terminal here;"
        echo "    asking for the password via ${askpass}"
        exec env SUDO_ASKPASS="$askpass" sudo -A -E "${BASH_SOURCE[0]}" "$TARGET"
    fi

    # Nothing left that can prompt. Say so precisely, and name the paths that
    # do work, rather than letting sudo emit its own error two frames down.
    echo "${TARGET} is root-owned, and this session has no terminal to read a" >&2
    echo "password on$([[ -n "$askpass" ]] && echo " and no display for ${askpass}")." >&2
    echo "" >&2
    echo "Any of these work:" >&2
    echo "  * run 'sudo ./scripts/apply-skeleton.sh' from a normal terminal" >&2
    echo "  * run './scripts/apply-skeleton.sh' inside the build container," >&2
    echo "    which is already root:  imlazy shell" >&2
    echo "  * skip it entirely -- the next build applies the skeleton itself" >&2
    echo "" >&2
    echo "'--dry-run' needs no privileges and reports what would change." >&2
    exit 1
fi

# shellcheck disable=SC1091
[[ -f "${PROJECT_ROOT}/scripts/lib/logging.sh" ]] && source "${PROJECT_ROOT}/scripts/lib/logging.sh"

if [[ ! -f "${PROJECT_ROOT}/scripts/lib/skeleton.sh" ]]; then
    echo "scripts/lib/skeleton.sh is missing; nothing to apply." >&2
    exit 1
fi
# shellcheck disable=SC1091
source "${PROJECT_ROOT}/scripts/lib/skeleton.sh"

echo ""
echo "Rootfs: ${TARGET}"
echo ""

if [[ "$DRY_RUN" == "true" ]]; then
    echo "Dry run -- reporting what is missing or wrong, changing nothing."
    echo ""
    if raven_skeleton_verify "$TARGET"; then
        echo ""
        echo "Nothing to do."
        exit 0
    fi
    echo ""
    echo "Re-run without --dry-run to apply."
    exit 1
fi

if ! raven_skeleton_root "$TARGET"; then
    echo "" >&2
    echo "The skeleton could not be applied in full. The errors above name the" >&2
    echo "paths that need a decision -- typically a populated directory sitting" >&2
    echo "where a symlink belongs, which is not safe to remove automatically." >&2
    exit 1
fi

echo ""
echo "Skeleton applied. Verifying the full layout..."
echo ""

# The merge check and the completeness check both matter, and check-layout.sh
# runs both. Applying without verifying is how a tree ends up "fixed" and still
# broken in a way nothing noticed.
if [[ -x "${PROJECT_ROOT}/scripts/check-layout.sh" ]]; then
    exec "${PROJECT_ROOT}/scripts/check-layout.sh" "$TARGET"
fi

raven_skeleton_verify "$TARGET"
