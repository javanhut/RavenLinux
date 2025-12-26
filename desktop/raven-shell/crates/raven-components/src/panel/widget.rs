use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Orientation, Picture, Separator, Window};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::RefCell;
use std::rc::Rc;
use tokio::sync::mpsc;

use raven_core::{
    save_raven_icon, ComponentId, ConfigPaths, DockDiff, DockState, Orientation as PanelOrientation,
    PanelConfig, RavenSettings, ShellCommand, ShellEvent,
};

use crate::common::{Component, ComponentContext};
use crate::panel::menus::{PowerMenu, SettingsMenu};
use crate::panel::{Clock, Dock};

const PANEL_SIZE: i32 = 38;

/// Panel component (taskbar)
pub struct PanelComponent {
    window: Option<Window>,
    dock: Option<Rc<RefCell<Dock>>>,
    state: Option<Rc<RefCell<DockState>>>,
    orientation: Rc<RefCell<PanelOrientation>>,
    settings_menu: Rc<RefCell<Option<SettingsMenu>>>,
    power_menu: Rc<RefCell<Option<PowerMenu>>>,
    command_tx: Option<mpsc::Sender<ShellCommand>>,
    ctx: Option<ComponentContext>,
    paths: ConfigPaths,
    initialized: bool,
}

impl PanelComponent {
    pub fn new() -> Self {
        Self {
            window: None,
            dock: None,
            state: None,
            orientation: Rc::new(RefCell::new(PanelOrientation::Top)),
            settings_menu: Rc::new(RefCell::new(None)),
            power_menu: Rc::new(RefCell::new(None)),
            command_tx: None,
            ctx: None,
            paths: ConfigPaths::new(),
            initialized: false,
        }
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
        command_tx: mpsc::Sender<ShellCommand>,
        orientation: PanelOrientation,
        paths: &ConfigPaths,
        settings_menu: &Rc<RefCell<Option<SettingsMenu>>>,
        power_menu: &Rc<RefCell<Option<PowerMenu>>>,
        orientation_rc: &Rc<RefCell<PanelOrientation>>,
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

        let start_btn = Self::create_raven_button(command_tx.clone(), paths);
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

        let tx = command_tx.clone();
        let settings_menu_clone = settings_menu.clone();
        let power_menu_clone = power_menu.clone();
        let orientation_clone = orientation_rc.clone();
        settings_btn.connect_clicked(move |_| {
            // Close power menu if open
            if let Some(menu) = power_menu_clone.borrow().as_ref() {
                menu.close();
            }
            *power_menu_clone.borrow_mut() = None;

            // Toggle settings menu
            if settings_menu_clone.borrow().is_some() {
                if let Some(menu) = settings_menu_clone.borrow().as_ref() {
                    menu.close();
                }
                *settings_menu_clone.borrow_mut() = None;
            } else {
                let menu = SettingsMenu::new(tx.clone(), *orientation_clone.borrow());
                menu.present();
                *settings_menu_clone.borrow_mut() = Some(menu);
            }
        });
        end_box.append(&settings_btn);

        let sep2 = Separator::new(separator_orientation);
        end_box.append(&sep2);

        // Power button
        let power_btn = Button::with_label("Power");
        power_btn.add_css_class("power-button");

        let tx = command_tx.clone();
        let settings_menu_clone = settings_menu.clone();
        let power_menu_clone = power_menu.clone();
        let orientation_clone = orientation_rc.clone();
        power_btn.connect_clicked(move |_| {
            // Close settings menu if open
            if let Some(menu) = settings_menu_clone.borrow().as_ref() {
                menu.close();
            }
            *settings_menu_clone.borrow_mut() = None;

            // Toggle power menu
            if power_menu_clone.borrow().is_some() {
                if let Some(menu) = power_menu_clone.borrow().as_ref() {
                    menu.close();
                }
                *power_menu_clone.borrow_mut() = None;
            } else {
                let menu = PowerMenu::new(tx.clone(), *orientation_clone.borrow());
                menu.present();
                *power_menu_clone.borrow_mut() = Some(menu);
            }
        });
        end_box.append(&power_btn);

        main_box.append(&end_box);

        main_box
    }

    fn create_raven_button(command_tx: mpsc::Sender<ShellCommand>, paths: &ConfigPaths) -> Button {
        let btn = Button::new();
        btn.add_css_class("start-button");
        btn.set_tooltip_text(Some("Raven Menu"));

        // Create icon
        let icon_path = save_raven_icon(&paths.icon_cache_dir);

        let picture = Picture::for_filename(&icon_path);
        picture.set_can_shrink(true);
        picture.add_css_class("raven-icon");

        let icon_box = GtkBox::new(Orientation::Horizontal, 0);
        icon_box.append(&picture);
        btn.set_child(Some(&icon_box));

        btn.connect_clicked(move |_| {
            let _ = command_tx.blocking_send(ShellCommand::ToggleComponent(ComponentId::Menu));
        });

        btn
    }

    fn rebuild_panel(&self) {
        let Some(window) = &self.window else {
            return;
        };
        let Some(dock_rc) = &self.dock else {
            return;
        };
        let Some(state_rc) = &self.state else {
            return;
        };
        let Some(ref command_tx) = self.command_tx else {
            return;
        };

        let new_orientation = *self.orientation.borrow();

        // Update window size
        if new_orientation.is_vertical() {
            window.set_default_size(PANEL_SIZE, -1);
        } else {
            window.set_default_size(-1, PANEL_SIZE);
        }

        // Update anchors
        Self::set_anchors(window, new_orientation);

        // Rebuild dock with new orientation
        let mut new_dock = Dock::new(command_tx.clone(), new_orientation.is_vertical());

        // Re-apply current state to new dock
        for (index, item) in state_rc.borrow().items_ordered().enumerate() {
            let diff = DockDiff::Add {
                index,
                item: item.clone(),
            };
            new_dock.apply_diff(diff);
        }

        *dock_rc.borrow_mut() = new_dock;

        // Rebuild content
        let content = Self::build_content(
            &dock_rc.borrow(),
            command_tx.clone(),
            new_orientation,
            &self.paths,
            &self.settings_menu,
            &self.power_menu,
            &self.orientation,
        );
        window.set_child(Some(&content));

        // Save new orientation
        let mut settings = RavenSettings::load(&self.paths.raven_settings);
        settings.panel_position = new_orientation;
        let _ = settings.save();
    }

    fn start_event_handler(panel: Rc<RefCell<Self>>) {
        let Some(ctx) = panel.borrow().ctx.clone() else {
            return;
        };

        let event_rx = ctx.event_rx.clone();

        glib::spawn_future_local(async move {
            while let Ok(event) = event_rx.recv().await {
                panel.borrow().process_event(event);
            }
        });
    }

    fn process_event(&self, event: ShellEvent) {
        match &event {
            ShellEvent::SettingsReloaded(settings) => {
                let new_orientation = settings.panel_position;
                let current = *self.orientation.borrow();

                if new_orientation != current {
                    *self.orientation.borrow_mut() = new_orientation;
                    self.rebuild_panel();
                }
            }

            ShellEvent::ConfigReloaded(config) => {
                if let Some(state_rc) = &self.state {
                    if let Some(dock_rc) = &self.dock {
                        let diffs = state_rc.borrow_mut().load_pinned(config);
                        for diff in diffs {
                            dock_rc.borrow_mut().apply_diff(diff);
                        }
                    }
                }
            }

            _ => {
                // Apply event to state and get diffs
                if let Some(state_rc) = &self.state {
                    if let Some(dock_rc) = &self.dock {
                        let diffs = state_rc.borrow_mut().apply_event(&event);
                        for diff in diffs {
                            dock_rc.borrow_mut().apply_diff(diff);
                        }
                    }
                }
            }
        }
    }
}

impl Default for PanelComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for PanelComponent {
    fn id(&self) -> ComponentId {
        ComponentId::Panel
    }

    fn init(&mut self, ctx: ComponentContext) {
        if self.initialized {
            return;
        }

        let settings = ctx.settings();
        let orientation = settings.panel_position;
        *self.orientation.borrow_mut() = orientation;

        // Create main window
        let window = Window::builder()
            .application(&ctx.app)
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

        // Store command sender
        self.command_tx = Some(ctx.command_tx.clone());

        // Load panel config
        let panel_config = PanelConfig::load(&self.paths.dock_config);

        // Create dock with command sender
        let dock = Dock::new(ctx.command_tx.clone(), orientation.is_vertical());

        // Create state and load pinned apps
        let mut state = DockState::new();
        let initial_diffs = state.load_pinned(&panel_config);

        // Apply initial diffs to dock
        let dock = Rc::new(RefCell::new(dock));
        for diff in initial_diffs {
            dock.borrow_mut().apply_diff(diff);
        }

        // Store state and dock
        self.state = Some(Rc::new(RefCell::new(state)));
        self.dock = Some(dock.clone());

        // Build panel content
        let content = Self::build_content(
            &dock.borrow(),
            ctx.command_tx.clone(),
            orientation,
            &self.paths,
            &self.settings_menu,
            &self.power_menu,
            &self.orientation,
        );
        window.set_child(Some(&content));

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
        // Panel is always visible, do nothing
    }

    fn is_visible(&self) -> bool {
        self.window.as_ref().map(|w| w.is_visible()).unwrap_or(false)
    }

    fn handle_event(&self, event: &ShellEvent) {
        self.process_event(event.clone());
    }

    fn is_always_visible(&self) -> bool {
        true
    }

    fn window(&self) -> Option<&Window> {
        self.window.as_ref()
    }
}

/// Wrapper to start event handling for Panel
pub fn start_panel_events(panel: Rc<RefCell<PanelComponent>>) {
    PanelComponent::start_event_handler(panel);
}
