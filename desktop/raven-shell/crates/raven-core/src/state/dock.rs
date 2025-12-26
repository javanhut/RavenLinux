use ahash::AHashMap;
use compact_str::CompactString;
use smallvec::SmallVec;

use crate::config::{DockItem, PanelConfig};
use crate::messages::ShellEvent;

/// Computed diff for efficient UI updates - only what changed
#[derive(Debug, Clone)]
pub enum DockDiff {
    /// Add a new item at index
    Add {
        index: usize,
        item: DockItem,
    },
    /// Remove an item by ID
    Remove {
        id: CompactString,
    },
    /// Update visual state (CSS classes) without recreating widget
    UpdateState {
        id: CompactString,
        running: bool,
        minimized: bool,
        focused: bool,
    },
    /// Reorder items (for future drag-drop support)
    Reorder {
        id: CompactString,
        new_index: usize,
    },
}

/// Dock state with efficient differential update calculation
#[derive(Default)]
pub struct DockState {
    /// All dock items indexed by ID
    items: AHashMap<CompactString, DockItem>,
    /// Display order (pinned first, then running)
    order: Vec<CompactString>,
    /// Currently focused window address
    focused: Option<CompactString>,
}

impl DockState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load initial pinned apps from config
    pub fn load_pinned(&mut self, config: &PanelConfig) -> SmallVec<[DockDiff; 8]> {
        let mut diffs = SmallVec::new();

        for pinned in &config.pinned_apps {
            let mut item = pinned.clone();
            item.pinned = true;
            item.running = false;

            let id = item.id.clone();
            let index = self.order.len();

            self.order.push(id.clone());
            self.items.insert(id, item.clone());

            diffs.push(DockDiff::Add { index, item });
        }

        diffs
    }

    /// Apply an event and compute minimal diff for UI update
    pub fn apply_event(&mut self, event: &ShellEvent) -> SmallVec<[DockDiff; 4]> {
        let mut diffs = SmallVec::new();

        match event {
            ShellEvent::WindowOpened {
                address,
                class,
                title,
                pid,
            } => {
                // Skip excluded classes
                if !DockItem::should_track(class) {
                    return diffs;
                }

                let id: CompactString = format!("hypr-{}", address).into();

                if let Some(existing) = self.items.get_mut(&id) {
                    // Update existing item (was pinned, now running)
                    let was_running = existing.running;
                    existing.running = true;
                    existing.pid = Some(*pid);
                    existing.address = Some(address.clone());

                    if !was_running {
                        diffs.push(DockDiff::UpdateState {
                            id,
                            running: true,
                            minimized: existing.minimized,
                            focused: self.focused.as_ref() == Some(address),
                        });
                    }
                } else {
                    // Create new item
                    let item = DockItem::new_running(
                        address.clone(),
                        class.clone(),
                        title.clone(),
                        *pid,
                    );

                    let index = self.order.len();
                    self.order.push(id.clone());
                    self.items.insert(id, item.clone());

                    diffs.push(DockDiff::Add { index, item });
                }
            }

            ShellEvent::WindowClosed { address } => {
                let id: CompactString = format!("hypr-{}", address).into();

                if let Some(item) = self.items.get_mut(&id) {
                    if item.pinned {
                        // Keep pinned item, just mark not running
                        item.running = false;
                        item.pid = None;
                        item.address = None;
                        item.minimized = false;
                        item.focused = false;

                        diffs.push(DockDiff::UpdateState {
                            id,
                            running: false,
                            minimized: false,
                            focused: false,
                        });
                    } else {
                        // Remove non-pinned item entirely
                        self.items.remove(&id);
                        self.order.retain(|i| i != &id);

                        diffs.push(DockDiff::Remove { id });
                    }
                }

                // Clear focus if this was the focused window
                if self.focused.as_ref() == Some(address) {
                    self.focused = None;
                }
            }

            ShellEvent::WindowFocused { address } => {
                let new_id: CompactString = format!("hypr-{}", address).into();

                // Unfocus previous window
                if let Some(old_addr) = self.focused.take() {
                    let old_id: CompactString = format!("hypr-{}", old_addr).into();
                    if let Some(old_item) = self.items.get_mut(&old_id) {
                        old_item.focused = false;
                        diffs.push(DockDiff::UpdateState {
                            id: old_id,
                            running: old_item.running,
                            minimized: old_item.minimized,
                            focused: false,
                        });
                    }
                }

                // Focus new window
                if let Some(new_item) = self.items.get_mut(&new_id) {
                    new_item.focused = true;
                    self.focused = Some(address.clone());

                    diffs.push(DockDiff::UpdateState {
                        id: new_id,
                        running: new_item.running,
                        minimized: new_item.minimized,
                        focused: true,
                    });
                }
            }

            ShellEvent::WindowMoved {
                address,
                workspace,
                is_special,
            } => {
                let id: CompactString = format!("hypr-{}", address).into();

                if let Some(item) = self.items.get_mut(&id) {
                    item.workspace_id = Some(*workspace);

                    // is_special indicates minimized (special workspace)
                    if item.minimized != *is_special {
                        item.minimized = *is_special;

                        diffs.push(DockDiff::UpdateState {
                            id,
                            running: item.running,
                            minimized: *is_special,
                            focused: item.focused,
                        });
                    }
                }
            }

            ShellEvent::WindowTitleChanged { address, title } => {
                let id: CompactString = format!("hypr-{}", address).into();

                if let Some(item) = self.items.get_mut(&id) {
                    // Update title (might want to update button label)
                    let new_name = if title.len() > 20 {
                        format!("{}...", &title[..17]).into()
                    } else {
                        title.clone()
                    };
                    item.name = new_name;
                    // Note: Title changes don't affect CSS classes, so no DockDiff needed
                    // UI can poll item.name if needed, or we add a TitleChanged diff
                }
            }

            ShellEvent::ConfigReloaded(config) => {
                // Add/update from new config
                for pinned in &config.pinned_apps {
                    if let Some(existing) = self.items.get_mut(&pinned.id) {
                        existing.pinned = true;
                    } else {
                        let mut item = pinned.clone();
                        item.pinned = true;

                        let index = self.order.len();
                        self.order.push(item.id.clone());
                        self.items.insert(item.id.clone(), item.clone());

                        diffs.push(DockDiff::Add { index, item });
                    }
                }
            }

            _ => {}
        }

        diffs
    }

    /// Pin or unpin an item
    pub fn set_pinned(&mut self, id: &CompactString, pinned: bool) -> Option<DockDiff> {
        if let Some(item) = self.items.get_mut(id) {
            item.pinned = pinned;

            if !pinned && !item.running {
                // Remove unpinned non-running item
                self.items.remove(id);
                self.order.retain(|i| i != id);
                return Some(DockDiff::Remove { id: id.clone() });
            }
        }
        None
    }

    /// Get item by ID
    pub fn get(&self, id: &CompactString) -> Option<&DockItem> {
        self.items.get(id)
    }

    /// Get item by window address
    pub fn get_by_address(&self, address: &CompactString) -> Option<&DockItem> {
        let id: CompactString = format!("hypr-{}", address).into();
        self.items.get(&id)
    }

    /// Get all items in display order
    pub fn items_ordered(&self) -> impl Iterator<Item = &DockItem> {
        self.order.iter().filter_map(|id| self.items.get(id))
    }

    /// Get pinned items for saving to config
    pub fn get_pinned_for_config(&self) -> PanelConfig {
        let pinned_apps = self
            .items
            .values()
            .filter(|item| item.pinned)
            .map(|item| DockItem {
                id: item.id.clone(),
                name: item.name.clone(),
                command: item.command.clone(),
                icon: item.icon.clone(),
                pinned: true,
                // Clear runtime state for serialization
                running: false,
                minimized: false,
                focused: false,
                pid: None,
                address: None,
                workspace_id: None,
            })
            .collect();

        PanelConfig { pinned_apps }
    }

    /// Get count of items
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
