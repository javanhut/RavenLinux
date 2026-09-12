//! The raven mark, from the bitmap `build.rs` decoded.
//!
//! One image, one size, straight RGBA. The mark used to be a polygon traced
//! by hand in `menu.rs`; it is now the same artwork every other part of Raven
//! Linux shows -- `branding/raven-logo-light.png` at the repository root --
//! which RavenBoot cannot read at run time and so carries in `.rodata`, the
//! way it carries its font.

use crate::gfx::Image;

include!(concat!(env!("OUT_DIR"), "/mark.rs"));

/// The mark, `MARK_PX` pixels square.
pub static MARK: Image = Image {
    width: MARK_PX,
    height: MARK_PX,
    rgba: include_bytes!(concat!(env!("OUT_DIR"), "/mark.bin")),
};
