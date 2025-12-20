#!/bin/bash
# Quick installer for the GBM-enabled raven-compositor

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSITOR_BIN="${SCRIPT_DIR}/desktop/compositor/target/release/raven-compositor"
INSTALL_DIR="${SCRIPT_DIR}/build/packages/bin"

echo "========================================"
echo "  Installing Raven Compositor (GBM)"
echo "========================================"
echo ""

# Check if compositor is built
if [[ ! -f "${COMPOSITOR_BIN}" ]]; then
    echo "ERROR: Compositor not found at: ${COMPOSITOR_BIN}"
    echo "Please build it first with:"
    echo "  cd desktop/compositor && cargo build --release"
    exit 1
fi

# Show binary info
echo "Source binary:"
ls -lh "${COMPOSITOR_BIN}"
echo ""

# Install
echo "Installing to: ${INSTALL_DIR}/"
sudo mkdir -p "${INSTALL_DIR}"
sudo cp "${COMPOSITOR_BIN}" "${INSTALL_DIR}/raven-compositor"
sudo chmod +x "${INSTALL_DIR}/raven-compositor"

echo ""
echo "✓ Installation complete!"
echo ""
ls -lh "${INSTALL_DIR}/raven-compositor"
echo ""
echo "The compositor is now ready for:"
echo "  - ISO building (stage4)"
echo "  - Direct testing in build environment"
echo "  - Deployment to target system"
echo ""
echo "GBM rendering features:"
echo "  ✓ virtio-gpu-pci support"
echo "  ✓ Hardware buffer management"  
echo "  ✓ Visual output enabled"
echo "  ✓ Enhanced logging"
echo ""
