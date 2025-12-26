use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, GestureClick, Label, Orientation, ScrolledWindow, Window,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use raven_core::{ComponentId, ShellEvent};
use crate::common::{Component, ComponentContext};

/// Keybinding entry
struct Keybinding {
    key: &'static str,
    description: &'static str,
}

/// Keybinding category with entries
struct KeybindingCategory {
    name: &'static str,
    icon: &'static str,
    bindings: &'static [Keybinding],
}

/// Keybindings overlay component - fullscreen display of keyboard shortcuts
pub struct KeybindingsComponent {
    window: Option<Window>,
    ctx: Option<ComponentContext>,
    initialized: bool,
}

impl KeybindingsComponent {
    pub fn new() -> Self {
        Self {
            window: None,
            ctx: None,
            initialized: false,
        }
    }

    fn categories() -> &'static [KeybindingCategory] {
        &[
            KeybindingCategory {
                name: "Applications",
                icon: "application-x-executable-symbolic",
                bindings: &[
                    Keybinding { key: "Super + Return", description: "Open terminal" },
                    Keybinding { key: "Super + Space", description: "Application launcher" },
                    Keybinding { key: "Super + E", description: "File manager" },
                    Keybinding { key: "Super + B", description: "Web browser" },
                    Keybinding { key: "Super + N", description: "Text editor" },
                ],
            },
            KeybindingCategory {
                name: "Windows",
                icon: "window-new-symbolic",
                bindings: &[
                    Keybinding { key: "Super + Q", description: "Close active window" },
                    Keybinding { key: "Super + F", description: "Toggle fullscreen" },
                    Keybinding { key: "Super + V", description: "Toggle floating" },
                    Keybinding { key: "Super + P", description: "Toggle pseudo-tiling" },
                    Keybinding { key: "Super + S", description: "Toggle split direction" },
                ],
            },
            KeybindingCategory {
                name: "Focus",
                icon: "focus-windows-symbolic",
                bindings: &[
                    Keybinding { key: "Super + H", description: "Focus left" },
                    Keybinding { key: "Super + L", description: "Focus right" },
                    Keybinding { key: "Super + K", description: "Focus up" },
                    Keybinding { key: "Super + J", description: "Focus down" },
                    Keybinding { key: "Alt + Tab", description: "Cycle windows" },
                ],
            },
            KeybindingCategory {
                name: "Movement",
                icon: "transform-move-symbolic",
                bindings: &[
                    Keybinding { key: "Super + Shift + H", description: "Move window left" },
                    Keybinding { key: "Super + Shift + L", description: "Move window right" },
                    Keybinding { key: "Super + Shift + K", description: "Move window up" },
                    Keybinding { key: "Super + Shift + J", description: "Move window down" },
                    Keybinding { key: "Super + Mouse", description: "Move/resize window" },
                ],
            },
            KeybindingCategory {
                name: "Workspaces",
                icon: "view-grid-symbolic",
                bindings: &[
                    Keybinding { key: "Super + 1-9", description: "Switch to workspace" },
                    Keybinding { key: "Super + Shift + 1-9", description: "Move window to workspace" },
                    Keybinding { key: "Super + Scroll", description: "Cycle workspaces" },
                    Keybinding { key: "Super + Tab", description: "Previous workspace" },
                    Keybinding { key: "Super + Grave", description: "Special workspace" },
                ],
            },
            KeybindingCategory {
                name: "Media",
                icon: "multimedia-audio-player-symbolic",
                bindings: &[
                    Keybinding { key: "XF86AudioRaiseVolume", description: "Volume up" },
                    Keybinding { key: "XF86AudioLowerVolume", description: "Volume down" },
                    Keybinding { key: "XF86AudioMute", description: "Toggle mute" },
                    Keybinding { key: "XF86AudioPlay", description: "Play/Pause" },
                    Keybinding { key: "XF86MonBrightnessUp/Down", description: "Brightness control" },
                ],
            },
            KeybindingCategory {
                name: "Screenshots",
                icon: "camera-photo-symbolic",
                bindings: &[
                    Keybinding { key: "Print", description: "Screenshot (full)" },
                    Keybinding { key: "Super + Print", description: "Screenshot (window)" },
                    Keybinding { key: "Shift + Print", description: "Screenshot (region)" },
                    Keybinding { key: "Super + Shift + S", description: "Screenshot to clipboard" },
                ],
            },
            KeybindingCategory {
                name: "System",
                icon: "preferences-system-symbolic",
                bindings: &[
                    Keybinding { key: "Super + Escape", description: "Power menu" },
                    Keybinding { key: "Super + /", description: "Show keybindings (this)" },
                    Keybinding { key: "Super + ,", description: "Settings" },
                    Keybinding { key: "Super + L", description: "Lock screen" },
                    Keybinding { key: "Ctrl + Alt + Delete", description: "Logout" },
                ],
            },
        ]
    }

    fn create_window(app: &gtk4::Application) -> Window {
        let window = Window::builder()
            .application(app)
            .title("Keyboard Shortcuts")
            .decorated(false)
            .build();

        // Initialize layer shell for fullscreen overlay
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::Exclusive);

        // Anchor to all edges for fullscreen
        window.set_anchor(Edge::Top, true);
        window.set_anchor(Edge::Bottom, true);
        window.set_anchor(Edge::Left, true);
        window.set_anchor(Edge::Right, true);

        window
    }

    fn build_ui(&self, window: &Window) -> GtkBox {
        let main_box = GtkBox::new(Orientation::Vertical, 24);
        main_box.add_css_class("keybindings-overlay");
        main_box.set_halign(gtk4::Align::Center);
        main_box.set_valign(gtk4::Align::Center);

        // Title
        let title = Label::new(Some("Keyboard Shortcuts"));
        title.add_css_class("keybindings-title");
        main_box.append(&title);

        // Subtitle
        let subtitle = Label::new(Some("Press any key or click to dismiss"));
        subtitle.add_css_class("keybindings-subtitle");
        main_box.append(&subtitle);

        // Scrollable content area
        let scroll = ScrolledWindow::new();
        scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        scroll.set_max_content_height(600);
        scroll.set_propagate_natural_height(true);
        scroll.add_css_class("keybindings-scroll");

        // Two-column layout for categories
        let columns_box = GtkBox::new(Orientation::Horizontal, 48);
        columns_box.set_halign(gtk4::Align::Center);
        columns_box.set_margin_top(16);

        let left_column = GtkBox::new(Orientation::Vertical, 24);
        let right_column = GtkBox::new(Orientation::Vertical, 24);

        let categories = Self::categories();
        let mid = (categories.len() + 1) / 2;

        for (i, category) in categories.iter().enumerate() {
            let category_box = Self::create_category_box(category);
            if i < mid {
                left_column.append(&category_box);
            } else {
                right_column.append(&category_box);
            }
        }

        columns_box.append(&left_column);
        columns_box.append(&right_column);

        scroll.set_child(Some(&columns_box));
        main_box.append(&scroll);

        // Setup dismiss on any key
        let key_controller = gtk4::EventControllerKey::new();
        let win = window.clone();
        key_controller.connect_key_pressed(move |_, _, _, _| {
            win.set_visible(false);
            glib::Propagation::Stop
        });
        window.add_controller(key_controller);

        // Setup dismiss on click
        let click_controller = GestureClick::new();
        let win = window.clone();
        click_controller.connect_pressed(move |_, _, _, _| {
            win.set_visible(false);
        });
        window.add_controller(click_controller);

        main_box
    }

    fn create_category_box(category: &KeybindingCategory) -> GtkBox {
        let category_box = GtkBox::new(Orientation::Vertical, 8);
        category_box.add_css_class("keybindings-category");

        // Category header
        let header_box = GtkBox::new(Orientation::Horizontal, 8);
        header_box.add_css_class("keybindings-category-header");

        let icon = gtk4::Image::from_icon_name(category.icon);
        icon.set_pixel_size(20);
        icon.add_css_class("keybindings-category-icon");
        header_box.append(&icon);

        let name = Label::new(Some(category.name));
        name.add_css_class("keybindings-category-name");
        header_box.append(&name);

        category_box.append(&header_box);

        // Keybindings list
        for binding in category.bindings {
            let row = Self::create_binding_row(binding);
            category_box.append(&row);
        }

        category_box
    }

    fn create_binding_row(binding: &Keybinding) -> GtkBox {
        let row = GtkBox::new(Orientation::Horizontal, 16);
        row.add_css_class("keybindings-row");

        // Key combination
        let key_label = Label::new(Some(binding.key));
        key_label.add_css_class("keybindings-key");
        key_label.set_halign(gtk4::Align::Start);
        key_label.set_width_chars(24);
        row.append(&key_label);

        // Description
        let desc_label = Label::new(Some(binding.description));
        desc_label.add_css_class("keybindings-description");
        desc_label.set_halign(gtk4::Align::Start);
        row.append(&desc_label);

        row
    }
}

impl Default for KeybindingsComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for KeybindingsComponent {
    fn id(&self) -> ComponentId {
        ComponentId::Keybindings
    }

    fn init(&mut self, ctx: ComponentContext) {
        if self.initialized {
            return;
        }

        let window = Self::create_window(&ctx.app);

        let content = self.build_ui(&window);
        window.set_child(Some(&content));

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
