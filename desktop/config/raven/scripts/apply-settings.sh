#!/bin/bash
# Raven Linux - Settings Applier
# Applies settings from settings.json to Hyprland

SETTINGS_FILE="$HOME/.config/raven/settings.json"

# Check if settings file exists
if [ ! -f "$SETTINGS_FILE" ]; then
    echo "Settings file not found: $SETTINGS_FILE"
    exit 0
fi

# Parse settings using jq if available
if ! command -v jq &> /dev/null; then
    echo "jq not installed, skipping dynamic settings application"
    exit 0
fi

# Read settings
BORDER_WIDTH=$(jq -r '.border_width // 2' "$SETTINGS_FILE")
GAP_SIZE=$(jq -r '.gap_size // 8' "$SETTINGS_FILE")
FOCUS_FOLLOWS_MOUSE=$(jq -r '.focus_follows_mouse // false' "$SETTINGS_FILE")
KB_LAYOUT=$(jq -r '.keyboard_layout // "us"' "$SETTINGS_FILE")

# Apply to Hyprland via hyprctl
hyprctl keyword general:border_size "$BORDER_WIDTH" 2>/dev/null
hyprctl keyword general:gaps_in "$((GAP_SIZE / 2))" 2>/dev/null
hyprctl keyword general:gaps_out "$GAP_SIZE" 2>/dev/null

if [ "$FOCUS_FOLLOWS_MOUSE" = "true" ]; then
    hyprctl keyword input:follow_mouse 1 2>/dev/null
else
    hyprctl keyword input:follow_mouse 1 2>/dev/null  # Still follow but don't focus
fi

hyprctl keyword input:kb_layout "$KB_LAYOUT" 2>/dev/null

# Apply wallpaper
~/.config/raven/scripts/set-wallpaper.sh

echo "Settings applied successfully"
