//! Software rendering implementation for RavenDE compositor
//!
//! This module implements a simple software renderer that composites
//! Wayland surfaces into a dumb framebuffer for display via DRM/KMS.

use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    wayland::{
        compositor::with_states,
        shell::wlr_layer::LayerSurface,
        shm::{with_buffer_contents, BufferData},
    },
};
use std::slice;
use tracing::warn;

/// Software renderer that composites surfaces into a framebuffer
pub struct SoftwareRenderer {
    width: u32,
    height: u32,
    /// Internal buffer for compositing (XRGB8888 format)
    buffer: Vec<u32>,
}

impl SoftwareRenderer {
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width * height) as usize;
        let mut buffer = Vec::with_capacity(size);
        buffer.resize(size, 0xFF0b0f14); // Dark background color
        
        Self {
            width,
            height,
            buffer,
        }
    }

    /// Clear the framebuffer with a solid color
    pub fn clear(&mut self, color: u32) {
        self.buffer.fill(color);
    }

    /// Render a layer surface at its configured position
    pub fn render_layer_surface(&mut self, surface: &LayerSurface) {
        let wl_surface = surface.wl_surface();
        
        // Layer surfaces are typically fullscreen or anchored to edges
        // For simplicity, we'll render them at (0, 0) and let them fill as needed
        self.render_surface(wl_surface, 0, 0);
    }

    /// Render a regular surface at a specific position
    pub fn render_surface(&mut self, surface: &WlSurface, x: i32, y: i32) {
        use smithay::wayland::compositor::BufferAssignment;
        
        // Get the surface's buffer
        with_states(surface, |states| {
            let mut attrs = states.cached_state.get::<smithay::wayland::compositor::SurfaceAttributes>();
            
            // Check if there's a buffer attached
            // attrs is a MutexGuard<CachedState>, call current() to get the state
            if let Some(buffer_assignment) = attrs.current().buffer.as_ref() {
                // Match on BufferAssignment enum
                if let BufferAssignment::NewBuffer(buffer) = buffer_assignment {
                    // Try to access as SHM buffer
                    let _ = with_buffer_contents(
                        buffer,
                        |ptr, len, data| {
                            self.blit_shm_buffer(ptr, len, data, x, y);
                        },
                    );
                }
            }
        });
    }

    /// Blit a SHM buffer to the framebuffer
    fn blit_shm_buffer(
        &mut self,
        ptr: *const u8,
        len: usize,
        buffer_data: BufferData,
        dst_x: i32,
        dst_y: i32,
    ) {
        let buf_width = buffer_data.width;
        let buf_height = buffer_data.height;
        let stride = buffer_data.stride as usize;

        // Ensure we don't read out of bounds
        if len < (buf_height as usize * stride) {
            warn!("Buffer too small: expected {} bytes, got {}", buf_height as usize * stride, len);
            return;
        }

        // Convert buffer data to u32 pixels
        // SHM buffers are typically ARGB8888 or XRGB8888
        let src_pixels = unsafe {
            slice::from_raw_parts(ptr as *const u32, len / 4)
        };

        // Blit each row
        for src_y in 0..buf_height {
            let dst_y_pos = dst_y + src_y as i32;
            
            // Skip if outside framebuffer
            if dst_y_pos < 0 || dst_y_pos >= self.height as i32 {
                continue;
            }

            for src_x in 0..buf_width {
                let dst_x_pos = dst_x + src_x as i32;
                
                // Skip if outside framebuffer
                if dst_x_pos < 0 || dst_x_pos >= self.width as i32 {
                    continue;
                }

                // Calculate source and destination indices
                let src_idx = (src_y as usize * stride / 4) + src_x as usize;
                let dst_idx = (dst_y_pos as usize * self.width as usize) + dst_x_pos as usize;

                if src_idx < src_pixels.len() && dst_idx < self.buffer.len() {
                    let pixel = src_pixels[src_idx];
                    
                    // Simple alpha blending (if alpha channel exists)
                    let alpha = (pixel >> 24) & 0xFF;
                    if alpha == 0xFF || alpha == 0 {
                        // Fully opaque or format doesn't have alpha - direct copy
                        self.buffer[dst_idx] = pixel;
                    } else {
                        // Alpha blending
                        let src_r = ((pixel >> 16) & 0xFF) as u32;
                        let src_g = ((pixel >> 8) & 0xFF) as u32;
                        let src_b = (pixel & 0xFF) as u32;
                        
                        let dst_pixel = self.buffer[dst_idx];
                        let dst_r = ((dst_pixel >> 16) & 0xFF) as u32;
                        let dst_g = ((dst_pixel >> 8) & 0xFF) as u32;
                        let dst_b = (dst_pixel & 0xFF) as u32;
                        
                        let inv_alpha = 255 - alpha;
                        let r = (src_r * alpha + dst_r * inv_alpha) / 255;
                        let g = (src_g * alpha + dst_g * inv_alpha) / 255;
                        let b = (src_b * alpha + dst_b * inv_alpha) / 255;
                        
                        self.buffer[dst_idx] = 0xFF000000 | (r << 16) | (g << 8) | b;
                    }
                }
            }
        }
    }

    /// Copy the composited buffer to a destination buffer (e.g., dumb buffer)
    pub fn copy_to_buffer(&self, dst: &mut [u8]) {
        let required_size = (self.width * self.height * 4) as usize;
        
        if dst.len() < required_size {
            warn!("Destination buffer too small: {} < {}", dst.len(), required_size);
            return;
        }

        // Copy our u32 buffer to u8 destination
        unsafe {
            let src_ptr = self.buffer.as_ptr() as *const u8;
            let dst_ptr = dst.as_mut_ptr();
            std::ptr::copy_nonoverlapping(src_ptr, dst_ptr, required_size);
        }
    }

    /// Get the internal buffer as a byte slice
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(
                self.buffer.as_ptr() as *const u8,
                self.buffer.len() * 4,
            )
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

/// Output information
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
