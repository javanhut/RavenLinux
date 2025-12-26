use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, GestureClick, Image, Label, Orientation, Window};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use tokio::sync::mpsc;

use raven_core::{ComponentId, ShellCommand, ShellEvent};
use crate::common::{Component, ComponentContext};

/// Power option definition
struct PowerOption {
    name: &'static str,
    icon: &'static str,
    description: &'static str,
    command: ShellCommand,
    css_class: Option<&'static str>,
}

/// Power menu component - fullscreen overlay
pub struct PowerComponent {
    window: Option<Window>,
    command_tx: Option<mpsc::Sender<ShellCommand>>,
    ctx: Option<ComponentContext>,
    initialized: bool,
}

impl PowerComponent {
    pub fn new() -> Self {
        Self {
            window: None,
            command_tx: None,
            ctx: None,
            initialized: false,
        }
    }

    fn power_options() -> Vec<PowerOption> {
        vec![
            PowerOption {
                name: "Lock",
                icon: "system-lock-screen-symbolic",
                description: "Lock the screen",
                command: ShellCommand::Lock,
                css_class: None,
            },
            PowerOption {
                name: "Logout",
                icon: "system-log-out-symbolic",
                description: "End the session",
                command: ShellCommand::Logout,
                css_class: None,
            },
            PowerOption {
                name: "Suspend",
                icon: "system-suspend-symbolic",
                description: "Sleep the computer",
                command: ShellCommand::Suspend,
                css_class: None,
            },
            PowerOption {
                name: "Hibernate",
                icon: "system-hibernate-symbolic",
                description: "Hibernate to disk",
                command: ShellCommand::Hibernate,
                css_class: None,
            },
            PowerOption {
                name: "Reboot",
                icon: "system-reboot-symbolic",
                description: "Restart the computer",
                command: ShellCommand::Reboot,
                css_class: Some("power-button-reboot"),
            },
            PowerOption {
                name: "Shutdown",
                icon: "system-shutdown-symbolic",
                description: "Power off",
                command: ShellCommand::Shutdown,
                css_class: Some("power-button-shutdown"),
            },
        ]
    }

    fn create_window(app: &gtk4::Application) -> Window {
        let window = Window::builder()
            .application(app)
            .title("Power Menu")
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

    fn build_ui(&self, command_tx: mpsc::Sender<ShellCommand>, window: &Window) -> GtkBox {
        let main_box = GtkBox::new(Orientation::Vertical, 24);
        main_box.add_css_class("power-overlay");
        main_box.set_halign(gtk4::Align::Center);
        main_box.set_valign(gtk4::Align::Center);

        // Title
        let title = Label::new(Some("Power Menu"));
        title.add_css_class("power-overlay-title");
        main_box.append(&title);

        // Subtitle
        let subtitle = Label::new(Some("Press Escape to cancel"));
        subtitle.add_css_class("power-overlay-subtitle");
        main_box.append(&subtitle);

        // Options grid
        let options_box = GtkBox::new(Orientation::Horizontal, 16);
        options_box.set_halign(gtk4::Align::Center);
        options_box.set_margin_top(24);

        for option in Self::power_options() {
            let btn = self.create_power_button(&option, command_tx.clone(), window);
            options_box.append(&btn);
        }

        main_box.append(&options_box);

        // Hint text
        let hint = Label::new(Some("Tip: Use Super + Escape to show this menu"));
        hint.add_css_class("power-overlay-hint");
        hint.set_margin_top(32);
        main_box.append(&hint);

        main_box
    }

    fn create_power_button(
        &self,
        option: &PowerOption,
        command_tx: mpsc::Sender<ShellCommand>,
        window: &Window,
    ) -> Button {
        let btn = Button::new();
        btn.add_css_class("power-overlay-button");

        if let Some(class) = option.css_class {
            btn.add_css_class(class);
        }

        let content = GtkBox::new(Orientation::Vertical, 8);
        content.set_halign(gtk4::Align::Center);

        // Icon
        let icon = Image::from_icon_name(option.icon);
        icon.set_pixel_size(48);
        icon.add_css_class("power-overlay-icon");
        content.append(&icon);

        // Name
        let name = Label::new(Some(option.name));
        name.add_css_class("power-overlay-button-name");
        content.append(&name);

        // Description
        let desc = Label::new(Some(option.description));
        desc.add_css_class("power-overlay-button-desc");
        content.append(&desc);

        btn.set_child(Some(&content));

        // Connect click
        let cmd = option.command.clone();
        let tx = command_tx.clone();
        let win = window.clone();
        btn.connect_clicked(move |_| {
            win.set_visible(false);
            let _ = tx.blocking_send(cmd.clone());
        });

        btn
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

    fn setup_click_to_close(&self, window: &Window) {
        let click_controller = GestureClick::new();
        let win = window.clone();
        click_controller.connect_pressed(move |_, _, _, _| {
            win.set_visible(false);
        });
        window.add_controller(click_controller);
    }
}

impl Default for PowerComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for PowerComponent {
    fn id(&self) -> ComponentId {
        ComponentId::Power
    }

    fn init(&mut self, ctx: ComponentContext) {
        if self.initialized {
            return;
        }

        self.command_tx = Some(ctx.command_tx.clone());

        let window = Self::create_window(&ctx.app);

        let content = self.build_ui(ctx.command_tx.clone(), &window);
        window.set_child(Some(&content));

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
