// Test binary for desktop components
// Usage: cargo run --bin test-components -- [menu|power|settings|keybindings|filemanager]

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Label, Window,
    ListBox, ListBoxRow, Orientation, ScrolledWindow, SearchEntry,
    Align,
};
use raven_core::theme::load_css;
use std::env;

fn main() {
    // Initialize GTK without Application (avoids DBus registration)
    gtk4::init().expect("Failed to initialize GTK4");

    load_css();

    let args: Vec<String> = env::args().collect();
    let component = args.get(1).map(|s| s.as_str()).unwrap_or("menu");

    match component {
        "menu" => create_menu_test(),
        "power" => create_power_test(),
        "settings" => create_settings_test(),
        "keybindings" => create_keybindings_test(),
        "filemanager" => create_filemanager_test(),
        _ => {
            eprintln!("Unknown component: {}", component);
            eprintln!("Usage: test-components [menu|power|settings|keybindings|filemanager]");
            std::process::exit(1);
        }
    }

    // Run the GTK main loop
    let main_loop = glib::MainLoop::new(None, false);
    main_loop.run();
}

fn create_menu_test() {
    use std::cell::RefCell;
    use std::rc::Rc;

    println!("Creating Menu test window...");

    let window = Window::builder()
        .title("Raven Menu")
        .default_width(450)
        .default_height(600)
        .build();

    window.connect_close_request(|_| {
        std::process::exit(0);
        #[allow(unreachable_code)]
        glib::Propagation::Proceed
    });

    let main_box = GtkBox::new(Orientation::Vertical, 0);
    main_box.add_css_class("menu-container");

    // Header
    let header = GtkBox::new(Orientation::Horizontal, 8);
    header.set_margin_start(16);
    header.set_margin_end(16);
    header.set_margin_top(16);
    header.set_margin_bottom(8);

    let title = Label::new(Some("Applications"));
    title.add_css_class("menu-header");
    header.append(&title);
    main_box.append(&header);

    // Search
    let search = SearchEntry::new();
    search.set_placeholder_text(Some("Search applications..."));
    search.add_css_class("menu-search");
    search.set_margin_start(16);
    search.set_margin_end(16);
    search.set_margin_bottom(8);
    main_box.append(&search);

    // Content area
    let content = GtkBox::new(Orientation::Horizontal, 0);
    content.set_vexpand(true);

    // Sample apps with categories: (name, description, category)
    let sample_apps: Vec<(&str, &str, &str)> = vec![
        ("Firefox", "Web Browser", "Internet"),
        ("Chromium", "Web Browser", "Internet"),
        ("Thunderbird", "Email Client", "Internet"),
        ("Terminal", "Command Line", "System"),
        ("Files", "File Manager", "System"),
        ("Settings", "System Settings", "System"),
        ("System Monitor", "View System Resources", "System"),
        ("Disk Usage", "Analyze Disk Space", "System"),
        ("Text Editor", "Edit Text Files", "Utilities"),
        ("Calculator", "Perform Calculations", "Utilities"),
        ("Archive Manager", "Compress Files", "Utilities"),
        ("Screenshot", "Capture Screen", "Utilities"),
        ("Character Map", "Special Characters", "Utilities"),
        ("VS Code", "Code Editor", "Development"),
        ("GNOME Builder", "IDE", "Development"),
        ("Meld", "Diff Viewer", "Development"),
        ("Image Viewer", "View Images", "Graphics"),
        ("GIMP", "Image Editor", "Graphics"),
        ("Inkscape", "Vector Graphics", "Graphics"),
        ("Font Viewer", "View Fonts", "Graphics"),
        ("Music Player", "Play Music", "Office"),
        ("Video Player", "Watch Videos", "Office"),
        ("LibreOffice Writer", "Word Processor", "Office"),
    ];

    // Shared state for filtering
    let current_category: Rc<RefCell<String>> = Rc::new(RefCell::new("All".to_string()));
    let current_search: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    let apps_data: Rc<Vec<(&str, &str, &str)>> = Rc::new(sample_apps);

    // Category sidebar using ListBox
    let category_list = ListBox::new();
    category_list.add_css_class("menu-sidebar");
    category_list.set_size_request(140, -1);

    let categories = ["All", "System", "Utilities", "Development", "Graphics", "Internet", "Office"];
    for cat in categories {
        let row = ListBoxRow::new();
        let hbox = GtkBox::new(Orientation::Horizontal, 8);
        hbox.set_margin_start(12);
        hbox.set_margin_end(12);
        hbox.set_margin_top(8);
        hbox.set_margin_bottom(8);
        let label = Label::new(Some(cat));
        label.add_css_class("menu-category-label");
        hbox.append(&label);
        row.set_child(Some(&hbox));
        category_list.append(&row);
    }
    // Select "All" by default
    if let Some(first_row) = category_list.row_at_index(0) {
        category_list.select_row(Some(&first_row));
    }
    content.append(&category_list);

    // App list
    let scroll = ScrolledWindow::new();
    scroll.set_hexpand(true);

    let app_list = ListBox::new();
    app_list.add_css_class("menu-app-list");

    // Function to populate app list
    let populate_apps = {
        let app_list = app_list.clone();
        let apps_data = apps_data.clone();
        let current_category = current_category.clone();
        let current_search = current_search.clone();

        move || {
            // Clear existing rows
            while let Some(child) = app_list.first_child() {
                app_list.remove(&child);
            }

            let cat = current_category.borrow();
            let search_text = current_search.borrow().to_lowercase();

            for (name, desc, app_cat) in apps_data.iter() {
                // Filter by category
                if *cat != "All" && *app_cat != cat.as_str() {
                    continue;
                }
                // Filter by search
                if !search_text.is_empty() {
                    if !name.to_lowercase().contains(&search_text)
                        && !desc.to_lowercase().contains(&search_text)
                    {
                        continue;
                    }
                }

                let row = ListBoxRow::new();
                let hbox = GtkBox::new(Orientation::Horizontal, 12);
                hbox.set_margin_start(12);
                hbox.set_margin_end(12);
                hbox.set_margin_top(8);
                hbox.set_margin_bottom(8);

                let vbox = GtkBox::new(Orientation::Vertical, 2);
                let name_label = Label::new(Some(*name));
                name_label.add_css_class("menu-app-name");
                name_label.set_halign(Align::Start);
                vbox.append(&name_label);

                let desc_label = Label::new(Some(*desc));
                desc_label.add_css_class("menu-app-comment");
                desc_label.set_halign(Align::Start);
                vbox.append(&desc_label);

                hbox.append(&vbox);
                row.set_child(Some(&hbox));
                app_list.append(&row);
            }
        }
    };

    // Initial population
    populate_apps();

    // Category selection handler
    {
        let populate_apps = populate_apps.clone();
        let current_category = current_category.clone();
        let categories = categories.clone();

        category_list.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                let idx = row.index() as usize;
                if idx < categories.len() {
                    *current_category.borrow_mut() = categories[idx].to_string();
                    populate_apps();
                }
            }
        });
    }

    // Search handler
    {
        let populate_apps = populate_apps.clone();
        let current_search = current_search.clone();

        search.connect_search_changed(move |entry| {
            *current_search.borrow_mut() = entry.text().to_string();
            populate_apps();
        });
    }

    // App click handler
    app_list.connect_row_activated(|_, row| {
        if let Some(child) = row.child() {
            if let Some(hbox) = child.first_child() {
                if let Some(vbox) = hbox.first_child() {
                    if let Some(label) = vbox.first_child() {
                        if let Ok(label) = label.downcast::<Label>() {
                            println!("Launching: {}", label.text());
                        }
                    }
                }
            }
        }
    });

    scroll.set_child(Some(&app_list));
    content.append(&scroll);
    main_box.append(&content);

    window.set_child(Some(&main_box));
    window.present();
}

fn create_power_test() {
    println!("Creating Power Menu test window...");
    println!("Press Escape or click outside to close");

    let window = Window::builder()
        .title("Power Menu")
        .default_width(800)
        .default_height(500)
        .build();

    window.connect_close_request(|_| {
        std::process::exit(0);
        #[allow(unreachable_code)]
        glib::Propagation::Proceed
    });

    let main_box = GtkBox::new(Orientation::Vertical, 24);
    main_box.add_css_class("power-overlay");
    main_box.set_valign(Align::Center);
    main_box.set_halign(Align::Center);
    main_box.set_margin_start(48);
    main_box.set_margin_end(48);
    main_box.set_margin_top(48);
    main_box.set_margin_bottom(48);

    let title = Label::new(Some("Power Options"));
    title.add_css_class("power-overlay-title");
    main_box.append(&title);

    let subtitle = Label::new(Some("What would you like to do?"));
    subtitle.add_css_class("power-overlay-subtitle");
    main_box.append(&subtitle);

    let button_box = GtkBox::new(Orientation::Horizontal, 16);
    button_box.set_halign(Align::Center);

    let options = [
        ("Lock", "Lock screen"),
        ("Logout", "End session"),
        ("Suspend", "Sleep mode"),
        ("Hibernate", "Save to disk"),
        ("Reboot", "Restart system"),
        ("Shutdown", "Power off"),
    ];

    for (name, desc) in options {
        let btn_box = GtkBox::new(Orientation::Vertical, 8);
        btn_box.add_css_class("power-overlay-button");
        btn_box.set_size_request(100, 100);
        btn_box.set_valign(Align::Center);
        btn_box.set_halign(Align::Center);

        let label = Label::new(Some(name));
        label.add_css_class("power-overlay-button-name");
        btn_box.append(&label);

        let desc_label = Label::new(Some(desc));
        desc_label.add_css_class("power-overlay-button-desc");
        btn_box.append(&desc_label);

        let btn = Button::new();
        btn.set_child(Some(&btn_box));
        btn.add_css_class("power-overlay-button");
        button_box.append(&btn);
    }

    main_box.append(&button_box);

    let hint = Label::new(Some("Press Escape to cancel"));
    hint.add_css_class("power-overlay-hint");
    main_box.append(&hint);

    window.set_child(Some(&main_box));

    // Escape to close
    let win = window.clone();
    let key_controller = gtk4::EventControllerKey::new();
    key_controller.connect_key_pressed(move |_, key, _, _| {
        if key == gtk4::gdk::Key::Escape {
            win.close();
        }
        glib::Propagation::Proceed
    });
    window.add_controller(key_controller);

    window.present();
}

fn create_settings_test() {
    use gtk4::{Switch, Scale, SpinButton, ColorButton};

    println!("Creating Settings test window...");

    let window = Window::builder()
        .title("Raven Settings")
        .default_width(900)
        .default_height(650)
        .build();

    window.connect_close_request(|_| {
        std::process::exit(0);
        #[allow(unreachable_code)]
        glib::Propagation::Proceed
    });

    let main_box = GtkBox::new(Orientation::Horizontal, 0);
    main_box.add_css_class("settings-window");

    // Sidebar
    let sidebar = GtkBox::new(Orientation::Vertical, 0);
    sidebar.add_css_class("settings-sidebar");
    sidebar.set_size_request(220, -1);

    let categories = [
        ("Appearance", "Theme and colors"),
        ("Desktop", "Wallpaper and icons"),
        ("Panel", "Taskbar settings"),
        ("Windows", "Window behavior"),
        ("Input", "Keyboard and mouse"),
        ("Power", "Power management"),
        ("Sound", "Audio settings"),
        ("About", "System information"),
    ];

    let list = ListBox::new();
    list.add_css_class("settings-category-list");

    for (name, desc) in categories {
        let row = ListBoxRow::new();
        row.add_css_class("settings-category-row");

        let hbox = GtkBox::new(Orientation::Horizontal, 12);
        hbox.set_margin_start(16);
        hbox.set_margin_end(16);
        hbox.set_margin_top(12);
        hbox.set_margin_bottom(12);

        let vbox = GtkBox::new(Orientation::Vertical, 2);
        let title = Label::new(Some(name));
        title.add_css_class("settings-category-name");
        title.set_halign(Align::Start);
        vbox.append(&title);

        let subtitle = Label::new(Some(desc));
        subtitle.add_css_class("settings-category-desc");
        subtitle.set_halign(Align::Start);
        vbox.append(&subtitle);

        hbox.append(&vbox);
        row.set_child(Some(&hbox));
        list.append(&row);
    }

    // Select first row by default
    if let Some(first_row) = list.row_at_index(0) {
        list.select_row(Some(&first_row));
    }

    sidebar.append(&list);
    main_box.append(&sidebar);

    // Content area with stack for pages
    let content_scroll = ScrolledWindow::new();
    content_scroll.set_hexpand(true);
    content_scroll.set_vexpand(true);

    let stack = gtk4::Stack::new();
    stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
    stack.set_transition_duration(150);

    // Helper functions defined inline
    fn settings_row(label_text: &str, widget: &gtk4::Widget) -> GtkBox {
        let row = GtkBox::new(Orientation::Horizontal, 12);
        row.add_css_class("settings-row");
        row.set_margin_start(8);
        row.set_margin_end(8);
        row.set_margin_top(8);
        row.set_margin_bottom(8);

        let label = Label::new(Some(label_text));
        label.set_hexpand(true);
        label.set_halign(Align::Start);
        row.append(&label);
        row.append(widget);
        row
    }

    fn settings_section(title: &str) -> GtkBox {
        let section = GtkBox::new(Orientation::Vertical, 4);
        section.add_css_class("settings-section");
        section.set_margin_top(16);

        let label = Label::new(Some(title));
        label.add_css_class("settings-section-title");
        label.set_halign(Align::Start);
        label.set_margin_bottom(8);
        section.append(&label);
        section
    }

    // ===== Appearance Page =====
    let appearance_page = GtkBox::new(Orientation::Vertical, 8);
    appearance_page.set_margin_start(24);
    appearance_page.set_margin_end(24);
    appearance_page.set_margin_top(24);

    let title = Label::new(Some("Appearance"));
    title.add_css_class("settings-page-title");
    title.set_halign(Align::Start);
    appearance_page.append(&title);

    let subtitle = Label::new(Some("Customize the look and feel of your desktop"));
    subtitle.add_css_class("settings-page-subtitle");
    subtitle.set_halign(Align::Start);
    appearance_page.append(&subtitle);

    let theme_section = settings_section("Theme");
    let theme_dropdown = gtk4::DropDown::from_strings(&["Dark", "Light", "Auto"]);
    theme_section.append(&settings_row("Color scheme", theme_dropdown.upcast_ref()));

    let accent_btn = ColorButton::new();
    accent_btn.set_rgba(&gtk4::gdk::RGBA::new(0.2, 0.6, 1.0, 1.0));
    theme_section.append(&settings_row("Accent color", accent_btn.upcast_ref()));

    let transparency = Switch::new();
    transparency.set_active(true);
    theme_section.append(&settings_row("Enable transparency", transparency.upcast_ref()));

    appearance_page.append(&theme_section);

    let fonts_section = settings_section("Fonts");
    let font_dropdown = gtk4::DropDown::from_strings(&["Inter", "Roboto", "Ubuntu", "System Default"]);
    fonts_section.append(&settings_row("Interface font", font_dropdown.upcast_ref()));

    let font_size = SpinButton::with_range(8.0, 24.0, 1.0);
    font_size.set_value(11.0);
    fonts_section.append(&settings_row("Font size", font_size.upcast_ref()));

    appearance_page.append(&fonts_section);
    stack.add_named(&appearance_page, Some("appearance"));

    // ===== Desktop Page =====
    let desktop_page = GtkBox::new(Orientation::Vertical, 8);
    desktop_page.set_margin_start(24);
    desktop_page.set_margin_end(24);
    desktop_page.set_margin_top(24);

    let title = Label::new(Some("Desktop"));
    title.add_css_class("settings-page-title");
    title.set_halign(Align::Start);
    desktop_page.append(&title);

    let subtitle = Label::new(Some("Wallpaper and desktop icons"));
    subtitle.add_css_class("settings-page-subtitle");
    subtitle.set_halign(Align::Start);
    desktop_page.append(&subtitle);

    let wallpaper_section = settings_section("Wallpaper");
    let wallpaper_btn = Button::with_label("Choose...");
    wallpaper_section.append(&settings_row("Background image", wallpaper_btn.upcast_ref()));

    let fit_dropdown = gtk4::DropDown::from_strings(&["Fill", "Fit", "Stretch", "Center", "Tile"]);
    wallpaper_section.append(&settings_row("Picture fit", fit_dropdown.upcast_ref()));

    desktop_page.append(&wallpaper_section);

    let icons_section = settings_section("Desktop Icons");
    let show_icons = Switch::new();
    show_icons.set_active(true);
    icons_section.append(&settings_row("Show desktop icons", show_icons.upcast_ref()));

    let icon_size = gtk4::DropDown::from_strings(&["Small", "Medium", "Large"]);
    icon_size.set_selected(1);
    icons_section.append(&settings_row("Icon size", icon_size.upcast_ref()));

    let show_home = Switch::new();
    show_home.set_active(true);
    icons_section.append(&settings_row("Show Home folder", show_home.upcast_ref()));

    let show_trash = Switch::new();
    show_trash.set_active(true);
    icons_section.append(&settings_row("Show Trash", show_trash.upcast_ref()));

    desktop_page.append(&icons_section);
    stack.add_named(&desktop_page, Some("desktop"));

    // ===== Panel Page =====
    let panel_page = GtkBox::new(Orientation::Vertical, 8);
    panel_page.set_margin_start(24);
    panel_page.set_margin_end(24);
    panel_page.set_margin_top(24);

    let title = Label::new(Some("Panel"));
    title.add_css_class("settings-page-title");
    title.set_halign(Align::Start);
    panel_page.append(&title);

    let subtitle = Label::new(Some("Configure the taskbar"));
    subtitle.add_css_class("settings-page-subtitle");
    subtitle.set_halign(Align::Start);
    panel_page.append(&subtitle);

    let position_section = settings_section("Position");
    let position_dropdown = gtk4::DropDown::from_strings(&["Bottom", "Top", "Left", "Right"]);
    position_section.append(&settings_row("Panel position", position_dropdown.upcast_ref()));

    let panel_size = SpinButton::with_range(24.0, 64.0, 2.0);
    panel_size.set_value(48.0);
    position_section.append(&settings_row("Panel height", panel_size.upcast_ref()));

    panel_page.append(&position_section);

    let behavior_section = settings_section("Behavior");
    let autohide = Switch::new();
    behavior_section.append(&settings_row("Auto-hide panel", autohide.upcast_ref()));

    let show_apps = Switch::new();
    show_apps.set_active(true);
    behavior_section.append(&settings_row("Show applications button", show_apps.upcast_ref()));

    let show_clock = Switch::new();
    show_clock.set_active(true);
    behavior_section.append(&settings_row("Show clock", show_clock.upcast_ref()));

    let show_systray = Switch::new();
    show_systray.set_active(true);
    behavior_section.append(&settings_row("Show system tray", show_systray.upcast_ref()));

    panel_page.append(&behavior_section);
    stack.add_named(&panel_page, Some("panel"));

    // ===== Windows Page =====
    let windows_page = GtkBox::new(Orientation::Vertical, 8);
    windows_page.set_margin_start(24);
    windows_page.set_margin_end(24);
    windows_page.set_margin_top(24);

    let title = Label::new(Some("Windows"));
    title.add_css_class("settings-page-title");
    title.set_halign(Align::Start);
    windows_page.append(&title);

    let subtitle = Label::new(Some("Window behavior and effects"));
    subtitle.add_css_class("settings-page-subtitle");
    subtitle.set_halign(Align::Start);
    windows_page.append(&subtitle);

    let behavior_section = settings_section("Behavior");
    let focus_dropdown = gtk4::DropDown::from_strings(&["Click to focus", "Focus follows mouse"]);
    behavior_section.append(&settings_row("Focus mode", focus_dropdown.upcast_ref()));

    let raise_on_focus = Switch::new();
    raise_on_focus.set_active(true);
    behavior_section.append(&settings_row("Raise window on focus", raise_on_focus.upcast_ref()));

    windows_page.append(&behavior_section);

    let effects_section = settings_section("Effects");
    let animations = Switch::new();
    animations.set_active(true);
    effects_section.append(&settings_row("Enable animations", animations.upcast_ref()));

    let blur = Switch::new();
    blur.set_active(true);
    effects_section.append(&settings_row("Blur behind windows", blur.upcast_ref()));

    let shadows = Switch::new();
    shadows.set_active(true);
    effects_section.append(&settings_row("Window shadows", shadows.upcast_ref()));

    let corner_radius = SpinButton::with_range(0.0, 20.0, 1.0);
    corner_radius.set_value(12.0);
    effects_section.append(&settings_row("Corner radius", corner_radius.upcast_ref()));

    windows_page.append(&effects_section);
    stack.add_named(&windows_page, Some("windows"));

    // ===== Input Page =====
    let input_page = GtkBox::new(Orientation::Vertical, 8);
    input_page.set_margin_start(24);
    input_page.set_margin_end(24);
    input_page.set_margin_top(24);

    let title = Label::new(Some("Input"));
    title.add_css_class("settings-page-title");
    title.set_halign(Align::Start);
    input_page.append(&title);

    let subtitle = Label::new(Some("Keyboard and mouse settings"));
    subtitle.add_css_class("settings-page-subtitle");
    subtitle.set_halign(Align::Start);
    input_page.append(&subtitle);

    let keyboard_section = settings_section("Keyboard");
    let repeat_delay = Scale::with_range(Orientation::Horizontal, 100.0, 1000.0, 50.0);
    repeat_delay.set_value(400.0);
    repeat_delay.set_size_request(200, -1);
    keyboard_section.append(&settings_row("Repeat delay", repeat_delay.upcast_ref()));

    let repeat_rate = Scale::with_range(Orientation::Horizontal, 10.0, 100.0, 5.0);
    repeat_rate.set_value(30.0);
    repeat_rate.set_size_request(200, -1);
    keyboard_section.append(&settings_row("Repeat rate", repeat_rate.upcast_ref()));

    input_page.append(&keyboard_section);

    let mouse_section = settings_section("Mouse");
    let mouse_speed = Scale::with_range(Orientation::Horizontal, 0.1, 3.0, 0.1);
    mouse_speed.set_value(1.0);
    mouse_speed.set_size_request(200, -1);
    mouse_section.append(&settings_row("Pointer speed", mouse_speed.upcast_ref()));

    let natural_scroll = Switch::new();
    mouse_section.append(&settings_row("Natural scrolling", natural_scroll.upcast_ref()));

    let tap_to_click = Switch::new();
    tap_to_click.set_active(true);
    mouse_section.append(&settings_row("Tap to click", tap_to_click.upcast_ref()));

    input_page.append(&mouse_section);
    stack.add_named(&input_page, Some("input"));

    // ===== Power Page =====
    let power_page = GtkBox::new(Orientation::Vertical, 8);
    power_page.set_margin_start(24);
    power_page.set_margin_end(24);
    power_page.set_margin_top(24);

    let title = Label::new(Some("Power"));
    title.add_css_class("settings-page-title");
    title.set_halign(Align::Start);
    power_page.append(&title);

    let subtitle = Label::new(Some("Power management settings"));
    subtitle.add_css_class("settings-page-subtitle");
    subtitle.set_halign(Align::Start);
    power_page.append(&subtitle);

    let power_section = settings_section("Power Saving");
    let blank_screen = gtk4::DropDown::from_strings(&["1 minute", "2 minutes", "5 minutes", "10 minutes", "Never"]);
    blank_screen.set_selected(2);
    power_section.append(&settings_row("Blank screen after", blank_screen.upcast_ref()));

    let auto_suspend = gtk4::DropDown::from_strings(&["15 minutes", "30 minutes", "1 hour", "Never"]);
    auto_suspend.set_selected(1);
    power_section.append(&settings_row("Automatic suspend", auto_suspend.upcast_ref()));

    power_page.append(&power_section);

    let battery_section = settings_section("Battery");
    let show_percentage = Switch::new();
    show_percentage.set_active(true);
    battery_section.append(&settings_row("Show battery percentage", show_percentage.upcast_ref()));

    let power_saver = Switch::new();
    battery_section.append(&settings_row("Power saver mode", power_saver.upcast_ref()));

    power_page.append(&battery_section);
    stack.add_named(&power_page, Some("power"));

    // ===== Sound Page =====
    let sound_page = GtkBox::new(Orientation::Vertical, 8);
    sound_page.set_margin_start(24);
    sound_page.set_margin_end(24);
    sound_page.set_margin_top(24);

    let title = Label::new(Some("Sound"));
    title.add_css_class("settings-page-title");
    title.set_halign(Align::Start);
    sound_page.append(&title);

    let subtitle = Label::new(Some("Audio settings"));
    subtitle.add_css_class("settings-page-subtitle");
    subtitle.set_halign(Align::Start);
    sound_page.append(&subtitle);

    let output_section = settings_section("Output");
    let volume = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 5.0);
    volume.set_value(75.0);
    volume.set_size_request(200, -1);
    output_section.append(&settings_row("Volume", volume.upcast_ref()));

    let output_device = gtk4::DropDown::from_strings(&["Speakers", "Headphones", "HDMI Audio"]);
    output_section.append(&settings_row("Output device", output_device.upcast_ref()));

    sound_page.append(&output_section);

    let alerts_section = settings_section("Alerts");
    let system_sounds = Switch::new();
    system_sounds.set_active(true);
    alerts_section.append(&settings_row("System sounds", system_sounds.upcast_ref()));

    let alert_volume = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 5.0);
    alert_volume.set_value(50.0);
    alert_volume.set_size_request(200, -1);
    alerts_section.append(&settings_row("Alert volume", alert_volume.upcast_ref()));

    sound_page.append(&alerts_section);
    stack.add_named(&sound_page, Some("sound"));

    // ===== About Page =====
    let about_page = GtkBox::new(Orientation::Vertical, 8);
    about_page.set_margin_start(24);
    about_page.set_margin_end(24);
    about_page.set_margin_top(24);

    let title = Label::new(Some("About"));
    title.add_css_class("settings-page-title");
    title.set_halign(Align::Start);
    about_page.append(&title);

    let subtitle = Label::new(Some("System information"));
    subtitle.add_css_class("settings-page-subtitle");
    subtitle.set_halign(Align::Start);
    about_page.append(&subtitle);

    let info_section = settings_section("System");

    let os_label = Label::new(Some("RavenLinux"));
    os_label.set_halign(Align::End);
    info_section.append(&settings_row("Operating System", os_label.upcast_ref()));

    let version_label = Label::new(Some("1.0.0"));
    version_label.set_halign(Align::End);
    info_section.append(&settings_row("Version", version_label.upcast_ref()));

    let kernel = std::process::Command::new("uname")
        .arg("-r")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());
    let kernel_label = Label::new(Some(&kernel));
    kernel_label.set_halign(Align::End);
    info_section.append(&settings_row("Kernel", kernel_label.upcast_ref()));

    let desktop_label = Label::new(Some("Raven Shell"));
    desktop_label.set_halign(Align::End);
    info_section.append(&settings_row("Desktop", desktop_label.upcast_ref()));

    about_page.append(&info_section);

    let hardware_section = settings_section("Hardware");

    let hostname = std::fs::read_to_string("/etc/hostname")
        .unwrap_or_else(|_| "Unknown".to_string())
        .trim()
        .to_string();
    let hostname_label = Label::new(Some(&hostname));
    hostname_label.set_halign(Align::End);
    hardware_section.append(&settings_row("Device Name", hostname_label.upcast_ref()));

    let mem_info = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let total_mem = mem_info
        .lines()
        .find(|l| l.starts_with("MemTotal"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u64>().ok())
        .map(|kb| format!("{:.1} GB", kb as f64 / 1024.0 / 1024.0))
        .unwrap_or_else(|| "Unknown".to_string());
    let mem_label = Label::new(Some(&total_mem));
    mem_label.set_halign(Align::End);
    hardware_section.append(&settings_row("Memory", mem_label.upcast_ref()));

    about_page.append(&hardware_section);
    stack.add_named(&about_page, Some("about"));

    // Set initial page
    stack.set_visible_child_name("appearance");

    // Connect sidebar selection to stack
    let stack_clone = stack.clone();
    let page_names = ["appearance", "desktop", "panel", "windows", "input", "power", "sound", "about"];
    list.connect_row_selected(move |_, row| {
        if let Some(row) = row {
            let idx = row.index() as usize;
            if idx < page_names.len() {
                stack_clone.set_visible_child_name(page_names[idx]);
            }
        }
    });

    content_scroll.set_child(Some(&stack));
    main_box.append(&content_scroll);

    window.set_child(Some(&main_box));
    window.present();
}

fn create_keybindings_test() {
    println!("Creating Keybindings test window...");
    println!("Press any key or Escape to close");

    let window = Window::builder()
        .title("Keyboard Shortcuts")
        .default_width(900)
        .default_height(600)
        .build();

    window.connect_close_request(|_| {
        std::process::exit(0);
        #[allow(unreachable_code)]
        glib::Propagation::Proceed
    });

    let main_box = GtkBox::new(Orientation::Vertical, 16);
    main_box.add_css_class("keybindings-overlay");
    main_box.set_margin_start(48);
    main_box.set_margin_end(48);
    main_box.set_margin_top(32);
    main_box.set_margin_bottom(32);

    let title = Label::new(Some("Keyboard Shortcuts"));
    title.add_css_class("keybindings-title");
    main_box.append(&title);

    let scroll = ScrolledWindow::new();
    scroll.set_vexpand(true);

    let grid = GtkBox::new(Orientation::Horizontal, 48);
    grid.set_halign(Align::Center);

    // Two columns
    for col in 0..2 {
        let column = GtkBox::new(Orientation::Vertical, 16);

        let categories = if col == 0 {
            vec![
                ("General", vec![
                    ("Super", "Open app menu"),
                    ("Super + Q", "Close window"),
                    ("Super + F", "Toggle fullscreen"),
                    ("Alt + Tab", "Switch windows"),
                ]),
                ("Workspaces", vec![
                    ("Super + 1-9", "Switch workspace"),
                    ("Super + Shift + 1-9", "Move to workspace"),
                ]),
            ]
        } else {
            vec![
                ("Windows", vec![
                    ("Super + Arrow", "Tile window"),
                    ("Super + M", "Maximize"),
                    ("Super + N", "Minimize"),
                ]),
                ("System", vec![
                    ("Super + L", "Lock screen"),
                    ("Super + E", "File manager"),
                    ("Super + T", "Terminal"),
                ]),
            ]
        };

        for (cat_name, bindings) in categories {
            let cat_label = Label::new(Some(cat_name));
            cat_label.add_css_class("keybindings-category-name");
            cat_label.set_halign(Align::Start);
            column.append(&cat_label);

            for (key, desc) in bindings {
                let row = GtkBox::new(Orientation::Horizontal, 16);
                row.add_css_class("keybindings-row");

                let key_label = Label::new(Some(key));
                key_label.add_css_class("keybindings-key");
                row.append(&key_label);

                let desc_label = Label::new(Some(desc));
                desc_label.add_css_class("keybindings-description");
                row.append(&desc_label);

                column.append(&row);
            }
        }

        grid.append(&column);
    }

    scroll.set_child(Some(&grid));
    main_box.append(&scroll);

    window.set_child(Some(&main_box));

    // Any key to close
    let win = window.clone();
    let key_controller = gtk4::EventControllerKey::new();
    key_controller.connect_key_pressed(move |_, _, _, _| {
        win.close();
        glib::Propagation::Proceed
    });
    window.add_controller(key_controller);

    window.present();
}

fn create_filemanager_test() {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::path::PathBuf;
    use gtk4::{GestureClick, DragSource, DropTarget};
    use gtk4::gdk::{ContentProvider, DragAction};
    use gtk4::glib::Value;

    println!("Creating File Manager test window...");

    let window = Window::builder()
        .title("Raven Files")
        .default_width(1000)
        .default_height(700)
        .build();

    window.connect_close_request(|_| {
        std::process::exit(0);
        #[allow(unreachable_code)]
        glib::Propagation::Proceed
    });

    // State
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home".to_string());
    let current_path: Rc<RefCell<PathBuf>> = Rc::new(RefCell::new(PathBuf::from(&home)));
    let history: Rc<RefCell<Vec<PathBuf>>> = Rc::new(RefCell::new(vec![PathBuf::from(&home)]));
    let history_pos: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));

    // Clipboard state for copy/cut
    #[derive(Clone)]
    enum ClipboardOp { Copy, Cut }
    let clipboard_path: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
    let clipboard_op: Rc<RefCell<Option<ClipboardOp>>> = Rc::new(RefCell::new(None));

    // Selected file path for context menu
    let selected_path: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));

    // Pinned directories for sidebar
    let pinned_dirs: Rc<RefCell<Vec<PathBuf>>> = Rc::new(RefCell::new(Vec::new()));

    let main_box = GtkBox::new(Orientation::Vertical, 0);
    main_box.add_css_class("file-manager");

    // Header/toolbar
    let header = GtkBox::new(Orientation::Horizontal, 8);
    header.add_css_class("fm-header");
    header.set_margin_start(8);
    header.set_margin_end(8);
    header.set_margin_top(8);
    header.set_margin_bottom(8);

    let back_btn = Button::from_icon_name("go-previous");
    back_btn.add_css_class("fm-nav-button");
    back_btn.set_sensitive(false);
    header.append(&back_btn);

    let forward_btn = Button::from_icon_name("go-next");
    forward_btn.add_css_class("fm-nav-button");
    forward_btn.set_sensitive(false);
    header.append(&forward_btn);

    let up_btn = Button::from_icon_name("go-up");
    up_btn.add_css_class("fm-nav-button");
    header.append(&up_btn);

    let home_btn = Button::from_icon_name("go-home");
    home_btn.add_css_class("fm-nav-button");
    header.append(&home_btn);

    let location = gtk4::Entry::new();
    location.set_text(&home);
    location.add_css_class("fm-location-bar");
    location.set_hexpand(true);
    header.append(&location);

    let search = SearchEntry::new();
    search.set_placeholder_text(Some("Search"));
    search.add_css_class("fm-search-entry");
    search.set_size_request(200, -1);
    header.append(&search);

    main_box.append(&header);

    // Content
    let content = GtkBox::new(Orientation::Horizontal, 0);
    content.set_vexpand(true);

    // Sidebar
    let sidebar = GtkBox::new(Orientation::Vertical, 0);
    sidebar.add_css_class("fm-sidebar");
    sidebar.set_size_request(180, -1);

    let places = [
        ("user-home", "Home", ""),
        ("user-desktop", "Desktop", "Desktop"),
        ("folder-documents", "Documents", "Documents"),
        ("folder-download", "Downloads", "Downloads"),
        ("folder-pictures", "Pictures", "Pictures"),
        ("folder-videos", "Videos", "Videos"),
        ("folder-music", "Music", "Music"),
        ("user-trash", "Trash", ".local/share/Trash/files"),
    ];

    let section = Label::new(Some("Places"));
    section.add_css_class("fm-sidebar-section");
    section.set_halign(Align::Start);
    section.set_margin_start(12);
    section.set_margin_top(12);
    section.set_margin_bottom(4);
    sidebar.append(&section);

    let sidebar_list = ListBox::new();
    sidebar_list.add_css_class("fm-sidebar-list");

    // Create paths for places - needed for drop targets
    let places_paths: Vec<PathBuf> = places.iter().map(|(_, _, subpath)| {
        if subpath.is_empty() {
            PathBuf::from(&home)
        } else {
            PathBuf::from(&home).join(subpath)
        }
    }).collect();

    // Shared refresh callback for sidebar drop targets (will be set later)
    let sidebar_refresh: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

    for (idx, (icon, name, _subpath)) in places.iter().enumerate() {
        let row = ListBoxRow::new();
        row.add_css_class("fm-sidebar-row");

        let hbox = GtkBox::new(Orientation::Horizontal, 8);
        hbox.set_margin_start(12);
        hbox.set_margin_end(12);
        hbox.set_margin_top(6);
        hbox.set_margin_bottom(6);

        let img = gtk4::Image::from_icon_name(*icon);
        img.add_css_class("fm-sidebar-icon");
        hbox.append(&img);

        let label = Label::new(Some(*name));
        label.add_css_class("fm-sidebar-label");
        hbox.append(&label);

        // Add drop target to sidebar row
        let drop_target = DropTarget::new(glib::Type::STRING, DragAction::MOVE | DragAction::COPY);
        let target_path = places_paths[idx].clone();
        let refresh_ref = sidebar_refresh.clone();

        drop_target.connect_drop(move |_target, value, _x, _y| {
            let src_path_str: Option<String> = value.get::<glib::GString>()
                .map(|s| s.to_string())
                .ok()
                .or_else(|| value.get::<String>().ok());

            if let Some(src_str) = src_path_str {
                let src_path = PathBuf::from(&src_str);
                // Don't allow dropping to self
                if src_path.parent() == Some(target_path.as_path()) {
                    return false;
                }
                if let Some(file_name) = src_path.file_name() {
                    let dest_path = target_path.join(file_name);
                    match std::fs::rename(&src_path, &dest_path) {
                        Ok(_) => {
                            println!("Moved {} to {}", src_path.display(), target_path.display());
                            if let Some(ref refresh) = *refresh_ref.borrow() {
                                refresh();
                            }
                            return true;
                        }
                        Err(e) => println!("Move to sidebar failed: {}", e),
                    }
                }
            }
            false
        });

        row.add_controller(drop_target);
        row.set_child(Some(&hbox));
        sidebar_list.append(&row);
    }

    sidebar.append(&sidebar_list);

    // Pinned section (initially hidden, shown when items are pinned)
    let pinned_section = Label::new(Some("Pinned"));
    pinned_section.add_css_class("fm-sidebar-section");
    pinned_section.set_halign(Align::Start);
    pinned_section.set_margin_start(12);
    pinned_section.set_margin_top(12);
    pinned_section.set_margin_bottom(4);
    pinned_section.set_visible(false);
    sidebar.append(&pinned_section);

    let pinned_list = ListBox::new();
    pinned_list.add_css_class("fm-sidebar-list");
    pinned_list.set_visible(false);
    sidebar.append(&pinned_list);

    // Wrap pinned UI in Rc for sharing
    let pinned_section = Rc::new(pinned_section);
    let pinned_list = Rc::new(pinned_list);

    content.append(&sidebar);

    // File list area
    let file_area = GtkBox::new(Orientation::Vertical, 0);
    file_area.add_css_class("fm-file-area");
    file_area.set_hexpand(true);
    file_area.set_vexpand(true);

    let scroll = ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_hexpand(true);
    // Only vertical scrolling, no horizontal
    scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);

    let file_list = ListBox::new();
    file_list.add_css_class("fm-file-list");
    file_list.set_selection_mode(gtk4::SelectionMode::Single);
    file_list.set_activate_on_single_click(false); // Require double-click

    // Status bar
    let status = GtkBox::new(Orientation::Horizontal, 8);
    status.add_css_class("fm-status-bar");
    status.set_margin_start(12);
    status.set_margin_end(12);
    status.set_margin_top(4);
    status.set_margin_bottom(4);

    let status_text = Label::new(Some("Ready"));
    status_text.add_css_class("fm-status-text");
    status.append(&status_text);

    // Shared state for drag operation
    let drag_source_path: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));

    // Function to populate file list
    let populate_files: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

    {
        let file_list = file_list.clone();
        let current_path = current_path.clone();
        let location = location.clone();
        let status_text = status_text.clone();
        let window = window.clone();
        let drag_source_path = drag_source_path.clone();
        let populate_files_ref = populate_files.clone();

        let populate_fn = Rc::new(move || {
            // Clear existing rows
            while let Some(child) = file_list.first_child() {
                file_list.remove(&child);
            }

            let path = current_path.borrow();
            location.set_text(&path.to_string_lossy());
            window.set_title(Some(&format!("{} - Raven Files", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string_lossy().to_string()))));

            match std::fs::read_dir(path.as_path()) {
                Ok(entries) => {
                    let mut items: Vec<_> = entries.filter_map(|e| e.ok()).collect();
                    // Sort: directories first, then by name
                    items.sort_by(|a, b| {
                        let a_dir = a.path().is_dir();
                        let b_dir = b.path().is_dir();
                        match (a_dir, b_dir) {
                            (true, false) => std::cmp::Ordering::Less,
                            (false, true) => std::cmp::Ordering::Greater,
                            _ => a.file_name().cmp(&b.file_name()),
                        }
                    });

                    let count = items.len();

                    // Add parent directory row if not at root
                    let current_dir = path.clone();
                    if current_dir != PathBuf::from("/") {
                        if let Some(parent) = current_dir.parent() {
                            let row = ListBoxRow::new();
                            row.add_css_class("fm-file-row");

                            let hbox = GtkBox::new(Orientation::Horizontal, 12);
                            hbox.set_margin_start(12);
                            hbox.set_margin_end(12);
                            hbox.set_margin_top(6);
                            hbox.set_margin_bottom(6);

                            let img = gtk4::Image::from_icon_name("go-up");
                            img.add_css_class("fm-file-icon-folder");
                            hbox.append(&img);

                            let label = Label::new(Some(".."));
                            label.set_halign(gtk4::Align::Start);
                            label.set_hexpand(true);
                            label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                            hbox.append(&label);

                            // Store parent path in row for navigation
                            row.set_widget_name(&parent.to_string_lossy());

                            // Add drop target for parent directory
                            let drop_target = DropTarget::new(glib::Type::STRING, DragAction::MOVE | DragAction::COPY);
                            let parent_path = parent.to_path_buf();
                            let populate_ref = populate_files_ref.clone();

                            drop_target.connect_drop(move |_target, value, _x, _y| {
                                let src_path_str: Option<String> = value.get::<glib::GString>()
                                    .map(|s| s.to_string())
                                    .ok()
                                    .or_else(|| value.get::<String>().ok());

                                if let Some(src_str) = src_path_str {
                                    let src_path = PathBuf::from(&src_str);
                                    if let Some(file_name) = src_path.file_name() {
                                        let dest_path = parent_path.join(file_name);
                                        if src_path != dest_path {
                                            match std::fs::rename(&src_path, &dest_path) {
                                                Ok(_) => {
                                                    println!("Moved {} to parent: {}", src_path.display(), dest_path.display());
                                                    if let Some(ref populate) = *populate_ref.borrow() {
                                                        populate();
                                                    }
                                                    return true;
                                                }
                                                Err(e) => println!("Move to parent failed: {}", e),
                                            }
                                        }
                                    }
                                }
                                false
                            });

                            row.add_controller(drop_target);
                            row.set_child(Some(&hbox));
                            file_list.append(&row);
                        }
                    }

                    for entry in items {
                        let row = ListBoxRow::new();
                        row.add_css_class("fm-file-row");

                        let hbox = GtkBox::new(Orientation::Horizontal, 12);
                        hbox.set_margin_start(12);
                        hbox.set_margin_end(12);
                        hbox.set_margin_top(6);
                        hbox.set_margin_bottom(6);

                        let entry_path = entry.path();
                        let is_dir = entry_path.is_dir();
                        let icon = if is_dir { "folder" } else { "text-x-generic" };
                        let img = gtk4::Image::from_icon_name(icon);
                        if is_dir {
                            img.add_css_class("fm-file-icon-folder");
                        }
                        hbox.append(&img);

                        let name = entry.file_name().to_string_lossy().to_string();
                        let label = Label::new(Some(&name));
                        label.add_css_class("fm-file-name");
                        if is_dir {
                            label.add_css_class("fm-file-name-folder");
                        }
                        label.set_hexpand(true);
                        label.set_halign(Align::Start);
                        label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                        hbox.append(&label);

                        if let Ok(meta) = entry.metadata() {
                            let size = if is_dir {
                                "--".to_string()
                            } else {
                                format_size(meta.len())
                            };
                            let size_label = Label::new(Some(&size));
                            size_label.add_css_class("fm-file-size");
                            size_label.set_width_chars(10);
                            size_label.set_xalign(1.0);
                            hbox.append(&size_label);
                        }

                        // Store path in row for navigation
                        row.set_widget_name(&entry_path.to_string_lossy());

                        // Add drag source to row
                        let drag_source = DragSource::new();
                        drag_source.set_actions(DragAction::MOVE | DragAction::COPY);

                        let entry_path_for_drag = entry_path.clone();
                        let drag_source_path_clone = drag_source_path.clone();
                        drag_source.connect_prepare(move |_source, _x, _y| {
                            *drag_source_path_clone.borrow_mut() = Some(entry_path_for_drag.clone());
                            let path_str: glib::GString = entry_path_for_drag.to_string_lossy().into();
                            Some(ContentProvider::for_value(&Value::from(path_str)))
                        });

                        let entry_path_for_icon = entry_path.clone();
                        drag_source.connect_drag_begin(move |source, _drag| {
                            let icon_name = if entry_path_for_icon.is_dir() { "folder" } else { "text-x-generic" };
                            if let Some(display) = gtk4::gdk::Display::default() {
                                let theme = gtk4::IconTheme::for_display(&display);
                                let paintable = theme.lookup_icon(
                                    icon_name,
                                    &[],
                                    32,
                                    1,
                                    gtk4::TextDirection::Ltr,
                                    gtk4::IconLookupFlags::empty(),
                                );
                                source.set_icon(Some(&paintable), 0, 0);
                            }
                        });

                        row.add_controller(drag_source);

                        // Add drop target to folders
                        if is_dir {
                            let drop_target = DropTarget::new(glib::Type::STRING, DragAction::MOVE | DragAction::COPY);
                            let target_path = entry_path.clone();
                            let populate_files_ref = populate_files_ref.clone();

                            drop_target.connect_drop(move |_target, value, _x, _y| {
                                // Try to get the path as GString first
                                let src_path_str: Option<String> = value.get::<glib::GString>()
                                    .map(|s| s.to_string())
                                    .ok()
                                    .or_else(|| value.get::<String>().ok());

                                if let Some(src_str) = src_path_str {
                                    let src_path = PathBuf::from(&src_str);
                                    println!("Drop: {} -> {}", src_path.display(), target_path.display());
                                    if src_path != target_path && src_path.parent() != Some(target_path.as_path()) {
                                        if let Some(file_name) = src_path.file_name() {
                                            let dest_path = target_path.join(file_name);
                                            match std::fs::rename(&src_path, &dest_path) {
                                                Ok(_) => {
                                                    println!("Moved {} to {}", src_path.display(), dest_path.display());
                                                    if let Some(ref populate) = *populate_files_ref.borrow() {
                                                        populate();
                                                    }
                                                    return true;
                                                }
                                                Err(e) => {
                                                    println!("Move failed: {}", e);
                                                }
                                            }
                                        }
                                    }
                                }
                                false
                            });

                            row.add_controller(drop_target);
                        }

                        row.set_child(Some(&hbox));
                        file_list.append(&row);
                    }

                    status_text.set_text(&format!("{} items", count));
                }
                Err(e) => {
                    status_text.set_text(&format!("Error: {}", e));
                }
            }
        });

        *populate_files.borrow_mut() = Some(populate_fn);
    }

    // Helper to call populate_files
    let call_populate = {
        let populate_files = populate_files.clone();
        Rc::new(move || {
            if let Some(ref f) = *populate_files.borrow() {
                f();
            }
        })
    };

    // Add drop target to file list for dropping into current directory
    {
        let current_path = current_path.clone();
        let call_populate = call_populate.clone();

        let drop_target = DropTarget::new(glib::Type::STRING, DragAction::MOVE | DragAction::COPY);
        drop_target.connect_drop(move |_target, value, _x, _y| {
            // Try to get the path as GString first
            let src_path_str: Option<String> = value.get::<glib::GString>()
                .map(|s| s.to_string())
                .ok()
                .or_else(|| value.get::<String>().ok());

            if let Some(src_str) = src_path_str {
                let src_path = PathBuf::from(&src_str);
                let dest_dir = current_path.borrow().clone();

                println!("Drop to current dir: {} -> {}", src_path.display(), dest_dir.display());

                // Don't move to same directory
                if src_path.parent() == Some(dest_dir.as_path()) {
                    println!("Already in same directory, skipping");
                    return false;
                }

                if let Some(file_name) = src_path.file_name() {
                    let dest_path = dest_dir.join(file_name);
                    match std::fs::rename(&src_path, &dest_path) {
                        Ok(_) => {
                            println!("Moved {} to {}", src_path.display(), dest_path.display());
                            call_populate();
                            return true;
                        }
                        Err(e) => {
                            println!("Move failed: {}", e);
                        }
                    }
                }
            }
            false
        });

        file_list.add_controller(drop_target);
    }

    // Initial population
    call_populate();

    // Navigate to path helper
    let navigate_to = {
        let current_path = current_path.clone();
        let history = history.clone();
        let history_pos = history_pos.clone();
        let back_btn = back_btn.clone();
        let forward_btn = forward_btn.clone();
        let call_populate = call_populate.clone();

        Rc::new(move |new_path: PathBuf, add_to_history: bool| {
            if new_path.is_dir() {
                *current_path.borrow_mut() = new_path.clone();

                if add_to_history {
                    let mut hist = history.borrow_mut();
                    let mut pos = history_pos.borrow_mut();
                    // Truncate forward history
                    hist.truncate(*pos + 1);
                    hist.push(new_path);
                    *pos = hist.len() - 1;
                }

                // Update button sensitivity
                let pos = *history_pos.borrow();
                let len = history.borrow().len();
                back_btn.set_sensitive(pos > 0);
                forward_btn.set_sensitive(pos < len - 1);

                call_populate();
            }
        })
    };

    // Double-click to open folder or show Open With dialog for files
    {
        let navigate_to = navigate_to.clone();
        let window = window.clone();
        file_list.connect_row_activated(move |_, row| {
            let path_str = row.widget_name();
            let path = PathBuf::from(path_str.as_str());
            if path.is_dir() {
                navigate_to(path, true);
            } else {
                // Show Open With dialog for files
                show_open_with_dialog(&window, &path);
            }
        });
    }

    // Helper to create context menu window
    fn show_context_menu(
        parent: &Window,
        has_selection: bool,
        has_clipboard: bool,
        selected_path: Rc<RefCell<Option<PathBuf>>>,
        current_path: Rc<RefCell<PathBuf>>,
        clipboard_path: Rc<RefCell<Option<PathBuf>>>,
        clipboard_op: Rc<RefCell<Option<ClipboardOp>>>,
        navigate_to: Rc<dyn Fn(PathBuf, bool)>,
        call_populate: Rc<dyn Fn()>,
        pinned_dirs: Rc<RefCell<Vec<PathBuf>>>,
        rebuild_pinned: Rc<dyn Fn()>,
    ) {
        let menu_window = Window::builder()
            .title("Context Menu")
            .transient_for(parent)
            .modal(true)
            .decorated(false)
            .default_width(180)
            .build();

        let menu_box = GtkBox::new(Orientation::Vertical, 0);
        menu_box.add_css_class("context-menu");

        // Helper to create menu buttons
        let create_btn = |label: &str| -> Button {
            let btn = Button::with_label(label);
            btn.add_css_class("flat");
            btn.set_halign(gtk4::Align::Fill);
            btn
        };

        if has_selection {
            // Open
            let open_btn = create_btn("Open");
            {
                let selected_path = selected_path.clone();
                let navigate_to = navigate_to.clone();
                let menu_window = menu_window.clone();
                open_btn.connect_clicked(move |_| {
                    menu_window.close();
                    if let Some(path) = selected_path.borrow().clone() {
                        if path.is_dir() {
                            navigate_to(path, true);
                        } else {
                            let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
                        }
                    }
                });
            }
            menu_box.append(&open_btn);

            // Open With (only for files)
            let is_file = selected_path.borrow().as_ref().map(|p| p.is_file()).unwrap_or(false);
            if is_file {
                let open_with_btn = create_btn("Open With...");
                {
                    let selected_path = selected_path.clone();
                    let parent = parent.clone();
                    let menu_window = menu_window.clone();
                    open_with_btn.connect_clicked(move |_| {
                        menu_window.close();
                        if let Some(path) = selected_path.borrow().clone() {
                            show_open_with_dialog(&parent, &path);
                        }
                    });
                }
                menu_box.append(&open_with_btn);
            }

            menu_box.append(&gtk4::Separator::new(Orientation::Horizontal));

            // Cut
            let cut_btn = create_btn("Cut");
            {
                let selected_path = selected_path.clone();
                let clipboard_path = clipboard_path.clone();
                let clipboard_op = clipboard_op.clone();
                let menu_window = menu_window.clone();
                cut_btn.connect_clicked(move |_| {
                    menu_window.close();
                    if let Some(path) = selected_path.borrow().clone() {
                        *clipboard_path.borrow_mut() = Some(path.clone());
                        *clipboard_op.borrow_mut() = Some(ClipboardOp::Cut);
                        println!("Cut: {}", path.display());
                    }
                });
            }
            menu_box.append(&cut_btn);

            // Copy
            let copy_btn = create_btn("Copy");
            {
                let selected_path = selected_path.clone();
                let clipboard_path = clipboard_path.clone();
                let clipboard_op = clipboard_op.clone();
                let menu_window = menu_window.clone();
                copy_btn.connect_clicked(move |_| {
                    menu_window.close();
                    if let Some(path) = selected_path.borrow().clone() {
                        *clipboard_path.borrow_mut() = Some(path.clone());
                        *clipboard_op.borrow_mut() = Some(ClipboardOp::Copy);
                        println!("Copy: {}", path.display());
                    }
                });
            }
            menu_box.append(&copy_btn);

            // Paste (if clipboard has content)
            if has_clipboard {
                let paste_btn = create_btn("Paste");
                {
                    let current_path = current_path.clone();
                    let clipboard_path = clipboard_path.clone();
                    let clipboard_op = clipboard_op.clone();
                    let call_populate = call_populate.clone();
                    let menu_window = menu_window.clone();
                    paste_btn.connect_clicked(move |_| {
                        menu_window.close();
                        do_paste(&current_path, &clipboard_path, &clipboard_op, &call_populate);
                    });
                }
                menu_box.append(&paste_btn);
            }

            menu_box.append(&gtk4::Separator::new(Orientation::Horizontal));

            // Rename
            let rename_btn = create_btn("Rename");
            {
                let selected_path = selected_path.clone();
                let call_populate = call_populate.clone();
                let parent = parent.clone();
                let menu_window = menu_window.clone();
                rename_btn.connect_clicked(move |_| {
                    menu_window.close();
                    if let Some(path) = selected_path.borrow().clone() {
                        show_rename_dialog(&parent, &path, &call_populate);
                    }
                });
            }
            menu_box.append(&rename_btn);

            // Move to Trash or Restore (if in trash)
            let in_trash = is_in_trash(&current_path.borrow());
            if in_trash {
                // Show Restore button when in trash
                let restore_btn = create_btn("Restore");
                {
                    let selected_path = selected_path.clone();
                    let call_populate = call_populate.clone();
                    let menu_window = menu_window.clone();
                    restore_btn.connect_clicked(move |_| {
                        menu_window.close();
                        if let Some(path) = selected_path.borrow().clone() {
                            match restore_from_trash(&path) {
                                Ok(restored_to) => {
                                    println!("Restored to: {}", restored_to.display());
                                }
                                Err(e) => {
                                    eprintln!("Failed to restore: {}", e);
                                }
                            }
                            call_populate();
                        }
                    });
                }
                menu_box.append(&restore_btn);
            } else {
                // Show Move to Trash button when not in trash
                let trash_btn = create_btn("Move to Trash");
                {
                    let selected_path = selected_path.clone();
                    let call_populate = call_populate.clone();
                    let menu_window = menu_window.clone();
                    trash_btn.connect_clicked(move |_| {
                        menu_window.close();
                        if let Some(path) = selected_path.borrow().clone() {
                            let _ = std::process::Command::new("gio")
                                .args(["trash", &path.to_string_lossy()])
                                .status();
                            println!("Trashed: {}", path.display());
                            call_populate();
                        }
                    });
                }
                menu_box.append(&trash_btn);
            }

            // Delete
            let delete_btn = create_btn("Delete");
            {
                let selected_path = selected_path.clone();
                let call_populate = call_populate.clone();
                let parent = parent.clone();
                let menu_window = menu_window.clone();
                delete_btn.connect_clicked(move |_| {
                    menu_window.close();
                    if let Some(path) = selected_path.borrow().clone() {
                        show_delete_dialog(&parent, &path, &call_populate);
                    }
                });
            }
            menu_box.append(&delete_btn);

            menu_box.append(&gtk4::Separator::new(Orientation::Horizontal));

            // Pin to Sidebar (only for directories, and if not already pinned)
            let is_dir = selected_path.borrow().as_ref().map(|p| p.is_dir()).unwrap_or(false);
            if is_dir {
                let already_pinned = selected_path.borrow().as_ref()
                    .map(|p| pinned_dirs.borrow().contains(p))
                    .unwrap_or(false);

                if already_pinned {
                    // Show Unpin option
                    let unpin_btn = create_btn("Unpin from Sidebar");
                    {
                        let selected_path = selected_path.clone();
                        let pinned_dirs = pinned_dirs.clone();
                        let rebuild_pinned = rebuild_pinned.clone();
                        let menu_window = menu_window.clone();
                        unpin_btn.connect_clicked(move |_| {
                            menu_window.close();
                            if let Some(path) = selected_path.borrow().clone() {
                                pinned_dirs.borrow_mut().retain(|p| p != &path);
                                rebuild_pinned();
                                println!("Unpinned: {}", path.display());
                            }
                        });
                    }
                    menu_box.append(&unpin_btn);
                } else {
                    // Show Pin option
                    let pin_btn = create_btn("Pin to Sidebar");
                    {
                        let selected_path = selected_path.clone();
                        let pinned_dirs = pinned_dirs.clone();
                        let rebuild_pinned = rebuild_pinned.clone();
                        let menu_window = menu_window.clone();
                        pin_btn.connect_clicked(move |_| {
                            menu_window.close();
                            if let Some(path) = selected_path.borrow().clone() {
                                // Don't add duplicates
                                if !pinned_dirs.borrow().contains(&path) {
                                    pinned_dirs.borrow_mut().push(path.clone());
                                    rebuild_pinned();
                                    println!("Pinned: {}", path.display());
                                }
                            }
                        });
                    }
                    menu_box.append(&pin_btn);
                }

                menu_box.append(&gtk4::Separator::new(Orientation::Horizontal));
            }

            // Properties
            let props_btn = create_btn("Properties");
            {
                let selected_path = selected_path.clone();
                let parent = parent.clone();
                let menu_window = menu_window.clone();
                props_btn.connect_clicked(move |_| {
                    menu_window.close();
                    if let Some(path) = selected_path.borrow().clone() {
                        show_properties_dialog(&parent, &path);
                    }
                });
            }
            menu_box.append(&props_btn);
        } else {
            // Background menu (no selection)
            if has_clipboard {
                let paste_btn = create_btn("Paste");
                {
                    let current_path = current_path.clone();
                    let clipboard_path = clipboard_path.clone();
                    let clipboard_op = clipboard_op.clone();
                    let call_populate = call_populate.clone();
                    let menu_window = menu_window.clone();
                    paste_btn.connect_clicked(move |_| {
                        menu_window.close();
                        do_paste(&current_path, &clipboard_path, &clipboard_op, &call_populate);
                    });
                }
                menu_box.append(&paste_btn);
                menu_box.append(&gtk4::Separator::new(Orientation::Horizontal));
            }

            // New Folder
            let new_folder_btn = create_btn("New Folder");
            {
                let current_path = current_path.clone();
                let call_populate = call_populate.clone();
                let parent = parent.clone();
                let menu_window = menu_window.clone();
                new_folder_btn.connect_clicked(move |_| {
                    menu_window.close();
                    show_new_folder_dialog(&parent, &current_path.borrow(), &call_populate);
                });
            }
            menu_box.append(&new_folder_btn);

            // New File
            let new_file_btn = create_btn("New File");
            {
                let current_path = current_path.clone();
                let call_populate = call_populate.clone();
                let parent = parent.clone();
                let menu_window = menu_window.clone();
                new_file_btn.connect_clicked(move |_| {
                    menu_window.close();
                    show_new_file_dialog(&parent, &current_path.borrow(), &call_populate);
                });
            }
            menu_box.append(&new_file_btn);

            menu_box.append(&gtk4::Separator::new(Orientation::Horizontal));

            // Connect to Location
            let connect_btn = create_btn("Connect to Location...");
            {
                let parent = parent.clone();
                let pinned_dirs = pinned_dirs.clone();
                let rebuild_pinned = rebuild_pinned.clone();
                let navigate_to = navigate_to.clone();
                let menu_window = menu_window.clone();
                connect_btn.connect_clicked(move |_| {
                    menu_window.close();
                    show_mount_location_dialog(
                        &parent,
                        pinned_dirs.clone(),
                        rebuild_pinned.clone(),
                        navigate_to.clone(),
                    );
                });
            }
            menu_box.append(&connect_btn);

            menu_box.append(&gtk4::Separator::new(Orientation::Horizontal));

            // Refresh
            let refresh_btn = create_btn("Refresh");
            {
                let call_populate = call_populate.clone();
                let menu_window = menu_window.clone();
                refresh_btn.connect_clicked(move |_| {
                    menu_window.close();
                    call_populate();
                });
            }
            menu_box.append(&refresh_btn);
        }

        menu_window.set_child(Some(&menu_box));

        // Close on Escape
        let menu_win = menu_window.clone();
        let key_controller = gtk4::EventControllerKey::new();
        key_controller.connect_key_pressed(move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                menu_win.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        menu_window.add_controller(key_controller);

        // Close on focus loss
        let focus_controller = gtk4::EventControllerFocus::new();
        let menu_win = menu_window.clone();
        focus_controller.connect_leave(move |_| {
            let win = menu_win.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(50), move || {
                win.close();
            });
        });
        menu_window.add_controller(focus_controller);

        menu_window.present();
    }

    // Paste helper function
    fn do_paste(
        current_path: &Rc<RefCell<PathBuf>>,
        clipboard_path: &Rc<RefCell<Option<PathBuf>>>,
        clipboard_op: &Rc<RefCell<Option<ClipboardOp>>>,
        call_populate: &Rc<dyn Fn()>,
    ) {
        let src = clipboard_path.borrow().clone();
        let op = clipboard_op.borrow().clone();
        if let (Some(src_path), Some(operation)) = (src, op) {
            let dest_dir = current_path.borrow().clone();
            if let Some(file_name) = src_path.file_name() {
                let dest_path = dest_dir.join(file_name);
                match operation {
                    ClipboardOp::Copy => {
                        if src_path.is_dir() {
                            let _ = std::process::Command::new("cp")
                                .args(["-r", &src_path.to_string_lossy(), &dest_path.to_string_lossy()])
                                .status();
                        } else {
                            let _ = std::fs::copy(&src_path, &dest_path);
                        }
                        println!("Copied to: {}", dest_path.display());
                    }
                    ClipboardOp::Cut => {
                        let _ = std::fs::rename(&src_path, &dest_path);
                        *clipboard_path.borrow_mut() = None;
                        *clipboard_op.borrow_mut() = None;
                        println!("Moved to: {}", dest_path.display());
                    }
                }
                call_populate();
            }
        }
    }

    // Create rebuild_pinned callback
    let rebuild_pinned: Rc<dyn Fn()> = {
        let pinned_dirs = pinned_dirs.clone();
        let pinned_section = pinned_section.clone();
        let pinned_list = pinned_list.clone();
        let sidebar_refresh = sidebar_refresh.clone();

        Rc::new(move || {
            // Clear existing pinned rows
            while let Some(row) = pinned_list.row_at_index(0) {
                pinned_list.remove(&row);
            }

            let dirs = pinned_dirs.borrow();
            let has_pinned = !dirs.is_empty();
            pinned_section.set_visible(has_pinned);
            pinned_list.set_visible(has_pinned);

            for path in dirs.iter() {
                let row = ListBoxRow::new();
                row.add_css_class("fm-sidebar-row");
                row.set_widget_name(&path.to_string_lossy());

                let hbox = GtkBox::new(Orientation::Horizontal, 8);
                hbox.set_margin_start(12);
                hbox.set_margin_end(12);
                hbox.set_margin_top(6);
                hbox.set_margin_bottom(6);

                let img = gtk4::Image::from_icon_name("folder");
                img.add_css_class("fm-sidebar-icon");
                hbox.append(&img);

                let name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Folder");
                let label = Label::new(Some(name));
                label.add_css_class("fm-sidebar-label");
                hbox.append(&label);

                // Add drop target to pinned row
                let drop_target = DropTarget::new(glib::Type::STRING, DragAction::MOVE | DragAction::COPY);
                let target_path = path.clone();
                let refresh_ref = sidebar_refresh.clone();

                drop_target.connect_drop(move |_target, value, _x, _y| {
                    let src_path_str: Option<String> = value.get::<glib::GString>()
                        .map(|s| s.to_string())
                        .ok()
                        .or_else(|| value.get::<String>().ok());

                    if let Some(src_str) = src_path_str {
                        let src_path = PathBuf::from(&src_str);
                        if src_path.parent() == Some(target_path.as_path()) {
                            return false;
                        }
                        if let Some(file_name) = src_path.file_name() {
                            let dest_path = target_path.join(file_name);
                            if std::fs::rename(&src_path, &dest_path).is_ok() {
                                if let Some(ref refresh) = *refresh_ref.borrow() {
                                    refresh();
                                }
                                return true;
                            }
                        }
                    }
                    false
                });

                row.add_controller(drop_target);
                row.set_child(Some(&hbox));
                pinned_list.append(&row);
            }
        })
    };

    // Pinned list click handler
    {
        let navigate_to = navigate_to.clone();
        let pinned_list = pinned_list.clone();
        pinned_list.connect_row_activated(move |_list, row| {
            let path_str = row.widget_name();
            let path = PathBuf::from(path_str.as_str());
            if path.exists() && path.is_dir() {
                navigate_to(path, true);
            }
        });
    }

    // Right-click handler
    {
        let selected_path = selected_path.clone();
        let current_path = current_path.clone();
        let clipboard_path = clipboard_path.clone();
        let clipboard_op = clipboard_op.clone();
        let navigate_to = navigate_to.clone();
        let call_populate = call_populate.clone();
        let file_list_clone = file_list.clone();
        let window = window.clone();
        let pinned_dirs = pinned_dirs.clone();
        let rebuild_pinned = rebuild_pinned.clone();

        let gesture = GestureClick::new();
        gesture.set_button(3); // Right click

        gesture.connect_pressed(move |_gesture, _n, _x, y| {
            // Find row at position
            if let Some(row) = file_list_clone.row_at_y(y as i32) {
                file_list_clone.select_row(Some(&row));
                let path_str = row.widget_name();
                *selected_path.borrow_mut() = Some(PathBuf::from(path_str.as_str()));
            } else {
                file_list_clone.unselect_all();
                *selected_path.borrow_mut() = None;
            }

            let has_selection = selected_path.borrow().is_some();
            let has_clipboard = clipboard_path.borrow().is_some();

            show_context_menu(
                &window,
                has_selection,
                has_clipboard,
                selected_path.clone(),
                current_path.clone(),
                clipboard_path.clone(),
                clipboard_op.clone(),
                navigate_to.clone(),
                call_populate.clone(),
                pinned_dirs.clone(),
                rebuild_pinned.clone(),
            );
        });

        file_list.add_controller(gesture);
    }

    // Set sidebar refresh callback
    {
        let call_populate = call_populate.clone();
        *sidebar_refresh.borrow_mut() = Some(call_populate);
    }

    // Sidebar navigation
    {
        let navigate_to = navigate_to.clone();
        let places_paths = places_paths.clone();

        sidebar_list.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                let idx = row.index() as usize;
                if idx < places_paths.len() {
                    navigate_to(places_paths[idx].clone(), true);
                }
            }
        });
    }

    // Back button
    {
        let history = history.clone();
        let history_pos = history_pos.clone();
        let current_path = current_path.clone();
        let back_btn_ref = back_btn.clone();
        let forward_btn = forward_btn.clone();
        let call_populate = call_populate.clone();

        back_btn.connect_clicked(move |_| {
            let mut pos = history_pos.borrow_mut();
            if *pos > 0 {
                *pos -= 1;
                let hist = history.borrow();
                *current_path.borrow_mut() = hist[*pos].clone();
                drop(pos);
                drop(hist);

                let pos = *history_pos.borrow();
                let len = history.borrow().len();
                back_btn_ref.set_sensitive(pos > 0);
                forward_btn.set_sensitive(pos < len - 1);

                call_populate();
            }
        });
    }

    // Forward button
    {
        let history = history.clone();
        let history_pos = history_pos.clone();
        let current_path = current_path.clone();
        let back_btn = back_btn.clone();
        let forward_btn_ref = forward_btn.clone();
        let call_populate = call_populate.clone();

        forward_btn.connect_clicked(move |_| {
            let mut pos = history_pos.borrow_mut();
            let len = history.borrow().len();
            if *pos < len - 1 {
                *pos += 1;
                let hist = history.borrow();
                *current_path.borrow_mut() = hist[*pos].clone();
                drop(pos);
                drop(hist);

                let pos = *history_pos.borrow();
                let len = history.borrow().len();
                back_btn.set_sensitive(pos > 0);
                forward_btn_ref.set_sensitive(pos < len - 1);

                call_populate();
            }
        });
    }

    // Up button
    {
        let navigate_to = navigate_to.clone();
        let current_path = current_path.clone();

        up_btn.connect_clicked(move |_| {
            let path = current_path.borrow().clone();
            if let Some(parent) = path.parent() {
                navigate_to(parent.to_path_buf(), true);
            }
        });
    }

    // Home button
    {
        let navigate_to = navigate_to.clone();
        let home = home.clone();

        home_btn.connect_clicked(move |_| {
            navigate_to(PathBuf::from(&home), true);
        });
    }

    // Location bar enter
    {
        let navigate_to = navigate_to.clone();

        location.connect_activate(move |entry| {
            let path = PathBuf::from(entry.text().as_str());
            navigate_to(path, true);
        });
    }

    scroll.set_child(Some(&file_list));
    file_area.append(&scroll);
    content.append(&file_area);
    main_box.append(&content);
    main_box.append(&status);

    window.set_child(Some(&main_box));
    window.present();
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn show_rename_dialog(parent: &Window, path: &std::path::Path, refresh: &std::rc::Rc<dyn Fn() + 'static>) {
    use gtk4::prelude::*;

    let dialog = Window::builder()
        .title("Rename")
        .transient_for(parent)
        .modal(true)
        .default_width(400)
        .default_height(120)
        .build();

    let vbox = gtk4::Box::new(Orientation::Vertical, 12);
    vbox.set_margin_start(16);
    vbox.set_margin_end(16);
    vbox.set_margin_top(16);
    vbox.set_margin_bottom(16);

    let label = Label::new(Some("Enter new name:"));
    label.set_halign(Align::Start);
    vbox.append(&label);

    let entry = gtk4::Entry::new();
    entry.set_text(&path.file_name().unwrap_or_default().to_string_lossy());
    entry.select_region(0, -1);
    vbox.append(&entry);

    let btn_box = gtk4::Box::new(Orientation::Horizontal, 8);
    btn_box.set_halign(Align::End);

    let cancel_btn = Button::with_label("Cancel");
    let rename_btn = Button::with_label("Rename");
    rename_btn.add_css_class("suggested-action");

    btn_box.append(&cancel_btn);
    btn_box.append(&rename_btn);
    vbox.append(&btn_box);

    dialog.set_child(Some(&vbox));

    let dialog_clone = dialog.clone();
    cancel_btn.connect_clicked(move |_| {
        dialog_clone.close();
    });

    let path = path.to_path_buf();
    let refresh = refresh.clone();
    let dialog_clone = dialog.clone();
    rename_btn.connect_clicked(move |_| {
        let new_name = entry.text();
        if !new_name.is_empty() {
            if let Some(parent_dir) = path.parent() {
                let new_path = parent_dir.join(new_name.as_str());
                if let Err(e) = std::fs::rename(&path, &new_path) {
                    eprintln!("Rename failed: {}", e);
                } else {
                    refresh();
                }
            }
        }
        dialog_clone.close();
    });

    dialog.present();
}

fn show_delete_dialog(parent: &Window, path: &std::path::Path, refresh: &std::rc::Rc<dyn Fn() + 'static>) {
    use gtk4::prelude::*;

    let dialog = Window::builder()
        .title("Delete")
        .transient_for(parent)
        .modal(true)
        .default_width(400)
        .default_height(150)
        .build();

    let vbox = gtk4::Box::new(Orientation::Vertical, 12);
    vbox.set_margin_start(16);
    vbox.set_margin_end(16);
    vbox.set_margin_top(16);
    vbox.set_margin_bottom(16);

    let icon = gtk4::Image::from_icon_name("dialog-warning");
    icon.set_pixel_size(48);
    vbox.append(&icon);

    let label = Label::new(Some(&format!(
        "Are you sure you want to permanently delete '{}'?\n\nThis action cannot be undone.",
        path.file_name().unwrap_or_default().to_string_lossy()
    )));
    label.set_wrap(true);
    label.set_justify(gtk4::Justification::Center);
    vbox.append(&label);

    let btn_box = gtk4::Box::new(Orientation::Horizontal, 8);
    btn_box.set_halign(Align::Center);

    let cancel_btn = Button::with_label("Cancel");
    let delete_btn = Button::with_label("Delete");
    delete_btn.add_css_class("destructive-action");

    btn_box.append(&cancel_btn);
    btn_box.append(&delete_btn);
    vbox.append(&btn_box);

    dialog.set_child(Some(&vbox));

    let dialog_clone = dialog.clone();
    cancel_btn.connect_clicked(move |_| {
        dialog_clone.close();
    });

    let path = path.to_path_buf();
    let refresh = refresh.clone();
    let dialog_clone = dialog.clone();
    delete_btn.connect_clicked(move |_| {
        let result = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };

        if let Err(e) = result {
            eprintln!("Delete failed: {}", e);
        } else {
            refresh();
        }
        dialog_clone.close();
    });

    dialog.present();
}

fn show_properties_dialog(parent: &Window, path: &std::path::Path) {
    use gtk4::prelude::*;

    let dialog = Window::builder()
        .title("Properties")
        .transient_for(parent)
        .modal(true)
        .default_width(350)
        .default_height(300)
        .build();

    let vbox = gtk4::Box::new(Orientation::Vertical, 8);
    vbox.set_margin_start(16);
    vbox.set_margin_end(16);
    vbox.set_margin_top(16);
    vbox.set_margin_bottom(16);

    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let is_dir = path.is_dir();

    let icon = gtk4::Image::from_icon_name(if is_dir { "folder" } else { "text-x-generic" });
    icon.set_pixel_size(64);
    vbox.append(&icon);

    let name_label = Label::new(Some(&name));
    name_label.add_css_class("title-2");
    vbox.append(&name_label);

    let add_row = |label: &str, value: &str| -> gtk4::Box {
        let row = gtk4::Box::new(Orientation::Horizontal, 8);
        let l = Label::new(Some(label));
        l.set_hexpand(true);
        l.set_halign(Align::Start);
        l.add_css_class("dim-label");
        let v = Label::new(Some(value));
        v.set_halign(Align::End);
        v.set_selectable(true);
        row.append(&l);
        row.append(&v);
        row
    };

    vbox.append(&add_row("Type:", if is_dir { "Folder" } else { "File" }));
    vbox.append(&add_row("Location:", &path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default()));

    if let Ok(meta) = std::fs::metadata(path) {
        if !is_dir {
            vbox.append(&add_row("Size:", &format_size(meta.len())));
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let mode = meta.mode();
            let perms = format!(
                "{}{}{}{}{}{}{}{}{}",
                if mode & 0o400 != 0 { 'r' } else { '-' },
                if mode & 0o200 != 0 { 'w' } else { '-' },
                if mode & 0o100 != 0 { 'x' } else { '-' },
                if mode & 0o040 != 0 { 'r' } else { '-' },
                if mode & 0o020 != 0 { 'w' } else { '-' },
                if mode & 0o010 != 0 { 'x' } else { '-' },
                if mode & 0o004 != 0 { 'r' } else { '-' },
                if mode & 0o002 != 0 { 'w' } else { '-' },
                if mode & 0o001 != 0 { 'x' } else { '-' },
            );
            vbox.append(&add_row("Permissions:", &perms));
        }

        if let Ok(modified) = meta.modified() {
            if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                let datetime = chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0);
                if let Some(dt) = datetime {
                    vbox.append(&add_row("Modified:", &dt.format("%Y-%m-%d %H:%M:%S").to_string()));
                }
            }
        }
    }

    let close_btn = Button::with_label("Close");
    close_btn.set_halign(Align::Center);
    close_btn.set_margin_top(16);

    let dialog_clone = dialog.clone();
    close_btn.connect_clicked(move |_| {
        dialog_clone.close();
    });

    vbox.append(&close_btn);
    dialog.set_child(Some(&vbox));
    dialog.present();
}

fn show_new_folder_dialog(parent: &Window, current_dir: &std::path::Path, refresh: &std::rc::Rc<dyn Fn() + 'static>) {
    use gtk4::prelude::*;

    let dialog = Window::builder()
        .title("New Folder")
        .transient_for(parent)
        .modal(true)
        .default_width(400)
        .default_height(120)
        .build();

    let vbox = gtk4::Box::new(Orientation::Vertical, 12);
    vbox.set_margin_start(16);
    vbox.set_margin_end(16);
    vbox.set_margin_top(16);
    vbox.set_margin_bottom(16);

    let label = Label::new(Some("Folder name:"));
    label.set_halign(Align::Start);
    vbox.append(&label);

    let entry = gtk4::Entry::new();
    entry.set_text("New Folder");
    entry.select_region(0, -1);
    vbox.append(&entry);

    let btn_box = gtk4::Box::new(Orientation::Horizontal, 8);
    btn_box.set_halign(Align::End);

    let cancel_btn = Button::with_label("Cancel");
    let create_btn = Button::with_label("Create");
    create_btn.add_css_class("suggested-action");

    btn_box.append(&cancel_btn);
    btn_box.append(&create_btn);
    vbox.append(&btn_box);

    dialog.set_child(Some(&vbox));

    let dialog_clone = dialog.clone();
    cancel_btn.connect_clicked(move |_| {
        dialog_clone.close();
    });

    let current_dir = current_dir.to_path_buf();
    let refresh = refresh.clone();
    let dialog_clone = dialog.clone();
    create_btn.connect_clicked(move |_| {
        let name = entry.text();
        if !name.is_empty() {
            let new_path = current_dir.join(name.as_str());
            if let Err(e) = std::fs::create_dir(&new_path) {
                eprintln!("Failed to create folder: {}", e);
            } else {
                refresh();
            }
        }
        dialog_clone.close();
    });

    dialog.present();
}

fn show_new_file_dialog(parent: &Window, current_dir: &std::path::Path, refresh: &std::rc::Rc<dyn Fn() + 'static>) {
    use gtk4::prelude::*;

    let dialog = Window::builder()
        .title("New File")
        .transient_for(parent)
        .modal(true)
        .default_width(400)
        .default_height(120)
        .build();

    let vbox = gtk4::Box::new(Orientation::Vertical, 12);
    vbox.set_margin_start(16);
    vbox.set_margin_end(16);
    vbox.set_margin_top(16);
    vbox.set_margin_bottom(16);

    let label = Label::new(Some("File name:"));
    label.set_halign(Align::Start);
    vbox.append(&label);

    let entry = gtk4::Entry::new();
    entry.set_text("new_file.txt");
    entry.select_region(0, -1);
    vbox.append(&entry);

    let btn_box = gtk4::Box::new(Orientation::Horizontal, 8);
    btn_box.set_halign(Align::End);

    let cancel_btn = Button::with_label("Cancel");
    let create_btn = Button::with_label("Create");
    create_btn.add_css_class("suggested-action");

    btn_box.append(&cancel_btn);
    btn_box.append(&create_btn);
    vbox.append(&btn_box);

    dialog.set_child(Some(&vbox));

    let dialog_clone = dialog.clone();
    cancel_btn.connect_clicked(move |_| {
        dialog_clone.close();
    });

    let current_dir = current_dir.to_path_buf();
    let refresh = refresh.clone();
    let dialog_clone = dialog.clone();
    create_btn.connect_clicked(move |_| {
        let name = entry.text();
        if !name.is_empty() {
            let new_path = current_dir.join(name.as_str());
            if let Err(e) = std::fs::File::create(&new_path) {
                eprintln!("Failed to create file: {}", e);
            } else {
                refresh();
            }
        }
        dialog_clone.close();
    });

    dialog.present();
}

// Get MIME type of a file using the file command
fn get_mime_type(path: &std::path::Path) -> String {
    use std::process::Command;

    let output = Command::new("file")
        .arg("--mime-type")
        .arg("-b")
        .arg(path)
        .output();

    match output {
        Ok(out) => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Err(_) => {
            // Fallback to extension-based detection
            match path.extension().and_then(|e: &std::ffi::OsStr| e.to_str()) {
                Some("txt") | Some("md") | Some("rs") | Some("toml") => "text/plain".to_string(),
                Some("png") => "image/png".to_string(),
                Some("jpg") | Some("jpeg") => "image/jpeg".to_string(),
                Some("gif") => "image/gif".to_string(),
                Some("pdf") => "application/pdf".to_string(),
                Some("html") | Some("htm") => "text/html".to_string(),
                Some("mp3") => "audio/mpeg".to_string(),
                Some("mp4") => "video/mp4".to_string(),
                _ => "application/octet-stream".to_string(),
            }
        }
    }
}

// Get applications that can handle a MIME type by parsing .desktop files
fn get_apps_for_mime(mime: &str) -> Vec<(String, String, String)> {
    use std::path::PathBuf;
    let mut apps = Vec::new();
    let app_dirs: [PathBuf; 3] = [
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        dirs::data_dir().map(|d| d.join("applications")).unwrap_or_default(),
    ];

    for dir in &app_dirs {
        if !dir.exists() {
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e: &std::ffi::OsStr| e.to_str()) == Some("desktop") {
                    if let Some(app) = parse_desktop_file(&path, mime) {
                        // Avoid duplicates
                        if !apps.iter().any(|(n, _, _)| n == &app.0) {
                            apps.push(app);
                        }
                    }
                }
            }
        }
    }

    // Sort by name
    apps.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    apps
}

// Parse a .desktop file and check if it supports the target MIME type
fn parse_desktop_file(path: &std::path::Path, target_mime: &str) -> Option<(String, String, String)> {
    let content = std::fs::read_to_string(path).ok()?;

    let mut name = None;
    let mut icon = None;
    let mut exec = None;
    let mut mime_types = String::new();
    let mut no_display = false;
    let mut in_desktop_entry = false;

    for line in content.lines() {
        let line = line.trim();

        if line == "[Desktop Entry]" {
            in_desktop_entry = true;
            continue;
        }

        if line.starts_with('[') && line != "[Desktop Entry]" {
            in_desktop_entry = false;
            continue;
        }

        if !in_desktop_entry {
            continue;
        }

        if let Some(value) = line.strip_prefix("Name=") {
            if name.is_none() {
                name = Some(value.to_string());
            }
        } else if let Some(value) = line.strip_prefix("Icon=") {
            icon = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("Exec=") {
            // Remove field codes like %f, %F, %u, %U
            let exec_cleaned = value
                .replace("%f", "")
                .replace("%F", "")
                .replace("%u", "")
                .replace("%U", "")
                .replace("%i", "")
                .replace("%c", "")
                .replace("%k", "")
                .trim()
                .to_string();
            exec = Some(exec_cleaned);
        } else if let Some(value) = line.strip_prefix("MimeType=") {
            mime_types = value.to_string();
        } else if line == "NoDisplay=true" {
            no_display = true;
        }
    }

    // Skip apps marked as NoDisplay
    if no_display {
        return None;
    }

    // Check if this app supports our MIME type
    let mime_base = target_mime.split('/').next().unwrap_or("");
    let supports_mime = mime_types.split(';').any(|m| {
        let m = m.trim();
        m == target_mime || m == format!("{}/*", mime_base) || m == "*/*"
    });

    // Also include apps that don't specify MimeType (they might work)
    if !supports_mime && !mime_types.is_empty() {
        return None;
    }

    let name = name?;
    let exec = exec?;
    let icon = icon.unwrap_or_else(|| "application-x-executable".to_string());

    Some((name, icon, exec))
}

// Show the "Open With" dialog
fn show_open_with_dialog(parent: &Window, file_path: &std::path::Path) {
    let dialog = Window::builder()
        .title("Open With")
        .transient_for(parent)
        .modal(true)
        .default_width(400)
        .default_height(500)
        .build();

    let vbox = GtkBox::new(Orientation::Vertical, 12);
    vbox.set_margin_top(16);
    vbox.set_margin_bottom(16);
    vbox.set_margin_start(16);
    vbox.set_margin_end(16);

    // File info header
    let file_name = file_path.file_name()
        .and_then(|n: &std::ffi::OsStr| n.to_str())
        .unwrap_or("file");
    let mime_type = get_mime_type(file_path);

    let header_label = Label::new(Some(&format!("Choose application to open:\n{}", file_name)));
    header_label.set_halign(Align::Start);
    header_label.set_margin_bottom(8);
    vbox.append(&header_label);

    let mime_label = Label::new(Some(&format!("Type: {}", mime_type)));
    mime_label.set_halign(Align::Start);
    mime_label.add_css_class("dim-label");
    mime_label.set_margin_bottom(12);
    vbox.append(&mime_label);

    // Scrolled window for app list
    let scrolled = gtk4::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_min_content_height(300);

    let app_list = ListBox::new();
    app_list.set_selection_mode(gtk4::SelectionMode::Single);
    app_list.add_css_class("boxed-list");

    // Get applications for this MIME type
    let apps = get_apps_for_mime(&mime_type);

    if apps.is_empty() {
        let empty_label = Label::new(Some("No applications found for this file type"));
        empty_label.set_margin_top(24);
        empty_label.set_margin_bottom(24);
        app_list.append(&empty_label);
    } else {
        for (app_name, app_icon, app_exec) in &apps {
            let row = ListBoxRow::new();
            row.set_widget_name(app_exec);

            let hbox = GtkBox::new(Orientation::Horizontal, 12);
            hbox.set_margin_top(8);
            hbox.set_margin_bottom(8);
            hbox.set_margin_start(8);
            hbox.set_margin_end(8);

            let icon = gtk4::Image::from_icon_name(app_icon);
            icon.set_pixel_size(32);
            hbox.append(&icon);

            let name_label = Label::new(Some(app_name));
            name_label.set_halign(Align::Start);
            hbox.append(&name_label);

            row.set_child(Some(&hbox));
            app_list.append(&row);
        }
    }

    scrolled.set_child(Some(&app_list));
    vbox.append(&scrolled);

    // Button box
    let btn_box = GtkBox::new(Orientation::Horizontal, 8);
    btn_box.set_halign(Align::End);
    btn_box.set_margin_top(12);

    let custom_btn = Button::with_label("Custom Command...");
    let cancel_btn = Button::with_label("Cancel");
    let open_btn = Button::with_label("Open");
    open_btn.add_css_class("suggested-action");

    btn_box.append(&custom_btn);
    btn_box.append(&cancel_btn);
    btn_box.append(&open_btn);
    vbox.append(&btn_box);

    dialog.set_child(Some(&vbox));

    // Cancel button
    let dialog_clone = dialog.clone();
    cancel_btn.connect_clicked(move |_| {
        dialog_clone.close();
    });

    // Custom command button
    let dialog_clone = dialog.clone();
    let file_path_clone = file_path.to_path_buf();
    custom_btn.connect_clicked(move |_| {
        dialog_clone.close();
        show_custom_open_dialog(&dialog_clone, &file_path_clone);
    });

    // Open button - launch selected app
    let dialog_clone = dialog.clone();
    let file_path_clone = file_path.to_path_buf();
    let app_list_clone = app_list.clone();
    open_btn.connect_clicked(move |_| {
        if let Some(row) = app_list_clone.selected_row() {
            let exec = row.widget_name().to_string();
            if !exec.is_empty() {
                // Split exec into command and args
                let parts: Vec<&str> = exec.split_whitespace().collect();
                if let Some(cmd) = parts.first() {
                    let mut command = std::process::Command::new(cmd);
                    for arg in parts.iter().skip(1) {
                        command.arg(arg);
                    }
                    command.arg(&file_path_clone);

                    if let Err(e) = command.spawn() {
                        eprintln!("Failed to launch application: {}", e);
                    }
                }
            }
        }
        dialog_clone.close();
    });

    // Double-click to open
    let dialog_clone = dialog.clone();
    let file_path_clone = file_path.to_path_buf();
    app_list.connect_row_activated(move |_list, row| {
        let exec = row.widget_name().to_string();
        if !exec.is_empty() {
            let parts: Vec<&str> = exec.split_whitespace().collect();
            if let Some(cmd) = parts.first() {
                let mut command = std::process::Command::new(cmd);
                for arg in parts.iter().skip(1) {
                    command.arg(arg);
                }
                command.arg(&file_path_clone);

                if let Err(e) = command.spawn() {
                    eprintln!("Failed to launch application: {}", e);
                }
            }
        }
        dialog_clone.close();
    });

    dialog.present();
}

// Show custom command dialog
fn show_custom_open_dialog(parent: &Window, file_path: &std::path::Path) {
    let dialog = Window::builder()
        .title("Open With Custom Command")
        .transient_for(parent)
        .modal(true)
        .default_width(400)
        .default_height(150)
        .build();

    let vbox = GtkBox::new(Orientation::Vertical, 12);
    vbox.set_margin_top(16);
    vbox.set_margin_bottom(16);
    vbox.set_margin_start(16);
    vbox.set_margin_end(16);

    let label = Label::new(Some("Enter command to open file:"));
    label.set_halign(Align::Start);
    vbox.append(&label);

    let entry = gtk4::Entry::new();
    entry.set_placeholder_text(Some("e.g., vim, code, gimp"));
    vbox.append(&entry);

    let btn_box = GtkBox::new(Orientation::Horizontal, 8);
    btn_box.set_halign(Align::End);
    btn_box.set_margin_top(8);

    let cancel_btn = Button::with_label("Cancel");
    let open_btn = Button::with_label("Open");
    open_btn.add_css_class("suggested-action");

    btn_box.append(&cancel_btn);
    btn_box.append(&open_btn);
    vbox.append(&btn_box);

    dialog.set_child(Some(&vbox));

    let dialog_clone = dialog.clone();
    cancel_btn.connect_clicked(move |_| {
        dialog_clone.close();
    });

    let dialog_clone = dialog.clone();
    let file_path_clone = file_path.to_path_buf();
    let entry_clone = entry.clone();
    open_btn.connect_clicked(move |_| {
        let cmd = entry_clone.text().to_string();
        if !cmd.is_empty() {
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            if let Some(program) = parts.first() {
                let mut command = std::process::Command::new(program);
                for arg in parts.iter().skip(1) {
                    command.arg(arg);
                }
                command.arg(&file_path_clone);

                if let Err(e) = command.spawn() {
                    eprintln!("Failed to launch command: {}", e);
                }
            }
        }
        dialog_clone.close();
    });

    // Enter key to activate
    let open_btn_clone = open_btn.clone();
    entry.connect_activate(move |_| {
        open_btn_clone.emit_clicked();
    });

    dialog.present();
}

// Get the XDG Trash directory path
fn get_trash_dir() -> std::path::PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"));
    home.join(".local/share/Trash")
}

// Check if a path is in the Trash/files directory
fn is_in_trash(path: &std::path::Path) -> bool {
    let trash_files = get_trash_dir().join("files");
    path.starts_with(&trash_files)
}

// Read the trashinfo file to get the original path
fn read_trash_info(trash_file: &std::path::Path) -> Option<std::path::PathBuf> {
    let file_name = trash_file.file_name()?;
    let trash_info_dir = get_trash_dir().join("info");
    let info_file = trash_info_dir.join(format!("{}.trashinfo", file_name.to_string_lossy()));

    let content = std::fs::read_to_string(&info_file).ok()?;

    for line in content.lines() {
        if let Some(path_str) = line.strip_prefix("Path=") {
            // URL decode the path (handles %20 for spaces, etc.)
            let decoded = url_decode(path_str);
            return Some(std::path::PathBuf::from(decoded));
        }
    }

    None
}

// Simple URL decoder for trash paths
fn url_decode(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2 {
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                    continue;
                }
            }
            result.push('%');
            result.push_str(&hex);
        } else {
            result.push(c);
        }
    }

    result
}

// Restore a file from trash to its original location
fn restore_from_trash(trash_file: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let original_path = read_trash_info(trash_file)
        .ok_or_else(|| "Could not read trashinfo file".to_string())?;

    // Create parent directories if they don't exist
    if let Some(parent) = original_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }
    }

    // Handle name conflicts
    let mut dest_path = original_path.clone();
    let mut counter = 1;
    while dest_path.exists() {
        let stem = original_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let ext = original_path.extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e))
            .unwrap_or_default();
        let parent = original_path.parent().unwrap_or_else(|| std::path::Path::new("/"));
        dest_path = parent.join(format!("{} ({}){}", stem, counter, ext));
        counter += 1;
    }

    // Move the file back
    std::fs::rename(trash_file, &dest_path)
        .map_err(|e| format!("Failed to restore file: {}", e))?;

    // Remove the .trashinfo file
    let file_name = trash_file.file_name().unwrap();
    let info_file = get_trash_dir()
        .join("info")
        .join(format!("{}.trashinfo", file_name.to_string_lossy()));
    let _ = std::fs::remove_file(info_file);

    Ok(dest_path)
}

// Detect mounted removable/external drives
fn detect_removable_drives() -> Vec<(String, std::path::PathBuf)> {
    let mut drives = Vec::new();

    // Try to parse /proc/mounts for mounted filesystems
    if let Ok(content) = std::fs::read_to_string("/proc/mounts") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let _device = parts[0];
                let mount_point = parts[1];

                // Skip system mounts and virtual filesystems
                if mount_point.starts_with("/media/")
                    || mount_point.starts_with("/mnt/")
                    || mount_point.starts_with("/run/media/")
                {
                    // Get a friendly name from the mount point
                    let name = std::path::Path::new(mount_point)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Removable")
                        .to_string();

                    // URL decode the mount path (handles spaces as \040)
                    let decoded_path = mount_point.replace("\\040", " ");

                    drives.push((name, std::path::PathBuf::from(decoded_path)));
                }
            }
        }
    }

    drives
}

// Show mount location dialog
fn show_mount_location_dialog(
    parent: &Window,
    pinned_dirs: std::rc::Rc<std::cell::RefCell<Vec<std::path::PathBuf>>>,
    rebuild_pinned: std::rc::Rc<dyn Fn()>,
    navigate_to: std::rc::Rc<dyn Fn(std::path::PathBuf, bool)>,
) {
    let dialog = Window::builder()
        .title("Connect to Location")
        .transient_for(parent)
        .modal(true)
        .default_width(450)
        .default_height(350)
        .build();

    let vbox = GtkBox::new(Orientation::Vertical, 12);
    vbox.set_margin_top(16);
    vbox.set_margin_bottom(16);
    vbox.set_margin_start(16);
    vbox.set_margin_end(16);

    // Notebook for tabs
    let notebook = gtk4::Notebook::new();

    // Tab 1: Local Directory
    let local_box = GtkBox::new(Orientation::Vertical, 12);
    local_box.set_margin_top(16);
    local_box.set_margin_start(16);
    local_box.set_margin_end(16);

    let local_label = Label::new(Some("Enter a path to add to sidebar:"));
    local_label.set_halign(Align::Start);
    local_box.append(&local_label);

    let local_entry = gtk4::Entry::new();
    local_entry.set_placeholder_text(Some("/path/to/directory"));
    local_box.append(&local_entry);

    let local_browse_btn = Button::with_label("Browse...");
    local_box.append(&local_browse_btn);

    notebook.append_page(&local_box, Some(&Label::new(Some("Local"))));

    // Tab 2: Network (SMB/NFS)
    let network_box = GtkBox::new(Orientation::Vertical, 8);
    network_box.set_margin_top(16);
    network_box.set_margin_start(16);
    network_box.set_margin_end(16);

    let protocol_box = GtkBox::new(Orientation::Horizontal, 8);
    let protocol_label = Label::new(Some("Protocol:"));
    protocol_box.append(&protocol_label);

    let protocol_combo = gtk4::DropDown::from_strings(&["smb://", "nfs://", "sftp://"]);
    protocol_combo.set_hexpand(true);
    protocol_box.append(&protocol_combo);
    network_box.append(&protocol_box);

    let server_box = GtkBox::new(Orientation::Horizontal, 8);
    let server_label = Label::new(Some("Server:"));
    server_label.set_width_chars(10);
    server_box.append(&server_label);
    let server_entry = gtk4::Entry::new();
    server_entry.set_placeholder_text(Some("server.local"));
    server_entry.set_hexpand(true);
    server_box.append(&server_entry);
    network_box.append(&server_box);

    let share_box = GtkBox::new(Orientation::Horizontal, 8);
    let share_label = Label::new(Some("Share:"));
    share_label.set_width_chars(10);
    share_box.append(&share_label);
    let share_entry = gtk4::Entry::new();
    share_entry.set_placeholder_text(Some("share_name"));
    share_entry.set_hexpand(true);
    share_box.append(&share_entry);
    network_box.append(&share_box);

    let user_box = GtkBox::new(Orientation::Horizontal, 8);
    let user_label = Label::new(Some("Username:"));
    user_label.set_width_chars(10);
    user_box.append(&user_label);
    let user_entry = gtk4::Entry::new();
    user_entry.set_placeholder_text(Some("(optional)"));
    user_entry.set_hexpand(true);
    user_box.append(&user_entry);
    network_box.append(&user_box);

    notebook.append_page(&network_box, Some(&Label::new(Some("Network"))));

    // Tab 3: Devices
    let devices_box = GtkBox::new(Orientation::Vertical, 8);
    devices_box.set_margin_top(16);
    devices_box.set_margin_start(16);
    devices_box.set_margin_end(16);

    let devices_label = Label::new(Some("Detected removable devices:"));
    devices_label.set_halign(Align::Start);
    devices_box.append(&devices_label);

    let devices_list = ListBox::new();
    devices_list.add_css_class("boxed-list");

    let drives = detect_removable_drives();
    if drives.is_empty() {
        let empty_label = Label::new(Some("No removable devices detected"));
        empty_label.set_margin_top(12);
        empty_label.set_margin_bottom(12);
        devices_list.append(&empty_label);
    } else {
        for (name, path) in &drives {
            let row = ListBoxRow::new();
            row.set_widget_name(&path.to_string_lossy());

            let hbox = GtkBox::new(Orientation::Horizontal, 12);
            hbox.set_margin_top(8);
            hbox.set_margin_bottom(8);
            hbox.set_margin_start(8);
            hbox.set_margin_end(8);

            let icon = gtk4::Image::from_icon_name("drive-removable-media");
            icon.set_pixel_size(24);
            hbox.append(&icon);

            let name_vbox = GtkBox::new(Orientation::Vertical, 2);
            let name_label = Label::new(Some(name));
            name_label.set_halign(Align::Start);
            name_vbox.append(&name_label);

            let path_label = Label::new(Some(&path.to_string_lossy()));
            path_label.set_halign(Align::Start);
            path_label.add_css_class("dim-label");
            name_vbox.append(&path_label);

            hbox.append(&name_vbox);
            row.set_child(Some(&hbox));
            devices_list.append(&row);
        }
    }

    let scrolled = gtk4::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_min_content_height(150);
    scrolled.set_child(Some(&devices_list));
    devices_box.append(&scrolled);

    notebook.append_page(&devices_box, Some(&Label::new(Some("Devices"))));

    vbox.append(&notebook);

    // Button box
    let btn_box = GtkBox::new(Orientation::Horizontal, 8);
    btn_box.set_halign(Align::End);
    btn_box.set_margin_top(12);

    let cancel_btn = Button::with_label("Cancel");
    let connect_btn = Button::with_label("Connect");
    connect_btn.add_css_class("suggested-action");

    btn_box.append(&cancel_btn);
    btn_box.append(&connect_btn);
    vbox.append(&btn_box);

    dialog.set_child(Some(&vbox));

    // Cancel button
    let dialog_clone = dialog.clone();
    cancel_btn.connect_clicked(move |_| {
        dialog_clone.close();
    });

    // Browse button for local directory (placeholder - users type path directly)
    local_browse_btn.connect_clicked(move |_| {
        // A full file chooser would need async handling
        // For now, users type the path directly in the entry field
    });

    // Connect button
    let dialog_clone = dialog.clone();
    let local_entry_clone = local_entry.clone();
    let server_entry_clone = server_entry.clone();
    let share_entry_clone = share_entry.clone();
    let protocol_combo_clone = protocol_combo.clone();
    let notebook_clone = notebook.clone();
    let devices_list_clone = devices_list.clone();
    let pinned_dirs_clone = pinned_dirs.clone();
    let rebuild_pinned_clone = rebuild_pinned.clone();
    let navigate_to_clone = navigate_to.clone();

    connect_btn.connect_clicked(move |_| {
        let current_tab = notebook_clone.current_page();

        match current_tab {
            Some(0) => {
                // Local directory
                let path_str = local_entry_clone.text().to_string();
                if !path_str.is_empty() {
                    let path = std::path::PathBuf::from(&path_str);
                    if path.exists() && path.is_dir() {
                        // Add to pinned dirs
                        if !pinned_dirs_clone.borrow().contains(&path) {
                            pinned_dirs_clone.borrow_mut().push(path.clone());
                            rebuild_pinned_clone();
                            navigate_to_clone(path, true);
                        }
                        dialog_clone.close();
                    } else {
                        eprintln!("Directory does not exist: {}", path_str);
                    }
                }
            }
            Some(1) => {
                // Network share
                let protocols = ["smb://", "nfs://", "sftp://"];
                let protocol_idx = protocol_combo_clone.selected() as usize;
                let protocol = protocols.get(protocol_idx).unwrap_or(&"smb://");
                let server = server_entry_clone.text().to_string();
                let share = share_entry_clone.text().to_string();

                if !server.is_empty() && !share.is_empty() {
                    let uri = format!("{}{}/{}", protocol, server, share);
                    // Try to open with gio
                    match std::process::Command::new("gio")
                        .args(["mount", &uri])
                        .status()
                    {
                        Ok(status) if status.success() => {
                            println!("Mounted: {}", uri);
                            dialog_clone.close();
                        }
                        _ => {
                            eprintln!("Failed to mount: {}", uri);
                        }
                    }
                }
            }
            Some(2) => {
                // Device selected
                if let Some(row) = devices_list_clone.selected_row() {
                    let path_str = row.widget_name().to_string();
                    let path = std::path::PathBuf::from(&path_str);
                    if path.exists() {
                        navigate_to_clone(path, true);
                        dialog_clone.close();
                    }
                }
            }
            _ => {}
        }
    });

    // Double-click on device to navigate
    let dialog_clone = dialog.clone();
    let navigate_to_clone = navigate_to.clone();
    devices_list.connect_row_activated(move |_list, row| {
        let path_str = row.widget_name().to_string();
        let path = std::path::PathBuf::from(&path_str);
        if path.exists() {
            navigate_to_clone(path, true);
            dialog_clone.close();
        }
    });

    dialog.present();
}
