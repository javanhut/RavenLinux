//! The panel: acquiring it from the firmware, and handing it to the kernel.
//!
//! Everything UEFI-specific about drawing lives here, so that [`crate::gfx`],
//! [`crate::font`] and the drawing half of [`crate::menu`] stay compilable for
//! the host and can be rendered to a PNG by `preview/`.
//!
//! # Why `blt` rather than the raw framebuffer
//!
//! [`GraphicsOutput`] exposes both a linear framebuffer and a `blt` operation.
//! The framebuffer is faster and looks tempting, but its pixel layout varies:
//! `PixelFormat::Rgb`, `Bgr`, an arbitrary channel `Bitmask`, and `BltOnly`,
//! which has no addressable framebuffer at all. Firmware reporting `BltOnly` is
//! rare on a physical machine and common on a virtual one.
//!
//! `blt` is defined for every one of those, because converting into the panel's
//! actual layout is the firmware's job. One full-screen `BufferToVideo` is a
//! few megabytes of copy on a menu that redraws about once a second, which is
//! not a price worth a correctness hole that only appears on hardware I cannot
//! test.

use uefi::prelude::*;
use uefi::proto::console::gop::{BltOp, BltPixel, BltRegion, GraphicsOutput};

use crate::gfx::{Canvas, Pixel};

/// [`Pixel`] and [`BltPixel`] must stay layout-compatible for [`Screen::present`]
/// to hand a canvas to the firmware without copying it.
///
/// Both are `#[repr(C)]` and both are `{ blue, green, red, _reserved }`, so
/// this is a check that the *sizes* still agree — which is what would change if
/// a future `uefi` widened the type. The field order is asserted by the
/// comment on [`Pixel`] and by the picture on the screen.
const _: () = assert!(core::mem::size_of::<Pixel>() == core::mem::size_of::<BltPixel>());

/// The panel, and the back buffer drawn for it.
///
/// Holds no borrow of the [`GraphicsOutput`] protocol. It is reopened for each
/// present, because an exclusive open cannot be held across the
/// `exit_boot_services` that booting an entry performs.
pub struct Screen {
    pub canvas: Canvas,
    width: usize,
    height: usize,
    scale: f32,
}

impl Screen {
    /// Open GOP and set up a back buffer for the mode the firmware is in.
    ///
    /// Returns `None` when the firmware exposes no GOP at all, which is the
    /// signal for `main` to fall back to the text console. That is rare on
    /// anything with a screen — GOP has been in the UEFI spec since 2.0 — but
    /// `-nographic` QEMU is exactly that case, and it is the one a developer
    /// hits most often.
    ///
    /// The firmware's current mode is used as-is rather than searched for a
    /// "best" one. Whatever the firmware picked is the mode the panel is
    /// already scanning out, the mode its own setup screens were drawn at, and
    /// the mode the kernel is about to inherit — so changing it would buy a
    /// slightly larger menu at the cost of a mode change the user sees as a
    /// black flash, twice.
    pub fn open(boot_services: &BootServices) -> Option<Self> {
        let handle = boot_services
            .get_handle_for_protocol::<GraphicsOutput>()
            .ok()?;
        let gop = boot_services
            .open_protocol_exclusive::<GraphicsOutput>(handle)
            .ok()?;

        let (width, height) = gop.current_mode_info().resolution();
        if width == 0 || height == 0 {
            return None;
        }

        // Close the exclusive open before the trial present below reopens it.
        drop(gop);

        let mut screen = Self {
            canvas: Canvas::new(width, height),
            width,
            height,
            scale: scale_for(height),
        };

        // Prove the panel can actually be drawn to before committing to it.
        //
        // `present` reopens GOP for every frame rather than holding it across
        // the `exit_boot_services` that booting performs, so the open that
        // succeeded above is not the open that matters. If the reopen fails --
        // firmware that will not hand the protocol back, a console driver that
        // has claimed it -- then every frame silently draws nothing, and the
        // user gets a blank screen with no menu and no way to reach one.
        //
        // Failing here instead turns that into the text menu, which is ugly but
        // is a menu. The trial paints the backdrop, so a success also leaves
        // the panel in the right colour a frame earlier than it otherwise would.
        screen.canvas.fill(crate::theme::BACKDROP);
        if !screen.present(boot_services) {
            return None;
        }

        Some(screen)
    }

    /// The logical-to-physical scale for this panel, which `menu::draw`
    /// multiplies every metric in [`crate::theme`] by. See [`scale_for`].
    #[must_use]
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Push the back buffer to the panel. Returns whether it drew.
    ///
    /// Only [`Self::open`] looks at the result, to decide whether this panel is
    /// usable at all. Once the menu is running there is nothing useful to do
    /// with a failure — the text console it would be reported on is the one the
    /// menu is replacing — and the next keypress redraws anyway.
    pub fn present(&self, boot_services: &BootServices) -> bool {
        let Ok(handle) = boot_services.get_handle_for_protocol::<GraphicsOutput>() else {
            return false;
        };
        let Ok(mut gop) = boot_services.open_protocol_exclusive::<GraphicsOutput>(handle) else {
            return false;
        };

        // Guard against the firmware having changed mode underneath us, which
        // some do when a display is hotplugged. Blitting a buffer sized for the
        // old mode is a firmware-side out-of-bounds read in the best case.
        if gop.current_mode_info().resolution() != (self.width, self.height) {
            return false;
        }

        // SAFETY: `Pixel` and `BltPixel` are both `#[repr(C)]` structs of four
        // `u8` in the order blue, green, red, reserved; the size equality is
        // asserted at compile time above. The slice is only read.
        let buffer = unsafe {
            core::slice::from_raw_parts(
                self.canvas.pixels().as_ptr().cast::<BltPixel>(),
                self.canvas.pixels().len(),
            )
        };

        gop.blt(BltOp::BufferToVideo {
            buffer,
            src: BltRegion::Full,
            dest: (0, 0),
            dims: (self.width, self.height),
        })
        .is_ok()
    }

    /// Paint the backdrop flat and leave it on the panel for the kernel.
    ///
    /// This is the whole of the seamless handover, and it works because of what
    /// the firmware does *not* do: `exit_boot_services` tears down GOP as a
    /// protocol, but nothing clears the scanout buffer it was pointing at. The
    /// kernel's EFI stub reads that same framebuffer's address out of the GOP
    /// mode information it collected beforehand and hands it to `efifb` /
    /// `simpledrm`, which map it and leave the existing contents alone until
    /// something draws.
    ///
    /// So the last thing RavenBoot draws is the first thing the kernel shows.
    /// With [`crate::theme::BACKDROP`] in there, the panel holds the desktop's
    /// own background colour from here until `raven-greeter`'s first frame — no
    /// black flash, no vendor logo, no scrolling console, given the
    /// `quiet loglevel=3` the Raven entries already carry.
    ///
    /// Flat rather than [`Canvas::gradient`]: see [`Canvas::fill`].
    pub fn hand_off(&mut self, boot_services: &BootServices) {
        self.canvas.fill(crate::theme::BACKDROP);
        let _ = self.present(boot_services);
    }
}

/// Logical-to-physical scale for a panel `height` pixels tall.
///
/// The greeter gets its scale from the compositor, which gets it from the
/// output's physical size. Neither exists here: GOP reports a resolution and
/// nothing about how large the panel is, so there is no honest way to compute a
/// DPI. Height alone is the available signal, and these thresholds are the
/// conventional reading of it — the same one every desktop applies by default
/// before a user overrides it.
///
/// Deliberately coarse, and deliberately not continuous. The only thing worse
/// than a boot menu that is slightly too small is one that is blurry, and a
/// fractional scale would land the text between the atlas's rasterized sizes.
fn scale_for(height: usize) -> f32 {
    match height {
        0..=1152 => 1.0,    // up to and including 1080p
        1153..=1800 => 1.5, // 1440p, and the 1600p 16:10 panels
        _ => 2.0,           // 4K and up
    }
}
