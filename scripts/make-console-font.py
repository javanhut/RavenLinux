#!/usr/bin/env python3
"""Rasterise a TTF/OTF into a PSF2 console font for the Linux virtual terminal.

The VT cannot use a TTF. It wants PSF: a fixed-size bitmap per glyph, plus a
table saying which Unicode code points map to each. This turns
`fonts/JetBrainsMonoNerdFontMono-Regular.ttf` into exactly that, so tty1 renders
in the same typeface as the rest of the system -- box drawing, block elements
and the Nerd Font icons that `crow` and `ravenshell` emit included.

Usage:
    make-console-font.py FONT.ttf -o raven-16x32.psfu --width 16 --height 32

Two constraints shape the glyph set:

  * The console holds at most 256 glyphs *usably*. It can address 512, but above
    256 it takes an attribute bit to index them -- and on fbcon, which is what an
    EFI framebuffer console is, that bit is the foreground intensity. Every cell
    drawn bright or bold then renders as glyph+256 instead of the glyph asked
    for. On a console whose output is mostly bright text the result is a screen
    of box-drawing characters: glyph 0 (space) plus 256 lands on U+2533.

    That is not the "costs you bright backgrounds, foreground unaffected" trade
    this file used to claim. It cost the whole console. 256 is the budget.

  * Every glyph is the same cell. The Nerd Font *Mono* variants are designed for
    this -- their icons are pre-scaled to a single cell -- which is why the
    build uses those and not the proportional ones.

Needs freetype-py. On Arch: pacman -S python-freetype-py
"""

import argparse
import sys

try:
    import freetype
except ImportError:
    sys.exit(
        "make-console-font.py needs freetype-py.\n"
        "  Arch:   pacman -S python-freetype-py\n"
        "  pip:    pip install freetype-py"
    )

PSF2_MAGIC = b"\x72\xb5\x4a\x86"
PSF2_HAS_UNICODE_TABLE = 0x01
PSF2_MAX_GLYPHS = 256

# 256 glyphs, and ASCII plus Latin-1 already spends 191 of them. This is a
# budget, not a wish list: everything below the cut simply is not in the font.
# Ordered by how much its absence hurts: text first, then the frame-drawing
# characters a TUI is built out of, then icons.
GLYPH_SET = [
    # --- text ---------------------------------------------------------------
    ("ASCII", range(0x20, 0x7F)),
    ("Latin-1 supplement", range(0xA0, 0x100)),
    (
        "punctuation",
        [
            0x2010, 0x2013, 0x2014, 0x2018, 0x2019, 0x201C, 0x201D,
            0x2022, 0x2026, 0x2030, 0x2039, 0x203A, 0x20AC, 0x2122,
        ],
    ),
    # --- the characters a TUI draws itself with ------------------------------
    # Curated, and ahead of the full ranges below: with only ~65 glyphs left
    # after text, declaration order decides what survives, and "every box
    # drawing character in order" would spend all of it on single-line variants
    # before reaching the heavy and rounded ones a prompt actually uses.
    (
        "box drawing (essential)",
        [
            0x2500, 0x2502, 0x250C, 0x2510, 0x2514, 0x2518,   # light
            0x251C, 0x2524, 0x252C, 0x2534, 0x253C,
            0x2550, 0x2551, 0x2554, 0x2557, 0x255A, 0x255D,   # double
            0x2560, 0x2563, 0x2566, 0x2569, 0x256C,
            0x256D, 0x256E, 0x256F, 0x2570,                   # rounded
            0x2501, 0x2503, 0x250F, 0x2513, 0x2517, 0x251B,   # heavy
        ],
    ),
    ("block elements", [0x2580, 0x2584, 0x2588, 0x258C, 0x2590, 0x2591, 0x2592, 0x2593]),
    ("box drawing (rest)", range(0x2500, 0x2580)),
    (
        "geometric shapes",
        [0x25A0, 0x25AA, 0x25AB, 0x25B2, 0x25B6, 0x25BC, 0x25C0, 0x25CB, 0x25CF],
    ),
    ("arrows", [0x2190, 0x2191, 0x2192, 0x2193, 0x2194, 0x2195, 0x21B5]),
    (
        "symbols",
        [
            0x2018, 0x2020, 0x2021, 0x2026, 0x2190, 0x2260, 0x2264, 0x2265,
            0x2502, 0x256D, 0x256E, 0x256F, 0x2570,
            0x2713, 0x2714, 0x2717, 0x2718, 0x26A0, 0x2764,
        ],
    ),
    # --- Nerd Font ----------------------------------------------------------
    # Powerline first: a prompt with these missing is visibly broken in a way a
    # missing folder icon is not.
    ("powerline", [0xE0A0, 0xE0A1, 0xE0A2, 0xE0A3, 0xE0B0, 0xE0B1, 0xE0B2, 0xE0B3]),
    (
        "powerline extra",
        [0xE0B4, 0xE0B5, 0xE0B6, 0xE0B7, 0xE0BC, 0xE0BD, 0xE0BE, 0xE0BF],
    ),
    (
        "devicons",
        [
            0xE702,  # git
            0xE703,  # git branch alt
            0xE711,  # linux
            0xE712,  # go
            0xE718,  # node
            0xE71E,  # python alt
            0xE725,  # git branch
            0xE726,  # git merge
            0xE727,  # git pull request
            0xE728,  # git commit
            0xE729,  # git compare
            0xE73C,  # python
            0xE7A8,  # rust
            0xE7C5,  # vim
        ],
    ),
    (
        "file and folder icons",
        [
            0xF015,  # home
            0xF016,  # file-o
            0xF01C,  # inbox
            0xF023,  # lock
            0xF07B,  # folder
            0xF07C,  # folder-open
            0xF0A0,  # hdd
            0xF0C4,  # scissors
            0xF0C5,  # copy
            0xF0E7,  # bolt
            0xF11C,  # keyboard
            0xF120,  # terminal
            0xF121,  # code
            0xF126,  # code-fork
            0xF15B,  # file
            0xF1C0,  # database
            0xF233,  # server
            0xF240,  # battery-full
            0xF244,  # battery-empty
        ],
    ),
    (
        "status icons",
        [
            0xF00C,  # check
            0xF00D,  # times
            0xF05A,  # info-circle
            0xF06A,  # exclamation-circle
            0xF071,  # warning
            0xF085,  # cogs
            0xF0AD,  # wrench
            0xF0E4,  # dashboard
            0xF110,  # spinner
            0xF017,  # clock
            0xF1EB,  # wifi
            0xF293,  # bluetooth
            0xF2C7,  # thermometer
        ],
    ),
]


def collect_codepoints():
    """Flatten GLYPH_SET, dropping duplicates but keeping the first ordering."""
    seen = set()
    ordered = []
    groups = []

    for name, points in GLYPH_SET:
        added = 0
        for cp in points:
            if cp in seen:
                continue
            seen.add(cp)
            ordered.append(cp)
            added += 1
        groups.append((name, added))

    return ordered, groups


# The characters whose ink must fit the cell without clipping. Deliberately not
# everything: see fit_to_cell.
TEXT_SAMPLE = list(range(0x20, 0x7F)) + list(range(0xA0, 0x100))


def measure_ink(face, codepoints):
    """Ink extents above and below the baseline, and the widest advance."""
    above = below = advance = 0
    for cp in codepoints:
        if face.get_char_index(cp) == 0:
            continue
        face.load_char(cp, freetype.FT_LOAD_RENDER | freetype.FT_LOAD_TARGET_MONO)
        glyph = face.glyph
        advance = max(advance, glyph.advance.x / 64.0)
        if glyph.bitmap.rows == 0:
            continue
        above = max(above, glyph.bitmap_top)
        below = max(below, glyph.bitmap.rows - glyph.bitmap_top)
    return above, below, advance


def fit_to_cell(face, cell_w, cell_h):
    """Largest em size that fits the cell, and the baseline to draw text on.

    Two things decide the size, and neither is the font's nominal line height:

      * The advance has to fit the cell width. Nothing can be done about a glyph
        wider than its cell, so this is a hard limit.

      * The *text* ink has to fit the cell height. Only the text: JetBrains
        Mono draws its box-drawing and Powerline glyphs deliberately taller than
        the line box so that they meet across cell boundaries, and at em 26
        U+2502 carries 40px of ink against a 35px line height. Requiring those
        to fit would shrink the whole font by a third to accommodate the overlap
        that is the entire point of them. They are meant to be clipped at the
        cell edge, and render_glyph clips rather than scaling.
    """
    for size in range(cell_h + 8, 3, -1):
        face.set_pixel_sizes(0, size)
        above, below, advance = measure_ink(face, TEXT_SAMPLE)
        if advance <= cell_w and above + below <= cell_h:
            # Split whatever leading is left over above and below the text.
            baseline = above + (cell_h - (above + below)) // 2
            # 16.16 fixed point, as FreeType's transform matrices want.
            hscale = int(round(cell_w / advance * 0x10000)) if advance else 0x10000
            return size, baseline, hscale

    sys.exit(
        f"No pixel size fits a {cell_w}x{cell_h} cell for this font.\n"
        "Try a wider or taller cell."
    )


# Characters that have to meet their neighbours across the cell boundary: box
# drawing, block elements, and the Powerline separators. Everything in here gets
# stretched to the full cell width when the advance falls short of it -- see
# render_glyph.
def is_tiling(cp):
    return (
        0x2500 <= cp <= 0x259F      # box drawing and block elements
        or 0xE0B0 <= cp <= 0xE0BF   # Powerline separators
    )


IDENTITY = freetype.Matrix(0x10000, 0, 0, 0x10000)


def render_glyph(face, cp, cell_w, cell_h, baseline, hscale=None):
    """Render one code point into a bit-packed cell, MSB first, or None."""
    if face.get_char_index(cp) == 0:
        return None

    # An advance narrower than the cell leaves a dead column on the right. For
    # ordinary text that is just the letter-spacing the font asked for; for a
    # horizontal rule it is a gap, and a row of them reads as a dashed line
    # instead of a solid one. At 8x16 JetBrains Mono advances 7px into an 8px
    # cell, so this is not a hypothetical.
    #
    # Stretching the geometric characters the last fraction to the cell edge is
    # invisible -- they are rules and blocks, not letterforms -- and it is the
    # only thing that makes them join.
    stretched = hscale is not None and hscale > 0x10000 and is_tiling(cp)
    if stretched:
        face.set_transform(freetype.Matrix(hscale, 0, 0, 0x10000), freetype.Vector(0, 0))

    face.load_char(cp, freetype.FT_LOAD_RENDER | freetype.FT_LOAD_TARGET_MONO)

    if stretched:
        face.set_transform(IDENTITY, freetype.Vector(0, 0))

    bmp = face.glyph.bitmap

    row_bytes = (cell_w + 7) // 8
    cell = bytearray(row_bytes * cell_h)

    # The font's own bearings, with no centring of any kind. The cell is exactly
    # one advance wide and the font is monospaced, so the bearing already places
    # the glyph correctly -- and centring would shift box-drawing characters off
    # the cell edges they have to meet at, turning every drawn line into a row
    # of dashes.
    x_off = face.glyph.bitmap_left
    y_off = baseline - face.glyph.bitmap_top

    for row in range(bmp.rows):
        y = y_off + row
        if y < 0 or y >= cell_h:
            continue
        for col in range(bmp.width):
            x = x_off + col
            if x < 0 or x >= cell_w:
                continue
            byte = bmp.buffer[row * bmp.pitch + (col >> 3)]
            if byte & (0x80 >> (col & 7)):
                cell[y * row_bytes + (x >> 3)] |= 0x80 >> (x & 7)

    return bytes(cell)


def build_psf2(ttf_path, cell_w, cell_h, verbose=False):
    face = freetype.Face(ttf_path)
    pixel_size, baseline, hscale = fit_to_cell(face, cell_w, cell_h)

    codepoints, groups = collect_codepoints()

    glyphs = []
    mapped = []
    missing = []

    for cp in codepoints:
        if len(glyphs) >= PSF2_MAX_GLYPHS:
            missing.append(cp)
            continue
        bitmap = render_glyph(face, cp, cell_w, cell_h, baseline, hscale)
        if bitmap is None:
            missing.append(cp)
            continue
        glyphs.append(bitmap)
        mapped.append(cp)

    row_bytes = (cell_w + 7) // 8
    charsize = row_bytes * cell_h

    header = (
        PSF2_MAGIC
        + (0).to_bytes(4, "little")           # version
        + (32).to_bytes(4, "little")          # header size
        + PSF2_HAS_UNICODE_TABLE.to_bytes(4, "little")
        + len(glyphs).to_bytes(4, "little")
        + charsize.to_bytes(4, "little")
        + cell_h.to_bytes(4, "little")
        + cell_w.to_bytes(4, "little")
    )

    unicode_table = bytearray()
    for cp in mapped:
        unicode_table += chr(cp).encode("utf-8")
        unicode_table += b"\xff"              # end of this glyph's entry

    if verbose:
        print(f"  font        {ttf_path}")
        stretch = hscale / 0x10000
        print(f"  cell        {cell_w}x{cell_h} (em {pixel_size}px, baseline {baseline})")
        if stretch > 1.001:
            print(f"  tiling      box drawing widened x{stretch:.3f} to reach the cell edge")
        for name, count in groups:
            print(f"  {name:<24} {count:>4} requested")
        print(f"  glyphs      {len(glyphs)} of {PSF2_MAX_GLYPHS}")
        if missing:
            shown = ", ".join(f"U+{c:04X}" for c in missing[:8])
            more = f" (+{len(missing) - 8} more)" if len(missing) > 8 else ""
            print(f"  not in font {len(missing)}: {shown}{more}")

    return header + b"".join(glyphs) + bytes(unicode_table), len(glyphs)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("font", help="TTF or OTF to rasterise")
    ap.add_argument("-o", "--output", required=True, help="PSF2 file to write")
    ap.add_argument("--width", type=int, default=16, help="cell width in pixels")
    ap.add_argument("--height", type=int, default=32, help="cell height in pixels")
    ap.add_argument("-v", "--verbose", action="store_true")
    args = ap.parse_args()

    data, count = build_psf2(args.font, args.width, args.height, args.verbose)

    with open(args.output, "wb") as fh:
        fh.write(data)

    print(
        f"  Wrote {args.output}: {count} glyphs, "
        f"{args.width}x{args.height}, {len(data)} bytes"
    )


if __name__ == "__main__":
    main()
