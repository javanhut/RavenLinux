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
    # ASCII is not listed here: build_psf2 places it at its VGA-identity
    # positions (glyph index == codepoint) before this list is consulted.
    #
    # The characters a TUI draws itself with come before Latin-1: after the
    # 127 slots the fixed layout spends, declaration order decides what
    # survives, and a banner with no box borders is more visible than a
    # missing accented letter.
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
    (
        "punctuation",
        [
            0x2010, 0x2013, 0x2014, 0x2018, 0x2019, 0x201C, 0x201D,
            0x2022, 0x2026, 0x2030, 0x2039, 0x203A, 0x20AC, 0x2122,
        ],
    ),
    # Letters before the symbol rows: an accented name renders, and the lost
    # currency/ornament signs fall back to '?' harmlessly.
    ("Latin-1 letters", range(0xC0, 0x100)),
    (
        "Latin-1 symbols (curated)",
        [0xA1, 0xA3, 0xA7, 0xA9, 0xAB, 0xAE, 0xB0, 0xB1, 0xBB, 0xBF],
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

    row_bytes = (cell_w + 7) // 8
    charsize = row_bytes * cell_h
    blank = bytes(charsize)

    glyphs = []
    mapped = []          # one list of codepoints per glyph (PSF allows several)
    missing = []

    # ------------------------------------------------------------------
    # The fixed layout: glyph index == codepoint for 0x00-0x7E.
    # ------------------------------------------------------------------
    # Not a convention for its own sake. The kernel's erase character is
    # (attr << 8) | 0x20 -- a *raw glyph index*, written into the screen
    # buffer by scr_memsetw and never passed through the font's unicode
    # table. Whatever bitmap sits at index 32 is what every cleared cell
    # on the console displays. This font used to pack glyphs densely from
    # U+0020 at index 0, which put '@' at index 32 -- and every blank
    # cell on screen rendered as '@'.
    #
    # Indices 0-31 are blank fillers mapped to the C0 controls, so a stray
    # NUL renders as nothing rather than as whatever happened to be there.
    for cp in range(0x20):
        glyphs.append(blank)
        mapped.append([cp])

    for cp in range(0x20, 0x7F):
        bitmap = render_glyph(face, cp, cell_w, cell_h, baseline, hscale)
        if bitmap is None:
            # Keep the position honest even if the face lacks the char:
            # a hole here would shift everything after it off-index.
            bitmap = blank
            missing.append(cp)
        glyphs.append(bitmap)
        mapped.append([cp])

    # NBSP shares space's glyph: same appearance, zero slots. Without a
    # mapping it would fall back to '?' in any UTF-8 text that uses it.
    mapped[0x20].append(0x00A0)

    # ------------------------------------------------------------------
    # Everything else competes for the remaining slots in GLYPH_SET order.
    # ------------------------------------------------------------------
    placed = {cp for cps in mapped for cp in cps}
    for cp in codepoints:
        if cp in placed:
            continue
        if len(glyphs) >= PSF2_MAX_GLYPHS:
            missing.append(cp)
            continue
        bitmap = render_glyph(face, cp, cell_w, cell_h, baseline, hscale)
        if bitmap is None:
            missing.append(cp)
            continue
        glyphs.append(bitmap)
        mapped.append([cp])

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
    for cps in mapped:
        for cp in cps:
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
