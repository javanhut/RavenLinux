//! RavenBoot - Custom UEFI Bootloader for RavenLinux
//!
//! A multi-boot capable bootloader that can coexist with other operating systems.
//! Supports booting Linux kernels directly via UEFI stub or traditional boot.

#![no_std]
#![no_main]

extern crate alloc;

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

use config::{BootConfig, BootEntry, EntryType, CONFIG_PATHS, KNOWN_BOOTLOADERS};
use linux::{boot_efi_stub, chainload_efi, KernelError};

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

    loop {
        // Display boot menu and get selection
        let selected = display_menu_with_table(&mut system_table, &config);

        // Get the selected entry
        let entry = &config.entries[selected];

        {
            let stdout = system_table.stdout();
            let _ = stdout.set_color(Color::White, Color::Black);
            let _ = writeln!(stdout, "\nBooting: {}", entry.name);
        }

        // Boot the selected entry
        let result = {
            let boot_services = system_table.boot_services();
            boot_entry(boot_services, image_handle, entry)
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
}

fn print_banner(stdout: &mut uefi::proto::console::text::Output) {
    let _ = writeln!(stdout, "");
    let _ = writeln!(stdout, "  +-------------------------------+");
    let _ = writeln!(stdout, "  |       R A V E N   B O O T     |");
    let _ = writeln!(stdout, "  |     RavenLinux Bootloader     |");
    let _ = writeln!(stdout, "  +-------------------------------+");
    let _ = writeln!(stdout, "              v{}", VERSION);
    let _ = writeln!(stdout, "");
}

fn display_menu_with_table(
    system_table: &mut SystemTable<Boot>,
    config: &BootConfig,
) -> usize {
    let mut selected: usize = config.default;
    let mut timeout = config.timeout;
    let mut countdown_active = true;

    loop {
        // Clear and redraw menu (scoped borrow of stdout)
        {
            let stdout = system_table.stdout();
            let _ = stdout.clear();
            print_banner(stdout);

            let _ = stdout.set_color(Color::White, Color::Black);
            let _ = writeln!(stdout, "  Select an operating system to boot:\n");

            // Draw entries
            for (i, entry) in config.entries.iter().take(config.entry_count).enumerate() {
                if i == selected {
                    let _ = stdout.set_color(Color::Black, Color::LightCyan);
                    let _ = writeln!(stdout, "  > {}  ", entry.name);
                } else {
                    let _ = stdout.set_color(Color::LightGray, Color::Black);
                    let _ = writeln!(stdout, "    {}  ", entry.name);
                }
            }

            let _ = stdout.set_color(Color::DarkGray, Color::Black);
            let _ = writeln!(stdout, "");
            let _ = writeln!(stdout, "  -----------------------------------");

            if countdown_active && timeout > 0 {
                let _ = writeln!(stdout, "  Auto-boot in {} seconds...", timeout);
            } else {
                let _ = writeln!(stdout, "  [Up/Down] Select  [Enter] Boot");
            }

            let _ = writeln!(stdout, "  [r] Reboot    [s] Shutdown");
        }

        // Wait for input or timeout (scoped borrow of boot_services)
        if countdown_active && timeout > 0 {
            let key_result = wait_for_key_timeout(system_table.boot_services(), 1_000_000);
            match key_result {
                Some(key) => {
                    countdown_active = false;
                    if let Key::Printable(c) = key {
                        if c == uefi::Char16::try_from('\r').unwrap() {
                            return selected;
                        }
                    }
                    selected = handle_key(key, selected, config.entry_count);
                }
                None => {
                    timeout -= 1;
                    if timeout == 0 {
                        return selected;
                    }
                }
            }
        } else {
            let key = wait_for_key(system_table.boot_services());
            match key {
                Key::Printable(c) if c == uefi::Char16::try_from('\r').unwrap() => {
                    return selected;
                }
                Key::Printable(c) if c == uefi::Char16::try_from('r').unwrap() || c == uefi::Char16::try_from('R').unwrap() => {
                    // Reboot - just loop for now
                }
                Key::Printable(c) if c == uefi::Char16::try_from('s').unwrap() || c == uefi::Char16::try_from('S').unwrap() => {
                    // Shutdown - just loop for now
                }
                _ => {
                    selected = handle_key(key, selected, config.entry_count);
                }
            }
        }
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

    // Try to load config from known paths
    for config_path in CONFIG_PATHS {
        if let Ok(config_data) = read_file_from_root(&mut root, config_path) {
            if let Ok(parsed) = BootConfig::parse(&config_data) {
                // Config loaded successfully - we have owned strings but need static refs
                // For now, fall back to defaults since we need &'static str in BootEntry
                // A full implementation would use a different structure for runtime
            }
        }
    }

    // Try to detect other operating systems and add them to the default config
    let mut config = BootConfig::default();
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
            if config.entry_count < config::MAX_ENTRIES {
                config.entries[config.entry_count] = BootEntry {
                    name: bootloader.name,
                    kernel: bootloader.path,
                    initrd: None,
                    cmdline: "",
                    entry_type: bootloader.entry_type,
                };
                config.entry_count += 1;
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
                entry.kernel,
                entry.initrd,
                entry.cmdline,
            )
        }
        EntryType::Windows | EntryType::Chainload | EntryType::EfiApp => {
            // Chainload another EFI application
            chainload_efi(boot_services, image_handle, entry.kernel)
        }
        EntryType::LinuxLegacy => {
            // Traditional Linux boot - not implemented
            Err(KernelError::NotImplemented)
        }
    }
}
