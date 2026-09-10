#!/usr/bin/env bash
# Host-side tests for raven-postinstall's boot mode.
#
# Runs the real script against a fake rvn and private state files, so nothing
# here needs root or touches the machine. Every path the postinstall service
# can take is covered: nothing recorded, already done, minimal, success,
# repeated failure, and the motd block that comes and goes with it.
#
#   ./scripts/installer/test-raven-postinstall.sh        # or: imlazy test
set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
POSTINSTALL="${SCRIPT_DIR}/raven-postinstall"

if [[ -t 1 ]]; then
    GREEN=$'\033[1;32m'; RED=$'\033[1;31m'; WHITE=$'\033[1;37m'; NC=$'\033[0m'
else
    GREEN=""; RED=""; WHITE=""; NC=""
fi

FAILURES=0
W="$(mktemp -d)"
trap 'rm -rf "${W}"' EXIT

section() { echo; echo "${WHITE}${1}${NC}"; }
pass() { echo "  ${GREEN}PASS${NC}  $1"; }
failed() {
    echo "  ${RED}FAIL${NC}  $1"
    [[ $# -gt 1 ]] && printf '        %s\n' "${@:2}"
    FAILURES=$((FAILURES + 1))
}
eq() { if [[ "$2" == "$3" ]]; then pass "$1"; else failed "$1" "want: $3" "got:  $2"; fi; }
exists() { if [[ -e "$2" ]]; then pass "$1"; else failed "$1" "$2 does not exist"; fi; }
absent() { if [[ -e "$2" ]]; then failed "$1" "$2 still exists"; else pass "$1"; fi; }
matches() { if grep -qE "$2" "$3" 2>/dev/null; then pass "$1"; else failed "$1" "no /$2/ in $3"; fi; }
lacks() { if grep -qE "$2" "$3" 2>/dev/null; then failed "$1" "unexpected /$2/ in $3"; else pass "$1"; fi; }

# A fake rvn that records its argv and exits as told.
mkdir -p "$W/bin" "$W/profiles"
cat > "$W/bin/rvn" <<'RVN'
#!/bin/sh
echo "rvn $*" >> "${FAKE_RVN_LOG}"
exit "$(cat "${FAKE_RVN_EXIT}")"
RVN
chmod +x "$W/bin/rvn"
printf 'seatd\nlibinput  # comment\n\n' > "$W/profiles/desktop.packages"
printf '# nothing\n' > "$W/profiles/minimal.packages"

export PATH="$W/bin:$PATH"
export FAKE_RVN_LOG="$W/rvn.log" FAKE_RVN_EXIT="$W/rvn.exit"
export RAVEN_PROFILE_DIR="$W/profiles"
export RAVEN_INSTALL_PROFILE_FILE="$W/install-profile"
export RAVEN_POSTINSTALL_DONE="$W/done"
export RAVEN_POSTINSTALL_PENDING="$W/pending"
export RAVEN_POSTINSTALL_FAILED="$W/failed"
export RAVEN_MOTD="$W/motd"
export RAVEN_POSTINSTALL_RETRY_DELAY=0
export RAVEN_POSTINSTALL_REQUIRE_ROOT=0
# The /usr/local step works on a private tree, never the host's. reset()
# removes it, so the profile tests below see the step as a silent no-op; the
# "/usr/local" section builds it closed on purpose. The group is one this user
# is in but not their primary one, so the chgrp actually changes something
# without root; a user with no supplementary group gets the primary and the
# gid assertions still hold.
export RAVEN_LOCAL_PREFIX="$W/local"
alt_gid=$(id -G | tr ' ' '\n' | awk -v p="$(id -g)" '$1 != p { print; exit }')
export RAVEN_LOCAL_GROUP="${alt_gid:-$(id -g)}"

reset() {
    rm -f "$W/rvn.log" "$W/done" "$W/pending" "$W/failed" "$W/motd" "$W/install-profile"
    rm -rf "$W/local"
    echo 0 > "$W/rvn.exit"
    : > "$W/rvn.log"
}
# A /usr/local as an image built before the skeleton opened it: root-style
# 755 directories, 644 and 755 files, primary group throughout.
closed_local() {
    rm -rf "$W/local"
    mkdir -p "$W/local/bin" "$W/local/share/applications"
    printf 'bin\n' > "$W/local/bin/tool"; chmod 755 "$W/local/bin/tool"
    printf 'desktop\n' > "$W/local/share/applications/x.desktop"; chmod 644 "$W/local/share/applications/x.desktop"
    chmod 755 "$W/local" "$W/local/bin" "$W/local/share" "$W/local/share/applications"
    chgrp -R "$(id -g)" "$W/local"
}
mode_gid() { stat -c '%a:%g' "$1"; }
rvn_calls() { awk '/^rvn/ { n++ } END { print n + 0 }' "$W/rvn.log" 2>/dev/null; }

# Does this machine have a route? The success paths need one, since the script
# waits for it; without one they are skipped rather than reported wrongly.
have_route() { awk 'NR > 1 && $2 == "00000000" { f = 1 } END { exit !f }' /proc/net/route 2>/dev/null; }

section "boot mode with nothing to do"
reset
out="$("$POSTINSTALL" --auto 2>&1)"; rc=$?
eq     "no recorded profile: exit 0"        "$rc" "0"
eq     "no recorded profile: silent"        "$out" ""
eq     "no recorded profile: rvn not run"   "$(rvn_calls)" "0"
absent "no recorded profile: no motd"       "$W/motd"

reset; echo "profile=desktop" > "$W/install-profile"; echo "profile=desktop" > "$W/done"
out="$("$POSTINSTALL" --auto 2>&1)"; rc=$?
eq     "already done: exit 0"               "$rc" "0"
eq     "already done: rvn not run"          "$(rvn_calls)" "0"

reset; echo "profile=minimal" > "$W/install-profile"
"$POSTINSTALL" --auto >/dev/null 2>&1; rc=$?
eq     "minimal: exit 0"                    "$rc" "0"
eq     "minimal: rvn not run"               "$(rvn_calls)" "0"
exists "minimal: recorded as done"          "$W/done"
matches "minimal: done names the profile"   '^profile=minimal$' "$W/done"

if have_route; then
    section "boot mode, network present"
    reset; echo "profile=desktop" > "$W/install-profile"; printf 'Welcome to this box\n' > "$W/motd"
    "$POSTINSTALL" --auto --wait 5 > "$W/out" 2>&1; rc=$?
    eq      "success: exit 0"                   "$rc" "0"
    eq      "success: one rvn call"             "$(rvn_calls)" "1"
    matches "success: rvn got -y and the list"  '^rvn install -y seatd libinput$' "$W/rvn.log"
    exists  "success: done marker"              "$W/done"
    matches "success: done names the profile"   '^profile=desktop$' "$W/done"
    absent  "success: no pending marker"        "$W/pending"
    absent  "success: no failed marker"         "$W/failed"
    matches "success: admin motd text kept"     '^Welcome to this box$' "$W/motd"
    lacks   "success: motd block removed"       'raven-postinstall' "$W/motd"

    reset; echo "profile=desktop" > "$W/install-profile"; echo 1 > "$W/rvn.exit"
    "$POSTINSTALL" --auto --wait 5 > "$W/out" 2>&1; rc=$?
    eq      "failure: exit 1"                   "$rc" "1"
    eq      "failure: three attempts"           "$(rvn_calls)" "3"
    absent  "failure: no done marker"           "$W/done"
    exists  "failure: failed marker"            "$W/failed"
    matches "failure: reason recorded"          '^rvn install failed$' "$W/failed"
    absent  "failure: pending cleared"          "$W/pending"
    matches "failure: motd says so"             'could not be installed' "$W/motd"
    matches "failure: motd says how to retry"   'sudo raven-postinstall' "$W/motd"
    matches "failure: motd block is bracketed"  '^# raven-postinstall: begin$' "$W/motd"

    # A later boot that succeeds clears everything the failure left.
    echo 0 > "$W/rvn.exit"; : > "$W/rvn.log"
    "$POSTINSTALL" --auto --wait 5 > "$W/out" 2>&1; rc=$?
    eq      "retry next boot: exit 0"           "$rc" "0"
    absent  "retry next boot: failed cleared"   "$W/failed"
    absent  "retry next boot: motd gone"        "$W/motd"
    exists  "retry next boot: done"             "$W/done"
else
    echo "  (no default route on this host; skipping the network paths)"
fi

section "/usr/local handed to wheel"
G="$RAVEN_LOCAL_GROUP"
reset; closed_local
out="$("$POSTINSTALL" --auto 2>&1)"; rc=$?
eq      "boot, closed prefix: exit 0"            "$rc" "0"
matches "boot, closed prefix: says so"           'writable by wheel' <(printf '%s\n' "$out")
eq      "boot: prefix is wheel's, setgid, g+w"   "$(mode_gid "$W/local")" "2775:$G"
eq      "boot: bin opened"                       "$(mode_gid "$W/local/bin")" "2775:$G"
eq      "boot: nested dir opened"                "$(mode_gid "$W/local/share/applications")" "2775:$G"
eq      "boot: executable now g+w"               "$(mode_gid "$W/local/bin/tool")" "775:$G"
eq      "boot: data file now g+w"                "$(mode_gid "$W/local/share/applications/x.desktop")" "664:$G"
eq      "boot: rvn not run"                      "$(rvn_calls)" "0"

out="$("$POSTINSTALL" --auto 2>&1)"; rc=$?
eq      "boot, open prefix: exit 0"              "$rc" "0"
eq      "boot, open prefix: silent"              "$out" ""

reset; closed_local
out="$("$POSTINSTALL" --dry-run --profile minimal 2>&1)"; rc=$?
eq      "dry-run: exit 0"                        "$rc" "0"
matches "dry-run: says what it would do"         'Would hand' <(printf '%s\n' "$out")
eq      "dry-run: prefix untouched"              "$(mode_gid "$W/local/bin")" "755:$(id -g)"

reset; closed_local
out="$(RAVEN_POSTINSTALL_REQUIRE_ROOT=1 "$POSTINSTALL" --profile minimal 2>&1)"; rc=$?
eq      "unprivileged: exit 0"                   "$rc" "0"
matches "unprivileged: asks for root"            'run this as root' <(printf '%s\n' "$out")
eq      "unprivileged: prefix untouched"         "$(mode_gid "$W/local/bin")" "755:$(id -g)"

reset; closed_local
out="$(RAVEN_LOCAL_PREFIX= "$POSTINSTALL" --auto 2>&1)"; rc=$?
eq      "empty prefix: skipped"                  "$(mode_gid "$W/local/bin")" "755:$(id -g)"
eq      "empty prefix: silent"                   "$out" ""

section "by hand"
reset
"$POSTINSTALL" -y --profile desktop > "$W/out" 2>&1; rc=$?
eq      "-y: exit 0"                        "$rc" "0"
matches "-y: rvn called with -y"            '^rvn install -y seatd libinput$' "$W/rvn.log"
exists  "-y: recorded as done"              "$W/done"

reset
"$POSTINSTALL" --dry-run --profile desktop > "$W/out" 2>&1; rc=$?
eq      "dry-run: exit 0"                   "$rc" "0"
matches "dry-run: rvn --dry-run"            '^rvn install --dry-run seatd libinput$' "$W/rvn.log"
absent  "dry-run: not recorded as done"     "$W/done"

reset
"$POSTINSTALL" -y --profile nosuch > "$W/out" 2>&1; rc=$?
eq      "unknown profile: exit 1"           "$rc" "1"
eq      "unknown profile: rvn not run"      "$(rvn_calls)" "0"

reset; echo 1 > "$W/rvn.exit"
"$POSTINSTALL" -y --profile desktop > "$W/out" 2>&1; rc=$?
eq      "-y with rvn failing: non-zero"     "$rc" "1"
absent  "-y with rvn failing: not done"     "$W/done"

echo
if (( FAILURES == 0 )); then
    echo "${GREEN}All raven-postinstall tests passed${NC}"
    exit 0
fi
echo "${RED}${FAILURES} raven-postinstall test(s) failed${NC}"
exit 1
