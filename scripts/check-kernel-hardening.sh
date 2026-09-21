#!/bin/bash
# =============================================================================
# RavenLinux kernel hardening and hybrid-scheduling floor checker
# =============================================================================
# Diffs a kernel config against the list of options RavenLinux requires, and
# exits non-zero if any of them has been dropped.
#
# This exists because of how the config is produced. configs/kernel/config-
# 6.17-raven is a full 216 KB .config, not a fragment, and build-kernel.sh
# restores it verbatim: `cp saved .config`, `make olddefconfig`, the two floor
# scripts, `make olddefconfig` again. Nothing in that path re-derives the
# security options, so the only thing keeping them on is that they are written
# in the file -- and the file is regenerated wholesale by generate_config(), by
# a `make menuconfig` session copied back over it, or by a kernel bump where
# `make olddefconfig` answers a renamed symbol with its default.
#
# That is not hypothetical. The 6.17.11-raven kernel shipped with
# HARDENED_USERCOPY, SLAB_FREELIST_RANDOM, SLAB_FREELIST_HARDENED,
# INIT_ON_ALLOC_DEFAULT_ON and SECURITY_YAMA all off, every one of which is on
# in the Arch kernel it replaced, and nobody noticed until someone went looking
# for __check_heap_object in /proc/kallsyms and did not find it. kernel-
# ports.sh and kernel-performance.sh guard their own options by reapplying them
# on every build; the security set had no such guard. This is it.
#
# It is deliberately not a floor script. kernel-ports.sh and kernel-
# performance.sh edit a kernel tree's .config with the kernel's own
# scripts/config, so they need a kernel tree and can only run during a build.
# This runs against the checked-in file with nothing but bash, so it can run in
# CI on a machine that will never compile a kernel, and it reports rather than
# silently repairs -- a config that has drifted is a thing somebody should look
# at, not a thing a build should paper over.
#
# Usage: ./scripts/check-kernel-hardening.sh [OPTIONS] [CONFIG]
#
# Options:
#   -q, --quiet     Only print failures and the summary
#   -h, --help      Show this help message
#
# CONFIG defaults to configs/kernel/config-6.17-raven beside this script. Any
# kernel config works: a build tree's .config, /boot/config-*, or the running
# kernel's own /proc/config.gz (gzip is detected and decompressed), which is
# the way to check what actually got built rather than what was asked for.
#
# Exits non-zero if any required option is missing or set to the wrong value.
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${RAVEN_ROOT:-$(dirname "$SCRIPT_DIR")}"

if [[ -f "${PROJECT_ROOT}/scripts/lib/logging.sh" ]]; then
    # shellcheck disable=SC1091
    source "${PROJECT_ROOT}/scripts/lib/logging.sh"
else
    RED=''; GREEN=''; YELLOW=''; CYAN=''; BOLD=''; NC=''
    log_info()    { echo "[INFO] $*"; }
    log_warn()    { echo "[WARN] $*"; }
    log_error()   { echo "[ERROR] $*" >&2; }
    log_success() { echo "[SUCCESS] $*"; }
fi

DEFAULT_CONFIG="${PROJECT_ROOT}/configs/kernel/config-6.17-raven"

# =============================================================================
# The required set
# =============================================================================
# One entry per line: SYMBOL<space>VALUE<space>reason. SYMBOL is written
# without the CONFIG_ prefix. VALUE is y, m, or a quoted string exactly as it
# appears in a .config. The reason is printed when the check fails, because a
# bare symbol name tells whoever broke it nothing about whether they are
# allowed to.
#
# Everything here is either something a regeneration has already dropped once,
# or something the rest of this list depends on. The full argument for each one
# is in docs/kernel-hardening.md and in a comment at the symbol itself in
# configs/kernel/config-6.17-raven.
REQUIRED=$(cat <<'REQ'
SECURITY y                        the LSM framework itself; without it every option below is unreachable
SECURITYFS y                      /sys/kernel/security, where the active LSM list is readable from
SECURITY_PATH y                   path-based LSM hooks, which Landlock selects
SECURITY_YAMA y                   ptrace_scope: one unprivileged process cannot attach to another that is not its child
SECURITY_LANDLOCK y               unprivileged sandboxing, inert until a process asks for it
LSM "landlock,yama"               the active LSM list; selinux and apparmor stay compiled in but unlisted, with no policy to load
HARDENED_USERCOPY y               bounds-checks every copy_to_user/copy_from_user against one object
SLAB_FREELIST_RANDOM y            shuffles each slab page's free list so the next kmalloc address is not predictable
SLAB_FREELIST_HARDENED y          obfuscates the freelist next pointer and catches double frees
INIT_ON_ALLOC_DEFAULT_ON y        zeroes pages and heap objects as they are handed out
INIT_STACK_ALL_ZERO y             zeroes stack variables at function entry
STACKPROTECTOR y                  stack canaries
STACKPROTECTOR_STRONG y           canaries on every function with a local array or an address-taken local
STRICT_KERNEL_RWX y               kernel text read-only, kernel data non-executable
STRICT_MODULE_RWX y               the same for module text and data
VMAP_STACK y                      guard-paged kernel stacks, so an overflow faults instead of scribbling
RANDOMIZE_BASE y                  KASLR
RANDOMIZE_MEMORY y                randomised physmap, vmalloc and vmemmap bases
MITIGATION_PAGE_TABLE_ISOLATION y KPTI
MITIGATION_RETPOLINE y            Spectre v2 mitigation
SCHED_CLUSTER y                   L2 cluster scheduler domain, for the E-core clusters on this hybrid part
SCHED_MC_PRIO y                   builds arch/x86/kernel/itmt.c, which is the whole of the kernel's ITMT support
X86_INTEL_PSTATE y                the driver that reads CPPC highest_perf and registers ITMT
ACPI_CPPC_LIB y                   CPPC, which is where the per-core performance ranking comes from
INTEL_HFI_THERMAL y               Hardware Feedback Interface: the firmware's live which-core-is-fastest table
THERMAL_NETLINK y                 how HFI updates reach user space; INTEL_HFI_THERMAL selects it
DEBUG_FS y                        the ITMT switch moved from sysctl to debugfs; without this there is no switch at all
MODULES y                         graphics and wireless ship as modules on purpose, see etc/raven/init.toml
REQ
)

# =============================================================================
# The deliberately-off set
# =============================================================================
# Same format, but VALUE is always n and a mismatch is a NOTE rather than a
# failure. These are decisions, not defaults: somebody weighed them and wrote
# the argument down. Turning one on may well be right later, and a checker that
# refused to let anyone improve the kernel would be a bad checker -- but it
# should not happen by accident, and whoever does it should update the
# reasoning at the same time instead of leaving a comment in the config that
# now contradicts the config.
DECIDED_OFF=$(cat <<'OFF'
MODULE_SIG n                      needs a persistent signing key and breaks DKMS rebuilds of evdi; see the comment at the symbol
INIT_ON_FREE_DEFAULT_ON n         3-5% for a narrow gain over init-on-alloc; use init_on_free=1 on the command line instead
OFF
)

# =============================================================================
QUIET=false
CONFIG_ARG=""

show_help() {
    sed -n '2,44p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        -q|--quiet) QUIET=true; shift ;;
        -h|--help)  show_help; exit 0 ;;
        -*)         echo "Unknown option: $1" >&2; show_help; exit 1 ;;
        *)
            if [[ -n "$CONFIG_ARG" ]]; then
                echo "Too many arguments: $1" >&2
                exit 1
            fi
            CONFIG_ARG="$1"; shift
            ;;
    esac
done

CONFIG_FILE="${CONFIG_ARG:-$DEFAULT_CONFIG}"

if [[ ! -f "$CONFIG_FILE" ]]; then
    log_error "Not a file: ${CONFIG_FILE}"
    log_info "Pass a kernel config, or run with no argument to check ${DEFAULT_CONFIG#"${PROJECT_ROOT}/"}"
    exit 1
fi

# /proc/config.gz is the most useful thing to point this at after a build, and
# it is gzip rather than text. Sniff the magic instead of trusting the name:
# build trees call it .config whatever is in it.
WORK_CONFIG="$CONFIG_FILE"
TMP_CONFIG=""
cleanup() { [[ -n "$TMP_CONFIG" ]] && rm -f "$TMP_CONFIG"; return 0; }
trap cleanup EXIT

if [[ "$(head -c 2 "$CONFIG_FILE" | od -An -tx1 | tr -d ' \n')" == "1f8b" ]]; then
    TMP_CONFIG="$(mktemp)"
    if ! gzip -dc "$CONFIG_FILE" > "$TMP_CONFIG" 2>/dev/null; then
        log_error "${CONFIG_FILE} looks gzipped but could not be decompressed"
        exit 1
    fi
    WORK_CONFIG="$TMP_CONFIG"
fi

# ---------------------------------------------------------------------------
# Read one symbol's state out of the config.
#
# Prints the value (y, m, a number, or a quoted string) for a symbol that is
# set, and the literal word "n" for one that is absent or explicitly not set --
# which is what Kconfig means by both of those, and what makes an absent symbol
# and a disabled symbol compare equal here.
# ---------------------------------------------------------------------------
config_value() {
    local symbol="$1" line
    line="$(grep -m1 -E "^CONFIG_${symbol}=" "$WORK_CONFIG" || true)"
    if [[ -n "$line" ]]; then
        echo "${line#CONFIG_"${symbol}"=}"
    else
        echo "n"
    fi
}

FAIL_COUNT=0
PASS_COUNT=0
NOTE_COUNT=0
FAILURES=()
NOTES=()

if [[ "$QUIET" != "true" ]]; then
    echo ""
    echo -e "${BOLD}${CYAN}RavenLinux Kernel Hardening Checker${NC}"
    echo -e "  ${CYAN}${CONFIG_FILE}${NC}"
    echo ""
fi

while read -r symbol want reason; do
    [[ -n "$symbol" ]] || continue
    got="$(config_value "$symbol")"
    if [[ "$got" == "$want" ]]; then
        PASS_COUNT=$((PASS_COUNT + 1))
        [[ "$QUIET" == "true" ]] || echo -e "  ${GREEN}[ OK ]${NC} CONFIG_${symbol}=${want}"
    else
        FAIL_COUNT=$((FAIL_COUNT + 1))
        FAILURES+=("CONFIG_${symbol}: want ${want}, got ${got} -- ${reason}")
        echo -e "  ${RED}[FAIL]${NC} CONFIG_${symbol}"
        echo -e "         ${RED}want ${want}, got ${got}${NC}"
        echo -e "         ${CYAN}${reason}${NC}"
    fi
done <<< "$REQUIRED"

while read -r symbol want reason; do
    [[ -n "$symbol" ]] || continue
    got="$(config_value "$symbol")"
    if [[ "$got" != "$want" ]]; then
        NOTE_COUNT=$((NOTE_COUNT + 1))
        NOTES+=("CONFIG_${symbol} is now ${got}; it was deliberately ${want} -- ${reason}")
    fi
done <<< "$DECIDED_OFF"

# The reasoning for every option above lives in a comment next to the symbol in
# the shipped config. A regenerated config keeps the settings that
# olddefconfig could carry over and loses every comment, so the absence of this
# marker means the file was rewritten by a tool and the argument for each of
# these options now exists only in docs/. Not a failure -- the settings are
# what this checks -- but it is worth saying out loud, because the next person
# to read the config will find bare symbols with no explanation.
ANNOTATED=true
if ! grep -q "docs/kernel-hardening.md" "$WORK_CONFIG"; then
    ANNOTATED=false
fi

if [[ "$QUIET" != "true" ]]; then
    echo ""
    echo "  Required options checked: $((PASS_COUNT + FAIL_COUNT))"
    echo -e "  Failed: ${RED}${FAIL_COUNT}${NC}"
    echo ""
fi

for note in "${NOTES[@]}"; do
    log_warn "$note"
done

if [[ "$ANNOTATED" == "false" && "$CONFIG_FILE" == "$DEFAULT_CONFIG" ]]; then
    log_warn "The hardening comments are gone from ${CONFIG_FILE#"${PROJECT_ROOT}/"}"
    log_warn "That file was regenerated by a tool. Restore the comment blocks from"
    log_warn "version control, or the next reader gets bare symbols with no argument."
fi

if [[ $FAIL_COUNT -eq 0 ]]; then
    if [[ $NOTE_COUNT -gt 0 ]]; then
        log_success "All required kernel options present (${NOTE_COUNT} deliberate-off option(s) changed, see above)"
    else
        log_success "All required kernel options present"
    fi
    exit 0
fi

log_error "${FAIL_COUNT} required kernel option(s) missing or wrong:"
for failure in "${FAILURES[@]}"; do
    echo -e "    ${RED}-${NC} ${failure}"
done
echo ""
log_info "Fix the line in configs/kernel/config-6.17-raven itself -- that file is"
log_info "applied verbatim by build-kernel.sh, so editing it is the whole change."
log_info "If the symbol was renamed by a kernel bump, update this script's list in"
log_info "the same commit. See docs/kernel-hardening.md for why each one is here."
exit 1
