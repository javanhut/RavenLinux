//! A small interpreter for the GRUB theme, so it can be looked at.
//!
//! GRUB cannot be run here — this machine has no `grub-mkfont`, no
//! `/usr/lib/grub`, and no container engine to reach one — and a theme is the
//! kind of thing that is wrong in ways no amount of reading catches: a
//! coordinate off by a percent, a nine-slice tile cut on the wrong pixel, two
//! components overlapping at a resolution nobody tried.
//!
//! So this reads the generated `theme.txt` and the generated PNGs and composes
//! what GRUB would compose. It is a *simulation of the file*, not of GRUB:
//! it parses the same text, loads the same tiles, and lays them out by the same
//! rules, which is what catches the errors above.
//!
//! # What it cannot tell you
//!
//! Everything GRUB-specific. Whether `gfxmenu` is in the core image, whether
//! the `.pf2` a font string names actually resolves, whether the firmware gave
//! GRUB the resolution the percentages were reasoned about — none of that is
//! visible from here. A clean render means the theme file is self-consistent,
//! not that GRUB will draw it.
//!
//! Text is drawn with this crate's own atlas at the size the font string names.
//! That is the same TTF at the same size GRUB will have been handed as `.pf2`,
//! so the metrics are right, but GRUB's own rasterizer is not this one.

use std::collections::HashMap;
use std::path::Path;

use crate::font;
use crate::gfx::{Canvas, Rect};
use crate::png;
use crate::theme::Color;

/// One `+ type { ... }` block.
struct Component {
    kind: String,
    props: HashMap<String, String>,
}

impl Component {
    fn get(&self, key: &str) -> Option<&str> {
        self.props.get(key).map(String::as_str)
    }
}

/// The root menu the ISO's own `grub.cfg` writes — see `setup_grub` in
/// `stage4-iso.sh`, which is a different and shorter list than the one
/// RavenBoot builds in `config.rs`. Previewing RavenBoot's list here would be
/// previewing a menu no BIOS machine ever sees.
///
/// "Raven Desktop (Huginn)" is appended by stage4 only when a compositor was
/// actually installed, so this is the fullest the root menu gets.
const ENTRIES: &[&str] = &[
    "Raven Linux",
    "Raven Linux (Serial)",
    "System >",
    "Raven Desktop (Huginn)",
];
const SELECTED: usize = 0;

/// GRUB's own English expansion of the token the theme uses for its countdown.
const TIMEOUT_MIDDLE: &str = "The highlighted entry will be executed automatically in 5s.";

pub fn render(dir: &Path, out: &Path, width: usize, height: usize) -> std::io::Result<()> {
    let text = std::fs::read_to_string(dir.join("theme.txt"))?;
    let (globals, components) = parse(&text);

    let mut canvas = Canvas::new(width, height);
    canvas.fill(
        globals
            .get("desktop-color")
            .and_then(|v| parse_hex(v))
            .unwrap_or(crate::theme::BACKDROP),
    );

    for component in &components {
        match component.kind.as_str() {
            "image" => draw_image(&mut canvas, component, dir, width, height)?,
            "label" => draw_label(&mut canvas, component, width, height),
            "boot_menu" => draw_boot_menu(&mut canvas, component, dir, width, height)?,
            "progress_bar" => draw_progress(&mut canvas, component, width, height),
            other => eprintln!("grubsim: ignoring unsupported component '{other}'"),
        }
    }

    png::write(out, &canvas)
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

fn parse(text: &str) -> (HashMap<String, String>, Vec<Component>) {
    let mut globals = HashMap::new();
    let mut components = Vec::new();
    let mut current: Option<Component> = None;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(rest) = line.strip_prefix('+') {
            let kind = rest.trim().trim_end_matches('{').trim().to_string();
            current = Some(Component {
                kind,
                props: HashMap::new(),
            });
            continue;
        }

        if line == "}" {
            if let Some(component) = current.take() {
                components.push(component);
            }
            continue;
        }

        // `key = value` inside a block, `key: value` at the top level. Both
        // take an optionally quoted value.
        let separator = if current.is_some() { '=' } else { ':' };
        if let Some((key, value)) = line.split_once(separator) {
            let value = value.trim().trim_matches('"').to_string();
            let key = key.trim().to_string();
            match &mut current {
                Some(component) => {
                    component.props.insert(key, value);
                }
                None => {
                    globals.insert(key, value);
                }
            }
        }
    }

    (globals, components)
}

/// A GRUB coordinate: `120`, `50%`, `50%-48`, `14%+110`.
fn coord(spec: Option<&str>, extent: usize) -> f32 {
    let Some(spec) = spec else { return 0.0 };
    let spec = spec.trim();

    let Some((percent, rest)) = spec.split_once('%') else {
        return spec.parse::<f32>().unwrap_or(0.0);
    };

    let mut value = percent.trim().parse::<f32>().unwrap_or(0.0) / 100.0 * extent as f32;
    let rest = rest.trim();
    if let Some(add) = rest.strip_prefix('+') {
        value += add.trim().parse::<f32>().unwrap_or(0.0);
    } else if let Some(sub) = rest.strip_prefix('-') {
        value -= sub.trim().parse::<f32>().unwrap_or(0.0);
    }
    value
}

fn parse_hex(value: &str) -> Option<Color> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(hex, 16).ok()?;
    Some(Color::from_argb(0xFF00_0000 | n))
}

/// The em size at the end of a GRUB font string, e.g. "JetBrains Mono Regular 16".
fn font_size(spec: Option<&str>) -> f32 {
    spec.and_then(|s| s.rsplit(' ').next())
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(16.0)
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

fn draw_image(
    canvas: &mut Canvas,
    component: &Component,
    dir: &Path,
    width: usize,
    height: usize,
) -> std::io::Result<()> {
    let Some(file) = component.get("file") else {
        return Ok(());
    };
    let image = png::read(&dir.join(file))?;
    let x = coord(component.get("left"), width) as i32;
    let y = coord(component.get("top"), height) as i32;
    blit(canvas, &image, x, y);
    Ok(())
}

fn draw_label(canvas: &mut Canvas, component: &Component, width: usize, height: usize) {
    let Some(text) = component.get("text") else {
        return;
    };
    let color = component
        .get("color")
        .and_then(parse_hex)
        .unwrap_or(crate::theme::TEXT);
    let face = font::face_for(font_size(component.get("font")));

    let left = coord(component.get("left"), width);
    let top = coord(component.get("top"), height);
    let box_width = component
        .get("width")
        .map_or(width as f32 - left, |w| coord(Some(w), width));

    let x = match component.get("align") {
        Some("center") => left + (box_width - face.measure(text)) / 2.0,
        Some("right") => left + box_width - face.measure(text),
        _ => left,
    };
    font::draw(canvas, face, x, top + face.ascent, text, color);
}

/// The countdown, which the theme styles as a text line with no bar.
fn draw_progress(canvas: &mut Canvas, component: &Component, width: usize, height: usize) {
    let text = match component.get("text") {
        Some("@TIMEOUT_NOTIFICATION_MIDDLE@") => TIMEOUT_MIDDLE,
        Some(other) => other,
        None => return,
    };
    let color = component
        .get("text_color")
        .and_then(parse_hex)
        .unwrap_or(crate::theme::WARNING);
    let face = font::face_for(font_size(component.get("font")));

    let left = coord(component.get("left"), width);
    let top = coord(component.get("top"), height);
    let box_width = component
        .get("width")
        .map_or(width as f32 - left, |w| coord(Some(w), width));

    // Centred unconditionally, because that is what `gui_progress_bar.c` does:
    // a progress bar has no `align` property, it draws its text in the middle
    // of the bar. Honouring `align` here made a theme that is correct look
    // wrong, which is the opposite of what this tool is for.
    let x = left + (box_width - face.measure(text)) / 2.0;
    font::draw(canvas, face, x, top + face.ascent, text, color);
}

fn draw_boot_menu(
    canvas: &mut Canvas,
    component: &Component,
    dir: &Path,
    width: usize,
    height: usize,
) -> std::io::Result<()> {
    let left = coord(component.get("left"), width);
    let top = coord(component.get("top"), height);
    let menu_width = coord(component.get("width"), width);
    let menu_height = coord(component.get("height"), height);

    let item_height = component
        .get("item_height")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(32.0);
    let item_spacing = component
        .get("item_spacing")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.0);
    let padding = component
        .get("item_padding")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.0);

    if let Some(style) = component.get("menu_pixmap_style") {
        nine_slice(
            canvas,
            dir,
            style,
            Rect::new(left, top, menu_width, menu_height),
        )?;
    }

    let face = font::face_for(font_size(component.get("item_font")));
    let color = component
        .get("item_color")
        .and_then(parse_hex)
        .unwrap_or(crate::theme::TEXT);
    let selected_color = component
        .get("selected_item_color")
        .and_then(parse_hex)
        .unwrap_or(color);

    for (index, entry) in ENTRIES.iter().enumerate() {
        let y = top + padding + index as f32 * (item_height + item_spacing);
        if y + item_height > top + menu_height {
            break;
        }

        let row = Rect::new(left + padding, y, menu_width - padding * 2.0, item_height);
        if index == SELECTED {
            if let Some(style) = component.get("selected_item_pixmap_style") {
                nine_slice(canvas, dir, style, row)?;
            }
        }

        // GRUB draws the entry's title as-is; there is no chevron and no
        // trimming of the "> " convention, which is exactly the difference
        // this render is here to show.
        let baseline = y + (item_height - face.line_height) / 2.0 + face.ascent;
        font::draw(
            canvas,
            face,
            row.x + 12.0,
            baseline,
            entry,
            if index == SELECTED {
                selected_color
            } else {
                color
            },
        );
    }

    Ok(())
}

/// Draw a `*_c.png` style set into `rect`, GRUB's way: fixed corners, tiled
/// edges, tiled centre.
fn nine_slice(canvas: &mut Canvas, dir: &Path, style: &str, rect: Rect) -> std::io::Result<()> {
    let prefix = style.trim_end_matches("*.png").trim_end_matches('_');
    let load = |suffix: &str| png::read(&dir.join(format!("{prefix}_{suffix}.png")));

    let (nw, n, ne) = (load("nw")?, load("n")?, load("ne")?);
    let (w, c, e) = (load("w")?, load("c")?, load("e")?);
    let (sw, s, se) = (load("sw")?, load("s")?, load("se")?);

    let (x, y) = (rect.x as i32, rect.y as i32);
    let (width, height) = (rect.width as i32, rect.height as i32);
    let (left, right) = (nw.width as i32, ne.width as i32);
    let (top, bottom) = (nw.height as i32, sw.height as i32);

    let inner_w = (width - left - right).max(0);
    let inner_h = (height - top - bottom).max(0);

    blit(canvas, &nw, x, y);
    blit(canvas, &ne, x + width - right, y);
    blit(canvas, &sw, x, y + height - bottom);
    blit(canvas, &se, x + width - right, y + height - bottom);

    tile(canvas, &n, x + left, y, inner_w, top);
    tile(canvas, &s, x + left, y + height - bottom, inner_w, bottom);
    tile(canvas, &w, x, y + top, left, inner_h);
    tile(canvas, &e, x + width - right, y + top, right, inner_h);
    tile(canvas, &c, x + left, y + top, inner_w, inner_h);

    Ok(())
}

fn tile(canvas: &mut Canvas, image: &png::Image, x: i32, y: i32, width: i32, height: i32) {
    if image.width == 0 || image.height == 0 {
        return;
    }
    let mut dy = 0;
    while dy < height {
        let mut dx = 0;
        while dx < width {
            blit_clipped(
                canvas,
                image,
                x + dx,
                y + dy,
                (width - dx).min(image.width as i32),
                (height - dy).min(image.height as i32),
            );
            dx += image.width as i32;
        }
        dy += image.height as i32;
    }
}

fn blit(canvas: &mut Canvas, image: &png::Image, x: i32, y: i32) {
    blit_clipped(canvas, image, x, y, image.width as i32, image.height as i32);
}

fn blit_clipped(canvas: &mut Canvas, image: &png::Image, x: i32, y: i32, width: i32, height: i32) {
    for row in 0..height {
        for col in 0..width {
            let (r, g, b) = image.pixel(col as usize, row as usize);
            canvas.blend(
                x + col,
                y + row,
                Color::from_argb(
                    0xFF00_0000 | (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b),
                ),
                1.0,
            );
        }
    }
}
