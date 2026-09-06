//! Software drawing into a pixel buffer.
//!
//! This is `raven-ui`'s `canvas.rs` ported to `no_std`, and deliberately so:
//! the boot menu and the login screen are two full-screen surfaces of the same
//! colour separated by a kernel, and they should be drawn by the same code
//! doing the same thing. The distance fields, the one-pixel coverage ramp and
//! the Bayer-dithered gradient below are all that module's, with the
//! `std`-only pieces replaced.
//!
//! What had to change, and why:
//!
//! * `f32::sqrt`, `f32::abs` and `f32::floor` live in `std`, not `core`. They
//!   come from `libm` here, which is the same implementation `std` uses on
//!   targets without a hardware intrinsic.
//! * The greeter borrows a `wl_shm` buffer from a compositor. There is no
//!   compositor here, so [`Canvas`] owns its pixels and [`crate::screen`]
//!   pushes them to the firmware.
//!
//! Nothing in this module mentions UEFI. That is what lets the whole of it,
//! plus `font.rs` and the drawing half of `menu.rs`, be compiled for the host
//! and rendered to a PNG — see `preview/` — which is the only way to look at a
//! boot menu without booting.

use crate::theme::Color;

/// One pixel, BGRx.
///
/// The channel order is UEFI's [`BltPixel`](uefi::proto::console::gop::BltPixel)
/// and the layout is `#[repr(C)]` to match it, so [`crate::screen::Screen`] can
/// hand a canvas straight to the firmware without converting a few million
/// pixels first. It is also, not coincidentally, the order `wl_shm`'s
/// `Argb8888` gives on a little-endian machine — the greeter is unpacking the
/// same bytes the same way.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pixel {
    pub blue: u8,
    pub green: u8,
    pub red: u8,
    pub reserved: u8,
}

impl Pixel {
    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self {
            blue,
            green,
            red,
            reserved: 0,
        }
    }
}

/// A rectangle in screen coordinates, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    #[must_use]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// An owned back buffer, with drawing operations on top.
///
/// Every frame is composed here in full and then handed to the firmware in one
/// go. Drawing straight to the panel would tear visibly on the redraw that
/// follows each keypress, and a boot menu that flickers when you press a key
/// reads as broken even when it is not.
pub struct Canvas {
    pixels: alloc::vec::Vec<Pixel>,
    width: i32,
    height: i32,
}

impl Canvas {
    /// A canvas of `width` x `height`, filled with opaque black.
    #[must_use]
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            pixels: alloc::vec![Pixel::new(0, 0, 0); width * height],
            width: width as i32,
            height: height as i32,
        }
    }

    #[must_use]
    pub fn width(&self) -> f32 {
        self.width as f32
    }

    #[must_use]
    pub fn height(&self) -> f32 {
        self.height as f32
    }

    #[must_use]
    pub fn pixels(&self) -> &[Pixel] {
        &self.pixels
    }

    /// Composite one pixel, source-over.
    ///
    /// Out-of-bounds coordinates are dropped rather than clamped. A shape
    /// partly off the edge should be clipped, not smeared along it, and the
    /// callers below all rely on being able to iterate a bounding box that may
    /// hang off the screen.
    pub fn blend(&mut self, x: i32, y: i32, color: Color, coverage: f32) {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return;
        }
        let alpha = f32::from(color.alpha()) / 255.0 * clamp01(coverage);
        if alpha <= 0.0 {
            return;
        }

        let index = (y * self.width + x) as usize;
        let Some(pixel) = self.pixels.get_mut(index) else {
            return;
        };

        let mix = |dst: u8, src: u8| -> u8 {
            (f32::from(src) * alpha + f32::from(dst) * (1.0 - alpha)) as u8
        };
        pixel.red = mix(pixel.red, color.red());
        pixel.green = mix(pixel.green, color.green());
        pixel.blue = mix(pixel.blue, color.blue());
    }

    /// Fill the whole canvas with a vertical gradient, `top` to `bottom`.
    ///
    /// Written directly rather than through [`Self::blend`], because this is
    /// the one operation that touches every pixel and it has no coverage or
    /// alpha to consider — it is the ground everything else is composited onto.
    ///
    /// # Dithering
    ///
    /// The backdrop runs from `0x16161F` to `0x0D0D14`: nine values of red
    /// across the whole height of the screen. Rounded to the nearest byte that
    /// is nine flat bands with hard edges, and on a large dark panel those
    /// edges are clearly visible — the gradient meant to *prevent* banding
    /// produces it instead. It shows up worse here than in the greeter, because
    /// a boot menu is often the first thing on a panel that has just woken up.
    ///
    /// So the channel value is perturbed by a 4x4 ordered (Bayer) threshold
    /// before it is truncated, which trades the hard edges for a stipple far
    /// below the noise floor of any real panel.
    pub fn gradient(&mut self, top: Color, bottom: Color) {
        #[rustfmt::skip]
        const BAYER: [[f32; 4]; 4] = [
            [ 0.0 / 16.0,  8.0 / 16.0,  2.0 / 16.0, 10.0 / 16.0],
            [12.0 / 16.0,  4.0 / 16.0, 14.0 / 16.0,  6.0 / 16.0],
            [ 3.0 / 16.0, 11.0 / 16.0,  1.0 / 16.0,  9.0 / 16.0],
            [15.0 / 16.0,  7.0 / 16.0, 13.0 / 16.0,  5.0 / 16.0],
        ];

        let height = (self.height.max(1) - 1).max(1) as f32;
        for y in 0..self.height {
            let t = y as f32 / height;
            let lerp = |a: u8, b: u8| -> f32 { f32::from(a) + (f32::from(b) - f32::from(a)) * t };
            let (bf, gf, rf) = (
                lerp(top.blue(), bottom.blue()),
                lerp(top.green(), bottom.green()),
                lerp(top.red(), bottom.red()),
            );

            let row = (y * self.width) as usize;
            let bayer_row = &BAYER[(y & 3) as usize];
            for x in 0..self.width as usize {
                let threshold = bayer_row[x & 3] - 0.5;
                let quantize = |v: f32| clamp(v + threshold, 0.0, 255.0) as u8;

                let Some(pixel) = self.pixels.get_mut(row + x) else {
                    continue;
                };
                pixel.blue = quantize(bf);
                pixel.green = quantize(gf);
                pixel.red = quantize(rf);
            }
        }
    }

    /// Replace every pixel with one flat colour.
    ///
    /// This is what the framebuffer is left holding when the kernel takes it
    /// over — see [`crate::screen::Screen::hand_off`]. Flat rather than the
    /// gradient, because the kernel's `efifb` inherits a rectangle of memory
    /// and nothing that describes it: whatever is in there stays on the panel
    /// through the whole of early boot, and a gradient would be visibly wrong
    /// the moment anything drew over part of it.
    pub fn fill(&mut self, color: Color) {
        self.pixels
            .fill(Pixel::new(color.red(), color.green(), color.blue()));
    }

    /// A filled rounded rectangle.
    ///
    /// `radius` is clamped to half the shorter side, so a "rounded rectangle"
    /// with an absurd radius becomes a capsule rather than folding inside out.
    pub fn rounded_rect(&mut self, rect: Rect, radius: f32, color: Color) {
        self.rounded_rect_impl(rect, radius, color, None);
    }

    /// The outline of a rounded rectangle, `thickness` wide, centred on the
    /// rectangle's edge.
    pub fn rounded_rect_outline(&mut self, rect: Rect, radius: f32, thickness: f32, color: Color) {
        self.rounded_rect_impl(rect, radius, color, Some(thickness.max(0.1)));
    }

    fn rounded_rect_impl(&mut self, rect: Rect, radius: f32, color: Color, outline: Option<f32>) {
        let radius = fmin(fmin(radius, rect.width / 2.0), rect.height / 2.0).max(0.0);

        // The distance field is evaluated about the rectangle's centre, so the
        // half-extents are what the formula needs.
        let (cx, cy) = (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
        let (hx, hy) = (rect.width / 2.0 - radius, rect.height / 2.0 - radius);

        // One pixel of margin so the anti-aliased edge is not clipped.
        for y in bounds(rect.y - 1.0, rect.y + rect.height + 1.0, self.height) {
            for x in bounds(rect.x - 1.0, rect.x + rect.width + 1.0, self.width) {
                // Pixel centre, not corner: sampling at the corner shifts every
                // shape half a pixel up and left.
                let px = x as f32 + 0.5 - cx;
                let py = y as f32 + 0.5 - cy;

                // Distance to the rounded rectangle's boundary. Negative
                // inside, positive outside.
                let dx = fabs(px) - hx;
                let dy = fabs(py) - hy;
                let (ox, oy) = (dx.max(0.0), dy.max(0.0));
                let outside = sqrt(ox * ox + oy * oy);
                let inside = fmin(dx.max(dy), 0.0);
                let distance = outside + inside - radius;

                let coverage = match outline {
                    // A ring: coverage falls off on both sides of the edge.
                    Some(thickness) => coverage_of(fabs(distance) - thickness / 2.0),
                    None => coverage_of(distance),
                };
                self.blend(x, y, color, coverage);
            }
        }
    }

    /// A filled circle.
    pub fn circle(&mut self, cx: f32, cy: f32, radius: f32, color: Color) {
        for y in bounds(cy - radius - 1.0, cy + radius + 1.0, self.height) {
            for x in bounds(cx - radius - 1.0, cx + radius + 1.0, self.width) {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                let distance = sqrt(dx * dx + dy * dy) - radius;
                self.blend(x, y, color, coverage_of(distance));
            }
        }
    }

    /// A horizontal hairline.
    pub fn rule(&mut self, x: f32, y: f32, width: f32, thickness: f32, color: Color) {
        self.rounded_rect(Rect::new(x, y, width, thickness.max(1.0)), 0.0, color);
    }

    /// A filled polygon, non-zero winding.
    ///
    /// The one shape here with no closed-form distance field, so it is
    /// anti-aliased by supersampling instead: coverage is the fraction of a 4x4
    /// grid of sample points inside the outline. Sixteen samples is four bits
    /// of coverage, which is under the one-pixel ramp everything else gets and
    /// is not enough for text — but the only polygon RavenBoot draws is the
    /// raven mark, once per frame, on a menu that redraws about once a second.
    ///
    /// The alternative, an exact scanline rasterizer with analytic coverage, is
    /// a few hundred lines to make one 56-pixel glyph very slightly crisper.
    pub fn polygon(&mut self, points: &[(f32, f32)], color: Color) {
        const GRID: i32 = 4;

        if points.len() < 3 {
            return;
        }

        let (mut min_x, mut min_y) = (f32::INFINITY, f32::INFINITY);
        let (mut max_x, mut max_y) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
        for &(x, y) in points {
            min_x = fmin(min_x, x);
            min_y = fmin(min_y, y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }

        for y in bounds(min_y - 1.0, max_y + 1.0, self.height) {
            for x in bounds(min_x - 1.0, max_x + 1.0, self.width) {
                let mut hits = 0;
                for sy in 0..GRID {
                    for sx in 0..GRID {
                        // Sample at the centre of each sub-cell, so the grid is
                        // symmetric about the pixel centre.
                        let px = x as f32 + (sx as f32 + 0.5) / GRID as f32;
                        let py = y as f32 + (sy as f32 + 0.5) / GRID as f32;
                        if contains(points, px, py) {
                            hits += 1;
                        }
                    }
                }
                if hits > 0 {
                    self.blend(x, y, color, hits as f32 / (GRID * GRID) as f32);
                }
            }
        }
    }
}

/// Whether `(px, py)` is inside the polygon, by crossing number.
fn contains(points: &[(f32, f32)], px: f32, py: f32) -> bool {
    let mut inside = false;
    let mut j = points.len() - 1;
    for i in 0..points.len() {
        let (xi, yi) = points[i];
        let (xj, yj) = points[j];
        if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Coverage for a pixel whose centre is `distance` from an edge.
///
/// The one-pixel linear ramp either side of the boundary is what makes the
/// edges smooth. A step function here is the difference between this looking
/// drawn and looking rasterized — which is exactly the difference between this
/// menu and the text-mode one it replaces.
fn coverage_of(distance: f32) -> f32 {
    clamp01(0.5 - distance)
}

/// The pixel rows or columns a shape can touch, clipped to the canvas.
fn bounds(low: f32, high: f32, limit: i32) -> core::ops::Range<i32> {
    let start = (floor(low) as i32).max(0);
    let end = (ceil(high) as i32).min(limit);
    start..end.max(start)
}

// ---------------------------------------------------------------------------
// Maths
// ---------------------------------------------------------------------------
//
// `core` has no floating-point maths at all — every one of these is a `std`
// method on `f32`. `libm` is the same code `std` compiles in on a target with
// no hardware intrinsic, so these are not approximations of the greeter's
// arithmetic, they are the identical arithmetic.

pub fn sqrt(v: f32) -> f32 {
    libm::sqrtf(v)
}

pub fn fabs(v: f32) -> f32 {
    libm::fabsf(v)
}

pub fn floor(v: f32) -> f32 {
    libm::floorf(v)
}

pub fn ceil(v: f32) -> f32 {
    libm::ceilf(v)
}

pub fn round(v: f32) -> f32 {
    libm::roundf(v)
}

/// `f32::min`, which is also `std`-only.
pub fn fmin(a: f32, b: f32) -> f32 {
    if a < b {
        a
    } else {
        b
    }
}

pub fn clamp(v: f32, low: f32, high: f32) -> f32 {
    if v < low {
        low
    } else if v > high {
        high
    } else {
        v
    }
}

pub fn clamp01(v: f32) -> f32 {
    clamp(v, 0.0, 1.0)
}
