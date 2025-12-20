//! Native (DRM/KMS) backend with libseat session management.
//!
//! Smithay 0.7 API with wlr-layer-shell support.

use crate::config::Config;
use crate::render::SoftwareRenderer;
use anyhow::{anyhow, Context, Result};
use smithay::{
    backend::{
        allocator::{
            Fourcc, Allocator, Buffer as AllocatorBuffer,
            dmabuf::AsDmabuf,
            gbm::{GbmAllocator, GbmBuffer, GbmDevice},
        },
        drm::{DrmDevice, DrmDeviceFd, DrmEvent, DrmSurface},
        input::{
            Axis, AxisRelativeDirection, ButtonState, Event as InputEvent, InputBackend,
            InputEvent as BackendInputEvent, KeyState, KeyboardKeyEvent, PointerAxisEvent,
            PointerButtonEvent, PointerMotionEvent,
        },
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        session::{libseat::LibSeatSession, Session},
    },
    delegate_compositor, delegate_layer_shell, delegate_output, delegate_seat, delegate_shm,
    delegate_xdg_shell,
    input::{
        keyboard::{keysyms as KeySyms, FilterResult, KeysymHandle, ModifiersState},
        pointer::{AxisFrame, ButtonEvent, MotionEvent},
        Seat, SeatHandler, SeatState,
    },
    output::{Mode as OutputMode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        calloop::EventLoop,
        drm::control::{connector::State as ConnectorState, Device as DrmControlDevice},
        input::Libinput,
        rustix::fs::OFlags,
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::{wl_buffer, wl_output, wl_seat, wl_surface::WlSurface},
            Client, Display, DisplayHandle, ListeningSocket,
        },
    },
    utils::{Logical, Point, Serial, Size, SERIAL_COUNTER},
    wayland::{
        buffer::BufferHandler,
        compositor::{CompositorClientState, CompositorHandler, CompositorState},
        shell::{
            wlr_layer::{
                Layer, LayerSurface, WlrLayerShellHandler, WlrLayerShellState,
            },
            xdg::{PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState},
        },
        shm::{ShmHandler, ShmState},
    },
};
use std::{
    env,
    fs,
    path::Path,
    process::{Command, Stdio},
    sync::{atomic::AtomicU64, Arc},
    thread,
    time::Duration,
};
use tracing::{debug, error, info, warn};

/// Tracked toplevel window
#[derive(Debug)]
struct TrackedToplevel {
    surface: ToplevelSurface,
    x: i32,
    y: i32,
    mapped: bool,
}

/// Tracked layer surface
#[derive(Debug)]
struct TrackedLayer {
    surface: LayerSurface,
    layer: Layer,
    namespace: String,
}

/// Compositor application state
pub struct RavenCompositor {
    compositor_state: CompositorState,
    xdg_shell_state: XdgShellState,
    layer_shell_state: WlrLayerShellState,
    shm_state: ShmState,
    seat_state: SeatState<Self>,
    seat: Seat<Self>,
    screen_width: u32,
    screen_height: u32,
    toplevels: Vec<TrackedToplevel>,
    // Use Vec of tuples since Layer doesn't implement Hash
    background_layers: Vec<TrackedLayer>,
    bottom_layers: Vec<TrackedLayer>,
    top_layers: Vec<TrackedLayer>,
    overlay_layers: Vec<TrackedLayer>,
    next_window_offset: i32,
    needs_redraw: bool,
    renderer: SoftwareRenderer,
    // Input state
    pointer_location: Point<f64, Logical>,
    keyboard_focus: Option<WlSurface>,
    // Rendering buffers
    gbm_buffer: Option<GbmBuffer>,
    drm_framebuffer: Option<smithay::reexports::drm::control::framebuffer::Handle>,
    drm_surface: Option<DrmSurface>,
}

impl RavenCompositor {
    fn new(display: &DisplayHandle, width: u32, height: u32) -> Self {
        let compositor_state = CompositorState::new::<Self>(display);
        let xdg_shell_state = XdgShellState::new::<Self>(display);
        let layer_shell_state = WlrLayerShellState::new::<Self>(display);
        let shm_state = ShmState::new::<Self>(display, vec![]);
        let mut seat_state = SeatState::new();
        let seat = seat_state.new_wl_seat(display, "seat0");
        let renderer = SoftwareRenderer::new(width, height);

        Self {
            compositor_state,
            xdg_shell_state,
            layer_shell_state,
            shm_state,
            seat_state,
            seat,
            screen_width: width,
            screen_height: height,
            toplevels: Vec::new(),
            background_layers: Vec::new(),
            bottom_layers: Vec::new(),
            top_layers: Vec::new(),
            overlay_layers: Vec::new(),
            next_window_offset: 50,
            needs_redraw: true,
            renderer,
            pointer_location: (0.0, 0.0).into(),
            keyboard_focus: None,
            gbm_buffer: None,
            drm_framebuffer: None,
            drm_surface: None,
        }
    }

    fn add_toplevel(&mut self, surface: ToplevelSurface) {
        let x = self.next_window_offset;
        let y = self.next_window_offset;
        info!("Adding new toplevel window at position ({}, {})", x, y);
        
        self.next_window_offset += 30;
        if self.next_window_offset > 300 {
            self.next_window_offset = 50;
        }

        self.toplevels.push(TrackedToplevel {
            surface,
            x,
            y,
            mapped: false,
        });
        self.needs_redraw = true;
        info!("Total toplevels: {}", self.toplevels.len());
    }

    fn add_layer(&mut self, surface: LayerSurface, layer: Layer, namespace: String) {
        info!("Adding layer surface: namespace={}, layer={:?}", namespace, layer);

        // Configure layer surface with screen size
        surface.with_pending_state(|state| {
            state.size = Some(Size::from((self.screen_width as i32, self.screen_height as i32)));
        });
        surface.send_configure();
        info!("Configured layer surface: {}x{}", self.screen_width, self.screen_height);

        let tracked = TrackedLayer {
            surface,
            layer: layer.clone(),
            namespace,
        };

        match layer {
            Layer::Background => self.background_layers.push(tracked),
            Layer::Bottom => self.bottom_layers.push(tracked),
            Layer::Top => self.top_layers.push(tracked),
            Layer::Overlay => self.overlay_layers.push(tracked),
        }
        self.needs_redraw = true;
        info!("Total layers: {}", self.layer_count());
    }

    fn remove_layer(&mut self, surface: &LayerSurface) {
        self.background_layers.retain(|l| l.surface != *surface);
        self.bottom_layers.retain(|l| l.surface != *surface);
        self.top_layers.retain(|l| l.surface != *surface);
        self.overlay_layers.retain(|l| l.surface != *surface);
        self.needs_redraw = true;
    }

    fn layer_count(&self) -> usize {
        self.background_layers.len()
            + self.bottom_layers.len()
            + self.top_layers.len()
            + self.overlay_layers.len()
    }

    /// Render all surfaces to the internal framebuffer
    fn render_all_surfaces(&mut self) {
        // Clear with dark background
        self.renderer.clear(0xFF0b0f14);
        
        let mut rendered_count = 0;

        // Render in layer order (back to front):
        // 1. Background layer (raven-desktop)
        for layer in &self.background_layers {
            debug!("Rendering background layer: {}", layer.namespace);
            self.renderer.render_layer_surface(&layer.surface);
            rendered_count += 1;
        }

        // 2. Bottom layer
        for layer in &self.bottom_layers {
            debug!("Rendering bottom layer: {}", layer.namespace);
            self.renderer.render_layer_surface(&layer.surface);
            rendered_count += 1;
        }

        // 3. Toplevels (regular windows)
        for toplevel in &self.toplevels {
            if toplevel.mapped {
                debug!("Rendering toplevel at ({}, {})", toplevel.x, toplevel.y);
                self.renderer.render_surface(
                    toplevel.surface.wl_surface(),
                    toplevel.x,
                    toplevel.y,
                );
                rendered_count += 1;
            }
        }

        // 4. Top layer (raven-shell panel)
        for layer in &self.top_layers {
            debug!("Rendering top layer: {}", layer.namespace);
            self.renderer.render_layer_surface(&layer.surface);
            rendered_count += 1;
        }

        // 5. Overlay layer (raven-menu, notifications, etc.)
        for layer in &self.overlay_layers {
            debug!("Rendering overlay layer: {}", layer.namespace);
            self.renderer.render_layer_surface(&layer.surface);
            rendered_count += 1;
        }

        debug!("Rendered {} surfaces total", rendered_count);
        
        // TODO: Render cursor
    }

    /// Get the rendered framebuffer as bytes
    fn get_framebuffer(&self) -> &[u8] {
        self.renderer.as_bytes()
    }

    /// Handle keyboard key event
    fn handle_keyboard_key<B: InputBackend>(&mut self, event: B::KeyboardKeyEvent) {
        let serial = SERIAL_COUNTER.next_serial();
        let time = event.time_msec();
        let key_code = event.key_code();
        let key_state = event.state();

        // Get keyboard from seat
        if let Some(keyboard) = self.seat.get_keyboard() {
            // Process key with XKB
            keyboard.input(
                self,
                key_code,
                key_state,
                serial,
                time,
                |state, modifiers, keysym| {
                    // Check for global shortcuts
                    if key_state == KeyState::Pressed {
                        state.handle_keyboard_shortcut(modifiers, keysym)
                    } else {
                        FilterResult::Forward
                    }
                },
            );
        }
    }

    /// Handle keyboard shortcut
    fn handle_keyboard_shortcut(&mut self, modifiers: &ModifiersState, keysym: KeysymHandle) -> FilterResult<()> {
        let sym = keysym.modified_sym();
        
        // Super key shortcuts
        if modifiers.logo {
            // Match against the raw u32 value
            match sym.raw() {
                // Super + Enter = Launch terminal
                keysym if keysym == KeySyms::KEY_Return => {
                    info!("Shortcut: Super+Enter - launching terminal");
                    spawn_app("raven-terminal").ok();
                    FilterResult::Intercept(())
                }
                // Super + Space = Launch menu
                keysym if keysym == KeySyms::KEY_space => {
                    info!("Shortcut: Super+Space - launching menu");
                    spawn_app("raven-menu").ok();
                    FilterResult::Intercept(())
                }
                // Super + Q = Close focused window
                keysym if keysym == KeySyms::KEY_q => {
                    info!("Shortcut: Super+Q - close window");
                    if let Some(ref surface) = self.keyboard_focus {
                        // Find and close the toplevel with this surface
                        if let Some(idx) = self.toplevels.iter().position(|t| t.surface.wl_surface() == surface) {
                            let toplevel = &self.toplevels[idx];
                            toplevel.surface.send_close();
                            info!("Sent close request to window");
                        }
                    }
                    FilterResult::Intercept(())
                }
                _ => FilterResult::Forward,
            }
        } else {
            FilterResult::Forward
        }
    }

    /// Handle pointer motion event
    fn handle_pointer_motion<B: InputBackend>(&mut self, event: B::PointerMotionEvent) {
        let delta = event.delta();
        self.pointer_location.x += delta.x;
        self.pointer_location.y += delta.y;

        // Clamp to screen bounds
        self.pointer_location.x = self.pointer_location.x.max(0.0).min(self.screen_width as f64 - 1.0);
        self.pointer_location.y = self.pointer_location.y.max(0.0).min(self.screen_height as f64 - 1.0);

        if let Some(pointer) = self.seat.get_pointer() {
            let serial = SERIAL_COUNTER.next_serial();
            let time = event.time_msec();
            
            // Find surface under pointer
            let under = self.surface_under(self.pointer_location);
            
            pointer.motion(
                self,
                under,
                &MotionEvent {
                    location: self.pointer_location,
                    serial,
                    time,
                },
            );
        }

        self.needs_redraw = true;
    }

    /// Handle pointer button event
    fn handle_pointer_button<B: InputBackend>(&mut self, event: B::PointerButtonEvent) {
        let serial = SERIAL_COUNTER.next_serial();
        let time = event.time_msec();
        let button = event.button_code();
        let state = event.state();

        if let Some(pointer) = self.seat.get_pointer() {
            pointer.button(
                self,
                &ButtonEvent {
                    serial,
                    time,
                    button,
                    state,
                },
            );

            // On click, focus the surface under pointer
            if state == ButtonState::Pressed {
                let under = self.surface_under(self.pointer_location);
                if let Some((surface, _)) = under {
                    self.focus_surface(surface);
                }
            }
        }
    }

    /// Handle pointer axis (scroll) event
    fn handle_pointer_axis<B: InputBackend>(&mut self, event: B::PointerAxisEvent) {
        let source = event.source();
        let horizontal = event.amount(Axis::Horizontal).unwrap_or(0.0);
        let vertical = event.amount(Axis::Vertical).unwrap_or(0.0);
        
        // amount_v120 returns Option<f64>, convert to i32
        let h_discrete = event.amount_v120(Axis::Horizontal)
            .map(|v| v as i32)
            .unwrap_or(0);
        let v_discrete = event.amount_v120(Axis::Vertical)
            .map(|v| v as i32)
            .unwrap_or(0);

        if let Some(pointer) = self.seat.get_pointer() {
            let frame = AxisFrame {
                source: Some(source),
                relative_direction: (
                    AxisRelativeDirection::Identical,
                    AxisRelativeDirection::Identical
                ),
                time: event.time_msec(),
                axis: (horizontal, vertical),
                v120: Some((h_discrete, v_discrete)),
                stop: (false, false),
            };

            pointer.axis(self, frame);
        }
    }

    /// Find the surface under a point
    fn surface_under(&self, point: Point<f64, Logical>) -> Option<(WlSurface, Point<f64, Logical>)> {
        let x = point.x;
        let y = point.y;

        // Check layers in reverse order (top to bottom)
        // 1. Overlay layers
        for layer in self.overlay_layers.iter().rev() {
            return Some((layer.surface.wl_surface().clone(), (0.0, 0.0).into()));
        }

        // 2. Top layers (panel)
        for layer in self.top_layers.iter().rev() {
            return Some((layer.surface.wl_surface().clone(), (0.0, 0.0).into()));
        }

        // 3. Toplevels (windows)
        for toplevel in self.toplevels.iter().rev() {
            if !toplevel.mapped {
                continue;
            }
            
            // Simple bounds check (TODO: get actual window size)
            let window_width = 800.0;
            let window_height = 600.0;
            
            if x >= toplevel.x as f64 && x < (toplevel.x as f64 + window_width) &&
               y >= toplevel.y as f64 && y < (toplevel.y as f64 + window_height) {
                let local_x = x - toplevel.x as f64;
                let local_y = y - toplevel.y as f64;
                return Some((toplevel.surface.wl_surface().clone(), (local_x, local_y).into()));
            }
        }

        // 4. Bottom layers
        for layer in self.bottom_layers.iter().rev() {
            return Some((layer.surface.wl_surface().clone(), (0.0, 0.0).into()));
        }

        // 5. Background layers (desktop)
        for layer in self.background_layers.iter().rev() {
            return Some((layer.surface.wl_surface().clone(), (0.0, 0.0).into()));
        }

        None
    }

    /// Focus a surface
    fn focus_surface(&mut self, surface: WlSurface) {
        if let Some(keyboard) = self.seat.get_keyboard() {
            keyboard.set_focus(self, Some(surface.clone()), SERIAL_COUNTER.next_serial());
            self.keyboard_focus = Some(surface);
        }
    }
}

// Client state
#[derive(Default)]
struct ClientState {
    compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {
        debug!("Client initialized");
    }
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {
        debug!("Client disconnected");
    }
}

// Smithay handlers

impl BufferHandler for RavenCompositor {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl CompositorHandler for RavenCompositor {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        debug!("Surface commit received");
        self.needs_redraw = true;

        for toplevel in &mut self.toplevels {
            if toplevel.surface.wl_surface() == surface && !toplevel.mapped {
                toplevel.mapped = true;
                info!("✓ Toplevel window now mapped and ready to render");
            }
        }
    }
}

impl ShmHandler for RavenCompositor {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl SeatHandler for RavenCompositor {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, _seat: &Seat<Self>, _focused: Option<&WlSurface>) {}
    fn cursor_image(
        &mut self,
        _seat: &Seat<Self>,
        _image: smithay::input::pointer::CursorImageStatus,
    ) {
    }
}

impl smithay::wayland::output::OutputHandler for RavenCompositor {}

impl XdgShellHandler for RavenCompositor {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        info!("New XDG toplevel created");
        surface.with_pending_state(|state| {
            state.size = Some(Size::from((800, 600)));
        });
        surface.send_configure();
        self.add_toplevel(surface);
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        surface.send_configure().ok();
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {}

    fn reposition_request(
        &mut self,
        _surface: PopupSurface,
        _positioner: PositionerState,
        _token: u32,
    ) {
    }
}

impl WlrLayerShellHandler for RavenCompositor {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: LayerSurface,
        _output: Option<wl_output::WlOutput>,
        layer: Layer,
        namespace: String,
    ) {
        info!(
            "New layer surface: namespace={}, layer={:?}",
            namespace, layer
        );
        self.add_layer(surface, layer, namespace);
    }

    fn layer_destroyed(&mut self, surface: LayerSurface) {
        info!("Layer surface destroyed");
        self.remove_layer(&surface);
    }
}

// Delegate macros
delegate_compositor!(RavenCompositor);
delegate_shm!(RavenCompositor);
delegate_seat!(RavenCompositor);
delegate_output!(RavenCompositor);
delegate_xdg_shell!(RavenCompositor);
delegate_layer_shell!(RavenCompositor);

fn is_virtual_machine() -> bool {
    if Path::new("/sys/hypervisor").exists() {
        return true;
    }
    if let Ok(vendor) = fs::read_to_string("/sys/class/dmi/id/sys_vendor") {
        let vendor = vendor.to_lowercase();
        if vendor.contains("qemu")
            || vendor.contains("kvm")
            || vendor.contains("vmware")
            || vendor.contains("virtualbox")
        {
            return true;
        }
    }
    false
}

fn ensure_runtime_dir() -> Result<()> {
    if env::var_os("XDG_RUNTIME_DIR").is_some() {
        return Ok(());
    }

    let uid = unsafe { libc::getuid() };
    let runtime_dir = format!("/run/user/{}", uid);

    if Path::new(&runtime_dir).exists() {
        env::set_var("XDG_RUNTIME_DIR", &runtime_dir);
        return Ok(());
    }

    let default = "/run/user/0";
    fs::create_dir_all(default).context("create default XDG_RUNTIME_DIR")?;
    env::set_var("XDG_RUNTIME_DIR", default);
    Ok(())
}

fn spawn_startup_apps() {
    info!("spawn_startup_apps: Waiting for compositor...");
    thread::sleep(Duration::from_millis(500));

    info!("spawn_startup_apps: Starting applications...");

    let _ = spawn_app("raven-desktop");
    thread::sleep(Duration::from_millis(200));

    let _ = spawn_app("raven-shell");
    thread::sleep(Duration::from_millis(300));

    if spawn_app("raven-terminal").is_err() {
        let _ = spawn_app("foot");
    }

    info!("spawn_startup_apps: Done");
}

fn spawn_app(name: &str) -> Result<()> {
    info!("Spawning: {}", name);
    Command::new(name)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context(format!("spawn {}", name))?;
    Ok(())
}

pub fn run_native(_config: &Config) -> Result<()> {
    eprintln!("=== run_native() called (Smithay 0.7) ===");
    ensure_runtime_dir()?;

    info!("Starting native backend with libseat session");

    // Create event loop
    let mut event_loop: EventLoop<'static, RavenCompositor> =
        EventLoop::try_new().context("Failed to create event loop")?;
    let loop_handle = event_loop.handle();

    // Initialize session
    info!("Initializing session (libseat)");
    let (mut session, session_notifier) = LibSeatSession::new().context(
        "Failed to create session. Ensure seatd is running.",
    )?;

    info!("Session created on seat: {}", session.seat());

    // Find DRM device
    let drm_node = find_drm_node(&session)?;
    info!("Found DRM device: {:?}", drm_node);

    // Open DRM device through session
    let drm_fd = session
        .open(&drm_node, OFlags::RDWR | OFlags::CLOEXEC | OFlags::NONBLOCK)
        .context("Failed to open DRM device")?;

    // Create DRM device fd wrapper - session.open returns OwnedFd directly
    let drm_device_fd = DrmDeviceFd::new(drm_fd.into());

    // Create DRM device
    let (drm_device, drm_notifier) = DrmDevice::new(drm_device_fd.clone(), true)
        .context("Failed to create DRM device")?;

    // Get display mode
    let res = drm_device_fd.resource_handles().context("Failed to get DRM resources")?;

    let all_connectors: Vec<_> = res
        .connectors()
        .iter()
        .filter_map(|conn| drm_device_fd.get_connector(*conn, false).ok())
        .collect();

    for conn in &all_connectors {
        info!(
            "Connector: {:?}-{} state={:?} modes={}",
            conn.interface(),
            conn.interface_id(),
            conn.state(),
            conn.modes().len()
        );
    }

    let connector = all_connectors
        .iter()
        .find(|c| c.state() == ConnectorState::Connected)
        .or_else(|| all_connectors.iter().find(|c| !c.modes().is_empty()))
        .or_else(|| all_connectors.first())
        .ok_or_else(|| anyhow!("No display connectors found"))?
        .clone();

    let mode = connector
        .modes()
        .first()
        .copied()
        .ok_or_else(|| anyhow!("No display modes available"))?;

    let (width, height) = mode.size();
    let refresh = mode.vrefresh();
    info!("Using mode: {}x{}@{}Hz", width, height, refresh);

    // Find encoder and CRTC
    let encoder_handle = connector
        .current_encoder()
        .or_else(|| connector.encoders().first().copied())
        .ok_or_else(|| anyhow!("No encoder found"))?;

    let encoder_info = drm_device_fd.get_encoder(encoder_handle)?;
    let crtc = encoder_info
        .crtc()
        .or_else(|| {
            res.filter_crtcs(encoder_info.possible_crtcs())
                .first()
                .copied()
        })
        .ok_or_else(|| anyhow!("No CRTC available"))?;

    info!("Using CRTC: {:?}", crtc);

    // Create DRM surface for rendering
    let mut drm_device = drm_device; // Make mutable
    let drm_surface = drm_device
        .create_surface(crtc, mode, &[connector.handle()])
        .context("Failed to create DRM surface")?;

    info!("DRM surface created");

    // Try to create GBM buffer for rendering (preferred for modern drivers like virtio-gpu)
    info!("Attempting GBM buffer allocation...");
    
    let gbm_device = match GbmDevice::new(drm_device_fd.clone()) {
        Ok(dev) => {
            info!("GBM device created successfully");
            info!("  Backend name: {}", dev.backend_name());
            dev
        }
        Err(e) => {
            error!("Failed to create GBM device: {}", e);
            error!("Cannot initialize rendering without GBM support");
            return Err(anyhow!("GBM initialization failed: {}", e));
        }
    };

    let mut allocator = GbmAllocator::new(
        gbm_device,
        gbm::BufferObjectFlags::RENDERING | gbm::BufferObjectFlags::SCANOUT,
    );
    
    info!("GBM allocator created, attempting buffer allocation...");
    info!("  Resolution: {}x{}", width, height);
    info!("  Format: XRGB8888");
    
    let gbm_buffer_result = allocator.create_buffer(
        width as u32, 
        height as u32, 
        Fourcc::Xrgb8888,
        &[smithay::backend::allocator::Modifier::Linear],
    );
    
    let (stored_gbm_buffer, stored_fb_handle) = match gbm_buffer_result {
        Ok(buffer) => {
            info!("GBM buffer allocated successfully");
            info!("  Buffer size: {}x{}", buffer.width(), buffer.height());
            info!("  Buffer format: {:?}", buffer.format());
            
            // Create framebuffer from GBM buffer
            info!("Creating DRM framebuffer from GBM buffer...");
            
            // Export GBM buffer as dmabuf
            let dmabuf = match buffer.export() {
                Ok(d) => {
                    info!("GBM buffer exported as dmabuf");
                    d
                }
                Err(e) => {
                    error!("Failed to export GBM buffer: {:?}", e);
                    return Err(anyhow!("GBM buffer export failed"));
                }
            };
            
            // Create framebuffer from dmabuf handles
            use smithay::reexports::drm::control::Device as ControlDevice;
            
            // Get first dmabuf plane info
            let handles = dmabuf.handles();
            let strides = dmabuf.strides();
            let offsets = dmabuf.offsets();
            
            info!("Dmabuf info: format={:?}, width={}, height={}", 
                  dmabuf.format().code, dmabuf.width(), dmabuf.height());
            
            // Create framebuffer - try simple add_framebuffer first for compatibility
            let fb_result = drm_device_fd.add_framebuffer(
                &buffer,
                32,  // depth (bits per pixel) 
                32,  // bpp (bits per pixel)
            );
            
            match fb_result {
                Ok(fb_handle) => {
                    info!("DRM framebuffer created: {:?}", fb_handle);

                    // Initial modeset/commit to activate display
                    info!("Performing initial modeset to activate display...");
                    use smithay::backend::drm::PlaneState;
                    let plane = drm_surface.plane();

                    let src_rect = smithay::utils::Rectangle::<f64, smithay::utils::Buffer>::from_size(
                        (width as f64, height as f64).into(),
                    ).to_f64();
                    let dst_rect = smithay::utils::Rectangle::<i32, smithay::utils::Physical>::from_size(
                        (width as i32, height as i32).into(),
                    );

                    let plane_state = PlaneState {
                        handle: plane,
                        config: Some(smithay::backend::drm::PlaneConfig {
                            src: src_rect,
                            dst: dst_rect,
                            alpha: 1.0,
                            transform: smithay::utils::Transform::Normal,
                            damage_clips: None,
                            fb: fb_handle,
                            fence: None,
                        }),
                    };

                    if let Err(e) = drm_surface.commit([plane_state].into_iter(), true) {
                        error!("Initial modeset failed: {}", e);
                        warn!("Display may not be active, but continuing...");
                    } else {
                        info!("✓ Initial modeset complete - display is now active!");
                    }
                    
                    (Some(buffer), Some(fb_handle))
                }
                Err(e) => {
                    error!("Failed to create framebuffer from GBM buffer: {}", e);
                    warn!("Continuing without framebuffer - visual output will not work");
                    (Some(buffer), None)
                }
            }
        }
        Err(e) => {
            error!("Failed to create GBM buffer: {}", e);
            error!("Visual output will not be available");
            error!("This is unexpected for virtio-gpu devices - please check your QEMU configuration");
            (None, None)
        }
    };

    // Create Wayland display
    info!("Initializing Wayland display");
    let mut display: Display<RavenCompositor> = Display::new()?;
    let dh = display.handle();

    // Create compositor state
    let mut state = RavenCompositor::new(&dh, width.into(), height.into());
    
    // Store GBM buffer, framebuffer, and DRM surface in state
    state.gbm_buffer = stored_gbm_buffer;
    state.drm_framebuffer = stored_fb_handle;
    state.drm_surface = Some(drm_surface);
    
    if state.gbm_buffer.is_some() {
        info!("✓ Created compositor state with GBM rendering enabled");
    } else {
        warn!("Created compositor state WITHOUT rendering (no GBM buffer)");
    }
    info!("✓ Layer-shell support: enabled");

    // Add input devices
    if let Err(e) = state.seat.add_keyboard(Default::default(), 200, 25) {
        warn!("Failed to add keyboard: {}", e);
    }
    state.seat.add_pointer();

    // Create Wayland socket
    let socket = ListeningSocket::bind_auto("wayland", 0..33)
        .context("Failed to create Wayland socket")?;
    let socket_name = socket.socket_name().unwrap();
    info!("Wayland socket: {:?}", socket_name);
    env::set_var("WAYLAND_DISPLAY", socket_name);

    // Create output
    let output = Output::new(
        "Virtual-1".to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Raven".to_string(),
            model: "Display".to_string(),
        },
    );

    output.change_current_state(
        Some(OutputMode {
            size: (width as i32, height as i32).into(),
            refresh: refresh as i32 * 1000,
        }),
        None,
        Some(Scale::Integer(1)),
        Some((0, 0).into()),
    );
    output.create_global::<RavenCompositor>(&dh);

    info!("Display initialized");

    // Insert session notifier
    loop_handle
        .insert_source(session_notifier, |_, _, _| {
            info!("Session event");
        })
        .map_err(|e| anyhow!("Failed to insert session source: {:?}", e))?;

    // Initialize libinput
    info!("Initializing libinput");
    let mut libinput = Libinput::new_with_udev(LibinputSessionInterface::from(session.clone()));
    libinput.udev_assign_seat(&session.seat()).ok();

    let libinput_backend = LibinputInputBackend::new(libinput);
    loop_handle
        .insert_source(libinput_backend, |event, _, state| {
            use BackendInputEvent::*;
            match event {
                Keyboard { event } => {
                    state.handle_keyboard_key::<LibinputInputBackend>(event);
                }
                PointerMotion { event } => {
                    state.handle_pointer_motion::<LibinputInputBackend>(event);
                }
                PointerButton { event } => {
                    state.handle_pointer_button::<LibinputInputBackend>(event);
                }
                PointerAxis { event } => {
                    state.handle_pointer_axis::<LibinputInputBackend>(event);
                }
                _ => {
                    debug!("Unhandled input event: {:?}", event);
                }
            }
        })
        .map_err(|e| anyhow!("Failed to insert libinput: {:?}", e))?;

    // Clone what we need for the VBlank handler
    let drm_device_fd_clone = drm_device_fd.clone();
    
    // Insert DRM notifier for vblank events
    loop_handle
        .insert_source(drm_notifier, move |event, _, state| {
            match event {
                DrmEvent::VBlank(_) => {
                    static VBLANK_COUNT: AtomicU64 = AtomicU64::new(0);
                    let count = VBLANK_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    
                    // Log periodically
                    if count == 0 || count % 300 == 0 {
                        info!(
                            "VBlank #{}: {} toplevels, {} layers, redraw={}",
                            count,
                            state.toplevels.len(),
                            state.layer_count(),
                            state.needs_redraw
                        );
                    }
                    
                    // Render frame if needed
                    if state.needs_redraw {
                        debug!("Rendering frame #{}", count);
                        
                        // Render all surfaces to internal software buffer
                        state.render_all_surfaces();
                        
                        // Copy to GBM buffer and present
                        if let (Some(ref mut buffer), Some(fb_handle), Some(ref surface)) = 
                            (&mut state.gbm_buffer, state.drm_framebuffer, &state.drm_surface) {
                            
                            // Write directly to GBM buffer using the write method
                            let framebuffer_bytes = state.renderer.as_bytes();
                            
                            match buffer.write(framebuffer_bytes) {
                                Ok(()) => {
                                    debug!("Framebuffer copied to GBM buffer ({} bytes)", framebuffer_bytes.len());
                                    
                                    // Commit to DRM to display the frame
                                    use smithay::backend::drm::PlaneState;
                                    let plane = surface.plane();
                                    
                                    let src_rect = smithay::utils::Rectangle::<f64, smithay::utils::Buffer>::from_size(
                                        (buffer.width() as f64, buffer.height() as f64).into(),
                                    ).to_f64();
                                    let dst_rect = smithay::utils::Rectangle::<i32, smithay::utils::Physical>::from_size(
                                        (buffer.width() as i32, buffer.height() as i32).into(),
                                    );
                                    
                                    let plane_state = PlaneState {
                                        handle: plane,
                                        config: Some(smithay::backend::drm::PlaneConfig {
                                            src: src_rect,
                                            dst: dst_rect,
                                            alpha: 1.0,
                                            transform: smithay::utils::Transform::Normal,
                                            damage_clips: None,
                                            fb: fb_handle,
                                            fence: None,
                                        }),
                                    };
                                    
                                    if let Err(e) = surface.commit([plane_state].into_iter(), false) {
                                        warn!("DRM commit failed: {}", e);
                                    } else {
                                        debug!("Frame presented to display");
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to write to GBM buffer: {}", e);
                                }
                            }
                        } else {
                            if count % 300 == 0 {
                                debug!("No GBM buffer available, skipping frame presentation");
                            }
                        }
                        
                        state.needs_redraw = false;
                    }
                }
                DrmEvent::Error(e) => {
                    error!("DRM error: {}", e);
                }
            }
        })
        .map_err(|e| anyhow!("Failed to insert DRM: {:?}", e))?;

    // Spawn startup apps
    thread::spawn(spawn_startup_apps);

    info!("Entering main event loop");
    eprintln!("=== ENTERING MAIN EVENT LOOP ===");

    // Main loop
    loop {
        if let Some(stream) = socket.accept()? {
            info!("New client connected");
            display
                .handle()
                .insert_client(stream, Arc::new(ClientState::default()))
                .ok();
        }

        display.dispatch_clients(&mut state)?;
        display.flush_clients()?;

        event_loop
            .dispatch(Some(Duration::from_millis(16)), &mut state)
            .context("Event loop error")?;
    }
}

fn find_drm_node(_session: &LibSeatSession) -> Result<std::path::PathBuf> {
    let udev = udev::Enumerator::new()?;
    let mut enumerator = udev;
    enumerator.match_subsystem("drm")?;
    enumerator.match_sysname("card[0-9]*")?;

    for device in enumerator.scan_devices()? {
        if let Some(devnode) = device.devnode() {
            return Ok(devnode.to_path_buf());
        }
    }

    Err(anyhow!("No DRM device found"))
}
