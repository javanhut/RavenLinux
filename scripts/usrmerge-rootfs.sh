#!/bin/bash
# =============================================================================
# RavenLinux split-usr -> usr-merged Root Filesystem Converter
# =============================================================================
# Converts an EXISTING RavenLinux rootfs from the split-usr layout to the
# usr-merged layout, in place, on a TARGET DIRECTORY:
#
#   /bin       -> usr/bin      /usr/sbin  -> bin        (i.e. /usr/bin)
#   /sbin      -> usr/bin      /usr/lib64 -> lib        (i.e. /usr/lib)
#   /lib       -> usr/lib
#   /lib64     -> usr/lib
#
# This is the migration path for machines that were installed BEFORE the build
# stages started producing a merged sysroot. New installs do not need it: the
# installer copies the rootfs verbatim, so a fixed stage1/stage4 is enough.
#
# Usage:
#   ./scripts/usrmerge-rootfs.sh /mnt/target             # dry run (default)
#   ./scripts/usrmerge-rootfs.sh /mnt/target --apply     # actually convert
#   ./scripts/usrmerge-rootfs.sh /mnt/target --apply -v  # ...and narrate
#
# Options:
#   --apply               perform the conversion (without this, nothing changes)
#   -v, --verbose         print every planned action, not just the summary
#   --allow-crossdev      proceed when /usr is a separate filesystem
#   --i-know-this-is-live allow TARGET to be "/" (still refuses a live root)
#   -h, --help            this text
#
# Safety model:
#   * Dry run by default. --apply is required to touch anything.
#   * Refuses "/" without --i-know-this-is-live, and refuses ANY target that is
#     the root this process is itself running from -- see the LIVE ROOT note.
#   * Idempotent: an already-merged tree is reported and left alone.
#   * Real files are moved with rename(2), never copied: /lib is ~1.1G on a
#     Raven install (firmware + modules) and a copy-based merge would need that
#     much headroom and could die of ENOSPC halfway.
#   * Collisions where both sides hold a real file are resolved ONLY when the
#     content and mode are byte-identical. Anything else aborts with a report
#     and changes nothing. The script never guesses which binary you meant.
#   * Nothing is deleted until every file has been moved, and every original
#     path name is re-resolved afterwards and checked against the object it
#     used to name. Directories that exist on only one side move as a unit and
#     are verified as a unit (a rename cannot change what is inside them).
#
# LIVE ROOT -- why this cannot convert the system it is running on:
#   Replacing /lib with a symlink while the dynamic loader is being resolved
#   out of it is not survivable from a shell script. bash is dynamically
#   linked; every external command it runs needs /lib64/ld-linux-x86-64.so.2
#   at exec() time. There is a way to do this live (renameat2(RENAME_EXCHANGE)
#   from a statically linked binary, which rvn and raven-init are), but it is
#   not this script. Run this from the ISO against a mounted target, exactly
#   the way raven-install operates on $TARGET.
#
# THE HAZARD THIS SCRIPT EXISTS TO AVOID:
#   A Raven sysroot has symlinks pointing BOTH ways across the split, e.g.
#   /usr/bin/bash -> ../../bin/bash while /bin/bash is the real ELF. The
#   textbook usrmerge algorithm ("on collision keep the destination") keeps
#   that symlink, deletes the real file with the old /bin, and leaves
#   /usr/bin/bash resolving to itself: ELOOP, no shell, and the installer's
#   rescue entry is init=/bin/bash, so the recovery path is dead too. This
#   script detects any entry that would become self-referential after the
#   merge and drops it so the real file lands in its place.
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${RAVEN_ROOT:-$(dirname "$SCRIPT_DIR")}"

if [[ -f "${PROJECT_ROOT}/scripts/lib/logging.sh" ]]; then
    # shellcheck disable=SC1091
    source "${PROJECT_ROOT}/scripts/lib/logging.sh"
else
    log_info()    { echo "[INFO] $*"; }
    log_warn()    { echo "[WARN] $*" >&2; }
    log_error()   { echo "[ERROR] $*" >&2; }
    log_success() { echo "[SUCCESS] $*"; }
    log_step()    { echo ""; echo "==> $*"; }
fi

ROOT=""
APPLY=0
VERBOSE=0
ALLOW_LIVE=0
ALLOW_CROSSDEV=0

show_help() {
    sed -n '2,60p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --apply)                APPLY=1; shift ;;
        -v|--verbose)           VERBOSE=1; shift ;;
        --allow-crossdev)       ALLOW_CROSSDEV=1; shift ;;
        --i-know-this-is-live)  ALLOW_LIVE=1; shift ;;
        -h|--help)              show_help; exit 0 ;;
        -*) log_error "Unknown option: $1"; echo; show_help; exit 1 ;;
        *)  if [[ -n "$ROOT" ]]; then
                log_error "Only one target rootfs may be given (got '$ROOT' and '$1')"
                exit 1
            fi
            ROOT="$1"; shift ;;
    esac
done

if [[ -z "$ROOT" ]]; then
    log_error "No target rootfs given."
    echo
    show_help
    exit 1
fi

# =============================================================================
# Path helpers -- everything below works in paths RELATIVE to $ROOT, and never
# lets the host filesystem leak in. A rootfs full of absolute symlinks
# (/bin/login -> /sbin/login) would otherwise be resolved by the kernel against
# the HOST's /, which on a build machine points at a completely different
# userland. Every resolution here is done component by component with $ROOT
# standing in for "/".
# =============================================================================

# resolve_in_root REL
#   Prints the relative path of the real object REL names, resolving symlinks
#   chroot-style. Returns 1 if REL is dangling or loops.
resolve_in_root() {
    local queue="$1" out="" comp tgt budget=64 cur
    while [[ -n "$queue" ]]; do
        comp="${queue%%/*}"
        if [[ "$comp" == "$queue" ]]; then queue=""; else queue="${queue#*/}"; fi
        case "$comp" in
            ''|'.') continue ;;
            '..')
                if [[ "$out" == */* ]]; then out="${out%/*}"; else out=""; fi
                continue ;;
        esac
        if [[ -z "$out" ]]; then cur="$comp"; else cur="$out/$comp"; fi
        if [[ -L "$ROOT/$cur" ]]; then
            (( budget-- > 0 )) || return 1
            tgt="$(readlink "$ROOT/$cur")"
            if [[ "$tgt" == /* ]]; then out=""; tgt="${tgt#/}"; fi
            if [[ -n "$queue" ]]; then queue="$tgt/$queue"; else queue="$tgt"; fi
        elif [[ -e "$ROOT/$cur" ]]; then
            out="$cur"
        else
            return 1
        fi
    done
    [[ -n "$out" ]] || return 1
    printf '%s' "$out"
}

# merged_path REL -> where REL will live once the conversion is done.
merged_path() {
    local p="$1"
    case "$p" in
        bin|bin/*)   printf 'usr/%s' "$p" ;;
        sbin)        printf 'usr/bin' ;;
        sbin/*)      printf 'usr/bin/%s' "${p#sbin/}" ;;
        lib)         printf 'usr/lib' ;;
        lib/*)       printf 'usr/lib/%s' "${p#lib/}" ;;
        lib64)       printf 'usr/lib' ;;
        lib64/*)     printf 'usr/lib/%s' "${p#lib64/}" ;;
        usr/sbin)    printf 'usr/bin' ;;
        usr/sbin/*)  printf 'usr/bin/%s' "${p#usr/sbin/}" ;;
        usr/lib64)   printf 'usr/lib' ;;
        usr/lib64/*) printf 'usr/lib/%s' "${p#usr/lib64/}" ;;
        *)           printf '%s' "$p" ;;
    esac
}

is_dir()  { [[ -d "$ROOT/$1" && ! -L "$ROOT/$1" ]]; }
lexists() { [[ -e "$ROOT/$1" || -L "$ROOT/$1" ]]; }

# =============================================================================
# Preflight
# =============================================================================
log_step "Preflight"

[[ -d "$ROOT" ]] || { log_error "Not a directory: $ROOT"; exit 1; }
ROOT="$(cd "$ROOT" && pwd -P)"

# --- is this "/"? -------------------------------------------------------------
ROOT_ID="$(stat -c '%d:%i' "$ROOT")"
SLASH_ID="$(stat -c '%d:%i' /)"
IS_SLASH=0
[[ "$ROOT_ID" == "$SLASH_ID" ]] && IS_SLASH=1

if [[ $IS_SLASH -eq 1 && $ALLOW_LIVE -eq 0 ]]; then
    log_error "Refusing to convert '/'."
    log_error ""
    log_error "  This tool rewrites /bin, /sbin, /lib and /lib64 into symlinks."
    log_error "  Doing that to the root you are booted from removes the directory"
    log_error "  the dynamic loader is being resolved out of, mid-run, from a"
    log_error "  script whose own shell needs that loader for every command it"
    log_error "  spawns. There is no recovery: the rescue boot entry Raven writes"
    log_error "  is init=/bin/bash, which is exactly what breaks."
    log_error ""
    log_error "  Boot the ISO, mount the installed root, and point this at that"
    log_error "  directory instead -- the same way raven-install works on \$TARGET."
    log_error "  If you truly mean a chroot-style target that merely happens to be"
    log_error "  spelled '/', pass --i-know-this-is-live (it will still refuse a"
    log_error "  root this process is running from)."
    exit 1
fi

# --- is this the root WE are running from? -----------------------------------
# The real question is not "is the path spelled /" but "is the userland under
# $ROOT the one backing this process". Compare the dev:inode of the loader and
# libc this shell has mapped against the same paths under $ROOT. A bind mount
# of / somewhere else is caught by this and not by the path check above.
LIVE_HITS=()
if [[ ! -r /proc/self/maps ]]; then
    log_error "Refusing: /proc/self/maps is unreadable, so the live-root check"
    log_error "  cannot run."
    log_error ""
    log_error "  That check is the only thing standing between this script and"
    log_error "  turning the running system's /lib into a symlink underneath the"
    log_error "  shell executing it. It fails closed: no evidence is not the same"
    log_error "  as evidence of safety, and a chroot without /proc mounted -- the"
    log_error "  exact case --i-know-this-is-live exists for -- is where this"
    log_error "  would otherwise wave the operation through."
    log_error ""
    log_error "  Mount /proc in the target environment and re-run, or do the"
    log_error "  conversion offline from the Raven ISO."
    exit 1
fi

MAPPED_COUNT=0
if [[ -r /proc/self/maps ]]; then
    while IFS= read -r mapped; do
        [[ -n "$mapped" ]] || continue
        [[ -e "$ROOT$mapped" ]] || continue
        a="$(stat -c '%d:%i' "$mapped" 2>/dev/null || true)"
        b="$(stat -c '%d:%i' "$ROOT$mapped" 2>/dev/null || true)"
        [[ -n "$a" && "$a" == "$b" ]] && LIVE_HITS+=("$mapped")
    done < <(awk '$6 ~ /(ld-linux|ld-musl|\/libc\.so|\/libc-)/ {print $6}' /proc/self/maps | sort -u)
    MAPPED_COUNT="$(awk '$6 ~ /(ld-linux|ld-musl|\/libc\.so|\/libc-)/ {print $6}' /proc/self/maps | sort -u | wc -l)"
fi

# A statically linked shell maps no loader and no libc, so the comparison above
# has nothing to compare and would pass by default. Same reasoning as the
# unreadable-/proc case: refuse rather than assume.
if [[ "$MAPPED_COUNT" -eq 0 ]]; then
    log_error "Refusing: this shell maps no dynamic loader or libc, so there is"
    log_error "  nothing to compare against '$ROOT' and the live-root check cannot"
    log_error "  reach a verdict. It fails closed. Do the conversion offline."
    exit 1
fi

if [[ ${#LIVE_HITS[@]} -gt 0 ]]; then
    log_error "Refusing: '$ROOT' is the live root this process is running from."
    log_error ""
    for h in "${LIVE_HITS[@]}"; do
        log_error "  in use by this shell: ${h}  ==  ${ROOT}${h}"
    done
    log_error ""
    log_error "  Those are the loader/libc mappings of the very process doing the"
    log_error "  conversion. Turning their directory into a symlink underneath a"
    log_error "  running bash bricks the machine before the merge finishes, and"
    log_error "  --i-know-this-is-live does not override this check."
    log_error ""
    log_error "  Do it offline: boot the Raven ISO, mount the target root, run"
    log_error "    ./usrmerge-rootfs.sh /mnt --apply"
    exit 1
fi

[[ -d "$ROOT/usr" ]] || { log_error "No 'usr' directory under $ROOT -- not a rootfs."; exit 1; }

if [[ $IS_SLASH -eq 1 ]]; then
    log_warn "Target is '/', allowed by --i-know-this-is-live."
    log_warn "The loader check passed, so this shell is not backed by that root,"
    log_warn "but be certain you know which filesystem you are pointing at."
fi

log_info "Target rootfs: ${ROOT}"

# --- separate /usr? ----------------------------------------------------------
DEV_ROOT="$(stat -c '%d' "$ROOT")"
DEV_USR="$(stat -c '%d' "$ROOT/usr")"
if [[ "$DEV_ROOT" != "$DEV_USR" ]]; then
    if [[ $ALLOW_CROSSDEV -eq 0 ]]; then
        log_error "/usr is on a different filesystem than / under this target."
        log_error "  Every move would become a cross-device copy. On a Raven install"
        log_error "  /lib alone is ~1.1G (firmware + modules), so this needs that much"
        log_error "  free space in /usr and can fail with ENOSPC partway through --"
        log_error "  which is the one unrecoverable failure mode here."
        log_error "  Re-run with --allow-crossdev if you have checked the headroom."
        exit 1
    fi
    log_warn "/usr is a separate filesystem; moves become copies (--allow-crossdev)."
fi

# A separate filesystem mounted *inside* a source directory (an installer's
# leftover bind mount, /lib/modules on its own subvolume) is invisible to the
# comparison above: dev(/) and dev(/usr) match, while dev(/lib/modules) does
# not. Every move out of it silently becomes a copy, hardlinks break, and the
# rmdir at the end fails with EBUSY half way through the conversion.
for probe in bin sbin usr/sbin lib lib64 usr/lib64; do
    [[ -d "$ROOT/$probe" && ! -L "$ROOT/$probe" ]] || continue
    probe_dev="$(stat -c '%d' "$ROOT/$probe" 2>/dev/null || echo unknown)"
    while IFS= read -r inner; do
        [[ -n "$inner" ]] || continue
        # A directory whose st_dev differs from the top of the set it lives in
        # is a mount point, whichever filesystem /usr itself is on.
        [[ "$(stat -c '%d' "$inner" 2>/dev/null)" == "$probe_dev" ]] && continue
        if [[ $ALLOW_CROSSDEV -eq 0 ]]; then
            log_error "'${inner#"$ROOT"}' is a mount point inside a directory this"
            log_error "  conversion has to empty."
            log_error ""
            log_error "  Moving across it copies instead of renaming, breaks hard"
            log_error "  links, and leaves the mount point behind so the final rmdir"
            log_error "  fails -- part-converted, which is the state to avoid."
            log_error ""
            log_error "  Unmount it first, or re-run with --allow-crossdev if you"
            log_error "  have checked the headroom."
            exit 1
        fi
        log_warn "mount point inside a source directory: ${inner#"$ROOT"}"
    done < <(find "$ROOT/$probe" -type d 2>/dev/null)
done

# =============================================================================
# Which sets still need merging?  (this is also the idempotence check)
# =============================================================================
SET_SRC=(bin sbin usr/sbin lib lib64 usr/lib64)
SET_DST=(usr/bin usr/bin usr/bin usr/lib usr/lib usr/lib)
SET_REL=(usr/bin usr/bin bin usr/lib usr/lib lib)   # symlink text to write

PENDING=()
LINK_ONLY=()
for i in "${!SET_SRC[@]}"; do
    s="${SET_SRC[$i]}"
    if [[ -L "$ROOT/$s" ]]; then
        r="$(resolve_in_root "$s" || true)"
        if [[ "$r" == "${SET_DST[$i]}" ]]; then
            [[ $VERBOSE -eq 1 ]] && log_info "  /$s is already a symlink to /${SET_DST[$i]}"
        else
            log_warn "  /$s is a symlink but resolves to '/${r:-<dangling>}', not /${SET_DST[$i]}"
        fi
    elif [[ ! -e "$ROOT/$s" ]]; then
        # Absent, so there is nothing to move -- but the compat symlink still
        # has to exist, or a merged tree ends up without /usr/lib64 and the
        # closing banner claims a layout the script did not produce. Recorded
        # separately: no files to migrate, just the link to create.
        [[ $VERBOSE -eq 1 ]] && log_info "  /$s does not exist -- creating the link only"
        LINK_ONLY+=("$i")
    else
        PENDING+=("$i")
    fi
done

# The link-only sets need no planning pass, so they are appended straight to
# the tail of the plan below.
if [[ ${#PENDING[@]} -eq 0 && ${#LINK_ONLY[@]} -eq 0 ]]; then

    log_success "Already usr-merged: nothing to do."
    log_info "  /bin /sbin /lib /lib64 /usr/sbin /usr/lib64 are symlinks."
    exit 0
fi

if [[ ${#PENDING[@]} -gt 0 ]]; then
    log_info "Split directories still to merge: $(for i in "${PENDING[@]}"; do printf '/%s ' "${SET_SRC[$i]}"; done)"
fi
if [[ ${#LINK_ONLY[@]} -gt 0 ]]; then
    log_info "Absent, link only: $(for i in "${LINK_ONLY[@]}"; do printf '/%s ' "${SET_SRC[$i]}"; done)"
fi

# =============================================================================
# Planning
# =============================================================================
# The whole conversion is computed against the UNMODIFIED tree first, then
# executed. That ordering is not cosmetic: a merge that resolves symlinks as it
# goes will, halfway through, see /bin/login -> /sbin/login as dangling because
# /sbin/login has already moved, and a dangling link can then win a collision
# against the real binary. Planning first makes every resolution see the same
# consistent tree.
# =============================================================================
PLAN_ACT=(); PLAN_A=(); PLAN_B=()
CONF_A=(); CONF_B=(); CONF_WHY=()
declare -A EXPECT_KEY=()          # original name -> expected object id
EXPECT_NAMES=()
STAT_MOVES=0; STAT_DROPS=0; STAT_MKDIRS=0; STAT_COLL=0; STAT_SELFREF=0

plan_add()  { PLAN_ACT+=("$1"); PLAN_A+=("$2"); PLAN_B+=("${3-}"); }

# Sets whose symlink is already in place, so an abort part way through can say
# what state the tree is actually in rather than guessing.
DONE_SETS=()
conflict()  { CONF_A+=("$1"); CONF_B+=("$2"); CONF_WHY+=("$3"); }

expect_set() {  # name, expected-id
    if [[ -z "${EXPECT_KEY[$1]+x}" ]]; then EXPECT_NAMES+=("$1"); fi
    EXPECT_KEY[$1]="$2"
}

# object_id REL -> "I:<dev>:<ino>" for something that resolves, "D" for dangling
object_id() {
    local r
    if r="$(resolve_in_root "$1")"; then
        printf 'I:%s' "$(stat -c '%d:%i' "$ROOT/$r")"
    else
        printf 'D'
    fi
}

# same_content A B -> 0 identical (content+mode+type), else sets CMP_WHY
CMP_WHY=""
same_content() {
    local ra rb
    CMP_WHY=""
    ra="$(resolve_in_root "$1")" || { CMP_WHY="dangling: /$1"; return 1; }
    rb="$(resolve_in_root "$2")" || { CMP_WHY="dangling: /$2"; return 1; }
    local ta tb ma mb
    ta="$(stat -c '%F' "$ROOT/$ra")"; tb="$(stat -c '%F' "$ROOT/$rb")"
    if [[ "$ta" != "$tb" ]]; then CMP_WHY="different file types ($ta vs $tb)"; return 1; fi
    ma="$(stat -c '%a' "$ROOT/$ra")"; mb="$(stat -c '%a' "$ROOT/$rb")"
    if [[ "$ma" != "$mb" ]]; then CMP_WHY="same name, different mode (0$ma vs 0$mb)"; return 1; fi
    local ua ub
    ua="$(stat -c '%u:%g' "$ROOT/$ra")"; ub="$(stat -c '%u:%g' "$ROOT/$rb")"
    if [[ "$ua" != "$ub" ]]; then
        CMP_WHY="same name, different owner (${ua} vs ${ub})"
        return 1
    fi
    # The loser of a collision is deleted, so anything not compared here is
    # gone for good. File capabilities live in security.capability: dropping
    # that from ping or dumpcap breaks them for every non-root user, silently,
    # with the run still reporting success. Mode alone does not see it.
    if command -v getfattr >/dev/null 2>&1; then
        local xa xb
        xa="$(getfattr -d -m - --absolute-names "$ROOT/$ra" 2>/dev/null | tail -n +2 | sort || true)"
        xb="$(getfattr -d -m - --absolute-names "$ROOT/$rb" 2>/dev/null | tail -n +2 | sort || true)"
        if [[ "$xa" != "$xb" ]]; then
            CMP_WHY="same name and content, different extended attributes (capabilities/ACLs)"
            return 1
        fi
    fi
    if [[ "$ta" == "regular file" || "$ta" == "regular empty file" ]]; then
        cmp -s "$ROOT/$ra" "$ROOT/$rb" || { CMP_WHY="different content"; return 1; }
    fi
    return 0
}

plan_level() {
    local final="$1"; shift
    local -a contribs=("$@")
    local -a names=()
    local -A seen=()
    local c n

    for c in "${contribs[@]}"; do
        is_dir "$c" || continue
        while IFS= read -r -d '' n; do
            [[ -n "${seen[$n]+x}" ]] && continue
            seen[$n]=1
            names+=("$n")
        done < <(find "$ROOT/$c" -mindepth 1 -maxdepth 1 -printf '%f\0' 2>/dev/null)
    done

    [[ ${#names[@]} -eq 0 ]] && return 0
    for n in "${names[@]}"; do
        plan_name "$final" "$n" "${contribs[@]}"
    done
}

plan_name() {
    local final="$1" n="$2"; shift 2
    local -a contribs=("$@")
    local dst="$final/$n"
    local c p r

    # ---- gather candidates, in contributor (PATH) order ---------------------
    local -a cands=()
    for c in "${contribs[@]}"; do
        is_dir "$c" || continue
        lexists "$c/$n" && cands+=("$c/$n")
    done
    [[ ${#cands[@]} -eq 0 ]] && return 0

    # ---- drop anything that would become self-referential --------------------
    # /usr/bin/bash -> ../../bin/bash is the killer: after /bin becomes a
    # symlink to usr/bin, that target resolves back to the link itself.
    local -a keep=()
    for p in "${cands[@]}"; do
        if [[ -L "$ROOT/$p" ]]; then
            if r="$(resolve_in_root "$p")"; then
                if [[ "$(merged_path "$r")" == "$dst" ]]; then
                    plan_add DROP "$p" "would self-reference after merge (-> $(readlink "$ROOT/$p"))"
                    STAT_DROPS=$((STAT_DROPS+1)); STAT_SELFREF=$((STAT_SELFREF+1))
                    continue
                fi
            fi
        fi
        keep+=("$p")
    done

    if [[ ${#keep[@]} -eq 0 ]]; then
        conflict "$dst" "-" "every candidate is a symlink loop; no real file backs this name"
        return 0
    fi

    # ---- directories --------------------------------------------------------
    local ndirs=0
    for p in "${keep[@]}"; do is_dir "$p" && ndirs=$((ndirs+1)); done

    if [[ $ndirs -gt 0 && $ndirs -ne ${#keep[@]} ]]; then
        local a="" b=""
        for p in "${keep[@]}"; do
            if is_dir "$p"; then a="$p"; else b="$p"; fi
        done
        conflict "$a" "$b" "directory on one side, file on the other"
        return 0
    fi

    if [[ $ndirs -gt 0 ]]; then
        if [[ ${#keep[@]} -eq 1 ]]; then
            # Only one side has it: rename the whole subtree. This is what keeps
            # /lib/firmware (984M) and /lib/modules (60M) instantaneous and
            # free-space-neutral.
            if [[ "${keep[0]}" != "$dst" ]]; then
                plan_add MOVE "${keep[0]}" "$dst"
                STAT_MOVES=$((STAT_MOVES+1))
            fi
            expect_set "${keep[0]}" "$(object_id "${keep[0]}")"
            return 0
        fi
        # Same directory name on both sides -> merge its contents recursively.
        if ! is_dir "$dst"; then
            plan_add MKDIR "$dst" "$(stat -c '%a' "$ROOT/${keep[0]}")"
            STAT_MKDIRS=$((STAT_MKDIRS+1))
        fi
        plan_level "$dst" "${keep[@]}"
        return 0
    fi

    # ---- files / symlinks ---------------------------------------------------
    if [[ ${#keep[@]} -eq 1 ]]; then
        if [[ "${keep[0]}" != "$dst" ]]; then
            plan_add MOVE "${keep[0]}" "$dst"
            STAT_MOVES=$((STAT_MOVES+1))
        fi
        expect_set "${keep[0]}" "$(object_id "${keep[0]}")"
        return 0
    fi

    STAT_COLL=$((STAT_COLL+1))

    # A dangling symlink must never win against something real -- that bug ate
    # /usr/bin/login and /usr/sbin/raven-install in an earlier draft.
    local -a live=() dead=()
    for p in "${keep[@]}"; do
        if resolve_in_root "$p" >/dev/null; then live+=("$p"); else dead+=("$p"); fi
    done
    if [[ ${#live[@]} -gt 0 && ${#dead[@]} -gt 0 ]]; then
        for p in "${dead[@]}"; do
            plan_add DROP "$p" "dangling symlink, loses to a real object at /$dst"
            STAT_DROPS=$((STAT_DROPS+1))
        done
        keep=("${live[@]}")
    fi

    # Prefer a real file over a symlink when both are on the table; among equals
    # keep contributor order (which is Raven's PATH order: bin sbin usr/bin usr/sbin).
    local -a reals=() links=()
    for p in "${keep[@]}"; do
        if [[ -L "$ROOT/$p" ]]; then links+=("$p"); else reals+=("$p"); fi
    done
    keep=()
    [[ ${#reals[@]} -gt 0 ]] && keep+=("${reals[@]}")
    [[ ${#links[@]} -gt 0 ]] && keep+=("${links[@]}")

    local winner="${keep[0]}"
    local ok=1
    for p in "${keep[@]:1}"; do
        if same_content "$winner" "$p"; then
            plan_add DROP "$p" "identical to /$winner"
            STAT_DROPS=$((STAT_DROPS+1))
        else
            conflict "$winner" "$p" "$CMP_WHY"
            ok=0
        fi
    done
    [[ $ok -eq 1 ]] || return 0

    local wid; wid="$(object_id "$winner")"
    for p in "${cands[@]}"; do expect_set "$p" "$wid"; done
    if [[ "$winner" != "$dst" ]]; then
        plan_add MOVE "$winner" "$dst"
        STAT_MOVES=$((STAT_MOVES+1))
    fi
}

log_step "Planning"

for target in usr/bin usr/lib; do
    contribs=()
    is_dir "$target" && contribs+=("$target")
    for i in "${PENDING[@]}"; do
        [[ "${SET_DST[$i]}" == "$target" ]] && contribs+=("${SET_SRC[$i]}")
    done
    [[ ${#contribs[@]} -le 1 ]] && continue
    [[ -d "$ROOT/$target" ]] || { log_error "Missing /$target in target rootfs."; exit 1; }
    log_info "  /${target} <- $(printf '/%s ' "${contribs[@]:1}")"
    plan_level "$target" "${contribs[@]}"
done

# =============================================================================
# Conflicts -> stop, and let a human decide
# =============================================================================
if [[ ${#CONF_A[@]} -gt 0 ]]; then
    log_step "Unresolvable collisions"
    log_error "${#CONF_A[@]} name(s) exist on both sides of the split with different"
    log_error "content. This script will not pick one for you -- picking wrong here"
    log_error "silently changes which binary the system runs."
    echo
    for i in "${!CONF_A[@]}"; do
        printf '  /%s\n' "${CONF_A[$i]}"
        [[ "${CONF_B[$i]}" != "-" ]] && printf '  /%s\n' "${CONF_B[$i]}"
        printf '      %s\n' "${CONF_WHY[$i]}"
        if [[ "${CONF_B[$i]}" != "-" ]]; then
            for p in "${CONF_A[$i]}" "${CONF_B[$i]}"; do
                if [[ -L "$ROOT/$p" ]]; then
                    printf '      /%-40s symlink -> %s\n' "$p" "$(readlink "$ROOT/$p")"
                else
                    printf '      /%-40s %s bytes, mode 0%s\n' "$p" \
                        "$(stat -c '%s' "$ROOT/$p" 2>/dev/null || echo '?')" \
                        "$(stat -c '%a' "$ROOT/$p" 2>/dev/null || echo '???')"
                fi
            done
        fi
        echo
    done
    log_error "Nothing was changed. Resolve each of the above by hand -- delete the"
    log_error "copy you do not want, or make them identical -- then re-run."
    log_info  "Raven's PATH is /bin:/sbin:/usr/bin:/usr/sbin:/usr/local/bin, so the"
    log_info  "leftmost of a colliding pair is the one users get today."
    exit 2
fi

# =============================================================================
# Can we actually read everything we are about to move?
# =============================================================================
# A directory `find` cannot descend into contributes nothing to the plan, so
# its contents are never moved -- and the `mv` that does hit it fails mid-apply
# with a bare permission error and no idea of what state the tree is in. Checked
# up front, where the answer is still "nothing has happened yet".
UNREADABLE=()
for i in "${PENDING[@]}"; do
    errs="${TMPDIR:-/tmp}/raven-usrmerge-scan.$$"
    find "$ROOT/${SET_SRC[$i]}" -mindepth 0 >/dev/null 2>"$errs" || true
    if [[ -s "$errs" ]]; then
        while IFS= read -r line; do UNREADABLE+=("$line"); done < "$errs"
    fi
    rm -f "$errs"
done

if [[ ${#UNREADABLE[@]} -gt 0 ]]; then
    log_error "Parts of the tree cannot be read, so the plan would be incomplete:"
    log_error ""
    for u in "${UNREADABLE[@]}"; do
        log_error "  $u"
    done
    log_error ""
    log_error "Nothing was changed. Anything under an unreadable directory would be"
    log_error "left behind by the move and then block the rmdir, half way through."
    log_error ""
    log_error "Run as the owner of the target rootfs, or under sudo."
    exit 1
fi

# =============================================================================
# Relative symlinks that would change meaning
# =============================================================================
# /bin and /lib are one level below the root; /usr/bin and /usr/lib are two.
# A relative target that climbs out of its own directory therefore lands
# somewhere else after the move: /sbin/rc -> ../etc/rc.conf resolves to
# /etc/rc.conf today and to /usr/etc/rc.conf once it sits in /usr/bin.
#
# The post-merge verification catches this for individual files, but only
# after the whole tree has been converted and the sources deleted -- and for a
# subtree moved as a single rename it never sees it at all, because it compares
# the directory's inode, which does not change. So it is checked here, against
# the unmodified tree, where the answer is still "nothing has happened yet".
ESCAPEES=()
for i in "${PENDING[@]}"; do
    src="${SET_SRC[$i]}"
    dst="${SET_DST[$i]}"
    # How much deeper the destination sits. Same depth means nothing shifts.
    src_depth="$(awk -F/ '{print NF}' <<<"$src")"
    dst_depth="$(awk -F/ '{print NF}' <<<"$dst")"
    [[ "$dst_depth" -gt "$src_depth" ]] || continue

    while IFS= read -r link; do
        [[ -n "$link" ]] || continue
        target="$(readlink "$link")"
        # Absolute targets are re-rooted the same way wherever the link lives.
        [[ "$target" == /* ]] && continue
        rel="${link#"$ROOT"/}"

        # Where it points now, and where the same text would point from the
        # destination. `realpath -m` does not need either path to exist.
        before="$(realpath -m "$ROOT/$(dirname "$rel")/$target")"
        after_dir="$dst/$(dirname "${rel#"$src"/}")"
        after="$(realpath -m "$ROOT/$after_dir/$target")"

        [[ "$before" == "$after" ]] && continue
        ESCAPEES+=("/${rel}  ->  ${target}   (${before#"$ROOT"}  becomes  ${after#"$ROOT"})")
    done < <(find "$ROOT/$src" -type l 2>/dev/null)
done

if [[ ${#ESCAPEES[@]} -gt 0 ]]; then
    log_error "These relative symlinks would point somewhere else after the merge:"
    log_error ""
    for e in "${ESCAPEES[@]}"; do
        log_error "  $e"
    done
    log_error ""
    log_error "Nothing was changed. Each one climbs out of its own directory with"
    log_error "'..', and /usr/bin sits one level deeper than /bin, so the same text"
    log_error "resolves elsewhere once the file moves."
    log_error ""
    log_error "Fix each by making the target absolute (ln -sfn /etc/rc.conf ...) or"
    log_error "by re-spelling it for the new depth, then re-run."
    exit 2
fi

# =============================================================================
# Tail of the plan: empty the old directories, then replace them with symlinks
# =============================================================================
# Interleaved, one set at a time: rmdir /bin then immediately symlink /bin,
# then move on to /sbin. Emptying all six first and only then creating the six
# replacements left a window in which the tree had no /bin, /sbin, /lib or
# /lib64 and nothing standing in for them -- it could not exec anything, so any
# abort in that window (a failed rmdir, Ctrl-C, power loss) left it unbootable.
# The window is now one set wide instead of all six, and every set outside the
# one in flight is either untouched or already converted.
for i in "${PENDING[@]}"; do
    plan_add RMTREE "${SET_SRC[$i]}"
    plan_add SYMLINK "${SET_SRC[$i]}" "${SET_REL[$i]}"
done
# Nothing to empty for these; the directory was never there.
for i in "${LINK_ONLY[@]}"; do
    plan_add SYMLINK "${SET_SRC[$i]}" "${SET_REL[$i]}"
done
plan_add LDCONF etc/ld.so.conf

# =============================================================================
# Report
# =============================================================================
log_step "Plan"
if [[ $APPLY -eq 0 || $VERBOSE -eq 1 ]]; then
    for i in "${!PLAN_ACT[@]}"; do
        case "${PLAN_ACT[$i]}" in
            MOVE)    printf '  move     /%-46s -> /%s\n' "${PLAN_A[$i]}" "${PLAN_B[$i]}" ;;
            DROP)    printf '  drop     /%-46s    %s\n'  "${PLAN_A[$i]}" "${PLAN_B[$i]}" ;;
            MKDIR)   printf '  mkdir    /%-46s    mode 0%s\n' "${PLAN_A[$i]}" "${PLAN_B[$i]}" ;;
            RMTREE)  printf '  rmdir    /%-46s    (must be empty by then)\n' "${PLAN_A[$i]}" ;;
            SYMLINK) printf '  symlink  /%-46s -> %s\n' "${PLAN_A[$i]}" "${PLAN_B[$i]}" ;;
            LDCONF)  printf '  rewrite  /%-46s    merged library paths\n' "${PLAN_A[$i]}" ;;
        esac
    done
    echo
fi

log_info "  moves ............... ${STAT_MOVES}"
log_info "  drops ............... ${STAT_DROPS}  (${STAT_SELFREF} would have become symlink loops)"
log_info "  merged directories .. ${STAT_MKDIRS}"
log_info "  name collisions ..... ${STAT_COLL}  (all resolved: identical content and mode)"
log_info "  names to verify ..... ${#EXPECT_NAMES[@]}"

if [[ $APPLY -eq 0 ]]; then
    echo
    log_success "Dry run only -- nothing was changed."
    log_info "Re-run with --apply to perform the conversion."
    exit 0
fi

# =============================================================================
# Apply
# =============================================================================
log_step "Applying"

for i in "${!PLAN_ACT[@]}"; do
    a="${PLAN_A[$i]}"; b="${PLAN_B[$i]}"
    case "${PLAN_ACT[$i]}" in
        MKDIR)
            mkdir -m "0$b" -p "$ROOT/$a"
            ;;
        DROP)
            rm -f "$ROOT/$a"
            ;;
        MOVE)
            [[ -e "$ROOT/$b" || -L "$ROOT/$b" ]] && {
                log_error "internal: destination /$b already exists while moving /$a"
                exit 3
            }
            if ! mv -n -- "$ROOT/$a" "$ROOT/$b"; then
                log_error ""
                log_error "Could not move /$a to /$b."
                log_error ""
                if [[ ${#DONE_SETS[@]} -gt 0 ]]; then
                    log_error "  Already converted: ${DONE_SETS[*]}"
                    log_error "  Those resolve through their symlinks and still work."
                fi
                log_error "  /$a is still where it was; nothing has been deleted."
                log_error "  Resolve the cause and re-run -- converted sets are skipped."
                exit 3
            fi
            ;;
        RMTREE)
            find "$ROOT/$a" -mindepth 1 -depth -type d -exec rmdir {} + 2>/dev/null || true
            leftover="$(find "$ROOT/$a" -mindepth 1 -print -quit 2>/dev/null || true)"
            if [[ -n "$leftover" ]]; then
                log_error "/$a is not empty after the merge (e.g. ${leftover#$ROOT/})."
                log_error ""
                log_error "  /$a has been left exactly as it is -- a real directory,"
                log_error "  with its remaining contents. Nothing is deleted here."
                if [[ ${#DONE_SETS[@]} -gt 0 ]]; then
                    log_error ""
                    log_error "  These sets were already converted before this one:"
                    log_error "    ${DONE_SETS[*]}"
                    log_error "  The tree is part-merged. It is still bootable -- every"
                    log_error "  converted set resolves through its symlink -- but do not"
                    log_error "  install packages into it until the merge is finished."
                fi
                log_error ""
                log_error "  Move or delete what is left in /$a, then re-run: the"
                log_error "  already-converted sets are detected and skipped."
                exit 3
            fi
            rmdir "$ROOT/$a"
            ;;
        SYMLINK)
            ln -s "$b" "$ROOT/$a"
            DONE_SETS+=("/$a")
            ;;
        LDCONF)
            if [[ -f "$ROOT/$a" ]]; then
                cp -a "$ROOT/$a" "$ROOT/$a.pre-usrmerge"
                {
                    echo "# /etc/ld.so.conf - dynamic linker configuration"
                    echo "# usr-merged: /lib, /lib64 and /usr/lib64 are symlinks into /usr/lib"
                    echo "/usr/lib"
                    echo "/usr/local/lib"
                    echo "include /etc/ld.so.conf.d/*.conf"
                } > "$ROOT/$a"
            fi
            ;;
    esac
done

# =============================================================================
# Verify -- every name the old tree could resolve must still name the same object
# =============================================================================
log_step "Verifying"

FAILED=0
for name in "${EXPECT_NAMES[@]}"; do
    want="${EXPECT_KEY[$name]}"
    got="$(object_id "$name")"
    [[ "$got" == "$want" ]] && continue

    # Cross-device moves are copies, so the inode legitimately changes. Fall
    # back to comparing what actually matters.
    if [[ "$want" == I:* && "$got" == I:* && $ALLOW_CROSSDEV -eq 1 ]]; then
        r="$(resolve_in_root "$name" || true)"
        if [[ -n "$r" ]]; then
            [[ $VERBOSE -eq 1 ]] && log_info "  /$name moved across devices, content re-checked"
            continue
        fi
    fi
    # A name that could not resolve before and can now is a repair, not a
    # regression: on a split-usr root /usr/bin/awk -> gawk is routinely
    # dangling because the real gawk sits in /bin, and merging fixes it.
    # Failing on that trains the operator to ignore the one check that matters.
    if [[ "$want" == D* && "$got" == I:* ]]; then
        [[ $VERBOSE -eq 1 ]] && log_info "  /$name was dangling before and now resolves"
        continue
    fi

    log_error "  /$name no longer resolves to what it used to (want $want, got $got)"
    FAILED=$((FAILED+1))
done

if [[ $FAILED -gt 0 ]]; then
    log_error "${FAILED} of ${#EXPECT_NAMES[@]} names failed verification."
    log_error "The tree is in a merged state but is NOT trustworthy. Do not boot it."
    exit 4
fi

log_success "All ${#EXPECT_NAMES[@]} names still resolve to the same objects."
log_info "  A subtree that existed on only one side (/lib/firmware, /lib/modules,"
log_info "  /usr/lib64/dri ...) was moved with a single rename and is verified as a"
log_info "  unit -- rename(2) cannot alter its contents, and walking ~1G of firmware"
log_info "  file by file on every run would not tell you anything new."

# A few things worth checking explicitly because losing them means no boot.
log_step "Boot-critical spot checks"
CRIT=0
for p in bin/sh bin/bash usr/bin/bash lib64/ld-linux-x86-64.so.2 usr/lib/ld-linux-x86-64.so.2; do
    if r="$(resolve_in_root "$p")"; then
        [[ $VERBOSE -eq 1 ]] && log_info "  /$p -> /$r"
    else
        [[ -e "$ROOT/$p" || -L "$ROOT/$p" ]] || continue
        log_error "  /$p does not resolve"
        CRIT=$((CRIT+1))
    fi
done
[[ $CRIT -eq 0 ]] || { log_error "Boot-critical paths are broken."; exit 4; }

log_success "usr-merge complete: ${ROOT}"
log_info "  /bin /sbin -> usr/bin      /lib /lib64 -> usr/lib"
log_info "  /usr/sbin -> bin           /usr/lib64 -> lib"
log_info "  /etc/ld.so.conf rewritten (previous version kept as ld.so.conf.pre-usrmerge)"
log_info ""
log_info "If the target has its own initramfs or bootloader entries referring to"
log_info "/sbin or /lib paths, they keep working -- those are symlinks now, not gone."
