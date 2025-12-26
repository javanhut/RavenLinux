use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Orientation, Window};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use tokio::sync::mpsc;

use raven_core::{Orientation as PanelOrientation, ShellCommand};

const PANEL_SIZE: i32 = 38;

/// Power menu popup
pub struct PowerMenu {
    window: Window,
}

impl PowerMenu {
    pub fn new(
        command_tx: mpsc::Sender<ShellCommand>,
        orientation: PanelOrientation,
    ) -> Self {
        let window = Window::new();
        window.set_title(Some("Power"));
        window.set_decorated(false);
        window.set_default_size(160, -1);

        // Initialize layer shell
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::OnDemand);

        // Position based on panel orientation
        Self::set_position(&window, orientation);

        // Build menu content
        let menu_box = GtkBox::new(Orientation::Vertical, 4);
        menu_box.add_css_class("settings-menu");
        menu_box.set_margin_top(8);
        menu_box.set_margin_bottom(8);
        menu_box.set_margin_start(8);
        menu_box.set_margin_end(8);

        // Logout button
        let logout_btn = Button::with_label("Logout");
        let tx = command_tx.clone();
        let win = window.clone();
        logout_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::Logout);
            win.close();
        });
        menu_box.append(&logout_btn);

        // Lock screen button
        let lock_btn = Button::with_label("Lock Screen");
        let tx = command_tx.clone();
        let win = window.clone();
        lock_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::Lock);
            win.close();
        });
        menu_box.append(&lock_btn);

        // Suspend button
        let suspend_btn = Button::with_label("Suspend");
        let tx = command_tx.clone();
        let win = window.clone();
        suspend_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::Suspend);
            win.close();
        });
        menu_box.append(&suspend_btn);

        // Reboot button
        let reboot_btn = Button::with_label("Reboot");
        let tx = command_tx.clone();
        let win = window.clone();
        reboot_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::Reboot);
            win.close();
        });
        menu_box.append(&reboot_btn);

        // Shutdown button
        let shutdown_btn = Button::with_label("Shutdown");
        shutdown_btn.add_css_class("context-menu-close");
        let tx = command_tx.clone();
        let win = window.clone();
        shutdown_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::Shutdown);
            win.close();
        });
        menu_box.append(&shutdown_btn);

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

    fn set_position(window: &Window, orientation: PanelOrientation) {
        // Reset all anchors
        window.set_anchor(Edge::Top, false);
        window.set_anchor(Edge::Bottom, false);
        window.set_anchor(Edge::Left, false);
        window.set_anchor(Edge::Right, false);

        match orientation {
            PanelOrientation::Top => {
                window.set_anchor(Edge::Top, true);
                window.set_anchor(Edge::Right, true);
                window.set_margin(Edge::Top, PANEL_SIZE + 4);
                window.set_margin(Edge::Right, 10);
            }
            PanelOrientation::Bottom => {
                window.set_anchor(Edge::Bottom, true);
                window.set_anchor(Edge::Right, true);
                window.set_margin(Edge::Bottom, PANEL_SIZE + 4);
                window.set_margin(Edge::Right, 10);
            }
            PanelOrientation::Left => {
                window.set_anchor(Edge::Left, true);
                window.set_anchor(Edge::Bottom, true);
                window.set_margin(Edge::Left, PANEL_SIZE + 4);
                window.set_margin(Edge::Bottom, 10);
            }
            PanelOrientation::Right => {
                window.set_anchor(Edge::Right, true);
                window.set_anchor(Edge::Bottom, true);
                window.set_margin(Edge::Right, PANEL_SIZE + 4);
                window.set_margin(Edge::Bottom, 10);
            }
        }
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
