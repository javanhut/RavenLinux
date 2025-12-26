use compact_str::CompactString;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, FlowBox, GestureClick, Image, Label, Orientation, Popover, Button};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tracing::debug;

use raven_core::ShellCommand;

/// Desktop icon representation
#[derive(Debug, Clone)]
pub struct DesktopIcon {
    pub name: CompactString,
    pub exec: CompactString,
    pub icon: CompactString,
    pub x: i32,
    pub y: i32,
}

impl DesktopIcon {
    pub fn new(name: &str, exec: &str, icon: &str) -> Self {
        Self {
            name: name.into(),
            exec: exec.into(),
            icon: icon.into(),
            x: 0,
            y: 0,
        }
    }
}

/// Desktop icon grid manager
pub struct IconGrid {
    container: FlowBox,
    icons: Vec<DesktopIcon>,
    command_tx: mpsc::Sender<ShellCommand>,
}

impl IconGrid {
    pub fn new(command_tx: mpsc::Sender<ShellCommand>) -> Self {
        let container = FlowBox::new();
        container.set_valign(gtk4::Align::Start);
        container.set_halign(gtk4::Align::Start);
        container.set_selection_mode(gtk4::SelectionMode::Single);
        container.set_activate_on_single_click(false);
        container.set_row_spacing(16);
        container.set_column_spacing(16);
        container.set_max_children_per_line(20);
        container.set_min_children_per_line(1);
        container.set_margin_top(50); // Space for panel
        container.set_margin_start(20);
        container.set_margin_end(20);
        container.add_css_class("desktop-icon-grid");

        Self {
            container,
            icons: Vec::new(),
            command_tx,
        }
    }

    /// Load icons from pinned apps config and Desktop folder
    pub fn load_icons(&mut self) {
        self.icons.clear();

        // Load from pinned apps config
        self.load_pinned_apps();

        // Load from ~/Desktop
        self.load_desktop_folder();

        // Refresh the visual grid
        self.refresh_grid();
    }

    fn load_pinned_apps(&mut self) {
        let config_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("raven/pinned-apps.json");

        if let Ok(data) = std::fs::read_to_string(&config_path) {
            if let Ok(config) = serde_json::from_str::<PinnedAppsConfig>(&data) {
                for app in config.pinned_apps {
                    self.icons.push(DesktopIcon {
                        name: app.name,
                        exec: app.exec,
                        icon: app.icon,
                        x: app.x,
                        y: app.y,
                    });
                }
            }
        }
    }

    fn load_desktop_folder(&mut self) {
        let desktop_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Desktop");

        if let Ok(entries) = std::fs::read_dir(&desktop_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "desktop").unwrap_or(false) {
                    if let Some(icon) = self.parse_desktop_file(&path) {
                        // Check if not already in list
                        if !self.icons.iter().any(|i| i.exec == icon.exec) {
                            self.icons.push(icon);
                        }
                    }
                }
            }
        }
    }

    fn parse_desktop_file(&self, path: &PathBuf) -> Option<DesktopIcon> {
        let content = std::fs::read_to_string(path).ok()?;

        let mut name = None;
        let mut exec = None;
        let mut icon = None;
        let mut hidden = false;
        let mut no_display = false;

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("Name=") && name.is_none() {
                name = Some(line.trim_start_matches("Name=").to_string());
            } else if line.starts_with("Exec=") && exec.is_none() {
                // Remove field codes like %f, %u, %F, %U
                let cmd = line.trim_start_matches("Exec=")
                    .replace("%f", "")
                    .replace("%F", "")
                    .replace("%u", "")
                    .replace("%U", "")
                    .replace("%c", "")
                    .replace("%k", "")
                    .trim()
                    .to_string();
                exec = Some(cmd);
            } else if line.starts_with("Icon=") && icon.is_none() {
                icon = Some(line.trim_start_matches("Icon=").to_string());
            } else if line == "Hidden=true" {
                hidden = true;
            } else if line == "NoDisplay=true" {
                no_display = true;
            }
        }

        if hidden || no_display {
            return None;
        }

        Some(DesktopIcon {
            name: name.unwrap_or_else(|| "Unknown".to_string()).into(),
            exec: exec?.into(),
            icon: icon.unwrap_or_else(|| "application-x-executable".to_string()).into(),
            x: 0,
            y: 0,
        })
    }

    /// Refresh the visual grid
    pub fn refresh_grid(&self) {
        // Remove all children
        while let Some(child) = self.container.first_child() {
            self.container.remove(&child);
        }

        // Add icon widgets
        for icon in &self.icons {
            let widget = self.create_icon_widget(icon);
            self.container.insert(&widget, -1);
        }
    }

    fn create_icon_widget(&self, icon: &DesktopIcon) -> GtkBox {
        let container = GtkBox::new(Orientation::Vertical, 4);
        container.set_halign(gtk4::Align::Center);
        container.add_css_class("desktop-icon");

        // Icon image (48x48)
        let image = self.load_icon_image(&icon.icon);
        image.set_pixel_size(48);
        image.add_css_class("desktop-icon-image");
        container.append(&image);

        // Label (max 12 chars, 2 lines)
        let name = if icon.name.len() > 12 {
            format!("{}...", &icon.name[..9])
        } else {
            icon.name.to_string()
        };
        let label = Label::new(Some(&name));
        label.set_max_width_chars(12);
        label.set_wrap(true);
        label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        label.set_lines(2);
        label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        label.add_css_class("desktop-icon-label");
        container.append(&label);

        // Double-click to launch
        let click_gesture = GestureClick::new();
        click_gesture.set_button(1);
        let exec = icon.exec.clone();
        let tx = self.command_tx.clone();
        click_gesture.connect_released(move |_gesture, n_press, _, _| {
            if n_press == 2 {
                debug!("Launching: {}", exec);
                let _ = tx.blocking_send(ShellCommand::LaunchApp(exec.clone()));
            }
        });
        container.add_controller(click_gesture);

        // Right-click context menu
        let right_click = GestureClick::new();
        right_click.set_button(3);
        let icon_name = icon.name.clone();
        let icon_exec = icon.exec.clone();
        let tx = self.command_tx.clone();
        let container_clone = container.clone();
        right_click.connect_pressed(move |_, _, _, _| {
            Self::show_icon_context_menu(&container_clone, &icon_name, &icon_exec, tx.clone());
        });
        container.add_controller(right_click);

        container
    }

    fn load_icon_image(&self, icon_name: &str) -> Image {
        // Try loading from common icon paths
        let icon_paths = [
            format!("/usr/share/icons/hicolor/48x48/apps/{}.png", icon_name),
            format!("/usr/share/icons/hicolor/48x48/apps/{}.svg", icon_name),
            format!("/usr/share/icons/hicolor/scalable/apps/{}.svg", icon_name),
            format!("/usr/share/pixmaps/{}.png", icon_name),
            format!("/usr/share/pixmaps/{}.svg", icon_name),
            format!("/usr/share/icons/Adwaita/48x48/apps/{}.png", icon_name),
        ];

        for path in &icon_paths {
            if std::path::Path::new(path).exists() {
                return Image::from_file(path);
            }
        }

        // Fall back to themed icon
        Image::from_icon_name(icon_name)
    }

    fn show_icon_context_menu(
        parent: &GtkBox,
        _name: &CompactString,
        exec: &CompactString,
        _tx: mpsc::Sender<ShellCommand>,
    ) {
        let popover = Popover::new();
        popover.set_parent(parent);
        popover.add_css_class("context-menu");

        let menu_box = GtkBox::new(Orientation::Vertical, 2);

        // Unpin button
        let unpin_btn = Button::with_label("Unpin from Desktop");
        let exec_clone = exec.clone();
        let popover_clone = popover.clone();
        unpin_btn.connect_clicked(move |_| {
            // Remove from pinned apps config
            if let Err(e) = remove_pinned_app(&exec_clone) {
                tracing::error!("Failed to unpin app: {}", e);
            }
            popover_clone.popdown();
        });
        menu_box.append(&unpin_btn);

        popover.set_child(Some(&menu_box));
        popover.popup();
    }

    pub fn widget(&self) -> &FlowBox {
        &self.container
    }

    /// Pin an app to desktop
    pub fn pin_app(&mut self, icon: DesktopIcon) {
        self.icons.push(icon.clone());
        save_pinned_apps(&self.icons);
        self.refresh_grid();
    }

    /// Unpin an app from desktop
    pub fn unpin_app(&mut self, exec: &str) {
        self.icons.retain(|i| i.exec.as_str() != exec);
        save_pinned_apps(&self.icons);
        self.refresh_grid();
    }
}

/// Pinned apps configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PinnedAppsConfig {
    #[serde(default)]
    pinned_apps: Vec<PinnedApp>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PinnedApp {
    name: CompactString,
    exec: CompactString,
    icon: CompactString,
    #[serde(default)]
    x: i32,
    #[serde(default)]
    y: i32,
}

fn save_pinned_apps(icons: &[DesktopIcon]) {
    let config = PinnedAppsConfig {
        pinned_apps: icons.iter().map(|i| PinnedApp {
            name: i.name.clone(),
            exec: i.exec.clone(),
            icon: i.icon.clone(),
            x: i.x,
            y: i.y,
        }).collect(),
    };

    let config_path = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("raven/pinned-apps.json");

    if let Some(parent) = config_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if let Ok(data) = serde_json::to_string_pretty(&config) {
        let _ = std::fs::write(config_path, data);
    }
}

fn remove_pinned_app(exec: &str) -> anyhow::Result<()> {
    let config_path = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("raven/pinned-apps.json");

    let mut config: PinnedAppsConfig = if config_path.exists() {
        let data = std::fs::read_to_string(&config_path)?;
        serde_json::from_str(&data)?
    } else {
        PinnedAppsConfig { pinned_apps: Vec::new() }
    };

    config.pinned_apps.retain(|app| app.exec.as_str() != exec);

    let data = serde_json::to_string_pretty(&config)?;
    std::fs::write(config_path, data)?;

    Ok(())
}
