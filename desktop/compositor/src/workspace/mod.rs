//! Workspace management

use std::collections::HashMap;

/// Layout mode for a workspace
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// Traditional floating windows
    Floating,
    /// Binary space partitioning tiling
    TilingBsp,
    /// Column-based tiling
    TilingColumns,
    /// Single maximized window
    Monocle,
}

impl Default for LayoutMode {
    fn default() -> Self {
        Self::TilingBsp
    }
}

/// A single workspace containing windows
#[derive(Debug)]
pub struct Workspace {
    pub index: usize,
    pub name: String,
    pub layout: LayoutMode,
    pub windows: Vec<WindowId>,
    pub focused: Option<usize>,
}

/// Unique window identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub u64);

impl Workspace {
    pub fn new(index: usize, name: String) -> Self {
        Self {
            index,
            name,
            layout: LayoutMode::default(),
            windows: Vec::new(),
            focused: None,
        }
    }

    pub fn add_window(&mut self, id: WindowId) {
        self.windows.push(id);
        self.focused = Some(self.windows.len() - 1);
    }

    pub fn remove_window(&mut self, id: WindowId) -> bool {
        if let Some(pos) = self.windows.iter().position(|w| *w == id) {
            self.windows.remove(pos);

            // Update focus
            if self.windows.is_empty() {
                self.focused = None;
            } else if let Some(focused) = self.focused {
                if focused >= self.windows.len() {
                    self.focused = Some(self.windows.len() - 1);
                }
            }
            true
        } else {
            false
        }
    }

    pub fn focused_window(&self) -> Option<WindowId> {
        self.focused.map(|i| self.windows[i])
    }

    pub fn focus_next(&mut self) {
        if self.windows.is_empty() {
            return;
        }

        self.focused = Some(match self.focused {
            Some(i) => (i + 1) % self.windows.len(),
            None => 0,
        });
    }

    pub fn focus_prev(&mut self) {
        if self.windows.is_empty() {
            return;
        }

        self.focused = Some(match self.focused {
            Some(0) => self.windows.len() - 1,
            Some(i) => i - 1,
            None => 0,
        });
    }

    pub fn set_layout(&mut self, layout: LayoutMode) {
        self.layout = layout;
    }

    pub fn toggle_layout(&mut self) {
        self.layout = match self.layout {
            LayoutMode::Floating => LayoutMode::TilingBsp,
            LayoutMode::TilingBsp => LayoutMode::TilingColumns,
            LayoutMode::TilingColumns => LayoutMode::Monocle,
            LayoutMode::Monocle => LayoutMode::Floating,
        };
    }
}

/// Manages all workspaces
pub struct WorkspaceManager {
    workspaces: Vec<Workspace>,
    active: usize,
    window_workspace: HashMap<WindowId, usize>,
}

impl WorkspaceManager {
    pub fn new(count: usize) -> Self {
        let workspaces = (0..count)
            .map(|i| Workspace::new(i, (i + 1).to_string()))
            .collect();

        Self {
            workspaces,
            active: 0,
            window_workspace: HashMap::new(),
        }
    }

    pub fn active(&self) -> &Workspace {
        &self.workspaces[self.active]
    }

    pub fn active_mut(&mut self) -> &mut Workspace {
        &mut self.workspaces[self.active]
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn switch_to(&mut self, index: usize) -> bool {
        if index < self.workspaces.len() && index != self.active {
            self.active = index;
            true
        } else {
            false
        }
    }

    pub fn get(&self, index: usize) -> Option<&Workspace> {
        self.workspaces.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Workspace> {
        self.workspaces.get_mut(index)
    }

    pub fn add_window(&mut self, id: WindowId) {
        self.workspaces[self.active].add_window(id);
        self.window_workspace.insert(id, self.active);
    }

    pub fn remove_window(&mut self, id: WindowId) {
        if let Some(workspace_idx) = self.window_workspace.remove(&id) {
            if let Some(workspace) = self.workspaces.get_mut(workspace_idx) {
                workspace.remove_window(id);
            }
        }
    }

    pub fn move_window_to(&mut self, id: WindowId, target: usize) -> bool {
        if target >= self.workspaces.len() {
            return false;
        }

        if let Some(current) = self.window_workspace.get(&id).copied() {
            if current == target {
                return false;
            }

            self.workspaces[current].remove_window(id);
            self.workspaces[target].add_window(id);
            self.window_workspace.insert(id, target);
            true
        } else {
            false
        }
    }

    pub fn window_workspace(&self, id: WindowId) -> Option<usize> {
        self.window_workspace.get(&id).copied()
    }

    pub fn workspace_count(&self) -> usize {
        self.workspaces.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Workspace> {
        self.workspaces.iter()
    }
}
