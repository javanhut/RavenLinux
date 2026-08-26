#!/bin/bash
# =============================================================================
# RavenLinux Build System - usr-merge layout library
# =============================================================================
# Source this file and call raven_usrmerge_root <root> anywhere the build
# creates a rootfs skeleton.
#
#   source "${PROJECT_ROOT}/scripts/lib/usrmerge.sh"
#   raven_usrmerge_root "${SYSROOT_DIR}"
#
# -----------------------------------------------------------------------------
# WHY THIS EXISTS
# -----------------------------------------------------------------------------
# RavenLinux consumes Arch packages through rvn. Arch's `filesystem` package
# ships /bin, /sbin, /lib and /lib64 as SYMLINKS into /usr, and it ships them as
# payload entries -- not as directories the extractor is free to skip. On a
# split-usr root those four names already exist as real directories, so
# extracting `filesystem` tries to create a symlink over an existing directory
# and the kernel returns EEXIST. The concrete symptom was:
#
#     rvn install openssh
#     ... filesystem: File exists (os error 17)
#
# and because `filesystem` is a dependency of essentially everything in the Arch
# repos, a split-usr root makes EVERY Arch package uninstallable, not just
# openssh. There is no way to paper over this from the package manager side: the
# root has to be usr-merged before Arch packages can land on it at all.
#
# So the sysroot follows Arch's layout exactly:
#
#     /bin       -> usr/bin
#     /sbin      -> usr/bin
#     /lib       -> usr/lib
#     /lib64     -> usr/lib
#     /usr/sbin  -> bin
#     /usr/lib64 -> lib
#
# Everything real lives in /usr/bin and /usr/lib. The six compat symlinks are
# not optional decoration -- they are load-bearing:
#
#   * 217 binaries in the sysroot carry PT_INTERP /lib64/ld-linux-x86-64.so.2.
#     Delete /lib64 instead of symlinking it and none of them execute.
#   * 19 scripts, /init among them, carry a #!/bin/sh or #!/bin/bash shebang.
#   * /etc/shells, /etc/passwd and init.toml name /bin and /sbin paths.
#   * build-initramfs.sh reads the sysroot at split paths (sysroot/bin/bash,
#     sysroot/lib64/ld-linux-x86-64.so.2) and resolves them through these links.
#
# -----------------------------------------------------------------------------
# THE RULE FOR CALLERS
# -----------------------------------------------------------------------------
# Install real files into /usr/bin and /usr/lib. NEVER create a symlink whose
# link path and target collapse to the same file after the merge:
#
#   ln -sf ../usr/bin/foo  "${SYSROOT}/bin/foo"    # WRONG: unlinks the binary,
#                                                  # leaves bin/foo dangling at
#                                                  # /usr/usr/bin/foo
#   ln -sf ../../bin/foo   "${SYSROOT}/usr/bin/foo"# WRONG: symlink loop (ELOOP)
#   ln -sf /usr/bin/foo    "${SYSROOT}/bin/foo"    # WRONG: self-referential; at
#                                                  # build time it escapes the
#                                                  # sysroot and resolves against
#                                                  # the BUILD HOST
#
# All three exit 0. An unfixed merge therefore produces an ISO that builds clean
# and boots with no shell.
#
# Same-directory alias links stay correct and are still wanted, e.g.
#   ln -sf coreutils   "${SYSROOT}/usr/bin/ls"
#   ln -sf raven-init  "${SYSROOT}/usr/bin/init"
#
# -----------------------------------------------------------------------------
# THE INITRAMFS IS DELIBERATELY NOT MERGED
# -----------------------------------------------------------------------------
# build-initramfs.sh, build-minimal.sh and stage1-base.sh's initramfs skeleton
# keep their split /bin + /usr/bin layout on purpose. The initramfs is a
# separate root that is switch_root'ed away from before rvn ever runs, so no
# Arch package is ever extracted into it and the EEXIST problem cannot arise
# there. Its links are all same-directory (`ln -sf coreutils bin/X`), so it is
# not exposed to the failure modes above either. Do not "fix" it.
# =============================================================================

# Idempotent source guard: build.sh sources the stage scripts into one shell.
if [[ -n "${RAVEN_USRMERGE_SH_LOADED:-}" ]]; then
    return 0 2>/dev/null || true
fi
RAVEN_USRMERGE_SH_LOADED=1

# Logging fallbacks, so this library works even if lib/logging.sh was not sourced.
if ! declare -F log_info >/dev/null 2>&1; then
    log_info()    { echo "[INFO] $*"; }
fi
if ! declare -F log_warn >/dev/null 2>&1; then
    log_warn()    { echo "[WARN] $*" >&2; }
fi
if ! declare -F log_error >/dev/null 2>&1; then
    log_error()   { echo "[ERROR] $*" >&2; }
fi
if ! declare -F log_success >/dev/null 2>&1; then
    log_success() { echo "[OK] $*"; }
fi

# The merge table, one "linkpath:target" per entry. The target is relative to
# the DIRECTORY THE LINK LIVES IN, which is why /bin -> usr/bin has no "../"
# and /usr/sbin -> bin has no "usr/".
RAVEN_USRMERGE_LINKS=(
    "bin:usr/bin"
    "sbin:usr/bin"
    "lib:usr/lib"
    "lib64:usr/lib"
    "usr/sbin:bin"
    "usr/lib64:lib"
)

# Directories that must be real before any link is made.
RAVEN_USRMERGE_REALDIRS=(
    usr/bin
    usr/lib
    usr/include
    usr/share
    usr/libexec
    usr/local/bin
    usr/local/lib
    usr/local/share
    usr/src
)

# ---------------------------------------------------------------------------
# _raven_is_elf <file> -- true for an ELF object, false for a script/anything
# ---------------------------------------------------------------------------
_raven_is_elf() {
    local f="$1" magic
    [[ -f "$f" ]] || return 1
    magic="$(head -c 4 "$f" 2>/dev/null | od -An -tx1 2>/dev/null | tr -d ' \n')"
    [[ "${magic}" == "7f454c46" ]]
}

# ---------------------------------------------------------------------------
# _raven_normalise <path> -- collapse "." and ".." textually. The path need not
# exist, so this cannot use realpath/pwd -P.
# ---------------------------------------------------------------------------
_raven_normalise() {
    local -a parts=() out=()
    IFS='/' read -r -a parts <<< "$1"
    local p
    for p in "${parts[@]}"; do
        case "$p" in
            ''|'.') continue ;;
            '..')   [[ ${#out[@]} -gt 0 ]] && unset 'out[-1]' ;;
            *)      out+=("$p") ;;
        esac
    done
    printf '/%s\n' "$(IFS=/; echo "${out[*]}")"
}

# ---------------------------------------------------------------------------
# _raven_canon_merged <root> <abs-host-path>
#   Map a path through the merge table, so /bin/x, /sbin/x and /usr/sbin/x all
#   report as /usr/bin/x, and /lib/x, /lib64/x, /usr/lib64/x as /usr/lib/x.
#   Everything below reasons about the merged names only, so that "is this link
#   self-referential?" has one answer instead of six.
# ---------------------------------------------------------------------------
_raven_canon_merged() {
    local root="$1" p="${2#$1}"
    case "$p" in
        /bin|/sbin)          p="/usr/bin" ;;
        /usr/sbin)           p="/usr/bin" ;;
        /lib|/lib64)         p="/usr/lib" ;;
        /usr/lib64)          p="/usr/lib" ;;
        /bin/*)              p="/usr/bin/${p#/bin/}" ;;
        /sbin/*)             p="/usr/bin/${p#/sbin/}" ;;
        /usr/sbin/*)         p="/usr/bin/${p#/usr/sbin/}" ;;
        /lib/*)              p="/usr/lib/${p#/lib/}" ;;
        /lib64/*)            p="/usr/lib/${p#/lib64/}" ;;
        /usr/lib64/*)        p="/usr/lib/${p#/usr/lib64/}" ;;
    esac
    printf '%s\n' "${root}${p}"
}

# ---------------------------------------------------------------------------
# _raven_link_target <root> <symlink>
#   One hop, resolved the way the BOOTED system would resolve it: an absolute
#   target is taken against <root>, not against the build host's /.
#
#   Using `readlink -f` here instead is the classic mistake -- it resolves
#   /sbin/login against the HOST root, so a link that is a self-referential
#   loop inside the sysroot looks perfectly healthy from outside it.
# ---------------------------------------------------------------------------
_raven_link_target() {
    local root="$1" link="$2" target base
    target="$(readlink "$link" 2>/dev/null)" || return 1
    [[ -n "$target" ]] || return 1
    if [[ "$target" == /* ]]; then
        base="${root}${target}"
    else
        # Resolve a relative target against the link's MERGED directory. A
        # purely textual dirname is wrong here: ".." out of /bin is /, but /bin
        # is a symlink into /usr, so the kernel lands in /usr. Canonicalising
        # first is what makes /usr/bin/ghost -> ../../bin/ghost come out as
        # /usr/bin/ghost, i.e. visibly self-referential.
        base="$(_raven_canon_merged "$root" "$(dirname "$link")")/${target}"
    fi
    _raven_canon_merged "$root" "$(_raven_normalise "$base")"
}

# ---------------------------------------------------------------------------
# _raven_link_points_at <root> <symlink> <candidate>
#   True when <symlink> resolves (root-aware, one hop) to <candidate>. This is
#   what identifies the back-pointing links (/usr/bin/bash -> ../../bin/bash)
#   that would become ELOOP after the merge.
# ---------------------------------------------------------------------------
_raven_link_points_at() {
    local root="$1" link="$2" candidate="$3" resolved
    resolved="$(_raven_link_target "$root" "$link")" || return 1
    # Both sides go through the merge table: /sbin/login and /usr/bin/login are
    # the same file post-merge, so comparing the raw paths would miss it.
    [[ "$resolved" == "$(_raven_canon_merged "$root" "$candidate")" ]]
}

# ---------------------------------------------------------------------------
# _raven_link_loops <root> <symlink>
#   True when following <symlink> root-aware revisits a path, i.e. the link is
#   an ELOOP on the booted system. Bounded walk; a chain that leaves the link
#   set or dead-ends is not a loop.
# ---------------------------------------------------------------------------
_raven_link_loops() {
    local root="$1" cur="$2" seen=" $2 " next i
    for ((i = 0; i < 16; i++)); do
        [[ -L "$cur" ]] || return 1
        next="$(_raven_link_target "$root" "$cur")" || return 1
        case "$seen" in
            *" ${next} "*) return 0 ;;
        esac
        seen+="${next} "
        cur="$next"
    done
    return 0
}

# ---------------------------------------------------------------------------
# _raven_merge_entry <root> <src> <dest>
#   Move one entry from a legacy directory into its merged home, resolving the
#   collision if <dest> already exists.
# ---------------------------------------------------------------------------
_raven_merge_entry() {
    local root="$1" src="$2" dest="$3"

    # ---- <src> is itself a symlink -------------------------------------------
    # Never cmp/file-type-test a symlink: an absolute target escapes the sysroot
    # and the test reads the BUILD HOST's copy instead. Decide on the link.
    if [[ -L "$src" ]]; then
        if [[ -e "$dest" || -L "$dest" ]]; then
            # Something is already at the merged path. A legacy alias link never
            # outranks it -- most of these are the forward /bin/X -> ../usr/bin/X
            # compat links, which the merge makes redundant by construction.
            rm -f "$src"
            return 0
        fi
        # Move it, then make sure it did not become self-referential at its new
        # home (e.g. /usr/sbin/login -> /sbin/login, where /sbin is now usr/bin).
        mv -f "$src" "$dest"
        if _raven_link_loops "$root" "$dest"; then
            log_warn "  usr-merge: dropping ${dest#$root} -> $(readlink "$dest"); it is a symlink loop after the merge"
            rm -f "$dest"
        fi
        return 0
    fi

    # Nothing there: straight move.
    if [[ ! -e "$dest" && ! -L "$dest" ]]; then
        mv -f "$src" "$dest"
        return 0
    fi

    # Two directories: merge recursively.
    if [[ -d "$src" && ! -L "$src" && -d "$dest" && ! -L "$dest" ]]; then
        local child
        for child in "$src"/* "$src"/.[!.]* "$src"/..?*; do
            [[ -e "$child" || -L "$child" ]] || continue
            _raven_merge_entry "$root" "$child" "${dest}/$(basename "$child")"
        done
        rmdir "$src" 2>/dev/null || true
        return 0
    fi

    # <dest> is a symlink that points back at <src>. After the merge that is a
    # self-referential loop, so the real file must take its place. This is the
    # /usr/bin/bash -> ../../bin/bash case; getting it wrong bricks the boot.
    if [[ -L "$dest" ]] && _raven_link_points_at "$root" "$dest" "$src"; then
        rm -f "$dest"
        mv -f "$src" "$dest"
        log_info "  usr-merge: ${dest#$root} taken from the legacy side (was a self-referential link)"
        return 0
    fi

    # <dest> is a dangling symlink: the real file wins.
    if [[ -L "$dest" && ! -e "$dest" ]]; then
        rm -f "$dest"
        mv -f "$src" "$dest"
        return 0
    fi

    # Byte-identical duplicates: drop the legacy copy. This is the overwhelming
    # majority of collisions (the audit found 145 such pairs, ~24.7 MiB).
    if [[ -f "$src" && -f "$dest" && ! -L "$src" && ! -L "$dest" ]] \
        && cmp -s "$src" "$dest"; then
        rm -f "$src"
        return 0
    fi

    # A real binary beats a hand-rolled shell shim. The known instance is
    # whoami: /sbin/whoami is the real coreutils ELF, /bin/whoami is an 829-byte
    # POSIX-sh fallback from early bring-up. Note this IS a behaviour change --
    # the shim never fails, the real binary errors out when the uid has no
    # passwd entry -- so it is logged rather than done silently.
    if _raven_is_elf "$src" && [[ -f "$dest" ]] && ! _raven_is_elf "$dest"; then
        log_warn "  usr-merge: ${dest#$root} replaced by the real binary from ${src#$root} (was a script shim)"
        rm -f "$dest"
        mv -f "$src" "$dest"
        return 0
    fi
    if _raven_is_elf "$dest" && [[ -f "$src" ]] && ! _raven_is_elf "$src"; then
        log_warn "  usr-merge: keeping the real binary at ${dest#$root}, dropping the script shim ${src#$root}"
        rm -f "$src"
        return 0
    fi

    # Genuine divergence. Keep what is already on the /usr side and say so
    # loudly rather than silently picking a winner.
    log_warn "  usr-merge CONFLICT: ${src#$root} and ${dest#$root} differ; keeping ${dest#$root}"
    rm -rf "$src"
    return 0
}

# ---------------------------------------------------------------------------
# _raven_usrmerge_one <root> <linkpath-rel> <target-rel>
# ---------------------------------------------------------------------------
_raven_usrmerge_one() {
    local root="$1" rel="$2" target="$3"
    local link="${root}/${rel}"
    # Resolve the target relative to the link's own directory, textually --
    # nothing here may exist yet, so no cd/pwd -P.
    local realdir="$(dirname "$link")/${target}"
    mkdir -p "$realdir"

    if [[ -L "$link" ]]; then
        if [[ "$(readlink "$link")" == "$target" ]]; then
            return 0
        fi
        log_info "  usr-merge: repointing /${rel} -> ${target}"
        rm -f "$link"
    elif [[ -d "$link" ]]; then
        log_info "  usr-merge: migrating /${rel} into $(_raven_normalise "/$(dirname "/${rel}")/${target}")"
        local child
        shopt -s nullglob dotglob
        local -a children=("$link"/*)
        shopt -u nullglob dotglob
        for child in "${children[@]}"; do
            [[ "$(basename "$child")" == "." || "$(basename "$child")" == ".." ]] && continue
            _raven_merge_entry "$root" "$child" "${realdir}/$(basename "$child")"
        done
        if ! rmdir "$link" 2>/dev/null; then
            log_error "usr-merge: /${rel} is not empty after migration; refusing to replace it with a symlink"
            return 1
        fi
    elif [[ -e "$link" ]]; then
        log_error "usr-merge: /${rel} exists and is not a directory or symlink; cannot merge"
        return 1
    fi

    ln -sfn "$target" "$link"
}

# ---------------------------------------------------------------------------
# raven_usrmerge_root <root>
#   Create (or repair) the usr-merged skeleton at <root>. Idempotent.
# ---------------------------------------------------------------------------
raven_usrmerge_root() {
    local root="${1:?raven_usrmerge_root: root required}"
    root="${root%/}"

    mkdir -p "$root"
    local d
    for d in "${RAVEN_USRMERGE_REALDIRS[@]}"; do
        mkdir -p "${root}/${d}"
    done

    local entry rel target rc=0
    for entry in "${RAVEN_USRMERGE_LINKS[@]}"; do
        rel="${entry%%:*}"
        target="${entry#*:}"
        _raven_usrmerge_one "$root" "$rel" "$target" || rc=1
    done

    # Normalise first, so the sweep judges the final link shapes.
    raven_usrmerge_normalise_links "$root" || rc=1
    raven_usrmerge_sweep_loops "$root" || rc=1

    return $rc
}

# ---------------------------------------------------------------------------
# raven_usrmerge_normalise_links <root>
#   Rewrite any symlink under the merged directories whose target reaches its
#   OWN directory by going out through a compat symlink -- e.g.
#   /usr/bin/sh -> ../../bin/bash becomes /usr/bin/sh -> bash.
#
#   Semantically identical (both name the same inode), but the same-directory
#   form cannot turn into a loop later if something replaces the file the chain
#   passes through. Leaving the ../../bin/ form is how a link that is merely
#   ugly today becomes ELOOP after the next stage runs.
# ---------------------------------------------------------------------------
raven_usrmerge_normalise_links() {
    local root="${1%/}" link tgt canon d n=0
    for d in usr/bin usr/lib; do
        [[ -d "${root}/${d}" ]] || continue
        while IFS= read -r -d '' link; do
            tgt="$(_raven_link_target "$root" "$link")" || continue
            canon="$(_raven_canon_merged "$root" "$tgt")"
            [[ "$(dirname "$canon")" == "$(dirname "$link")" ]] || continue
            # basename equal means the link points at itself -- that is a loop,
            # not something to normalise. Leave it for the sweep.
            [[ "$(basename "$canon")" == "$(basename "$link")" ]] && continue
            [[ "$(readlink "$link")" == "$(basename "$canon")" ]] && continue
            ln -sfn "$(basename "$canon")" "$link"
            n=$((n + 1))
        done < <(find "${root}/${d}/" -type l -print0 2>/dev/null)
    done
    [[ $n -eq 0 ]] || log_info "  usr-merge: normalised ${n} symlink(s) to same-directory targets"
    return 0
}

# ---------------------------------------------------------------------------
# raven_usrmerge_sweep_loops <root>
#   Remove every symlink under the merged directories that became a loop.
#
#   A pre-existing link like /usr/bin/bash -> ../../bin/bash was perfectly
#   valid under split-usr and is a self-reference the moment /bin becomes a
#   symlink. /usr/bin/sh and /usr/bin/bash going ELOOP is a hard boot failure:
#   /init and 18 other scripts have a #!/bin/sh or #!/bin/bash shebang.
#
#   These are invisible to `find -follow` when the target is absolute, because
#   the host kernel resolves /sbin/login against the HOST root where no loop
#   exists. The resolution here is root-aware, which is why it finds them.
# ---------------------------------------------------------------------------
raven_usrmerge_sweep_loops() {
    local root="${1%/}" link n=0
    local d
    for d in usr/bin usr/lib; do
        [[ -d "${root}/${d}" ]] || continue
        while IFS= read -r -d '' link; do
            if _raven_link_loops "$root" "$link"; then
                log_warn "  usr-merge: removing symlink loop ${link#$root} -> $(readlink "$link")"
                rm -f "$link"
                n=$((n + 1))
            fi
        done < <(find "${root}/${d}/" -type l -print0 2>/dev/null)
    done
    [[ $n -eq 0 ]] || log_warn "  usr-merge: removed ${n} symlink loop(s); the real files must be reinstalled by the stage that owns them"
    return 0
}

# ---------------------------------------------------------------------------
# raven_usrmerge_verify <root>
#   Prove the six links exist, point where they should, resolve to a directory,
#   and contain no loop. Returns non-zero on the first failure.
# ---------------------------------------------------------------------------
raven_usrmerge_verify() {
    local root="${1:?raven_usrmerge_verify: root required}"
    root="${root%/}"
    local entry rel target link got rc=0

    for entry in "${RAVEN_USRMERGE_LINKS[@]}"; do
        rel="${entry%%:*}"
        target="${entry#*:}"
        link="${root}/${rel}"
        if [[ ! -L "$link" ]]; then
            log_error "usr-merge: /${rel} is not a symlink"
            rc=1
            continue
        fi
        got="$(readlink "$link")"
        if [[ "$got" != "$target" ]]; then
            log_error "usr-merge: /${rel} -> ${got} (expected ${target})"
            rc=1
            continue
        fi
        if [[ ! -d "$link/" ]]; then
            log_error "usr-merge: /${rel} does not resolve to a directory (loop or missing target)"
            rc=1
        fi
    done

    # The ELF interpreter path 217 binaries depend on.
    if [[ ! -d "${root}/lib64/" ]]; then
        log_error "usr-merge: /lib64 does not resolve; glibc binaries will not exec"
        rc=1
    fi

    # Any symlink loop anywhere under the merged directories, resolved the way
    # the BOOTED system resolves them. `find -follow` is not sufficient here:
    # an absolute target such as /sbin/login is resolved by the host kernel
    # against the host root, where the loop does not exist.
    local link n=0 d
    for d in usr/bin usr/lib; do
        [[ -d "${root}/${d}" ]] || continue
        while IFS= read -r -d '' link; do
            if _raven_link_loops "$root" "$link"; then
                log_error "usr-merge: symlink loop ${link#$root} -> $(readlink "$link")"
                n=$((n + 1))
            fi
        done < <(find "${root}/${d}/" -type l -print0 2>/dev/null)
    done
    [[ $n -eq 0 ]] || rc=1

    return $rc
}
