#!/bin/bash
# =============================================================================
# Build Raven Desktop Environment locally (development)
# =============================================================================
# Fetches and builds the compositor from GitHub, installs config.
# Desktop Go apps have been removed; new GUI will be built on RavenCompositor.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
export RAVEN_ROOT="$PROJECT_ROOT"
export RAVEN_BUILD="${PROJECT_ROOT}/build"

cd "$PROJECT_ROOT"

# Source libraries
source "${SCRIPT_DIR}/lib/repos.sh"

echo "=== Building Raven Desktop Environment ==="
echo "Project root: $PROJECT_ROOT"
echo ""

# Build compositor from GitHub
echo ">>> Fetching and building RavenCompositor..."
fetch_repo compositor
COMPOSITOR_DIR="$(get_repo_dir compositor)"
cd "$COMPOSITOR_DIR"
cargo build --release
cd "$PROJECT_ROOT"
echo "RavenCompositor built"
echo ""

# Build terminal from GitHub
echo ">>> Fetching and building raven-terminal..."
fetch_repo terminal
build_go_repo terminal
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
echo "  - $(get_repo_dir compositor)/target/release/raven-compositor"
echo "  - $(get_repo_dir compositor)/target/release/raven-shell"
echo "  - $(get_repo_dir compositor)/target/release/raven-settings"
echo "  - ${RAVEN_BUILD}/packages/bin/raven-terminal"
echo ""
echo "Configuration installed to:"
echo "  - ~/.config/raven/settings.json"
echo "  - ~/.config/raven/scripts/"
echo ""
echo "To use Raven Desktop:"
echo "  1. Run: raven-compositor --backend winit  (nested)"
echo "  2. Or switch to a TTY and run: raven-compositor"
