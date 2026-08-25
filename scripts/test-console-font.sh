#!/bin/bash
# =============================================================================
# Host-side tests for the PSF console font generator
# =============================================================================
#
# Checks that make-console-font.py produces a PSF2 the kernel will accept, at
# every cell size stage4 builds, with the glyphs a TUI actually needs.
#
# Skips itself when the host has no freetype-py, the same way stage4 does: the
# build is fail-soft about the console font and the tests should be too.
#
#   ./scripts/test-console-font.sh          # or: make test

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "${SCRIPT_DIR}")"
GENERATOR="${SCRIPT_DIR}/make-console-font.py"
TTF="${PROJECT_ROOT}/fonts/JetBrainsMonoNerdFontMono-Regular.ttf"

if [[ -t 1 ]]; then
    GREEN=$'\033[1;32m'; RED=$'\033[1;31m'; YELLOW=$'\033[1;33m'
    WHITE=$'\033[1;37m'; NC=$'\033[0m'
else
    GREEN=""; RED=""; YELLOW=""; WHITE=""; NC=""
fi

FAILURES=0
WORKDIR="$(mktemp -d)"
trap 'rm -rf "${WORKDIR}"' EXIT

pass()   { echo "  ${GREEN}PASS${NC}  $1"; }
failed() { echo "  ${RED}FAIL${NC}  $1"; [[ $# -gt 1 ]] && printf '        %s\n' "${@:2}"; FAILURES=$((FAILURES + 1)); }
skip()   { echo "  ${YELLOW}SKIP${NC}  $1"; }

echo
echo "${WHITE}console font generator${NC}"

# --- prerequisites -----------------------------------------------------------
PYTHON=""
for candidate in python3 python; do
    if command -v "$candidate" >/dev/null 2>&1 && "$candidate" -c 'import freetype' 2>/dev/null; then
        PYTHON="$candidate"
        break
    fi
done

if [[ -z "$PYTHON" ]]; then
    skip "no Python with freetype-py; install python-freetype-py to run these"
    exit 0
fi

if [[ ! -f "$TTF" ]]; then
    failed "font not found at ${TTF}"
    exit 1
fi

# --- a reader for what the generator wrote -----------------------------------
# Deliberately a second implementation rather than importing the generator's:
# a bug shared by writer and reader would pass a round-trip test.
read_psf() {
    "$PYTHON" - "$1" <<'PY'
import struct, sys
data = open(sys.argv[1], "rb").read()
magic, ver, hdr, flags, count, charsize, height, width = struct.unpack_from("<4sIIIIIII", data, 0)
if magic != b"\x72\xb5\x4a\x86":
    print("BADMAGIC"); sys.exit(0)
print(f"version={ver} header={hdr} unicode={flags & 1} glyphs={count} "
      f"charsize={charsize} width={width} height={height}")
expected = height * ((width + 7) // 8)
print(f"charsize_ok={charsize == expected}")

bitmaps = data[hdr:hdr + count * charsize]
print(f"bitmap_len_ok={len(bitmaps) == count * charsize}")

# Unicode table: UTF-8 code points per glyph, 0xFF terminated.
table, idx, cur, cps = data[hdr + count * charsize:], 0, bytearray(), {}
for byte in table:
    if byte == 0xFF:
        if cur:
            cps[cur.decode("utf-8", "replace")] = idx
        cur = bytearray(); idx += 1
    else:
        cur.append(byte)
print(f"table_entries={idx}")
print("CODEPOINTS=" + ",".join(f"{ord(c):04X}" for c in cps))

# Ink coverage of the tiling characters, which have to reach the cell edges.
rb = (width + 7) // 8
def rows_set(ch):
    if ch not in cps: return None
    g = bitmaps[cps[ch] * charsize:(cps[ch] + 1) * charsize]
    return [any(g[r * rb + b] for b in range(rb)) for r in range(height)]
def cols_set(ch):
    if ch not in cps: return None
    g = bitmaps[cps[ch] * charsize:(cps[ch] + 1) * charsize]
    return [any(g[r * rb + (c >> 3)] & (0x80 >> (c & 7)) for r in range(height)) for c in range(width)]

v = rows_set("│")
print(f"vbar_top={bool(v and v[0])} vbar_bottom={bool(v and v[-1])}")
h = cols_set("─")
print(f"hbar_left={bool(h and h[0])} hbar_right={bool(h and h[-1])}")
p = rows_set("")
print(f"powerline_top={bool(p and p[0])} powerline_bottom={bool(p and p[-1])}")
a = rows_set("A")
print(f"letter_A_has_ink={bool(a and any(a))}")
PY
}

# --- every size stage4 builds ------------------------------------------------
# Glyphs a TUI is unusable without, plus one from each Nerd Font block.
REQUIRED_CODEPOINTS=(
    0041 007A 00E9 20AC        # text, accents, currency
    2500 2502 250C 2534 253C   # box drawing, including junctions
    2588 2591                  # block elements
    E0A0 E0B0                  # powerline
    E725 F07B F120 F00C        # devicon, folder, terminal, check
)

for size in 8x16 10x20 12x24 16x32; do
    w="${size%x*}"
    h="${size#*x}"
    out="${WORKDIR}/raven-${size}.psfu"

    if ! "$PYTHON" "$GENERATOR" "$TTF" --width "$w" --height "$h" -o "$out" >/dev/null 2>&1; then
        failed "${size}: generator exited non-zero"
        continue
    fi

    info="$(read_psf "$out")"

    if grep -q BADMAGIC <<<"$info"; then
        failed "${size}: not a PSF2 file"
        continue
    fi

    check() { # check DESC KEY EXPECTED
        local got
        got="$(grep -oP "(?<=\b$2=)\S+" <<<"$info" | head -1)"
        if [[ "$got" == "$3" ]]; then pass "${size}: $1"; else failed "${size}: $1" "$2 was '$got', wanted '$3'"; fi
    }

    check "header width is ${w}"             width  "$w"
    check "header height is ${h}"            height "$h"
    check "charsize matches the cell"        charsize_ok True
    check "bitmap block is the right length" bitmap_len_ok True
    check "carries a unicode table"          unicode 1

    glyphs="$(grep -oP '(?<=\bglyphs=)\d+' <<<"$info" | head -1)"
    entries="$(grep -oP '(?<=\btable_entries=)\d+' <<<"$info" | head -1)"

    if [[ "$glyphs" -gt 0 && "$glyphs" -le 512 ]]; then
        pass "${size}: ${glyphs} glyphs, within the console's limit of 512"
    else
        failed "${size}: ${glyphs} glyphs is outside 1..512"
    fi

    if [[ "$entries" == "$glyphs" ]]; then
        pass "${size}: unicode table has one entry per glyph"
    else
        failed "${size}: ${entries} table entries for ${glyphs} glyphs"
    fi

    present="$(grep -oP '(?<=^CODEPOINTS=).*' <<<"$info")"
    missing=()
    for cp in "${REQUIRED_CODEPOINTS[@]}"; do
        grep -q "\b${cp}\b" <<<"$present" || missing+=("U+${cp}")
    done
    if [[ ${#missing[@]} -eq 0 ]]; then
        pass "${size}: every required glyph is present"
    else
        failed "${size}: missing ${missing[*]}"
    fi

    # The whole point of rasterising box drawing from this font rather than
    # scaling it to fit: the stems must reach the cell edges or every drawn
    # line comes out as a column of dashes.
    check "vertical bar reaches the top"     vbar_top True
    check "vertical bar reaches the bottom"  vbar_bottom True
    check "horizontal bar reaches the left"  hbar_left True
    check "horizontal bar reaches the right" hbar_right True
    check "powerline separator fills the cell" powerline_top True
    check "letter A actually has ink"        letter_A_has_ink True
done

# --- kbd agrees it is a font -------------------------------------------------
if command -v psfgettable >/dev/null 2>&1; then
    if psfgettable "${WORKDIR}/raven-16x32.psfu" >/dev/null 2>&1; then
        pass "kbd's psfgettable parses the unicode table"
    else
        failed "kbd's psfgettable rejects the font"
    fi
else
    skip "psfgettable not installed; no cross-check against kbd"
fi

echo
if [[ $FAILURES -eq 0 ]]; then
    echo "  ${GREEN}All console font tests passed.${NC}"
    exit 0
fi
echo "  ${RED}${FAILURES} console font test(s) failed.${NC}"
exit 1
