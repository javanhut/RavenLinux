#!/bin/bash
# =============================================================================
# RavenLinux Root Filesystem -> Container Image
# =============================================================================
# Packages the *built RavenLinux root filesystem* (build/sysroot) as a runnable
# container image. This is RavenLinux itself — NOT the Arch-based builder. Use
# it to run RavenLinux in a container and test terminal tools without booting
# the ISO:
#
#   ./scripts/build.sh all        # (or: make build)  -> populates the sysroot
#   ./scripts/export-rootfs.sh    # -> image 'ravenlinux:latest'
#   docker run --rm -it --platform linux/amd64 ravenlinux
#
# Environment:
#   RAVEN_ENGINE         docker | podman (default: auto-detect)
#   RAVEN_IMAGE          builder image used to read the sysroot (default: ravenlinux-build)
#   RAVEN_ROOTFS_IMAGE   output image tag (default: ravenlinux:latest)
#   RAVEN_BUILD_VOLUME   named volume holding the sysroot on macOS (default: raven-build)
#   RAVEN_PLATFORM       image platform (default: linux/amd64; set empty to skip)
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RAVEN_ROOT="$(dirname "$SCRIPT_DIR")"

BUILDER_IMAGE="${RAVEN_IMAGE:-ravenlinux-build}"
ROOTFS_IMAGE="${RAVEN_ROOTFS_IMAGE:-ravenlinux:latest}"
BUILD_VOLUME="${RAVEN_BUILD_VOLUME-raven-build}"
PLATFORM="${RAVEN_PLATFORM-linux/amd64}"

# -----------------------------------------------------------------------------
# Pick a container engine (mirrors docker-build.sh)
# -----------------------------------------------------------------------------
ENGINE="${RAVEN_ENGINE:-}"
if [[ -z "$ENGINE" ]]; then
    if command -v docker &>/dev/null; then ENGINE="docker"
    elif command -v podman &>/dev/null; then ENGINE="podman"
    else echo "ERROR: neither 'docker' nor 'podman' found in PATH." >&2; exit 1
    fi
fi

PLATFORM_FLAGS=()
[[ -n "$PLATFORM" ]] && PLATFORM_FLAGS=(--platform "$PLATFORM")

# -----------------------------------------------------------------------------
# Locate the sysroot. On macOS it lives in a named volume (the bind-mounted
# repo can't be used for the heavy build tree); on Linux it's ./build directly.
# -----------------------------------------------------------------------------
MOUNT=()
if [[ "$(uname -s)" == "Darwin" && -n "$BUILD_VOLUME" ]]; then
    MOUNT=(-v "${BUILD_VOLUME}:/raven/build")
    SYSROOT_LOC="volume '${BUILD_VOLUME}'"
else
    MOUNT=(-v "${RAVEN_ROOT}:/raven")
    SYSROOT_LOC="${RAVEN_ROOT}/build/sysroot"
fi

echo ">> Engine: ${ENGINE}   Builder: ${BUILDER_IMAGE}"
echo ">> Reading RavenLinux sysroot from ${SYSROOT_LOC}"

# Sanity check: a real RavenLinux rootfs has a shell.
if ! "$ENGINE" run --rm "${PLATFORM_FLAGS[@]}" "${MOUNT[@]}" "$BUILDER_IMAGE" \
        bash -c '[[ -e /raven/build/sysroot/bin/bash || -e /raven/build/sysroot/bin/sh ]]'; then
    echo "ERROR: no built RavenLinux sysroot found (no /bin/sh in ${SYSROOT_LOC})." >&2
    echo "       Run a build first:  make build" >&2
    exit 1
fi

echo ">> Packaging sysroot into image '${ROOTFS_IMAGE}'..."

# Stream a tar of the sysroot out of a throwaway builder container and import it
# as a flat image. Virtual filesystems are runtime-only, so keep the mount-point
# directories but drop any contents.
"$ENGINE" run --rm "${PLATFORM_FLAGS[@]}" "${MOUNT[@]}" "$BUILDER_IMAGE" \
    tar --numeric-owner \
        --exclude='./proc/*' --exclude='./sys/*' \
        --exclude='./run/*'  --exclude='./tmp/*' \
        -C /raven/build/sysroot -cf - . \
  | "$ENGINE" import "${PLATFORM_FLAGS[@]}" \
        -c 'CMD ["/bin/bash"]' \
        -c 'ENV PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin' \
        -c 'ENV HOME=/root' \
        -c 'WORKDIR /root' \
        - "$ROOTFS_IMAGE"

echo ""
echo ">> Done. RavenLinux image ready: ${ROOTFS_IMAGE}"
echo ">> Run it:"
if [[ -n "$PLATFORM" ]]; then
    echo "     ${ENGINE} run --rm -it --platform ${PLATFORM} ${ROOTFS_IMAGE}"
else
    echo "     ${ENGINE} run --rm -it ${ROOTFS_IMAGE}"
fi
