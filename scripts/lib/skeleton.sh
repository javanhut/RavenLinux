#!/bin/bash
# =============================================================================
# RavenLinux Build System - rootfs skeleton library
# =============================================================================
# Source this file and call raven_skeleton_root <root> after the usr-merge:
#
#   source "${PROJECT_ROOT}/scripts/lib/usrmerge.sh"
#   source "${PROJECT_ROOT}/scripts/lib/skeleton.sh"
#   raven_usrmerge_root "${SYSROOT_DIR}"     # merge shape
#   raven_skeleton_root "${SYSROOT_DIR}"     # everything else
#
# -----------------------------------------------------------------------------
# WHY THIS EXISTS
# -----------------------------------------------------------------------------
# usrmerge.sh answers "are /bin /sbin /lib /lib64 shaped the way Arch expects?"
# That is necessary and not sufficient. A rootfs can be perfectly merged and
# still be missing two thirds of the directories a Linux system needs, because
# an ABSENT path conflicts with nothing -- check-layout.sh passes on it, `rvn`
# installs over it happily, and the failure shows up at runtime instead:
#
#   * /var/empty missing            -> sshd refuses to start ("must be owned by
#                                      root and not group or world-writable")
#   * /etc/mtab missing             -> df and umount fall back to guessing
#   * /etc/raven/init.d missing     -> the documented `cp <template>
#                                      /etc/raven/init.d/` workflow in
#                                      control.rs copies into nowhere
#   * /root at 0755 instead of 0750 -> root's shell history and dotfiles are
#                                      readable by every user on the system
#   * /var/spool/mail without the sticky bit -> users delete each other's mail
#
# The skeleton also has to survive the stages that run AFTER it. stage4's
# cleanup_sysroot deletes /usr/share/man and /usr/include outright to shrink
# the squashfs, and both are shipped by Arch's `filesystem` package. That is
# why raven_skeleton_root is called a second time in stage4, after cleanup and
# before the squashfs is sealed: the last writer wins, and the last writer
# should be this table.
#
# -----------------------------------------------------------------------------
# WHERE THE NUMBERS COME FROM
# -----------------------------------------------------------------------------
# Modes and group ownership are Arch's, read from the mtree of the installed
# `filesystem` package (/var/lib/pacman/local/filesystem-*/mtree, which records
# `mode=` and `gid=` per entry) rather than copied from a wiki page. Arch is
# the binding authority here and not merely a reference: `rvn` extracts Arch
# packages onto this tree, so a path whose mode or gid differs from Arch's is
# a path where an installed package and the base image disagree about who may
# write. FHS 3.0 and the UAPI file-hierarchy fill in what Arch leaves out
# (/media, /etc/opt), and are noted per entry below where they are the only
# source.
#
# -----------------------------------------------------------------------------
# THE RULE FOR CALLERS
# -----------------------------------------------------------------------------
# Everything here is idempotent and additive. It creates what is absent, fixes
# a mode that has drifted, and never deletes a file. It deliberately does NOT
# touch the six usr-merge links -- usrmerge.sh owns those, and a second writer
# with its own opinion about /bin is exactly how the merge gets half-applied.
# =============================================================================

# Idempotent source guard: build.sh sources the stage scripts into one shell.
if [[ -n "${RAVEN_SKELETON_SH_LOADED:-}" ]]; then
    return 0 2>/dev/null || true
fi
RAVEN_SKELETON_SH_LOADED=1

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

# =============================================================================
# Directories: "path:mode" or "path:mode:gid"
# =============================================================================
# Paths are relative to the root, so no leading slash. A third field is the
# NUMERIC gid the directory must carry; numeric because the build host's
# /etc/group is not the target's, and `chgrp ftp` inside a container either
# resolves against the wrong database or fails outright.

# --- Arch `filesystem`: top level -------------------------------------------
RAVEN_SKELETON_DIRS_TOPLEVEL=(
    "boot:755"
    "dev:755"                 # mount point for devtmpfs
    "etc:755"
    "home:755"
    "mnt:755"
    "opt:755"
    "proc:555"                # 0555: a failed procfs mount must not be
    "sys:555"                 # papered over by files landing on the real root
    "root:750"                # NOT 755 -- see the header
    "run:755"
    "srv:755"
    "srv/ftp:555:11"          # anonymous FTP root, group ftp, unwritable
    "srv/http:755"
    "tmp:1777"
    "usr:755"
    "var:755"
    # FHS 3.0 requires /media; Arch omits it because removable media now mounts
    # under /run/media/$USER. Empty, conflicts with nothing, closes the gap.
    "media:755"
)

# --- Arch `filesystem`: /usr ------------------------------------------------
RAVEN_SKELETON_DIRS_USR=(
    "usr/bin:755"
    "usr/include:755"         # deleted by stage4 cleanup_sysroot; restored here
    "usr/lib:755"
    "usr/libexec:755"
    "usr/share:755"
    "usr/share/misc:755"
    "usr/src:755"
    "usr/local:755"
    "usr/local/bin:755"
    "usr/local/etc:755"
    "usr/local/games:755"
    "usr/local/include:755"
    "usr/local/lib:755"
    "usr/local/man:755"
    "usr/local/sbin:755"
    "usr/local/share:755"
    "usr/local/src:755"
    # All eight sections pre-created, as `filesystem` ships them. Also deleted
    # by cleanup_sysroot, which is why this runs again in stage4.
    "usr/share/man:755"
    "usr/share/man/man1:755"
    "usr/share/man/man2:755"
    "usr/share/man/man3:755"
    "usr/share/man/man4:755"
    "usr/share/man/man5:755"
    "usr/share/man/man6:755"
    "usr/share/man/man7:755"
    "usr/share/man/man8:755"
    # Pristine vendor copies of the shipped /etc files, so a clobbered config
    # has a restore path that does not require reinstalling the package.
    "usr/share/factory:755"
    "usr/share/factory/etc:755"
)

# --- Arch `filesystem`: /var ------------------------------------------------
RAVEN_SKELETON_DIRS_VAR=(
    "var/cache:755"
    "var/empty:755"           # sshd privilege-separation chroot; must be empty
    "var/games:775:50"        # group games
    "var/lib:755"
    "var/lib/misc:755"
    "var/local:755"
    "var/log:755"
    "var/log/old:755"
    "var/opt:755"
    "var/spool:755"
    "var/spool/mail:1777"     # target of /var/mail; sticky, same as /tmp
    "var/tmp:1777"
)

# --- systemd / freedesktop drop-in directories ------------------------------
# Raven runs raven-init, not systemd. These exist anyway because the PACKAGES
# are Arch's: a package installed with `rvn` ships its units, tmpfiles, sysusers
# and udev rules into these paths regardless of what PID 1 is. Pre-creating them
# means those fragments land in a directory with a known mode, and means a tool
# that enumerates a drop-in directory gets an empty list instead of ENOENT --
# which is the difference between "no rules configured" and a hard error.
#
# The vendor side (/usr/lib) and the admin side (/etc) are both listed, because
# the whole point of the split is that the admin can override a vendor fragment
# by dropping a same-named file into the /etc copy.
RAVEN_SKELETON_DIRS_SYSTEMD=(
    # unit trees
    "usr/lib/systemd:755"
    "usr/lib/systemd/system:755"
    "usr/lib/systemd/user:755"
    "usr/lib/systemd/system-preset:755"
    "usr/lib/systemd/user-preset:755"
    "usr/lib/systemd/system-generators:755"
    "usr/lib/systemd/user-generators:755"
    "usr/lib/systemd/system-environment-generators:755"
    "usr/lib/systemd/user-environment-generators:755"
    "usr/lib/systemd/system-sleep:755"
    "usr/lib/systemd/system-shutdown:755"
    "usr/lib/systemd/network:755"
    "etc/systemd:755"
    "etc/systemd/system:755"
    "etc/systemd/user:755"
    "etc/systemd/network:755"
    # declarative configuration, vendor + admin
    "usr/lib/tmpfiles.d:755"
    "etc/tmpfiles.d:755"
    "usr/lib/sysusers.d:755"
    "etc/sysusers.d:755"
    "usr/lib/sysctl.d:755"
    "etc/sysctl.d:755"
    "usr/lib/modules-load.d:755"
    "etc/modules-load.d:755"
    "usr/lib/binfmt.d:755"
    "etc/binfmt.d:755"
    "usr/lib/environment.d:755"
    "etc/environment.d:755"
    "usr/lib/modprobe.d:755"
    "etc/modprobe.d:755"
    "usr/lib/depmod.d:755"
    "etc/depmod.d:755"
    "usr/lib/ld.so.conf.d:755"
    "etc/ld.so.conf.d:755"
    # udev: rules and the hardware database
    "usr/lib/udev:755"
    "usr/lib/udev/rules.d:755"
    "usr/lib/udev/hwdb.d:755"
    "etc/udev:755"
    "etc/udev/rules.d:755"
    "etc/udev/hwdb.d:755"
    # kernel install hooks (mkinitcpio, bootloader entry generation)
    "usr/lib/kernel:755"
    "usr/lib/kernel/install.d:755"
    "etc/kernel:755"
    "etc/kernel/install.d:755"
    # kernel payload
    "usr/lib/modules:755"
    "usr/lib/firmware:755"
    # persistent state these subsystems expect to own
    "var/lib/systemd:755"
    "var/lib/dbus:755"
    "var/log/journal:755"
    # D-Bus: policy and service activation, vendor + admin
    "usr/share/dbus-1:755"
    "usr/share/dbus-1/system.d:755"
    "usr/share/dbus-1/session.d:755"
    "usr/share/dbus-1/system-services:755"
    "usr/share/dbus-1/services:755"
    "etc/dbus-1:755"
    "etc/dbus-1/system.d:755"
    "etc/dbus-1/session.d:755"
    # polkit: actions are vendor-owned, rules are admin-owned
    "usr/share/polkit-1:755"
    "usr/share/polkit-1/actions:755"
    "usr/share/polkit-1/rules.d:755"
    "etc/polkit-1:755"
    "etc/polkit-1/rules.d:755"
    # desktop-file and metadata targets every GUI package installs into
    "usr/share/applications:755"
    "usr/share/icons:755"
    "usr/share/pixmaps:755"
    "usr/share/metainfo:755"
    "usr/share/fonts:755"
    "usr/share/locale:755"
    "usr/share/licenses:755"
    "usr/share/terminfo:755"
    "usr/share/zoneinfo:755"
    # pkg-config search path, both halves
    "usr/lib/pkgconfig:755"
    "usr/share/pkgconfig:755"
    # shell completion drop-ins
    "usr/share/bash-completion:755"
    "usr/share/bash-completion/completions:755"
    "usr/share/zsh:755"
    "usr/share/zsh/site-functions:755"
)

# --- /etc fragment directories ----------------------------------------------
RAVEN_SKELETON_DIRS_ETC=(
    "etc/profile.d:755"
    "etc/skel:755"
    "etc/opt:755"             # FHS-required: config for /opt
    "etc/xdg:755"
    "etc/security:755"
    "etc/pam.d:755"
    "etc/ssl:755"
    "etc/ssl/certs:755"
    "etc/ca-certificates:755"
    "etc/sudoers.d:750"       # 0750: sudo refuses a world-readable include dir
)

# --- Raven's own -------------------------------------------------------------
# /etc/raven/init.d is read by load_dropin_services (init/src/main.rs) and is
# the directory control.rs tells the user to copy a service template into.
# /etc/raven/shutdown.d is its shutdown-hook counterpart (main.rs:1363).
# Both were absent, so both instructions pointed at nothing.
# /etc/raven/sleep.d is the same idea either side of a suspend (power.rs): the
# scripts in it run with "pre" before the machine sleeps and "post" after it
# wakes, which is where a driver that cannot survive S3 gets reloaded.
RAVEN_SKELETON_DIRS_RAVEN=(
    "etc/raven:755"
    "etc/raven/init.d:755"
    "etc/raven/shutdown.d:755"
    "etc/raven/sleep.d:755"
    "usr/share/raven:755"
    "usr/share/raven/services:755"
    "var/log/raven:755"       # created at boot by main.rs:94; shipped for parity
    "var/lib/pacman:755"      # rvn's db_path (RavenPackageManager/src/config.rs)
    "var/lib/pacman/sync:755"
    "var/lib/pacman/local:755"
    "var/lib/rvn:755"         # rvn's devel.json registry (src/devel.rs)
    # cawd's saved wifi passphrases. 0700 for the same reason CAW gives: the
    # file mode alone would not stop another user listing which networks this
    # machine knows.
    #
    # This has to exist before cawd runs, and cawd cannot create it. Its
    # `profile::save` makes the `profiles` leaf with a single-level mkdir(2),
    # so a missing parent gives ENOENT, the save fails, and the passphrase is
    # never written -- `caw connect` still associates, and the machine simply
    # does not rejoin after a reboot. Upstream relies on systemd creating it
    # from `StateDirectory=caw`; nothing honours that here.
    "var/lib/caw:700"
    "var/cache/pacman:755"
    "var/cache/pacman/pkg:755"
)

# =============================================================================
# Symlinks: "linkpath:target"
# =============================================================================
# RELATIVE TARGETS ONLY. An absolute target resolves against the BUILD HOST
# while the sysroot is being populated and against the booted root afterwards;
# those are different filesystems, and the bug is invisible until boot. This is
# the same rule usrmerge.sh states for the merge links, for the same reason.
#
# The first five are shipped as payload entries by Arch's `filesystem` package,
# so a real directory at any of them is an install-time conflict, not a
# cosmetic difference. The last two are Raven's own.
RAVEN_SKELETON_LINKS=(
    "etc/mtab:../proc/self/mounts"     # df, mount, umount read this
    "var/run:../run"                   # pre-/run daemons writing PID files
    "var/lock:../run/lock"             # UUCP-style serial device locks
    "var/mail:spool/mail"              # $MAIL, login's new-mail check
    "usr/local/share/man:../man"       # locally installed manpages
)

# ---------------------------------------------------------------------------
# NOT linked here, deliberately.
# ---------------------------------------------------------------------------
# /etc/os-release was in the table above and had to come out. The UAPI spec
# puts the canonical file at /usr/lib/os-release with /etc/os-release as a
# symlink to it, which is correct for a distribution that owns /usr/lib -- and
# wrong for this one, because Arch's `filesystem` package OWNS
# /usr/lib/os-release and ships `ID=arch` in it. `filesystem` is a dependency
# of essentially every Arch package, so the first `rvn install <anything>`
# overwrites it and the symlink hands Raven's identity to Arch: the running
# system reports NAME="Arch Linux".
#
# It broke the version string on the way there too. stage2-native.sh writes a
# real /etc/os-release and then `sed -i`s @RAVEN_VERSION@ out of it; with the
# symlink in place the heredoc wrote THROUGH it into /usr/lib/os-release, and
# the later skeleton pass replaced the substituted file with the link again --
# leaving PRETTY_NAME="Raven Linux @RAVEN_VERSION@" on the shipped image.
#
# So Raven's identity lives in a real /etc/os-release, at a path no Arch
# package claims, written and owned by stage2. Do not add it back.
#
# /etc/localtime is likewise absent: it is a real decision (the installer
# writes the timezone the operator picked), and a table that re-points it on
# every run would silently reset every installed system to UTC.

# =============================================================================
# Groups and users: "name:id"
# =============================================================================
# Arch's canonical numbering, from /usr/lib/sysusers.d/arch.conf plus the
# udev-assigned device groups every Arch install carries.
#
# These numbers are not arbitrary and not ours to choose. `rvn` extracts Arch
# packages, and a tar payload records ownership NUMERICALLY -- Arch's
# `filesystem` ships /srv/ftp as gid 11 and /var/games as gid 50. If gid 11 is
# `audio` on this system, an installed package's FTP root silently becomes
# group-writable by everyone in the audio group. Aligning the table is the only
# way that comes out right.
RAVEN_SKELETON_GROUPS=(
    "root:0"
    "bin:1"
    "daemon:2"
    "sys:3"
    "adm:4"
    "tty:5"                   # devpts mounts gid=5; no terminals without it
    "disk:6"
    "lp:7"
    "mem:8"
    "kmem:9"
    "wheel:10"
    "ftp:11"
    "mail:12"
    "uucp:14"
    "log:19"
    "rfkill:24"
    "smmsp:25"
    "proc:26"
    "games:50"
    "lock:54"
    "network:90"
    "video:91"
    "audio:92"
    "optical:93"
    "floppy:94"
    "storage:95"
    "scanner:96"
    "input:97"
    "power:98"
    "nobody:65534"
)

RAVEN_SKELETON_USERS=(
    # "name:uid:gid:gecos:home:shell"
    "bin:1:1:::/usr/bin/nologin"
    "daemon:2:2:::/usr/bin/nologin"
    "mail:8:12::/var/spool/mail:/usr/bin/nologin"   # uid 8, gid 12: intentional
    "ftp:14:11::/srv/ftp:/usr/bin/nologin"
    "http:33:33::/srv/http:/usr/bin/nologin"
)

# `http` has no group in arch.conf's group block because sysusers derives it
# from the user; spelled out here since we are writing the files directly.
RAVEN_SKELETON_GROUPS_EXTRA=(
    "http:33"
)

# =============================================================================
# Directories
# =============================================================================

# ---------------------------------------------------------------------------
# _raven_skel_mkdir <root> <entry>
#   entry is "path:mode" or "path:mode:gid".
#
#   The chmod is a SEPARATE call on purpose. `mkdir -m 1777` does not produce
#   1777: mkdir(2) masks the setuid, setgid and sticky bits out of its mode
#   argument regardless of umask, so -m 1777 yields 0777 -- world-writable with
#   no sticky bit, which is strictly worse than either half. /tmp, /var/tmp and
#   /var/spool/mail all depend on getting this right.
# ---------------------------------------------------------------------------
_raven_skel_mkdir() {
    local root="$1" entry="$2"
    local path mode gid rest

    path="${entry%%:*}"
    rest="${entry#*:}"
    if [[ "$rest" == *:* ]]; then
        mode="${rest%%:*}"
        gid="${rest#*:}"
    else
        mode="$rest"
        gid=""
    fi

    local full="${root}/${path}"

    # A symlink already sitting here is either one of ours (/var/run -> ../run,
    # whose target /run is in this same table) or a merge link. Following it
    # with mkdir -p is correct; chmod'ing through it is not, since a symlink
    # mode is meaningless and the chmod would land on the target instead.
    if [[ -L "$full" ]]; then
        mkdir -p "$full/" 2>/dev/null || true
        return 0
    fi

    if [[ -e "$full" && ! -d "$full" ]]; then
        log_error "skeleton: /${path} exists and is not a directory; expected mode ${mode}"
        return 1
    fi

    mkdir -p "$full" || return 1
    chmod "$mode" "$full" || return 1
    [[ -n "$gid" ]] && { chown ":${gid}" "$full" || return 1; }
    return 0
}

# ---------------------------------------------------------------------------
# raven_skeleton_dirs <root>
# ---------------------------------------------------------------------------
raven_skeleton_dirs() {
    local root="${1:?raven_skeleton_dirs: root required}"
    root="${root%/}"
    local entry rc=0

    for entry in \
        "${RAVEN_SKELETON_DIRS_TOPLEVEL[@]}" \
        "${RAVEN_SKELETON_DIRS_USR[@]}" \
        "${RAVEN_SKELETON_DIRS_VAR[@]}" \
        "${RAVEN_SKELETON_DIRS_ETC[@]}" \
        "${RAVEN_SKELETON_DIRS_SYSTEMD[@]}" \
        "${RAVEN_SKELETON_DIRS_RAVEN[@]}"
    do
        _raven_skel_mkdir "$root" "$entry" || rc=1
    done

    return $rc
}

# =============================================================================
# Symlinks
# =============================================================================

# ---------------------------------------------------------------------------
# raven_skeleton_links <root>
# ---------------------------------------------------------------------------
raven_skeleton_links() {
    local root="${1:?raven_skeleton_links: root required}"
    root="${root%/}"
    local entry link target full rc=0

    for entry in "${RAVEN_SKELETON_LINKS[@]}"; do
        link="${entry%%:*}"
        target="${entry#*:}"
        full="${root}/${link}"

        # Already correct: leave it alone, so a rerun logs nothing.
        if [[ -L "$full" && "$(readlink "$full")" == "$target" ]]; then
            continue
        fi

        # A real directory where a symlink belongs. Empty is recoverable --
        # `rvn` itself removes an empty directory to make way for a package's
        # symlink (find_conflicts in RavenPackageManager/src/extract.rs). A
        # POPULATED one is a bug in whichever stage put files there, and
        # silently deleting it would destroy them.
        if [[ -d "$full" && ! -L "$full" ]]; then
            if rmdir "$full" 2>/dev/null; then
                log_info "  skeleton: replaced empty directory /${link} with a symlink -> ${target}"
            else
                log_error "skeleton: /${link} is a non-empty directory; expected a symlink -> ${target}"
                log_error "          move its contents to /${target} and rerun"
                rc=1
                continue
            fi
        fi

        # A regular file, or a symlink pointing somewhere else.
        if [[ -e "$full" || -L "$full" ]]; then
            rm -f "$full"
        fi

        # The parent has to exist; /usr/local/share/man's parent is created by
        # the directory table, but a link added later may sit somewhere the
        # table does not create.
        mkdir -p "$(dirname "$full")"
        ln -sfn "$target" "$full" || rc=1
    done

    return $rc
}

# =============================================================================
# Groups and users
# =============================================================================
# Raven has no systemd, so nothing runs systemd-sysusers at boot and nothing
# will materialise these accounts later. They have to be in the shipped
# /etc/passwd and /etc/group or they do not exist at all.

# ---------------------------------------------------------------------------
# _raven_skel_groups <root>
#   Reconcile /etc/group (and /etc/gshadow) with RAVEN_SKELETON_GROUPS.
#
#   Three cases, in order of how much damage getting them wrong does:
#     * name absent            -> append it
#     * name present, wrong id -> renumber it, and chgrp every file that
#                                 carried the old id
#     * id held by a group not in the table -> hard error, no write
#
#   The renumber case is the one that matters. Raven shipped audio at gid 11
#   and video at 12, which are Arch's ftp and mail. Every Arch package that
#   ships a file owned by ftp or mail records those ids numerically in its tar
#   payload, so on the old numbering `rvn install` produced an /srv/ftp owned
#   by the audio group. Renaming is not enough; the numbers have to match.
# ---------------------------------------------------------------------------
_raven_skel_groups() {
    local root="$1"
    local gfile="${root}/etc/group"
    local gsfile="${root}/etc/gshadow"

    mkdir -p "${root}/etc"
    [[ -f "$gfile" ]] || : > "$gfile"

    declare -A have_gid=() have_members=()
    declare -a order=()
    local line name pw gid members entry

    while IFS= read -r line || [[ -n "$line" ]]; do
        [[ -z "$line" || "$line" == \#* ]] && continue
        IFS=':' read -r name pw gid members <<< "$line"
        [[ -z "$name" ]] && continue
        order+=("$name")
        have_gid["$name"]="$gid"
        have_members["$name"]="$members"
    done < "$gfile"

    # Desired numbering, kept in declaration order so appends are deterministic.
    declare -a want_order=()
    declare -A want_gid=()
    for entry in "${RAVEN_SKELETON_GROUPS[@]}" "${RAVEN_SKELETON_GROUPS_EXTRA[@]}"; do
        name="${entry%%:*}"
        want_order+=("$name")
        want_gid["$name"]="${entry##*:}"
    done

    # Resolve the final mapping before writing anything.
    declare -A final_gid=()
    declare -a renumber=()
    for name in "${order[@]}"; do
        final_gid["$name"]="${have_gid[$name]}"
    done
    for name in "${want_order[@]}"; do
        [[ -n "${have_gid[$name]+x}" ]] || continue
        if [[ "${have_gid[$name]}" != "${want_gid[$name]}" ]]; then
            renumber+=("${name}:${have_gid[$name]}:${want_gid[$name]}")
            final_gid["$name"]="${want_gid[$name]}"
        fi
    done

    # Every id, existing and about to be added, must be unique. Checked against
    # the FINAL mapping: audio 11 -> 92 and ftp being added at 11 is fine, and
    # only looks like a collision if you check before the renumber is applied.
    declare -A by_gid=()
    local dup=0 g
    for name in "${order[@]}"; do
        g="${final_gid[$name]}"
        if [[ -n "${by_gid[$g]+x}" ]]; then
            log_error "skeleton: gid ${g} claimed by both '${by_gid[$g]}' and '${name}'"
            dup=1
        fi
        by_gid["$g"]="$name"
    done
    for name in "${want_order[@]}"; do
        [[ -n "${have_gid[$name]+x}" ]] && continue
        g="${want_gid[$name]}"
        if [[ -n "${by_gid[$g]+x}" ]]; then
            log_error "skeleton: cannot add group '${name}' at gid ${g}; '${by_gid[$g]}' holds it"
            log_error "          resolve by hand -- renumbering someone else's group is not this script's call"
            dup=1
            continue
        fi
        by_gid["$g"]="$name"
    done
    [[ $dup -eq 0 ]] || { log_error "skeleton: /etc/group left unchanged"; return 1; }

    # Rewrite in place: existing lines keep their order and members, missing
    # groups are appended.
    local tmp="${gfile}.raven-skel.$$"
    : > "$tmp"
    for name in "${order[@]}"; do
        printf '%s:x:%s:%s\n' "$name" "${final_gid[$name]}" "${have_members[$name]}" >> "$tmp"
    done
    local added=0
    for name in "${want_order[@]}"; do
        [[ -n "${have_gid[$name]+x}" ]] && continue
        printf '%s:x:%s:\n' "$name" "${want_gid[$name]}" >> "$tmp"
        added=$((added + 1))
    done
    chmod 644 "$tmp"
    mv -f "$tmp" "$gfile"

    # gshadow mirrors group. '!' is a locked password, '::' is no admin and no
    # members -- membership lives in /etc/group.
    [[ -f "$gsfile" ]] || : > "$gsfile"
    for name in "${want_order[@]}"; do
        grep -q "^${name}:" "$gsfile" 2>/dev/null || printf '%s:!::\n' "$name" >> "$gsfile"
    done
    chmod 600 "$gsfile"

    # Files still carrying a renumbered gid. Nothing owns these today, but the
    # renumber is only correct if this runs -- otherwise the old ids become
    # orphaned numbers that `ls -l` prints raw.
    # Paths the DIRECTORY table owns by explicit gid are excluded from the
    # sweep. /srv/ftp is deliberately gid 11 because that is ftp -- and while
    # audio is being renumbered out of 11, a blind `find -gid 11` sweeps
    # /srv/ftp along with it and lands it on 92. The exclusion list is derived
    # from the tables rather than hardcoded, so adding a gid-bearing directory
    # cannot silently reintroduce this.
    local -a prune=()
    local entry rest
    for entry in "${RAVEN_SKELETON_DIRS_TOPLEVEL[@]}" "${RAVEN_SKELETON_DIRS_USR[@]}" \
                 "${RAVEN_SKELETON_DIRS_VAR[@]}" "${RAVEN_SKELETON_DIRS_ETC[@]}" \
                 "${RAVEN_SKELETON_DIRS_SYSTEMD[@]}" "${RAVEN_SKELETON_DIRS_RAVEN[@]}"; do
        rest="${entry#*:}"
        [[ "$rest" == *:* ]] || continue          # no gid field, not table-owned
        prune+=(-path "${root}/${entry%%:*}" -o)
    done

    local spec old new n
    for spec in "${renumber[@]}"; do
        name="${spec%%:*}"; spec="${spec#*:}"
        old="${spec%%:*}"; new="${spec#*:}"
        log_warn "  skeleton: group '${name}' renumbered ${old} -> ${new} to match Arch"
        # ${prune[@]} is deliberately unquoted-as-a-unit: these are separate
        # find arguments, and the trailing -o joins them to the -gid test.
        n="$( { find "$root" -xdev \( "${prune[@]}" -false \) -prune -o -gid "$old" -print 2>/dev/null || true; } | wc -l)"
        if [[ "$n" -gt 0 ]]; then
            log_warn "            re-owning ${n} file(s) from gid ${old} to ${new}"
            find "$root" -xdev \( "${prune[@]}" -false \) -prune -o -gid "$old" \
                -exec chown -h ":${new}" {} + 2>/dev/null || true
        fi
    done

    [[ $added -eq 0 ]] || log_info "  skeleton: added ${added} group(s) to /etc/group"
    return 0
}

# ---------------------------------------------------------------------------
# _raven_skel_users <root>
#   Append the Arch system users that are absent. Deliberately does NOT
#   renumber an existing user: a uid change means every file that user owns
#   has to move with it, and unlike the group case there is no table here
#   that says which of Raven's own accounts are safe to touch.
# ---------------------------------------------------------------------------
_raven_skel_users() {
    local root="$1"
    local pfile="${root}/etc/passwd"
    local sfile="${root}/etc/shadow"

    mkdir -p "${root}/etc"
    [[ -f "$pfile" ]] || : > "$pfile"

    declare -A have_user=() have_uid=()
    local line name pw uid rest entry gecos home shell gid

    while IFS= read -r line || [[ -n "$line" ]]; do
        [[ -z "$line" || "$line" == \#* ]] && continue
        IFS=':' read -r name pw uid rest <<< "$line"
        [[ -z "$name" ]] && continue
        have_user["$name"]=1
        have_uid["$uid"]="$name"
    done < "$pfile"

    local added=0 skipped=0
    for entry in "${RAVEN_SKELETON_USERS[@]}"; do
        IFS=':' read -r name uid gid gecos home shell <<< "$entry"
        [[ -n "${have_user[$name]+x}" ]] && continue
        if [[ -n "${have_uid[$uid]+x}" ]]; then
            log_warn "  skeleton: skipping user '${name}'; uid ${uid} already belongs to '${have_uid[$uid]}'"
            skipped=$((skipped + 1))
            continue
        fi
        [[ -n "$home" ]] || home="/"
        printf '%s:x:%s:%s:%s:%s:%s\n' "$name" "$uid" "$gid" "$gecos" "$home" "$shell" >> "$pfile"
        have_uid["$uid"]="$name"
        added=$((added + 1))
    done
    chmod 644 "$pfile"

    # '!*' is "no password, and none can be set" -- the correct state for a
    # system account. An account present in passwd but absent from shadow is
    # the one shape that can leave a passwordless login open.
    [[ -f "$sfile" ]] || : > "$sfile"
    for entry in "${RAVEN_SKELETON_USERS[@]}"; do
        name="${entry%%:*}"
        grep -q "^${name}:" "$sfile" 2>/dev/null || printf '%s:!*:::::::\n' "$name" >> "$sfile"
    done
    chmod 600 "$sfile"

    [[ $added -eq 0 ]]   || log_info "  skeleton: added ${added} system user(s) to /etc/passwd"
    [[ $skipped -eq 0 ]] || log_warn "  skeleton: ${skipped} system user(s) skipped on uid conflict"
    return 0
}

# ---------------------------------------------------------------------------
# raven_skeleton_accounts <root>
# ---------------------------------------------------------------------------
raven_skeleton_accounts() {
    local root="${1:?raven_skeleton_accounts: root required}"
    root="${root%/}"
    local rc=0
    _raven_skel_groups "$root" || rc=1
    _raven_skel_users  "$root" || rc=1
    return $rc
}

# =============================================================================
# Vendor defaults
# =============================================================================

# ---------------------------------------------------------------------------
# raven_skeleton_factory <root>
#   Populate /usr/share/factory/etc with a pristine copy of each shipped /etc
#   file, so a clobbered config can be restored without reinstalling anything.
#   Copies only what is already there; creates nothing.
# ---------------------------------------------------------------------------
raven_skeleton_factory() {
    local root="${1:?raven_skeleton_factory: root required}"
    root="${root%/}"
    local dest="${root}/usr/share/factory/etc"
    mkdir -p "$dest"

    local f src n=0
    for f in passwd group shadow gshadow shells fstab crypttab nsswitch.conf \
             host.conf hosts resolv.conf ld.so.conf profile securetty issue \
             subuid subgid login.defs environment locale.conf; do
        src="${root}/etc/${f}"
        [[ -f "$src" && ! -L "$src" ]] || continue
        # Only refresh when the content actually differs, so a rerun does not
        # churn mtimes and invalidate the squashfs cache for nothing.
        cmp -s "$src" "${dest}/${f}" && continue
        cp -p "$src" "${dest}/${f}"
        n=$((n + 1))
    done
    [[ $n -eq 0 ]] || log_info "  skeleton: refreshed ${n} vendor default(s) in /usr/share/factory/etc"
    return 0
}

# =============================================================================
# Orchestrator
# =============================================================================

# ---------------------------------------------------------------------------
# raven_skeleton_root <root>
#   Idempotent. Safe to call more than once, and stage4 does exactly that --
#   once early so the tree is right while it is being populated, and once after
#   cleanup_sysroot has deleted /usr/share/man and /usr/include out from under
#   it, immediately before the squashfs is sealed.
# ---------------------------------------------------------------------------
raven_skeleton_root() {
    local root="${1:?raven_skeleton_root: root required}"
    root="${root%/}"
    local rc=0

    # Accounts first. They decide what the numbers MEAN, and the directory
    # table then applies those numbers to paths (/srv/ftp at gid 11 = ftp).
    # Running dirs first sets the ownership and the renumber sweep immediately
    # walks over it.
    raven_skeleton_accounts "$root" || rc=1
    raven_skeleton_dirs     "$root" || rc=1
    raven_skeleton_links    "$root" || rc=1
    raven_skeleton_factory  "$root" || rc=1

    [[ $rc -eq 0 ]] || log_error "skeleton: one or more steps failed for ${root}"
    return $rc
}

# =============================================================================
# Verification
# =============================================================================

# ---------------------------------------------------------------------------
# raven_skeleton_verify <root> [--quiet]
#   Presence and mode, which is the check usrmerge.sh and check-layout.sh do
#   not make: an ABSENT path is not a merge error and not a package conflict,
#   so both of them pass on a rootfs that is missing /var/empty entirely.
#
#   Prints one line per failure and returns non-zero if there was any.
# ---------------------------------------------------------------------------
raven_skeleton_verify() {
    local root="${1:?raven_skeleton_verify: root required}"
    root="${root%/}"
    local quiet="${2:-}"
    local entry path mode gid rest full got fails=0

    for entry in \
        "${RAVEN_SKELETON_DIRS_TOPLEVEL[@]}" \
        "${RAVEN_SKELETON_DIRS_USR[@]}" \
        "${RAVEN_SKELETON_DIRS_VAR[@]}" \
        "${RAVEN_SKELETON_DIRS_ETC[@]}" \
        "${RAVEN_SKELETON_DIRS_SYSTEMD[@]}" \
        "${RAVEN_SKELETON_DIRS_RAVEN[@]}"
    do
        path="${entry%%:*}"
        rest="${entry#*:}"
        if [[ "$rest" == *:* ]]; then mode="${rest%%:*}"; gid="${rest#*:}"; else mode="$rest"; gid=""; fi
        full="${root}/${path}"

        # A symlink here is legitimate when the table also lists it as a link
        # (/var/run is both a directory target and a link source).
        [[ -L "$full" ]] && continue

        if [[ ! -d "$full" ]]; then
            echo "  missing directory  /${path}  (expected mode ${mode})"
            fails=$((fails + 1))
            continue
        fi
        got="$(stat -c '%a' "$full" 2>/dev/null)"
        # stat drops a leading zero on three-digit modes; compare numerically.
        if [[ "$((10#${got:-0}))" -ne "$((10#${mode}))" ]]; then
            echo "  wrong mode         /${path}  is ${got}, expected ${mode}"
            fails=$((fails + 1))
        fi
        if [[ -n "$gid" ]]; then
            got="$(stat -c '%g' "$full" 2>/dev/null)"
            if [[ "$got" != "$gid" ]]; then
                echo "  wrong group        /${path}  is gid ${got}, expected ${gid}"
                fails=$((fails + 1))
            fi
        fi
    done

    local link target
    for entry in "${RAVEN_SKELETON_LINKS[@]}"; do
        link="${entry%%:*}"
        target="${entry#*:}"
        full="${root}/${link}"
        if [[ ! -L "$full" ]]; then
            if [[ -e "$full" ]]; then
                echo "  not a symlink      /${link}  (expected -> ${target})"
            else
                echo "  missing symlink    /${link}  (expected -> ${target})"
            fi
            fails=$((fails + 1))
            continue
        fi
        got="$(readlink "$full")"
        if [[ "$got" != "$target" ]]; then
            echo "  wrong target       /${link}  -> ${got}, expected ${target}"
            fails=$((fails + 1))
        fi
    done

    # Accounts: name present at the right numeric id.
    local name want line f
    for entry in "${RAVEN_SKELETON_GROUPS[@]}" "${RAVEN_SKELETON_GROUPS_EXTRA[@]}"; do
        name="${entry%%:*}"; want="${entry##*:}"
        f="${root}/etc/group"
        [[ -f "$f" ]] || continue
        # `|| true`: grep exits 1 when the group is absent, which is a finding
        # to report, not a reason to abort. Without it a caller running under
        # `set -e` -- apply-skeleton.sh does -- dies on the first missing group
        # and prints a truncated report that looks like a clean one.
        line="$(grep "^${name}:" "$f" 2>/dev/null || true)"
        line="$(head -1 <<< "$line")"
        if [[ -z "$line" ]]; then
            echo "  missing group      ${name} (gid ${want})"
            fails=$((fails + 1))
        elif [[ "$(cut -d: -f3 <<< "$line")" != "$want" ]]; then
            echo "  wrong gid          ${name} is $(cut -d: -f3 <<< "$line"), expected ${want}"
            fails=$((fails + 1))
        fi
    done
    for entry in "${RAVEN_SKELETON_USERS[@]}"; do
        name="${entry%%:*}"
        f="${root}/etc/passwd"
        [[ -f "$f" ]] || continue
        if ! grep -q "^${name}:" "$f" 2>/dev/null; then
            echo "  missing user       ${name}"
            fails=$((fails + 1))
        fi
    done

    if [[ $fails -eq 0 && "$quiet" != "--quiet" ]]; then
        echo "  skeleton complete"
    fi
    return $(( fails > 0 ))
}
