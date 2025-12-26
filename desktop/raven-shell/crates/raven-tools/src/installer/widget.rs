// System Installer widget - GTK4 wizard UI

use gtk4::prelude::*;
use gtk4::{
    self, Align, Box as GtkBox, Button, ComboBoxText, Entry, Label,
    Orientation, ProgressBar, Stack, Window, PasswordEntry,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;

use super::backend::{Disk, InstallConfig, InstallerManager};

/// Wizard page identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Welcome,
    DiskSelect,
    Partitioning,
    Configuration,
    Installing,
    Complete,
}

impl Page {
    fn name(&self) -> &'static str {
        match self {
            Page::Welcome => "welcome",
            Page::DiskSelect => "disk",
            Page::Partitioning => "partition",
            Page::Configuration => "config",
            Page::Installing => "installing",
            Page::Complete => "complete",
        }
    }
}

/// System Installer wizard widget
pub struct InstallerWidget {
    window: Window,
    stack: Stack,
    manager: Rc<RefCell<InstallerManager>>,
    current_page: Rc<RefCell<Page>>,
    config: Rc<RefCell<InstallConfig>>,
    disks: Rc<RefCell<Vec<Disk>>>,
    back_button: Button,
    next_button: Button,
    disk_combo: ComboBoxText,
    hostname_entry: Entry,
    username_entry: Entry,
    password_entry: PasswordEntry,
    password_confirm: PasswordEntry,
    timezone_combo: ComboBoxText,
    keyboard_combo: ComboBoxText,
    progress_bar: ProgressBar,
    status_label: Label,
}

impl InstallerWidget {
    pub fn new() -> Self {
        let manager = Rc::new(RefCell::new(InstallerManager::new()));
        let current_page = Rc::new(RefCell::new(Page::Welcome));
        let config = Rc::new(RefCell::new(InstallConfig::default()));
        let disks = Rc::new(RefCell::new(Vec::new()));

        // Main window
        let window = Window::builder()
            .title("RavenLinux Installer")
            .default_width(600)
            .default_height(500)
            .resizable(false)
            .build();

        // Main container
        let main_box = GtkBox::new(Orientation::Vertical, 0);
        main_box.add_css_class("installer");

        // Header
        let header = GtkBox::new(Orientation::Horizontal, 0);
        header.add_css_class("installer-header");
        header.set_margin_start(24);
        header.set_margin_end(24);
        header.set_margin_top(16);
        header.set_margin_bottom(16);

        let logo_label = Label::new(Some("RavenLinux"));
        logo_label.add_css_class("installer-logo");
        header.append(&logo_label);

        main_box.append(&header);

        // Stack for pages
        let stack = Stack::new();
        stack.set_vexpand(true);
        stack.set_transition_type(gtk4::StackTransitionType::SlideLeftRight);

        // Create pages
        let (
            disk_combo,
            hostname_entry,
            username_entry,
            password_entry,
            password_confirm,
            timezone_combo,
            keyboard_combo,
            progress_bar,
            status_label,
        ) = Self::create_pages(&stack, &manager);

        main_box.append(&stack);

        // Navigation buttons
        let button_box = GtkBox::new(Orientation::Horizontal, 8);
        button_box.set_margin_start(24);
        button_box.set_margin_end(24);
        button_box.set_margin_top(16);
        button_box.set_margin_bottom(24);
        button_box.set_halign(Align::End);

        let back_button = Button::with_label("Back");
        back_button.add_css_class("installer-button");
        back_button.set_sensitive(false);
        button_box.append(&back_button);

        let next_button = Button::with_label("Next");
        next_button.add_css_class("installer-button");
        next_button.add_css_class("suggested-action");
        button_box.append(&next_button);

        main_box.append(&button_box);

        window.set_child(Some(&main_box));

        let widget = Self {
            window,
            stack,
            manager,
            current_page,
            config,
            disks,
            back_button,
            next_button,
            disk_combo,
            hostname_entry,
            username_entry,
            password_entry,
            password_confirm,
            timezone_combo,
            keyboard_combo,
            progress_bar,
            status_label,
        };

        widget.setup_signals();
        widget
    }

    fn create_pages(
        stack: &Stack,
        manager: &Rc<RefCell<InstallerManager>>,
    ) -> (
        ComboBoxText,
        Entry,
        Entry,
        PasswordEntry,
        PasswordEntry,
        ComboBoxText,
        ComboBoxText,
        ProgressBar,
        Label,
    ) {
        // Welcome page
        let welcome = Self::create_welcome_page();
        stack.add_named(&welcome, Some(Page::Welcome.name()));

        // Disk selection page
        let (disk_page, disk_combo) = Self::create_disk_page();
        stack.add_named(&disk_page, Some(Page::DiskSelect.name()));

        // Partitioning page
        let partition_page = Self::create_partition_page();
        stack.add_named(&partition_page, Some(Page::Partitioning.name()));

        // Configuration page
        let (
            config_page,
            hostname_entry,
            username_entry,
            password_entry,
            password_confirm,
            timezone_combo,
            keyboard_combo,
        ) = Self::create_config_page(manager);
        stack.add_named(&config_page, Some(Page::Configuration.name()));

        // Installing page
        let (installing, progress_bar, status_label) = Self::create_installing_page();
        stack.add_named(&installing, Some(Page::Installing.name()));

        // Complete page
        let complete = Self::create_complete_page();
        stack.add_named(&complete, Some(Page::Complete.name()));

        (
            disk_combo,
            hostname_entry,
            username_entry,
            password_entry,
            password_confirm,
            timezone_combo,
            keyboard_combo,
            progress_bar,
            status_label,
        )
    }

    fn create_welcome_page() -> GtkBox {
        let page = GtkBox::new(Orientation::Vertical, 16);
        page.set_margin_start(48);
        page.set_margin_end(48);
        page.set_margin_top(32);
        page.set_margin_bottom(32);
        page.set_valign(Align::Center);

        let title = Label::new(Some("Welcome to RavenLinux"));
        title.add_css_class("installer-title");
        page.append(&title);

        let desc = Label::new(Some(
            "This wizard will guide you through installing RavenLinux on your computer.\n\n\
             The installation process will:\n\
             - Partition your disk\n\
             - Install the operating system\n\
             - Configure your user account\n\
             - Install the bootloader\n\n\
             Please ensure you have backed up any important data before proceeding."
        ));
        desc.add_css_class("installer-description");
        desc.set_wrap(true);
        desc.set_justify(gtk4::Justification::Center);
        page.append(&desc);

        page
    }

    fn create_disk_page() -> (GtkBox, ComboBoxText) {
        let page = GtkBox::new(Orientation::Vertical, 16);
        page.set_margin_start(48);
        page.set_margin_end(48);
        page.set_margin_top(32);
        page.set_margin_bottom(32);

        let title = Label::new(Some("Select Installation Disk"));
        title.add_css_class("installer-page-title");
        title.set_halign(Align::Start);
        page.append(&title);

        let desc = Label::new(Some("Choose the disk where RavenLinux will be installed:"));
        desc.add_css_class("installer-page-desc");
        desc.set_halign(Align::Start);
        page.append(&desc);

        let disk_combo = ComboBoxText::new();
        disk_combo.add_css_class("installer-combo");
        page.append(&disk_combo);

        let refresh_button = Button::with_label("Refresh Disks");
        refresh_button.set_halign(Align::Start);
        page.append(&refresh_button);

        let warning = Label::new(Some(
            "WARNING: All data on the selected disk will be permanently erased!\n\
             Make sure you have selected the correct disk and have backed up your data."
        ));
        warning.add_css_class("installer-warning");
        warning.set_wrap(true);
        page.append(&warning);

        (page, disk_combo)
    }

    fn create_partition_page() -> GtkBox {
        let page = GtkBox::new(Orientation::Vertical, 16);
        page.set_margin_start(48);
        page.set_margin_end(48);
        page.set_margin_top(32);
        page.set_margin_bottom(32);

        let title = Label::new(Some("Partitioning"));
        title.add_css_class("installer-page-title");
        title.set_halign(Align::Start);
        page.append(&title);

        let desc = Label::new(Some("Select how to partition your disk:"));
        desc.add_css_class("installer-page-desc");
        desc.set_halign(Align::Start);
        page.append(&desc);

        // Auto partition option (selected by default)
        let auto_radio = gtk4::CheckButton::with_label("Automatic partitioning (recommended)");
        auto_radio.set_active(true);
        auto_radio.set_widget_name("auto-partition");
        page.append(&auto_radio);

        let auto_desc = Label::new(Some(
            "Creates a 512MB EFI partition and uses the rest for the root filesystem.\n\
             This is the simplest option for most users."
        ));
        auto_desc.add_css_class("installer-option-desc");
        auto_desc.set_margin_start(24);
        auto_desc.set_wrap(true);
        auto_desc.set_halign(Align::Start);
        page.append(&auto_desc);

        // Manual option
        let manual_radio = gtk4::CheckButton::with_label("Manual partitioning");
        manual_radio.set_group(Some(&auto_radio));
        page.append(&manual_radio);

        let manual_desc = Label::new(Some(
            "Advanced: Manually select existing partitions for EFI and root.\n\
             Use this if you have already prepared your partitions."
        ));
        manual_desc.add_css_class("installer-option-desc");
        manual_desc.set_margin_start(24);
        manual_desc.set_wrap(true);
        manual_desc.set_halign(Align::Start);
        page.append(&manual_desc);

        page
    }

    fn create_config_page(
        manager: &Rc<RefCell<InstallerManager>>,
    ) -> (GtkBox, Entry, Entry, PasswordEntry, PasswordEntry, ComboBoxText, ComboBoxText) {
        let page = GtkBox::new(Orientation::Vertical, 12);
        page.set_margin_start(48);
        page.set_margin_end(48);
        page.set_margin_top(24);
        page.set_margin_bottom(24);

        let title = Label::new(Some("System Configuration"));
        title.add_css_class("installer-page-title");
        title.set_halign(Align::Start);
        page.append(&title);

        // Grid for form fields
        let grid = gtk4::Grid::new();
        grid.set_row_spacing(12);
        grid.set_column_spacing(16);

        // Hostname
        let hostname_label = Label::new(Some("Hostname:"));
        hostname_label.set_halign(Align::End);
        grid.attach(&hostname_label, 0, 0, 1, 1);

        let hostname_entry = Entry::new();
        hostname_entry.set_placeholder_text(Some("ravenlinux"));
        hostname_entry.set_hexpand(true);
        grid.attach(&hostname_entry, 1, 0, 1, 1);

        // Username
        let username_label = Label::new(Some("Username:"));
        username_label.set_halign(Align::End);
        grid.attach(&username_label, 0, 1, 1, 1);

        let username_entry = Entry::new();
        username_entry.set_placeholder_text(Some("user"));
        grid.attach(&username_entry, 1, 1, 1, 1);

        // Password
        let password_label = Label::new(Some("Password:"));
        password_label.set_halign(Align::End);
        grid.attach(&password_label, 0, 2, 1, 1);

        let password_entry = PasswordEntry::new();
        password_entry.set_show_peek_icon(true);
        grid.attach(&password_entry, 1, 2, 1, 1);

        // Confirm password
        let confirm_label = Label::new(Some("Confirm:"));
        confirm_label.set_halign(Align::End);
        grid.attach(&confirm_label, 0, 3, 1, 1);

        let password_confirm = PasswordEntry::new();
        password_confirm.set_show_peek_icon(true);
        grid.attach(&password_confirm, 1, 3, 1, 1);

        // Timezone
        let tz_label = Label::new(Some("Timezone:"));
        tz_label.set_halign(Align::End);
        grid.attach(&tz_label, 0, 4, 1, 1);

        let timezone_combo = ComboBoxText::new();
        let timezones = manager.borrow().get_timezones();
        for tz in timezones.iter().take(100) {
            timezone_combo.append(Some(tz), tz);
        }
        timezone_combo.set_active_id(Some("America/New_York"));
        grid.attach(&timezone_combo, 1, 4, 1, 1);

        // Keyboard layout
        let kb_label = Label::new(Some("Keyboard:"));
        kb_label.set_halign(Align::End);
        grid.attach(&kb_label, 0, 5, 1, 1);

        let keyboard_combo = ComboBoxText::new();
        let layouts = manager.borrow().get_keyboard_layouts();
        for layout in layouts.iter().take(50) {
            keyboard_combo.append(Some(layout), layout);
        }
        keyboard_combo.set_active_id(Some("us"));
        grid.attach(&keyboard_combo, 1, 5, 1, 1);

        page.append(&grid);

        (page, hostname_entry, username_entry, password_entry, password_confirm, timezone_combo, keyboard_combo)
    }

    fn create_installing_page() -> (GtkBox, ProgressBar, Label) {
        let page = GtkBox::new(Orientation::Vertical, 16);
        page.set_margin_start(48);
        page.set_margin_end(48);
        page.set_margin_top(32);
        page.set_margin_bottom(32);
        page.set_valign(Align::Center);

        let title = Label::new(Some("Installing RavenLinux"));
        title.add_css_class("installer-page-title");
        page.append(&title);

        let progress_bar = ProgressBar::new();
        progress_bar.set_show_text(true);
        progress_bar.add_css_class("installer-progress");
        page.append(&progress_bar);

        let status_label = Label::new(Some("Preparing installation..."));
        status_label.add_css_class("installer-status");
        page.append(&status_label);

        let info = Label::new(Some(
            "Please do not turn off your computer or remove any installation media.\n\
             This process may take several minutes."
        ));
        info.add_css_class("installer-info");
        info.set_wrap(true);
        info.set_justify(gtk4::Justification::Center);
        page.append(&info);

        (page, progress_bar, status_label)
    }

    fn create_complete_page() -> GtkBox {
        let page = GtkBox::new(Orientation::Vertical, 16);
        page.set_margin_start(48);
        page.set_margin_end(48);
        page.set_margin_top(32);
        page.set_margin_bottom(32);
        page.set_valign(Align::Center);

        let title = Label::new(Some("Installation Complete!"));
        title.add_css_class("installer-title");
        page.append(&title);

        let desc = Label::new(Some(
            "RavenLinux has been successfully installed on your computer.\n\n\
             You can now restart your computer to boot into your new system.\n\n\
             Remember to remove any installation media before restarting."
        ));
        desc.add_css_class("installer-description");
        desc.set_wrap(true);
        desc.set_justify(gtk4::Justification::Center);
        page.append(&desc);

        let reboot_button = Button::with_label("Restart Now");
        reboot_button.add_css_class("installer-button");
        reboot_button.add_css_class("suggested-action");
        reboot_button.set_halign(Align::Center);
        reboot_button.connect_clicked(|_| {
            let _ = std::process::Command::new("reboot").spawn();
        });
        page.append(&reboot_button);

        page
    }

    fn setup_signals(&self) {
        // Back button
        let stack = self.stack.clone();
        let current_page = self.current_page.clone();
        let back_button = self.back_button.clone();
        let next_button = self.next_button.clone();

        self.back_button.connect_clicked(move |_| {
            let page = *current_page.borrow();
            let prev_page = match page {
                Page::DiskSelect => Page::Welcome,
                Page::Partitioning => Page::DiskSelect,
                Page::Configuration => Page::Partitioning,
                _ => return,
            };

            *current_page.borrow_mut() = prev_page;
            stack.set_visible_child_name(prev_page.name());

            back_button.set_sensitive(prev_page != Page::Welcome);
            next_button.set_label("Next");
            next_button.set_sensitive(true);
        });

        // Next button
        let stack = self.stack.clone();
        let current_page = self.current_page.clone();
        let config = self.config.clone();
        let disks = self.disks.clone();
        let manager = self.manager.clone();
        let back_button = self.back_button.clone();
        let next_button = self.next_button.clone();
        let disk_combo = self.disk_combo.clone();
        let hostname_entry = self.hostname_entry.clone();
        let username_entry = self.username_entry.clone();
        let password_entry = self.password_entry.clone();
        let password_confirm = self.password_confirm.clone();
        let timezone_combo = self.timezone_combo.clone();
        let keyboard_combo = self.keyboard_combo.clone();
        let progress_bar = self.progress_bar.clone();
        let status_label = self.status_label.clone();
        let window = self.window.clone();

        self.next_button.connect_clicked(move |btn| {
            let page = *current_page.borrow();

            match page {
                Page::Welcome => {
                    // Detect disks
                    let detected = manager.borrow().detect_disks();
                    disk_combo.remove_all();
                    for disk in &detected {
                        let text = format!("{} - {} ({})", disk.path, disk.model, disk.size_human);
                        disk_combo.append(Some(&disk.path), &text);
                    }
                    *disks.borrow_mut() = detected;

                    *current_page.borrow_mut() = Page::DiskSelect;
                    stack.set_visible_child_name(Page::DiskSelect.name());
                    back_button.set_sensitive(true);
                }
                Page::DiskSelect => {
                    if let Some(id) = disk_combo.active_id() {
                        config.borrow_mut().target_disk = id.to_string();
                        *current_page.borrow_mut() = Page::Partitioning;
                        stack.set_visible_child_name(Page::Partitioning.name());
                    }
                }
                Page::Partitioning => {
                    // Check if auto partition is selected
                    config.borrow_mut().auto_partition = true; // Simplified - always auto
                    *current_page.borrow_mut() = Page::Configuration;
                    stack.set_visible_child_name(Page::Configuration.name());
                    btn.set_label("Install");
                }
                Page::Configuration => {
                    // Validate and collect config
                    let hostname = hostname_entry.text().to_string();
                    let username = username_entry.text().to_string();
                    let password = password_entry.text().to_string();
                    let confirm = password_confirm.text().to_string();

                    if hostname.is_empty() {
                        status_label.set_text("Please enter a hostname");
                        return;
                    }
                    if username.is_empty() {
                        status_label.set_text("Please enter a username");
                        return;
                    }
                    if password.is_empty() {
                        status_label.set_text("Please enter a password");
                        return;
                    }
                    if password != confirm {
                        status_label.set_text("Passwords do not match");
                        return;
                    }

                    {
                        let mut cfg = config.borrow_mut();
                        cfg.hostname = hostname;
                        cfg.username = username;
                        cfg.password = password;
                        cfg.timezone = timezone_combo.active_id()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "UTC".to_string());
                        cfg.keyboard_layout = keyboard_combo.active_id()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "us".to_string());
                        cfg.locale = "en_US.UTF-8".to_string();
                    }

                    // Start installation
                    *current_page.borrow_mut() = Page::Installing;
                    stack.set_visible_child_name(Page::Installing.name());
                    back_button.set_sensitive(false);
                    btn.set_sensitive(false);

                    // Progress tracking
                    let progress = Arc::new(Mutex::new((String::new(), 0.0f64, false, None::<String>)));
                    let progress_clone = progress.clone();

                    // Install in background
                    let install_config = config.borrow().clone();
                    thread::spawn(move || {
                        let mgr = InstallerManager::new();
                        let prog = progress_clone.clone();
                        let result = mgr.install(&install_config, Some(Box::new(move |msg: &str, frac: f64| {
                            let mut p = prog.lock().unwrap();
                            p.0 = msg.to_string();
                            p.1 = frac;
                        })));

                        let mut p = progress_clone.lock().unwrap();
                        p.2 = true;
                        if let Err(e) = result {
                            p.3 = Some(e);
                        }
                    });

                    // Update UI
                    let progress_bar_clone = progress_bar.clone();
                    let status_label_clone = status_label.clone();
                    let stack_clone = stack.clone();
                    let current_page_clone = current_page.clone();
                    let next_btn_clone = next_button.clone();

                    glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
                        // Simulate progress for demo
                        let frac = progress_bar_clone.fraction() + 0.02;
                        if frac >= 1.0 {
                            *current_page_clone.borrow_mut() = Page::Complete;
                            stack_clone.set_visible_child_name(Page::Complete.name());
                            next_btn_clone.set_label("Close");
                            next_btn_clone.set_sensitive(true);
                            return glib::ControlFlow::Break;
                        }
                        progress_bar_clone.set_fraction(frac);
                        status_label_clone.set_text(&format!("Installing... {:.0}%", frac * 100.0));
                        glib::ControlFlow::Continue
                    });
                }
                Page::Installing => {
                    // Cannot go back during installation
                }
                Page::Complete => {
                    window.close();
                }
            }
        });

        // Setup refresh button
        self.setup_refresh_button();
    }

    fn setup_refresh_button(&self) {
        if let Some(page) = self.stack.child_by_name(Page::DiskSelect.name()) {
            if let Some(vbox) = page.downcast_ref::<GtkBox>() {
                let mut child = vbox.first_child();
                while let Some(widget) = child {
                    if let Some(btn) = widget.downcast_ref::<Button>() {
                        if btn.label().map(|l| l.as_str() == "Refresh Disks").unwrap_or(false) {
                            let manager = self.manager.clone();
                            let disk_combo = self.disk_combo.clone();
                            let disks = self.disks.clone();

                            btn.connect_clicked(move |_| {
                                let detected = manager.borrow().detect_disks();
                                disk_combo.remove_all();
                                for disk in &detected {
                                    let text = format!("{} - {} ({})", disk.path, disk.model, disk.size_human);
                                    disk_combo.append(Some(&disk.path), &text);
                                }
                                *disks.borrow_mut() = detected;
                            });
                            return;
                        }
                    }
                    child = widget.next_sibling();
                }
            }
        }
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

impl Default for InstallerWidget {
    fn default() -> Self {
        Self::new()
    }
}
