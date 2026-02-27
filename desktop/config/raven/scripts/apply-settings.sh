#!/bin/bash
# Raven Linux - Settings Applier
# Applies settings from settings.json to raven-compositor via raven-shell

SETTINGS_FILE="$HOME/.config/raven/settings.json"

# Check if settings file exists
if [ ! -f "$SETTINGS_FILE" ]; then
    echo "Settings file not found: $SETTINGS_FILE"
    exit 0
fi

# Reload compositor settings via raven-shell IPC
if command -v raven-shell &> /dev/null; then
    raven-shell reload-settings 2>/dev/null
fi

# Apply wallpaper
~/.config/raven/scripts/set-wallpaper.sh

echo "Settings applied successfully"
