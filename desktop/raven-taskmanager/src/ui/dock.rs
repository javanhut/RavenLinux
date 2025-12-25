use ahash::AHashMap;
use compact_str::CompactString;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, GestureClick, Orientation, Popover};
use tokio::sync::mpsc;

use crate::config::DockItem;
use crate::messages::PanelCommand;
use crate::state::DockDiff;

/// Dock widget with efficient differential updates
pub struct Dock {
    container: GtkBox,
    buttons: AHashMap<CompactString, Button>,
    command_tx: mpsc::Sender<PanelCommand>,
    orientation: Orientation,
}

impl Dock {
    pub fn new(command_tx: mpsc::Sender<PanelCommand>, vertical: bool) -> Self {
        let orientation = if vertical {
            Orientation::Vertical
        } else {
            Orientation::Horizontal
        };

        let container = GtkBox::new(orientation, 4);
        container.add_css_class("dock-container");

        Self {
            container,
            buttons: AHashMap::new(),
            command_tx,
            orientation,
        }
    }

    /// Apply a diff to update the dock efficiently
    pub fn apply_diff(&mut self, diff: DockDiff) {
        match diff {
            DockDiff::Add { index, item } => {
                self.add_item(index, &item);
            }
            DockDiff::Remove { id } => {
                self.remove_item(&id);
            }
            DockDiff::UpdateState {
                id,
                running,
                minimized,
                focused,
            } => {
                self.update_item_state(&id, running, minimized, focused);
            }
            DockDiff::Reorder { id, new_index } => {
                self.reorder_item(&id, new_index);
            }
        }
    }

    /// Add a new dock item
    fn add_item(&mut self, index: usize, item: &DockItem) {
        let btn = self.create_button(item);
        self.buttons.insert(item.id.clone(), btn.clone());

        // Insert at correct position
        if index == 0 {
            self.container.prepend(&btn);
        } else {
            // Find the widget at index-1 and insert after it
            let mut child = self.container.first_child();
            for _ in 0..(index.saturating_sub(1)) {
                if let Some(c) = child {
                    child = c.next_sibling();
                } else {
                    break;
                }
            }

            if let Some(sibling) = child {
                self.container.insert_child_after(&btn, Some(&sibling));
            } else {
                self.container.append(&btn);
            }
        }
    }

    /// Remove a dock item
    fn remove_item(&mut self, id: &CompactString) {
        if let Some(btn) = self.buttons.remove(id) {
            self.container.remove(&btn);
        }
    }

    /// Update CSS classes for an item without recreating it
    fn update_item_state(&self, id: &CompactString, running: bool, minimized: bool, focused: bool) {
        if let Some(btn) = self.buttons.get(id) {
            // Running state
            if running {
                btn.add_css_class("dock-item-running");
            } else {
                btn.remove_css_class("dock-item-running");
            }

            // Minimized state
            if minimized {
                btn.add_css_class("dock-item-minimized");
            } else {
                btn.remove_css_class("dock-item-minimized");
            }

            // Focused state
            if focused {
                btn.add_css_class("dock-item-focused");
            } else {
                btn.remove_css_class("dock-item-focused");
            }
        }
    }

    /// Reorder an item to a new position
    fn reorder_item(&mut self, id: &CompactString, new_index: usize) {
        if let Some(btn) = self.buttons.get(id) {
            // Remove and re-add at new position
            self.container.remove(btn);

            if new_index == 0 {
                self.container.prepend(btn);
            } else {
                let mut child = self.container.first_child();
                for _ in 0..(new_index.saturating_sub(1)) {
                    if let Some(c) = child {
                        child = c.next_sibling();
                    } else {
                        break;
                    }
                }

                if let Some(sibling) = child {
                    self.container.insert_child_after(btn, Some(&sibling));
                } else {
                    self.container.append(btn);
                }
            }
        }
    }

    /// Create a button for a dock item
    fn create_button(&self, item: &DockItem) -> Button {
        let btn = Button::with_label(&item.name);
        btn.add_css_class("dock-item");

        if item.pinned {
            btn.add_css_class("dock-item-pinned");
        }
        if item.running {
            btn.add_css_class("dock-item-running");
        }
        if item.minimized {
            btn.add_css_class("dock-item-minimized");
        }
        if item.focused {
            btn.add_css_class("dock-item-focused");
        }

        // Left click: focus or launch
        let tx = self.command_tx.clone();
        let address = item.address.clone();
        let command = item.command.clone();
        let running = item.running;

        btn.connect_clicked(move |_| {
            if running {
                if let Some(ref addr) = address {
                    let _ = tx.blocking_send(PanelCommand::FocusWindow(addr.clone()));
                }
            } else {
                let _ = tx.blocking_send(PanelCommand::LaunchApp(command.clone()));
            }
        });

        // Right click: context menu
        let gesture = GestureClick::new();
        gesture.set_button(3); // Right button

        let tx = self.command_tx.clone();
        let item_id = item.id.clone();
        let item_address = item.address.clone();
        let item_pinned = item.pinned;
        let item_running = item.running;
        let item_minimized = item.minimized;

        let btn_clone = btn.clone();
        gesture.connect_pressed(move |_, _, _, _| {
            Self::show_context_menu(
                &btn_clone,
                tx.clone(),
                item_id.clone(),
                item_address.clone(),
                item_pinned,
                item_running,
                item_minimized,
            );
        });

        btn.add_controller(gesture);

        btn
    }

    /// Show context menu for a dock item
    fn show_context_menu(
        btn: &Button,
        tx: mpsc::Sender<PanelCommand>,
        id: CompactString,
        address: Option<CompactString>,
        pinned: bool,
        running: bool,
        minimized: bool,
    ) {
        let popover = Popover::new();
        popover.set_parent(btn);
        popover.add_css_class("context-menu");

        let menu_box = GtkBox::new(Orientation::Vertical, 2);

        // Pin/Unpin button
        let pin_btn = Button::with_label(if pinned {
            "Unpin from Dock"
        } else {
            "Pin to Dock"
        });

        let tx_pin = tx.clone();
        let id_pin = id.clone();
        let popover_clone = popover.clone();
        pin_btn.connect_clicked(move |_| {
            let _ = tx_pin.blocking_send(PanelCommand::PinApp {
                id: id_pin.clone(),
                pinned: !pinned,
            });
            let _ = tx_pin.blocking_send(PanelCommand::SaveDockConfig);
            popover_clone.popdown();
        });
        menu_box.append(&pin_btn);

        // Running app options
        if running {
            if let Some(addr) = address {
                // Minimize/Restore
                let min_btn = Button::with_label(if minimized { "Restore" } else { "Minimize" });

                let tx_min = tx.clone();
                let addr_min = addr.clone();
                let popover_clone = popover.clone();
                min_btn.connect_clicked(move |_| {
                    let cmd = if minimized {
                        PanelCommand::RestoreWindow(addr_min.clone())
                    } else {
                        PanelCommand::MinimizeWindow(addr_min.clone())
                    };
                    let _ = tx_min.blocking_send(cmd);
                    popover_clone.popdown();
                });
                menu_box.append(&min_btn);

                // Close
                let close_btn = Button::with_label("Close");
                close_btn.add_css_class("context-menu-close");

                let tx_close = tx.clone();
                let addr_close = addr.clone();
                let popover_clone = popover.clone();
                close_btn.connect_clicked(move |_| {
                    let _ = tx_close.blocking_send(PanelCommand::CloseWindow(addr_close.clone()));
                    popover_clone.popdown();
                });
                menu_box.append(&close_btn);
            }
        }

        popover.set_child(Some(&menu_box));
        popover.popup();
    }

    /// Get the container widget
    pub fn widget(&self) -> &GtkBox {
        &self.container
    }

    /// Update button label (for title changes)
    pub fn update_label(&self, id: &CompactString, label: &str) {
        if let Some(btn) = self.buttons.get(id) {
            btn.set_label(label);
        }
    }
}
