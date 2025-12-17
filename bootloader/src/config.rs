//! Boot configuration handling

use alloc::string::String;
use alloc::vec::Vec;
use core::str;

extern crate alloc;

/// Maximum number of boot entries
pub const MAX_ENTRIES: usize = 16;

/// Maximum submenu depth
pub const MAX_SUBMENU_DEPTH: usize = 3;

/// A single boot entry
#[derive(Clone)]
pub struct BootEntry {
    pub name: String,
    pub kernel: String,
    pub initrd: Option<String>,
    pub cmdline: String,
    pub entry_type: EntryType,
    /// Child entries for submenu type
    pub children: Vec<BootEntry>,
}

/// Type of boot entry
#[derive(Clone, Copy, PartialEq)]
pub enum EntryType {
    /// Linux kernel with EFI stub
    LinuxEfi,
    /// Traditional Linux boot (load kernel + initrd manually)
    LinuxLegacy,
    /// Another EFI application (chainload)
    EfiApp,
    /// Windows Boot Manager
    Windows,
    /// Other OS via chainload
    Chainload,
    /// Submenu containing other entries
    Submenu,
    /// UEFI Shell
    UefiShell,
    /// Back to parent menu
    Back,
    /// Reboot system
    Reboot,
    /// Shutdown system
    Shutdown,
}

/// Boot configuration
pub struct BootConfig {
    pub entries: Vec<BootEntry>,
    pub default: usize,
    pub timeout: u32,
}

impl Default for BootEntry {
    fn default() -> Self {
        Self {
            name: String::new(),
            kernel: String::new(),
            initrd: None,
            cmdline: String::new(),
            entry_type: EntryType::LinuxEfi,
            children: Vec::new(),
        }
    }
}

impl BootEntry {
    /// Create a new submenu entry
    pub fn submenu(name: &str, children: Vec<BootEntry>) -> Self {
        Self {
            name: String::from(name),
            kernel: String::new(),
            initrd: None,
            cmdline: String::new(),
            entry_type: EntryType::Submenu,
            children,
        }
    }

    /// Create a back entry
    pub fn back() -> Self {
        Self {
            name: String::from("< Back"),
            kernel: String::new(),
            initrd: None,
            cmdline: String::new(),
            entry_type: EntryType::Back,
            children: Vec::new(),
        }
    }

    /// Create a reboot entry
    pub fn reboot() -> Self {
        Self {
            name: String::from("Reboot"),
            kernel: String::new(),
            initrd: None,
            cmdline: String::new(),
            entry_type: EntryType::Reboot,
            children: Vec::new(),
        }
    }

    /// Create a shutdown entry
    pub fn shutdown() -> Self {
        Self {
            name: String::from("Shutdown"),
            kernel: String::new(),
            initrd: None,
            cmdline: String::new(),
            entry_type: EntryType::Shutdown,
            children: Vec::new(),
        }
    }

    /// Create a UEFI shell entry
    pub fn uefi_shell() -> Self {
        Self {
            name: String::from("UEFI Shell"),
            kernel: String::new(),
            initrd: None,
            cmdline: String::new(),
            entry_type: EntryType::UefiShell,
            children: Vec::new(),
        }
    }
}

impl Default for BootConfig {
    fn default() -> Self {
        // Create default boot configuration with submenus.
        // Note: On the live ISO we expect a boot.cfg/boot.conf to override this.
        let mut entries: Vec<BootEntry> = Vec::new();

        // Default RavenLinux entry (terminal mode)
        entries.push(BootEntry {
            name: String::from("Raven Linux"),
            kernel: String::from("\\EFI\\raven\\vmlinuz"),
            initrd: Some(String::from("\\EFI\\raven\\initrd.img")),
            cmdline: String::from("rdinit=/init quiet loglevel=3 console=ttyS0,115200 console=tty0"),
            entry_type: EntryType::LinuxEfi,
            children: Vec::new(),
        });

        // Serial-first entries for headless debugging (make ttyS0 the primary console)
        entries.push(BootEntry {
            name: String::from("Raven Linux (Serial Console)"),
            kernel: String::from("\\EFI\\raven\\vmlinuz"),
            initrd: Some(String::from("\\EFI\\raven\\initrd.img")),
            cmdline: String::from(
                "rdinit=/init quiet loglevel=3 raven.console=serial console=tty0 console=ttyS0,115200",
            ),
            entry_type: EntryType::LinuxEfi,
            children: Vec::new(),
        });

        entries.push(BootEntry {
            name: String::from("Raven Linux (Serial Console, Verbose)"),
            kernel: String::from("\\EFI\\raven\\vmlinuz"),
            initrd: Some(String::from("\\EFI\\raven\\initrd.img")),
            cmdline: String::from(
                "rdinit=/init loglevel=7 raven.console=serial console=tty0 console=ttyS0,115200",
            ),
            entry_type: EntryType::LinuxEfi,
            children: Vec::new(),
        });

        // Graphical submenu
        let mut graphical_entries: Vec<BootEntry> = Vec::new();

        graphical_entries.push(BootEntry {
            name: String::from("Raven Desktop (Wayland)"),
            kernel: String::from("\\EFI\\raven\\vmlinuz"),
            initrd: Some(String::from("\\EFI\\raven\\initrd.img")),
            cmdline: String::from("rdinit=/init quiet loglevel=3 raven.graphics=wayland raven.wayland=weston console=ttyS0,115200 console=tty0"),
            entry_type: EntryType::LinuxEfi,
            children: Vec::new(),
        });

        graphical_entries.push(BootEntry {
            name: String::from("Raven Compositor (Wayland)"),
            kernel: String::from("\\EFI\\raven\\vmlinuz"),
            initrd: Some(String::from("\\EFI\\raven\\initrd.img")),
            cmdline: String::from("rdinit=/init quiet loglevel=3 raven.graphics=wayland raven.wayland=raven console=ttyS0,115200 console=tty0"),
            entry_type: EntryType::LinuxEfi,
            children: Vec::new(),
        });

        graphical_entries.push(BootEntry {
            name: String::from("Hyprland (Wayland)"),
            kernel: String::from("\\EFI\\raven\\vmlinuz"),
            initrd: Some(String::from("\\EFI\\raven\\initrd.img")),
            cmdline: String::from("rdinit=/init quiet loglevel=3 raven.graphics=wayland raven.wayland=hyprland console=ttyS0,115200 console=tty0"),
            entry_type: EntryType::LinuxEfi,
            children: Vec::new(),
        });

        graphical_entries.push(BootEntry {
            name: String::from("Weston (Wayland)"),
            kernel: String::from("\\EFI\\raven\\vmlinuz"),
            initrd: Some(String::from("\\EFI\\raven\\initrd.img")),
            cmdline: String::from("rdinit=/init quiet loglevel=3 raven.graphics=wayland raven.wayland=weston console=ttyS0,115200 console=tty0"),
            entry_type: EntryType::LinuxEfi,
            children: Vec::new(),
        });

        graphical_entries.push(BootEntry {
            name: String::from("X11"),
            kernel: String::from("\\EFI\\raven\\vmlinuz"),
            initrd: Some(String::from("\\EFI\\raven\\initrd.img")),
            cmdline: String::from("rdinit=/init quiet loglevel=3 raven.graphics=x11 console=ttyS0,115200 console=tty0"),
            entry_type: EntryType::LinuxEfi,
            children: Vec::new(),
        });

        graphical_entries.push(BootEntry::back());

        entries.push(BootEntry::submenu("Raven Linux (Graphical) >", graphical_entries));

        // Recovery mode
        entries.push(BootEntry {
            name: String::from("Raven Linux (Recovery)"),
            kernel: String::from("\\EFI\\raven\\vmlinuz"),
            initrd: Some(String::from("\\EFI\\raven\\initrd.img")),
            cmdline: String::from("rdinit=/init single console=ttyS0,115200 console=tty0"),
            entry_type: EntryType::LinuxEfi,
            children: Vec::new(),
        });

        entries.push(BootEntry {
            name: String::from("Raven Linux (Recovery, Serial Console)"),
            kernel: String::from("\\EFI\\raven\\vmlinuz"),
            initrd: Some(String::from("\\EFI\\raven\\initrd.img")),
            cmdline: String::from(
                "rdinit=/init single raven.console=serial console=tty0 console=ttyS0,115200",
            ),
            entry_type: EntryType::LinuxEfi,
            children: Vec::new(),
        });

        // System submenu
        let mut system_entries: Vec<BootEntry> = Vec::new();
        system_entries.push(BootEntry::uefi_shell());
        system_entries.push(BootEntry::reboot());
        system_entries.push(BootEntry::shutdown());
        system_entries.push(BootEntry::back());

        entries.push(BootEntry::submenu("System >", system_entries));

        Self {
            entries,
            default: 0,
            timeout: 5,
        }
    }
}

impl BootConfig {
    /// Parse configuration from file contents
    pub fn parse(data: &[u8]) -> Result<BootConfig, ()> {
        // Configuration file format (boot.conf):
        //
        // timeout = 5
        // default = 0
        //
        // [entry]
        // name = "RavenLinux"
        // kernel = "\EFI\raven\vmlinuz"
        // initrd = "\EFI\raven\initramfs.img"
        // cmdline = "root=LABEL=RAVEN_ROOT rw quiet"
        // type = linux-efi
        //
        // [entry]
        // name = "Windows"
        // path = "\EFI\Microsoft\Boot\bootmgfw.efi"
        // type = chainload

        let text = str::from_utf8(data).map_err(|_| ())?;

        let mut entries: Vec<BootEntry> = Vec::new();
        let mut default: usize = 0;
        let mut timeout: u32 = 5;

        // Current entry being parsed
        let mut current_entry: Option<BootEntry> = None;

        for line in text.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            // Check for section header
            if line == "[entry]" {
                // Save previous entry if exists
                if let Some(entry) = current_entry.take() {
                    if !entry.name.is_empty()
                        && !entry.kernel.is_empty()
                        && entries.len() < MAX_ENTRIES
                    {
                        entries.push(entry);
                    }
                }
                // Start new entry
                current_entry = Some(BootEntry {
                    name: String::new(),
                    kernel: String::new(),
                    initrd: None,
                    cmdline: String::new(),
                    entry_type: EntryType::LinuxEfi,
                    children: Vec::new(),
                });
                continue;
            }

            // Parse key = value pairs
            if let Some((key, value)) = parse_key_value(line) {
                if let Some(ref mut entry) = current_entry {
                    // Entry-level settings
                    match key {
                        "name" => entry.name = value,
                        "kernel" | "path" => entry.kernel = value,
                        "initrd" => entry.initrd = Some(value),
                        "cmdline" | "options" => entry.cmdline = value,
                        "type" => {
                            entry.entry_type = match value.as_str() {
                                "linux-efi" | "linux" => EntryType::LinuxEfi,
                                "linux-legacy" => EntryType::LinuxLegacy,
                                "chainload" | "efi" => EntryType::Chainload,
                                "windows" => EntryType::Windows,
                                "submenu" => EntryType::Submenu,
                                "uefi-shell" | "shell" => EntryType::UefiShell,
                                "back" => EntryType::Back,
                                "reboot" => EntryType::Reboot,
                                "shutdown" | "poweroff" => EntryType::Shutdown,
                                _ => EntryType::LinuxEfi,
                            };
                        }
                        _ => {}
                    }
                } else {
                    // Global settings
                    match key {
                        "timeout" => {
                            if let Ok(t) = value.parse() {
                                timeout = t;
                            }
                        }
                        "default" => {
                            if let Ok(d) = value.parse() {
                                default = d;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Save final entry
        if let Some(entry) = current_entry {
            if !entry.name.is_empty() && !entry.kernel.is_empty() && entries.len() < MAX_ENTRIES {
                entries.push(entry);
            }
        }

        // If no entries parsed, return error
        if entries.is_empty() {
            return Err(());
        }

        // Ensure default is valid
        if default >= entries.len() {
            default = 0;
        }

        Ok(BootConfig {
            entries,
            default,
            timeout,
        })
    }
}

/// Parse a key = value line, handling quoted values
fn parse_key_value(line: &str) -> Option<(&str, String)> {
    let mut parts = line.splitn(2, '=');
    let key = parts.next()?.trim();
    let value = parts.next()?.trim();

    // Remove quotes if present
    let value = if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        &value[1..value.len() - 1]
    } else {
        value
    };

    Some((key, String::from(value)))
}

/// Known bootloader locations for auto-detection
pub struct KnownBootloader {
    pub path: &'static str,
    pub name: &'static str,
    pub entry_type: EntryType,
}

/// List of known bootloaders to scan for
pub const KNOWN_BOOTLOADERS: &[KnownBootloader] = &[
    KnownBootloader {
        path: "\\EFI\\Microsoft\\Boot\\bootmgfw.efi",
        name: "Windows",
        entry_type: EntryType::Windows,
    },
    KnownBootloader {
        path: "\\EFI\\ubuntu\\shimx64.efi",
        name: "Ubuntu",
        entry_type: EntryType::Chainload,
    },
    KnownBootloader {
        path: "\\EFI\\ubuntu\\grubx64.efi",
        name: "Ubuntu",
        entry_type: EntryType::Chainload,
    },
    KnownBootloader {
        path: "\\EFI\\fedora\\shimx64.efi",
        name: "Fedora",
        entry_type: EntryType::Chainload,
    },
    KnownBootloader {
        path: "\\EFI\\fedora\\grubx64.efi",
        name: "Fedora",
        entry_type: EntryType::Chainload,
    },
    KnownBootloader {
        path: "\\EFI\\debian\\shimx64.efi",
        name: "Debian",
        entry_type: EntryType::Chainload,
    },
    KnownBootloader {
        path: "\\EFI\\debian\\grubx64.efi",
        name: "Debian",
        entry_type: EntryType::Chainload,
    },
    KnownBootloader {
        path: "\\EFI\\arch\\grubx64.efi",
        name: "Arch Linux",
        entry_type: EntryType::Chainload,
    },
    KnownBootloader {
        path: "\\EFI\\manjaro\\grubx64.efi",
        name: "Manjaro",
        entry_type: EntryType::Chainload,
    },
    KnownBootloader {
        path: "\\EFI\\opensuse\\grubx64.efi",
        name: "openSUSE",
        entry_type: EntryType::Chainload,
    },
    KnownBootloader {
        path: "\\EFI\\centos\\shimx64.efi",
        name: "CentOS",
        entry_type: EntryType::Chainload,
    },
    KnownBootloader {
        path: "\\EFI\\rocky\\shimx64.efi",
        name: "Rocky Linux",
        entry_type: EntryType::Chainload,
    },
    KnownBootloader {
        path: "\\EFI\\linuxmint\\grubx64.efi",
        name: "Linux Mint",
        entry_type: EntryType::Chainload,
    },
    KnownBootloader {
        path: "\\EFI\\pop\\grubx64.efi",
        name: "Pop!_OS",
        entry_type: EntryType::Chainload,
    },
];

/// Configuration file paths to try
pub const CONFIG_PATHS: &[&str] = &[
    // Prefer 8.3-safe names first (some firmware FAT drivers don't support LFN/VFAT).
    "\\EFI\\raven\\boot.cfg",
    "\\EFI\\raven\\boot.conf",
    "\\EFI\\BOOT\\raven.cfg",
    "\\EFI\\BOOT\\raven.conf",
    "\\raven\\boot.cfg",
    "\\raven\\boot.conf",
];
