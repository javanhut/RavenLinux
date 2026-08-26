#!/bin/bash
# =============================================================================
# RavenLinux Root Filesystem Layout Checker
# =============================================================================
# Verifies that a rootfs is correctly usr-merged, the way Arch (and therefore
# anything installed with `rvn`) expects it to be:
#
#     /bin      -> usr/bin      /usr/sbin  -> bin
#     /sbin     -> usr/bin      /usr/lib64 -> lib
#     /lib      -> usr/lib
#     /lib64    -> usr/lib
#
# A rootfs that gets this wrong does not fail loudly -- it boots, and then
# `rvn install <anything>` drops binaries into a /sbin that PATH resolution
# and the ELF interpreter never look at, or `rvn install filesystem` aborts
# with a pile of "exists in filesystem" conflicts. This script is the cheap
# pre-flight for both.
#
# Checks performed:
#   1. /bin /sbin /lib /lib64 are symlinks resolving to the expected usr/*
#   2. /usr/sbin and /usr/lib64 are symlinks to bin and lib
#   3. no symlink loop anywhere in the tree (every link resolved, cycles found)
#   4. no dangling symlink among the top-level layout links
#   5. nothing installed as a real file inside a merged-away directory
#   6. the paths Arch's `filesystem` package ships as symlinks match what this
#      rootfs has, so `rvn install filesystem` would not conflict
#
# Usage: ./scripts/check-layout.sh [OPTIONS] [ROOTFS]
#
# Options:
#   -q, --quiet     Only print failures and the summary
#   -h, --help      Show this help message
#
# ROOTFS defaults to ${SYSROOT_DIR:-build/sysroot}.
#
# Exits non-zero if any check fails.
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${RAVEN_ROOT:-$(dirname "$SCRIPT_DIR")}"

if [[ -f "${PROJECT_ROOT}/scripts/lib/logging.sh" ]]; then
    # shellcheck disable=SC1091
    source "${PROJECT_ROOT}/scripts/lib/logging.sh"
else
    RED=''; GREEN=''; YELLOW=''; BLUE=''; CYAN=''; BOLD=''; NC=''
    log_info()    { echo "[INFO] $*"; }
    log_warn()    { echo "[WARN] $*" >&2; }
    log_error()   { echo "[ERROR] $*" >&2; }
    log_success() { echo "[SUCCESS] $*"; }
    log_section() { echo ""; echo "=== $* ==="; echo ""; }
fi

QUIET=false

# =============================================================================
# Expected layout
# =============================================================================

# vpath : literal target Arch uses : path it must resolve to inside the rootfs
TOPLEVEL_LINKS=(
    "/bin:usr/bin:/usr/bin"
    "/sbin:usr/bin:/usr/bin"
    "/lib:usr/lib:/usr/lib"
    "/lib64:usr/lib:/usr/lib"
)

USR_LINKS=(
    "/usr/sbin:bin:/usr/bin"
    "/usr/lib64:lib:/usr/lib"
)

# Directories that usr-merge merges away: after the merge these are symlinks,
# so a real file living under one of them is a package that was installed
# against a split-usr layout and is invisible to the merged one.
MERGED_AWAY=(/bin /sbin /lib /lib64 /usr/sbin /usr/lib64)

# Every path Arch's `filesystem` package ships as a symlink, with its target.
# Hardcoded because it is stable across releases; derived from the local
# machine with `pacman -Ql filesystem` (and cross-checked against the package's
# own mtree, /var/lib/pacman/local/filesystem-*/mtree, which records
# `type=link link=<target>` for exactly these eleven paths -- filesystem
# 2025.10.12-1). If any of these exists in the rootfs as something other than
# a symlink to the same target, `rvn install filesystem` conflicts on it.
FILESYSTEM_PKG_LINKS=(
    "/bin:usr/bin"
    "/lib:usr/lib"
    "/lib64:usr/lib"
    "/sbin:usr/bin"
    "/usr/lib64:lib"
    "/usr/sbin:bin"
    "/var/lock:../run/lock"
    "/var/run:../run"
    "/usr/local/share/man:../man"
    "/var/mail:spool/mail"
    "/etc/mtab:../proc/self/mounts"
)

# Kernel ELOOP budget; a resolution needing more hops than this is a loop.
MAX_HOPS=40

# =============================================================================
# Counters
# =============================================================================

PASS_COUNT=0
FAIL_COUNT=0
FAILURES=()

check_pass() {
    PASS_COUNT=$((PASS_COUNT + 1))
    [[ "$QUIET" == "true" ]] && return 0
    echo -e "  ${GREEN}[PASS]${NC} $1"
}

check_fail() {
    FAIL_COUNT=$((FAIL_COUNT + 1))
    FAILURES+=("$1")
    echo -e "  ${RED}[FAIL]${NC} $1"
}

check_note() {
    [[ "$QUIET" == "true" ]] && return 0
    echo -e "         ${CYAN}${1}${NC}"
}

# Section header that honours --quiet.
section() {
    [[ "$QUIET" == "true" ]] && return 0
    log_section "$1"
}

show_help() {
    sed -n '2,37p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

# =============================================================================
# Path helpers
# =============================================================================

# Lexically collapse "." and ".." in an absolute virtual path. No symlink is
# followed, so this works on targets whose components do not exist yet.
# Usage: normalize_vpath /var/../usr/bin  ->  /usr/bin
normalize_vpath() {
    local vpath="$1"
    local out=() comp
    local IFS='/'
    for comp in $vpath; do
        case "$comp" in
            ""|".") ;;
            "..")   [[ ${#out[@]} -gt 0 ]] && unset 'out[-1]' ;;
            *)      out+=("$comp") ;;
        esac
    done
    [[ ${#out[@]} -eq 0 ]] && { echo "/"; return 0; }
    echo "/${out[*]}"
}

# Turn a symlink's target into an absolute virtual path, as seen from the link.
# Usage: link_target_vpath /usr/sbin bin  ->  /usr/bin
link_target_vpath() {
    local link_vpath="$1" target="$2"
    if [[ "$target" == /* ]]; then
        normalize_vpath "$target"
    else
        normalize_vpath "$(dirname "$link_vpath")/$target"
    fi
}

# -----------------------------------------------------------------------------
# Symlink map: read the whole tree once so resolution costs no subprocesses.
# LINKMAP[<virtual path>] = <raw target>
# -----------------------------------------------------------------------------
declare -A LINKMAP=()
LINK_COUNT=0

build_link_map() {
    local p t vpath
    if find "$SCAN_ROOT" -maxdepth 0 -printf '' >/dev/null 2>&1; then
        # GNU find: one process for the entire tree.
        local errs="${TMPDIR:-/tmp}/raven-layout-find.$$"
        while IFS=$'\t' read -r p t; do
            vpath="${p#"$ROOTFS"}"
            [[ -z "$vpath" ]] && continue
            LINKMAP["$vpath"]="$t"
            LINK_COUNT=$((LINK_COUNT + 1))
        done < <(find "$SCAN_ROOT" $PRUNE -type l -printf '%p\t%l\n' 2>"$errs")
        scan_errors "$errs"
    else
        # BSD find (macOS): no -printf, so pay for a readlink per link.
        local errs="${TMPDIR:-/tmp}/raven-layout-find.$$"
        while IFS= read -r p; do
            vpath="${p#"$ROOTFS"}"
            [[ -z "$vpath" ]] && continue
            LINKMAP["$vpath"]="$(readlink "$p")"
            LINK_COUNT=$((LINK_COUNT + 1))
        done < <(find "$SCAN_ROOT" $PRUNE -type l 2>"$errs")
        scan_errors "$errs"
    fi
}

# A directory `find` could not read means the link map is incomplete, so
# "no symlink loops" would be a statement about the part of the tree that
# happened to be readable. That is the one direction a checker must never
# fail in, so an unreadable directory is a hard failure with its own message
# rather than a silent gap. Run the checker as the user who owns the rootfs
# (or under sudo) if this fires.
SCAN_INCOMPLETE=0
scan_errors() {
    local errs="$1"
    [[ -s "$errs" ]] || { rm -f "$errs"; return 0; }
    SCAN_INCOMPLETE=1
    check_fail "could not read the whole tree, so the symlink scan is incomplete"
    local line
    while IFS= read -r line; do
        printf '           %s\n' "$line" >&2
    done < "$errs"
    rm -f "$errs"
}

# Resolve a virtual path inside the rootfs the way the kernel would inside the
# booted system: absolute link targets are re-rooted at ROOTFS, relative ones
# resolve against the link's own directory, and nothing escapes the rootfs.
#
# Sets RESOLVED to the resolved virtual path (or, on failure, the offending
# component) and returns:
#   0  resolved
#   2  dangling  (a component does not exist)
#   3  loop      (the ELOOP hop budget ran out, as the kernel counts it)
SCAN_ROOT=""
# The kernel's own directories are not part of a rootfs layout: /proc is full
# of magic links that resolve differently per process, and walking a live /sys
# costs minutes for nothing. Filled in by main() once SCAN_ROOT is known, and
# matched by full path so that only the top-level ones are pruned -- a real
# directory named `proc` deeper in the tree is still scanned. Deliberately
# unquoted at the use site: these are separate find arguments.
PRUNE=""
RESOLVED=""
resolve_in_root() {
    local vpath="$1"
    local cur="" rest="${vpath#/}" comp target next
    local hops=0

    while [[ -n "$rest" ]]; do
        if [[ "$rest" == */* ]]; then
            comp="${rest%%/*}"
            rest="${rest#*/}"
        else
            comp="$rest"
            rest=""
        fi

        case "$comp" in
            ""|".") continue ;;
            "..")   cur="${cur%/*}"; continue ;;
        esac

        next="${cur}/${comp}"

        if [[ -n "${LINKMAP[$next]+x}" ]]; then
            # The hop budget is the whole rule, exactly as the kernel has it.
            # Refusing to traverse one link twice is stricter than the kernel
            # and fails correct trees: on a usr-merged root a two-link chain
            # routinely passes through /bin more than once.
            hops=$((hops + 1))
            if [[ $hops -gt $MAX_HOPS ]]; then
                RESOLVED="$next"
                return 3
            fi
            target="${LINKMAP[$next]}"
            if [[ "$target" == /* ]]; then
                cur=""
                rest="${target#/}${rest:+/$rest}"
            else
                rest="${target}${rest:+/$rest}"
            fi
            continue
        fi

        cur="$next"
        if [[ ! -e "${ROOTFS}${next}" ]]; then
            RESOLVED="$next"
            return 2
        fi
    done

    RESOLVED="${cur:-/}"
    return 0
}

# Describe what actually sits at a virtual path, for failure messages.
describe_vpath() {
    local vpath="$1" full="${ROOTFS}${vpath}"
    if [[ -L "$full" ]]; then
        echo "a symlink -> $(readlink "$full")"
    elif [[ -d "$full" ]]; then
        echo "a real directory"
    elif [[ -f "$full" ]]; then
        echo "a regular file"
    elif [[ -e "$full" ]]; then
        echo "a non-directory file"
    else
        echo "missing"
    fi
}

# =============================================================================
# Checks
# =============================================================================

# Shared by checks 1 and 2: one entry of "vpath:arch target:expected resolution".
check_layout_link() {
    local entry="$1"
    local vpath arch_target expect rc
    IFS=':' read -r vpath arch_target expect <<< "$entry"

    if [[ ! -L "${ROOTFS}${vpath}" ]]; then
        check_fail "${vpath} is not a symlink (it is $(describe_vpath "$vpath")); expected ${vpath} -> ${arch_target}"
        return 1
    fi

    rc=0
    resolve_in_root "$vpath" || rc=$?
    case $rc in
        2)
            check_fail "${vpath} -> $(readlink "${ROOTFS}${vpath}") is dangling (${RESOLVED} does not exist)"
            return 1
            ;;
        3)
            check_fail "${vpath} -> $(readlink "${ROOTFS}${vpath}") is a symlink loop (cycle at ${RESOLVED})"
            return 1
            ;;
    esac

    if [[ "$RESOLVED" != "$expect" ]]; then
        check_fail "${vpath} -> $(readlink "${ROOTFS}${vpath}") resolves to ${RESOLVED}, expected ${expect}"
        return 1
    fi

    if [[ ! -d "${ROOTFS}${RESOLVED}" ]]; then
        check_fail "${vpath} resolves to ${RESOLVED}, which is not a directory"
        return 1
    fi

    check_pass "${vpath} -> $(readlink "${ROOTFS}${vpath}")  (resolves to ${RESOLVED})"
    return 0
}

check_toplevel_links() {
    section "Top-level layout symlinks"
    local entry
    for entry in "${TOPLEVEL_LINKS[@]}"; do
        check_layout_link "$entry" || true
    done
    return 0
}

check_usr_links() {
    section "/usr internal symlinks"
    local entry
    for entry in "${USR_LINKS[@]}"; do
        check_layout_link "$entry" || true
    done
    return 0
}

check_no_loops() {
    section "Symlink loops"

    local vpath rc loops=0 checked=0
    local -a loop_paths=()

    for vpath in "${!LINKMAP[@]}"; do
        checked=$((checked + 1))
        rc=0
        resolve_in_root "$vpath" || rc=$?
        if [[ $rc -eq 3 ]]; then
            loops=$((loops + 1))
            loop_paths+=("${vpath} -> ${LINKMAP[$vpath]}  (cycle at ${RESOLVED})")
        fi
    done

    if [[ $loops -eq 0 ]]; then
        check_pass "no symlink loops (${checked} symlink(s) resolved)"
    else
        check_fail "${loops} symlink loop(s) found among ${checked} symlink(s)"
        local entry n=0
        for entry in "${loop_paths[@]}"; do
            n=$((n + 1))
            if [[ $n -gt 10 ]]; then
                echo -e "         ${RED}... and $((loops - 10)) more${NC}"
                break
            fi
            echo -e "         ${RED}${entry}${NC}"
        done
    fi
    return 0
}

check_no_dangling() {
    section "Dangling layout symlinks"

    local entry vpath rc dangling=0
    for entry in "${TOPLEVEL_LINKS[@]}" "${USR_LINKS[@]}"; do
        IFS=':' read -r vpath _ _ <<< "$entry"
        [[ -L "${ROOTFS}${vpath}" ]] || continue
        rc=0
        resolve_in_root "$vpath" || rc=$?
        if [[ $rc -eq 2 ]]; then
            dangling=$((dangling + 1))
            check_fail "${vpath} -> ${LINKMAP[$vpath]} is dangling (${RESOLVED} does not exist)"
        fi
    done

    if [[ $dangling -eq 0 ]]; then
        check_pass "no dangling symlinks among the layout links"
    fi
    return 0
}

check_merged_away_dirs() {
    section "Real files in merged-away directories"

    local vpath full count sample
    for vpath in "${MERGED_AWAY[@]}"; do
        full="${ROOTFS}${vpath}"

        if [[ -L "$full" ]]; then
            check_pass "${vpath} holds nothing of its own (it is a symlink)"
            continue
        fi

        if [[ ! -e "$full" ]]; then
            check_pass "${vpath} holds nothing of its own (absent)"
            continue
        fi

        if [[ ! -d "$full" ]]; then
            check_fail "${vpath} exists as $(describe_vpath "$vpath"); it must be a symlink into /usr"
            continue
        fi

        # `|| true`: find exits 1 on an unreadable subdirectory and the
        # pipeline takes its status, which under `set -e` killed the whole
        # report mid-run -- no summary, no failure list, and checks 5 and 6
        # never ran. scan_errors has already flagged the unreadable tree.
        count="$( { find "$full" -mindepth 1 2>/dev/null || true; } | wc -l)"
        if [[ "$count" -eq 0 ]]; then
            check_pass "${vpath} is a real directory but is empty (nothing installed into it)"
            continue
        fi

        check_fail "${vpath} is a real directory holding ${count} entr$([[ $count -eq 1 ]] && echo y || echo ies) installed outside /usr"
        while IFS= read -r sample; do
            echo -e "         ${RED}${sample#"$ROOTFS"}${NC}"
        done < <(find "$full" -mindepth 1 2>/dev/null | head -5)
        if [[ "$count" -gt 5 ]]; then
            echo -e "         ${RED}... and $((count - 5)) more${NC}"
        fi
    done
    return 0
}

check_filesystem_pkg_compat() {
    section "rvn install filesystem (Arch filesystem package symlinks)"

    local entry vpath want want_vpath have have_vpath rc_have rc_want resolved_have resolved_want

    for entry in "${FILESYSTEM_PKG_LINKS[@]}"; do
        IFS=':' read -r vpath want <<< "$entry"
        want_vpath="$(link_target_vpath "$vpath" "$want")"

        if [[ -L "${ROOTFS}${vpath}" ]]; then
            have="${LINKMAP[$vpath]:-$(readlink "${ROOTFS}${vpath}")}"
            have_vpath="$(link_target_vpath "$vpath" "$have")"

            if [[ "$have_vpath" == "$want_vpath" ]]; then
                check_pass "${vpath} -> ${have}  (matches filesystem: ${want})"
                continue
            fi

            # Different spelling is fine as long as it lands in the same place.
            rc_have=0; resolve_in_root "$vpath" || rc_have=$?; resolved_have="$RESOLVED"
            rc_want=0; resolve_in_root "$want_vpath" || rc_want=$?; resolved_want="$RESOLVED"
            if [[ $rc_have -eq 0 && $rc_want -eq 0 && "$resolved_have" == "$resolved_want" ]]; then
                check_pass "${vpath} -> ${have}  (spelled differently from filesystem's '${want}', same destination ${resolved_have})"
                continue
            fi

            check_fail "${vpath} -> ${have} but filesystem ships ${vpath} -> ${want}; rvn install filesystem would conflict"
            continue
        fi

        if [[ ! -e "${ROOTFS}${vpath}" ]]; then
            check_pass "${vpath} absent (filesystem will create it as -> ${want})"
            continue
        fi

        # rvn's rule, from find_conflicts in RavenPackageManager/src/extract.rs:
        # only a *populated* real directory is a type conflict. An empty one is
        # removed and replaced by the link -- unless another package owns it,
        # which this script cannot see from the filesystem alone.
        if [[ -d "${ROOTFS}${vpath}" && ! -L "${ROOTFS}${vpath}" ]] \
            && [[ -z "$( { find "${ROOTFS}${vpath}" -mindepth 1 -maxdepth 1 2>/dev/null || true; } | head -1)" ]]; then
            check_pass "${vpath} is an empty real directory; filesystem's symlink -> ${want} replaces it"
            continue
        fi

        check_fail "${vpath} is $(describe_vpath "$vpath") but filesystem ships it as a symlink -> ${want}; rvn install filesystem would conflict"
    done
    return 0
}

# =============================================================================
# Main
# =============================================================================

main() {
    local rootfs_arg=""

    while [[ $# -gt 0 ]]; do
        case "$1" in
            -q|--quiet) QUIET=true; shift ;;
            -h|--help)  show_help; exit 0 ;;
            -*)         log_error "Unknown option: $1"; show_help; exit 1 ;;
            *)
                if [[ -n "$rootfs_arg" ]]; then
                    log_error "Too many arguments: $1"
                    exit 1
                fi
                rootfs_arg="$1"; shift
                ;;
        esac
    done

    local target="${rootfs_arg:-${SYSROOT_DIR:-${PROJECT_ROOT}/build/sysroot}}"

    if [[ ! -d "$target" ]]; then
        log_error "Not a directory: ${target}"
        log_info "Usage: $(basename "${BASH_SOURCE[0]}") [OPTIONS] [ROOTFS]"
        exit 1
    fi

    # Absolute, with any trailing slash gone: every check builds paths by
    # string-prefixing ROOTFS onto a virtual path.
    ROOTFS="$(cd "$target" && pwd -P)"
    # ROOTFS is a *prefix*, so checking / has to spell it as the empty string
    # or every virtual path would come out doubled. `find` cannot take an empty
    # argument, so the scan keeps the real spelling separately -- without this
    # the link map came back empty and a perfectly merged host failed all six
    # checks with "Symlinks scanned: 0".
    SCAN_ROOT="$ROOTFS"
    if [[ "$ROOTFS" == "/" ]]; then
        ROOTFS=""
    fi

    local pseudo
    for pseudo in proc sys dev run; do
        PRUNE+=" -path ${SCAN_ROOT%/}/${pseudo} -prune -o"
    done

    if [[ "$QUIET" != "true" ]]; then
        echo ""
        echo -e "${BOLD}${CYAN}RavenLinux Layout Checker${NC}"
        echo ""
        log_info "Rootfs: ${ROOTFS:-/}"
    fi

    build_link_map

    check_toplevel_links
    check_usr_links
    check_no_loops
    check_no_dangling
    check_merged_away_dirs
    check_filesystem_pkg_compat

    section "Summary"

    echo "  Rootfs:  ${ROOTFS:-/}"
    echo "  Symlinks scanned: ${LINK_COUNT}"
    echo -e "  Passed:  ${GREEN}${PASS_COUNT}${NC}"
    echo -e "  Failed:  ${RED}${FAIL_COUNT}${NC}"
    echo ""

    if [[ $FAIL_COUNT -eq 0 ]]; then
        log_success "Layout is correctly usr-merged"
        return 0
    fi

    log_error "Layout is NOT correctly usr-merged (${FAIL_COUNT} failed check(s)):"
    local failure
    for failure in "${FAILURES[@]}"; do
        echo -e "    ${RED}-${NC} ${failure}"
    done
    echo ""
    log_info "Fix the rootfs so /bin /sbin /lib /lib64 are symlinks into /usr"
    log_info "(and /usr/sbin -> bin, /usr/lib64 -> lib) before installing packages into it."
    return 1
}

main "$@"
