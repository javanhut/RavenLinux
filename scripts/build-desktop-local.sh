#!/bin/bash
# Build Raven Desktop Environment components locally
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

echo "=== Building Raven Desktop Environment ==="
echo "Project root: $PROJECT_ROOT"
echo ""

# Build compositor
echo ">>> Building raven-compositor (Rust)..."
cd "$PROJECT_ROOT/desktop/compositor"
if [ -w target-user ]; then
    cargo build --release --target-dir=target-user
else
    echo "WARNING: target-user directory not writable, using sudo..."
    sudo -u "$SUDO_USER" cargo build --release --target-dir=target-user 2>/dev/null || \
    cargo build --release --target-dir=target-user
fi
cd "$PROJECT_ROOT"
echo "✓ raven-compositor built"
echo ""

# Build GTK components
echo ">>> Building raven-shell (panel)..."
cd "$PROJECT_ROOT/desktop/raven-shell"
go build -o raven-shell main.go
cd "$PROJECT_ROOT"
echo "✓ raven-shell built"
echo ""

echo ">>> Building raven-desktop (background)..."
cd "$PROJECT_ROOT/desktop/raven-desktop"
go build -o raven-desktop main.go
cd "$PROJECT_ROOT"
echo "✓ raven-desktop built"
echo ""

echo ">>> Building raven-menu (start menu)..."
cd "$PROJECT_ROOT/desktop/raven-menu"
go build -o raven-menu main.go
cd "$PROJECT_ROOT"
echo "✓ raven-menu built"
echo ""

# Build terminal
echo ">>> Building raven-terminal..."
cd "$PROJECT_ROOT/tools/raven-terminal"
go build -o raven-terminal main.go
cd "$PROJECT_ROOT"
echo "✓ raven-terminal built"
echo ""

echo "=== Build Complete ==="
echo ""
echo "Binaries located at:"
echo "  - desktop/compositor/target-user/release/raven-compositor"
echo "  - desktop/raven-shell/raven-shell"
echo "  - desktop/raven-desktop/raven-desktop"
echo "  - desktop/raven-menu/raven-menu"
echo "  - tools/raven-terminal/raven-terminal"
echo ""
echo "To test locally, run: ./scripts/test-desktop-local.sh"
