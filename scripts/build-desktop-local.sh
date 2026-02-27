#!/bin/bash
# Build Raven Desktop Environment components locally
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

echo "=== Building Raven Desktop Environment ==="
echo "Project root: $PROJECT_ROOT"
echo ""

# Build GTK4 layer-shell components
echo ">>> Building raven-shell (panel)..."
cd "$PROJECT_ROOT/desktop/raven-shell"
go build -o raven-shell main.go
cd "$PROJECT_ROOT"
echo "raven-shell built"
echo ""

echo ">>> Building raven-desktop (background)..."
cd "$PROJECT_ROOT/desktop/raven-desktop"
go build -o raven-desktop main.go
cd "$PROJECT_ROOT"
echo "raven-desktop built"
echo ""

echo ">>> Building raven-menu (start menu)..."
cd "$PROJECT_ROOT/desktop/raven-menu"
go build -o raven-menu main.go
cd "$PROJECT_ROOT"
echo "raven-menu built"
echo ""

echo ">>> Building raven-settings-menu..."
cd "$PROJECT_ROOT/desktop/raven-settings-menu"
go build -o raven-settings-menu main.go
cd "$PROJECT_ROOT"
echo "raven-settings-menu built"
echo ""

# Build terminal
echo ">>> Building raven-terminal..."
cd "$PROJECT_ROOT/tools/raven-terminal"
go build -o raven-terminal main.go
cd "$PROJECT_ROOT"
echo "raven-terminal built"
echo ""

echo "=== Installing Raven Configuration ==="
echo ""

# Install config if it exists
if [[ -f "$PROJECT_ROOT/desktop/config/install.sh" ]]; then
    echo ">>> Installing Raven desktop configuration..."
    chmod +x "$PROJECT_ROOT/desktop/config/install.sh"
    "$PROJECT_ROOT/desktop/config/install.sh"
    echo ""
fi

echo "=== Build Complete ==="
echo ""
echo "Binaries located at:"
echo "  - desktop/raven-shell/raven-shell"
echo "  - desktop/raven-desktop/raven-desktop"
echo "  - desktop/raven-menu/raven-menu"
echo "  - desktop/raven-settings-menu/raven-settings-menu"
echo "  - tools/raven-terminal/raven-terminal"
echo ""
echo "Configuration installed to:"
echo "  - ~/.config/raven/settings.json"
echo "  - ~/.config/raven/scripts/"
echo ""
echo "To use Raven Desktop:"
echo "  1. Build raven-compositor: cd RavenCompositor && cargo build --release"
echo "  2. Run: raven-compositor --backend winit  (nested)"
echo "  3. Or use raven-wayland-session for full session"
