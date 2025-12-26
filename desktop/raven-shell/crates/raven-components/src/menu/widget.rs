use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Image, Label, ListBox, ListBoxRow, Orientation,
    ScrolledWindow, SearchEntry, Separator, Window,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::RefCell;
use std::rc::Rc;
use tokio::sync::mpsc;
use tracing::debug;

use raven_core::{ComponentId, ShellCommand, ShellEvent};

use crate::common::{Component, ComponentContext};
use crate::menu::apps::{AppCategory, AppDatabase, AppEntry};

const MENU_WIDTH: i32 = 350;
const MENU_HEIGHT: i32 = 500;

/// Application menu component
pub struct MenuComponent {
    window: Option<Window>,
    app_db: Option<Rc<RefCell<AppDatabase>>>,
    app_list: Option<Rc<ListBox>>,
    current_category: Rc<RefCell<AppCategory>>,
    command_tx: Option<mpsc::Sender<ShellCommand>>,
    ctx: Option<ComponentContext>,
    initialized: bool,
}

impl MenuComponent {
    pub fn new() -> Self {
        Self {
            window: None,
            app_db: None,
            app_list: None,
            current_category: Rc::new(RefCell::new(AppCategory::All)),
            command_tx: None,
            ctx: None,
            initialized: false,
        }
    }

    fn create_window(app: &gtk4::Application) -> Window {
        let window = Window::builder()
            .application(app)
            .title("Raven Menu")
            .decorated(false)
            .default_width(MENU_WIDTH)
            .default_height(MENU_HEIGHT)
            .build();

        // Initialize layer shell
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::Exclusive);

        // Anchor to top-left, below panel
        window.set_anchor(Edge::Top, true);
        window.set_anchor(Edge::Left, true);
        window.set_anchor(Edge::Bottom, true);
        window.set_margin(Edge::Top, 40); // Below panel
        window.set_margin(Edge::Left, 0);

        window
    }

    fn build_ui(
        &self,
        command_tx: mpsc::Sender<ShellCommand>,
        window_ref: &Window,
    ) -> GtkBox {
        let main_box = GtkBox::new(Orientation::Vertical, 0);
        main_box.add_css_class("menu-container");

        // Header
        let header = Label::new(Some("Raven"));
        header.add_css_class("menu-header");
        header.set_halign(gtk4::Align::Start);
        header.set_margin_start(16);
        header.set_margin_top(12);
        header.set_margin_bottom(8);
        main_box.append(&header);

        // Search entry
        let search_entry = SearchEntry::new();
        search_entry.set_placeholder_text(Some("Search applications..."));
        search_entry.add_css_class("menu-search");
        search_entry.set_margin_start(8);
        search_entry.set_margin_end(8);
        search_entry.set_margin_bottom(8);
        main_box.append(&search_entry);

        // Content area (categories + apps)
        let content_box = GtkBox::new(Orientation::Horizontal, 0);
        content_box.set_vexpand(true);

        // Categories sidebar
        let category_scroll = ScrolledWindow::new();
        category_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        category_scroll.set_min_content_width(100);
        category_scroll.add_css_class("menu-sidebar");

        let category_list = ListBox::new();
        category_list.set_selection_mode(gtk4::SelectionMode::Single);
        category_list.add_css_class("menu-category-list");

        // Populate categories
        if let Some(app_db) = &self.app_db {
            for category in app_db.borrow().active_categories() {
                let row = Self::create_category_row(category);
                category_list.append(&row);

                // Select "All" by default
                if category == AppCategory::All {
                    category_list.select_row(Some(&row));
                }
            }
        }

        category_scroll.set_child(Some(&category_list));
        content_box.append(&category_scroll);

        // Separator
        let sep = Separator::new(Orientation::Vertical);
        content_box.append(&sep);

        // Apps list
        let app_scroll = ScrolledWindow::new();
        app_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        app_scroll.set_hexpand(true);
        app_scroll.add_css_class("menu-app-scroll");

        let app_list = ListBox::new();
        app_list.set_selection_mode(gtk4::SelectionMode::Single);
        app_list.add_css_class("menu-app-list");

        // Populate initial apps
        Self::populate_apps_list(&app_list, &self.app_db, AppCategory::All);

        app_scroll.set_child(Some(&app_list));
        content_box.append(&app_scroll);

        main_box.append(&content_box);

        // Power section
        let power_box = GtkBox::new(Orientation::Horizontal, 8);
        power_box.add_css_class("menu-power-section");
        power_box.set_halign(gtk4::Align::Center);
        power_box.set_margin_top(8);
        power_box.set_margin_bottom(8);

        // Logout button
        let logout_btn = Button::with_label("Logout");
        logout_btn.add_css_class("menu-power-button");
        let tx = command_tx.clone();
        let win = window_ref.clone();
        logout_btn.connect_clicked(move |_| {
            win.set_visible(false);
            let _ = tx.blocking_send(ShellCommand::Logout);
        });
        power_box.append(&logout_btn);

        // Reboot button
        let reboot_btn = Button::with_label("Reboot");
        reboot_btn.add_css_class("menu-power-button");
        reboot_btn.add_css_class("menu-power-reboot");
        let tx = command_tx.clone();
        let win = window_ref.clone();
        reboot_btn.connect_clicked(move |_| {
            win.set_visible(false);
            let _ = tx.blocking_send(ShellCommand::Reboot);
        });
        power_box.append(&reboot_btn);

        // Shutdown button
        let shutdown_btn = Button::with_label("Shutdown");
        shutdown_btn.add_css_class("menu-power-button");
        shutdown_btn.add_css_class("menu-power-shutdown");
        let tx = command_tx.clone();
        let win = window_ref.clone();
        shutdown_btn.connect_clicked(move |_| {
            win.set_visible(false);
            let _ = tx.blocking_send(ShellCommand::Shutdown);
        });
        power_box.append(&shutdown_btn);

        main_box.append(&power_box);

        // Connect search
        let app_db = self.app_db.clone();
        let app_list_clone = app_list.clone();
        let current_category = self.current_category.clone();
        search_entry.connect_search_changed(move |entry| {
            let query = entry.text().to_string();
            Self::filter_apps(&app_list_clone, &app_db, &query, *current_category.borrow());
        });

        // Connect category selection
        let app_db = self.app_db.clone();
        let app_list_clone = app_list.clone();
        let current_category = self.current_category.clone();
        category_list.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                let idx = row.index() as usize;
                if let Some(ref db) = app_db {
                    let categories = db.borrow().active_categories();
                    if let Some(&category) = categories.get(idx) {
                        *current_category.borrow_mut() = category;
                        Self::populate_apps_list(&app_list_clone, &app_db, category);
                    }
                }
            }
        });

        // Connect app activation (double-click)
        let app_db = self.app_db.clone();
        let tx = command_tx.clone();
        let win = window_ref.clone();
        let current_category = self.current_category.clone();
        app_list.connect_row_activated(move |_, row| {
            let idx = row.index() as usize;
            if let Some(ref db) = app_db {
                let category = *current_category.borrow();
                let apps = db.borrow().apps_by_category(category);
                if let Some(app) = apps.get(idx) {
                    debug!("Launching: {}", app.exec);
                    let _ = tx.blocking_send(ShellCommand::LaunchApp(app.exec.clone()));
                    win.set_visible(false);
                }
            }
        });

        // Focus search on show
        let entry = search_entry.clone();
        glib::idle_add_local_once(move || {
            entry.grab_focus();
        });

        main_box
    }

    fn create_category_row(category: AppCategory) -> ListBoxRow {
        let row = ListBoxRow::new();
        row.add_css_class("menu-category-row");

        let label = Label::new(Some(category.name()));
        label.set_halign(gtk4::Align::Start);
        label.set_margin_start(12);
        label.set_margin_end(12);
        label.set_margin_top(8);
        label.set_margin_bottom(8);

        row.set_child(Some(&label));
        row
    }

    fn populate_apps_list(
        list: &ListBox,
        app_db: &Option<Rc<RefCell<AppDatabase>>>,
        category: AppCategory,
    ) {
        // Clear existing
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }

        if let Some(ref db) = app_db {
            let apps = db.borrow().apps_by_category(category);
            for app in &apps {
                let row = Self::create_app_row(app);
                list.append(&row);
            }
        }
    }

    fn filter_apps(
        list: &ListBox,
        app_db: &Option<Rc<RefCell<AppDatabase>>>,
        query: &str,
        category: AppCategory,
    ) {
        // Clear existing
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }

        if let Some(ref db) = app_db {
            let db_ref = db.borrow();
            let apps = if query.is_empty() {
                db_ref.apps_by_category(category)
            } else {
                db_ref.search(query)
            };

            for app in &apps {
                let row = Self::create_app_row(app);
                list.append(&row);
            }
        }
    }

    fn create_app_row(app: &AppEntry) -> ListBoxRow {
        let row = ListBoxRow::new();
        row.add_css_class("menu-app-row");

        let hbox = GtkBox::new(Orientation::Horizontal, 12);
        hbox.set_margin_start(12);
        hbox.set_margin_end(12);
        hbox.set_margin_top(8);
        hbox.set_margin_bottom(8);

        // Icon
        let icon = Image::from_icon_name(&app.icon);
        icon.set_pixel_size(32);
        icon.add_css_class("menu-app-icon");
        hbox.append(&icon);

        // Text content
        let text_box = GtkBox::new(Orientation::Vertical, 2);
        text_box.set_hexpand(true);

        let name_label = Label::new(Some(&app.name));
        name_label.add_css_class("menu-app-name");
        name_label.set_halign(gtk4::Align::Start);
        text_box.append(&name_label);

        if !app.comment.is_empty() {
            let comment_label = Label::new(Some(&app.comment));
            comment_label.add_css_class("menu-app-comment");
            comment_label.set_halign(gtk4::Align::Start);
            comment_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            text_box.append(&comment_label);
        }

        hbox.append(&text_box);
        row.set_child(Some(&hbox));
        row
    }

    fn setup_keyboard(&self, window: &Window) {
        let key_controller = gtk4::EventControllerKey::new();
        let win = window.clone();
        key_controller.connect_key_pressed(move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                win.set_visible(false);
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        window.add_controller(key_controller);
    }
}

impl Default for MenuComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for MenuComponent {
    fn id(&self) -> ComponentId {
        ComponentId::Menu
    }

    fn init(&mut self, ctx: ComponentContext) {
        if self.initialized {
            return;
        }

        // Store command sender
        self.command_tx = Some(ctx.command_tx.clone());

        // Load application database
        self.app_db = Some(Rc::new(RefCell::new(AppDatabase::new())));

        // Create window
        let window = Self::create_window(&ctx.app);

        // Build UI
        let content = self.build_ui(ctx.command_tx.clone(), &window);
        window.set_child(Some(&content));

        // Setup keyboard
        self.setup_keyboard(&window);

        // Hide initially
        window.set_visible(false);

        self.window = Some(window);
        self.ctx = Some(ctx);
        self.initialized = true;
    }

    fn show(&self) {
        if let Some(window) = &self.window {
            window.present();
        }
    }

    fn hide(&self) {
        if let Some(window) = &self.window {
            window.set_visible(false);
        }
    }

    fn is_visible(&self) -> bool {
        self.window.as_ref().map(|w| w.is_visible()).unwrap_or(false)
    }

    fn handle_event(&self, event: &ShellEvent) {
        match event {
            ShellEvent::SettingsReloaded(_) => {
                // Could reload app database if needed
            }
            _ => {}
        }
    }

    fn window(&self) -> Option<&Window> {
        self.window.as_ref()
    }
}
