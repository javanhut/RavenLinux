//! The boot menu's one look, compiled in.
//!
//! Constants rather than a schema, for the reason huginn's and RavenLogin's
//! `theme.rs` both give: a format a user can write is a format that must not
//! change between releases. Here the argument is stronger than either, because
//! this file is read by firmware before any filesystem the user controls has
//! been mounted, and a boot menu that fails to parse is a machine that does
//! not boot.
//!
//! # Why these numbers are copied rather than shared
//!
//! [`ACCENT`], [`BACKDROP`], [`SURFACE`], [`BORDER`], [`TEXT`] and [`TEXT_DIM`]
//! are RavenLogin's, to the digit, which are huginn's. They are duplicated for
//! the reason RavenLogin's `theme.rs` sets out — huginn's `theme` module is
//! `pub(crate)` in another repository — plus one this crate adds: RavenBoot is
//! `#![no_std]`, links no Raven crate at all, and could not depend on a shared
//! `raven-theme` even if RavenGUI promoted one, unless that crate were itself
//! `no_std`.
//!
//! The seam this protects is the one the user actually sees. RavenBoot paints
//! [`BACKDROP`], hands the framebuffer to the kernel still holding it, and
//! `raven-greeter` paints the same [`BACKDROP`] over the top. If these three
//! numbers ever stop matching, the handover flashes.

/// A colour, as `0xAARRGGBB`.
///
/// Packed into one integer so it is `Copy` and comparable, and converted at the
/// edges. UEFI's `BltPixel` is BGRx bytes, which is the same order `wl_shm`'s
/// `Argb8888` gives little-endian — so the greeter and the boot menu are
/// unpacking the same constant the same way on the same hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(u32);

impl Color {
    #[must_use]
    pub const fn from_argb(argb: u32) -> Self {
        Self(argb)
    }

    #[must_use]
    pub const fn alpha(self) -> u8 {
        (self.0 >> 24) as u8
    }

    #[must_use]
    pub const fn red(self) -> u8 {
        (self.0 >> 16) as u8
    }

    #[must_use]
    pub const fn green(self) -> u8 {
        (self.0 >> 8) as u8
    }

    #[must_use]
    pub const fn blue(self) -> u8 {
        self.0 as u8
    }

    /// The same colour at a different opacity.
    #[must_use]
    pub const fn with_alpha(self, alpha: u8) -> Self {
        Self((self.0 & 0x00FF_FFFF) | ((alpha as u32) << 24))
    }
}

// ---------------------------------------------------------------------------
// Colour
// ---------------------------------------------------------------------------

/// The whole screen behind everything. huginn's `BACKGROUND`, RavenLogin's
/// `BACKDROP`.
///
/// This is the single most important constant in the file. It is what the
/// framebuffer still holds when the kernel takes it over, so it is the colour
/// of the entire boot, from the firmware handing off to RavenBoot until the
/// greeter's first frame.
pub const BACKDROP: Color = Color::from_argb(0xFF16_161F);

/// The darker vignette the backdrop falls off to at the edges.
///
/// The gradient is very shallow on purpose. It exists to stop a large flat
/// panel showing its own banding, not to be seen as a gradient.
pub const BACKDROP_EDGE: Color = Color::from_argb(0xFF0D_0D14);

/// The card the menu sits on.
pub const SURFACE: Color = Color::from_argb(0xFF1A_1B26);

/// Hairline borders.
pub const BORDER: Color = Color::from_argb(0xFF2A_2E45);

/// Focus rings, the selected row's caret, the wordmark's initial.
pub const ACCENT: Color = Color::from_argb(0xFF7A_A2F7);

/// The wash behind the selected row.
///
/// The accent at 10%, composited onto [`SURFACE`] rather than replacing it. A
/// solid accent fill would make the selected entry the brightest thing on the
/// screen and the wordmark the second — which is backwards, since the
/// selection moves and the wordmark does not. The ring carries the selection;
/// this only warms the row underneath it.
pub const ROW_SELECTED: Color = ACCENT.with_alpha(0x1A);

/// Ordinary text: entry names, the wordmark.
pub const TEXT: Color = Color::from_argb(0xFFC0_CAF5);

/// Secondary text: the prompt, the key hints, the version.
pub const TEXT_DIM: Color = Color::from_argb(0xFF56_5F89);

/// A failed boot.
pub const ERROR: Color = Color::from_argb(0xFFF7_768E);

/// The auto-boot countdown, and anything else that is a caution rather than a
/// failure.
pub const WARNING: Color = Color::from_argb(0xFFE0_AF68);

// ---------------------------------------------------------------------------
// Metric
// ---------------------------------------------------------------------------
//
// Everything below is in logical pixels against a 1080p panel, and is scaled by
// `gfx::Screen::scale` before use — so the menu is the same physical size on a
// 4K laptop panel as on a 1080p monitor, which is the same rule the greeter
// follows for the same reason.

/// The menu card's width.
///
/// Wider than the greeter's 380pt card because the content is different: a
/// login prompt holds one field and a name, and a boot menu holds entry names
/// written by whoever wrote `boot.cfg`. At 520 an entry can say "Raven Linux
/// (Recovery, Serial Console)" without eliding.
pub const CARD_WIDTH: f32 = 520.0;

/// The card's corner radius, and the padding inside it.
pub const CARD_RADIUS: f32 = 16.0;
pub const CARD_PADDING: f32 = 16.0;

/// One menu row. `ROW_HEIGHT` and `ROW_RADIUS` are the greeter's `FIELD_HEIGHT`
/// and `FIELD_RADIUS`: a row the user moves a selection through and a field the
/// user types into are the same control at different moments of the same boot.
pub const ROW_HEIGHT: f32 = 44.0;
pub const ROW_RADIUS: f32 = 10.0;
pub const ROW_GAP: f32 = 2.0;

/// The selection ring. huginn's `FOCUS_RING_WIDTH`.
pub const FOCUS_RING_WIDTH: f32 = 2.0;

/// The gutter between the card's edge and a row's text, which is where the
/// caret sits on the selected row.
pub const ROW_TEXT_INSET: f32 = 34.0;

/// The raven mark above the wordmark, and the gap under it.
pub const MARK_SIZE: f32 = 56.0;
pub const MARK_GAP: f32 = 14.0;

/// Text sizes. `BODY_SIZE` and `SMALL_SIZE` are the greeter's.
pub const WORDMARK_SIZE: f32 = 30.0;
pub const PROMPT_SIZE: f32 = 14.0;
pub const BODY_SIZE: f32 = 15.0;
pub const SMALL_SIZE: f32 = 12.5;

/// The gap between the wordmark and the prompt, and between the prompt and the
/// card.
pub const HEADER_GAP: f32 = 10.0;
pub const PROMPT_GAP: f32 = 22.0;

/// The footer's rule and the space around it.
pub const FOOTER_GAP: f32 = 18.0;
pub const FOOTER_RULE_GAP: f32 = 12.0;
