//! The look: Huginn's palette, as libadwaita named colours.
//!
//! Not invented here. Every value below is Huginn's `theme.rs` — `BACKGROUND`
//! #16161f, `BORDER` #2a2a3a, `TEXT` #d0d0e0, `ACCENT` #7aa2f7 — and the
//! mapping onto libadwaita's named colours is the one RavenSettingsUI already
//! uses, so the installer, the settings window and the panels the compositor
//! draws are one desktop rather than three opinions about a colour.
//!
//! ## Glass
//!
//! The window is translucent, and that is all it is: alpha on the background,
//! with the desktop showing through. The blur behind a Raven panel is drawn by
//! the compositor, and huginn blurs behind `wlr-layer-shell` surfaces -- the
//! launcher, the dock, quick settings -- not behind ordinary application
//! windows. So this is the same glass RavenSettingsUI has, for the same
//! reason, and its own comment says so: "Alpha only -- the blur behind it is
//! the compositor's to draw."
//!
//! ## Dark only
//!
//! There is no light variant, because there is nothing to match one against:
//! `huginn`'s theme has a single palette and it is this one. A light installer
//! would be a light window in front of a dark desktop.

use adw::prelude::*;
use gtk::gdk;

/// Huginn's `theme::ACCENT`, and the value RavenSettingsUI ships as
/// `DEFAULT_ACCENT`. Overridden by `~/.config/raven/desktop.toml`.
pub const DEFAULT_ACCENT: &str = "#7AA2F7";

/// The palette, and the layout that goes with it.
///
/// Split from the accent below because the accent is the one value a person
/// can change, and changing it must not mean rebuilding the rest.
const BASE_CSS: &str = r#"
/* Huginn's theme.rs, as libadwaita's named colours, so every stock widget
   follows without being asked. Identical to RavenSettingsUI's BASE_CSS. */
@define-color window_bg_color #16161f;
@define-color window_fg_color #d0d0e0;
@define-color headerbar_bg_color #16161f;
@define-color headerbar_fg_color #d0d0e0;
@define-color headerbar_border_color #2a2a3a;
@define-color headerbar_shade_color rgba(0,0,0,0.36);
@define-color view_bg_color #1a1a26;
@define-color view_fg_color #d0d0e0;
@define-color card_bg_color #1e1e2b;
@define-color card_fg_color #d0d0e0;
@define-color dialog_bg_color #1e1e2b;
@define-color dialog_fg_color #d0d0e0;
@define-color popover_bg_color #1e1e2b;
@define-color popover_fg_color #d0d0e0;
@define-color sidebar_bg_color #141420;
@define-color borders #2a2a3a;

window.raven { background-color: @window_bg_color; }

/* Everything between the window and the content paints nothing, so the
   window's background -- which is the half-transparent one under .glass -- is
   what shows through. A single opaque layer anywhere in here and the glass is
   a flat colour with extra steps. */
window.raven headerbar,
window.raven toolbarview,
window.raven stack,
window.raven preferencespage,
window.raven preferencespage > scrolledwindow,
window.raven preferencespage viewport,
window.raven statuspage,
window.raven statuspage > scrolledwindow,
window.raven .nav-bar {
  background-color: transparent;
}

window.raven headerbar {
  box-shadow: none;
  border-bottom: 1px solid @headerbar_border_color;
}

/* The wizard's Back/Next bar: a header bar's twin at the other end of the
   window, so the content sits between two hairlines rather than running off
   the bottom edge. */
window.raven .nav-bar { border-top: 1px solid @headerbar_border_color; }

/* Huginn's GAP is 8 and its overview RADIUS is 10; boxed lists sit at 12,
   between the two, which is what RavenSettingsUI's cards use. */
window.raven list.boxed-list {
  border-radius: 12px;
  border: 1px solid @borders;
}

/* Glass: alpha only. See the module comment for why there is no blur here. */
window.raven.glass { background-color: alpha(#16161f, 0.72); }
window.raven.glass list.boxed-list {
  background-color: alpha(#ffffff, 0.04);
  border-color: alpha(#ffffff, 0.11);
}
window.raven.glass list.boxed-list > row { background-color: transparent; }
window.raven.glass headerbar,
window.raven.glass .nav-bar { border-color: alpha(#ffffff, 0.07); }
window.raven.glass entry,
window.raven.glass dropdown > button,
window.raven.glass .log-view {
  background-color: alpha(#ffffff, 0.07);
}
window.raven.glass .card { background-color: alpha(#ffffff, 0.085); border-color: alpha(#ffffff, 0.11); }

/* The installer's own handful. */

/* The one row that says a disk is about to be erased. Not `.error`, which is
   libadwaita's red on a transparent ground: this needs to read as a block. */
window.raven .danger-row {
  background-color: alpha(#f7768e, 0.13);
  box-shadow: inset 0 0 0 1px alpha(#f7768e, 0.45);
  border-radius: 12px;
}

/* Progress. levelbar and progressbar both follow the accent, the way
   RavenSettingsUI's do. */
window.raven progressbar > trough { background-color: alpha(#ffffff, 0.08); min-height: 8px; }
window.raven progressbar > trough > progress {
  background-color: @accent_bg_color;
  min-height: 8px;
}

/* The installer's own output, behind "Details". Monospace on a panel a shade
   darker than the window, so it reads as a transcript rather than as content. */
window.raven .log-view {
  background-color: #12121a;
  border: 1px solid @borders;
  border-radius: 12px;
}
window.raven .log-view text { background-color: transparent; color: #b8b8cc; }

/* The phase checklist. A phase that has not started is dimmed by
   set_sensitive(false); this is what the one in progress looks like. */
window.raven .phase-current { color: @accent_bg_color; font-weight: 600; }

window.raven .page-title { font-size: 24px; font-weight: 700; }
window.raven .mono { font-family: monospace; }
"#;

/// What the desktop is set to, or what it is when nothing has set it.
pub struct Appearance {
    pub accent: String,
    pub glass: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            accent: DEFAULT_ACCENT.into(),
            glass: true,
        }
    }
}

/// Read `accent` and `transparency` out of `~/.config/raven/desktop.toml`.
///
/// Scanned line by line rather than parsed, and deliberately: the two values
/// wanted are a quoted string and a bool, the file is written by
/// RavenSettingsUI with `toml::to_string_pretty` so they are one per line, and
/// a TOML parser in the dependency tree of a program that reads two keys is
/// not a trade worth making. Anything it does not understand -- a multi-line
/// value, an inline table -- leaves that key at its default, which is what a
/// missing file does too.
///
/// On the live ISO there is no such file, and the defaults are Huginn's own.
pub fn read_appearance() -> Appearance {
    let mut a = Appearance::default();

    let Some(home) = std::env::var_os("HOME") else {
        return a;
    };
    let path = std::path::Path::new(&home).join(".config/raven/desktop.toml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return a;
    };

    let mut in_appearance = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_appearance = line == "[appearance]";
            continue;
        }
        if !in_appearance {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"');
        match key.trim() {
            "accent" if is_hex(value) => a.accent = value.to_string(),
            "transparency" => a.glass = value == "true",
            _ => {}
        }
    }
    a
}

fn is_hex(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Install the palette. Called once, before any window is built.
pub fn load(appearance: &Appearance) {
    let Some(display) = gdk::Display::default() else {
        return;
    };

    // Dark, always. See the module comment: there is no light Huginn to match.
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);

    let base = gtk::CssProvider::new();
    base.load_from_string(BASE_CSS);
    gtk::style_context_add_provider_for_display(
        &display,
        &base,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // A second provider at a higher priority, exactly as RavenSettingsUI does
    // it, so the accent is one small sheet layered over the palette rather
    // than a string substituted into it.
    let accent = gtk::CssProvider::new();
    accent.load_from_string(&format!(
        "@define-color accent_bg_color {a};\n\
         @define-color accent_color {a};\n\
         @define-color accent_fg_color #16161f;\n",
        a = appearance.accent
    ));
    gtk::style_context_add_provider_for_display(
        &display,
        &accent,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
    );
}

/// Mark a window as one of ours. `.raven` is the palette; `.glass` is the
/// translucency, and is what the desktop's "Window transparency" switch turns
/// off for every Raven application including this one.
pub fn apply_to_window(window: &adw::ApplicationWindow, appearance: &Appearance) {
    window.add_css_class("raven");
    if appearance.glass {
        window.add_css_class("glass");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_check() {
        assert!(is_hex("#7AA2F7"));
        assert!(is_hex("#000000"));
        assert!(!is_hex("7AA2F7"));
        assert!(!is_hex("#7AA2F"));
        assert!(!is_hex("#GGGGGG"));
    }

    #[test]
    fn the_default_accent_is_huginns() {
        // huginn's theme::ACCENT is 0xFF7AA2F7. If that ever moves, this is
        // the line that has to move with it.
        assert_eq!(DEFAULT_ACCENT, "#7AA2F7");
        assert!(is_hex(DEFAULT_ACCENT));
    }

    #[test]
    fn the_palette_is_defined_before_it_is_used() {
        for c in ["window_bg_color", "borders", "card_bg_color", "view_bg_color"] {
            assert!(
                BASE_CSS.contains(&format!("@define-color {c} ")),
                "{c} is used but never defined"
            );
        }
    }
}
