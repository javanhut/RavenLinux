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
//! ## Light and dark
//!
//! Dark is Huginn's palette, below. Light is the desktop's other mode: when
//! RavenSettingsUI's `theme_mode` is `light`, `LIGHT_CSS` is layered over the
//! palette with the same named colours RavenSettingsUI's
//! `raven-glass-light.css` uses, so a light desktop gets a light installer.
//! `auto` is dark, as it is everywhere in Raven. On the live ISO there is no
//! desktop file at all, and the installer is dark.
//!
//! The file is followed while the installer runs (`watch`), which costs one
//! directory monitor; an installer is short-lived, but it is also the first
//! thing a person sees after changing the look in Settings on a live session.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::{gdk, gio, glib};

/// Huginn's `theme::ACCENT`, and the value RavenSettingsUI ships as
/// `DEFAULT_ACCENT`. Overridden by `~/.config/raven/desktop.toml`.
pub const DEFAULT_ACCENT: &str = "#7AA2F7";

/// The palette, and the layout that goes with it.
///
/// Split from the accent below because the accent is the one value a person
/// can change, and changing it must not mean rebuilding the rest.
const BASE_CSS: &str = r#"
/* Huginn's theme.rs, as libadwaita's named colours, so every stock widget
   follows without being asked. The same values RavenSettingsUI started from;
   Settings now ships the fuller raven-glass.css, which this does not embed --
   this is the installer's own, smaller sheet in the same palette. */
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

/// Layered over `BASE_CSS` when the desktop is light: RavenSettingsUI's
/// `raven-glass-light.css` named colours, and light twins of every
/// white-on-dark alpha the base sheet hardcodes.
const LIGHT_CSS: &str = r#"
@define-color window_bg_color #f2f2f7;
@define-color window_fg_color #1c1c22;
@define-color headerbar_bg_color #f2f2f7;
@define-color headerbar_fg_color #1c1c22;
@define-color headerbar_border_color alpha(#000000, 0.08);
@define-color headerbar_shade_color alpha(#000000, 0.07);
@define-color view_bg_color #ffffff;
@define-color view_fg_color #1c1c22;
@define-color card_bg_color alpha(#ffffff, 0.70);
@define-color card_fg_color #1c1c22;
@define-color dialog_bg_color #f7f7fa;
@define-color dialog_fg_color #1c1c22;
@define-color popover_bg_color #ffffff;
@define-color popover_fg_color #1c1c22;
@define-color sidebar_bg_color #e9e9ef;
@define-color borders alpha(#000000, 0.09);

window.raven.glass { background-color: alpha(#f2f2f7, 0.88); }
window.raven.glass list.boxed-list {
  background-color: alpha(#ffffff, 0.72);
  border-color: alpha(#000000, 0.07);
}
window.raven.glass headerbar,
window.raven.glass .nav-bar { border-color: alpha(#000000, 0.07); }
window.raven.glass entry,
window.raven.glass dropdown > button,
window.raven.glass .log-view {
  background-color: alpha(#ffffff, 0.85);
}
window.raven.glass .card { background-color: alpha(#ffffff, 0.72); border-color: alpha(#000000, 0.07); }
window.raven .danger-row { background-color: alpha(#f7768e, 0.16); }
window.raven progressbar > trough { background-color: alpha(#000000, 0.10); }
window.raven .log-view { background-color: #fafafc; }
window.raven .log-view text { color: #3a3a46; }
"#;

/// `[appearance] theme_mode` in `desktop.toml`. Auto is dark, as it is
/// everywhere in Raven.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    Light,
    #[default]
    Dark,
    Auto,
}

/// What the desktop is set to, or what it is when nothing has set it.
#[derive(Debug, Clone, PartialEq)]
pub struct Appearance {
    pub theme_mode: ThemeMode,
    pub accent: String,
    pub glass: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::Dark,
            accent: DEFAULT_ACCENT.into(),
            glass: true,
        }
    }
}

/// `$XDG_CONFIG_HOME/raven/desktop.toml`, else `~/.config/raven/desktop.toml`.
fn desktop_path() -> Option<std::path::PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".config")))
        .map(|c| c.join("raven/desktop.toml"))
}

/// Read `theme_mode`, `accent` and `transparency` out of the desktop file.
///
/// On the live ISO there is no such file, and the defaults are Huginn's own.
pub fn read_appearance() -> Appearance {
    desktop_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|text| parse_appearance(&text))
        .unwrap_or_default()
}

/// Scanned line by line rather than parsed, and deliberately: the three
/// values wanted are two quoted strings and a bool, the file is written by
/// RavenSettingsUI with `toml::to_string_pretty` so they are one per line, and
/// a TOML parser in the dependency tree of a program that reads three keys is
/// not a trade worth making. Anything it does not understand -- a multi-line
/// value, an inline table, an unknown mode -- leaves that key at its default,
/// which is what a missing file does too.
fn parse_appearance(text: &str) -> Appearance {
    let mut a = Appearance::default();
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
            "theme_mode" => match value {
                "light" => a.theme_mode = ThemeMode::Light,
                "dark" => a.theme_mode = ThemeMode::Dark,
                "auto" => a.theme_mode = ThemeMode::Auto,
                _ => {}
            },
            _ => {}
        }
    }
    a
}

fn is_hex(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

thread_local! {
    /// The accent-and-mode sheet, kept so a change replaces it rather than
    /// stacking another on top.
    static OVERLAY: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
    static DESKTOP_MONITOR: RefCell<Option<gio::FileMonitor>> = const { RefCell::new(None) };
}

/// How long the desktop file has to stay quiet before it is re-read.
/// RavenSettingsUI writes it by rename, which arrives as a burst of events.
const DESKTOP_SETTLE: Duration = Duration::from_millis(150);

/// Install the palette. Called once, before any window is built.
pub fn load(appearance: &Appearance) {
    let Some(display) = gdk::Display::default() else {
        return;
    };

    let base = gtk::CssProvider::new();
    base.load_from_string(BASE_CSS);
    gtk::style_context_add_provider_for_display(
        &display,
        &base,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    apply_mode(appearance);
}

/// The light/dark scheme and the accent. A second provider at a higher
/// priority, exactly as RavenSettingsUI does it, so the accent (and, when
/// light, `LIGHT_CSS`) is one small sheet layered over the palette rather than
/// a string substituted into it. Replaced, never stacked, on every call.
fn apply_mode(appearance: &Appearance) {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    adw::StyleManager::default().set_color_scheme(match appearance.theme_mode {
        ThemeMode::Dark => adw::ColorScheme::ForceDark,
        ThemeMode::Light => adw::ColorScheme::ForceLight,
        ThemeMode::Auto => adw::ColorScheme::PreferDark,
    });
    let css = overlay_css(appearance);
    OVERLAY.with(|slot| {
        if let Some(old) = slot.borrow_mut().take() {
            gtk::style_context_remove_provider_for_display(&display, &old);
        }
        let provider = gtk::CssProvider::new();
        provider.load_from_string(&css);
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
        );
        *slot.borrow_mut() = Some(provider);
    });
}

fn overlay_css(appearance: &Appearance) -> String {
    format!(
        "@define-color accent_bg_color {a};\n\
         @define-color accent_color {a};\n\
         @define-color accent_fg_color #16161f;\n{light}",
        a = appearance.accent,
        light = if appearance.theme_mode == ThemeMode::Light {
            LIGHT_CSS
        } else {
            ""
        },
    )
}

/// Mark a window as one of ours. `.raven` is the palette; `.glass` is the
/// translucency, and is what the desktop's "Window transparency" switch turns
/// off for every Raven application including this one.
pub fn apply_to_window(window: &adw::ApplicationWindow, appearance: &Appearance) {
    window.add_css_class("raven");
    if appearance.glass {
        window.add_css_class("glass");
    } else {
        window.remove_css_class("glass");
    }
}

/// Follow `desktop.toml` while `window` is open: mode, accent and glass all
/// change in place. The directory is watched rather than the file, because
/// the file may not exist yet and RavenSettingsUI replaces it by rename.
pub fn watch(window: &adw::ApplicationWindow) {
    let Some(path) = desktop_path() else {
        return;
    };
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
        return;
    };
    let name = name.to_os_string();
    let Ok(monitor) = gio::File::for_path(dir)
        .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
    else {
        return;
    };
    let window = window.downgrade();
    let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    monitor.connect_changed(move |_, file, other, event| {
        if matches!(
            event,
            gio::FileMonitorEvent::AttributeChanged
                | gio::FileMonitorEvent::PreUnmount
                | gio::FileMonitorEvent::Unmounted
        ) {
            return;
        }
        let names_desktop = |f: Option<&gio::File>| {
            f.and_then(|f| f.basename())
                .is_some_and(|b| b.as_os_str() == name.as_os_str())
        };
        if !names_desktop(Some(file)) && !names_desktop(other) {
            return;
        }
        if let Some(id) = pending.borrow_mut().take() {
            id.remove();
        }
        let fired = pending.clone();
        let window = window.clone();
        let id = glib::timeout_add_local_once(DESKTOP_SETTLE, move || {
            fired.borrow_mut().take();
            let appearance = read_appearance();
            apply_mode(&appearance);
            if let Some(window) = window.upgrade() {
                apply_to_window(&window, &appearance);
            }
        });
        *pending.borrow_mut() = Some(id);
    });
    DESKTOP_MONITOR.with(|m| *m.borrow_mut() = Some(monitor));
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
    fn appearance_is_read_from_its_section_only() {
        let a = parse_appearance(
            "[appearance]\ntheme_mode = \"light\"\naccent = \"#F7768E\"\ntransparency = false\n\
             [panel]\ntheme_mode = \"dark\"\n",
        );
        assert_eq!(a.theme_mode, ThemeMode::Light);
        assert_eq!(a.accent, "#F7768E");
        assert!(!a.glass);
        assert!(overlay_css(&a).contains("window_bg_color #f2f2f7"));

        // Unknown or bad values keep the defaults; auto is its own mode.
        let a = parse_appearance("[appearance]\ntheme_mode = \"sepia\"\naccent = \"red\"\n");
        assert_eq!(a, Appearance::default());
        assert!(!overlay_css(&a).contains("window_bg_color"));
        let a = parse_appearance("[appearance]\ntheme_mode = \"auto\"\n");
        assert_eq!(a.theme_mode, ThemeMode::Auto);
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
