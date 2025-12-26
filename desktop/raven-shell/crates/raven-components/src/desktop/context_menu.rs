use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Label, Orientation, Separator, Window};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use tokio::sync::mpsc;

use raven_core::{ComponentId, ShellCommand};

/// Desktop right-click context menu
pub struct DesktopContextMenu {
    window: Window,
}

impl DesktopContextMenu {
    pub fn new(command_tx: mpsc::Sender<ShellCommand>, x: f64, y: f64) -> Self {
        let window = Window::new();
        window.set_title(Some("Desktop Menu"));
        window.set_decorated(false);
        window.set_default_size(200, -1);

        // Initialize layer shell
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::OnDemand);

        // Position at cursor
        window.set_anchor(Edge::Top, true);
        window.set_anchor(Edge::Left, true);
        window.set_margin(Edge::Top, y as i32);
        window.set_margin(Edge::Left, x as i32);

        // Build menu content
        let menu_box = GtkBox::new(Orientation::Vertical, 0);
        menu_box.add_css_class("desktop-context-menu");
        menu_box.set_margin_top(8);
        menu_box.set_margin_bottom(8);
        menu_box.set_margin_start(8);
        menu_box.set_margin_end(8);

        // Quick Actions Section
        let quick_label = Label::new(Some("Quick Actions"));
        quick_label.add_css_class("settings-section-label");
        quick_label.set_halign(gtk4::Align::Start);
        menu_box.append(&quick_label);

        // Open Terminal
        let terminal_btn = Button::with_label("Open Terminal");
        let tx = command_tx.clone();
        let win = window.clone();
        terminal_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::LaunchApp("raven-terminal".into()));
            win.close();
        });
        menu_box.append(&terminal_btn);

        // Open File Manager
        let files_btn = Button::with_label("Open File Manager");
        let tx = command_tx.clone();
        let win = window.clone();
        files_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::LaunchApp(
                "raven-files || nautilus || thunar || pcmanfm".into(),
            ));
            win.close();
        });
        menu_box.append(&files_btn);

        // Open App Menu
        let menu_btn = Button::with_label("Applications");
        let tx = command_tx.clone();
        let win = window.clone();
        menu_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::ToggleComponent(ComponentId::Menu));
            win.close();
        });
        menu_box.append(&menu_btn);

        // Separator
        let sep1 = Separator::new(Orientation::Horizontal);
        sep1.add_css_class("settings-menu-separator");
        menu_box.append(&sep1);

        // Configuration Section
        let config_label = Label::new(Some("Configuration"));
        config_label.add_css_class("settings-section-label");
        config_label.set_halign(gtk4::Align::Start);
        menu_box.append(&config_label);

        // Change Wallpaper
        let wallpaper_btn = Button::with_label("Change Wallpaper");
        let tx = command_tx.clone();
        let win = window.clone();
        wallpaper_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::LaunchApp(
                "waypaper || nitrogen || gnome-control-center background".into(),
            ));
            win.close();
        });
        menu_box.append(&wallpaper_btn);

        // Settings
        let settings_btn = Button::with_label("Desktop Settings");
        let tx = command_tx.clone();
        let win = window.clone();
        settings_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::ShowComponent(ComponentId::Settings));
            win.close();
        });
        menu_box.append(&settings_btn);

        // Separator
        let sep2 = Separator::new(Orientation::Horizontal);
        sep2.add_css_class("settings-menu-separator");
        menu_box.append(&sep2);

        // System Section
        let sys_label = Label::new(Some("System"));
        sys_label.add_css_class("settings-section-label");
        sys_label.set_halign(gtk4::Align::Start);
        menu_box.append(&sys_label);

        // Refresh Desktop
        let refresh_btn = Button::with_label("Refresh Desktop");
        let tx = command_tx.clone();
        let win = window.clone();
        refresh_btn.connect_clicked(move |_| {
            // Send a reload config command to refresh icons
            let _ = tx.blocking_send(ShellCommand::ReloadConfig);
            win.close();
        });
        menu_box.append(&refresh_btn);

        window.set_child(Some(&menu_box));

        // Close on Escape
        let key_controller = gtk4::EventControllerKey::new();
        let win = window.clone();
        key_controller.connect_key_pressed(move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                win.close();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        window.add_controller(key_controller);

        // Close on focus lost
        let focus_controller = gtk4::EventControllerFocus::new();
        let win = window.clone();
        focus_controller.connect_leave(move |_| {
            let win = win.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(100), move || {
                win.close();
            });
        });
        window.add_controller(focus_controller);

        Self { window }
    }

    pub fn present(&self) {
        self.window.present();
    }

    pub fn close(&self) {
        self.window.close();
    }

    pub fn window(&self) -> &Window {
        &self.window
    }
}
