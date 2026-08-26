#!/bin/bash
# =============================================================================
# RavenLinux package manifest checker
# =============================================================================
# Checks every packages/**/package.toml for symlink entries that collapse to a
# self-reference once the root is usr-merged:
#
#     symlinks = [ { target = "/usr/bin/foo", link = "/bin/foo" } ]
#
# On a usr-merged root /bin IS /usr/bin, so that link path and that target name
# the same file. Creating it replaces the real binary with a symlink to itself
# -- the failure scripts/lib/usrmerge.sh documents, and which cost the ISO its
# copies of huginn and muninn-lock once already.
#
# The reason this needs a checker rather than a code review is that it does not
# fail loudly. `ln -sf` exits 0. The package installs, `rvn` reports success,
# and the binary is gone -- discovered at boot, if at all. It was present in 16
# entries across 12 packages, `bash` and `rvn` among them, before anyone
# noticed.
#
# The merge table comes from scripts/lib/usrmerge.sh rather than being restated
# here, so the checker and the merge cannot disagree about what /bin means.
#
# Usage: ./scripts/check-manifests.sh [OPTIONS] [PACKAGES_DIR]
#
# Options:
#   -q, --quiet     Only print failures and the summary
#   -h, --help      Show this help message
#
# PACKAGES_DIR defaults to packages/ beside this script.
#
# Exits non-zero if any manifest declares a self-referential symlink.
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${RAVEN_ROOT:-$(dirname "$SCRIPT_DIR")}"

if [[ -f "${PROJECT_ROOT}/scripts/lib/logging.sh" ]]; then
    # shellcheck disable=SC1091
    source "${PROJECT_ROOT}/scripts/lib/logging.sh"
else
    RED=''; GREEN=''; CYAN=''; BOLD=''; NC=''
    log_info()    { echo "[INFO] $*"; }
    log_error()   { echo "[ERROR] $*" >&2; }
    log_success() { echo "[SUCCESS] $*"; }
fi

# The merge table, and the canonicaliser that reads it.
# shellcheck disable=SC1091
source "${PROJECT_ROOT}/scripts/lib/usrmerge.sh"

QUIET=false
PACKAGES_ARG=""

show_help() {
    sed -n '2,33p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        -q|--quiet) QUIET=true; shift ;;
        -h|--help)  show_help; exit 0 ;;
        -*)         echo "Unknown option: $1" >&2; show_help; exit 1 ;;
        *)
            if [[ -n "$PACKAGES_ARG" ]]; then
                echo "Too many arguments: $1" >&2
                exit 1
            fi
            PACKAGES_ARG="$1"; shift
            ;;
    esac
done

PACKAGES_DIR="${PACKAGES_ARG:-${PROJECT_ROOT}/packages}"

if [[ ! -d "$PACKAGES_DIR" ]]; then
    log_error "Not a directory: ${PACKAGES_DIR}"
    exit 1
fi

# ---------------------------------------------------------------------------
# Extract every { target = "...", link = "..." } pair from a manifest.
#
# Deliberately line-oriented rather than a TOML parse: the repo writes one
# entry per line, bash has no TOML reader, and a checker that needs a Python
# dependency to run is a checker that stops running.
# ---------------------------------------------------------------------------
FAIL_COUNT=0
CHECK_COUNT=0
FAILURES=()

check_manifest() {
    local manifest="$1"
    local rel="${manifest#"${PROJECT_ROOT}/"}"
    local in_block=false line target link ctarget clink

    while IFS= read -r line; do
        # Track the symlinks array, so a `link =` in some other table is not
        # mistaken for one of ours.
        if [[ "$line" =~ ^[[:space:]]*symlinks[[:space:]]*= ]]; then
            in_block=true
        fi
        [[ "$in_block" == "true" ]] || continue
        if [[ "$line" =~ ^[[:space:]]*\] ]]; then
            in_block=false
            continue
        fi

        [[ "$line" =~ target[[:space:]]*=[[:space:]]*\"([^\"]+)\" ]] || continue
        target="${BASH_REMATCH[1]}"
        [[ "$line" =~ link[[:space:]]*=[[:space:]]*\"([^\"]+)\" ]] || continue
        link="${BASH_REMATCH[1]}"

        CHECK_COUNT=$((CHECK_COUNT + 1))

        # Empty root: these are paths in the installed system, not host paths.
        ctarget="$(_raven_canon_merged "" "$target")"
        clink="$(_raven_canon_merged "" "$link")"

        if [[ "$ctarget" == "$clink" ]]; then
            FAIL_COUNT=$((FAIL_COUNT + 1))
            FAILURES+=("${rel}: ${link} -> ${target} (both are ${ctarget} after the merge)")
            echo -e "  ${RED}[FAIL]${NC} ${rel}"
            echo -e "         ${RED}${link} -> ${target}${NC}"
            echo -e "         ${CYAN}both name ${ctarget} on a usr-merged root; the link would replace the target${NC}"
        fi
    done < "$manifest"
}

if [[ "$QUIET" != "true" ]]; then
    echo ""
    echo -e "${BOLD}${CYAN}RavenLinux Manifest Checker${NC}"
    echo ""
fi

manifest_count=0
while IFS= read -r manifest; do
    manifest_count=$((manifest_count + 1))
    check_manifest "$manifest"
done < <(find "$PACKAGES_DIR" -name package.toml -type f | sort)

if [[ "$QUIET" != "true" ]]; then
    echo ""
    echo "  Manifests: ${manifest_count}"
    echo "  Symlink entries checked: ${CHECK_COUNT}"
    echo -e "  Failed: ${RED}${FAIL_COUNT}${NC}"
    echo ""
fi

if [[ $FAIL_COUNT -eq 0 ]]; then
    log_success "No self-referential symlinks in any package manifest"
    exit 0
fi

log_error "${FAIL_COUNT} self-referential symlink(s):"
for failure in "${FAILURES[@]}"; do
    echo -e "    ${RED}-${NC} ${failure}"
done
echo ""
log_info "Remove the entry. A binary installed to /usr/bin is already reachable"
log_info "at /bin/<name> through the merge link; the alias is not needed and"
log_info "creating it destroys the binary. See scripts/lib/usrmerge.sh."
exit 1
