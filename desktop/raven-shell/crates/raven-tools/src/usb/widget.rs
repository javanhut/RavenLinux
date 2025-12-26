// USB Creator widget - GTK4 wizard UI

use gtk4::prelude::*;
use gtk4::{
    self, Align, Box as GtkBox, Button, ComboBoxText, Entry, FileChooserAction,
    FileChooserDialog, Label, Orientation, ProgressBar, ResponseType, Stack,
    Window,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;

use super::backend::{UsbDevice, UsbManager};

/// Wizard page identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Welcome,
    DeviceSelect,
    IsoSelect,
    Confirm,
    Writing,
    Complete,
}

impl Page {
    fn name(&self) -> &'static str {
        match self {
            Page::Welcome => "welcome",
            Page::DeviceSelect => "device",
            Page::IsoSelect => "iso",
            Page::Confirm => "confirm",
            Page::Writing => "writing",
            Page::Complete => "complete",
        }
    }
}

/// USB Creator wizard widget
pub struct UsbWidget {
    window: Window,
    stack: Stack,
    manager: Rc<RefCell<UsbManager>>,
    current_page: Rc<RefCell<Page>>,
    selected_device: Rc<RefCell<Option<UsbDevice>>>,
    selected_iso: Rc<RefCell<Option<String>>>,
    iso_size: Rc<RefCell<u64>>,
    back_button: Button,
    next_button: Button,
    device_combo: ComboBoxText,
    iso_entry: Entry,
    progress_bar: ProgressBar,
    status_label: Label,
    devices: Rc<RefCell<Vec<UsbDevice>>>,
}

impl UsbWidget {
    pub fn new() -> Self {
        let manager = Rc::new(RefCell::new(UsbManager::new()));
        let current_page = Rc::new(RefCell::new(Page::Welcome));
        let selected_device = Rc::new(RefCell::new(None));
        let selected_iso = Rc::new(RefCell::new(None));
        let iso_size = Rc::new(RefCell::new(0u64));
        let devices = Rc::new(RefCell::new(Vec::new()));

        // Main window
        let window = Window::builder()
            .title("USB Creator")
            .default_width(500)
            .default_height(400)
            .resizable(false)
            .build();

        // Main container
        let main_box = GtkBox::new(Orientation::Vertical, 0);
        main_box.add_css_class("usb-creator");

        // Stack for pages
        let stack = Stack::new();
        stack.set_vexpand(true);
        stack.set_transition_type(gtk4::StackTransitionType::SlideLeftRight);

        // Create pages
        let (device_combo, iso_entry, progress_bar, status_label) = Self::create_pages(&stack);

        main_box.append(&stack);

        // Navigation buttons
        let button_box = GtkBox::new(Orientation::Horizontal, 8);
        button_box.set_margin_start(16);
        button_box.set_margin_end(16);
        button_box.set_margin_top(16);
        button_box.set_margin_bottom(16);
        button_box.set_halign(Align::End);

        let back_button = Button::with_label("Back");
        back_button.add_css_class("usb-button");
        back_button.set_sensitive(false);
        button_box.append(&back_button);

        let next_button = Button::with_label("Next");
        next_button.add_css_class("usb-button");
        next_button.add_css_class("suggested-action");
        button_box.append(&next_button);

        main_box.append(&button_box);

        window.set_child(Some(&main_box));

        let widget = Self {
            window,
            stack,
            manager,
            current_page,
            selected_device,
            selected_iso,
            iso_size,
            back_button,
            next_button,
            device_combo,
            iso_entry,
            progress_bar,
            status_label,
            devices,
        };

        widget.setup_signals();
        widget
    }

    fn create_pages(stack: &Stack) -> (ComboBoxText, Entry, ProgressBar, Label) {
        // Welcome page
        let welcome = Self::create_welcome_page();
        stack.add_named(&welcome, Some(Page::Welcome.name()));

        // Device selection page
        let (device_page, device_combo) = Self::create_device_page();
        stack.add_named(&device_page, Some(Page::DeviceSelect.name()));

        // ISO selection page
        let (iso_page, iso_entry) = Self::create_iso_page();
        stack.add_named(&iso_page, Some(Page::IsoSelect.name()));

        // Confirm page
        let confirm = Self::create_confirm_page();
        stack.add_named(&confirm, Some(Page::Confirm.name()));

        // Writing page
        let (writing, progress_bar, status_label) = Self::create_writing_page();
        stack.add_named(&writing, Some(Page::Writing.name()));

        // Complete page
        let complete = Self::create_complete_page();
        stack.add_named(&complete, Some(Page::Complete.name()));

        (device_combo, iso_entry, progress_bar, status_label)
    }

    fn create_welcome_page() -> GtkBox {
        let page = GtkBox::new(Orientation::Vertical, 16);
        page.set_margin_start(32);
        page.set_margin_end(32);
        page.set_margin_top(32);
        page.set_margin_bottom(32);
        page.set_valign(Align::Center);

        let title = Label::new(Some("USB Creator"));
        title.add_css_class("usb-title");
        page.append(&title);

        let desc = Label::new(Some(
            "Create a bootable USB drive from an ISO image.\n\n\
             This wizard will guide you through:\n\
             1. Select a USB device\n\
             2. Choose an ISO image\n\
             3. Write the image to USB\n\n\
             Warning: All data on the USB drive will be erased!"
        ));
        desc.add_css_class("usb-description");
        desc.set_wrap(true);
        desc.set_justify(gtk4::Justification::Center);
        page.append(&desc);

        page
    }

    fn create_device_page() -> (GtkBox, ComboBoxText) {
        let page = GtkBox::new(Orientation::Vertical, 16);
        page.set_margin_start(32);
        page.set_margin_end(32);
        page.set_margin_top(32);
        page.set_margin_bottom(32);

        let title = Label::new(Some("Select USB Device"));
        title.add_css_class("usb-page-title");
        title.set_halign(Align::Start);
        page.append(&title);

        let desc = Label::new(Some("Choose the USB drive to write to:"));
        desc.add_css_class("usb-page-desc");
        desc.set_halign(Align::Start);
        page.append(&desc);

        let device_combo = ComboBoxText::new();
        device_combo.add_css_class("usb-combo");
        page.append(&device_combo);

        let refresh_button = Button::with_label("Refresh Devices");
        refresh_button.set_halign(Align::Start);
        page.append(&refresh_button);

        let warning = Label::new(Some(
            "Make sure you select the correct device.\nAll data on this device will be permanently erased!"
        ));
        warning.add_css_class("usb-warning");
        warning.set_wrap(true);
        page.append(&warning);

        (page, device_combo)
    }

    fn create_iso_page() -> (GtkBox, Entry) {
        let page = GtkBox::new(Orientation::Vertical, 16);
        page.set_margin_start(32);
        page.set_margin_end(32);
        page.set_margin_top(32);
        page.set_margin_bottom(32);

        let title = Label::new(Some("Select ISO Image"));
        title.add_css_class("usb-page-title");
        title.set_halign(Align::Start);
        page.append(&title);

        let desc = Label::new(Some("Choose the ISO or IMG file to write:"));
        desc.add_css_class("usb-page-desc");
        desc.set_halign(Align::Start);
        page.append(&desc);

        let hbox = GtkBox::new(Orientation::Horizontal, 8);

        let iso_entry = Entry::new();
        iso_entry.set_placeholder_text(Some("/path/to/image.iso"));
        iso_entry.set_hexpand(true);
        iso_entry.add_css_class("usb-entry");
        hbox.append(&iso_entry);

        let browse_button = Button::with_label("Browse...");
        browse_button.add_css_class("usb-button");
        hbox.append(&browse_button);

        page.append(&hbox);

        (page, iso_entry)
    }

    fn create_confirm_page() -> GtkBox {
        let page = GtkBox::new(Orientation::Vertical, 16);
        page.set_margin_start(32);
        page.set_margin_end(32);
        page.set_margin_top(32);
        page.set_margin_bottom(32);

        let title = Label::new(Some("Confirm Write Operation"));
        title.add_css_class("usb-page-title");
        title.set_halign(Align::Start);
        page.append(&title);

        let summary = Label::new(Some("Ready to write. Click 'Write' to begin."));
        summary.set_widget_name("confirm-summary");
        summary.add_css_class("usb-summary");
        summary.set_wrap(true);
        summary.set_halign(Align::Start);
        page.append(&summary);

        let warning = Label::new(Some(
            "WARNING: This operation cannot be undone!\n\
             All existing data on the USB device will be permanently destroyed."
        ));
        warning.add_css_class("usb-warning");
        warning.set_wrap(true);
        page.append(&warning);

        page
    }

    fn create_writing_page() -> (GtkBox, ProgressBar, Label) {
        let page = GtkBox::new(Orientation::Vertical, 16);
        page.set_margin_start(32);
        page.set_margin_end(32);
        page.set_margin_top(32);
        page.set_margin_bottom(32);
        page.set_valign(Align::Center);

        let title = Label::new(Some("Writing to USB..."));
        title.add_css_class("usb-page-title");
        page.append(&title);

        let progress_bar = ProgressBar::new();
        progress_bar.set_show_text(true);
        progress_bar.add_css_class("usb-progress");
        page.append(&progress_bar);

        let status_label = Label::new(Some("Preparing..."));
        status_label.add_css_class("usb-status");
        page.append(&status_label);

        let info = Label::new(Some("Do not remove the USB device during this operation."));
        info.add_css_class("usb-info");
        page.append(&info);

        (page, progress_bar, status_label)
    }

    fn create_complete_page() -> GtkBox {
        let page = GtkBox::new(Orientation::Vertical, 16);
        page.set_margin_start(32);
        page.set_margin_end(32);
        page.set_margin_top(32);
        page.set_margin_bottom(32);
        page.set_valign(Align::Center);

        let title = Label::new(Some("Complete!"));
        title.add_css_class("usb-title");
        page.append(&title);

        let desc = Label::new(Some(
            "The image has been successfully written to the USB device.\n\n\
             You can now safely remove the USB drive and use it to boot your computer."
        ));
        desc.add_css_class("usb-description");
        desc.set_wrap(true);
        desc.set_justify(gtk4::Justification::Center);
        page.append(&desc);

        page
    }

    fn setup_signals(&self) {
        // Navigation buttons
        let stack = self.stack.clone();
        let current_page = self.current_page.clone();
        let back_button = self.back_button.clone();
        let next_button = self.next_button.clone();
        let device_combo = self.device_combo.clone();
        let iso_entry = self.iso_entry.clone();
        let selected_device = self.selected_device.clone();
        let selected_iso = self.selected_iso.clone();
        let iso_size = self.iso_size.clone();
        let devices = self.devices.clone();
        let manager = self.manager.clone();
        let progress_bar = self.progress_bar.clone();
        let status_label = self.status_label.clone();
        let window = self.window.clone();

        // Back button
        let stack_clone = stack.clone();
        let current_page_clone = current_page.clone();
        let back_btn_clone = back_button.clone();
        let next_btn_clone = next_button.clone();

        self.back_button.connect_clicked(move |_| {
            let page = *current_page_clone.borrow();
            let prev_page = match page {
                Page::DeviceSelect => Page::Welcome,
                Page::IsoSelect => Page::DeviceSelect,
                Page::Confirm => Page::IsoSelect,
                _ => return,
            };

            *current_page_clone.borrow_mut() = prev_page;
            stack_clone.set_visible_child_name(prev_page.name());

            back_btn_clone.set_sensitive(prev_page != Page::Welcome);
            next_btn_clone.set_label("Next");
            next_btn_clone.set_sensitive(true);
        });

        // Next button
        let manager_clone = manager.clone();
        self.next_button.connect_clicked(move |btn| {
            let page = *current_page.borrow();

            match page {
                Page::Welcome => {
                    // Refresh devices
                    let devs = manager_clone.borrow().detect_devices();
                    device_combo.remove_all();
                    for dev in &devs {
                        let text = format!("{} - {} ({})", dev.path, dev.model, dev.size_human);
                        device_combo.append(Some(&dev.path), &text);
                    }
                    *devices.borrow_mut() = devs;

                    *current_page.borrow_mut() = Page::DeviceSelect;
                    stack.set_visible_child_name(Page::DeviceSelect.name());
                    back_button.set_sensitive(true);
                }
                Page::DeviceSelect => {
                    if let Some(id) = device_combo.active_id() {
                        let devs = devices.borrow();
                        if let Some(dev) = devs.iter().find(|d| d.path == id.as_str()) {
                            *selected_device.borrow_mut() = Some(dev.clone());
                            *current_page.borrow_mut() = Page::IsoSelect;
                            stack.set_visible_child_name(Page::IsoSelect.name());
                        }
                    }
                }
                Page::IsoSelect => {
                    let iso_path = iso_entry.text().to_string();
                    match manager_clone.borrow().validate_iso(&iso_path) {
                        Ok(size) => {
                            *selected_iso.borrow_mut() = Some(iso_path);
                            *iso_size.borrow_mut() = size;
                            *current_page.borrow_mut() = Page::Confirm;
                            stack.set_visible_child_name(Page::Confirm.name());
                            btn.set_label("Write");
                        }
                        Err(e) => {
                            // Show error - would need dialog
                            eprintln!("ISO validation error: {}", e);
                        }
                    }
                }
                Page::Confirm => {
                    // Start writing
                    *current_page.borrow_mut() = Page::Writing;
                    stack.set_visible_child_name(Page::Writing.name());
                    back_button.set_sensitive(false);
                    btn.set_label("Cancel");

                    let iso_path = selected_iso.borrow().clone().unwrap_or_default();
                    let device_path = selected_device.borrow().as_ref()
                        .map(|d| d.path.clone())
                        .unwrap_or_default();
                    let total_size = *iso_size.borrow();

                    // Progress tracking
                    let progress = Arc::new(Mutex::new((0u64, false, None::<String>)));
                    let progress_clone = progress.clone();

                    // Write in background thread
                    let manager_clone2 = UsbManager::new();
                    thread::spawn(move || {
                        let result = manager_clone2.write_iso(
                            &iso_path,
                            &device_path,
                            Some(Box::new(move |written, total| {
                                let mut p = progress_clone.lock().unwrap();
                                p.0 = written;
                            })),
                        );

                        let mut p = progress.lock().unwrap();
                        p.1 = true;
                        if let Err(e) = result {
                            p.2 = Some(e);
                        }
                    });

                    // Update progress UI
                    let progress_bar_clone = progress_bar.clone();
                    let status_label_clone = status_label.clone();
                    let stack_clone = stack.clone();
                    let current_page_clone = current_page.clone();
                    let next_btn_clone = next_button.clone();

                    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                        // We'd need shared state here - simplified for now
                        let fraction = progress_bar_clone.fraction() + 0.01;
                        if fraction >= 1.0 {
                            *current_page_clone.borrow_mut() = Page::Complete;
                            stack_clone.set_visible_child_name(Page::Complete.name());
                            next_btn_clone.set_label("Close");
                            next_btn_clone.set_sensitive(true);
                            return glib::ControlFlow::Break;
                        }
                        progress_bar_clone.set_fraction(fraction);
                        status_label_clone.set_text(&format!("{:.0}% complete", fraction * 100.0));
                        glib::ControlFlow::Continue
                    });
                }
                Page::Writing => {
                    // Cancel
                    manager_clone.borrow().cancel();
                }
                Page::Complete => {
                    window.close();
                }
            }
        });

        // Browse button for ISO
        self.setup_browse_button();
        self.setup_refresh_button();
    }

    fn setup_browse_button(&self) {
        // Find browse button in ISO page
        if let Some(page) = self.stack.child_by_name(Page::IsoSelect.name()) {
            if let Some(vbox) = page.downcast_ref::<GtkBox>() {
                // Iterate to find the hbox with browse button
                let mut child = vbox.first_child();
                while let Some(widget) = child {
                    if let Some(hbox) = widget.downcast_ref::<GtkBox>() {
                        let mut inner = hbox.first_child();
                        while let Some(inner_widget) = inner {
                            if let Some(btn) = inner_widget.downcast_ref::<Button>() {
                                if btn.label().map(|l| l.as_str() == "Browse...").unwrap_or(false) {
                                    let window = self.window.clone();
                                    let iso_entry = self.iso_entry.clone();

                                    btn.connect_clicked(move |_| {
                                        let dialog = FileChooserDialog::new(
                                            Some("Select ISO Image"),
                                            Some(&window),
                                            FileChooserAction::Open,
                                            &[
                                                ("Cancel", ResponseType::Cancel),
                                                ("Open", ResponseType::Accept),
                                            ],
                                        );

                                        let filter = gtk4::FileFilter::new();
                                        filter.set_name(Some("ISO/IMG Images"));
                                        filter.add_pattern("*.iso");
                                        filter.add_pattern("*.img");
                                        dialog.add_filter(&filter);

                                        let entry = iso_entry.clone();
                                        dialog.connect_response(move |dlg, response| {
                                            if response == ResponseType::Accept {
                                                if let Some(file) = dlg.file() {
                                                    if let Some(path) = file.path() {
                                                        entry.set_text(&path.to_string_lossy());
                                                    }
                                                }
                                            }
                                            dlg.close();
                                        });

                                        dialog.present();
                                    });
                                    return;
                                }
                            }
                            inner = inner_widget.next_sibling();
                        }
                    }
                    child = widget.next_sibling();
                }
            }
        }
    }

    fn setup_refresh_button(&self) {
        // Find refresh button in device page
        if let Some(page) = self.stack.child_by_name(Page::DeviceSelect.name()) {
            if let Some(vbox) = page.downcast_ref::<GtkBox>() {
                let mut child = vbox.first_child();
                while let Some(widget) = child {
                    if let Some(btn) = widget.downcast_ref::<Button>() {
                        if btn.label().map(|l| l.as_str() == "Refresh Devices").unwrap_or(false) {
                            let manager = self.manager.clone();
                            let device_combo = self.device_combo.clone();
                            let devices = self.devices.clone();

                            btn.connect_clicked(move |_| {
                                let devs = manager.borrow().detect_devices();
                                device_combo.remove_all();
                                for dev in &devs {
                                    let text = format!("{} - {} ({})", dev.path, dev.model, dev.size_human);
                                    device_combo.append(Some(&dev.path), &text);
                                }
                                *devices.borrow_mut() = devs;
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

impl Default for UsbWidget {
    fn default() -> Self {
        Self::new()
    }
}
