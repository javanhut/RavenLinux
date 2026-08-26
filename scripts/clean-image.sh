#!/bin/bash
set -euo pipefail

image="${RAVEN_IMAGE:-ravenlinux-build}"
engine="${RAVEN_ENGINE:-}"
if [[ -z "$engine" ]]; then
    if command -v docker >/dev/null 2>&1; then
        engine=docker
    elif command -v podman >/dev/null 2>&1; then
        engine=podman
    fi
fi
[[ -n "$engine" ]] || { echo "ERROR: docker or podman is required." >&2; exit 1; }
"$engine" rmi -f "$image"
