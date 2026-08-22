#!/bin/bash
# Raven Desktop - Configuration Installer
# Installs Raven compositor settings and scripts

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_DIR="$HOME/.config"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

echo "Installing Raven Desktop Configuration..."

# Create directories
mkdir -p "$CONFIG_DIR/raven/scripts"

# Install Raven scripts
cp "$SCRIPT_DIR/raven/scripts/set-wallpaper.sh" "$CONFIG_DIR/raven/scripts/"
cp "$SCRIPT_DIR/raven/scripts/apply-settings.sh" "$CONFIG_DIR/raven/scripts/"
chmod +x "$CONFIG_DIR/raven/scripts/"*.sh
echo "Installed Raven scripts"

# Install default wallpapers (system-wide if writable)
if [ -d "$SCRIPT_DIR/raven/backgrounds" ]; then
    if [ -w /usr/share/backgrounds ]; then
        mkdir -p /usr/share/backgrounds
        cp "$SCRIPT_DIR/raven/backgrounds"/* /usr/share/backgrounds/ 2>/dev/null || true
        echo "Installed Raven wallpapers"
    else
        echo "Skipping wallpapers (no permission for /usr/share/backgrounds)"
    fi
fi

# Create default settings if not exists
if [ ! -f "$CONFIG_DIR/raven/settings.json" ]; then
    cat > "$CONFIG_DIR/raven/settings.json" << 'EOF'
{
  "theme": "dark",
  "accent_color": "#009688",
  "font_size": 14,
  "icon_theme": "Papirus-Dark",
  "cursor_theme": "Adwaita",
  "panel_opacity": 0.95,
  "enable_animations": true,
  "wallpaper_path": "",
  "wallpaper_mode": "fill",
  "show_desktop_icons": false,
  "panel_position": "top",
  "panel_height": 38,
  "show_clock": true,
  "clock_format": "24h",
  "show_workspaces": true,
  "border_width": 2,
  "gap_size": 8,
  "focus_follows_mouse": false,
  "titlebar_buttons": "close,minimize,maximize",
  "keyboard_layout": "us",
  "mouse_speed": 0.5,
  "touchpad_natural_scroll": true,
  "touchpad_tap_to_click": true,
  "screen_timeout": 300,
  "suspend_timeout": 900,
  "lid_close_action": "suspend",
  "master_volume": 80,
  "mute_on_lock": false
}
EOF
    echo "Created default settings"
fi

# Create Screenshots directory
mkdir -p "$HOME/Pictures/Screenshots"

echo ""
echo "Raven Desktop configuration installed successfully!"
echo ""
echo "To use Raven Desktop:"
echo "  1. Build raven-compositor: cd RavenCompositor && cargo build --release"
echo "  2. Install raven-shell, raven-desktop, raven-menu, raven-settings-menu"
echo "  3. Run raven-compositor --backend winit (nested) or use raven-wayland-session"
echo ""
echo "Required dependencies:"
echo "  - gtk4-layer-shell"
echo "  - swaybg (wallpaper)"
echo "  - mako or dunst (notifications)"
echo "  - wl-clipboard (clipboard)"
echo "  - grim + slurp (screenshots)"
echo "  - brightnessctl (brightness control)"
echo "  - playerctl (media control)"
echo "  - wpctl (audio control via wireplumber)"
echo ""
