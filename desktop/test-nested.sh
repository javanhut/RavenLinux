#!/bin/bash
# test-nested.sh - Test Raven desktop components
# Usage: ./test-nested.sh [--nested]
#   Default: Run components on current Wayland session
#   --nested: Launch nested raven-compositor (winit backend)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NESTED_MODE=false

# Parse args
for arg in "$@"; do
    case $arg in
        --nested) NESTED_MODE=true ;;
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
        error "No display found. Run this from within a graphical session."
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
        command -v raven-compositor >/dev/null 2>&1 || missing+=("raven-compositor")
    fi

    if [ ${#missing[@]} -ne 0 ]; then
        error "Missing dependencies: ${missing[*]}"
    fi

    log "All dependencies found"
}

# Cleanup function
cleanup() {
    log "Cleaning up..."
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
    echo "Components will run on your CURRENT Wayland session."
    echo "Press Ctrl+C to stop all components."
    echo ""

    check_wayland_env
    check_deps

    mkdir -p ~/.config/raven

    log "Starting Raven components on current session..."
    echo ""

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

# Nested test mode - launch raven-compositor with winit backend
run_nested_test() {
    echo ""
    echo "=========================================="
    echo "  Raven Desktop - Nested Test Environment"
    echo "=========================================="
    echo ""

    check_wayland_env
    check_deps

    log "Starting nested raven-compositor (winit backend)..."
    echo ""
    echo "The compositor will open in a window."
    echo "Use Super+Alt+Q to exit the nested session."
    echo ""

    raven-compositor --backend winit

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
