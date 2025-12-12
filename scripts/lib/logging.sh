#!/bin/bash
# =============================================================================
# RavenLinux Build System - Shared Logging Library
# =============================================================================
# Source this file in build scripts to get consistent logging functionality
#
# Usage:
#   source "$(dirname "${BASH_SOURCE[0]}")/lib/logging.sh"
#   init_logging "script-name"
#   log_info "Some message"
#   run_logged make -j4
#   finalize_logging
#
# Environment variables:
#   RAVEN_LOG_DIR      - Override log directory (default: build/logs)
#   RAVEN_NO_LOG       - Set to "1" to disable file logging
#   RAVEN_LOG_VERBOSE  - Set to "1" for extra verbose output

# =============================================================================
# Configuration
# =============================================================================

# Determine project root (works when sourced from different locations)
if [[ -z "${RAVEN_ROOT:-}" ]]; then
    # Try to find project root by looking for scripts/lib directory
    _LOGGING_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    RAVEN_ROOT="$(cd "${_LOGGING_SCRIPT_DIR}/../.." && pwd)"
fi

# Build and log directories
RAVEN_BUILD="${RAVEN_BUILD:-${RAVEN_ROOT}/build}"
RAVEN_LOG_DIR="${RAVEN_LOG_DIR:-${RAVEN_BUILD}/logs}"

# Logging state
_RAVEN_LOG_FILE=""
_RAVEN_LOG_ENABLED=true
_RAVEN_SCRIPT_NAME=""
_RAVEN_BUILD_START_TIME=""

# Check if logging should be disabled
if [[ "${RAVEN_NO_LOG:-0}" == "1" ]]; then
    _RAVEN_LOG_ENABLED=false
fi

# =============================================================================
# Colors for terminal output
# =============================================================================

# Determine if we should use colors:
# - NO_COLOR env var disables colors (https://no-color.org/)
# - TERM=dumb or unset means no color support
# - Not a terminal (piped/redirected) means no colors
# - FORCE_COLOR=1 overrides and enables colors
_use_colors() {
    # User explicitly disabled colors
    [[ -n "${NO_COLOR:-}" ]] && return 1

    # User explicitly enabled colors
    [[ "${FORCE_COLOR:-}" == "1" ]] && return 0

    # Not a terminal (piped or redirected)
    [[ ! -t 1 ]] && return 1

    # Dumb terminal or no TERM set
    [[ -z "${TERM:-}" || "${TERM:-}" == "dumb" ]] && return 1

    # Check if terminal supports colors (if tput available)
    if command -v tput &>/dev/null; then
        [[ "$(tput colors 2>/dev/null || echo 0)" -ge 8 ]] && return 0
        return 1
    fi

    # Default: assume colors work in interactive terminal
    return 0
}

if _use_colors; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    BLUE='\033[0;34m'
    CYAN='\033[0;36m'
    MAGENTA='\033[0;35m'
    BOLD='\033[1m'
    NC='\033[0m' # No Color
else
    # No colors
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    CYAN=''
    MAGENTA=''
    BOLD=''
    NC=''
fi

# =============================================================================
# Core Logging Functions
# =============================================================================

# Initialize logging for a script
# Usage: init_logging "script-name" ["optional description"]
init_logging() {
    local script_name="$1"
    local description="${2:-}"

    _RAVEN_SCRIPT_NAME="$script_name"
    _RAVEN_BUILD_START_TIME=$(date +%s)

    if [[ "$_RAVEN_LOG_ENABLED" == "true" ]]; then
        mkdir -p "$RAVEN_LOG_DIR"

        local timestamp=$(date +"%Y%m%d_%H%M%S")
        _RAVEN_LOG_FILE="${RAVEN_LOG_DIR}/${script_name}_${timestamp}.log"

        # Create log file with header
        {
            echo "=============================================================================="
            echo "  RavenLinux Build Log"
            echo "=============================================================================="
            echo "  Script:      ${script_name}"
            if [[ -n "$description" ]]; then
                echo "  Description: ${description}"
            fi
            echo "  Started:     $(date '+%Y-%m-%d %H:%M:%S')"
            echo "  Hostname:    $(hostname 2>/dev/null || cat /etc/hostname 2>/dev/null || echo 'unknown')"
            echo "  User:        $(whoami 2>/dev/null || echo "${USER:-unknown}")"
            echo "  Working Dir: $(pwd)"
            echo "  Kernel:      $(uname -r)"
            echo "=============================================================================="
            echo ""
        } > "$_RAVEN_LOG_FILE"

        log_info "Logging to: ${_RAVEN_LOG_FILE}"
    fi
}

# Finalize logging (call at end of script)
finalize_logging() {
    local exit_code="${1:-0}"
    local end_time=$(date +%s)
    local duration=$((end_time - _RAVEN_BUILD_START_TIME))
    local duration_str=$(format_duration $duration)

    if [[ "$_RAVEN_LOG_ENABLED" == "true" ]] && [[ -n "$_RAVEN_LOG_FILE" ]]; then
        {
            echo ""
            echo "=============================================================================="
            if [[ "$exit_code" == "0" ]]; then
                echo "  Build Completed Successfully"
            else
                echo "  Build Failed (exit code: ${exit_code})"
            fi
            echo "=============================================================================="
            echo "  Finished: $(date '+%Y-%m-%d %H:%M:%S')"
            echo "  Duration: ${duration_str}"
            echo "=============================================================================="
        } >> "$_RAVEN_LOG_FILE"
    fi
}

# Format duration in human readable form
format_duration() {
    local seconds="$1"
    local hours=$((seconds / 3600))
    local minutes=$(((seconds % 3600) / 60))
    local secs=$((seconds % 60))

    if [[ $hours -gt 0 ]]; then
        printf "%dh %dm %ds" $hours $minutes $secs
    elif [[ $minutes -gt 0 ]]; then
        printf "%dm %ds" $minutes $secs
    else
        printf "%ds" $secs
    fi
}

# =============================================================================
# Log Message Functions
# =============================================================================

# Internal function to write to log file
_write_to_log() {
    local message="$1"
    if [[ "$_RAVEN_LOG_ENABLED" == "true" ]] && [[ -n "$_RAVEN_LOG_FILE" ]]; then
        echo "$message" >> "$_RAVEN_LOG_FILE"
    fi
}

# Log info message (blue)
log_info() {
    local message="$1"
    local timestamp=$(date '+%H:%M:%S')
    echo -e "${BLUE}[INFO]${NC} $message"
    _write_to_log "[${timestamp}] [INFO] $message"
}

# Log success message (green)
log_success() {
    local message="$1"
    local timestamp=$(date '+%H:%M:%S')
    echo -e "${GREEN}[SUCCESS]${NC} $message"
    _write_to_log "[${timestamp}] [SUCCESS] $message"
}

# Log warning message (yellow)
log_warn() {
    local message="$1"
    local timestamp=$(date '+%H:%M:%S')
    echo -e "${YELLOW}[WARN]${NC} $message"
    _write_to_log "[${timestamp}] [WARN] $message"
}

# Log error message (red) - does NOT exit
log_error() {
    local message="$1"
    local timestamp=$(date '+%H:%M:%S')
    echo -e "${RED}[ERROR]${NC} $message" >&2
    _write_to_log "[${timestamp}] [ERROR] $message"
}

# Log error and exit
log_fatal() {
    local message="$1"
    local exit_code="${2:-1}"
    log_error "$message"
    finalize_logging "$exit_code"
    exit "$exit_code"
}

# Log step message (cyan) - for major steps
log_step() {
    local message="$1"
    local timestamp=$(date '+%H:%M:%S')
    echo -e "${CYAN}[STEP]${NC} $message"
    _write_to_log "[${timestamp}] [STEP] $message"
}

# Log debug message (magenta) - only if verbose
log_debug() {
    local message="$1"
    if [[ "${RAVEN_LOG_VERBOSE:-0}" == "1" ]]; then
        local timestamp=$(date '+%H:%M:%S')
        echo -e "${MAGENTA}[DEBUG]${NC} $message"
        _write_to_log "[${timestamp}] [DEBUG] $message"
    fi
}

# Log a section header
log_section() {
    local title="$1"
    local line="=========================================="
    echo ""
    echo -e "${BOLD}${line}${NC}"
    echo -e "${BOLD}  ${title}${NC}"
    echo -e "${BOLD}${line}${NC}"
    echo ""

    if [[ "$_RAVEN_LOG_ENABLED" == "true" ]] && [[ -n "$_RAVEN_LOG_FILE" ]]; then
        {
            echo ""
            echo "$line"
            echo "  $title"
            echo "$line"
            echo ""
        } >> "$_RAVEN_LOG_FILE"
    fi
}

# =============================================================================
# Command Execution with Logging
# =============================================================================

# Run a command with output captured to both terminal and log file
# Usage: run_logged make -j4
run_logged() {
    if [[ "$_RAVEN_LOG_ENABLED" == "true" ]] && [[ -n "$_RAVEN_LOG_FILE" ]]; then
        local timestamp=$(date '+%H:%M:%S')
        echo "[${timestamp}] [CMD] $*" >> "$_RAVEN_LOG_FILE"

        # Run command with output to both terminal and log
        "$@" 2>&1 | tee -a "$_RAVEN_LOG_FILE"
        local exit_code=${PIPESTATUS[0]}

        if [[ $exit_code -ne 0 ]]; then
            echo "[${timestamp}] [CMD] Command failed with exit code: ${exit_code}" >> "$_RAVEN_LOG_FILE"
        fi

        return $exit_code
    else
        "$@"
    fi
}

# Run a command silently but capture output to log on failure
# Usage: run_silent make -j4
run_silent() {
    local output
    local exit_code

    output=$("$@" 2>&1)
    exit_code=$?

    if [[ $exit_code -ne 0 ]]; then
        log_error "Command failed: $*"
        if [[ "$_RAVEN_LOG_ENABLED" == "true" ]] && [[ -n "$_RAVEN_LOG_FILE" ]]; then
            {
                echo "[ERROR] Command output:"
                echo "$output"
            } >> "$_RAVEN_LOG_FILE"
        fi
        echo "$output" >&2
    fi

    return $exit_code
}

# Run a command and capture output to log (no terminal output)
# Usage: run_quiet make -j4
run_quiet() {
    if [[ "$_RAVEN_LOG_ENABLED" == "true" ]] && [[ -n "$_RAVEN_LOG_FILE" ]]; then
        local timestamp=$(date '+%H:%M:%S')
        echo "[${timestamp}] [CMD] $*" >> "$_RAVEN_LOG_FILE"
        "$@" >> "$_RAVEN_LOG_FILE" 2>&1
        return $?
    else
        "$@" >/dev/null 2>&1
    fi
}

# =============================================================================
# Utility Functions
# =============================================================================

# Get the current log file path
get_log_file() {
    echo "$_RAVEN_LOG_FILE"
}

# Check if logging is enabled
is_logging_enabled() {
    [[ "$_RAVEN_LOG_ENABLED" == "true" ]] && [[ -n "$_RAVEN_LOG_FILE" ]]
}

# Print a summary of recent log files
list_recent_logs() {
    local count="${1:-10}"

    if [[ -d "$RAVEN_LOG_DIR" ]]; then
        echo "Recent build logs (${RAVEN_LOG_DIR}):"
        ls -lt "$RAVEN_LOG_DIR"/*.log 2>/dev/null | head -n "$count" | while read -r line; do
            echo "  $line"
        done
    else
        echo "No log directory found at: $RAVEN_LOG_DIR"
    fi
}

# Tail the most recent log file
tail_latest_log() {
    local lines="${1:-50}"

    if [[ -d "$RAVEN_LOG_DIR" ]]; then
        local latest=$(ls -t "$RAVEN_LOG_DIR"/*.log 2>/dev/null | head -1)
        if [[ -n "$latest" ]]; then
            echo "Tailing: $latest"
            tail -n "$lines" "$latest"
        else
            echo "No log files found"
        fi
    fi
}

# Search logs for errors
search_logs_for_errors() {
    local log_file="${1:-$_RAVEN_LOG_FILE}"

    if [[ -f "$log_file" ]]; then
        echo "Errors found in $log_file:"
        grep -n -i "error\|failed\|fatal" "$log_file" || echo "  No errors found"
    fi
}

# =============================================================================
# Trap handler for unexpected exits
# =============================================================================

# Set up trap to finalize logging on exit
_logging_exit_trap() {
    local exit_code=$?
    if [[ $exit_code -ne 0 ]] && [[ -n "$_RAVEN_LOG_FILE" ]]; then
        finalize_logging "$exit_code"
    fi
}

# Enable automatic finalization on exit (optional - call explicitly if needed)
enable_logging_trap() {
    trap _logging_exit_trap EXIT
}
