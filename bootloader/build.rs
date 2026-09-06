//! Rasterizes the boot menu's font into an atlas at build time.
//!
//! RavenBoot is `#![no_std]` and runs before any filesystem it can trust, so it
//! cannot open a font file, and there is no rasterizer it could link if it did:
//! every one of them wants an allocator it doesn't have or a `std` it isn't.
//! What it can do is carry pre-rendered coverage bitmaps in its own `.rodata`,
//! which is what this produces.
//!
//! The font is `fonts/JetBrainsMonoNerdFontMono-Regular.ttf` — the same file
//! `stage2` installs for the console and `stage-gui` installs for the desktop.
//! Rendering the boot menu in the system's own typeface is most of the reason
//! this file exists; the alternative is the firmware's built-in 8x16 bitmap,
//! which is the look being replaced.
//!
//! # Why only one weight
//!
//! There is no bold face. `raven-greeter` has none either: its hierarchy comes
//! from size and colour — `TEXT` against `TEXT_DIM` — and not from weight. A
//! second weight would double this atlas to buy a distinction the design does
//! not use.
//!
//! # Output
//!
//! * `$OUT_DIR/atlas.bin` — every glyph's 8-bit coverage, tightly packed.
//! * `$OUT_DIR/atlas.rs` — the index, included by `src/font.rs`.

use std::fmt::Write as _;
use std::path::PathBuf;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};

/// The em sizes rasterized, in a roughly 1.18x geometric ladder.
///
/// `font::face_for` picks the nearest of these to the size a caller asks for,
/// and layout then measures against the face it actually got — so nothing
/// breaks if `theme.rs` changes a size, it just lands on a neighbouring rung.
/// That indirection is the point: the ladder is deliberately not derived from
/// the theme constants, because a generator that has to be edited in lockstep
/// with a constant it cannot see is a generator that will drift out of it.
///
/// The range spans every size the theme asks for at every scale `scale_for`
/// returns: `SMALL_SIZE` (12.5) at 1.0 sits at the bottom, `WORDMARK_SIZE` (30)
/// at 2.0 at the top. These are em sizes -- see `em_to_px_scale` below.
const SIZES: &[f32] = &[
    12.0, 14.0, 16.0, 19.0, 22.0, 26.0, 30.0, 36.0, 44.0, 52.0, 60.0,
];

/// Printable ASCII, `' '` through `'~'`.
///
/// Entry names come from `boot.cfg`, which is written by whoever installed the
/// system, so the covered range has to be a promise rather than a survey of
/// what the default config happens to say. ASCII is where that promise stops:
/// the full Nerd Font is some thousands of glyphs, and carrying them at eleven
/// sizes would be a larger `.efi` than the kernel it loads. `font::draw_text`
/// substitutes anything outside this range, so a non-ASCII entry name renders
/// as replacement characters rather than as a gap.
const FIRST: u32 = 0x20;
const LAST: u32 = 0x7E;

// `pub` so that `preview/build.rs`, which reaches this file through `#[path]`,
// can call it. Cargo runs it as a build script's `main` either way.
pub fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    // The fonts live at the repository root, not in this crate, because they
    // are the image's fonts and this crate is one of four things that installs
    // them. Found by walking up rather than by a fixed `../fonts`, because
    // `preview/` includes this file too and sits one directory deeper.
    let font_path = find_font(&manifest).unwrap_or_else(|| {
        panic!(
            "no fonts/JetBrainsMonoNerdFontMono-Regular.ttf above {}",
            manifest.display()
        )
    });
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", font_path.display());

    let data = std::fs::read(&font_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", font_path.display()));
    let font = FontRef::try_from_slice(&data).expect("not a usable TrueType font");

    let mut atlas: Vec<u8> = Vec::new();
    let mut faces = String::new();

    // `PxScale` is not the em size. ab_glyph scales so that ascent - descent
    // comes to the number given, which for JetBrains Mono's 1020/-300 over a
    // 1000-unit em makes a "14" render at 10.6px of em -- about a third small.
    // Everywhere else in Raven a size is the em size, because that is what CSS
    // and cosmic-text mean by one, and `theme.rs` copies the greeter's numbers
    // on the assumption that they mean the same thing. So convert.
    let em_to_px_scale =
        font.height_unscaled() / font.units_per_em().expect("font has no units_per_em");

    for &size in SIZES {
        let px = size * em_to_px_scale;
        let scaled = font.as_scaled(PxScale::from(px));

        // JetBrains Mono is monospaced, so one advance describes every glyph
        // and there is no reason to store it per glyph. Measured rather than
        // assumed: taking it from 'M' would be wrong for a font that isn't.
        let advance = scaled.h_advance(font.glyph_id('M'));

        let mut glyphs = String::new();
        for code in FIRST..=LAST {
            let ch = char::from_u32(code).unwrap();
            let glyph = font.glyph_id(ch).with_scale(PxScale::from(px));

            // A blank glyph — space — outlines to nothing. It still needs an
            // index entry, because lookup is by offset from FIRST and a missing
            // row would shift every glyph after it.
            let Some(outlined) = font.outline_glyph(glyph) else {
                let _ = writeln!(glyphs, "    G {{ off: 0, w: 0, h: 0, left: 0, top: 0 }},");
                continue;
            };

            let bounds = outlined.px_bounds();
            let w = bounds.width().ceil() as usize;
            let h = bounds.height().ceil() as usize;
            let offset = atlas.len();

            // Tightly packed: the ink's bounding box, not the em square. Most
            // glyphs are well under half the cell they sit in, and at these
            // sizes that difference is the bulk of the file.
            let mut bitmap = vec![0u8; w * h];
            outlined.draw(|x, y, coverage| {
                let (x, y) = (x as usize, y as usize);
                if x < w && y < h {
                    bitmap[y * w + x] = (coverage * 255.0).round().clamp(0.0, 255.0) as u8;
                }
            });
            atlas.extend_from_slice(&bitmap);

            // Relative to the glyph's origin on the baseline, y down — so
            // `top` is negative for anything above the baseline, which is
            // almost everything.
            let _ = writeln!(
                glyphs,
                "    G {{ off: {offset}, w: {w}, h: {h}, left: {}, top: {} }},",
                bounds.min.x.round() as i32,
                bounds.min.y.round() as i32,
            );
        }

        let _ = writeln!(
            faces,
            "    Face {{\n        px: {size:.1},\n        ascent: {:.3},\n        descent: {:.3},\n        line_height: {:.3},\n        advance: {advance:.3},\n        glyphs: &[\n{glyphs}        ],\n    }},",
            scaled.ascent(),
            // Stored as a positive drop below the baseline. ab_glyph reports it
            // negative, following the font's own sign convention, and every use
            // site here wants "how far down" -- a sign that has to be flipped at
            // each of them is a sign that gets forgotten at one of them.
            -scaled.descent(),
            scaled.height() + scaled.line_gap(),
        );
    }

    let index = format!(
        "// Generated by build.rs. Do not edit.\n\
         //\n\
         // {} sizes x {} glyphs, {} bytes of coverage.\n\
         \n\
         pub static ATLAS: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/atlas.bin\"));\n\
         \n\
         pub const FIRST_CHAR: u32 = {FIRST};\n\
         pub const LAST_CHAR: u32 = {LAST};\n\
         \n\
         pub static FACES: &[Face] = &[\n{faces}];\n",
        SIZES.len(),
        LAST - FIRST + 1,
        atlas.len(),
    );

    std::fs::write(out.join("atlas.bin"), &atlas).expect("cannot write atlas.bin");
    std::fs::write(out.join("atlas.rs"), index).expect("cannot write atlas.rs");
}

/// The repository's font, from anywhere below the repository root.
pub fn find_font(start: &std::path::Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(here) = dir {
        let candidate = here.join("fonts/JetBrainsMonoNerdFontMono-Regular.ttf");
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = here.parent();
    }
    None
}
