// Navigation history for file manager

use std::path::PathBuf;

/// Navigation history manager
#[derive(Debug, Default)]
pub struct History {
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
    current: Option<PathBuf>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a new path, clearing forward history
    pub fn push(&mut self, path: PathBuf) {
        if let Some(current) = self.current.take() {
            self.back.push(current);
        }
        self.current = Some(path);
        self.forward.clear();
    }

    /// Go back to previous path
    pub fn back(&mut self) -> Option<PathBuf> {
        if let Some(prev) = self.back.pop() {
            if let Some(current) = self.current.take() {
                self.forward.push(current);
            }
            self.current = Some(prev.clone());
            Some(prev)
        } else {
            None
        }
    }

    /// Go forward to next path
    pub fn forward(&mut self) -> Option<PathBuf> {
        if let Some(next) = self.forward.pop() {
            if let Some(current) = self.current.take() {
                self.back.push(current);
            }
            self.current = Some(next.clone());
            Some(next)
        } else {
            None
        }
    }

    /// Check if can go back
    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    /// Check if can go forward
    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    /// Get current path
    pub fn current(&self) -> Option<&PathBuf> {
        self.current.as_ref()
    }
}
