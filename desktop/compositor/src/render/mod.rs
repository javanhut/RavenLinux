//! Rendering and output management

pub struct Renderer {
    // TODO: OpenGL/Vulkan renderer state
}

impl Renderer {
    pub fn new() -> Self {
        Self {}
    }

    pub fn render_frame(&mut self) {
        // TODO: Render all visible windows
    }
}

pub struct Output {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: u32,
    pub scale: f64,
}

impl Output {
    pub fn new(name: String, width: u32, height: u32, refresh_rate: u32) -> Self {
        Self {
            name,
            width,
            height,
            refresh_rate,
            scale: 1.0,
        }
    }
}
