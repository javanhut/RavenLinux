#!/bin/bash
# =============================================================================
# Host-side tests for raven-install
# =============================================================================
#
# Runs on the build host, needs no ISO, no container and no root. It sources
# individual functions out of raven-install and exercises them against a mock
# target tree, which covers the part of the installer that is pure file
# manipulation -- user creation, group membership, sudoers, the init handoff.
#
# What it deliberately does not cover: partitioning, mkfs, and the copy. Those
# need a real block device, and a test that fabricates one is testing losetup.
# Use 'raven-install --dry-run' on the live image for those.
#
#   ./scripts/installer/test-raven-install.sh        # or: imlazy test

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALLER="${SCRIPT_DIR}/raven-install"

if [[ -t 1 ]]; then
    GREEN=$'\033[1;32m'; RED=$'\033[1;31m'; WHITE=$'\033[1;37m'; NC=$'\033[0m'
else
    GREEN=""; RED=""; WHITE=""; NC=""
fi

FAILURES=0
WORKDIR="$(mktemp -d)"
trap 'rm -rf "${WORKDIR}"' EXIT

section() { echo; echo "${WHITE}${1}${NC}"; }

pass() { echo "  ${GREEN}PASS${NC}  $1"; }
failed() {
    echo "  ${RED}FAIL${NC}  $1"
    [[ $# -gt 1 ]] && printf '        %s\n' "${@:2}"
    FAILURES=$((FAILURES + 1))
}

eq() { # eq DESC ACTUAL EXPECTED
    if [[ "$2" == "$3" ]]; then pass "$1"; else failed "$1" "want: $3" "got:  $2"; fi
}

matches() { # matches DESC ERE FILE
    if grep -qE "$2" "$3" 2>/dev/null; then pass "$1"; else failed "$1" "no /$2/ in $3"; fi
}

lacks() { # lacks DESC ERE FILE
    if grep -qE "$2" "$3" 2>/dev/null; then failed "$1" "unexpected /$2/ in $3"; else pass "$1"; fi
}

exists() { # exists DESC PATH
    if [[ -e "$2" ]]; then pass "$1"; else failed "$1" "$2 does not exist"; fi
}

absent() { # absent DESC PATH
    if [[ -e "$2" ]]; then failed "$1" "$2 still exists"; else pass "$1"; fi
}

# Lift one function out of the installer by name. Cheaper and far less
# surprising than sourcing the whole script, which would parse arguments, set
# traps and run main().
import_fn() {
    local fn
    for fn in "$@"; do
        local body
        body="$(awk -v f="$fn" '$0 ~ "^" f "\\(\\) \\{" {p=1} p {print} p && /^\}$/ {exit}' "$INSTALLER")"
        if [[ -z "$body" ]]; then
            failed "internal: cannot find function ${fn}() in raven-install"
            continue
        fi
        eval "$body"
    done
}

# The installer's own logging and helpers, stubbed. The functions under test
# call these; none of them is what is being tested.
RED=""; GREEN=""; YELLOW=""; BLUE=""; WHITE=""; CYAN=""; NC=""
ok()   { :; }
warn() { :; }
fail() { :; }
info() { :; }
step() { :; }
have() { command -v "$1" >/dev/null 2>&1; }
die()  { echo "unexpected die: $*" >&2; exit 9; }

import_fn partdev valid_username valid_hostname \
          add_group_member remove_group_member next_free_uid \
          create_user grant_sudo set_hostname set_locale_and_time \
          switch_to_raven_init

# =============================================================================
# Partition device naming
# =============================================================================
section "partition device naming"

eq "nvme gets a p"        "$(partdev /dev/nvme0n1 1)" "/dev/nvme0n1p1"
eq "nvme third partition" "$(partdev /dev/nvme0n1 3)" "/dev/nvme0n1p3"
eq "sata gets no p"       "$(partdev /dev/sda 1)"     "/dev/sda1"
eq "sata second"          "$(partdev /dev/sda 2)"     "/dev/sda2"
eq "mmc gets a p"         "$(partdev /dev/mmcblk0 1)" "/dev/mmcblk0p1"
eq "virtio gets no p"     "$(partdev /dev/vda 2)"     "/dev/vda2"

# =============================================================================
# Input validation
# =============================================================================
section "input validation"

for u in raven javan a_b user-1 _sys; do
    valid_username "$u" && pass "username '${u}' accepted" || failed "username '${u}' rejected"
done
for u in "1abc" "Root" "has space" "" "$(printf 'x%.0s' {1..40})"; do
    valid_username "$u" && failed "username '${u:0:20}' accepted" || pass "username '${u:0:20}' rejected"
done

for h in raven raven-laptop zephyrus g14 a; do
    valid_hostname "$h" && pass "hostname '${h}' accepted" || failed "hostname '${h}' rejected"
done
for h in "-raven" "raven-" "raven.local" "has space" ""; do
    valid_hostname "$h" && failed "hostname '${h}' accepted" || pass "hostname '${h}' rejected"
done

# =============================================================================
# A mock sysroot, mirroring what stage4 puts in the squashfs
# =============================================================================
build_mock_target() {
    local t="$1"
    rm -rf "$t"
    mkdir -p "$t"/{etc/raven,etc/skel,etc/sudoers.d,sbin,bin,usr/bin,home,usr/share/zoneinfo/America}

    cat > "$t/etc/passwd" <<'EOF'
root:x:0:0:root:/root:/bin/bash
raven:x:1000:1000:Raven User:/home/raven:/bin/bash
nobody:x:65534:65534:Nobody:/:/bin/false
EOF
    cat > "$t/etc/group" <<'EOF'
root:x:0:
wheel:x:10:raven
audio:x:11:raven
video:x:12:raven
input:x:13:raven
users:x:100:raven
raven:x:1000:
nobody:x:65534:
EOF
    cat > "$t/etc/shadow" <<'EOF'
root::0:0:99999:7:::
raven::0:0:99999:7:::
EOF
    # The commented-out wheel line is the interesting case: an image whose
    # sudoers ships with wheel disabled turns "you have sudo" into
    # "user is not in the sudoers file" the first time it is used.
    cat > "$t/etc/sudoers" <<'EOF'
Defaults env_reset
root ALL=(ALL:ALL) ALL
# %wheel ALL=(ALL:ALL) ALL
EOF
    cat > "$t/etc/raven/init.toml" <<'EOF'
[system]
hostname = "raven-linux"
log_level = "info"

[[services]]
name = "getty-tty1"
exec = "/sbin/agetty"
args = ["--noclear", "--skip-login", "--login-program", "/bin/raven-shell", "tty1", "linux"]
tty = "/dev/tty1"
enabled = true

[[services]]
name = "getty-ttyS0"
exec = "/sbin/agetty"
args = ["--noclear", "--autologin", "root", "-L", "115200", "ttyS0", "vt102"]
enabled = false
EOF
    echo 'export PS1' > "$t/etc/skel/.bashrc"
    mkdir -p "$t/home/raven"
    echo marker > "$t/home/raven/marker"

    # The live image's init arrangement, which the installer has to undo.
    touch "$t/init" && chmod +x "$t/init"
    ln -sf /init "$t/sbin/init"
    touch "$t/sbin/raven-init" && chmod +x "$t/sbin/raven-init"
    touch "$t/bin/login" "$t/bin/bash" "$t/usr/bin/ravenshell"
    chmod +x "$t/bin/login" "$t/bin/bash" "$t/usr/bin/ravenshell"
    touch "$t/usr/share/zoneinfo/America/New_York"
}

# =============================================================================
# Configuring a target for a new user
# =============================================================================
section "configure: a user other than the shipped placeholder"

TARGET="${WORKDIR}/newuser"
build_mock_target "$TARGET"

HOSTNAME="zephyrus"; USERNAME="javan"; FULLNAME="Javan"; USER_SUDO=1
TIMEZONE="America/New_York"; LOCALE="en_US.UTF-8"; KEYMAP="us"

set_hostname
create_user
set_locale_and_time
switch_to_raven_init

eq      "/etc/hostname"                   "$(cat "$TARGET/etc/hostname")" "zephyrus"
matches "/etc/hosts carries the hostname" '127\.0\.1\.1[[:space:]]+zephyrus' "$TARGET/etc/hosts"
matches "init.toml hostname rewritten"    'hostname = "zephyrus"' "$TARGET/etc/raven/init.toml"

matches "user in passwd with ravenshell"  '^javan:x:1000:1000:Javan:/home/javan:/usr/bin/ravenshell$' "$TARGET/etc/passwd"
matches "user group created"              '^javan:x:1000:$' "$TARGET/etc/group"
matches "shadow entry is locked"          '^javan:!:' "$TARGET/etc/shadow"
exists  "home directory"                  "$TARGET/home/javan"
exists  "skel copied into home"           "$TARGET/home/javan/.bashrc"

lacks   "placeholder gone from passwd"    '^raven:' "$TARGET/etc/passwd"
lacks   "placeholder gone from shadow"    '^raven:' "$TARGET/etc/shadow"
lacks   "placeholder gone from group"     'raven'   "$TARGET/etc/group"
absent  "placeholder home removed"        "$TARGET/home/raven"

matches "added to wheel"                  '^wheel:x:10:javan$' "$TARGET/etc/group"
matches "added to audio"                  '^audio:x:11:javan$' "$TARGET/etc/group"
matches "added to video"                  '^video:x:12:javan$' "$TARGET/etc/group"
matches "sudoers.d grants wheel"          '^%wheel ALL=\(ALL:ALL\) ALL$' "$TARGET/etc/sudoers.d/10-wheel"
matches "sudoers reads sudoers.d"         '@includedir /etc/sudoers\.d' "$TARGET/etc/sudoers"

eq      "localtime points at the zone"    "$(readlink "$TARGET/etc/localtime")" "/usr/share/zoneinfo/America/New_York"
eq      "/etc/timezone"                   "$(cat "$TARGET/etc/timezone")" "America/New_York"
matches "locale.conf"                     '^LANG=en_US\.UTF-8$' "$TARGET/etc/locale.conf"
matches "vconsole.conf"                   '^KEYMAP=us$' "$TARGET/etc/vconsole.conf"
eq      "machine-id is 32 hex digits"     "$(tr -d '\n' < "$TARGET/etc/machine-id" | grep -cE '^[0-9a-f]{32}$')" "1"

eq      "/sbin/init points at raven-init" "$(readlink "$TARGET/sbin/init")" "raven-init"
absent  "live /init removed"              "$TARGET/init"
matches "tty1 asks for a login"           '^args = \["--noclear", "tty1", "linux"\]$' "$TARGET/etc/raven/init.toml"
lacks   "no skip-login left on tty1"      'skip-login' "$TARGET/etc/raven/init.toml"
matches "serial getty args untouched"     '"--autologin", "root"' "$TARGET/etc/raven/init.toml"
exists  "first-boot marked done"          "$TARGET/etc/.raven-first-boot-done"

# =============================================================================
# Configuring a target for the placeholder's own name
# =============================================================================
section "configure: installing as the shipped placeholder name"

TARGET="${WORKDIR}/sameuser"
build_mock_target "$TARGET"

HOSTNAME="raven"; USERNAME="raven"; FULLNAME="Javan Storm"; USER_SUDO=1

set_hostname
create_user

matches "placeholder updated in place" '^raven:x:1000:1000:Javan Storm:/home/raven:/usr/bin/ravenshell$' "$TARGET/etc/passwd"
matches "not listed in wheel twice"    '^wheel:x:10:raven$' "$TARGET/etc/group"
matches "existing shadow entry kept"   '^raven::' "$TARGET/etc/shadow"
exists  "existing home preserved"      "$TARGET/home/raven/marker"

# =============================================================================
# The other half of "installed and bootable": the initramfs has to read root=
# off the kernel command line, or the disk the installer just wrote never gets
# mounted and the boot falls back to hunting for a live squashfs.
# =============================================================================
section "initramfs: root= parsing"

INITRAMFS_BUILDER="${SCRIPT_DIR}/../build-initramfs.sh"

if [[ ! -f "$INITRAMFS_BUILDER" ]]; then
    failed "scripts/build-initramfs.sh not found"
else
    # The parser lives inside the quoted heredoc that becomes the initramfs
    # /init, so it has to come out of there rather than off the filesystem.
    parser="$(awk '/^raven_root_from_cmdline\(\) \{/,/^\}$/' "$INITRAMFS_BUILDER")"
    if [[ -z "$parser" ]]; then
        failed "raven_root_from_cmdline() not found in build-initramfs.sh" \
               "raven-install greps the initramfs for this name to tell whether" \
               "the image can boot from a disk; renaming it breaks that check."
    else
        # The parser reads /proc/cmdline. Shadow cat(1) so it reads a fixture.
        cat() {
            if [[ "${1:-}" == "/proc/cmdline" ]]; then
                printf '%s\n' "$FAKE_CMDLINE"
            else
                command cat "$@"
            fi
        }
        eval "$parser"

        check_cmdline() { # DESC CMDLINE WANT_MODE WANT_SPEC WANT_RW WANT_INIT
            RAVEN_ROOT_MODE="live"; RAVEN_ROOT_SPEC=""; RAVEN_ROOT_FSTYPE=""
            RAVEN_ROOT_FLAGS=""; RAVEN_ROOT_RW="rw"; RAVEN_ROOT_WAIT=30
            RAVEN_INIT_OVERRIDE=""
            FAKE_CMDLINE="$2"
            raven_root_from_cmdline
            eq "$1" \
               "${RAVEN_ROOT_MODE}|${RAVEN_ROOT_SPEC}|${RAVEN_ROOT_RW}|${RAVEN_INIT_OVERRIDE}" \
               "${3}|${4}|${5}|${6}"
        }

        check_cmdline "live ISO, no root=" \
            "rdinit=/init quiet loglevel=3 console=tty0" live "" rw ""
        check_cmdline "installed, root=UUID" \
            "root=UUID=abc-123 rw quiet console=tty0" disk "UUID=abc-123" rw ""
        check_cmdline "installed, mounted read-only" \
            "root=UUID=abc-123 ro" disk "UUID=abc-123" ro ""
        check_cmdline "rescue entry (init=/bin/bash)" \
            "root=UUID=abc-123 rw init=/bin/bash" disk "UUID=abc-123" rw "/bin/bash"
        check_cmdline "root=LABEL" \
            "root=LABEL=RAVEN_ROOT rw" disk "LABEL=RAVEN_ROOT" rw ""
        check_cmdline "root=PARTUUID" \
            "root=PARTUUID=deadbeef-01 rw rootfstype=ext4" disk "PARTUUID=deadbeef-01" rw ""
        check_cmdline "root as a device path" \
            "root=/dev/nvme0n1p3 rw" disk "/dev/nvme0n1p3" rw ""
        check_cmdline "raven.live overrides root=" \
            "root=UUID=abc-123 rw raven.live" live "" rw ""

        unset -f cat
    fi
fi

# =============================================================================
section "result"
if [[ $FAILURES -eq 0 ]]; then
    echo "  ${GREEN}All raven-install tests passed.${NC}"
    exit 0
fi
echo "  ${RED}${FAILURES} raven-install test(s) failed.${NC}"
exit 1
