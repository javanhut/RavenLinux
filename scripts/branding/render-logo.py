#!/usr/bin/env python3
"""Derive every logo size in the tree from branding/raven-logo*.png.

Usage: scripts/branding/render-logo.py [--source PNG] [--out DIR]

With no arguments it rebuilds the two masters in branding/ from
branding/raven-logo-source.png and then writes the PNG ladder
(16..512) and the two SVG wrappers next to them, under branding/sizes/.
Standard library only: the ISO build container has no PIL.
"""
import argparse, base64, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from rlogo import read_png, write_png, bbox, crop, pad_square, resize, recolor

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BRANDING = os.path.join(ROOT, "branding")
SIZES = (16, 22, 24, 32, 48, 64, 96, 128, 256, 512)
MASTER = 512
MARGIN = 0.04          # breathing room inside the square, as a fraction of the mark
LIGHT_BODY = (0xC0, 0xCA, 0xF5)   # theme::TEXT, the Raven Glass foreground

def to_light(r, g, b, a):
    # The body is black; the wing is saturated purple. Anything near-neutral
    # is body and becomes the light foreground; the purple keeps its colour.
    return LIGHT_BODY if max(r, g, b) - min(r, g, b) < 48 else (r, g, b)

def svg(png_path, out_path):
    b64 = base64.b64encode(open(png_path, "rb").read()).decode()
    with open(out_path, "w") as f:
        f.write('<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" '
                f'width="128" height="128" viewBox="0 0 {MASTER} {MASTER}">'
                f'<image width="{MASTER}" height="{MASTER}" xlink:href="data:image/png;base64,{b64}"/></svg>\n')

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--source", default=os.path.join(BRANDING, "raven-logo-source.png"))
    ap.add_argument("--out", default=os.path.join(BRANDING, "sizes"))
    args = ap.parse_args()

    w, h, px = read_png(args.source)
    w, h, px = crop(w, h, px, *bbox(w, h, px))
    w, h, px = pad_square(w, h, px, MARGIN)
    w, h, dark = resize(w, h, px, MASTER, MASTER)
    _, _, light = recolor(w, h, dark, to_light)

    write_png(os.path.join(BRANDING, "raven-logo.png"), w, h, dark)
    write_png(os.path.join(BRANDING, "raven-logo-light.png"), w, h, light)
    svg(os.path.join(BRANDING, "raven-logo.png"), os.path.join(BRANDING, "raven-logo.svg"))
    svg(os.path.join(BRANDING, "raven-logo-light.png"), os.path.join(BRANDING, "raven-logo-light.svg"))

    os.makedirs(args.out, exist_ok=True)
    for name, src in (("raven-logo", dark), ("raven-logo-light", light)):
        for s in SIZES:
            write_png(os.path.join(args.out, f"{name}-{s}.png"), *resize(w, h, src, s, s))
    print(f"wrote masters to {BRANDING} and {2 * len(SIZES)} sizes to {args.out}")

if __name__ == "__main__":
    main()
