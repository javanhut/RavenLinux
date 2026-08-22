#!/bin/bash
# Wrapper script for raven-wifi that preserves environment variables

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Check if we're already root
if [ "$EUID" -eq 0 ]; then
    # Already root, just run it
    exec "$SCRIPT_DIR/raven-wifi"
else
    # Not root, run with sudo -E to preserve environment
    echo "Running raven-wifi with sudo..."
    exec sudo -E "$SCRIPT_DIR/raven-wifi"
fi
