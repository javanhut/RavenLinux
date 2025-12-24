#!/bin/bash
# Raven Linux - Wallpaper Setter
# Reads wallpaper path from raven settings and applies it

SETTINGS_FILE="$HOME/.config/raven/settings.json"
DEFAULT_WALLPAPER="/usr/share/backgrounds/raven-wallpaper.png"

# Check if jq is available for JSON parsing
if ! command -v jq &> /dev/null; then
    # Fallback to grep/sed for basic parsing
    if [ -f "$SETTINGS_FILE" ]; then
        WALLPAPER=$(grep -o '"wallpaper_path"[[:space:]]*:[[:space:]]*"[^"]*"' "$SETTINGS_FILE" | sed 's/.*"\([^"]*\)"$/\1/')
        MODE=$(grep -o '"wallpaper_mode"[[:space:]]*:[[:space:]]*"[^"]*"' "$SETTINGS_FILE" | sed 's/.*"\([^"]*\)"$/\1/')
    fi
else
    if [ -f "$SETTINGS_FILE" ]; then
        WALLPAPER=$(jq -r '.wallpaper_path // empty' "$SETTINGS_FILE")
        MODE=$(jq -r '.wallpaper_mode // "fill"' "$SETTINGS_FILE")
    fi
fi

# Use default if no wallpaper set
if [ -z "$WALLPAPER" ] || [ ! -f "$WALLPAPER" ]; then
    WALLPAPER="$DEFAULT_WALLPAPER"
fi

# Use fill mode if not specified
if [ -z "$MODE" ]; then
    MODE="fill"
fi

# Kill existing swaybg instances
pkill -x swaybg 2>/dev/null

# Check if wallpaper file exists
if [ -f "$WALLPAPER" ]; then
    # Start swaybg with the wallpaper
    swaybg -i "$WALLPAPER" -m "$MODE" &
    disown
else
    # Set solid color background as fallback
    swaybg -c "#87ceeb" &
    disown
fi
