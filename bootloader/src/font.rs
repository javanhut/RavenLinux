//! Text, from the atlas `build.rs` rasterized.
//!
//! There is no shaping and no kerning here, and for this font there is nothing
//! to shape: JetBrains Mono is monospaced, so a run of ASCII is laid out by
//! advancing a fixed step per character. That is the whole layout engine, and
//! it is exact rather than approximate — the same positions a real shaper would
//! produce for this font.
//!
//! What is *not* skipped is anti-aliasing. Each glyph is 8-bit coverage,
//! composited through [`Canvas::blend`] exactly like every shape in `gfx`, so
//! text and rounded rectangles have the same edges. The text-mode menu this
//! replaces had no choice about that; here it is free.

use crate::gfx::{round, Canvas};
use crate::theme::Color;

/// One glyph's ink, as a window into [`ATLAS`].
#[derive(Clone, Copy)]
pub struct G {
    pub off: usize,
    pub w: usize,
    pub h: usize,
    /// Ink origin relative to the pen, y down. `top` is negative above the
    /// baseline, which is where nearly all of it is.
    pub left: i32,
    pub top: i32,
}

/// The font at one rasterized size.
pub struct Face {
    pub px: f32,
    /// Baseline to the top of the tallest glyph.
    pub ascent: f32,
    /// Baseline to the bottom of the deepest glyph, as a positive drop — see
    /// `build.rs`, which flips ab_glyph's sign to get it.
    pub descent: f32,
    pub line_height: f32,
    /// One advance for every glyph — see `build.rs` on why this is per-face.
    pub advance: f32,
    pub glyphs: &'static [G],
}

include!(concat!(env!("OUT_DIR"), "/atlas.rs"));

impl Face {
    /// The width of `text`, in pixels.
    #[must_use]
    pub fn measure(&self, text: &str) -> f32 {
        // `chars().count()` and not `len()`: a multi-byte character is one
        // cell, drawn as one replacement glyph, not two or three.
        text.chars().count() as f32 * self.advance
    }

    /// The glyph for `ch`, or the replacement for anything outside the atlas.
    ///
    /// Out-of-range characters become `?` rather than nothing. A boot entry
    /// named in a language this atlas does not cover should look wrong, not
    /// look empty: an empty row is indistinguishable from a bug in the menu,
    /// and the user still has to be able to count the rows to reach the one
    /// they want.
    fn glyph(&self, ch: char) -> Option<&G> {
        let code = ch as u32;
        let code = if (FIRST_CHAR..=LAST_CHAR).contains(&code) {
            code
        } else {
            '?' as u32
        };
        self.glyphs.get((code - FIRST_CHAR) as usize)
    }
}

/// The rasterized size nearest to `px`.
///
/// Nearest rather than next-smallest: half a pixel of scale error is invisible,
/// and always rounding down would make every string on a 1440p panel one rung
/// too small. Callers measure against the [`Face`] they get back, never against
/// the size they asked for, so landing on a neighbouring rung shifts the layout
/// with the text instead of against it.
#[must_use]
pub fn face_for(px: f32) -> &'static Face {
    let mut best = &FACES[0];
    let mut best_error = f32::INFINITY;
    for face in FACES {
        let error = crate::gfx::fabs(face.px - px);
        if error < best_error {
            best_error = error;
            best = face;
        }
    }
    best
}

/// Draw `text` with its left edge at `x` and its baseline at `baseline`.
///
/// Positions are rounded to whole pixels before the glyph is composited. The
/// atlas is rasterized on the pixel grid, so blending it at a fractional offset
/// would resample already-anti-aliased coverage and soften every stem — the
/// classic reason bitmap text looks muddy next to vector text.
pub fn draw(canvas: &mut Canvas, face: &Face, x: f32, baseline: f32, text: &str, color: Color) {
    let mut pen = x;
    for ch in text.chars() {
        if let Some(glyph) = face.glyph(ch) {
            blit(canvas, glyph, round(pen), round(baseline), color);
        }
        pen += face.advance;
    }
}

/// Draw `text` centred on `cx`.
pub fn draw_centered(
    canvas: &mut Canvas,
    face: &Face,
    cx: f32,
    baseline: f32,
    text: &str,
    color: Color,
) {
    draw(
        canvas,
        face,
        cx - face.measure(text) / 2.0,
        baseline,
        text,
        color,
    );
}

/// Draw `text` with letters spread by an extra `tracking` pixels each.
///
/// Only the wordmark uses this. Letterspacing a monospaced face is normally a
/// mistake — the face is already evenly spaced — but at display size a short
/// all-caps word reads as a mark rather than as a word, and tracking is what
/// makes the difference.
pub fn draw_tracked(
    canvas: &mut Canvas,
    face: &Face,
    x: f32,
    baseline: f32,
    text: &str,
    tracking: f32,
    color: Color,
) {
    let mut pen = x;
    for ch in text.chars() {
        if let Some(glyph) = face.glyph(ch) {
            blit(canvas, glyph, round(pen), round(baseline), color);
        }
        pen += face.advance + tracking;
    }
}

/// The width [`draw_tracked`] will occupy.
///
/// The trailing letter contributes no tracking, which is what keeps a centred
/// wordmark actually centred rather than sitting half a gap to the left.
#[must_use]
pub fn measure_tracked(face: &Face, text: &str, tracking: f32) -> f32 {
    let count = text.chars().count();
    if count == 0 {
        return 0.0;
    }
    count as f32 * face.advance + (count - 1) as f32 * tracking
}

/// Composite one glyph's coverage at a pen position.
fn blit(canvas: &mut Canvas, glyph: &G, pen_x: f32, baseline: f32, color: Color) {
    if glyph.w == 0 || glyph.h == 0 {
        return;
    }
    let Some(bitmap) = ATLAS.get(glyph.off..glyph.off + glyph.w * glyph.h) else {
        return;
    };

    let origin_x = pen_x as i32 + glyph.left;
    let origin_y = baseline as i32 + glyph.top;

    for row in 0..glyph.h {
        for col in 0..glyph.w {
            let coverage = bitmap[row * glyph.w + col];
            if coverage == 0 {
                continue;
            }
            canvas.blend(
                origin_x + col as i32,
                origin_y + row as i32,
                color,
                f32::from(coverage) / 255.0,
            );
        }
    }
}
