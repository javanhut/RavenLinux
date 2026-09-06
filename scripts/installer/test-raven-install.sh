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
          ensure_group add_group_member remove_group_member next_free_uid \
          create_user grant_sudo set_hostname set_locale_and_time \
          switch_to_raven_init remove_live_credentials

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
audio:x:92:raven
video:x:91:raven
input:x:97:raven
users:x:100:raven
raven:x:1000:
caw:x:970:raven
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
# The gids are pinned deliberately. They are Arch's canonical numbers, and a
# tar payload records ownership NUMERICALLY -- Arch's `filesystem` ships
# /srv/ftp as gid 11, so when audio sat at 11 an `rvn install` produced an FTP
# root owned by the audio group. Pinning them here is what stops that drifting
# back. See scripts/lib/skeleton.sh for the full table.
matches "added to audio"                  '^audio:x:92:javan$' "$TARGET/etc/group"
matches "added to video"                  '^video:x:91:javan$' "$TARGET/etc/group"
matches "added to input"                  '^input:x:97:javan$' "$TARGET/etc/group"

# render was silently skipped before ensure_group existed: the image ships no
# such group, add_group_member fails on a group that is not there, and the
# caller swallowed it with `|| true`. udev gives /dev/dri/renderD* to group
# `render` at 0660, so with the group absent the node stayed root-only 0600 and
# no GPU-accelerated Wayland client could open it. No pinned gid here: unlike
# audio/video/input these have no canonical Arch number, so the test asserts
# the membership and lets ensure_group pick.
matches "render group created"            '^render:x:[0-9]+:javan$' "$TARGET/etc/group"
matches "seat group created"              '^seat:x:[0-9]+:javan$' "$TARGET/etc/group"
# caw gates cawd's state-changing socket commands on group membership. The
# image ships it with only the placeholder as a member, which the placeholder
# removal strips, so the new user has to be put back in.
matches "added to caw"                    '^caw:x:970:javan$' "$TARGET/etc/group"
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
section "initramfs: boot banner label"
# The banner is printed before /proc is mounted and before the root-mode
# parser runs, so it decides for itself from the command line. It used to say
# "Live Boot" unconditionally, which meant an installed machine announced
# itself as a live image on every boot. Its answer must agree with the parser
# tested above, so the two are checked against the same command lines.
banner_label() { # CMDLINE -> the label the initramfs would print
    local cl="$1" fixture
    fixture="$(mktemp)"
    printf '%s\n' "$cl" > "$fixture"
    local label="Live Boot"
    if grep -qE '(^| )root=[^ ]' "$fixture" 2>/dev/null \
        && ! grep -qE '(^| )raven\.live( |$)' "$fixture" 2>/dev/null; then
        label="Installed System"
    fi
    rm -f "$fixture"
    printf '%s' "$label"
}

check_banner() { # DESC CMDLINE WANT
    eq "$1" "$(banner_label "$2")" "$3"
}

check_banner "live ISO says Live Boot" \
    "rdinit=/init quiet loglevel=3 console=tty0" "Live Boot"
check_banner "installed disk does not say Live Boot" \
    "root=UUID=abc-123 rw quiet console=tty0" "Installed System"
check_banner "raven.live still says Live Boot" \
    "root=UUID=abc-123 rw raven.live" "Live Boot"
check_banner "no root= at all says Live Boot" \
    "BOOT_IMAGE=/vmlinuz rootwait" "Live Boot"

# The logic above mirrors what is embedded in build-initramfs.sh. If that
# drifts, these four checks are testing a copy and measuring nothing, so pin
# them to the real source -- unconditionally, because a pin that quietly skips
# itself is the same as no pin.
if grep -q 'RAVEN_BOOT_LABEL="Installed System"' "${INITRAMFS_BUILDER}" \
    && grep -q 'raven\\.live' "${INITRAMFS_BUILDER}" \
    && ! grep -q '${WHITE}Live Boot${NC}' "${INITRAMFS_BUILDER}"; then
    pass "build-initramfs.sh derives the banner label from the command line"
else
    failed "build-initramfs.sh no longer derives the banner label" \
           "the banner checks above are testing a copy that has drifted from it"
fi

# =============================================================================
# The live desktop runs as root; the installed one does not
# =============================================================================
#
# raven-init picks the session account from raven.user= on the kernel cmdline,
# falling back to the lowest-uid regular account. The live image asks for root
# outright -- its desktop exists to install the machine, and its tty1 is
# already an unauthenticated root shell -- and the installed machine must not
# inherit that, because there the desktop is somebody's session and uid 0 would
# make every application it launches root and the installer's own video, render
# and input groups meaningless.
#
# There is no runtime check keeping those apart. It is which boot entry is
# read: the ISO's, or the one install_bootloader writes to the new disk. So the
# thing to test is the entries.
section "session account: live vs installed"

STAGE4="${SCRIPT_DIR}/../stages/stage4-iso.sh"
BOOTCFG="${SCRIPT_DIR}/../../bootloader/src/config.rs"

if [[ -f "${STAGE4}" ]]; then
    grub_line="$(grep -E '^\s*linux /boot/vmlinuz.*raven\.graphics=wayland' "${STAGE4}" || true)"
    if [[ "$grub_line" == *"raven.user=root"* ]]; then
        pass "the ISO's GRUB desktop entry asks for a root session"
    else
        failed "the ISO's GRUB desktop entry does not carry raven.user=root" \
               "got: ${grub_line:-<no wayland linux line>}"
    fi
fi

if [[ -f "${BOOTCFG}" ]]; then
    # RavenBoot's compiled-in menu, which is what the ISO boots under UEFI:
    # these defaults are read only when the ESP has no boot.cfg, and
    # install_bootloader always writes one.
    boot_line="$(grep -E 'cmdline: String::from\(.*raven\.graphics=wayland' "${BOOTCFG}" || true)"
    if [[ "$boot_line" == *"raven.user=root"* ]]; then
        pass "RavenBoot's built-in desktop entry asks for a root session"
    else
        failed "RavenBoot's built-in desktop entry does not carry raven.user=root" \
               "got: ${boot_line:-<no wayland cmdline>}"
    fi
fi

# The one that matters. install_bootloader writes the installed machine's
# entries, and a raven.user=root that leaked into them would hand every
# installed desktop a root session -- silently, because it works.
installed_desktop="$(grep -E 'raven\.graphics=wayland raven\.wayland=huginn' "$INSTALLER" | grep 'cmdline' || true)"
if [[ -z "$installed_desktop" ]]; then
    failed "cannot find the installed system's Desktop boot entry in raven-install"
elif [[ "$installed_desktop" == *"raven.user"* ]]; then
    failed "the INSTALLED system's Desktop entry names a session user" \
           "an installed desktop must use the account raven-install created" \
           "got: ${installed_desktop}"
else
    pass "the installed system's Desktop entry names no session user"
fi

# And nothing else raven-install writes to that ESP does either.
if grep -q 'raven\.user' "$INSTALLER"; then
    failed "raven-install mentions raven.user somewhere" \
           "$(grep -n 'raven\.user' "$INSTALLER" | head -3)"
else
    pass "raven-install never writes raven.user into a boot entry"
fi

# =============================================================================
# The live image's passwordless sudo rule does not survive the install
# =============================================================================
#
# stage4 grants wheel a passwordless route to raven-install so the graphical
# installer can reach root on an image whose session runs as the `raven`
# placeholder and whose tty1 is already an unauthenticated root shell. Neither
# is true of the installed machine, where the same rule would be a real hole
# and an invisible one -- nothing on an installed system ever prompts for it,
# so nobody would find out it was there.
section "the live installer's sudo rule is removed at install time"

TARGET="${WORKDIR}/livecreds"
build_mock_target "$TARGET"
mkdir -p "${TARGET}/etc/sudoers.d"
LIVE_RULE="${TARGET}/etc/sudoers.d/10-raven-live-installer"
echo '%wheel ALL=(root) NOPASSWD: /usr/bin/raven-install, /usr/bin/reboot' > "${LIVE_RULE}"
printf 'keep me\n' > "${TARGET}/etc/sudoers.d/10-wheel"

remove_live_credentials

absent "the live NOPASSWD rule is gone from the installed system" "${LIVE_RULE}"
exists "other sudoers drop-ins are left alone"  "${TARGET}/etc/sudoers.d/10-wheel"

# Idempotent: an installed system being cloned onto a second disk has no such
# file, and the function is called on every install regardless.
remove_live_credentials && pass "running it again on a system without the rule is fine" \
    || failed "removing an absent rule reported an error"

# stage4 has to write the file this removes, at the path this removes, or the
# two halves are a rule that is granted and never withdrawn.
STAGE4="${SCRIPT_DIR}/../stages/stage4-iso.sh"
if [[ -f "${STAGE4}" ]]; then
    matches "stage4 writes the rule this function removes" \
        '10-raven-live-installer' "${STAGE4}"

    # And it grants only the two commands the window runs. A rule that drifted
    # to ALL would hand wheel a passwordless root shell on the live image, and
    # the tty1 argument for why that is acceptable does not stretch that far.
    rule_line="$(grep -E '^%wheel .*NOPASSWD' "${STAGE4}" || true)"
    if [[ "$rule_line" == *"NOPASSWD: /usr/bin/raven-install, /usr/bin/reboot" ]]; then
        pass "the rule is scoped to raven-install and reboot"
    else
        failed "the live sudo rule is not scoped to the two expected commands" \
               "got: ${rule_line:-<no %wheel NOPASSWD line>}"
    fi
fi

# =============================================================================
# The front-end interface: --answers
# =============================================================================
#
# Black-box, deliberately. load_answers reads two top-level arrays and writes
# six OPT_ variables, so lifting it out of the script the way import_fn does
# would be testing a reassembled copy of it. Running the real script is also
# the only way to check that the option parsing in front of it agrees.
#
# As a non-root user every run dies in preflight, which is after load_answers
# and before anything is written -- so the message on stdout says exactly how
# far it got.
section "answers file: parsing"

ANSWERS_DIR="${WORKDIR}/answers"
mkdir -p "${ANSWERS_DIR}"

write_answers() { # write_answers NAME <<< content
    local f="${ANSWERS_DIR}/$1"
    cat > "$f"
    chmod 0600 "$f"
    echo "$f"
}

run_answers() { # run_answers FILE -> combined output
    bash "$INSTALLER" --answers "$1" --non-interactive --dry-run 2>&1
}

says() { # says DESC FILE ERE
    local out
    out="$(run_answers "$2")"
    if grep -qE "$3" <<< "$out"; then
        pass "$1"
    else
        failed "$1" "no /$3/ in:" "$(head -3 <<< "$out")"
    fi
}

GOOD="$(write_answers good.conf <<'A'
# a comment, and a blank line follow

disk=/dev/nvme0n1
hostname=raven-test
username=javan
fullname=Javan Storm
user_password=hunter2
root_password=
user_sudo=1
timezone=UTC
locale=en_US.UTF-8
keymap=us
profile=minimal
swap=none
A
)"
says "a valid file is accepted"          "$GOOD" "Answers loaded"
says "and then stops at the root check"  "$GOOD" "must run as root"

BAD_KEY="$(write_answers badkey.conf <<'A'
disk=/dev/sda
hostname=raven
username=raven
hostnmae=typo
A
)"
says "a misspelled key is refused, with its line" "$BAD_KEY" "badkey.conf:4: unknown key 'hostnmae'"

MISSING="$(write_answers missing.conf <<'A'
disk=/dev/sda
hostname=raven
A
)"
says "a missing required key is named" "$MISSING" "missing required key 'username'"

NOT_KV="$(write_answers notkv.conf <<'A'
disk=/dev/sda
hostname=raven
username=raven
this line has no equals sign
A
)"
says "a line that is not key=value is refused" "$NOT_KV" "notkv.conf:4: not key=value"

# It holds passwords in the clear. A file the rest of the machine can read is
# worth stopping for, not warning about: by the time a warning has scrolled
# past, the install has already used it.
LOOSE="${ANSWERS_DIR}/loose.conf"
cp "$GOOD" "$LOOSE"; chmod 0644 "$LOOSE"
says "a world-readable answers file is refused" "$LOOSE" "mode 644; it holds passwords"

says "a missing answers file is refused" "${ANSWERS_DIR}/nope.conf" "No such answers file"

eq "--non-interactive without --answers is refused" \
   "$(bash "$INSTALLER" --non-interactive 2>&1 | head -1)" \
   "--non-interactive requires --answers FILE"

eq "--progress-fd needs an open descriptor" \
   "$(bash "$INSTALLER" --progress-fd 77 --probe 2>&1 | head -1)" \
   "--progress-fd 77 is not open for writing"

# The value is the rest of the line, unquoted and untrimmed, because a password
# is a value like any other. The one character that cannot appear in it is a
# newline -- which would end the record and turn the remainder into a key.
section "answers file: values are taken verbatim"

SPICY="$(write_answers spicy.conf <<'A'
disk=/dev/sda
hostname=raven
username=raven
user_password=  a b#c=d"e'f$(reboot)
A
)"
says "a password with spaces, quotes and a # is accepted" "$SPICY" "Answers loaded"
says "and nothing in it was executed"                     "$SPICY" "must run as root"

# =============================================================================
# The front-end interface: --probe
# =============================================================================
section "probe"

PROBE_OUT="${WORKDIR}/probe.txt"
bash "$INSTALLER" --probe > "${PROBE_OUT}" 2>/dev/null

matches "reports its protocol version" '^probe\.protocol=1$' "${PROBE_OUT}"
matches "reports the installer version" '^probe\.installer_version=' "${PROBE_OUT}"
matches "ends with a terminator"        '^probe\.end=1$'      "${PROBE_OUT}"
matches "reports the firmware"          '^preflight\.firmware=(uefi|bios)$' "${PROBE_OUT}"
matches "reports Secure Boot"           '^preflight\.secureboot=(on|off|unknown)$' "${PROBE_OUT}"
matches "offers at least one filesystem" '^preflight\.fs=' "${PROBE_OUT}"
matches "names the source tree"         '^source\.kind=(squashfs|live-root)$' "${PROBE_OUT}"
matches "suggests a swap size"          '^swap\.suggested=[0-9]+G$' "${PROBE_OUT}"
matches "lists the answer keys"         '^answers\.key=disk$'  "${PROBE_OUT}"
matches "lists the required ones"       '^answers\.required=' "${PROBE_OUT}"

# Nothing here is fatal: a machine the installer would refuse is a machine a
# front-end has to *show* somebody, and a probe that exited on it would have
# nothing to show them. As an ordinary user the root check is the one that
# fails, which makes it the test.
matches "a failed check is reported, not exited on" '^probe\.ok=0$' "${PROBE_OUT}"
matches "and the reason is given"                   '^probe\.error=raven-install must run as root' "${PROBE_OUT}"

# Every disk record is bracketed. An unbalanced pair means a front-end attaches
# one disk's partitions to another disk's name, which is the one parsing bug
# here that erases the wrong device.
eq "every disk.begin has a disk.end" \
   "$(grep -c '^disk\.begin=' "${PROBE_OUT}")" \
   "$(grep -c '^disk\.end=' "${PROBE_OUT}")"

# Both required by an installer that has none of its own defaults for them.
for k in $(grep '^answers.required=' "${PROBE_OUT}" | cut -d= -f2-); do
    if grep -q "^answers.key=${k}$" "${PROBE_OUT}"; then
        pass "required key '${k}' is also an accepted key"
    else
        failed "required key '${k}' is not in the accepted list"
    fi
done

# =============================================================================
# The front-end interface: the progress stream
# =============================================================================
section "progress stream"

STREAM="${WORKDIR}/stream.txt"
bash "$INSTALLER" --probe --progress-fd 3 3>"${STREAM}" >/dev/null 2>&1

matches "phases are announced with an id"  '^phase preflight ' "${STREAM}"
matches "status lines carry a verb"        '^ok '              "${STREAM}"
lacks   "no colour escapes reach the stream" $'\033'          "${STREAM}"

# Every verb on the stream is one this protocol defines. An unknown verb is a
# record a front-end silently drops.
if grep -vqE '^(phase|pct|ok|warn|fail|info|dirty|done)( |$)' "${STREAM}"; then
    failed "every record starts with a known verb" \
           "$(grep -vE '^(phase|pct|ok|warn|fail|info|dirty|done)( |$)' "${STREAM}" | head -3)"
else
    pass "every record starts with a known verb"
fi

# =============================================================================
# --dry-run writes nothing
# =============================================================================
#
# The one property everything else leans on. --dry-run is what makes it safe to
# run the installer to look at it, and what the graphical front-end's own
# development mode depends on; if the guard in confirm() ever moved below the
# non-interactive early return, "look at the plan" would become "erase the
# disk" with no error and no warning.
#
# Tested by control flow rather than by argument: every tool that can write to
# a disk is replaced by one that records that it was called. Then the same run
# is done again *without* --dry-run, and has to record four of them -- because
# a test that cannot detect a write proves nothing by not detecting one.
section "--dry-run writes nothing"

if ! unshare -r true 2>/dev/null; then
    # The run has to get past preflight's root check, and a user namespace is
    # how that is done without being root -- uid 0 inside it, and no privilege
    # at all over the host's block devices, so a guard that failed would be
    # stopped by the kernel rather than by luck.
    pass "no unshare -r on this host; --dry-run write test skipped"
else
    DRY="${WORKDIR}/dryrun"
    mkdir -p "${DRY}/bin"

    for t in wipefs mkfs.vfat mkfs.ext4 mkfs.xfs mkfs.btrfs mkswap dd \
             partx blockdev swapoff mount umount udevadm; do
        printf '#!/bin/sh\necho "%s $*" >> "%s/CALLED"\nexit 0\n' "$t" "${DRY}" > "${DRY}/bin/${t}"
        chmod 0755 "${DRY}/bin/${t}"
    done
    # This one fails, so the control run stops at the first real operation
    # instead of walking through a formatting it cannot do anyway.
    printf '#!/bin/sh\necho "sfdisk $*" >> "%s/CALLED"\nexit 1\n' "${DRY}" > "${DRY}/bin/sfdisk"
    chmod 0755 "${DRY}/bin/sfdisk"

    # A block device is needed only because choose_disk tests for one with
    # [[ -b ]]. An unbound loop device is the harmless way to satisfy that, and
    # is preferred over a real disk: the stubs and the namespace are what make
    # this safe, but naming a device that has nothing on it means no single one
    # of those three has to hold on its own.
    DRY_DISK=""
    for d in /dev/loop0 /dev/loop1 /dev/loop2; do
        [[ -b "$d" ]] && { DRY_DISK="$d"; break; }
    done
    [[ -z "${DRY_DISK}" ]] && DRY_DISK="$(lsblk -dn -o PATH 2>/dev/null | head -1)"

    if [[ -z "${DRY_DISK}" ]]; then
        pass "no block device to name as a target; --dry-run write test skipped"
    else
        cat > "${DRY}/answers" <<ANS
disk=${DRY_DISK}
hostname=raven
username=raven
timezone=UTC
profile=minimal
swap=none
ANS
        chmod 0600 "${DRY}/answers"

        dry_run_installer() { # dry_run_installer [extra args...]
            rm -f "${DRY}/CALLED"
            unshare -r env PATH="${DRY}/bin:${PATH}" \
                bash "$INSTALLER" --answers "${DRY}/answers" --non-interactive "$@" \
                >/dev/null 2>&1
            # In a subshell: a missing file is reported by the shell doing the
            # redirection, before wc's own stderr redirection can catch it, and
            # a missing file is the expected result of the passing case.
            ( wc -l < "${DRY}/CALLED" ) 2>/dev/null | tr -d ' ' || echo 0
        }

        eq "--dry-run calls no disk-writing tool at all" "$(dry_run_installer --dry-run)" "0"

        # The control. Without it the assertion above is satisfied by a test
        # that simply cannot see anything.
        without="$(dry_run_installer)"
        if [[ "${without}" -ge 1 ]]; then
            pass "without --dry-run it calls ${without} of them, so the check has teeth"
        else
            failed "without --dry-run it called none either" \
                   "the stubs are not being reached; this test proves nothing"
        fi

        # And the guard is in front of the non-interactive path specifically:
        # that early return was added for the graphical front-end, and confirm()
        # has to reach the --dry-run exit before it.
        matches "--dry-run says so, and says nothing was written" \
            'dry-run: stopping here' \
            <(unshare -r env PATH="${DRY}/bin:${PATH}" bash "$INSTALLER" \
                --answers "${DRY}/answers" --non-interactive --dry-run 2>&1)
    fi
fi

# =============================================================================
# Parity with the graphical front-end
# =============================================================================
#
# installer-ui/ is a second way to drive this script, and the two agree through
# three lists: the answer keys it writes, the phase ids it draws a row for, and
# the protocol version it refuses to guess past. Nothing at run time notices
# when one of those drifts -- a key the installer stopped accepting is an
# install that dies after the summary -- so it is noticed here.
section "parity: installer-ui"

UI_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)/installer-ui"

if [[ ! -d "${UI_DIR}" ]]; then
    pass "installer-ui/ is not in this tree; parity checks skipped"
else
    # Keys the front-end writes into the answers file, from its to_file().
    ui_keys="$(grep -oE 'put\("[a-z_]+"' "${UI_DIR}/src/answers.rs" \
               | sed 's/put("//; s/"//' | sort -u)"
    installer_keys="$(grep '^answers.key=' "${PROBE_OUT}" | cut -d= -f2- | sort -u)"

    if [[ "$ui_keys" == "$installer_keys" ]]; then
        pass "the keys installer-ui writes are exactly the keys raven-install accepts"
    else
        failed "installer-ui and raven-install disagree about the answer keys" \
               "only in installer-ui: $(comm -23 <(echo "$ui_keys") <(echo "$installer_keys") | tr '\n' ' ')" \
               "only in raven-install: $(comm -13 <(echo "$ui_keys") <(echo "$installer_keys") | tr '\n' ' ')"
    fi

    # Phase ids: the script's `phase <id>` calls against the PHASES table the
    # front-end draws its checklist from.
    installer_phases="$(grep -oE '^\s*phase [a-z]+ ' "$INSTALLER" | awk '{print $2}' | sort -u)"
    ui_phases="$(grep -oE '^\s*\("[a-z]+", "' "${UI_DIR}/src/engine.rs" \
                 | sed 's/.*("//; s/", "//' | sort -u)"

    if [[ "$ui_phases" == "$installer_phases" ]]; then
        pass "the phases installer-ui draws are exactly the phases raven-install announces"
    else
        failed "installer-ui and raven-install disagree about the phases" \
               "only in installer-ui: $(comm -23 <(echo "$ui_phases") <(echo "$installer_phases") | tr '\n' ' ')" \
               "only in raven-install: $(comm -13 <(echo "$ui_phases") <(echo "$installer_phases") | tr '\n' ' ')"
    fi

    # The version each side stamps on the wire. installer-ui refuses to drive a
    # protocol it does not know, so these two moving apart is a front-end that
    # will not start rather than one that misreads a record -- but it should
    # still be a deliberate change, made in both places.
    installer_proto="$(awk -F'"' '/^RAVEN_INSTALL_PROTOCOL=/ {print $2}' "$INSTALLER")"
    ui_proto="$(awk -F'"' '/^pub const SUPPORTED_PROTOCOL/ {print $2}' "${UI_DIR}/src/probe.rs")"
    eq "both sides speak the same protocol version" "$ui_proto" "$installer_proto"
fi

# =============================================================================
section "result"
if [[ $FAILURES -eq 0 ]]; then
    echo "  ${GREEN}All raven-install tests passed.${NC}"
    exit 0
fi
echo "  ${RED}${FAILURES} raven-install test(s) failed.${NC}"
exit 1
