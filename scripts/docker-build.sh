#!/bin/bash
# =============================================================================
# RavenLinux Containerized Build Helper
# =============================================================================
# Builds RavenLinux inside a Linux container so it can be produced from macOS,
# Windows, or any Linux host. Works with both Docker and Podman.
#
# Usage:
#   ./scripts/docker-build.sh [BUILD_ARGS...]
#
# Examples:
#   ./scripts/docker-build.sh                 # build everything (build.sh all)
#   ./scripts/docker-build.sh image           # build the toolchain image only
#   ./scripts/docker-build.sh stage0          # only the cross toolchain
#   ./scripts/docker-build.sh -j 8 stage1     # stage1 with 8 jobs
#   ./scripts/docker-build.sh --clean all     # clean rebuild
#   ./scripts/docker-build.sh shell           # drop into an interactive shell
#
# Environment:
#   RAVEN_ENGINE   Force the container engine: "docker" or "podman"
#   RAVEN_IMAGE    Image tag to build/use (default: ravenlinux-build)
#   RAVEN_NO_BUILD Set to 1 to skip the image build (assume it exists)
#
# Notes:
#   - The container runs --privileged because the build uses chroot, overlayfs
#     mounts and loop devices.
#   - The repository is bind-mounted at /raven, so the resulting ISO
#     (raven-<ver>-<arch>.iso) lands in the repo root on the host.
#   - On macOS the build's working tree (build/) is kept on a native engine
#     volume instead of the virtiofs bind mount — virtiofs mishandles the
#     symlinks an LFS build unpacks. See the volume note below. On Linux the
#     bind mount is native, so build/ stays directly on the host as before.
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RAVEN_ROOT="$(dirname "$SCRIPT_DIR")"
IMAGE="${RAVEN_IMAGE:-ravenlinux-build}"
# Dockerfile to build the toolchain image from. Override with RAVEN_DOCKERFILE
# (e.g. Dockerfile.minimal for the slim headless image).
DOCKERFILE="${RAVEN_DOCKERFILE:-Dockerfile}"

# RavenLinux only targets x86_64 (the Arch base image is x86_64-only). On an
# arm64 host (e.g. Apple Silicon) the image runs under emulation; make the
# platform explicit so `run` matches the amd64 image. Override with
# RAVEN_PLATFORM= (empty) to let the engine choose.
PLATFORM="${RAVEN_PLATFORM-linux/amd64}"
PLATFORM_FLAGS=()
[[ -n "$PLATFORM" ]] && PLATFORM_FLAGS=(--platform "$PLATFORM")

# -----------------------------------------------------------------------------
# Pick a container engine
# -----------------------------------------------------------------------------
ENGINE="${RAVEN_ENGINE:-}"
if [[ -z "$ENGINE" ]]; then
    if command -v docker &>/dev/null; then
        ENGINE="docker"
    elif command -v podman &>/dev/null; then
        ENGINE="podman"
    else
        echo "ERROR: neither 'docker' nor 'podman' found in PATH." >&2
        echo "Install Docker Desktop or Podman, then re-run." >&2
        exit 1
    fi
fi
echo ">> Using container engine: ${ENGINE}"

# -----------------------------------------------------------------------------
# `image` subcommand: build the toolchain image only, then exit.
# -----------------------------------------------------------------------------
# Lets callers (and the Makefile) pre-build the Linux build host without running
# a build or dropping into a shell.
if [[ "${1:-}" == "image" ]]; then
    echo ">> Building image '${IMAGE}' (toolchain only)..."
    "$ENGINE" build -t "$IMAGE" -f "${RAVEN_ROOT}/${DOCKERFILE}" "${RAVEN_ROOT}"
    echo ">> Image '${IMAGE}' is ready. Run a build with: ./scripts/docker-build.sh all"
    exit 0
fi

# -----------------------------------------------------------------------------
# Build the image (cached after first run)
# -----------------------------------------------------------------------------
if [[ "${RAVEN_NO_BUILD:-0}" != "1" ]]; then
    echo ">> Building image '${IMAGE}' (cached after first run)..."
    "$ENGINE" build -t "$IMAGE" -f "${RAVEN_ROOT}/${DOCKERFILE}" "${RAVEN_ROOT}"
fi

# -----------------------------------------------------------------------------
# Assemble the build command to run inside the container
# -----------------------------------------------------------------------------
if [[ "${1:-}" == "shell" ]]; then
    CMD=(/bin/bash)
    shift || true
else
    CMD=(./scripts/build.sh "$@")
    # Default to a full build when no stage/args were given.
    if [[ $# -eq 0 ]]; then
        CMD=(./scripts/build.sh all)
    fi
fi

# Podman maps the host user into the container by default, which breaks the
# build's root-owned chroot steps; --userns=keep-id or running rootful is
# needed. Using --privileged + default root user works for both engines.
RUN_FLAGS=(
    --rm -it
    --privileged
    -v "${RAVEN_ROOT}:/raven"
    -w /raven
)

# Podman needs SELinux relabeling on some hosts; :z is harmless on Docker-less
# Linux but unsupported on macOS bind mounts, so only add it for Podman/Linux.
if [[ "$ENGINE" == "podman" && "$(uname -s)" == "Linux" ]]; then
    RUN_FLAGS=(
        --rm -it
        --privileged
        -v "${RAVEN_ROOT}:/raven:z"
        -w /raven
    )
fi

# -----------------------------------------------------------------------------
# Keep the build's working tree (build/) off the macOS bind mount.
# -----------------------------------------------------------------------------
# On macOS the repo bind mount reaches the Linux VM over virtiofs/sshfs, which
# mishandles the symlinks an LFS build unpacks: GNU tar (and bsdtar) abort with
# "Cannot open: Permission denied" / "Could not stat" the instant they extract a
# symlink (e.g. musl's ld-musl-x86_64.so.1 in the stage0 toolchain). A native
# engine volume avoids that entirely (and is far faster for the build's millions
# of small files). It also persists between runs, so the toolchain download and
# completed stages are cached. The final ISO is written to the repo root
# (/raven, the bind mount), so it still lands on the host. On Linux hosts the
# bind mount is native, so build/ stays directly visible on the host as before.
#
# Override the volume name with RAVEN_BUILD_VOLUME; set it empty to force the
# plain bind mount (build/ on the host) even on macOS.
if [[ "$(uname -s)" == "Darwin" && "${1:-}" != "image" ]]; then
    BUILD_VOLUME="${RAVEN_BUILD_VOLUME-raven-build}"
    if [[ -n "$BUILD_VOLUME" ]]; then
        RUN_FLAGS+=(-v "${BUILD_VOLUME}:/raven/build")
        echo ">> Using '${ENGINE}' volume '${BUILD_VOLUME}' for /raven/build (macOS:"
        echo "   avoids virtiofs symlink failures; the ISO still lands in the repo root)."
    fi
fi

echo ">> Running: ${CMD[*]}"
exec "$ENGINE" run "${PLATFORM_FLAGS[@]}" "${RUN_FLAGS[@]}" "$IMAGE" "${CMD[@]}"
