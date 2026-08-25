# Fonts

RavenLinux ships one family: **JetBrains Mono Nerd Font Mono**, as the four
`.ttf` files in this directory.

Put the exact `.ttf` or `.otf` files you want in the ISO here. The build copies
everything in this directory into `/usr/share/fonts` and nothing from the build
host, so this is the whole font set the image gets.

## The two things a font has to be

A Linux system needs the same typeface in two incompatible formats, and the
build produces both from the files here.

| Where | Format | Comes from |
|-------|--------|------------|
| tty1, the console | PSF2 bitmap, one fixed cell per glyph | generated at build time by `scripts/make-console-font.py` |
| the Wayland session | the `.ttf` itself, via fontconfig | copied straight to `/usr/share/fonts` |

The virtual terminal cannot read a TTF. It wants PSF: a bitmap of a fixed size
for every glyph, plus a table saying which code points map to which bitmap. So
stage4 rasterises `JetBrainsMonoNerdFontMono-Regular.ttf` into
`/usr/share/kbd/consolefonts/raven-<W>x<H>.psfu` at four cell sizes, and
`raven-console-font` loads one at boot.

Without that step the console runs on the kernel's built-in 8x16 VGA font: no
Nerd Font glyphs at all, box drawing that does not join up, and text about two
millimetres tall on a HiDPI laptop panel.

## Why the *Mono* variant

Nerd Fonts come in three widths. The `Mono` files are the ones whose icons are
scaled down to a single character cell. The others draw icons one and a half or
two cells wide, which a terminal has no way to represent and a PSF font cannot
express at all — every glyph in a PSF is exactly one cell.

If you replace these files, use the `NerdFontMono` variants.

## The 512-glyph budget

The console holds at most 512 glyphs, so `make-console-font.py` ships a curated
subset rather than the font's several thousand: ASCII, Latin-1, all of box
drawing and block elements, and about sixty Nerd Font icons. The list is
`GLYPH_SET` at the top of that script; add to it and rebuild if something you
need is missing.

Above 256 glyphs the console repurposes the intensity bit to index them, which
costs bright *background* colours. Foreground colours are unaffected. That is
the standard trade and the only way to fit box drawing and icons in one font.

## Changing the console font

At runtime, without rebuilding:

```bash
setfont /usr/share/kbd/consolefonts/raven-12x24.psfu
```

Permanently, in order of precedence:

```bash
raven.font=12x24                     # on the kernel command line
raven.font=none                      # leave the kernel's own font alone
echo 'FONT=12x24' > /etc/raven/console-font.conf
```

With none of those set, `raven-console-font` picks a cell size from the
framebuffer width: 16x32 at 2560px and above, then 12x24, 10x20, and 8x16 for
anything under 1280px or a VGA text console.

To build one by hand:

```bash
./scripts/make-console-font.py fonts/JetBrainsMonoNerdFontMono-Regular.ttf \
    --width 16 --height 32 -o raven-16x32.psfu -v
```

It needs `python-freetype-py`. The build is fail-soft without it: you get a
working ISO whose console runs on the kernel font.
