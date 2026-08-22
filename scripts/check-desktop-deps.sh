#!/bin/bash
# Check runtime dependencies for Raven Desktop Environment

echo "=== Checking Raven Desktop Dependencies ==="
echo ""

check_cmd() {
    if command -v "$1" >/dev/null 2>&1; then
        echo "✓ $1 found: $(command -v "$1")"
        return 0
    else
        echo "✗ $1 NOT FOUND"
        return 1
    fi
}

check_pkg() {
    if pkg-config --exists "$1" 2>/dev/null; then
        version=$(pkg-config --modversion "$1" 2>/dev/null || echo "unknown")
        echo "✓ $1 (version: $version)"
        return 0
    else
        echo "✗ $1 NOT FOUND"
        return 1
    fi
}

check_path() {
    if [ -e "$1" ]; then
        echo "✓ $1 exists"
        return 0
    else
        echo "✗ $1 NOT FOUND"
        return 1
    fi
}

echo "Build Dependencies:"
check_cmd go || true
check_cmd cargo || true
check_cmd rustc || true
check_cmd pkg-config || true
echo ""

echo "GTK4 and Layer Shell:"
check_pkg gtk4 || true
check_pkg gtk4-layer-shell-0 || true
echo ""

echo "Wayland:"
check_pkg wayland-client || true
check_pkg wayland-server || true
check_pkg wayland-protocols || true
echo ""

echo "Graphics:"
check_pkg gl || check_pkg opengl || true
check_pkg glfw3 || true
if pkg-config --exists glfw3 2>/dev/null; then
    # Check if GLFW has Wayland support
    if pkg-config --variable=wayland glfw3 2>/dev/null | grep -qi "true\|1\|yes"; then
        echo "✓ GLFW3 has Wayland support"
    else
        echo "⚠ GLFW3 may not have Wayland support (check build flags)"
    fi
fi
echo ""

echo "System Services:"
check_cmd seatd || true
check_cmd dbus-daemon || true
echo ""

echo "DRM/KMS:"
check_path /dev/dri || true
if [ -d /dev/dri ]; then
    echo "  DRI devices:"
    ls -la /dev/dri/ 2>/dev/null | grep -E "card|render" | sed 's/^/    /' || true
fi
echo ""

echo "Kernel Graphics Drivers:"
if [ -d /sys/class/drm ]; then
    echo "  DRM connectors:"
    for status in /sys/class/drm/*/status; do
        [ -f "$status" ] || continue
        connector="$(basename "$(dirname "$status")")"
        case "$connector" in
            card*|renderD*) continue ;;
        esac
        state="$(cat "$status" 2>/dev/null || echo "unknown")"
        echo "    $connector: $state"
    done
else
    echo "✗ /sys/class/drm NOT FOUND"
fi
echo ""

echo "=== Dependency Check Complete ==="
