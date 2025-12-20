#!/bin/bash
# Fix ownership of compositor target directories
# YOU MUST RUN THIS WITH: sudo ./scripts/fix-permissions.sh

set -e

echo "=== Fixing Compositor Target Directory Permissions ==="
echo ""

TARGET_DIR="/home/javanstorm/Development/CustomLinux/RavenLinux/desktop/compositor"

if [ "$EUID" -ne 0 ]; then
    echo "ERROR: This script must be run with sudo"
    echo "Run: sudo ./scripts/fix-permissions.sh"
    exit 1
fi

echo "Fixing ownership of:"
echo "  - $TARGET_DIR/target"
echo "  - $TARGET_DIR/target-user"
echo ""

chown -R javanstorm:javanstorm "$TARGET_DIR/target" 2>/dev/null || true
chown -R javanstorm:javanstorm "$TARGET_DIR/target-user" 2>/dev/null || true

echo "✓ Permissions fixed!"
echo ""
echo "Now you can run:"
echo "  ./scripts/build-desktop-local.sh"
