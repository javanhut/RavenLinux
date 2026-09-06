//! The boot menu, drawn.
//!
//! Nothing here talks to the firmware. [`draw`] takes a [`View`] — plain data
//! that `main` assembles from its navigation state — and a [`Canvas`], which is
//! what lets the whole of this module be compiled for the host and rendered to
//! a PNG by `preview/`. Input handling stays in `main`, because it is the only
//! part that genuinely needs UEFI.
//!
//! # The layout
//!
//! One centred column, in the greeter's proportions:
//!
//! ```text
//!                        [ mark ]
//!                       R A V E N
//!             Select an operating system to boot
//!
//!             +--------------------------------+
//!             |  > Raven Linux                 |
//!             |    Raven Linux (Serial)      > |
//!             |    Recovery                  > |
//!             +--------------------------------+
//!
//!                  Booting Raven Linux in 5s
//!
//!  RavenBoot 0.1.0            [^v] Move  [Enter] Select  [Esc] Back
//! ```
//!
//! The block is centred vertically as a unit and the footer is pinned to the
//! bottom, which is the greeter's arrangement — its card is centred and its
//! hostname line is pinned — so the two screens have their weight in the same
//! place either side of the kernel.

use alloc::string::String;

use crate::font;
use crate::gfx::{fmin, round, Canvas, Rect};
use crate::theme;

/// What a row does, which is the only thing the drawing needs to know about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// Boots something, or performs an action such as rebooting.
    Action,
    /// Opens a submenu. Drawn with a chevron on the right.
    Submenu,
    /// Returns to the parent menu. Drawn with a chevron on the left.
    Back,
}

/// One row.
pub struct Row<'a> {
    pub label: &'a str,
    pub kind: RowKind,
}

/// Everything the menu draws, as data.
pub struct View<'a> {
    pub rows: &'a [Row<'a>],
    pub selected: usize,
    /// The line above the card. The menu title at the root, the submenu's name
    /// below it.
    pub prompt: &'a str,
    /// The auto-boot countdown, or a boot failure, under the card.
    pub status: Option<Status<'a>>,
    /// Bottom-left. The greeter puts the hostname here.
    pub version: &'a str,
    /// Whether `Esc` goes back or does nothing, which changes the last hint.
    pub can_go_back: bool,
}

pub struct Status<'a> {
    pub text: &'a str,
    pub tone: Tone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// The countdown.
    Caution,
    /// A boot that failed.
    Failure,
}

/// Letterspacing for the wordmark, as a fraction of the face's advance.
///
/// Tracking a monospaced face is normally a mistake — it is already evenly
/// spaced — but at display size a five-letter all-caps word needs to read as a
/// mark and not as a word, and this is what does it.
pub const WORDMARK_TRACKING: f32 = 0.28;

/// Draw one frame.
///
/// `scale` is [`crate::screen::Screen::scale`]: every theme metric is in
/// logical pixels and is multiplied by it here, in one place, so that no
/// drawing site below has to remember to.
pub fn draw(canvas: &mut Canvas, view: &View, scale: f32) {
    let s = |v: f32| v * scale;
    let (w, h) = (canvas.width(), canvas.height());

    // The ground everything else composites onto — and the colour the kernel
    // inherits a moment later. See `screen::Screen::hand_off`.
    canvas.gradient(theme::BACKDROP, theme::BACKDROP_EDGE);

    let card_width = fmin(s(theme::CARD_WIDTH), w - s(48.0));
    let row_height = s(theme::ROW_HEIGHT);
    let row_gap = s(theme::ROW_GAP);
    let card_padding = s(theme::CARD_PADDING);

    let prompt_face = font::face_for(s(theme::PROMPT_SIZE));
    let body_face = font::face_for(s(theme::BODY_SIZE));
    let small_face = font::face_for(s(theme::SMALL_SIZE));
    let wordmark_face = font::face_for(s(theme::WORDMARK_SIZE));

    // --- how much room is there, and how many rows fit in it -----------------

    let footer_margin = s(28.0);
    let footer_height = footer_margin + small_face.line_height + s(theme::FOOTER_RULE_GAP);
    let status_height = s(theme::FOOTER_GAP) + small_face.line_height;

    let header_height = s(theme::MARK_SIZE)
        + s(theme::MARK_GAP)
        + wordmark_face.line_height
        + s(theme::HEADER_GAP)
        + prompt_face.line_height
        + s(theme::PROMPT_GAP);

    let room_for_rows = h - header_height - footer_height - status_height - card_padding * 2.0;
    let visible = visible_rows(room_for_rows, row_height, row_gap, view.rows.len());
    let window = scroll_window(view.selected, visible, view.rows.len());

    let card_height = card_padding * 2.0
        + visible as f32 * row_height
        + (visible.saturating_sub(1)) as f32 * row_gap;

    // The header, the card and the status line, centred as one block.
    //
    // The status line's height is reserved whether or not there is a status to
    // draw. Measuring the block from what is actually on screen would let the
    // rows drift upward the moment the countdown stops, and a menu whose
    // entries move when you press a key looks like it is malfunctioning.
    let block_height = header_height + card_height + status_height;
    let top = round((h - block_height) / 2.0).max(s(24.0));

    let cx = round(w / 2.0);
    let card_x = round(cx - card_width / 2.0);

    // --- the mark and the wordmark -------------------------------------------

    let mark_size = s(theme::MARK_SIZE);
    draw_mark(canvas, cx, top + mark_size / 2.0, mark_size);

    let wordmark_baseline = top + mark_size + s(theme::MARK_GAP) + wordmark_face.ascent;
    let tracking = wordmark_face.advance * WORDMARK_TRACKING;
    let wordmark_width = font::measure_tracked(wordmark_face, "RAVEN", tracking);
    font::draw_tracked(
        canvas,
        wordmark_face,
        // The trailing letter carries no tracking, so centring on the measured
        // width would sit the word half a gap to the left of the mark above it.
        round(cx - wordmark_width / 2.0),
        round(wordmark_baseline),
        "RAVEN",
        tracking,
        theme::TEXT,
    );

    // --- the prompt ----------------------------------------------------------

    let prompt_baseline =
        wordmark_baseline + wordmark_face.descent + s(theme::HEADER_GAP) + prompt_face.ascent;
    font::draw_centered(
        canvas,
        prompt_face,
        cx,
        round(prompt_baseline),
        view.prompt,
        theme::TEXT_DIM,
    );

    // --- the card ------------------------------------------------------------

    let card_y = round(top + header_height);
    let card = Rect::new(card_x, card_y, card_width, card_height);
    canvas.rounded_rect(card, s(theme::CARD_RADIUS), theme::SURFACE);
    canvas.rounded_rect_outline(card, s(theme::CARD_RADIUS), s(1.0), theme::BORDER);

    for (slot, index) in (window..window + visible).enumerate() {
        let Some(row) = view.rows.get(index) else {
            break;
        };
        let y = card_y + card_padding + slot as f32 * (row_height + row_gap);
        draw_row(
            canvas,
            row,
            Rect::new(
                card_x + card_padding,
                round(y),
                card_width - card_padding * 2.0,
                row_height,
            ),
            body_face,
            index == view.selected,
            scale,
        );
    }

    // A row hidden above or below the window is signalled rather than merely
    // absent: without this the menu silently claims the list ends where the
    // card does, and an entry the user knows exists appears to have vanished.
    if window > 0 {
        draw_chevron(canvas, cx, card_y + card_padding / 2.0, s(4.0), Dir::Up);
    }
    if window + visible < view.rows.len() {
        draw_chevron(
            canvas,
            cx,
            card_y + card_height - card_padding / 2.0,
            s(4.0),
            Dir::Down,
        );
    }

    // --- the status line -----------------------------------------------------

    if let Some(status) = &view.status {
        let baseline = card_y + card_height + s(theme::FOOTER_GAP) + small_face.ascent;
        let color = match status.tone {
            Tone::Caution => theme::WARNING,
            Tone::Failure => theme::ERROR,
        };
        font::draw_centered(canvas, small_face, cx, round(baseline), status.text, color);
    }

    // --- the footer ----------------------------------------------------------

    draw_footer(canvas, view, w, h, small_face, scale);
}

/// One menu row: its wash, its ring, its caret and its label.
fn draw_row(
    canvas: &mut Canvas,
    row: &Row,
    rect: Rect,
    face: &font::Face,
    selected: bool,
    scale: f32,
) {
    let s = |v: f32| v * scale;
    let radius = s(theme::ROW_RADIUS);

    if selected {
        // A wash plus a ring, rather than a solid accent fill. The selection
        // moves and the wordmark does not, so a filled row would make the
        // brightest thing on the screen the one that keeps moving.
        canvas.rounded_rect(rect, radius, theme::ROW_SELECTED);
        canvas.rounded_rect_outline(rect, radius, s(theme::FOCUS_RING_WIDTH), theme::ACCENT);
    }

    let text_inset = s(theme::ROW_TEXT_INSET);
    let baseline = round(rect.y + (rect.height - face.line_height) / 2.0 + face.ascent);

    // The caret marks the selection inside the gutter the ring already claims,
    // so rows do not shift sideways as the selection moves over them.
    if selected {
        let size = s(4.5);
        draw_chevron(
            canvas,
            rect.x + text_inset / 2.0,
            rect.y + rect.height / 2.0,
            size,
            Dir::Right,
        );
    }

    // `boot.cfg` and the built-in defaults both spell a submenu "Recovery >" and
    // a return "< Back", because a text-mode menu had no other way to say it.
    // This one has, so the stand-in character is stripped and a chevron drawn
    // instead. Stripping is safe for a config that does not use the convention,
    // because it only ever removes a leading or trailing '>' or '<'.
    let label = match row.kind {
        RowKind::Submenu => row.label.trim_end_matches(['>', ' ']),
        RowKind::Back => row.label.trim_start_matches(['<', ' ']),
        RowKind::Action => row.label,
    };

    // The right-hand gutter the chevron sits in, kept clear of the label.
    let available = rect.width - text_inset - s(24.0);
    font::draw(
        canvas,
        face,
        round(rect.x + text_inset),
        baseline,
        &elide(label, face, available),
        theme::TEXT,
    );

    match row.kind {
        RowKind::Submenu => draw_chevron(
            canvas,
            rect.x + rect.width - s(16.0),
            rect.y + rect.height / 2.0,
            s(4.5),
            Dir::Right,
        ),
        RowKind::Back => draw_chevron(
            canvas,
            rect.x + rect.width - s(16.0),
            rect.y + rect.height / 2.0,
            s(4.5),
            Dir::Left,
        ),
        RowKind::Action => {}
    }
}

/// The version on the left, the key hints on the right.
///
/// This is `raven-ui`'s `draw_footer`, including the part that matters most:
/// when the two halves would collide it drops the right one rather than letting
/// them draw through each other. The greeter keeps the hostname for the same
/// reason this keeps the version — the half you cannot work out by looking at
/// the screen is the half worth the space.
fn draw_footer(canvas: &mut Canvas, view: &View, w: f32, h: f32, face: &font::Face, scale: f32) {
    let s = |v: f32| v * scale;
    let margin = s(28.0);
    let baseline = round(h - margin);

    canvas.rule(
        margin,
        round(h - margin - face.line_height - s(theme::FOOTER_RULE_GAP)),
        w - margin * 2.0,
        s(1.0),
        theme::BORDER,
    );

    font::draw(
        canvas,
        face,
        margin,
        baseline,
        view.version,
        theme::TEXT_DIM,
    );

    let hints: &[(Cap, &str)] = if view.can_go_back {
        &[
            (Cap::UpDown, "Move"),
            (Cap::Text("Enter"), "Select"),
            (Cap::Text("Esc"), "Back"),
        ]
    } else {
        &[(Cap::UpDown, "Move"), (Cap::Text("Enter"), "Boot")]
    };

    let hints_width = hints_width(hints, face, scale);
    let version_width = face.measure(view.version);
    if version_width + hints_width + margin * 3.0 > w {
        return;
    }
    draw_hints(
        canvas,
        hints,
        w - margin - hints_width,
        baseline,
        face,
        scale,
    );
}

/// What goes inside a keycap.
enum Cap<'a> {
    Text(&'a str),
    /// The two arrow keys, drawn as chevrons — there is no arrow in the atlas,
    /// which is printable ASCII only.
    UpDown,
}

fn cap_width(cap: &Cap, face: &font::Face, scale: f32) -> f32 {
    let pad = scale * 7.0;
    match cap {
        Cap::Text(text) => face.measure(text) + pad * 2.0,
        Cap::UpDown => scale * 9.0 * 2.0 + pad * 2.0,
    }
}

fn hints_width(hints: &[(Cap, &str)], face: &font::Face, scale: f32) -> f32 {
    let gap = scale * 8.0;
    let group_gap = scale * 20.0;
    let mut width = 0.0;
    for (i, (cap, label)) in hints.iter().enumerate() {
        if i > 0 {
            width += group_gap;
        }
        width += cap_width(cap, face, scale) + gap + face.measure(label);
    }
    width
}

fn draw_hints(
    canvas: &mut Canvas,
    hints: &[(Cap, &str)],
    x: f32,
    baseline: f32,
    face: &font::Face,
    scale: f32,
) {
    let gap = scale * 8.0;
    let group_gap = scale * 20.0;
    let cap_height = face.line_height + scale * 4.0;
    let cap_y = baseline - face.ascent - scale * 2.0;

    let mut pen = x;
    for (i, (cap, label)) in hints.iter().enumerate() {
        if i > 0 {
            pen += group_gap;
        }
        let width = cap_width(cap, face, scale);
        let rect = Rect::new(round(pen), round(cap_y), width, cap_height);
        canvas.rounded_rect_outline(rect, scale * 4.0, scale, theme::BORDER);

        match cap {
            Cap::Text(text) => font::draw_centered(
                canvas,
                face,
                rect.x + width / 2.0,
                baseline,
                text,
                theme::TEXT_DIM,
            ),
            Cap::UpDown => {
                let cy = rect.y + cap_height / 2.0;
                let size = scale * 3.5;
                draw_chevron(
                    canvas,
                    rect.x + width / 2.0 - scale * 5.0,
                    cy,
                    size,
                    Dir::Up,
                );
                draw_chevron(
                    canvas,
                    rect.x + width / 2.0 + scale * 5.0,
                    cy,
                    size,
                    Dir::Down,
                );
            }
        }

        pen += width + gap;
        font::draw(canvas, face, round(pen), baseline, label, theme::TEXT_DIM);
        pen += face.measure(label);
    }
}

#[derive(Clone, Copy)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

/// A small solid triangle, centred on `(cx, cy)`.
///
/// Solid rather than a stroked chevron: at 4-5 logical pixels a stroke is
/// thinner than the anti-aliasing either side of it and reads as a grey smudge,
/// where a filled triangle stays a shape.
fn draw_chevron(canvas: &mut Canvas, cx: f32, cy: f32, size: f32, dir: Dir) {
    let (a, b, c) = match dir {
        Dir::Right => (
            (cx - size * 0.6, cy - size),
            (cx + size * 0.7, cy),
            (cx - size * 0.6, cy + size),
        ),
        Dir::Left => (
            (cx + size * 0.6, cy - size),
            (cx - size * 0.7, cy),
            (cx + size * 0.6, cy + size),
        ),
        Dir::Up => (
            (cx - size, cy + size * 0.6),
            (cx, cy - size * 0.7),
            (cx + size, cy + size * 0.6),
        ),
        Dir::Down => (
            (cx - size, cy - size * 0.6),
            (cx, cy + size * 0.7),
            (cx + size, cy - size * 0.6),
        ),
    };
    canvas.polygon(&[a, b, c], theme::ACCENT);
}

/// The raven, centred on `(cx, cy)` and `size` pixels across.
///
/// A silhouette rather than an outline, and a polygon rather than a glyph,
/// because there is no raven in the atlas and there is no raven in any font the
/// image ships. The points are a stylised bird in flight, normalised to a unit
/// box so the shape is resolution-independent and the same at every scale.
///
/// Drawn in the accent, which makes it the one saturated thing on the screen —
/// it is also the only element that never changes, so it can carry the colour
/// that everything else borrows for focus.
pub fn draw_mark(canvas: &mut Canvas, cx: f32, cy: f32, size: f32) {
    /// The silhouette, in a unit box with y down.
    ///
    /// A raven perched in profile, facing left: heavy straight bill, high
    /// crown, the folded wing running down the back, and the long wedge tail
    /// that is the easiest way to tell a corvid from a generic bird at this
    /// size. Traced clockwise from the tip of the bill.
    // These are coordinates in a unit box, and one of them is close enough to
    // 1/PI for clippy's `approx_constant` -- which is deny-by-default -- to
    // decide it is a mistyped constant. Moving the point to appease the lint
    // would be changing a drawing to satisfy a linter.
    #[allow(clippy::approx_constant)]
    #[rustfmt::skip]
    const RAVEN: &[(f32, f32)] = &[
        // The bill, upper mandible, base to tip. Roughly half the skull's
        // length and nearly as deep at the base as it is long: stubby, heavy,
        // carried level, closing to a small hook. A bill that is longer,
        // shallower or angled down is a kingfisher, and several drafts of this
        // mark were.
        (0.400, 0.330),
        (0.318, 0.362),
        (0.232, 0.400),
        (0.160, 0.440),
        (0.126, 0.468), // tip
        (0.148, 0.492), // the hook
        // Lower mandible, tip back to the gape.
        (0.240, 0.520),
        (0.330, 0.540),
        (0.404, 0.556), // the gape, at the corner of the mouth
        // The throat hackles: the shaggy beard a raven has and a crow does not.
        // One shallow step -- at 56 pixels a sawtooth becomes mush, and a deep
        // step reads as a rendering fault rather than as feathers.
        (0.420, 0.626),
        (0.455, 0.682),
        (0.448, 0.718),
        (0.478, 0.790),
        // The neck, cut square at the bottom. A head mark needs an edge to end
        // on; a neck that tapers to nothing reads as a severed one.
        (0.566, 0.858),
        (0.734, 0.898),
        (0.940, 0.852),
        (0.928, 0.696),
        (0.912, 0.554), // nape
        // The skull: tall, and flat across the crown rather than domed.
        (0.902, 0.434),
        (0.876, 0.328),
        (0.818, 0.244),
        (0.724, 0.188),
        (0.616, 0.174), // crown
        (0.514, 0.206),
        (0.438, 0.262), // the forehead break, back down to the bill
    ];

    /// The eye, as a fraction of the same unit box: centre and radius.
    ///
    /// Punched out of the silhouette rather than drawn on top of it, in the
    /// backdrop's own darker edge colour so it reads as a hole. This is the
    /// single change that made the mark legible: a filled bird's head with no
    /// eye is a wedge, and every early draft of this shape was mistaken for
    /// waterfowl until the eye went in.
    const EYE: (f32, f32, f32) = (0.556, 0.352, 0.058);

    let left = cx - size / 2.0;
    let top = cy - size / 2.0;
    let mut points = [(0.0f32, 0.0f32); RAVEN.len()];
    for (slot, &(x, y)) in points.iter_mut().zip(RAVEN) {
        *slot = (left + x * size, top + y * size);
    }
    canvas.polygon(&points, theme::ACCENT);

    let (ex, ey, er) = EYE;
    canvas.circle(
        left + ex * size,
        top + ey * size,
        er * size,
        theme::BACKDROP_EDGE,
    );
}

/// How many rows fit in `room`, at least one and never more than there are.
fn visible_rows(room: f32, row_height: f32, row_gap: f32, total: usize) -> usize {
    if total == 0 {
        return 0;
    }
    let step = row_height + row_gap;
    let fits = if room < row_height {
        1
    } else {
        (((room - row_height) / step) as usize) + 1
    };
    fits.clamp(1, total)
}

/// The first row of the window that keeps `selected` visible.
///
/// Centres the selection in the window rather than scrolling only at the edges.
/// A boot menu is navigated by wrapping around from the last entry to the
/// first — see `MenuNav::move_down` — and edge scrolling makes that wrap look
/// like the list jumped, because the window has to move the whole way in one
/// keypress either way.
fn scroll_window(selected: usize, visible: usize, total: usize) -> usize {
    if total <= visible {
        return 0;
    }
    let half = visible / 2;
    selected.saturating_sub(half).min(total - visible)
}

/// `text`, shortened with an ellipsis until it fits `available` pixels.
///
/// An entry name comes from `boot.cfg` and can be any length. Drawing it past
/// the card's edge would put it on the backdrop, where it would look like a
/// second UI rather than a long name.
fn elide(text: &str, face: &font::Face, available: f32) -> String {
    if face.measure(text) <= available || available <= 0.0 {
        return String::from(text);
    }
    // "..." rather than a single ellipsis character: the atlas is ASCII.
    let room = ((available / face.advance) as usize).saturating_sub(3);
    let mut out = String::new();
    for ch in text.chars().take(room) {
        out.push(ch);
    }
    out.push_str("...");
    out
}
