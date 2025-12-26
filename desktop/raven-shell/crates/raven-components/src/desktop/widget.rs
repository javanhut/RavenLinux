use gtk4::prelude::*;
use gtk4::gdk::DragAction;
use gtk4::{DropTarget, GestureClick, Overlay, Picture, Window};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use tokio::sync::mpsc;
use tracing::debug;

use raven_core::{ComponentId, ConfigPaths, RavenSettings, ShellCommand, ShellEvent};

use crate::common::{Component, ComponentContext};
use crate::desktop::context_menu::DesktopContextMenu;
use crate::desktop::icons::IconGrid;

/// Default wallpaper search paths
const WALLPAPER_PATHS: &[&str] = &[
    "/usr/share/backgrounds/raven-wallpaper.png",
    "/usr/share/backgrounds/raven-sky.ppm",
    "/usr/share/backgrounds/default.png",
    "/usr/share/backgrounds/gnome/adwaita-l.jpg",
];

/// Desktop background component
pub struct DesktopComponent {
    window: Option<Window>,
    wallpaper: Option<Picture>,
    icon_grid: Option<Rc<RefCell<IconGrid>>>,
    context_menu: Rc<RefCell<Option<DesktopContextMenu>>>,
    command_tx: Option<mpsc::Sender<ShellCommand>>,
    ctx: Option<ComponentContext>,
    paths: ConfigPaths,
    initialized: bool,
}

impl DesktopComponent {
    pub fn new() -> Self {
        Self {
            window: None,
            wallpaper: None,
            icon_grid: None,
            context_menu: Rc::new(RefCell::new(None)),
            command_tx: None,
            ctx: None,
            paths: ConfigPaths::new(),
            initialized: false,
        }
    }

    fn create_window(app: &gtk4::Application) -> Window {
        let window = Window::builder()
            .application(app)
            .title("Raven Desktop")
            .decorated(false)
            .build();

        // Initialize layer shell for desktop background
        window.init_layer_shell();
        window.set_layer(Layer::Background);

        // Anchor to all edges for full coverage
        window.set_anchor(Edge::Top, true);
        window.set_anchor(Edge::Bottom, true);
        window.set_anchor(Edge::Left, true);
        window.set_anchor(Edge::Right, true);

        // No exclusive zone
        window.set_exclusive_zone(-1);

        window
    }

    fn find_wallpaper(settings: &RavenSettings) -> Option<PathBuf> {
        // First check custom wallpaper from settings
        if let Some(ref path) = settings.wallpaper_path {
            let path = PathBuf::from(path);
            if path.exists() {
                debug!("Using wallpaper from settings: {:?}", path);
                return Some(path);
            }
        }

        // Search default locations
        for path_str in WALLPAPER_PATHS {
            let path = PathBuf::from(path_str);
            if path.exists() {
                debug!("Using wallpaper: {:?}", path);
                return Some(path);
            }
        }

        None
    }

    fn create_wallpaper(path: &PathBuf) -> Picture {
        let picture = Picture::for_filename(path);
        // Picture will scale to fill available space
        picture.set_hexpand(true);
        picture.set_vexpand(true);
        picture.set_can_shrink(true);
        picture.add_css_class("desktop-wallpaper");
        picture
    }

    fn build_ui(&self, ctx: &ComponentContext) -> Overlay {
        let overlay = Overlay::new();
        overlay.add_css_class("desktop-container");

        let settings = ctx.settings();

        // Create wallpaper background
        if let Some(path) = Self::find_wallpaper(&settings) {
            let wallpaper = Self::create_wallpaper(&path);
            overlay.set_child(Some(&wallpaper));
        } else {
            // Fallback: dark background
            let bg = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
            bg.add_css_class("desktop-background-fallback");
            overlay.set_child(Some(&bg));
        }

        // Create icon grid if enabled
        if settings.show_desktop_icons() {
            if let Some(ref command_tx) = self.command_tx {
                let mut icon_grid = IconGrid::new(command_tx.clone());
                icon_grid.load_icons();

                let grid_widget = icon_grid.widget().clone();
                overlay.add_overlay(&grid_widget);
            }
        }

        overlay
    }

    fn setup_context_menu(&self, window: &Window) {
        let right_click = GestureClick::new();
        right_click.set_button(3);

        let tx = self.command_tx.clone();
        let context_menu = self.context_menu.clone();

        right_click.connect_pressed(move |_, _, x, y| {
            if let Some(ref command_tx) = tx {
                // Close existing menu
                if let Some(menu) = context_menu.borrow().as_ref() {
                    menu.close();
                }

                // Create new context menu at cursor position
                let menu = DesktopContextMenu::new(command_tx.clone(), x, y);
                menu.present();
                *context_menu.borrow_mut() = Some(menu);
            }
        });

        window.add_controller(right_click);
    }

    fn setup_drop_target(&self, window: &Window) {
        let drop_target = DropTarget::new(glib::Type::STRING, DragAction::COPY | DragAction::MOVE);

        let icon_grid = self.icon_grid.clone();

        drop_target.connect_drop(move |_target, value, _x, _y| {
            // Get the dropped file path
            let src_path_str: Option<String> = value
                .get::<glib::GString>()
                .map(|s| s.to_string())
                .ok()
                .or_else(|| value.get::<String>().ok());

            if let Some(src_str) = src_path_str {
                let src_path = PathBuf::from(&src_str);

                // Get desktop directory
                let desktop_dir = dirs::desktop_dir()
                    .or_else(|| {
                        dirs::home_dir().map(|h| h.join("Desktop"))
                    })
                    .unwrap_or_else(|| PathBuf::from("/tmp"));

                // Ensure desktop directory exists
                if !desktop_dir.exists() {
                    if std::fs::create_dir_all(&desktop_dir).is_err() {
                        return false;
                    }
                }

                // Get the file name
                if let Some(file_name) = src_path.file_name() {
                    let dest_path = desktop_dir.join(file_name);

                    // Don't copy to self
                    if src_path == dest_path {
                        return false;
                    }

                    // Copy or move the file
                    let result = if src_path.is_dir() {
                        // For directories, use a recursive copy
                        copy_dir_recursive(&src_path, &dest_path)
                    } else {
                        std::fs::copy(&src_path, &dest_path).map(|_| ())
                    };

                    if result.is_ok() {
                        debug!("Copied to desktop: {}", dest_path.display());

                        // Refresh icon grid if available
                        if let Some(ref grid) = icon_grid {
                            grid.borrow_mut().load_icons();
                        }
                        return true;
                    } else {
                        debug!("Failed to copy to desktop: {:?}", result.err());
                    }
                }
            }
            false
        });

        window.add_controller(drop_target);
    }

    fn update_wallpaper(&self, path: &PathBuf) {
        if let Some(window) = &self.window {
            if path.exists() {
                debug!("Updating wallpaper to: {:?}", path);

                // Get the overlay
                if let Some(overlay) = window.child().and_then(|c| c.downcast::<Overlay>().ok()) {
                    let wallpaper = Self::create_wallpaper(path);
                    overlay.set_child(Some(&wallpaper));
                }

                // Save to settings
                let mut settings = RavenSettings::load(&self.paths.raven_settings);
                settings.wallpaper_path = Some(path.to_string_lossy().to_string());
                let _ = settings.save();
            }
        }
    }

    fn refresh_icons(&self) {
        if let Some(icon_grid) = &self.icon_grid {
            icon_grid.borrow_mut().load_icons();
        }
    }
}

impl Default for DesktopComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for DesktopComponent {
    fn id(&self) -> ComponentId {
        ComponentId::Desktop
    }

    fn init(&mut self, ctx: ComponentContext) {
        if self.initialized {
            return;
        }

        // Store command sender
        self.command_tx = Some(ctx.command_tx.clone());

        // Create window
        let window = Self::create_window(&ctx.app);

        // Build UI
        let content = self.build_ui(&ctx);
        window.set_child(Some(&content));

        // Setup context menu
        self.setup_context_menu(&window);

        // Setup drop target for file drops
        self.setup_drop_target(&window);

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
        // Desktop is always visible, do nothing
    }

    fn is_visible(&self) -> bool {
        self.window.as_ref().map(|w| w.is_visible()).unwrap_or(false)
    }

    fn handle_event(&self, event: &ShellEvent) {
        match event {
            ShellEvent::WallpaperChanged(path) => {
                self.update_wallpaper(path);
            }
            ShellEvent::DesktopIconsChanged => {
                self.refresh_icons();
            }
            ShellEvent::SettingsReloaded(settings) => {
                // Update wallpaper if changed
                if let Some(ref path) = settings.wallpaper_path {
                    let path = PathBuf::from(path);
                    self.update_wallpaper(&path);
                }
            }
            _ => {}
        }
    }

    fn is_always_visible(&self) -> bool {
        true
    }

    fn window(&self) -> Option<&Window> {
        self.window.as_ref()
    }
}

// Helper function to copy a directory recursively
fn copy_dir_recursive(src: &PathBuf, dest: &PathBuf) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let entry_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if entry_path.is_dir() {
            copy_dir_recursive(&entry_path, &dest_path)?;
        } else {
            std::fs::copy(&entry_path, &dest_path)?;
        }
    }

    Ok(())
}
