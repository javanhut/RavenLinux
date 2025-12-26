#!/bin/bash
# Build Raven Desktop Environment components locally
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

echo "=== Building Raven Desktop Environment ==="
echo "Project root: $PROJECT_ROOT"
echo ""

# Note: Raven Desktop now uses Hyprland as the compositor
# The raven-compositor is no longer built - install Hyprland from your package manager

# Build Rust-based desktop shell (unified panel, desktop, menu, settings)
echo ">>> Building raven-shell (Rust workspace)..."
cd "$PROJECT_ROOT/desktop/raven-shell"
cargo build --release
cd "$PROJECT_ROOT"
echo "raven-shell built"
echo ""

# Build terminal
echo ">>> Building raven-terminal..."
cd "$PROJECT_ROOT/tools/raven-terminal"
go build -o raven-terminal main.go
cd "$PROJECT_ROOT"
echo "raven-terminal built"
echo ""

echo "=== Installing Hyprland Configuration ==="
echo ""

# Install Hyprland config if it exists
if [[ -f "$PROJECT_ROOT/desktop/config/install.sh" ]]; then
    echo ">>> Installing Raven Hyprland configuration..."
    chmod +x "$PROJECT_ROOT/desktop/config/install.sh"
    "$PROJECT_ROOT/desktop/config/install.sh"
    echo ""
fi

echo "=== Build Complete ==="
echo ""
echo "Binaries located at:"
echo "  - desktop/raven-shell/target/release/raven-shell"
echo "  - desktop/raven-shell/target/release/raven-ctl"
echo "  - tools/raven-terminal/raven-terminal"
echo ""
echo "Configuration installed to:"
echo "  - ~/.config/hypr/hyprland.conf"
echo "  - ~/.config/raven/settings.json"
echo "  - ~/.config/raven/scripts/"
echo ""
echo "To use Raven Desktop:"
echo "  1. Log out and select 'Hyprland' from your display manager"
echo "  2. Or run: Hyprland"
echo ""
echo "Note: Raven Desktop now uses Hyprland as the compositor."
