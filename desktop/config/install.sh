#!/bin/bash
# Raven Desktop - Configuration Installer
# Installs Hyprland config and Raven scripts

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_DIR="$HOME/.config"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

source "${PROJECT_ROOT}/scripts/lib/hyprland-config.sh"

echo "Installing Raven Desktop Configuration..."

# Create directories
mkdir -p "$CONFIG_DIR/hypr"
mkdir -p "$CONFIG_DIR/raven/scripts"

# Install Hyprland config
if [ -f "$CONFIG_DIR/hypr/hyprland.conf" ]; then
    echo "Backing up existing Hyprland config..."
    cp "$CONFIG_DIR/hypr/hyprland.conf" "$CONFIG_DIR/hypr/hyprland.conf.backup.$(date +%Y%m%d_%H%M%S)"
fi

write_hyprland_config "$CONFIG_DIR/hypr/hyprland.conf"
echo "Installed Hyprland configuration"

# Install Raven scripts
cp "$SCRIPT_DIR/raven/scripts/set-wallpaper.sh" "$CONFIG_DIR/raven/scripts/"
cp "$SCRIPT_DIR/raven/scripts/apply-settings.sh" "$CONFIG_DIR/raven/scripts/"
chmod +x "$CONFIG_DIR/raven/scripts/"*.sh
echo "Installed Raven scripts"

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
echo "To use Raven Desktop with Hyprland:"
echo "  1. Make sure Hyprland is installed"
echo "  2. Install raven-shell, raven-desktop, raven-menu, raven-settings-menu"
echo "  3. Log out and select 'Hyprland' from your display manager"
echo ""
echo "Required dependencies:"
echo "  - hyprland"
echo "  - gtk4-layer-shell"
echo "  - swaybg (wallpaper)"
echo "  - mako or dunst (notifications)"
echo "  - wl-clipboard (clipboard)"
echo "  - grim + slurp (screenshots)"
echo "  - brightnessctl (brightness control)"
echo "  - playerctl (media control)"
echo "  - wpctl (audio control via wireplumber)"
echo ""
