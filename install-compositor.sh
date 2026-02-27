#!/bin/bash
# Quick installer for raven-compositor from GitHub build

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="${SCRIPT_DIR}/build/packages/bin"

# Source repos library
source "${SCRIPT_DIR}/scripts/lib/repos.sh"

COMPOSITOR_DIR="$(get_repo_dir compositor)"
COMPOSITOR_BIN="${COMPOSITOR_DIR}/target/release/raven-compositor"

echo "========================================"
echo "  Installing Raven Compositor"
echo "========================================"
echo ""

# Check if compositor is built
if [[ ! -f "${COMPOSITOR_BIN}" ]]; then
    echo "Compositor not found. Building from GitHub..."
    fetch_repo compositor
    cd "$COMPOSITOR_DIR"
    cargo build --release
    cd "$SCRIPT_DIR"
fi

# Show binary info
echo "Source binary:"
ls -lh "${COMPOSITOR_BIN}"
echo ""

# Install
echo "Installing to: ${INSTALL_DIR}/"
sudo mkdir -p "${INSTALL_DIR}"
for bin in raven-compositor raven-shell raven-settings; do
    if [[ -f "${COMPOSITOR_DIR}/target/release/${bin}" ]]; then
        sudo cp "${COMPOSITOR_DIR}/target/release/${bin}" "${INSTALL_DIR}/${bin}"
        sudo chmod +x "${INSTALL_DIR}/${bin}"
        echo "  Installed ${bin}"
    fi
done

echo ""
echo "Installation complete!"
echo ""
ls -lh "${INSTALL_DIR}"/raven-compositor "${INSTALL_DIR}"/raven-shell "${INSTALL_DIR}"/raven-settings 2>/dev/null
echo ""
echo "The compositor is now ready for:"
echo "  - ISO building (stage4)"
echo "  - Direct testing in build environment"
echo "  - Deployment to target system"
echo ""
