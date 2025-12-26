use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, DropDown, FileChooserAction, FileChooserNative, Label, Orientation,
    ResponseType, Scale, SpinButton, StringList, Switch, Adjustment, Entry, Window,
};
use std::sync::Arc;
use parking_lot::RwLock;

use raven_core::RavenSettings;

/// Settings category for sidebar navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    Appearance,
    Desktop,
    Panel,
    Windows,
    Input,
    Power,
    Sound,
    About,
}

impl SettingsCategory {
    pub fn all() -> &'static [SettingsCategory] {
        &[
            SettingsCategory::Appearance,
            SettingsCategory::Desktop,
            SettingsCategory::Panel,
            SettingsCategory::Windows,
            SettingsCategory::Input,
            SettingsCategory::Power,
            SettingsCategory::Sound,
            SettingsCategory::About,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            SettingsCategory::Appearance => "Appearance",
            SettingsCategory::Desktop => "Desktop",
            SettingsCategory::Panel => "Panel",
            SettingsCategory::Windows => "Windows",
            SettingsCategory::Input => "Input",
            SettingsCategory::Power => "Power",
            SettingsCategory::Sound => "Sound",
            SettingsCategory::About => "About",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            SettingsCategory::Appearance => "Theme, colors, fonts",
            SettingsCategory::Desktop => "Wallpaper, icons",
            SettingsCategory::Panel => "Position, clock, workspaces",
            SettingsCategory::Windows => "Borders, gaps, focus",
            SettingsCategory::Input => "Keyboard, mouse, touchpad",
            SettingsCategory::Power => "Screen timeout, lid action",
            SettingsCategory::Sound => "Volume, audio settings",
            SettingsCategory::About => "System information",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            SettingsCategory::Appearance => "preferences-desktop-theme-symbolic",
            SettingsCategory::Desktop => "preferences-desktop-wallpaper-symbolic",
            SettingsCategory::Panel => "preferences-desktop-display-symbolic",
            SettingsCategory::Windows => "preferences-system-windows-symbolic",
            SettingsCategory::Input => "input-keyboard-symbolic",
            SettingsCategory::Power => "system-shutdown-symbolic",
            SettingsCategory::Sound => "audio-volume-high-symbolic",
            SettingsCategory::About => "help-about-symbolic",
        }
    }
}

/// Create a setting row with label and control
fn create_setting_row(title: &str, description: Option<&str>, control: &impl IsA<gtk4::Widget>) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 16);
    row.add_css_class("settings-row");
    row.set_margin_start(16);
    row.set_margin_end(16);
    row.set_margin_top(8);
    row.set_margin_bottom(8);

    let label_box = GtkBox::new(Orientation::Vertical, 2);
    label_box.set_hexpand(true);

    let title_label = Label::new(Some(title));
    title_label.add_css_class("settings-label");
    title_label.set_halign(gtk4::Align::Start);
    label_box.append(&title_label);

    if let Some(desc) = description {
        let desc_label = Label::new(Some(desc));
        desc_label.add_css_class("settings-description");
        desc_label.set_halign(gtk4::Align::Start);
        label_box.append(&desc_label);
    }

    row.append(&label_box);
    row.append(control);

    row
}

/// Create a section title
fn create_section_title(title: &str) -> Label {
    let label = Label::new(Some(title));
    label.add_css_class("settings-section-title");
    label.set_halign(gtk4::Align::Start);
    label.set_margin_start(16);
    label.set_margin_top(16);
    label.set_margin_bottom(8);
    label
}

/// Build the Appearance settings page
pub fn build_appearance_page(settings: Arc<RwLock<RavenSettings>>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 0);
    page.add_css_class("settings-page");

    // Theme section
    page.append(&create_section_title("Theme"));

    // Theme dropdown
    let themes = StringList::new(&["Dark", "Light", "System"]);
    let theme_dropdown = DropDown::new(Some(themes), gtk4::Expression::NONE);
    theme_dropdown.add_css_class("settings-control");
    {
        let s = settings.read();
        let idx = match s.theme() {
            "light" => 1,
            "system" => 2,
            _ => 0,
        };
        theme_dropdown.set_selected(idx);
    }
    let settings_clone = settings.clone();
    theme_dropdown.connect_selected_notify(move |dd| {
        let mut s = settings_clone.write();
        let theme = match dd.selected() {
            1 => "light",
            2 => "system",
            _ => "dark",
        };
        s.set_theme(theme);
        let _ = s.save();
    });
    page.append(&create_setting_row("Theme", Some("Application color scheme"), &theme_dropdown));

    // Accent color buttons
    let colors = ["#009688", "#2196F3", "#9C27B0", "#FF5722", "#4CAF50", "#FFC107"];
    let color_box = GtkBox::new(Orientation::Horizontal, 8);
    for color in colors {
        let btn = Button::new();
        btn.add_css_class("color-button");
        btn.set_size_request(32, 32);
        // Set button color via inline style
        let css_provider = gtk4::CssProvider::new();
        css_provider.load_from_data(&format!(
            ".color-button-{} {{ background-color: {}; border-radius: 16px; min-width: 32px; min-height: 32px; }}",
            color.trim_start_matches('#'), color
        ));
        btn.add_css_class(&format!("color-button-{}", color.trim_start_matches('#')));
        if let Some(display) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &css_provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
            );
        }
        let color_str = color.to_string();
        let settings_clone = settings.clone();
        btn.connect_clicked(move |_| {
            let mut s = settings_clone.write();
            s.set_accent_color(&color_str);
            let _ = s.save();
        });
        color_box.append(&btn);
    }
    page.append(&create_setting_row("Accent Color", Some("Primary UI accent color"), &color_box));

    // Font size
    let adj = Adjustment::new(14.0, 10.0, 24.0, 1.0, 1.0, 0.0);
    let font_spin = SpinButton::new(Some(&adj), 1.0, 0);
    font_spin.add_css_class("settings-control");
    {
        let s = settings.read();
        font_spin.set_value(s.font_size() as f64);
    }
    let settings_clone = settings.clone();
    font_spin.connect_value_changed(move |spin| {
        let mut s = settings_clone.write();
        s.set_font_size(spin.value() as i32);
        let _ = s.save();
    });
    page.append(&create_setting_row("Font Size", Some("Base font size in pixels"), &font_spin));

    // Panel opacity
    let opacity_adj = Adjustment::new(75.0, 0.0, 100.0, 1.0, 10.0, 0.0);
    let opacity_scale = Scale::new(Orientation::Horizontal, Some(&opacity_adj));
    opacity_scale.set_size_request(200, -1);
    opacity_scale.add_css_class("settings-control");
    {
        let s = settings.read();
        opacity_scale.set_value(s.panel_opacity());
    }
    let settings_clone = settings.clone();
    opacity_scale.connect_value_changed(move |scale| {
        let mut s = settings_clone.write();
        s.set_panel_opacity(scale.value());
        let _ = s.save();
    });
    page.append(&create_setting_row("Panel Opacity", Some("Transparency of the panel"), &opacity_scale));

    // Enable animations
    let anim_switch = Switch::new();
    anim_switch.add_css_class("settings-control");
    {
        let s = settings.read();
        anim_switch.set_active(s.enable_animations());
    }
    let settings_clone = settings.clone();
    anim_switch.connect_state_set(move |_, state| {
        let mut s = settings_clone.write();
        s.set_enable_animations(state);
        let _ = s.save();
        glib::Propagation::Proceed
    });
    page.append(&create_setting_row("Enable Animations", Some("UI transition animations"), &anim_switch));

    page
}

/// Build the Desktop settings page
pub fn build_desktop_page(settings: Arc<RwLock<RavenSettings>>, window: &Window) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 0);
    page.add_css_class("settings-page");

    page.append(&create_section_title("Wallpaper"));

    // Wallpaper path with browse button
    let path_box = GtkBox::new(Orientation::Horizontal, 8);
    let path_entry = Entry::new();
    path_entry.set_hexpand(true);
    path_entry.add_css_class("settings-control");
    {
        let s = settings.read();
        path_entry.set_text(s.wallpaper_path());
    }
    let settings_clone = settings.clone();
    let entry_clone = path_entry.clone();
    path_entry.connect_changed(move |entry| {
        let mut s = settings_clone.write();
        s.set_wallpaper_path(&entry.text());
        let _ = s.save();
    });

    let browse_btn = Button::with_label("Browse");
    browse_btn.add_css_class("settings-button");
    let settings_clone = settings.clone();
    let win = window.clone();
    browse_btn.connect_clicked(move |_| {
        let dialog = FileChooserNative::new(
            Some("Select Wallpaper"),
            Some(&win),
            FileChooserAction::Open,
            Some("Select"),
            Some("Cancel"),
        );

        let filter = gtk4::FileFilter::new();
        filter.set_name(Some("Images"));
        filter.add_mime_type("image/*");
        dialog.add_filter(&filter);

        let settings_ref = settings_clone.clone();
        let entry_ref = entry_clone.clone();
        dialog.connect_response(move |dialog, response| {
            if response == ResponseType::Accept {
                if let Some(file) = dialog.file() {
                    if let Some(path) = file.path() {
                        let path_str = path.to_string_lossy().to_string();
                        entry_ref.set_text(&path_str);
                        let mut s = settings_ref.write();
                        s.set_wallpaper_path(&path_str);
                        let _ = s.save();
                    }
                }
            }
        });
        dialog.show();
    });

    path_box.append(&path_entry);
    path_box.append(&browse_btn);
    page.append(&create_setting_row("Wallpaper Path", Some("Path to wallpaper image"), &path_box));

    // Wallpaper mode
    let modes = StringList::new(&["Fill", "Fit", "Stretch", "Center", "Tile"]);
    let mode_dropdown = DropDown::new(Some(modes), gtk4::Expression::NONE);
    mode_dropdown.add_css_class("settings-control");
    {
        let s = settings.read();
        let idx = match s.wallpaper_mode() {
            "fit" => 1,
            "stretch" => 2,
            "center" => 3,
            "tile" => 4,
            _ => 0,
        };
        mode_dropdown.set_selected(idx);
    }
    let settings_clone = settings.clone();
    mode_dropdown.connect_selected_notify(move |dd| {
        let mut s = settings_clone.write();
        let mode = match dd.selected() {
            1 => "fit",
            2 => "stretch",
            3 => "center",
            4 => "tile",
            _ => "fill",
        };
        s.set_wallpaper_mode(mode);
        let _ = s.save();
    });
    page.append(&create_setting_row("Wallpaper Mode", Some("How to display the wallpaper"), &mode_dropdown));

    // Show desktop icons
    let icons_switch = Switch::new();
    icons_switch.add_css_class("settings-control");
    {
        let s = settings.read();
        icons_switch.set_active(s.show_desktop_icons());
    }
    let settings_clone = settings.clone();
    icons_switch.connect_state_set(move |_, state| {
        let mut s = settings_clone.write();
        s.set_show_desktop_icons(state);
        let _ = s.save();
        glib::Propagation::Proceed
    });
    page.append(&create_setting_row("Show Desktop Icons", Some("Display icons on desktop"), &icons_switch));

    page
}

/// Build the Panel settings page
pub fn build_panel_page(settings: Arc<RwLock<RavenSettings>>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 0);
    page.add_css_class("settings-page");

    page.append(&create_section_title("Panel"));

    // Panel position
    let positions = StringList::new(&["Top", "Bottom", "Left", "Right"]);
    let pos_dropdown = DropDown::new(Some(positions), gtk4::Expression::NONE);
    pos_dropdown.add_css_class("settings-control");
    {
        let s = settings.read();
        let idx = match s.panel_position_str() {
            "bottom" => 1,
            "left" => 2,
            "right" => 3,
            _ => 0,
        };
        pos_dropdown.set_selected(idx);
    }
    let settings_clone = settings.clone();
    pos_dropdown.connect_selected_notify(move |dd| {
        let mut s = settings_clone.write();
        let pos = match dd.selected() {
            1 => "bottom",
            2 => "left",
            3 => "right",
            _ => "top",
        };
        s.set_panel_position(pos);
        let _ = s.save();
    });
    page.append(&create_setting_row("Panel Position", Some("Edge of screen for panel"), &pos_dropdown));

    // Panel height
    let height_adj = Adjustment::new(38.0, 24.0, 64.0, 1.0, 4.0, 0.0);
    let height_spin = SpinButton::new(Some(&height_adj), 1.0, 0);
    height_spin.add_css_class("settings-control");
    {
        let s = settings.read();
        height_spin.set_value(s.panel_height as f64);
    }
    let settings_clone = settings.clone();
    height_spin.connect_value_changed(move |spin| {
        let mut s = settings_clone.write();
        s.set_panel_height(spin.value() as i32);
        let _ = s.save();
    });
    page.append(&create_setting_row("Panel Height", Some("Height in pixels"), &height_spin));

    page.append(&create_section_title("Clock"));

    // Show clock
    let clock_switch = Switch::new();
    clock_switch.add_css_class("settings-control");
    {
        let s = settings.read();
        clock_switch.set_active(s.show_clock());
    }
    let settings_clone = settings.clone();
    clock_switch.connect_state_set(move |_, state| {
        let mut s = settings_clone.write();
        s.set_show_clock(state);
        let _ = s.save();
        glib::Propagation::Proceed
    });
    page.append(&create_setting_row("Show Clock", Some("Display clock in panel"), &clock_switch));

    // Clock format
    let formats = StringList::new(&["24-hour", "12-hour"]);
    let format_dropdown = DropDown::new(Some(formats), gtk4::Expression::NONE);
    format_dropdown.add_css_class("settings-control");
    {
        let s = settings.read();
        let idx = if s.clock_format().contains("12") || s.clock_format().contains("%I") { 1 } else { 0 };
        format_dropdown.set_selected(idx);
    }
    let settings_clone = settings.clone();
    format_dropdown.connect_selected_notify(move |dd| {
        let mut s = settings_clone.write();
        let fmt = if dd.selected() == 1 { "%I:%M %p" } else { "%H:%M" };
        s.set_clock_format(fmt);
        let _ = s.save();
    });
    page.append(&create_setting_row("Clock Format", Some("Time display format"), &format_dropdown));

    page.append(&create_section_title("Workspaces"));

    // Show workspaces
    let ws_switch = Switch::new();
    ws_switch.add_css_class("settings-control");
    {
        let s = settings.read();
        ws_switch.set_active(s.show_workspaces());
    }
    let settings_clone = settings.clone();
    ws_switch.connect_state_set(move |_, state| {
        let mut s = settings_clone.write();
        s.set_show_workspaces(state);
        let _ = s.save();
        glib::Propagation::Proceed
    });
    page.append(&create_setting_row("Show Workspaces", Some("Display workspace indicator"), &ws_switch));

    page
}

/// Build the Windows settings page
pub fn build_windows_page(settings: Arc<RwLock<RavenSettings>>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 0);
    page.add_css_class("settings-page");

    page.append(&create_section_title("Window Decoration"));

    // Border width
    let border_adj = Adjustment::new(2.0, 0.0, 10.0, 1.0, 1.0, 0.0);
    let border_spin = SpinButton::new(Some(&border_adj), 1.0, 0);
    border_spin.add_css_class("settings-control");
    {
        let s = settings.read();
        border_spin.set_value(s.border_width() as f64);
    }
    let settings_clone = settings.clone();
    border_spin.connect_value_changed(move |spin| {
        let mut s = settings_clone.write();
        s.set_border_width(spin.value() as i32);
        let _ = s.save();
    });
    page.append(&create_setting_row("Border Width", Some("Window border thickness"), &border_spin));

    // Gap size
    let gap_adj = Adjustment::new(8.0, 0.0, 32.0, 1.0, 4.0, 0.0);
    let gap_spin = SpinButton::new(Some(&gap_adj), 1.0, 0);
    gap_spin.add_css_class("settings-control");
    {
        let s = settings.read();
        gap_spin.set_value(s.gap_size() as f64);
    }
    let settings_clone = settings.clone();
    gap_spin.connect_value_changed(move |spin| {
        let mut s = settings_clone.write();
        s.set_gap_size(spin.value() as i32);
        let _ = s.save();
    });
    page.append(&create_setting_row("Gap Size", Some("Space between windows"), &gap_spin));

    page.append(&create_section_title("Focus"));

    // Focus follows mouse
    let focus_switch = Switch::new();
    focus_switch.add_css_class("settings-control");
    {
        let s = settings.read();
        focus_switch.set_active(s.focus_follows_mouse());
    }
    let settings_clone = settings.clone();
    focus_switch.connect_state_set(move |_, state| {
        let mut s = settings_clone.write();
        s.set_focus_follows_mouse(state);
        let _ = s.save();
        glib::Propagation::Proceed
    });
    page.append(&create_setting_row("Focus Follows Mouse", Some("Focus window on hover"), &focus_switch));

    page
}

/// Build the Input settings page
pub fn build_input_page(settings: Arc<RwLock<RavenSettings>>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 0);
    page.add_css_class("settings-page");

    page.append(&create_section_title("Keyboard"));

    // Keyboard layout
    let layouts = StringList::new(&["US", "UK", "DE", "FR", "ES", "IT", "RU", "JP"]);
    let layout_dropdown = DropDown::new(Some(layouts), gtk4::Expression::NONE);
    layout_dropdown.add_css_class("settings-control");
    {
        let s = settings.read();
        let idx = match s.keyboard_layout().to_uppercase().as_str() {
            "UK" => 1,
            "DE" => 2,
            "FR" => 3,
            "ES" => 4,
            "IT" => 5,
            "RU" => 6,
            "JP" => 7,
            _ => 0,
        };
        layout_dropdown.set_selected(idx);
    }
    let settings_clone = settings.clone();
    layout_dropdown.connect_selected_notify(move |dd| {
        let mut s = settings_clone.write();
        let layout = match dd.selected() {
            1 => "uk",
            2 => "de",
            3 => "fr",
            4 => "es",
            5 => "it",
            6 => "ru",
            7 => "jp",
            _ => "us",
        };
        s.set_keyboard_layout(layout);
        let _ = s.save();
    });
    page.append(&create_setting_row("Keyboard Layout", Some("Input language layout"), &layout_dropdown));

    page.append(&create_section_title("Mouse"));

    // Mouse speed
    let speed_adj = Adjustment::new(50.0, 0.0, 100.0, 1.0, 10.0, 0.0);
    let speed_scale = Scale::new(Orientation::Horizontal, Some(&speed_adj));
    speed_scale.set_size_request(200, -1);
    speed_scale.add_css_class("settings-control");
    {
        let s = settings.read();
        speed_scale.set_value(s.mouse_speed());
    }
    let settings_clone = settings.clone();
    speed_scale.connect_value_changed(move |scale| {
        let mut s = settings_clone.write();
        s.set_mouse_speed(scale.value());
        let _ = s.save();
    });
    page.append(&create_setting_row("Mouse Speed", Some("Pointer acceleration"), &speed_scale));

    page.append(&create_section_title("Touchpad"));

    // Natural scrolling
    let scroll_switch = Switch::new();
    scroll_switch.add_css_class("settings-control");
    {
        let s = settings.read();
        scroll_switch.set_active(s.touchpad_natural_scroll());
    }
    let settings_clone = settings.clone();
    scroll_switch.connect_state_set(move |_, state| {
        let mut s = settings_clone.write();
        s.set_touchpad_natural_scroll(state);
        let _ = s.save();
        glib::Propagation::Proceed
    });
    page.append(&create_setting_row("Natural Scrolling", Some("Reverse scroll direction"), &scroll_switch));

    // Tap to click
    let tap_switch = Switch::new();
    tap_switch.add_css_class("settings-control");
    {
        let s = settings.read();
        tap_switch.set_active(s.touchpad_tap_to_click());
    }
    let settings_clone = settings.clone();
    tap_switch.connect_state_set(move |_, state| {
        let mut s = settings_clone.write();
        s.set_touchpad_tap_to_click(state);
        let _ = s.save();
        glib::Propagation::Proceed
    });
    page.append(&create_setting_row("Tap to Click", Some("Tap touchpad to click"), &tap_switch));

    page
}

/// Build the Power settings page
pub fn build_power_page(settings: Arc<RwLock<RavenSettings>>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 0);
    page.add_css_class("settings-page");

    page.append(&create_section_title("Display"));

    // Screen timeout
    let timeouts = StringList::new(&["Never", "1 minute", "5 minutes", "10 minutes", "15 minutes", "30 minutes"]);
    let timeout_dropdown = DropDown::new(Some(timeouts), gtk4::Expression::NONE);
    timeout_dropdown.add_css_class("settings-control");
    {
        let s = settings.read();
        let idx = match s.screen_timeout() {
            0 => 0,
            60 => 1,
            300 => 2,
            600 => 3,
            900 => 4,
            _ => 5,
        };
        timeout_dropdown.set_selected(idx);
    }
    let settings_clone = settings.clone();
    timeout_dropdown.connect_selected_notify(move |dd| {
        let mut s = settings_clone.write();
        let timeout = match dd.selected() {
            0 => 0,
            1 => 60,
            2 => 300,
            3 => 600,
            4 => 900,
            _ => 1800,
        };
        s.set_screen_timeout(timeout);
        let _ = s.save();
    });
    page.append(&create_setting_row("Screen Timeout", Some("Turn off display after"), &timeout_dropdown));

    page.append(&create_section_title("Suspend"));

    // Suspend timeout
    let suspend_times = StringList::new(&["Never", "5 minutes", "15 minutes", "30 minutes", "1 hour", "2 hours"]);
    let suspend_dropdown = DropDown::new(Some(suspend_times), gtk4::Expression::NONE);
    suspend_dropdown.add_css_class("settings-control");
    {
        let s = settings.read();
        let idx = match s.suspend_timeout() {
            0 => 0,
            300 => 1,
            900 => 2,
            1800 => 3,
            3600 => 4,
            _ => 5,
        };
        suspend_dropdown.set_selected(idx);
    }
    let settings_clone = settings.clone();
    suspend_dropdown.connect_selected_notify(move |dd| {
        let mut s = settings_clone.write();
        let timeout = match dd.selected() {
            0 => 0,
            1 => 300,
            2 => 900,
            3 => 1800,
            4 => 3600,
            _ => 7200,
        };
        s.set_suspend_timeout(timeout);
        let _ = s.save();
    });
    page.append(&create_setting_row("Suspend Timeout", Some("Suspend system after"), &suspend_dropdown));

    // Lid close action
    let actions = StringList::new(&["Suspend", "Hibernate", "Power Off", "Do Nothing"]);
    let action_dropdown = DropDown::new(Some(actions), gtk4::Expression::NONE);
    action_dropdown.add_css_class("settings-control");
    {
        let s = settings.read();
        let idx = match s.lid_close_action() {
            "hibernate" => 1,
            "poweroff" => 2,
            "nothing" => 3,
            _ => 0,
        };
        action_dropdown.set_selected(idx);
    }
    let settings_clone = settings.clone();
    action_dropdown.connect_selected_notify(move |dd| {
        let mut s = settings_clone.write();
        let action = match dd.selected() {
            1 => "hibernate",
            2 => "poweroff",
            3 => "nothing",
            _ => "suspend",
        };
        s.set_lid_close_action(action);
        let _ = s.save();
    });
    page.append(&create_setting_row("Lid Close Action", Some("Action when laptop lid closes"), &action_dropdown));

    page
}

/// Build the Sound settings page
pub fn build_sound_page(settings: Arc<RwLock<RavenSettings>>) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 0);
    page.add_css_class("settings-page");

    page.append(&create_section_title("Volume"));

    // Master volume
    let vol_adj = Adjustment::new(75.0, 0.0, 100.0, 1.0, 10.0, 0.0);
    let vol_scale = Scale::new(Orientation::Horizontal, Some(&vol_adj));
    vol_scale.set_size_request(200, -1);
    vol_scale.add_css_class("settings-control");
    {
        let s = settings.read();
        vol_scale.set_value(s.master_volume() as f64);
    }
    let settings_clone = settings.clone();
    vol_scale.connect_value_changed(move |scale| {
        let mut s = settings_clone.write();
        s.set_master_volume(scale.value() as i32);
        let _ = s.save();
        // Apply volume change
        let vol = scale.value() as u32;
        std::process::Command::new("wpctl")
            .args(["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{}%", vol)])
            .spawn()
            .ok();
    });
    page.append(&create_setting_row("Master Volume", Some("System audio volume"), &vol_scale));

    // Mute on lock
    let mute_switch = Switch::new();
    mute_switch.add_css_class("settings-control");
    {
        let s = settings.read();
        mute_switch.set_active(s.mute_on_lock());
    }
    let settings_clone = settings.clone();
    mute_switch.connect_state_set(move |_, state| {
        let mut s = settings_clone.write();
        s.set_mute_on_lock(state);
        let _ = s.save();
        glib::Propagation::Proceed
    });
    page.append(&create_setting_row("Mute on Lock", Some("Mute audio when screen locks"), &mute_switch));

    page.append(&create_section_title("Test"));

    // Test audio button
    let test_btn = Button::with_label("Test Audio");
    test_btn.add_css_class("settings-button");
    test_btn.connect_clicked(|_| {
        std::process::Command::new("paplay")
            .arg("/usr/share/sounds/freedesktop/stereo/bell.oga")
            .spawn()
            .ok();
    });
    page.append(&create_setting_row("Audio Test", Some("Play a test sound"), &test_btn));

    page
}

/// Build the About page
pub fn build_about_page() -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 16);
    page.add_css_class("settings-page");
    page.add_css_class("about-page");
    page.set_halign(gtk4::Align::Center);
    page.set_valign(gtk4::Align::Center);

    // Logo/branding
    let logo = Label::new(Some("RAVEN"));
    logo.add_css_class("about-logo");
    page.append(&logo);

    // Version
    let version = Label::new(Some("Version 1.0.0"));
    version.add_css_class("about-version");
    page.append(&version);

    // Description
    let desc = Label::new(Some("A modern desktop environment for Hyprland"));
    desc.add_css_class("about-description");
    page.append(&desc);

    // System info section
    let info_box = GtkBox::new(Orientation::Vertical, 8);
    info_box.add_css_class("about-info");
    info_box.set_margin_top(24);

    // Hostname
    if let Ok(hostname) = std::fs::read_to_string("/etc/hostname") {
        let host_label = Label::new(Some(&format!("Host: {}", hostname.trim())));
        host_label.add_css_class("about-info-item");
        info_box.append(&host_label);
    }

    // Kernel version
    if let Ok(output) = std::process::Command::new("uname").arg("-r").output() {
        if let Ok(kernel) = String::from_utf8(output.stdout) {
            let kernel_label = Label::new(Some(&format!("Kernel: {}", kernel.trim())));
            kernel_label.add_css_class("about-info-item");
            info_box.append(&kernel_label);
        }
    }

    // Desktop
    let desktop_label = Label::new(Some("Desktop: Hyprland"));
    desktop_label.add_css_class("about-info-item");
    info_box.append(&desktop_label);

    page.append(&info_box);

    // Links
    let links_box = GtkBox::new(Orientation::Horizontal, 16);
    links_box.set_halign(gtk4::Align::Center);
    links_box.set_margin_top(24);

    let website_btn = Button::with_label("Website");
    website_btn.add_css_class("settings-button");
    website_btn.add_css_class("about-link");
    website_btn.connect_clicked(|_| {
        std::process::Command::new("xdg-open")
            .arg("https://github.com/javanhut/RavenLinux")
            .spawn()
            .ok();
    });
    links_box.append(&website_btn);

    let docs_btn = Button::with_label("Documentation");
    docs_btn.add_css_class("settings-button");
    docs_btn.add_css_class("about-link");
    docs_btn.connect_clicked(|_| {
        std::process::Command::new("xdg-open")
            .arg("https://github.com/javanhut/RavenLinux/wiki")
            .spawn()
            .ok();
    });
    links_box.append(&docs_btn);

    page.append(&links_box);

    page
}
