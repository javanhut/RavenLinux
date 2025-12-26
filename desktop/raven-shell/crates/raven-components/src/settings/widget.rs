use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Image, Label, ListBox, ListBoxRow, Orientation,
    ScrolledWindow, Stack, StackTransitionType, Window,
};
use gtk4_layer_shell::{KeyboardMode, Layer, LayerShell};
use std::rc::Rc;
use std::sync::Arc;
use parking_lot::RwLock;

use raven_core::{ComponentId, RavenSettings, ShellEvent};
use crate::common::{Component, ComponentContext};
use crate::settings::pages::{self, SettingsCategory};

const SETTINGS_WIDTH: i32 = 800;
const SETTINGS_HEIGHT: i32 = 600;

/// Settings component - multi-page settings interface
pub struct SettingsComponent {
    window: Option<Window>,
    stack: Option<Rc<Stack>>,
    config: Option<Arc<RwLock<RavenSettings>>>,
    ctx: Option<ComponentContext>,
    initialized: bool,
}

impl SettingsComponent {
    pub fn new() -> Self {
        Self {
            window: None,
            stack: None,
            config: None,
            ctx: None,
            initialized: false,
        }
    }

    fn create_window(app: &gtk4::Application) -> Window {
        let window = Window::builder()
            .application(app)
            .title("Raven Settings")
            .decorated(false)
            .default_width(SETTINGS_WIDTH)
            .default_height(SETTINGS_HEIGHT)
            .build();

        // Initialize layer shell for overlay
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::Exclusive);

        window
    }

    fn build_ui(&self, ctx: &ComponentContext, window: &Window) -> GtkBox {
        let main_box = GtkBox::new(Orientation::Vertical, 0);
        main_box.add_css_class("settings-window");

        // Header
        let header = self.build_header(window);
        main_box.append(&header);

        // Content area (sidebar + pages)
        let content_box = GtkBox::new(Orientation::Horizontal, 0);
        content_box.set_vexpand(true);

        // Sidebar
        let sidebar = self.build_sidebar();
        content_box.append(&sidebar);

        // Stack for pages
        let stack = Stack::new();
        stack.set_hexpand(true);
        stack.set_transition_type(StackTransitionType::Crossfade);
        stack.set_transition_duration(200);
        stack.add_css_class("settings-content");

        // Add pages to stack
        let settings = ctx.config.clone();

        // Appearance
        let appearance_scroll = ScrolledWindow::new();
        appearance_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        appearance_scroll.set_child(Some(&pages::build_appearance_page(settings.clone())));
        stack.add_named(&appearance_scroll, Some("appearance"));

        // Desktop
        let desktop_scroll = ScrolledWindow::new();
        desktop_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        desktop_scroll.set_child(Some(&pages::build_desktop_page(settings.clone(), window)));
        stack.add_named(&desktop_scroll, Some("desktop"));

        // Panel
        let panel_scroll = ScrolledWindow::new();
        panel_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        panel_scroll.set_child(Some(&pages::build_panel_page(settings.clone())));
        stack.add_named(&panel_scroll, Some("panel"));

        // Windows
        let windows_scroll = ScrolledWindow::new();
        windows_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        windows_scroll.set_child(Some(&pages::build_windows_page(settings.clone())));
        stack.add_named(&windows_scroll, Some("windows"));

        // Input
        let input_scroll = ScrolledWindow::new();
        input_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        input_scroll.set_child(Some(&pages::build_input_page(settings.clone())));
        stack.add_named(&input_scroll, Some("input"));

        // Power
        let power_scroll = ScrolledWindow::new();
        power_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        power_scroll.set_child(Some(&pages::build_power_page(settings.clone())));
        stack.add_named(&power_scroll, Some("power"));

        // Sound
        let sound_scroll = ScrolledWindow::new();
        sound_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        sound_scroll.set_child(Some(&pages::build_sound_page(settings.clone())));
        stack.add_named(&sound_scroll, Some("sound"));

        // About
        let about_scroll = ScrolledWindow::new();
        about_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        about_scroll.set_child(Some(&pages::build_about_page()));
        stack.add_named(&about_scroll, Some("about"));

        // Set default page
        stack.set_visible_child_name("appearance");

        content_box.append(&stack);
        main_box.append(&content_box);

        main_box
    }

    fn build_header(&self, window: &Window) -> GtkBox {
        let header = GtkBox::new(Orientation::Horizontal, 16);
        header.add_css_class("settings-header");
        header.set_margin_start(24);
        header.set_margin_end(16);
        header.set_margin_top(16);
        header.set_margin_bottom(16);

        // Title section
        let title_box = GtkBox::new(Orientation::Vertical, 4);
        title_box.set_hexpand(true);

        let title = Label::new(Some("Raven Settings"));
        title.add_css_class("settings-title");
        title.set_halign(gtk4::Align::Start);
        title_box.append(&title);

        let subtitle = Label::new(Some("Configure your Raven desktop environment"));
        subtitle.add_css_class("settings-subtitle");
        subtitle.set_halign(gtk4::Align::Start);
        title_box.append(&subtitle);

        header.append(&title_box);

        // Close button
        let close_btn = Button::new();
        close_btn.add_css_class("settings-close-button");
        let close_icon = Image::from_icon_name("window-close-symbolic");
        close_btn.set_child(Some(&close_icon));
        let win = window.clone();
        close_btn.connect_clicked(move |_| {
            win.set_visible(false);
        });
        header.append(&close_btn);

        header
    }

    fn build_sidebar(&self) -> GtkBox {
        let sidebar = GtkBox::new(Orientation::Vertical, 0);
        sidebar.add_css_class("settings-sidebar");
        sidebar.set_size_request(220, -1);

        let scroll = ScrolledWindow::new();
        scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        scroll.set_vexpand(true);

        let list = ListBox::new();
        list.set_selection_mode(gtk4::SelectionMode::Single);
        list.add_css_class("settings-category-list");

        // Add category rows
        for category in SettingsCategory::all() {
            let row = self.create_category_row(*category);
            list.append(&row);

            // Select Appearance by default
            if *category == SettingsCategory::Appearance {
                list.select_row(Some(&row));
            }
        }

        scroll.set_child(Some(&list));
        sidebar.append(&scroll);

        sidebar
    }

    fn create_category_row(&self, category: SettingsCategory) -> ListBoxRow {
        let row = ListBoxRow::new();
        row.add_css_class("settings-category-row");

        let hbox = GtkBox::new(Orientation::Horizontal, 12);
        hbox.set_margin_start(16);
        hbox.set_margin_end(16);
        hbox.set_margin_top(12);
        hbox.set_margin_bottom(12);

        // Icon
        let icon = Image::from_icon_name(category.icon());
        icon.set_pixel_size(24);
        icon.add_css_class("settings-category-icon");
        hbox.append(&icon);

        // Text
        let text_box = GtkBox::new(Orientation::Vertical, 2);

        let name = Label::new(Some(category.name()));
        name.add_css_class("settings-category-name");
        name.set_halign(gtk4::Align::Start);
        text_box.append(&name);

        let desc = Label::new(Some(category.description()));
        desc.add_css_class("settings-category-desc");
        desc.set_halign(gtk4::Align::Start);
        text_box.append(&desc);

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

    fn setup_category_selection(&self, stack: Rc<Stack>, window: &Window) {
        // Find the sidebar listbox and connect selection
        if let Some(content) = window.child() {
            if let Some(main_box) = content.downcast_ref::<GtkBox>() {
                // Navigate to content box (second child)
                if let Some(child) = main_box.last_child() {
                    if let Some(content_box) = child.downcast_ref::<GtkBox>() {
                        // Get sidebar (first child)
                        if let Some(sidebar) = content_box.first_child() {
                            if let Some(sidebar_box) = sidebar.downcast_ref::<GtkBox>() {
                                if let Some(scroll) = sidebar_box.first_child() {
                                    if let Some(scroll_win) = scroll.downcast_ref::<ScrolledWindow>() {
                                        if let Some(list) = scroll_win.child() {
                                            if let Some(listbox) = list.downcast_ref::<ListBox>() {
                                                let stack_clone = stack.clone();
                                                listbox.connect_row_selected(move |_, row| {
                                                    if let Some(row) = row {
                                                        let idx = row.index() as usize;
                                                        let categories = SettingsCategory::all();
                                                        if let Some(category) = categories.get(idx) {
                                                            let page_name = match category {
                                                                SettingsCategory::Appearance => "appearance",
                                                                SettingsCategory::Desktop => "desktop",
                                                                SettingsCategory::Panel => "panel",
                                                                SettingsCategory::Windows => "windows",
                                                                SettingsCategory::Input => "input",
                                                                SettingsCategory::Power => "power",
                                                                SettingsCategory::Sound => "sound",
                                                                SettingsCategory::About => "about",
                                                            };
                                                            stack_clone.set_visible_child_name(page_name);
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Default for SettingsComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for SettingsComponent {
    fn id(&self) -> ComponentId {
        ComponentId::Settings
    }

    fn init(&mut self, ctx: ComponentContext) {
        if self.initialized {
            return;
        }

        let window = Self::create_window(&ctx.app);

        // Store config reference
        self.config = Some(ctx.config.clone());

        // Build UI
        let content = self.build_ui(&ctx, &window);
        window.set_child(Some(&content));

        // Find stack in the content for category selection
        if let Some(content_box) = content.last_child() {
            if let Some(cb) = content_box.downcast_ref::<GtkBox>() {
                if let Some(stack_widget) = cb.last_child() {
                    if let Some(stack) = stack_widget.downcast_ref::<Stack>() {
                        let stack_rc = Rc::new(stack.clone());
                        self.stack = Some(stack_rc.clone());
                        self.setup_category_selection(stack_rc, &window);
                    }
                }
            }
        }

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

    fn handle_event(&self, _event: &ShellEvent) {
        // Events handled by daemon
    }

    fn window(&self) -> Option<&Window> {
        self.window.as_ref()
    }
}
