//! Native (DRM/KMS) backend with libseat session management.
//!
//! This implementation uses libseat for proper session management,
//! allowing the compositor to run without root privileges when
//! seatd or systemd-logind is available.
//!
//! ## VM Support
//!
//! When running in a VM without a connected display, set `RAVEN_FORCE_MODE=WxH`
//! (e.g., `RAVEN_FORCE_MODE=1024x768`) to create a synthetic display mode.
//! This is useful for testing in headless VMs or when QEMU isn't configured
//! with a proper display device.

use crate::config::Config;
use anyhow::{anyhow, Context, Result};
use smithay::{
    backend::{
        allocator::{dumb::DumbBuffer, Fourcc, Swapchain},
        drm::{DrmDevice, DrmEvent},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        session::{auto::AutoSession, Session},
    },
    reexports::{
        calloop::{generic::Generic, EventLoop, Interest, Mode, PostAction},
        drm::control::{connector::State as ConnectorState, Device as ControlDevice, Mode as DrmMode},
        input::Libinput,
        nix::fcntl::OFlag,
        wayland_server::{protocol::wl_output, Display},
    },
    wayland::{
        compositor::compositor_init,
        output::{Mode as WlMode, Output, PhysicalProperties},
        seat::Seat,
        shell::xdg::{xdg_shell_init, XdgRequest},
        shm::init_shm_global,
    },
};
use std::{
    cell::RefCell,
    env,
    ffi::OsString,
    fs,
    os::unix::io::{FromRawFd, RawFd},
    path::Path,
    rc::Rc,
    time::Duration,
};
use tracing::{debug, error, info, warn};

/// Check if we're running in a virtual machine
fn is_virtual_machine() -> bool {
    // Check for hypervisor via sysfs
    if Path::new("/sys/hypervisor").exists() {
        return true;
    }
    // Check for common VM indicators in DMI
    if let Ok(vendor) = fs::read_to_string("/sys/class/dmi/id/sys_vendor") {
        let vendor = vendor.to_lowercase();
        if vendor.contains("qemu") || vendor.contains("kvm") ||
           vendor.contains("vmware") || vendor.contains("virtualbox") ||
           vendor.contains("xen") || vendor.contains("microsoft") {
            return true;
        }
    }
    // Check for VM-specific CPU flags
    if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
        if cpuinfo.contains("hypervisor") {
            return true;
        }
    }
    false
}

/// Parse RAVEN_FORCE_MODE environment variable (e.g., "1024x768")
fn parse_forced_mode() -> Option<(u16, u16)> {
    env::var("RAVEN_FORCE_MODE").ok().and_then(|mode| {
        let parts: Vec<&str> = mode.split('x').collect();
        if parts.len() == 2 {
            let width = parts[0].parse::<u16>().ok()?;
            let height = parts[1].parse::<u16>().ok()?;
            if width >= 640 && height >= 480 && width <= 4096 && height <= 4096 {
                return Some((width, height));
            }
        }
        warn!("Invalid RAVEN_FORCE_MODE format: '{}', expected 'WxH' (e.g., '1024x768')", mode);
        None
    })
}

/// Create a synthetic DRM mode for VMs without connected displays
fn create_synthetic_mode(width: u16, height: u16) -> DrmMode {
    use drm_ffi::drm_mode_modeinfo;

    // Create a mode similar to what DRM drivers generate
    // Using standard CVT timing for 60Hz refresh
    let htotal = width + 160;  // Approximate horizontal blanking
    let vtotal = height + 36;  // Approximate vertical blanking
    let clock = (htotal as u32 * vtotal as u32 * 60) / 1000; // Pixel clock in kHz

    // Build mode name (e.g., "1024x768") - needs to be [c_char; 32]
    let mut name: [libc::c_char; 32] = [0; 32];
    let name_str = format!("{}x{}", width, height);
    let name_bytes = name_str.as_bytes();
    for (i, &byte) in name_bytes.iter().take(31).enumerate() {
        name[i] = byte as libc::c_char;
    }

    let modeinfo = drm_mode_modeinfo {
        clock,
        hdisplay: width,
        hsync_start: width + 48,
        hsync_end: width + 112,
        htotal,
        hskew: 0,
        vdisplay: height,
        vsync_start: height + 3,
        vsync_end: height + 9,
        vtotal,
        vscan: 0,
        vrefresh: 60,
        flags: 0,
        type_: 0x40, // DRM_MODE_TYPE_USERDEF
        name,
    };

    DrmMode::from(modeinfo)
}

fn ensure_runtime_dir() -> Result<()> {
    if env::var_os("XDG_RUNTIME_DIR").is_some() {
        return Ok(());
    }

    // Try standard locations
    let uid = unsafe { libc::getuid() };
    let runtime_dir = format!("/run/user/{}", uid);

    if Path::new(&runtime_dir).exists() {
        env::set_var("XDG_RUNTIME_DIR", &runtime_dir);
        return Ok(());
    }

    // Live ISO runs as root; create default path
    let default = "/run/user/0";
    fs::create_dir_all(default).context("create default XDG_RUNTIME_DIR")?;
    env::set_var("XDG_RUNTIME_DIR", default);
    Ok(())
}

/// Find a connected DRM device
fn find_drm_device(session: &mut AutoSession) -> Result<RawFd> {
    // Use udev to find DRM devices
    let udev = udev::Enumerator::new()?;
    let mut enumerator = udev;
    enumerator.match_subsystem("drm")?;
    enumerator.match_sysname("card[0-9]*")?;

    for device in enumerator.scan_devices()? {
        if let Some(devnode) = device.devnode() {
            info!("Trying DRM device: {:?}", devnode);

            // Open through session for proper DRM master handling
            match session.open(
                devnode,
                OFlag::O_RDWR | OFlag::O_CLOEXEC | OFlag::O_NOCTTY | OFlag::O_NONBLOCK,
            ) {
                Ok(fd) => {
                    info!("Opened DRM device: {:?}", devnode);
                    return Ok(fd);
                }
                Err(err) => {
                    warn!("Failed to open {:?}: {}", devnode, err);
                }
            }
        }
    }

    Err(anyhow!("No usable DRM device found"))
}

pub fn run_native(_config: &Config) -> Result<()> {
    ensure_runtime_dir()?;

    info!("Starting native backend with libseat session");

    // Create event loop
    let mut event_loop: EventLoop<'static, ()> =
        EventLoop::try_new().context("Failed to create event loop")?;
    let loop_handle = event_loop.handle();

    // Initialize session (libseat or logind)
    info!("Initializing session (libseat/logind)");
    let (mut session, session_notifier) = AutoSession::new(None).context(
        "Failed to create session. Ensure seatd is running: 'seatd -g video &'",
    )?;

    info!("Session created on seat: {}", session.seat());

    // Find and open DRM device through session
    let drm_fd = find_drm_device(&mut session)?;

    // Create Wayland display
    info!("Initializing Wayland display");
    let mut display = Display::new();

    // Initialize Wayland globals
    compositor_init(&mut display, |_surface, _dispatch_data| {}, None);
    init_shm_global(&mut display, Vec::new(), None);

    let (_shell_state, _xdg_global, _zxdg_global) = xdg_shell_init(
        &mut display,
        |request, _dispatch_data| match request {
            XdgRequest::NewToplevel { surface } => {
                surface.send_configure();
            }
            XdgRequest::NewPopup { surface, .. } => {
                surface.send_configure();
            }
            _ => {}
        },
        None,
    );

    let (mut seat, _seat_global) = Seat::new(&mut display, "seat0".to_string(), None);

    // Add input capabilities
    seat.add_pointer(|_| {});
    if let Err(err) = seat.add_keyboard(Default::default(), 200, 25, |_, _| {}) {
        warn!("Failed to initialize keyboard: {}", err);
    }

    // Create Wayland socket
    let socket_name: OsString = display
        .add_socket_auto()
        .context("Failed to create Wayland socket")?;
    info!("Wayland socket: {:?}", socket_name);

    // Set WAYLAND_DISPLAY for child processes
    env::set_var("WAYLAND_DISPLAY", &socket_name);

    // Initialize DRM device
    info!("Initializing DRM device");
    let drm_device = DrmDevice::new(
        unsafe { std::fs::File::from_raw_fd(drm_fd) },
        true,
        None,
    )
    .context("Failed to create DRM device")?;

    // Get DRM resources
    let res = drm_device
        .resource_handles()
        .context("Failed to get DRM resources")?;

    // Find connected connector
    // List all connectors for debugging
    let all_connectors: Vec<_> = res
        .connectors()
        .iter()
        .filter_map(|conn| drm_device.get_connector(*conn).ok())
        .collect();

    for conn in &all_connectors {
        info!(
            "Found connector: {:?}-{} state={:?} modes={}",
            conn.interface(),
            conn.interface_id(),
            conn.state(),
            conn.modes().len()
        );
    }

    // First try to find a connected connector
    let connector_info = all_connectors
        .iter()
        .find(|conn| conn.state() == ConnectorState::Connected)
        .or_else(|| {
            // For virtual GPUs (virtio-gpu), connector might be "Unknown" but still usable
            // if it has modes available
            all_connectors
                .iter()
                .find(|conn| !conn.modes().is_empty())
        })
        .or_else(|| {
            // Last resort: just take the first connector
            all_connectors.first()
        })
        .ok_or_else(|| anyhow!("No display connectors found"))?
        .clone();

    info!(
        "Using display: {:?}-{} ({:?})",
        connector_info.interface(),
        connector_info.interface_id(),
        connector_info.state()
    );

    // Find encoder and CRTC
    let encoder_handle = connector_info
        .encoders()
        .iter()
        .filter_map(|&e| e)
        .next()
        .ok_or_else(|| anyhow!("No encoder found"))?;

    let encoder_info = drm_device.get_encoder(encoder_handle)?;
    let crtc = encoder_info
        .crtc()
        .or_else(|| {
            res.filter_crtcs(encoder_info.possible_crtcs())
                .first()
                .copied()
        })
        .ok_or_else(|| anyhow!("No CRTC available"))?;

    // Get first available mode (usually the native resolution)
    // If no modes are available and we're in a VM, try to create a synthetic mode
    let mode = connector_info
        .modes()
        .first()
        .copied()
        .or_else(|| {
            // Check for forced mode via environment variable
            if let Some((width, height)) = parse_forced_mode() {
                info!("Using forced mode from RAVEN_FORCE_MODE: {}x{}", width, height);
                return Some(create_synthetic_mode(width, height));
            }

            // Check if we're in a VM and can use a default synthetic mode
            let is_vm = is_virtual_machine();
            let is_virtual_connector = matches!(
                connector_info.interface(),
                smithay::reexports::drm::control::connector::Interface::Virtual
            );

            if is_vm || is_virtual_connector {
                warn!("No display modes available, but running in VM environment");
                warn!("Creating synthetic 1024x768@60Hz mode for testing");
                warn!("Tip: Set RAVEN_FORCE_MODE=WxH for custom resolution");
                warn!("Tip: For proper display, use QEMU with: -device virtio-vga-gl -display gtk,gl=on");
                return Some(create_synthetic_mode(1024, 768));
            }

            None
        })
        .ok_or_else(|| {
            // Provide helpful error message based on environment
            if is_virtual_machine() {
                anyhow!(
                    "No display modes available.\n\
                    You appear to be in a VM. Try one of:\n\
                    1. Set RAVEN_FORCE_MODE=1024x768 environment variable\n\
                    2. Use proper QEMU display: -device virtio-vga-gl -display gtk,gl=on\n\
                    3. Use VNC/SPICE: -device qxl-vga -vnc :0"
                )
            } else {
                anyhow!(
                    "No display modes available.\n\
                    Check that your display is connected and detected by the kernel.\n\
                    Run 'cat /sys/class/drm/*/status' to see connector states."
                )
            }
        })?;

    let (width, height) = mode.size();
    let refresh = mode.vrefresh();
    info!("Using mode: {}x{}@{}Hz", width, height, refresh);

    // Create DRM surface
    let surface = Rc::new(
        drm_device
            .create_surface(crtc, mode, &[connector_info.handle()])
            .context("Failed to create DRM surface")?,
    );

    // Get supported formats for swapchain
    let mods = surface
        .supported_formats(surface.plane())
        .context("read supported formats")?
        .iter()
        .filter_map(|format| {
            if format.code == Fourcc::Xrgb8888 {
                Some(format.modifier)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    // Create swapchain with dumb buffers for software rendering
    let allocator = DrmDevice::new(
        unsafe { std::fs::File::from_raw_fd(drm_fd) },
        false,
        None,
    )
    .context("create DRM allocator")?;

    let mut swapchain: Swapchain<DrmDevice<std::fs::File>, DumbBuffer<std::fs::File>, _> =
        Swapchain::new(allocator, width.into(), height.into(), Fourcc::Xrgb8888, mods);

    // Acquire initial buffer and do first modeset
    let first_buffer = swapchain
        .acquire()
        .context("acquire initial buffer")?
        .ok_or_else(|| anyhow!("No buffer available"))?;

    let framebuffer = surface
        .add_framebuffer(first_buffer.handle(), 32, 32)
        .context("create initial framebuffer")?;

    *first_buffer.userdata() = Some(framebuffer);

    // Initial modeset
    surface
        .commit([(framebuffer, surface.plane())].iter(), true)
        .context("initial modeset commit")?;

    info!("Display initialized successfully");

    // Create Wayland output
    let (wl_output, _output_global) = Output::new(
        &mut display,
        format!("{:?}-{}", connector_info.interface(), connector_info.interface_id()),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: wl_output::Subpixel::Unknown,
            make: "Raven".to_string(),
            model: "Display".to_string(),
        },
        None,
    );

    wl_output.change_current_state(
        Some(WlMode {
            size: (width as i32, height as i32).into(),
            refresh: refresh as i32 * 1000,
        }),
        None,
        Some(1),
        Some((0, 0).into()),
    );

    // Initialize libinput for input handling
    info!("Initializing libinput");
    let mut libinput_context =
        Libinput::new_with_udev(LibinputSessionInterface::from(session.clone()));
    libinput_context.udev_assign_seat(&session.seat()).ok();

    let libinput_backend = LibinputInputBackend::new(libinput_context, None);

    // Wrap display for event loop access
    let display = Rc::new(RefCell::new(display));
    let display_clone = display.clone();

    // Insert Wayland display into event loop
    let display_fd = display.borrow().get_poll_fd();
    loop_handle
        .insert_source(
            Generic::from_fd(display_fd, Interest::READ, Mode::Level),
            move |_, _, _| {
                display_clone
                    .borrow_mut()
                    .dispatch(Duration::from_millis(0), &mut ())
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                display_clone.borrow_mut().flush_clients(&mut ());
                Ok(PostAction::Continue)
            },
        )
        .map_err(|e| anyhow!("Failed to insert display source: {:?}", e))?;

    // Insert session notifier for VT switching
    loop_handle
        .insert_source(session_notifier, |_, _, _| {
            // Session events are handled automatically by AutoSession
            info!("Session event received");
        })
        .map_err(|e| anyhow!("Failed to insert session source: {:?}", e))?;

    // Insert libinput backend
    loop_handle
        .insert_source(libinput_backend, |event, _, _| {
            debug!("Input event: {:?}", event);
        })
        .map_err(|e| anyhow!("Failed to insert libinput source: {:?}", e))?;

    // VBlank handler state
    let surface_clone = surface.clone();
    let mut current_buffer = first_buffer;
    let mut frame_count: u64 = 0;

    // Insert DRM device for vblank events
    loop_handle
        .insert_source(drm_device, move |event, _, _| {
            match event {
                DrmEvent::VBlank(_crtc) => {
                    // Acquire next buffer
                    if let Ok(Some(next_buffer)) = swapchain.acquire() {
                        // Create framebuffer if needed
                        if next_buffer.userdata().is_none() {
                            if let Ok(fb) = surface_clone.add_framebuffer(next_buffer.handle(), 32, 32) {
                                *next_buffer.userdata() = Some(fb);
                            }
                        }

                        // Fill buffer with animated color
                        let fill = ((frame_count / 2) % 60 + 20) as u8;
                        let mut handle = *next_buffer.handle();
                        match surface_clone.map_dumb_buffer(&mut handle) {
                            Ok(mut db) => {
                                // Dark blue-gray Raven color
                                for chunk in db.as_mut().chunks_exact_mut(4) {
                                    chunk[0] = fill / 5;      // B
                                    chunk[1] = fill / 5;      // G
                                    chunk[2] = fill / 4;      // R (slightly more red for warmth)
                                    chunk[3] = 255;           // A
                                }
                                drop(db); // Explicitly drop before handle goes out of scope
                            }
                            Err(_) => {}
                        }
                        drop(handle);

                        // Page flip
                        if let Some(fb) = *next_buffer.userdata() {
                            if let Err(e) = surface_clone.page_flip([(fb, surface_clone.plane())].iter(), true) {
                                warn!("Page flip failed: {}", e);
                            }
                        }

                        let _ = next_buffer; // Keep buffer alive
                        frame_count = frame_count.wrapping_add(1);
                    }
                }
                DrmEvent::Error(e) => {
                    error!("DRM error: {}", e);
                }
            }
        })
        .map_err(|e| anyhow!("Failed to insert DRM source: {:?}", e))?;

    info!("Entering main event loop");
    info!("Press Ctrl+Alt+F2 to switch to another TTY");

    // Main event loop
    loop {
        // Dispatch events with 16ms timeout (~60fps)
        event_loop
            .dispatch(Some(Duration::from_millis(16)), &mut ())
            .context("Event loop error")?;

        // Flush Wayland clients
        display.borrow_mut().flush_clients(&mut ());
    }
}
