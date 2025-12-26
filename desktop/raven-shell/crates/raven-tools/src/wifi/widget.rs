// WiFi Manager widget - GTK4 UI

use gtk4::prelude::*;
use gtk4::{
    self, Align, Box as GtkBox, Button, Entry, Label, ListBox, ListBoxRow,
    Orientation, ScrolledWindow, Window, ResponseType,
    MessageDialog, MessageType, ButtonsType,
};
use std::cell::RefCell;
use std::rc::Rc;

use super::backend::{Network, WiFiManager, Backend};

/// WiFi Manager window
pub struct WiFiWidget {
    window: Window,
    manager: Rc<RefCell<WiFiManager>>,
    network_list: ListBox,
    status_label: Label,
    scan_button: Button,
}

impl WiFiWidget {
    pub fn new() -> Self {
        let manager = Rc::new(RefCell::new(WiFiManager::new()));

        // Main window
        let window = Window::builder()
            .title("WiFi Manager")
            .default_width(400)
            .default_height(500)
            .resizable(true)
            .build();

        // Main container
        let main_box = GtkBox::new(Orientation::Vertical, 0);
        main_box.add_css_class("wifi-manager");

        // Header bar
        let header = Self::create_header(&manager);
        main_box.append(&header);

        // Status section
        let status_label = Label::new(Some("Not connected"));
        status_label.add_css_class("wifi-status");
        status_label.set_halign(Align::Start);
        status_label.set_margin_start(16);
        status_label.set_margin_end(16);
        status_label.set_margin_top(8);
        status_label.set_margin_bottom(8);
        main_box.append(&status_label);

        // Network list
        let scrolled = ScrolledWindow::builder()
            .vexpand(true)
            .hexpand(true)
            .build();

        let network_list = ListBox::new();
        network_list.add_css_class("wifi-network-list");
        network_list.set_selection_mode(gtk4::SelectionMode::None);
        scrolled.set_child(Some(&network_list));
        main_box.append(&scrolled);

        // Bottom bar with buttons
        let button_box = GtkBox::new(Orientation::Horizontal, 8);
        button_box.set_margin_start(16);
        button_box.set_margin_end(16);
        button_box.set_margin_top(8);
        button_box.set_margin_bottom(16);
        button_box.set_halign(Align::End);

        let saved_button = Button::with_label("Saved Networks");
        saved_button.add_css_class("wifi-button");
        button_box.append(&saved_button);

        let scan_button = Button::with_label("Scan");
        scan_button.add_css_class("wifi-button");
        scan_button.add_css_class("suggested-action");
        button_box.append(&scan_button);

        main_box.append(&button_box);

        window.set_child(Some(&main_box));

        let widget = Self {
            window,
            manager,
            network_list,
            status_label,
            scan_button,
        };

        widget.setup_signals(saved_button);
        widget.update_status();
        widget.scan_networks();

        widget
    }

    fn create_header(manager: &Rc<RefCell<WiFiManager>>) -> GtkBox {
        let header = GtkBox::new(Orientation::Horizontal, 0);
        header.add_css_class("wifi-header");
        header.set_margin_start(16);
        header.set_margin_end(16);
        header.set_margin_top(16);
        header.set_margin_bottom(8);

        let title = Label::new(Some("WiFi Networks"));
        title.add_css_class("wifi-title");
        title.set_halign(Align::Start);
        title.set_hexpand(true);
        header.append(&title);

        // Backend indicator
        let backend_text = match manager.borrow().backend() {
            Backend::Iwd => "IWD",
            Backend::WpaSupplicant => "wpa_supplicant",
            Backend::None => "iw (limited)",
        };
        let backend_label = Label::new(Some(backend_text));
        backend_label.add_css_class("wifi-backend");
        header.append(&backend_label);

        header
    }

    fn setup_signals(&self, saved_button: Button) {
        // Scan button
        let manager = self.manager.clone();
        let network_list = self.network_list.clone();
        let status_label = self.status_label.clone();

        self.scan_button.connect_clicked(move |btn| {
            btn.set_sensitive(false);
            btn.set_label("Scanning...");

            let networks = manager.borrow().scan();
            Self::populate_network_list(&network_list, &networks, &manager, &status_label);

            btn.set_label("Scan");
            btn.set_sensitive(true);
        });

        // Saved networks button
        let manager = self.manager.clone();
        let window = self.window.clone();
        saved_button.connect_clicked(move |_| {
            Self::show_saved_networks_dialog(&window, &manager);
        });

        // Network row activation
        let manager = self.manager.clone();
        let status_label = self.status_label.clone();
        let window = self.window.clone();
        let network_list = self.network_list.clone();

        self.network_list.connect_row_activated(move |_, row| {
            if let Some(ssid) = row.widget_name().as_str().strip_prefix("network-") {
                Self::handle_network_click(
                    &window,
                    &manager,
                    ssid,
                    &status_label,
                    &network_list,
                );
            }
        });
    }

    fn populate_network_list(
        list: &ListBox,
        networks: &[Network],
        manager: &Rc<RefCell<WiFiManager>>,
        status_label: &Label,
    ) {
        // Clear existing rows
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }

        for network in networks {
            let row = Self::create_network_row(network, manager);
            list.append(&row);
        }

        // Update status
        let (ssid, ip) = manager.borrow().get_status();
        if let Some(ssid) = ssid {
            let ip_str = ip.unwrap_or_else(|| "No IP".to_string());
            status_label.set_text(&format!("Connected: {} ({})", ssid, ip_str));
        } else {
            status_label.set_text("Not connected");
        }
    }

    fn create_network_row(network: &Network, manager: &Rc<RefCell<WiFiManager>>) -> ListBoxRow {
        let row = ListBoxRow::new();
        row.set_widget_name(&format!("network-{}", network.ssid));
        row.add_css_class("wifi-network-row");

        if network.connected {
            row.add_css_class("connected");
        }

        let hbox = GtkBox::new(Orientation::Horizontal, 12);
        hbox.set_margin_start(16);
        hbox.set_margin_end(16);
        hbox.set_margin_top(12);
        hbox.set_margin_bottom(12);

        // Signal icon
        let signal_icon = Self::get_signal_icon(network.signal);
        let signal_label = Label::new(Some(signal_icon));
        signal_label.add_css_class("wifi-signal");
        hbox.append(&signal_label);

        // Network info
        let info_box = GtkBox::new(Orientation::Vertical, 2);
        info_box.set_hexpand(true);

        let name_label = Label::new(Some(&network.ssid));
        name_label.add_css_class("wifi-network-name");
        name_label.set_halign(Align::Start);
        info_box.append(&name_label);

        let mut details = vec![network.security.clone()];
        if network.connected {
            details.push("Connected".to_string());
        }
        if manager.borrow().is_known_network(&network.ssid) {
            details.push("Saved".to_string());
        }

        let detail_label = Label::new(Some(&details.join(" - ")));
        detail_label.add_css_class("wifi-network-detail");
        detail_label.set_halign(Align::Start);
        info_box.append(&detail_label);

        hbox.append(&info_box);

        // Signal strength percentage
        let strength_label = Label::new(Some(&format!("{}%", network.signal)));
        strength_label.add_css_class("wifi-signal-percent");
        hbox.append(&strength_label);

        row.set_child(Some(&hbox));
        row
    }

    fn get_signal_icon(signal: i32) -> &'static str {
        if signal >= 75 {
            "||||"
        } else if signal >= 50 {
            "|||"
        } else if signal >= 25 {
            "||"
        } else {
            "|"
        }
    }

    fn handle_network_click(
        window: &Window,
        manager: &Rc<RefCell<WiFiManager>>,
        ssid: &str,
        status_label: &Label,
        network_list: &ListBox,
    ) {
        let mgr = manager.borrow();
        let (current_ssid, _) = mgr.get_status();

        // If connected to this network, disconnect
        if current_ssid.as_ref().map(|s| s.as_str()) == Some(ssid) {
            drop(mgr);
            if let Err(e) = manager.borrow().disconnect() {
                Self::show_error(window, &format!("Disconnect failed: {}", e));
            } else {
                status_label.set_text("Disconnected");
                // Refresh list
                let networks = manager.borrow().scan();
                Self::populate_network_list(network_list, &networks, manager, status_label);
            }
            return;
        }

        // Check if it's a known network
        let is_known = mgr.is_known_network(ssid);
        drop(mgr);

        if is_known {
            // Connect without password
            match manager.borrow().connect(ssid, None) {
                Ok(()) => {
                    let networks = manager.borrow().scan();
                    Self::populate_network_list(network_list, &networks, manager, status_label);
                }
                Err(e) => {
                    Self::show_error(window, &format!("Connection failed: {}", e));
                }
            }
        } else {
            // Show password dialog
            Self::show_password_dialog(window, manager, ssid, status_label, network_list);
        }
    }

    fn show_password_dialog(
        window: &Window,
        manager: &Rc<RefCell<WiFiManager>>,
        ssid: &str,
        status_label: &Label,
        network_list: &ListBox,
    ) {
        let dialog = gtk4::Dialog::builder()
            .title("Enter Password")
            .transient_for(window)
            .modal(true)
            .build();

        dialog.add_button("Cancel", ResponseType::Cancel);
        dialog.add_button("Connect", ResponseType::Ok);

        let content = dialog.content_area();
        content.set_margin_start(20);
        content.set_margin_end(20);
        content.set_margin_top(20);
        content.set_margin_bottom(20);
        content.set_spacing(12);

        let label = Label::new(Some(&format!("Enter password for \"{}\":", ssid)));
        content.append(&label);

        let entry = Entry::new();
        entry.set_visibility(false);
        entry.set_placeholder_text(Some("Password"));
        content.append(&entry);

        let show_pass = gtk4::CheckButton::with_label("Show password");
        let entry_clone = entry.clone();
        show_pass.connect_toggled(move |btn| {
            entry_clone.set_visibility(btn.is_active());
        });
        content.append(&show_pass);

        let manager = manager.clone();
        let ssid = ssid.to_string();
        let status_label = status_label.clone();
        let network_list = network_list.clone();

        dialog.connect_response(move |dlg, response| {
            if response == ResponseType::Ok {
                let password = entry.text().to_string();
                if !password.is_empty() {
                    match manager.borrow().connect(&ssid, Some(&password)) {
                        Ok(()) => {
                            let networks = manager.borrow().scan();
                            Self::populate_network_list(&network_list, &networks, &manager, &status_label);
                        }
                        Err(e) => {
                            status_label.set_text(&format!("Failed: {}", e));
                        }
                    }
                }
            }
            dlg.close();
        });

        dialog.present();
    }

    fn show_saved_networks_dialog(window: &Window, manager: &Rc<RefCell<WiFiManager>>) {
        let dialog = gtk4::Dialog::builder()
            .title("Saved Networks")
            .transient_for(window)
            .modal(true)
            .default_width(300)
            .default_height(400)
            .build();

        dialog.add_button("Close", ResponseType::Close);

        let content = dialog.content_area();
        content.set_margin_start(16);
        content.set_margin_end(16);
        content.set_margin_top(16);
        content.set_margin_bottom(16);

        let scrolled = ScrolledWindow::builder()
            .vexpand(true)
            .hexpand(true)
            .build();

        let list = ListBox::new();
        list.add_css_class("wifi-saved-list");

        let saved = manager.borrow().get_saved_networks();
        for ssid in saved {
            let row = ListBoxRow::new();
            let hbox = GtkBox::new(Orientation::Horizontal, 8);
            hbox.set_margin_start(8);
            hbox.set_margin_end(8);
            hbox.set_margin_top(8);
            hbox.set_margin_bottom(8);

            let label = Label::new(Some(&ssid));
            label.set_hexpand(true);
            label.set_halign(Align::Start);
            hbox.append(&label);

            let forget_btn = Button::with_label("Forget");
            forget_btn.add_css_class("destructive-action");

            let manager_clone = manager.clone();
            let ssid_clone = ssid.clone();
            let row_clone = row.clone();
            forget_btn.connect_clicked(move |_| {
                if manager_clone.borrow().forget_network(&ssid_clone).is_ok() {
                    if let Some(parent) = row_clone.parent() {
                        if let Some(list) = parent.downcast_ref::<ListBox>() {
                            list.remove(&row_clone);
                        }
                    }
                }
            });
            hbox.append(&forget_btn);

            row.set_child(Some(&hbox));
            list.append(&row);
        }

        if list.first_child().is_none() {
            let empty_label = Label::new(Some("No saved networks"));
            empty_label.add_css_class("dim-label");
            content.append(&empty_label);
        } else {
            scrolled.set_child(Some(&list));
            content.append(&scrolled);
        }

        dialog.connect_response(|dlg, _| {
            dlg.close();
        });

        dialog.present();
    }

    fn show_error(window: &Window, message: &str) {
        let dialog = MessageDialog::builder()
            .transient_for(window)
            .modal(true)
            .message_type(MessageType::Error)
            .buttons(ButtonsType::Ok)
            .text(message)
            .build();

        dialog.connect_response(|dlg, _| {
            dlg.close();
        });

        dialog.present();
    }

    fn update_status(&self) {
        let (ssid, ip) = self.manager.borrow().get_status();
        if let Some(ssid) = ssid {
            let ip_str = ip.unwrap_or_else(|| "No IP".to_string());
            self.status_label.set_text(&format!("Connected: {} ({})", ssid, ip_str));
        } else {
            self.status_label.set_text("Not connected");
        }
    }

    fn scan_networks(&self) {
        let networks = self.manager.borrow().scan();
        Self::populate_network_list(
            &self.network_list,
            &networks,
            &self.manager,
            &self.status_label,
        );
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn show(&self) {
        self.window.present();
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
    }
}

impl Default for WiFiWidget {
    fn default() -> Self {
        Self::new()
    }
}
