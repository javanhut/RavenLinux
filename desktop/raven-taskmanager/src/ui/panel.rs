use async_channel::Receiver;
use gtk4::prelude::*;
use gtk4::{
    Application, Box as GtkBox, Button, Orientation, Picture, Separator, Window,
};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::RefCell;
use std::rc::Rc;
use tokio::sync::mpsc;

use crate::config::{ConfigPaths, Orientation as PanelOrientation, PanelConfig, RavenSettings};
use crate::css;
use crate::messages::{PanelCommand, PanelEvent};
use crate::state::DockState;
use crate::ui::menus::{PowerMenu, SettingsMenu};
use crate::ui::{Clock, Dock};

const PANEL_SIZE: i32 = 38;

/// Main panel/taskbar widget
pub struct Panel {
    window: Window,
    dock: Rc<RefCell<Dock>>,
    state: Rc<RefCell<DockState>>,
    orientation: Rc<RefCell<PanelOrientation>>,
    command_tx: mpsc::Sender<PanelCommand>,
    paths: ConfigPaths,
    settings_menu: Rc<RefCell<Option<SettingsMenu>>>,
    power_menu: Rc<RefCell<Option<PowerMenu>>>,
}

impl Panel {
    pub fn new(
        app: &Application,
        event_rx: Receiver<PanelEvent>,
        command_tx: mpsc::Sender<PanelCommand>,
    ) -> Rc<Self> {
        let paths = ConfigPaths::new();

        // Load initial config
        let settings = RavenSettings::load(&paths.raven_settings);
        let panel_config = PanelConfig::load(&paths.dock_config);
        let orientation = settings.panel_position;

        // Create main window
        let window = Window::builder()
            .application(app)
            .title("Raven Panel")
            .decorated(false)
            .build();

        // Set size based on orientation
        if orientation.is_vertical() {
            window.set_default_size(PANEL_SIZE, -1);
        } else {
            window.set_default_size(-1, PANEL_SIZE);
        }

        // Initialize layer shell
        window.init_layer_shell();
        window.set_layer(Layer::Top);
        window.auto_exclusive_zone_enable();
        Self::set_anchors(&window, orientation);

        // Create dock
        let dock = Dock::new(command_tx.clone(), orientation.is_vertical());

        // Create state and load pinned apps
        let mut state = DockState::new();
        let initial_diffs = state.load_pinned(&panel_config);

        // Apply initial diffs to dock
        let dock = Rc::new(RefCell::new(dock));
        for diff in initial_diffs {
            dock.borrow_mut().apply_diff(diff);
        }

        // Build panel content
        let content = Self::build_content(
            &dock.borrow(),
            command_tx.clone(),
            orientation,
        );
        window.set_child(Some(&content));

        let panel = Rc::new(Self {
            window,
            dock,
            state: Rc::new(RefCell::new(state)),
            orientation: Rc::new(RefCell::new(orientation)),
            command_tx,
            paths,
            settings_menu: Rc::new(RefCell::new(None)),
            power_menu: Rc::new(RefCell::new(None)),
        });

        // Start event handler
        panel.clone().start_event_handler(event_rx);

        panel
    }

    fn set_anchors(window: &Window, orientation: PanelOrientation) {
        // Reset all anchors
        window.set_anchor(Edge::Top, false);
        window.set_anchor(Edge::Bottom, false);
        window.set_anchor(Edge::Left, false);
        window.set_anchor(Edge::Right, false);

        match orientation {
            PanelOrientation::Top => {
                window.set_anchor(Edge::Top, true);
                window.set_anchor(Edge::Left, true);
                window.set_anchor(Edge::Right, true);
            }
            PanelOrientation::Bottom => {
                window.set_anchor(Edge::Bottom, true);
                window.set_anchor(Edge::Left, true);
                window.set_anchor(Edge::Right, true);
            }
            PanelOrientation::Left => {
                window.set_anchor(Edge::Left, true);
                window.set_anchor(Edge::Top, true);
                window.set_anchor(Edge::Bottom, true);
            }
            PanelOrientation::Right => {
                window.set_anchor(Edge::Right, true);
                window.set_anchor(Edge::Top, true);
                window.set_anchor(Edge::Bottom, true);
            }
        }
    }

    fn build_content(
        dock: &Dock,
        command_tx: mpsc::Sender<PanelCommand>,
        orientation: PanelOrientation,
    ) -> GtkBox {
        let is_vertical = orientation.is_vertical();
        let box_orientation = if is_vertical {
            Orientation::Vertical
        } else {
            Orientation::Horizontal
        };
        let separator_orientation = if is_vertical {
            Orientation::Horizontal
        } else {
            Orientation::Vertical
        };

        let main_box = GtkBox::new(box_orientation, 0);
        main_box.set_homogeneous(false);
        main_box.add_css_class("panel-container");

        // Start section: Raven button
        let start_box = GtkBox::new(box_orientation, 6);
        if is_vertical {
            start_box.set_margin_top(6);
        } else {
            start_box.set_margin_start(6);
        }
        start_box.add_css_class("panel-section");

        let start_btn = Self::create_raven_button(command_tx.clone());
        start_box.append(&start_btn);

        main_box.append(&start_box);

        // First spacer
        let spacer1 = GtkBox::new(box_orientation, 0);
        if is_vertical {
            spacer1.set_vexpand(true);
        } else {
            spacer1.set_hexpand(true);
        }
        main_box.append(&spacer1);

        // Center: Dock
        main_box.append(dock.widget());

        // Second spacer
        let spacer2 = GtkBox::new(box_orientation, 0);
        if is_vertical {
            spacer2.set_vexpand(true);
        } else {
            spacer2.set_hexpand(true);
        }
        main_box.append(&spacer2);

        // End section: Clock + Settings + Power
        let end_box = GtkBox::new(box_orientation, 6);
        if is_vertical {
            end_box.set_margin_bottom(6);
        } else {
            end_box.set_margin_end(6);
        }
        end_box.add_css_class("panel-section");

        // Clock
        let clock = Clock::new();
        end_box.append(clock.widget());

        let sep1 = Separator::new(separator_orientation);
        end_box.append(&sep1);

        // Settings button
        let settings_btn = Button::with_label("Settings");
        settings_btn.add_css_class("settings-button");
        end_box.append(&settings_btn);

        let sep2 = Separator::new(separator_orientation);
        end_box.append(&sep2);

        // Power button
        let power_btn = Button::with_label("Power");
        power_btn.add_css_class("power-button");
        end_box.append(&power_btn);

        main_box.append(&end_box);

        main_box
    }

    fn create_raven_button(command_tx: mpsc::Sender<PanelCommand>) -> Button {
        let btn = Button::new();
        btn.add_css_class("start-button");
        btn.set_tooltip_text(Some("Raven Menu"));

        // Create icon
        let paths = ConfigPaths::new();
        let icon_path = css::save_raven_icon(&paths.icon_cache_dir);

        let picture = Picture::for_filename(&icon_path);
        picture.set_can_shrink(true);
        picture.add_css_class("raven-icon");

        let icon_box = GtkBox::new(Orientation::Horizontal, 0);
        icon_box.append(&picture);
        btn.set_child(Some(&icon_box));

        btn.connect_clicked(move |_| {
            let _ = command_tx.blocking_send(PanelCommand::LaunchApp("raven-menu".into()));
        });

        btn
    }

    fn start_event_handler(self: Rc<Self>, event_rx: Receiver<PanelEvent>) {
        let panel = self.clone();

        glib::spawn_future_local(async move {
            while let Ok(event) = event_rx.recv().await {
                panel.handle_event(event);
            }
        });
    }

    fn handle_event(&self, event: PanelEvent) {
        match &event {
            PanelEvent::SettingsReloaded(settings) => {
                let new_orientation = settings.panel_position;
                let current = *self.orientation.borrow();

                if new_orientation != current {
                    self.rebuild_panel(new_orientation);
                }
            }

            PanelEvent::ConfigReloaded(config) => {
                let diffs = self.state.borrow_mut().load_pinned(config);
                for diff in diffs {
                    self.dock.borrow_mut().apply_diff(diff);
                }
            }

            _ => {
                // Apply event to state and get diffs
                let diffs = self.state.borrow_mut().apply_event(&event);

                // Apply diffs to dock UI
                for diff in diffs {
                    self.dock.borrow_mut().apply_diff(diff);
                }
            }
        }
    }

    fn rebuild_panel(&self, new_orientation: PanelOrientation) {
        *self.orientation.borrow_mut() = new_orientation;

        // Update window size
        if new_orientation.is_vertical() {
            self.window.set_default_size(PANEL_SIZE, -1);
        } else {
            self.window.set_default_size(-1, PANEL_SIZE);
        }

        // Update anchors
        Self::set_anchors(&self.window, new_orientation);

        // Rebuild content
        let mut dock = Dock::new(self.command_tx.clone(), new_orientation.is_vertical());

        // Re-apply current state to new dock
        for item in self.state.borrow().items_ordered() {
            let diff = crate::state::DockDiff::Add {
                index: 0, // Will be appended
                item: item.clone(),
            };
            dock.apply_diff(diff);
        }

        *self.dock.borrow_mut() = dock;

        let content = Self::build_content(
            &self.dock.borrow(),
            self.command_tx.clone(),
            new_orientation,
        );
        self.window.set_child(Some(&content));

        // Save new orientation
        let mut settings = RavenSettings::load(&self.paths.raven_settings);
        settings.panel_position = new_orientation;
        let _ = settings.save(&self.paths.raven_settings);
    }

    pub fn show_settings_menu(&self) {
        // Close power menu if open
        if let Some(menu) = self.power_menu.borrow().as_ref() {
            menu.close();
        }
        *self.power_menu.borrow_mut() = None;

        // Toggle settings menu
        if self.settings_menu.borrow().is_some() {
            if let Some(menu) = self.settings_menu.borrow().as_ref() {
                menu.close();
            }
            *self.settings_menu.borrow_mut() = None;
        } else {
            let menu = SettingsMenu::new(
                self.command_tx.clone(),
                *self.orientation.borrow(),
            );
            menu.present();
            *self.settings_menu.borrow_mut() = Some(menu);
        }
    }

    pub fn show_power_menu(&self) {
        // Close settings menu if open
        if let Some(menu) = self.settings_menu.borrow().as_ref() {
            menu.close();
        }
        *self.settings_menu.borrow_mut() = None;

        // Toggle power menu
        if self.power_menu.borrow().is_some() {
            if let Some(menu) = self.power_menu.borrow().as_ref() {
                menu.close();
            }
            *self.power_menu.borrow_mut() = None;
        } else {
            let menu = PowerMenu::new(
                self.command_tx.clone(),
                *self.orientation.borrow(),
            );
            menu.present();
            *self.power_menu.borrow_mut() = Some(menu);
        }
    }

    pub fn present(&self) {
        self.window.present();
    }

    pub fn window(&self) -> &Window {
        &self.window
    }
}
