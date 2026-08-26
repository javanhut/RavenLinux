#!/bin/bash
set -euo pipefail

echo ">> Removing RavenLinux build output..."
if ! rm -rf build 2>/dev/null; then
    echo "ERROR: build contains container-owned files." >&2
    echo "Run: sudo chown -R \"$(id -u):$(id -g)\" build" >&2
    exit 1
fi
echo ">> Build output removed."
