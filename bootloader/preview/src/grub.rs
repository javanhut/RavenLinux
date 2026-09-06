//! Generates the GRUB theme's assets, from the same code that draws the menu.
//!
//! GRUB is RavenBoot's BIOS-only counterpart: the ISO boots it when the machine
//! has no UEFI, and `raven-install` never puts it on a disk. It is still the
//! first screen those machines show, though, so it should not be the one screen
//! in the image wearing somebody else's colours.
//!
//! # What GRUB can be told
//!
//! A GRUB theme is a `theme.txt` plus PNGs. It can place labels and images by
//! percentage, colour them with hex, and draw the menu and its selected row
//! from nine-slice pixmaps. What it cannot do is anything computed: no
//! distance fields, no letterspacing, no scale ladder, and no shapes it did not
//! load off disk.
//!
//! So the parts that are *drawings* — the raven, the card, the selection ring,
//! the tracked wordmark — are rendered here by [`crate::gfx`] and [`crate::menu`]
//! at build time and shipped as pixmaps, and the parts that are *text* are left
//! to GRUB with the same font and the same hex colours out of
//! [`crate::theme`]. Nothing is transcribed by hand, which is the point: a
//! second copy of `0xFF7AA2F7` typed into a `theme.txt` is a copy that goes
//! stale the first time the accent changes.
//!
//! # Why nothing here has an alpha channel
//!
//! The theme sets a flat `desktop-color` rather than a background image, so
//! every ground colour is known at generation time: the mark and the card sit
//! on [`theme::BACKDROP`], and the selection sits on [`theme::SURFACE`]. Each
//! tile is therefore rendered *onto* its own ground and shipped opaque.
//!
//! That is not a shortcut around transparency, it is avoiding a worse problem.
//! A gradient background would have to be a stretched image, and GRUB stretching
//! a one-pixel-wide column reproduces the banding the Bayer dither in
//! `Canvas::gradient` exists to prevent — nine flat bands across the screen. A
//! flat backdrop has no banding to prevent, and it is the same
//! [`theme::BACKDROP`] the desktop uses.

use std::path::Path;

use crate::gfx::{Canvas, Rect};
use crate::menu;
use crate::png;
use crate::theme;

/// The raven, in pixels.
///
/// One size, because GRUB has no equivalent of `screen::scale_for` — a theme is
/// laid out in percentages but its images are whatever size they were saved at.
/// 96 is chosen for a 1080p panel, which is what a BIOS machine that has got as
/// far as a graphical GRUB is most likely to have.
const MARK: usize = 96;

/// The wordmark's em size. Larger than the boot menu's 30, because this one is
/// not scaled up on a big panel and has to hold its own at native size.
const WORDMARK: f32 = 40.0;

/// Nine-slice tile size for the card and the selected row.
///
/// The corner tiles are the shape's corner radius square, so they carry the
/// whole curve; the edges are `EDGE` across and are tiled by GRUB to whatever
/// width the menu turns out to be.
const EDGE: usize = 8;

/// The two em sizes `theme.txt` names, and that stage4 hands to grub-mkfont.
/// Larger than the boot menu's 15 and 12.5 for the same reason [`WORDMARK`] is:
/// nothing here is scaled up on a big panel.
const BODY_SIZE: f32 = 16.0;
const SMALL_SIZE: f32 = 13.0;

pub fn generate(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;

    mark(dir)?;
    wordmark(dir)?;

    // The card the menu sits in: SURFACE with a hairline BORDER, on the
    // backdrop. `menu.rs` draws exactly this with `CARD_RADIUS` and a 1px
    // outline.
    nine_slice(
        dir,
        "card",
        theme::CARD_RADIUS as usize,
        theme::BACKDROP,
        |canvas, rect| {
            canvas.rounded_rect(rect, theme::CARD_RADIUS, theme::SURFACE);
            canvas.rounded_rect_outline(rect, theme::CARD_RADIUS, 1.0, theme::BORDER);
        },
    )?;

    // The selected row: the accent wash and the focus ring, on the card. Same
    // two calls `menu::draw_row` makes.
    nine_slice(
        dir,
        "select",
        theme::ROW_RADIUS as usize,
        theme::SURFACE,
        |canvas, rect| {
            canvas.rounded_rect(rect, theme::ROW_RADIUS, theme::ROW_SELECTED);
            canvas.rounded_rect_outline(
                rect,
                theme::ROW_RADIUS,
                theme::FOCUS_RING_WIDTH,
                theme::ACCENT,
            );
        },
    )?;

    std::fs::write(dir.join("theme.txt"), theme_txt())?;
    Ok(())
}

fn mark(dir: &Path) -> std::io::Result<()> {
    let mut canvas = Canvas::new(MARK, MARK);
    canvas.fill(theme::BACKDROP);
    let half = MARK as f32 / 2.0;
    menu::draw_mark(&mut canvas, half, half, MARK as f32);
    png::write(&dir.join("raven.png"), &canvas)
}

fn wordmark(dir: &Path) -> std::io::Result<()> {
    let face = crate::font::face_for(WORDMARK);
    let tracking = face.advance * menu::WORDMARK_TRACKING;
    let (width, height) = wordmark_size();

    let mut canvas = Canvas::new(width, height);
    canvas.fill(theme::BACKDROP);
    crate::font::draw_tracked(
        &mut canvas,
        face,
        0.0,
        face.ascent,
        "RAVEN",
        tracking,
        theme::TEXT,
    );
    png::write(&dir.join("wordmark.png"), &canvas)
}

/// Render a shape once and cut it into the nine tiles GRUB expects.
///
/// GRUB names them by compass point: `_nw` `_n` `_ne` / `_w` `_c` `_e` / `_sw`
/// `_s` `_se`. It draws the corners at fixed size, tiles the edges along their
/// run, and tiles the centre to fill — so the corners must contain the whole
/// curve, and the edges must be a slice the shape does not vary along.
///
/// Cutting one rendering rather than drawing nine is what keeps the seams
/// invisible: every tile comes from the same anti-aliased edge, at the same
/// subpixel phase.
fn nine_slice(
    dir: &Path,
    name: &str,
    radius: usize,
    ground: theme::Color,
    draw: impl Fn(&mut Canvas, Rect),
) -> std::io::Result<()> {
    let size = radius * 2 + EDGE;
    let mut canvas = Canvas::new(size, size);
    canvas.fill(ground);
    draw(&mut canvas, Rect::new(0.0, 0.0, size as f32, size as f32));

    let r = radius;
    let far = radius + EDGE;
    // The centre column and row of the rendering, which is the part that is
    // uniform along the shape's run and so is safe to tile.
    let tiles: [(&str, usize, usize, usize, usize); 9] = [
        ("nw", 0, 0, r, r),
        ("n", r, 0, EDGE, r),
        ("ne", far, 0, r, r),
        ("w", 0, r, r, EDGE),
        ("c", r, r, EDGE, EDGE),
        ("e", far, r, r, EDGE),
        ("sw", 0, far, r, r),
        ("s", r, far, EDGE, r),
        ("se", far, far, r, r),
    ];

    for (suffix, x, y, w, h) in tiles {
        png::write_region(
            &dir.join(format!("{name}_{suffix}.png")),
            &canvas,
            x,
            y,
            w,
            h,
        )?;
    }
    Ok(())
}

/// How many rows the card is sized for.
///
/// GRUB cannot size a menu to its contents: `boot_menu` is a box, and the card
/// pixmap fills whatever box it is given, however many entries land in it. So
/// the height is chosen rather than derived.
///
/// Five is what the ISO's own `grub.cfg` needs: four at the root once the
/// desktop entry is appended, and five under System on an EFI machine. Anything
/// past five scrolls, which is what the scrollbar is for, and anything under it
/// leaves one row of empty card -- which is the better failure of the two.
const ROWS: usize = 5;

/// `theme.txt`, with every colour and coordinate computed from
/// [`crate::theme`].
///
/// Written from here rather than checked in by hand for the reason at the top
/// of this file: a hex string typed into a config is a copy of a constant, and
/// it goes stale silently.
///
/// # Why the layout is centred in pixels
///
/// Every size in this theme is pixels — GRUB has no scale ladder, and its
/// images are whatever size they were saved at — so anchoring the *top* of the
/// composition to a percentage makes the gaps grow and shrink against fixed
/// content. The first draft did that, and on a 1024x768 panel the prompt landed
/// on the first row of the menu.
///
/// So the block's height is added up here and the whole thing is hung from
/// `50%-half`, which is what `menu::draw` does with the same numbers. It is
/// centred at every resolution, and the pieces cannot drift into each other.
fn theme_txt() -> String {
    let hex = |c: theme::Color| format!("#{:02x}{:02x}{:02x}", c.red(), c.green(), c.blue());

    let (wordmark_w, wordmark_h) = wordmark_size();
    let prompt_h = line_height_at(BODY_SIZE);
    let card_h = 2 * theme::CARD_PADDING as usize
        + ROWS * theme::ROW_HEIGHT as usize
        + (ROWS - 1) * theme::ROW_GAP as usize;

    // Offsets down the block, in the order menu.rs stacks them.
    let mark_top = 0;
    let wordmark_top = MARK + theme::MARK_GAP as usize;
    let prompt_top = wordmark_top + wordmark_h + theme::HEADER_GAP as usize;
    let menu_top = prompt_top + prompt_h + theme::PROMPT_GAP as usize;
    let status_top = menu_top + card_h + theme::FOOTER_GAP as usize;

    let block = menu_top + card_h;
    let half = block / 2;

    // `50%-half+offset`, folded into a single signed term so GRUB gets the one
    // addition its coordinate grammar allows.
    let from_centre = |offset: usize| -> String {
        let delta = offset as isize - half as isize;
        if delta < 0 {
            format!("50%-{}", -delta)
        } else {
            format!("50%+{delta}")
        }
    };

    format!(
        r##"# Generated by `cargo run -- grub-theme` in bootloader/preview.
# Do not edit: every colour and metric here comes from bootloader/src/theme.rs,
# and an edit made here is lost the next time it is regenerated.
#
# This is the BIOS menu. Its UEFI counterpart is RavenBoot, which draws the same
# design with real geometry -- see bootloader/src/menu.rs. GRUB cannot match it
# exactly and is not asked to: what it matches is the palette, the typeface, the
# card, and the accent focus ring.
#
# The one visible difference is the "> " on a submenu's title. RavenBoot strips
# it and draws a chevron; GRUB has no chevron and prints the title as written,
# so the convention stays where it is useful.

desktop-color: "{backdrop}"
title-text: ""

# The mark and the wordmark, both drawn by the same code as RavenBoot's.
+ image {{
    id = "raven"
    file = "raven.png"
    left = 50%-{mark_half}
    top = {mark_y}
}}

+ image {{
    file = "wordmark.png"
    left = 50%-{wordmark_half}
    top = {wordmark_y}
}}

+ label {{
    text = "Select an operating system to boot"
    font = "{body_font}"
    color = "{text_dim}"
    left = 0
    top = {prompt_y}
    width = 100%
    align = "center"
}}

+ boot_menu {{
    left = 50%-{menu_half}
    top = {menu_y}
    width = {menu_width}
    height = {card_h}

    item_font = "{body_font}"
    item_color = "{text}"
    selected_item_color = "{text}"
    item_height = {row_height}
    item_spacing = {row_gap}
    item_padding = {row_padding}
    item_icon_space = 0
    icon_width = 0
    icon_height = 0

    menu_pixmap_style = "card_*.png"
    selected_item_pixmap_style = "select_*.png"

    scrollbar = true
    scrollbar_width = 4
    scrollbar_thumb = "select_*.png"
}}

# GRUB's own countdown, styled as the status line RavenBoot draws under the
# card. The bar itself is switched off -- there is no progress bar in the UEFI
# menu either, just the sentence.
+ progress_bar {{
    id = "__timeout__"
    text = "@TIMEOUT_NOTIFICATION_MIDDLE@"
    font = "{small_font}"
    text_color = "{warning}"
    fg_color = "{backdrop}"
    bg_color = "{backdrop}"
    border_color = "{backdrop}"
    left = 0
    top = {status_y}
    width = 100%
    height = {small_line}
}}

+ label {{
    text = "GRUB"
    font = "{small_font}"
    color = "{text_dim}"
    left = 28
    top = 100%-40
}}

+ label {{
    text = "Up/Down  Move      Enter  Boot      Esc  Back"
    font = "{small_font}"
    color = "{text_dim}"
    left = 40%
    top = 100%-40
    width = 60%-28
    align = "right"
}}
"##,
        backdrop = hex(theme::BACKDROP),
        text = hex(theme::TEXT),
        text_dim = hex(theme::TEXT_DIM),
        warning = hex(theme::WARNING),
        mark_half = MARK / 2,
        mark_y = from_centre(mark_top),
        wordmark_half = wordmark_w / 2,
        wordmark_y = from_centre(wordmark_top),
        prompt_y = from_centre(prompt_top),
        menu_y = from_centre(menu_top),
        status_y = from_centre(status_top),
        menu_width = theme::CARD_WIDTH as usize,
        menu_half = theme::CARD_WIDTH as usize / 2,
        row_height = theme::ROW_HEIGHT as usize,
        row_gap = theme::ROW_GAP as usize,
        row_padding = theme::CARD_PADDING as usize,
        small_line = line_height_at(SMALL_SIZE),
        // grub-mkfont names a face "<family> <style> <size>", and stage4 builds
        // these two sizes from the repository's JetBrains Mono.
        body_font = "JetBrains Mono Regular 16",
        small_font = "JetBrains Mono Regular 13",
    )
}

/// A line's height at `size`, in pixels.
///
/// `face_for` snaps to the nearest rasterized rung, so a face asked for 13 may
/// come back as 12 and report a line height a pixel and a half short. GRUB will
/// have a real `.pf2` at the size named in `theme.txt`, so the layout has to be
/// measured at the size actually asked for and not at the rung the atlas
/// happened to pick.
fn line_height_at(size: f32) -> usize {
    let face = crate::font::face_for(size);
    (face.line_height * size / face.px).ceil() as usize
}

/// The wordmark PNG's exact size.
///
/// One function, called by both the renderer and the `theme.txt` writer, so the
/// image and the coordinates that position it cannot disagree.
fn wordmark_size() -> (usize, usize) {
    let face = crate::font::face_for(WORDMARK);
    let tracking = face.advance * menu::WORDMARK_TRACKING;
    (
        crate::font::measure_tracked(face, "RAVEN", tracking).ceil() as usize,
        (face.ascent + face.descent).ceil() as usize,
    )
}
