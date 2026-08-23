//! Boot menu UI

// Menu rendering and interaction code
// This module handles the visual boot menu

pub struct MenuState {
    pub selected: usize,
    pub scroll_offset: usize,
    pub editing: bool,
}

impl Default for MenuState {
    fn default() -> Self {
        Self {
            selected: 0,
            scroll_offset: 0,
            editing: false,
        }
    }
}
