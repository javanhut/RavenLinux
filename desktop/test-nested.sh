#!/bin/bash
# test-nested.sh - Test Raven desktop components
# Usage: ./test-nested.sh [--nested] [--sudo]
#   Default: Run components on current Hyprland session
#   --nested: Attempt nested Hyprland
#   --sudo: Pass --i-am-really-stupid to Hyprland (for running with sudo -E)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEST_CONFIG_DIR="$SCRIPT_DIR/.test-config"
TEST_HYPR_CONF="$TEST_CONFIG_DIR/hyprland.conf"
NESTED_MODE=false
SUDO_MODE=false

# Parse args
for arg in "$@"; do
    case $arg in
        --nested) NESTED_MODE=true ;;
        --sudo) SUDO_MODE=true ;;
    esac
done

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log() { echo -e "${GREEN}[TEST]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }
info() { echo -e "${BLUE}[INFO]${NC} $1"; }

# Check Wayland environment
check_wayland_env() {
    log "Checking Wayland environment..."

    if [ -z "$XDG_RUNTIME_DIR" ]; then
        error "XDG_RUNTIME_DIR is not set. Are you in a graphical session?"
    fi

    if [ -z "$WAYLAND_DISPLAY" ] && [ -z "$DISPLAY" ]; then
        error "No display found. Run this from within a graphical session (Hyprland, sway, etc.)"
    fi

    if [ -n "$WAYLAND_DISPLAY" ]; then
        log "Wayland session detected: $WAYLAND_DISPLAY"
    elif [ -n "$DISPLAY" ]; then
        warn "X11 session detected."
    fi
}

# Check dependencies
check_deps() {
    log "Checking dependencies..."

    local missing=()
    command -v go >/dev/null 2>&1 || missing+=("go")

    if $NESTED_MODE; then
        command -v Hyprland >/dev/null 2>&1 || missing+=("Hyprland")
    fi

    if [ ${#missing[@]} -ne 0 ]; then
        error "Missing dependencies: ${missing[*]}"
    fi

    log "All dependencies found"
}

# Create test config directory and hyprland config
setup_test_config() {
    log "Setting up test configuration..."

    mkdir -p "$TEST_CONFIG_DIR"
    mkdir -p ~/.config/raven

    # Create test hyprland.conf
    cat > "$TEST_HYPR_CONF" << 'HYPRCONF'
# Raven Desktop Test Configuration
# This is a minimal config for testing the desktop components

# Monitor config - auto-detect for nested session
monitor=,preferred,auto,1

# Input config
input {
    kb_layout = us
    follow_mouse = 1
    sensitivity = 0
}

# General appearance
general {
    gaps_in = 5
    gaps_out = 10
    border_size = 2
    col.active_border = rgba(7aa2f7ff) rgba(c4a7e7ff) 45deg
    col.inactive_border = rgba(414868aa)
    layout = dwindle
}

decoration {
    rounding = 8
    blur {
        enabled = true
        size = 6
        passes = 2
    }
    shadow {
        enabled = true
        range = 15
        render_power = 3
    }
}

animations {
    enabled = yes
    bezier = ease, 0.25, 0.1, 0.25, 1.0
    animation = windows, 1, 4, ease, slide
    animation = fade, 1, 3, ease
    animation = workspaces, 1, 4, ease, slide
}

# Dwindle layout
dwindle {
    pseudotile = yes
    preserve_split = yes
}

# Window rules for Raven components
windowrulev2 = float, class:^(raven-menu)$
windowrulev2 = float, class:^(raven-settings)$
windowrulev2 = float, class:^(raven-power)$
windowrulev2 = float, class:^(raven-keybindings)$
windowrulev2 = size 800 600, class:^(raven-file-manager)$

# Basic keybindings
$mod = SUPER

bind = $mod, Return, exec, foot
bind = $mod, Q, killactive
bind = $mod, M, exit
bind = $mod, E, exec, RAVEN_FILE_MANAGER_CMD
bind = $mod, Space, exec, RAVEN_MENU_CMD
bind = $mod, S, exec, RAVEN_SETTINGS_CMD
bind = $mod, K, exec, RAVEN_KEYBINDINGS_CMD
bind = $mod, Escape, exec, RAVEN_POWER_CMD

# Window management
bind = $mod, V, togglefloating
bind = $mod, F, fullscreen
bind = $mod, P, pseudo

# Focus movement
bind = $mod, H, movefocus, l
bind = $mod, L, movefocus, r
bind = $mod, K, movefocus, u
bind = $mod, J, movefocus, d

# Workspace switching
bind = $mod, 1, workspace, 1
bind = $mod, 2, workspace, 2
bind = $mod, 3, workspace, 3
bind = $mod, 4, workspace, 4
bind = $mod, 5, workspace, 5

# Move to workspace
bind = $mod SHIFT, 1, movetoworkspace, 1
bind = $mod SHIFT, 2, movetoworkspace, 2
bind = $mod SHIFT, 3, movetoworkspace, 3
bind = $mod SHIFT, 4, movetoworkspace, 4
bind = $mod SHIFT, 5, movetoworkspace, 5

# Mouse bindings
bindm = $mod, mouse:272, movewindow
bindm = $mod, mouse:273, resizewindow

HYPRCONF

    # Replace placeholder commands with actual go run commands
    sed -i "s|RAVEN_FILE_MANAGER_CMD|cd $SCRIPT_DIR/raven-file-manager \&\& go run .|g" "$TEST_HYPR_CONF"
    sed -i "s|RAVEN_MENU_CMD|cd $SCRIPT_DIR/raven-menu \&\& go run .|g" "$TEST_HYPR_CONF"
    sed -i "s|RAVEN_SETTINGS_CMD|cd $SCRIPT_DIR/raven-settings-menu \&\& go run .|g" "$TEST_HYPR_CONF"
    sed -i "s|RAVEN_KEYBINDINGS_CMD|cd $SCRIPT_DIR/raven-keybindings \&\& go run .|g" "$TEST_HYPR_CONF"
    sed -i "s|RAVEN_POWER_CMD|cd $SCRIPT_DIR/raven-power \&\& go run .|g" "$TEST_HYPR_CONF"

    # Add exec-once for desktop and shell
    cat >> "$TEST_HYPR_CONF" << EXECONCE

# Auto-start Raven desktop components
exec-once = cd $SCRIPT_DIR/raven-desktop && go run .
exec-once = sleep 2 && cd $SCRIPT_DIR/raven-shell && go run .
EXECONCE

    log "Test configuration created at $TEST_HYPR_CONF"
}

# Cleanup function
cleanup() {
    log "Cleaning up..."
    # Kill any lingering go processes from our test
    pkill -f "go-build.*raven-" 2>/dev/null || true
}

# Array to track background PIDs
PIDS=()

# Run component in background
run_component() {
    local name="$1"
    local dir="$2"

    if [ -d "$SCRIPT_DIR/$dir" ]; then
        log "Starting $name..."
        (cd "$SCRIPT_DIR/$dir" && go run . 2>&1 | sed "s/^/[$name] /") &
        PIDS+=($!)
        sleep 1
    else
        warn "Directory not found: $dir"
    fi
}

# Direct test mode - run on current session
run_direct_test() {
    echo ""
    echo "=========================================="
    echo "  Raven Desktop - Direct Test Mode"
    echo "=========================================="
    echo ""
    echo "Components will run on your CURRENT Hyprland session."
    echo "Press Ctrl+C to stop all components."
    echo ""

    check_wayland_env
    check_deps

    # Ensure config directory exists
    mkdir -p ~/.config/raven

    log "Starting Raven components on current session..."
    echo ""

    # Start core components
    run_component "raven-desktop" "raven-desktop"
    run_component "raven-shell" "raven-shell"

    echo ""
    info "Core components started (desktop + shell)"
    echo ""
    echo "To test other components, open a new terminal and run:"
    echo "  cd $SCRIPT_DIR/raven-menu && go run ."
    echo "  cd $SCRIPT_DIR/raven-file-manager && go run ."
    echo "  cd $SCRIPT_DIR/raven-settings-menu && go run ."
    echo "  cd $SCRIPT_DIR/raven-power && go run ."
    echo "  cd $SCRIPT_DIR/raven-keybindings && go run ."
    echo ""
    info "Press Enter to stop all components..."

    read -r

    log "Stopping components..."
    for pid in "${PIDS[@]}"; do
        kill "$pid" 2>/dev/null || true
    done
}

# Nested test mode
run_nested_test() {
    echo ""
    echo "=========================================="
    echo "  Raven Desktop - Nested Test Environment"
    echo "=========================================="
    echo ""

    check_wayland_env
    check_deps
    setup_test_config

    log "Starting nested Hyprland session..."
    echo ""
    echo "Keybindings:"
    echo "  SUPER + Return    - Open terminal (foot)"
    echo "  SUPER + Space     - App launcher (raven-menu)"
    echo "  SUPER + E         - File manager"
    echo "  SUPER + S         - Settings"
    echo "  SUPER + Escape    - Power menu"
    echo "  SUPER + Q         - Close window"
    echo "  SUPER + M         - Exit session"
    echo ""

    # Launch nested Hyprland with Wayland backend
    local hypr_args="-c $TEST_HYPR_CONF"
    if $SUDO_MODE; then
        hypr_args="$hypr_args --i-am-really-stupid"
    fi
    WLR_BACKENDS=wayland Hyprland $hypr_args

    log "Nested session ended"
}

trap cleanup EXIT

# Main
main() {
    if $NESTED_MODE; then
        run_nested_test
    else
        run_direct_test
    fi
}

main "$@"
