#!/bin/bash
# Host-side tests for raven-fstrim.
#
# Runs the real script with a fake fstrim, findmnt, ionice and nice on the
# PATH and a private stamp and conf, so nothing here touches the machine or
# needs root. Covers: not due, due, forced, disabled, live media, a failing
# fstrim, and that the daemon's first sleep is the configured boot delay.
#
#   ./scripts/test-raven-fstrim.sh        # or: imlazy test
set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FSTRIM="${SCRIPT_DIR}/../configs/raven-fstrim"

if [[ -t 1 ]]; then GREEN=$'\033[1;32m'; RED=$'\033[1;31m'; WHITE=$'\033[1;37m'; NC=$'\033[0m'; else GREEN=""; RED=""; WHITE=""; NC=""; fi
FAILURES=0
W="$(mktemp -d)"; trap 'rm -rf "${W}"' EXIT
pass() { echo "  ${GREEN}PASS${NC}  $1"; }
failed() { echo "  ${RED}FAIL${NC}  $1"; [[ $# -gt 1 ]] && printf '        %s\n' "${@:2}"; FAILURES=$((FAILURES + 1)); }
eq() { if [[ "$2" == "$3" ]]; then pass "$1"; else failed "$1" "want: $3" "got:  $2"; fi; }
exists() { if [[ -e "$2" ]]; then pass "$1"; else failed "$1" "$2 does not exist"; fi; }
absent() { if [[ -e "$2" ]]; then failed "$1" "$2 exists"; else pass "$1"; fi; }

mkdir -p "$W/bin"
cat > "$W/bin/fstrim" <<'F'
#!/bin/sh
echo "fstrim $*" >> "$FAKE_LOG"
exit "$(cat "$FAKE_FSTRIM_EXIT")"
F
cat > "$W/bin/findmnt" <<'F'
#!/bin/sh
cat "$FAKE_ROOTFS"
F
# ionice and nice: record that they wrapped the call, then run it.
printf '#!/bin/sh\necho "ionice $1 $2" >> "$FAKE_LOG"; shift 2; exec "$@"\n' > "$W/bin/ionice"
printf '#!/bin/sh\necho "nice $1 $2" >> "$FAKE_LOG"; shift 2; exec "$@"\n' > "$W/bin/nice"
chmod +x "$W/bin/"*
export PATH="$W/bin:$PATH"
export FAKE_LOG="$W/log" FAKE_FSTRIM_EXIT="$W/exit" FAKE_ROOTFS="$W/rootfs"
export RAVEN_FSTRIM_CONF="$W/fstrim.conf" RAVEN_FSTRIM_STAMP="$W/state/fstrim.stamp" RAVEN_FSTRIM_CHECK_SECS=1

reset() { rm -rf "$W/state" "$W/log" "$W/fstrim.conf"; : > "$W/log"; echo 0 > "$W/exit"; echo ext4 > "$W/rootfs"; }
calls() { grep -c '^fstrim' "$W/log" 2>/dev/null; }

echo; echo "${WHITE}raven-fstrim${NC}"

reset
out="$("$FSTRIM" --once 2>&1)"; rc=$?
eq     "no stamp: due, trims"             "$rc" "0"
eq     "no stamp: one fstrim call"        "$(calls)" "1"
grep -q '^fstrim -av$' "$W/log" && pass "trims every mounted filesystem (-av)" || failed "fstrim args" "$(cat "$W/log")"
grep -q '^ionice -c 3$' "$W/log" && pass "idle I/O class" || failed "ionice" "$(cat "$W/log")"
grep -q '^nice -n 19$' "$W/log" && pass "lowest CPU priority" || failed "nice" "$(cat "$W/log")"
exists "stamp written"                     "$W/state/fstrim.stamp"

: > "$W/log"
"$FSTRIM" --once > "$W/out" 2>&1; rc=$?
eq     "fresh stamp: not due"              "$rc" "0"
eq     "fresh stamp: no fstrim call"       "$(calls)" "0"
grep -q 'not due' "$W/out" && pass "says it is not due" || failed "message" "$(cat "$W/out")"

"$FSTRIM" --now > "$W/out" 2>&1; rc=$?
eq     "--now ignores the stamp"           "$(calls)" "1"

reset; mkdir -p "$W/state"; touch -d '8 days ago' "$W/state/fstrim.stamp"
"$FSTRIM" --once > "$W/out" 2>&1
eq     "stamp 8 days old: due"             "$(calls)" "1"
reset; mkdir -p "$W/state"; touch -d '6 days ago' "$W/state/fstrim.stamp"
"$FSTRIM" --once > "$W/out" 2>&1
eq     "stamp 6 days old: not due"         "$(calls)" "0"
reset; mkdir -p "$W/state"; touch -d '3 days ago' "$W/state/fstrim.stamp"; echo 'INTERVAL_DAYS=2' > "$W/fstrim.conf"
"$FSTRIM" --once > "$W/out" 2>&1
eq     "conf shortens the interval"        "$(calls)" "1"

reset; echo 'ENABLED=no' > "$W/fstrim.conf"
"$FSTRIM" --now > "$W/out" 2>&1; rc=$?
eq     "disabled: exit 0"                  "$rc" "0"
eq     "disabled: no fstrim call"          "$(calls)" "0"

reset; echo overlay > "$W/rootfs"
"$FSTRIM" --now > "$W/out" 2>&1; rc=$?
eq     "live media: exit 0"                "$rc" "0"
eq     "live media: no fstrim call"        "$(calls)" "0"
grep -q 'live media' "$W/out" && pass "live media: says why" || failed "message" "$(cat "$W/out")"

reset; echo 1 > "$W/exit"
"$FSTRIM" --now > "$W/out" 2>&1; rc=$?
eq     "fstrim failure: non-zero"          "$rc" "1"
absent "fstrim failure: no stamp"          "$W/state/fstrim.stamp"

# Daemon: boot delay first, then a check. With a 0 s delay and 1 s checks it
# must have trimmed once within a couple of seconds.
reset; echo 'BOOT_DELAY_SECS=0' > "$W/fstrim.conf"
"$FSTRIM" > "$W/out" 2>&1 &
pid=$!
sleep 2.5
kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null
eq     "daemon: trims once when due"       "$(calls)" "1"
exists "daemon: stamp written"             "$W/state/fstrim.stamp"

echo
if (( FAILURES == 0 )); then echo "${GREEN}All raven-fstrim tests passed${NC}"; exit 0; fi
echo "${RED}${FAILURES} raven-fstrim test(s) failed${NC}"; exit 1
