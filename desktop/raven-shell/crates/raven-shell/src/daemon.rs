use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

use gtk4::prelude::*;
use gtk4::Application;
use glib::ControlFlow;
use tracing::{error, info};

use raven_core::{
    load_css, ComponentId, ConfigPaths, RavenSettings, ServiceHub, ShellEvent,
};
use raven_components::{
    Component, ComponentContext,
    panel::PanelComponent,
    desktop::DesktopComponent,
    menu::MenuComponent,
    power::PowerComponent,
    settings::SettingsComponent,
    keybindings::KeybindingsComponent,
    file_manager::FileManagerComponent,
};

const APP_ID: &str = "org.ravenlinux.shell";

/// Run the shell daemon
pub fn run() -> anyhow::Result<()> {
    info!("Initializing Raven Shell daemon");

    // Create service hub (starts tokio runtime and services)
    let services = Arc::new(ServiceHub::new()?);
    let _guard = services.enter_runtime();

    // Load configuration
    let paths = ConfigPaths::new();
    let settings = RavenSettings::load(&paths.raven_settings);
    let config = Arc::new(RwLock::new(settings));

    // Create GTK application
    let app = Application::builder()
        .application_id(APP_ID)
        .flags(gtk4::gio::ApplicationFlags::NON_UNIQUE)
        .build();

    let services_clone = services.clone();
    let config_clone = config.clone();

    app.connect_activate(move |app| {
        // Load CSS theme
        load_css();

        // Create component context
        let ctx = ComponentContext::new(app, &services_clone, config_clone.clone());

        // Initialize components
        let mut components: HashMap<ComponentId, Box<dyn Component>> = HashMap::new();

        // Always-on components
        let mut panel = Box::new(PanelComponent::new());
        panel.init(ctx.clone());
        panel.show();
        components.insert(ComponentId::Panel, panel);

        let mut desktop = Box::new(DesktopComponent::new());
        desktop.init(ctx.clone());
        desktop.show();
        components.insert(ComponentId::Desktop, desktop);

        // Overlay components (lazy-loaded but created now)
        let mut menu = Box::new(MenuComponent::new());
        menu.init(ctx.clone());
        components.insert(ComponentId::Menu, menu);

        let mut power = Box::new(PowerComponent::new());
        power.init(ctx.clone());
        components.insert(ComponentId::Power, power);

        let mut settings = Box::new(SettingsComponent::new());
        settings.init(ctx.clone());
        components.insert(ComponentId::Settings, settings);

        let mut keybindings = Box::new(KeybindingsComponent::new());
        keybindings.init(ctx.clone());
        components.insert(ComponentId::Keybindings, keybindings);

        let mut file_manager = Box::new(FileManagerComponent::new());
        file_manager.init(ctx.clone());
        components.insert(ComponentId::FileManager, file_manager);

        // Wrap components for event loop
        let components = Arc::new(RwLock::new(components));

        // Start event dispatch loop
        let event_rx = services_clone.event_receiver();
        let components_clone = components.clone();

        glib::spawn_future_local(async move {
            while let Ok(event) = event_rx.recv().await {
                let components = components_clone.read();

                // Handle component visibility events
                match &event {
                    ShellEvent::ShowComponent(id) => {
                        if let Some(component) = components.get(id) {
                            component.show();
                        }
                    }
                    ShellEvent::HideComponent(id) => {
                        if let Some(component) = components.get(id) {
                            component.hide();
                        }
                    }
                    ShellEvent::ToggleComponent(id) => {
                        if let Some(component) = components.get(id) {
                            component.toggle();
                        }
                    }
                    _ => {}
                }

                // Forward event to all components
                for component in components.values() {
                    component.handle_event(&event);
                }
            }
        });

        info!("Raven Shell daemon activated");
    });

    // Run GTK main loop
    let exit_code = app.run();

    info!("Raven Shell daemon exiting");

    std::process::exit(exit_code.into());
}
