//! RavenBoot - Custom UEFI Bootloader for RavenLinux
//!
//! A multi-boot capable bootloader that can coexist with other operating systems.
//! Supports booting Linux kernels directly via UEFI stub or traditional boot.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;
use uefi::prelude::*;
use uefi::proto::console::text::{Color, Key, ScanCode};
use uefi::proto::loaded_image::LoadedImage;
use uefi::proto::media::file::{File, FileAttribute, FileMode};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::CString16;

mod config;
mod linux;
mod menu;

use config::{BootConfig, BootEntry, EntryType, CONFIG_PATHS, KNOWN_BOOTLOADERS, MAX_SUBMENU_DEPTH};
use linux::{boot_efi_stub, chainload_efi, KernelError};

/// Menu navigation state
struct MenuNav<'a> {
    /// Stack of menu levels (indices into parent's children)
    stack: [usize; MAX_SUBMENU_DEPTH],
    /// Current depth (0 = root menu)
    depth: usize,
    /// Selected index at current level
    selected: usize,
    /// Reference to root config
    config: &'a BootConfig,
}

impl<'a> MenuNav<'a> {
    fn new(config: &'a BootConfig) -> Self {
        Self {
            stack: [0; MAX_SUBMENU_DEPTH],
            depth: 0,
            selected: config.default.min(config.entries.len().saturating_sub(1)),
            config,
        }
    }

    /// Get current menu entries
    fn current_entries(&self) -> &[BootEntry] {
        if self.depth == 0 {
            &self.config.entries
        } else {
            // Navigate to current submenu
            let mut entries = &self.config.entries;
            for i in 0..self.depth {
                let idx = self.stack[i];
                if idx < entries.len() && entries[idx].entry_type == EntryType::Submenu {
                    entries = &entries[idx].children;
                }
            }
            entries
        }
    }

    /// Get selected entry
    fn selected_entry(&self) -> Option<&BootEntry> {
        let entries = self.current_entries();
        entries.get(self.selected)
    }

    /// Enter a submenu
    fn enter_submenu(&mut self) {
        if self.depth < MAX_SUBMENU_DEPTH - 1 {
            self.stack[self.depth] = self.selected;
            self.depth += 1;
            self.selected = 0;
        }
    }

    /// Go back to parent menu
    fn go_back(&mut self) {
        if self.depth > 0 {
            self.depth -= 1;
            self.selected = self.stack[self.depth];
        }
    }

    /// Move selection up
    fn move_up(&mut self) {
        let len = self.current_entries().len();
        if self.selected > 0 {
            self.selected -= 1;
        } else if len > 0 {
            self.selected = len - 1;
        }
    }

    /// Move selection down
    fn move_down(&mut self) {
        let len = self.current_entries().len();
        if self.selected < len.saturating_sub(1) {
            self.selected += 1;
        } else {
            self.selected = 0;
        }
    }

    /// Get menu title for current level
    fn menu_title(&self) -> &str {
        if self.depth == 0 {
            "Select an operating system to boot:"
        } else {
            // Get parent submenu name
            let mut entries = &self.config.entries;
            for i in 0..self.depth - 1 {
                let idx = self.stack[i];
                if idx < entries.len() {
                    entries = &entries[idx].children;
                }
            }
            let parent_idx = self.stack[self.depth - 1];
            if parent_idx < entries.len() {
                &entries[parent_idx].name
            } else {
                "Submenu"
            }
        }
    }
}

/// Bootloader version
const VERSION: &str = "0.1.0";

/// Main entry point
#[entry]
fn main(image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    // Initialize UEFI services (also sets up allocator)
    uefi::helpers::init(&mut system_table).unwrap();

    // Clear screen and set colors
    {
        let stdout = system_table.stdout();
        let _ = stdout.clear();
        let _ = stdout.set_color(Color::LightCyan, Color::Black);
        print_banner(stdout);
    }

    // Load configuration
    let config = {
        let boot_services = system_table.boot_services();
        match load_config(boot_services, image_handle) {
            Ok(cfg) => cfg,
            Err(_) => {
                let stdout = system_table.stdout();
                let _ = writeln!(stdout, "Warning: Could not load config, using defaults");
                BootConfig::default()
            }
        }
    };

    // Main menu loop with submenu support
    let mut nav = MenuNav::new(&config);

    loop {
        // Display boot menu and handle navigation
        let action = display_menu_with_nav(&mut system_table, &mut nav);

        match action {
            MenuAction::Boot(entry) => {
                {
                    let stdout = system_table.stdout();
                    let _ = stdout.set_color(Color::White, Color::Black);
                    let _ = writeln!(stdout, "\nBooting: {}", entry.name);
                }

                // Boot the selected entry
                let result = {
                    let boot_services = system_table.boot_services();
                    boot_entry(boot_services, image_handle, &entry)
                };

                // If we get here, boot failed
                {
                    let stdout = system_table.stdout();
                    let _ = stdout.set_color(Color::Red, Color::Black);
                    let _ = writeln!(stdout, "\nBoot failed: {:?}", result);
                    let _ = writeln!(stdout, "Press any key to return to menu...");
                }
                wait_for_key(system_table.boot_services());
            }
            MenuAction::Reboot => {
                let runtime_services = system_table.runtime_services();
                runtime_services.reset(uefi::table::runtime::ResetType::COLD, Status::SUCCESS, None);
            }
            MenuAction::Shutdown => {
                let runtime_services = system_table.runtime_services();
                runtime_services.reset(uefi::table::runtime::ResetType::SHUTDOWN, Status::SUCCESS, None);
            }
            MenuAction::UefiShell => {
                // Try to launch UEFI shell
                let result = {
                    let boot_services = system_table.boot_services();
                    launch_uefi_shell(boot_services, image_handle)
                };
                if result.is_err() {
                    let stdout = system_table.stdout();
                    let _ = stdout.set_color(Color::Red, Color::Black);
                    let _ = writeln!(stdout, "\nUEFI Shell not found.");
                    let _ = writeln!(stdout, "Press any key to return to menu...");
                    wait_for_key(system_table.boot_services());
                }
            }
            MenuAction::Continue => {
                // Navigation action, continue loop
            }
        }
    }
}

/// Result of menu interaction
enum MenuAction {
    Boot(BootEntry),
    Reboot,
    Shutdown,
    UefiShell,
    Continue,
}

/// Try to launch UEFI shell from common locations
fn launch_uefi_shell(boot_services: &BootServices, image_handle: Handle) -> Result<(), KernelError> {
    // Common UEFI shell locations
    let shell_paths = [
        "\\EFI\\BOOT\\Shell.efi",
        "\\EFI\\Shell.efi",
        "\\Shell.efi",
        "\\EFI\\tools\\Shell.efi",
        "\\shellx64.efi",
    ];

    for path in shell_paths {
        if let Ok(()) = chainload_efi(boot_services, image_handle, path) {
            return Ok(());
        }
    }

    Err(KernelError::EfiAppNotFound)
}

fn print_banner(stdout: &mut uefi::proto::console::text::Output) {
    let _ = stdout.set_color(Color::LightCyan, Color::Black);
    let _ = writeln!(stdout, "");
    let _ = stdout.set_color(Color::White, Color::Black);
    let _ = writeln!(stdout, r"    ██████╗  █████╗ ██╗   ██╗███████╗███╗   ██╗");
    let _ = writeln!(stdout, r"    ██╔══██╗██╔══██╗██║   ██║██╔════╝████╗  ██║");
    let _ = writeln!(stdout, r"    ██████╔╝███████║██║   ██║█████╗  ██╔██╗ ██║");
    let _ = writeln!(stdout, r"    ██╔══██╗██╔══██║╚██╗ ██╔╝██╔══╝  ██║╚██╗██║");
    let _ = writeln!(stdout, r"    ██║  ██║██║  ██║ ╚████╔╝ ███████╗██║ ╚████║");
    let _ = writeln!(stdout, r"    ╚═╝  ╚═╝╚═╝  ╚═╝  ╚═══╝  ╚══════╝╚═╝  ╚═══╝");
    let _ = stdout.set_color(Color::Cyan, Color::Black);
    let _ = writeln!(stdout, r"    ╔══════════════════════════════════════════════════════╗");
    let _ = writeln!(stdout, r"    ║              B O O T L O A D E R  v{}              ║", VERSION);
    let _ = writeln!(stdout, r"    ╚══════════════════════════════════════════════════════╝");
    let _ = writeln!(stdout, "");
}

fn display_menu_with_nav(
    system_table: &mut SystemTable<Boot>,
    nav: &mut MenuNav,
) -> MenuAction {
    let mut timeout = nav.config.timeout;
    let mut countdown_active = nav.depth == 0; // Only countdown on root menu

    loop {
        let entries = nav.current_entries();

        // Clear and redraw menu (scoped borrow of stdout)
        {
            let stdout = system_table.stdout();
            let _ = stdout.clear();
            print_banner(stdout);

            let _ = stdout.set_color(Color::LightGray, Color::Black);
            let _ = writeln!(stdout, "    {}", nav.menu_title());
            let _ = writeln!(stdout, "");

            // Menu width for full-line highlight
            const MENU_WIDTH: usize = 56;

            // Draw entries with full-line highlight
            for (i, entry) in entries.iter().enumerate() {
                if i == nav.selected {
                    // Selected entry - full line highlighted
                    let _ = stdout.set_color(Color::White, Color::Cyan);
                    let _ = write!(stdout, "    ");
                    let _ = write!(stdout, " >> ");
                    let _ = write!(stdout, "{}", entry.name);
                    // Pad to fill the line
                    let padding = MENU_WIDTH.saturating_sub(8 + entry.name.len());
                    for _ in 0..padding {
                        let _ = write!(stdout, " ");
                    }
                    let _ = writeln!(stdout, "");
                } else {
                    // Unselected entry
                    let _ = stdout.set_color(Color::LightGray, Color::Black);
                    let _ = writeln!(stdout, "        {}", entry.name);
                }
            }

            let _ = stdout.set_color(Color::DarkGray, Color::Black);
            let _ = writeln!(stdout, "");
            let _ = writeln!(stdout, "    ──────────────────────────────────────────────────────");

            if countdown_active && timeout > 0 {
                let _ = stdout.set_color(Color::Yellow, Color::Black);
                let _ = writeln!(stdout, "    Auto-boot in {} seconds...  Press any key to stop", timeout);
            } else {
                let _ = stdout.set_color(Color::LightGray, Color::Black);
                if nav.depth > 0 {
                    let _ = writeln!(stdout, "    [↑/↓] Select   [Enter] Select   [Esc/Backspace] Back");
                } else {
                    let _ = writeln!(stdout, "    [↑/↓] Select   [Enter] Boot/Enter   [Esc] Exit");
                }
            }
        }

        // Wait for input or timeout
        if countdown_active && timeout > 0 {
            let key_result = wait_for_key_timeout(system_table.boot_services(), 1_000_000);
            match key_result {
                Some(key) => {
                    countdown_active = false;
                    // Handle enter key during countdown
                    if let Key::Printable(c) = key {
                        if c == uefi::Char16::try_from('\r').unwrap() {
                            if let Some(entry) = nav.selected_entry() {
                                return handle_entry_selection(nav, entry.clone());
                            }
                        }
                    }
                    handle_nav_key(key, nav);
                }
                None => {
                    timeout -= 1;
                    if timeout == 0 {
                        // Auto-boot default entry
                        if let Some(entry) = nav.selected_entry() {
                            return handle_entry_selection(nav, entry.clone());
                        }
                    }
                }
            }
        } else {
            let key = wait_for_key(system_table.boot_services());
            match key {
                Key::Printable(c) if c == uefi::Char16::try_from('\r').unwrap() => {
                    if let Some(entry) = nav.selected_entry() {
                        return handle_entry_selection(nav, entry.clone());
                    }
                }
                Key::Special(ScanCode::ESCAPE) => {
                    if nav.depth > 0 {
                        nav.go_back();
                    }
                }
                Key::Printable(c) if c == uefi::Char16::try_from('\x08').unwrap() => {
                    // Backspace
                    if nav.depth > 0 {
                        nav.go_back();
                    }
                }
                _ => {
                    handle_nav_key(key, nav);
                }
            }
        }
    }
}

fn handle_entry_selection(nav: &mut MenuNav, entry: BootEntry) -> MenuAction {
    match entry.entry_type {
        EntryType::Submenu => {
            nav.enter_submenu();
            MenuAction::Continue
        }
        EntryType::Back => {
            nav.go_back();
            MenuAction::Continue
        }
        EntryType::Reboot => MenuAction::Reboot,
        EntryType::Shutdown => MenuAction::Shutdown,
        EntryType::UefiShell => MenuAction::UefiShell,
        _ => MenuAction::Boot(entry),
    }
}

fn handle_nav_key(key: Key, nav: &mut MenuNav) {
    match key {
        Key::Special(ScanCode::UP) => nav.move_up(),
        Key::Special(ScanCode::DOWN) => nav.move_down(),
        _ => {}
    }
}

fn handle_key(key: Key, current: usize, max: usize) -> usize {
    match key {
        Key::Special(ScanCode::UP) => {
            if current > 0 {
                current - 1
            } else {
                max.saturating_sub(1)
            }
        }
        Key::Special(ScanCode::DOWN) => {
            if current < max.saturating_sub(1) {
                current + 1
            } else {
                0
            }
        }
        _ => current,
    }
}

fn wait_for_key(boot_services: &BootServices) -> Key {
    loop {
        if let Some(key) = try_get_key(boot_services) {
            return key;
        }
        boot_services.stall(10_000); // 10ms
    }
}

fn wait_for_key_timeout(boot_services: &BootServices, timeout_us: u64) -> Option<Key> {
    let iterations = timeout_us / 10_000;
    for _ in 0..iterations {
        if let Some(key) = try_get_key(boot_services) {
            return Some(key);
        }
        boot_services.stall(10_000);
    }
    None
}

fn try_get_key(boot_services: &BootServices) -> Option<Key> {
    let stdin = boot_services.get_handle_for_protocol::<uefi::proto::console::text::Input>();
    if let Ok(handle) = stdin {
        if let Ok(mut input) =
            boot_services.open_protocol_exclusive::<uefi::proto::console::text::Input>(handle)
        {
            if let Ok(Some(key)) = input.read_key() {
                return Some(key);
            }
        }
    }
    None
}

fn load_config(boot_services: &BootServices, image_handle: Handle) -> Result<BootConfig, ()> {
    // Get device handle from our loaded image
    let loaded_image = boot_services
        .open_protocol_exclusive::<LoadedImage>(image_handle)
        .map_err(|_| ())?;

    let device_handle = loaded_image.device().ok_or(())?;

    // Open filesystem
    let mut fs = boot_services
        .open_protocol_exclusive::<SimpleFileSystem>(device_handle)
        .map_err(|_| ())?;

    let mut root = fs.open_volume().map_err(|_| ())?;

    // Try to load config from known paths, falling back to defaults.
    let mut config: BootConfig = BootConfig::default();
    for config_path in CONFIG_PATHS {
        if let Ok(config_data) = read_file_from_root(&mut root, config_path) {
            if let Ok(parsed) = BootConfig::parse(&config_data) {
                config = parsed;
                break;
            }
        }
    }

    // Try to detect other operating systems and add them to the config
    detect_other_os(&mut root, &mut config);

    Ok(config)
}

/// Try to read a file from the ESP root
fn read_file_from_root(
    root: &mut uefi::proto::media::file::Directory,
    path: &str,
) -> Result<alloc::vec::Vec<u8>, ()> {
    use alloc::vec::Vec;

    // Convert path to UCS-2
    let path_cstr = CString16::try_from(path).map_err(|_| ())?;

    // Try to open the file
    let file_handle = root
        .open(&path_cstr, FileMode::Read, FileAttribute::empty())
        .map_err(|_| ())?;

    let mut file = match file_handle.into_type().map_err(|_| ())? {
        uefi::proto::media::file::FileType::Regular(f) => f,
        _ => return Err(()),
    };

    // Get file size
    let mut info_buf = [0u8; 256];
    let info: &uefi::proto::media::file::FileInfo = file
        .get_info(&mut info_buf)
        .map_err(|_| ())?;

    let file_size = info.file_size() as usize;

    // Read file
    let mut buffer = Vec::with_capacity(file_size);
    buffer.resize(file_size, 0);

    file.read(&mut buffer).map_err(|_| ())?;

    Ok(buffer)
}

/// Check if a file exists on the ESP
fn file_exists(root: &mut uefi::proto::media::file::Directory, path: &str) -> bool {
    let path_cstr = match CString16::try_from(path) {
        Ok(p) => p,
        Err(_) => return false,
    };

    if let Ok(handle) = root.open(&path_cstr, FileMode::Read, FileAttribute::empty()) {
        // Close the file handle by dropping it
        drop(handle);
        true
    } else {
        false
    }
}

/// Detect other operating systems on the EFI System Partition
fn detect_other_os(root: &mut uefi::proto::media::file::Directory, config: &mut BootConfig) {
    // Check for known bootloaders
    for bootloader in KNOWN_BOOTLOADERS {
        if file_exists(root, bootloader.path) {
            // Found another OS, add it to the config
            if config.entries.len() < config::MAX_ENTRIES {
                config.entries.push(BootEntry {
                    name: bootloader.name.into(),
                    kernel: bootloader.path.into(),
                    initrd: None,
                    cmdline: String::new(),
                    entry_type: bootloader.entry_type,
                    children: Vec::new(),
                });
            }
        }
    }
}

fn boot_entry(
    boot_services: &BootServices,
    image_handle: Handle,
    entry: &BootEntry,
) -> Result<(), KernelError> {
    match entry.entry_type {
        EntryType::LinuxEfi => {
            // Boot Linux kernel with EFI stub
            boot_efi_stub(
                boot_services,
                image_handle,
                entry.kernel.as_str(),
                entry.initrd.as_deref(),
                entry.cmdline.as_str(),
            )
        }
        EntryType::Windows | EntryType::Chainload | EntryType::EfiApp => {
            // Chainload another EFI application
            chainload_efi(boot_services, image_handle, entry.kernel.as_str())
        }
        EntryType::LinuxLegacy => {
            // Traditional Linux boot - not implemented
            Err(KernelError::NotImplemented)
        }
        // These entry types are handled by the menu system, not boot_entry
        EntryType::Submenu | EntryType::Back | EntryType::Reboot |
        EntryType::Shutdown | EntryType::UefiShell => {
            Err(KernelError::NotImplemented)
        }
    }
}
