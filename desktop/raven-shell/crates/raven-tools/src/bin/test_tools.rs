// Test binary for Phase 5 tools
// Usage: cargo run --bin test-tools -- [wifi|usb|installer]

use gtk4::prelude::*;
use gtk4::Application;
use raven_core::theme::load_css;
use raven_tools::{WiFiWidget, UsbWidget, InstallerWidget};
use std::cell::RefCell;
use std::env;
use std::rc::Rc;

fn main() {
    let args: Vec<String> = env::args().collect();
    let tool = args.get(1).map(|s| s.as_str()).unwrap_or("wifi");

    let app = Application::builder()
        .application_id("com.raven.tools.test")
        .build();

    let tool_name = tool.to_string();

    // Storage for widgets to keep them alive
    let wifi_widget: Rc<RefCell<Option<WiFiWidget>>> = Rc::new(RefCell::new(None));
    let usb_widget: Rc<RefCell<Option<UsbWidget>>> = Rc::new(RefCell::new(None));
    let installer_widget: Rc<RefCell<Option<InstallerWidget>>> = Rc::new(RefCell::new(None));

    let wifi_clone = wifi_widget.clone();
    let usb_clone = usb_widget.clone();
    let installer_clone = installer_widget.clone();

    app.connect_activate(move |app| {
        // Load CSS theme
        load_css();

        match tool_name.as_str() {
            "wifi" => {
                println!("Launching WiFi Manager...");
                let widget = WiFiWidget::new();
                let window = widget.window().clone();
                window.set_application(Some(app));
                *wifi_clone.borrow_mut() = Some(widget);
                window.present();

                window.connect_close_request(|_| {
                    std::process::exit(0);
                    #[allow(unreachable_code)]
                    glib::Propagation::Proceed
                });
            }
            "usb" => {
                println!("Launching USB Creator...");
                let widget = UsbWidget::new();
                let window = widget.window().clone();
                window.set_application(Some(app));
                *usb_clone.borrow_mut() = Some(widget);
                window.present();

                window.connect_close_request(|_| {
                    std::process::exit(0);
                    #[allow(unreachable_code)]
                    glib::Propagation::Proceed
                });
            }
            "installer" => {
                println!("Launching System Installer...");
                let widget = InstallerWidget::new();
                let window = widget.window().clone();
                window.set_application(Some(app));
                *installer_clone.borrow_mut() = Some(widget);
                window.present();

                window.connect_close_request(|_| {
                    std::process::exit(0);
                    #[allow(unreachable_code)]
                    glib::Propagation::Proceed
                });
            }
            _ => {
                eprintln!("Unknown tool: {}", tool_name);
                eprintln!("Usage: test-tools [wifi|usb|installer]");
                std::process::exit(1);
            }
        }
    });

    app.run_with_args::<String>(&[]);
}
