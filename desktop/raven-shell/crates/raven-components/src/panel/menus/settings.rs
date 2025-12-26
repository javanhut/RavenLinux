use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Label, Orientation, Separator, Window};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use tokio::sync::mpsc;

use raven_core::{Orientation as PanelOrientation, ShellCommand};

const PANEL_SIZE: i32 = 38;

/// Settings menu popup
pub struct SettingsMenu {
    window: Window,
}

impl SettingsMenu {
    pub fn new(
        command_tx: mpsc::Sender<ShellCommand>,
        current_orientation: PanelOrientation,
    ) -> Self {
        let window = Window::new();
        window.set_title(Some("Settings"));
        window.set_decorated(false);
        window.set_default_size(220, -1);

        // Initialize layer shell
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::OnDemand);

        // Position based on panel orientation
        Self::set_position(&window, current_orientation);

        // Build menu content
        let menu_box = GtkBox::new(Orientation::Vertical, 0);
        menu_box.add_css_class("settings-menu");
        menu_box.set_margin_top(8);
        menu_box.set_margin_bottom(8);
        menu_box.set_margin_start(8);
        menu_box.set_margin_end(8);

        // Quick Settings Section
        let quick_label = Label::new(Some("Quick Settings"));
        quick_label.add_css_class("settings-section-label");
        quick_label.set_halign(gtk4::Align::Start);
        menu_box.append(&quick_label);

        // WiFi
        let wifi_btn = Button::with_label("WiFi");
        wifi_btn.add_css_class("quick-toggle");
        let tx = command_tx.clone();
        let win = window.clone();
        wifi_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::LaunchApp("raven-wifi".into()));
            win.close();
        });
        menu_box.append(&wifi_btn);

        // Bluetooth
        let bt_btn = Button::with_label("Bluetooth");
        bt_btn.add_css_class("quick-toggle");
        let tx = command_tx.clone();
        let win = window.clone();
        bt_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::LaunchApp(
                "blueman-manager || blueberry || gnome-bluetooth-panel".into(),
            ));
            win.close();
        });
        menu_box.append(&bt_btn);

        // Sound
        let sound_btn = Button::with_label("Sound");
        sound_btn.add_css_class("quick-toggle");
        let tx = command_tx.clone();
        let win = window.clone();
        sound_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::LaunchApp(
                "pavucontrol || gnome-control-center sound || pwvucontrol".into(),
            ));
            win.close();
        });
        menu_box.append(&sound_btn);

        // Separator
        let sep1 = Separator::new(Orientation::Horizontal);
        sep1.add_css_class("settings-menu-separator");
        menu_box.append(&sep1);

        // System Section
        let sys_label = Label::new(Some("System"));
        sys_label.add_css_class("settings-section-label");
        sys_label.set_halign(gtk4::Align::Start);
        menu_box.append(&sys_label);

        // Display
        let display_btn = Button::with_label("Display");
        let tx = command_tx.clone();
        let win = window.clone();
        display_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::LaunchApp(
                "wdisplays || nwg-displays || gnome-control-center display".into(),
            ));
            win.close();
        });
        menu_box.append(&display_btn);

        // Network
        let network_btn = Button::with_label("Network");
        let tx = command_tx.clone();
        let win = window.clone();
        network_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::LaunchApp(
                "nm-connection-editor || gnome-control-center network || raven-wifi".into(),
            ));
            win.close();
        });
        menu_box.append(&network_btn);

        // Power & Battery
        let power_btn = Button::with_label("Power & Battery");
        let tx = command_tx.clone();
        let win = window.clone();
        power_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::LaunchApp(
                "gnome-control-center power || xfce4-power-manager-settings".into(),
            ));
            win.close();
        });
        menu_box.append(&power_btn);

        // Keyboard
        let keyboard_btn = Button::with_label("Keyboard");
        let tx = command_tx.clone();
        let win = window.clone();
        keyboard_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::LaunchApp(
                "gnome-control-center keyboard || fcitx5-configtool".into(),
            ));
            win.close();
        });
        menu_box.append(&keyboard_btn);

        // Separator
        let sep2 = Separator::new(Orientation::Horizontal);
        sep2.add_css_class("settings-menu-separator");
        menu_box.append(&sep2);

        // Appearance Section
        let appear_label = Label::new(Some("Appearance"));
        appear_label.add_css_class("settings-section-label");
        appear_label.set_halign(gtk4::Align::Start);
        menu_box.append(&appear_label);

        // Theme
        let theme_btn = Button::with_label("Theme & Appearance");
        let tx = command_tx.clone();
        let win = window.clone();
        theme_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::LaunchApp(
                "nwg-look || lxappearance || gnome-control-center appearance".into(),
            ));
            win.close();
        });
        menu_box.append(&theme_btn);

        // Wallpaper
        let wallpaper_btn = Button::with_label("Wallpaper");
        let tx = command_tx.clone();
        let win = window.clone();
        wallpaper_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::LaunchApp(
                "waypaper || nitrogen || gnome-control-center background".into(),
            ));
            win.close();
        });
        menu_box.append(&wallpaper_btn);

        // Separator
        let sep3 = Separator::new(Orientation::Horizontal);
        sep3.add_css_class("settings-menu-separator");
        menu_box.append(&sep3);

        // Panel Position Section
        let panel_label = Label::new(Some("Panel Position"));
        panel_label.add_css_class("settings-section-label");
        panel_label.set_halign(gtk4::Align::Start);
        menu_box.append(&panel_label);

        // Position buttons
        let position_box = GtkBox::new(Orientation::Vertical, 4);

        let top_bottom_box = GtkBox::new(Orientation::Horizontal, 4);
        top_bottom_box.set_homogeneous(true);

        let top_btn = Button::with_label("Top");
        if current_orientation == PanelOrientation::Top {
            top_btn.add_css_class("quick-toggle-active");
        }
        let tx = command_tx.clone();
        let win = window.clone();
        top_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::SetPanelPosition(PanelOrientation::Top));
            win.close();
        });
        top_bottom_box.append(&top_btn);

        let bottom_btn = Button::with_label("Bottom");
        if current_orientation == PanelOrientation::Bottom {
            bottom_btn.add_css_class("quick-toggle-active");
        }
        let tx = command_tx.clone();
        let win = window.clone();
        bottom_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::SetPanelPosition(PanelOrientation::Bottom));
            win.close();
        });
        top_bottom_box.append(&bottom_btn);

        position_box.append(&top_bottom_box);

        let left_right_box = GtkBox::new(Orientation::Horizontal, 4);
        left_right_box.set_homogeneous(true);

        let left_btn = Button::with_label("Left");
        if current_orientation == PanelOrientation::Left {
            left_btn.add_css_class("quick-toggle-active");
        }
        let tx = command_tx.clone();
        let win = window.clone();
        left_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::SetPanelPosition(PanelOrientation::Left));
            win.close();
        });
        left_right_box.append(&left_btn);

        let right_btn = Button::with_label("Right");
        if current_orientation == PanelOrientation::Right {
            right_btn.add_css_class("quick-toggle-active");
        }
        let tx = command_tx.clone();
        let win = window.clone();
        right_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::SetPanelPosition(PanelOrientation::Right));
            win.close();
        });
        left_right_box.append(&right_btn);

        position_box.append(&left_right_box);
        menu_box.append(&position_box);

        // Separator
        let sep4 = Separator::new(Orientation::Horizontal);
        sep4.add_css_class("settings-menu-separator");
        menu_box.append(&sep4);

        // All Settings
        let all_btn = Button::with_label("All Settings...");
        let tx = command_tx.clone();
        let win = window.clone();
        all_btn.connect_clicked(move |_| {
            let _ = tx.blocking_send(ShellCommand::LaunchApp(
                "raven-settings || gnome-control-center || systemsettings5 || xfce4-settings-manager"
                    .into(),
            ));
            win.close();
        });
        menu_box.append(&all_btn);

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
                window.set_margin(Edge::Right, 100);
            }
            PanelOrientation::Bottom => {
                window.set_anchor(Edge::Bottom, true);
                window.set_anchor(Edge::Right, true);
                window.set_margin(Edge::Bottom, PANEL_SIZE + 4);
                window.set_margin(Edge::Right, 100);
            }
            PanelOrientation::Left => {
                window.set_anchor(Edge::Left, true);
                window.set_anchor(Edge::Top, true);
                window.set_margin(Edge::Left, PANEL_SIZE + 4);
                window.set_margin(Edge::Top, 100);
            }
            PanelOrientation::Right => {
                window.set_anchor(Edge::Right, true);
                window.set_anchor(Edge::Top, true);
                window.set_margin(Edge::Right, PANEL_SIZE + 4);
                window.set_margin(Edge::Top, 100);
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
