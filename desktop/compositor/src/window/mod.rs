//! Window management

use crate::workspace::WindowId;

#[derive(Debug, Clone)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub struct Window {
    pub id: WindowId,
    pub app_id: Option<String>,
    pub title: Option<String>,
    pub geometry: WindowGeometry,
    pub floating: bool,
    pub fullscreen: bool,
    pub maximized: bool,
    pub minimized: bool,
}

impl Window {
    pub fn new(id: WindowId) -> Self {
        Self {
            id,
            app_id: None,
            title: None,
            geometry: WindowGeometry {
                x: 0,
                y: 0,
                width: 800,
                height: 600,
            },
            floating: false,
            fullscreen: false,
            maximized: false,
            minimized: false,
        }
    }

    pub fn set_title(&mut self, title: String) {
        self.title = Some(title);
    }

    pub fn set_app_id(&mut self, app_id: String) {
        self.app_id = Some(app_id);
    }

    pub fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
    }

    pub fn toggle_floating(&mut self) {
        self.floating = !self.floating;
    }
}
