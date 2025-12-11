//! Boot configuration handling

use alloc::string::String;
use alloc::vec::Vec;
use core::str;

extern crate alloc;

/// Maximum number of boot entries
pub const MAX_ENTRIES: usize = 16;

/// A single boot entry
#[derive(Clone)]
pub struct BootEntry {
    pub name: &'static str,
    pub kernel: &'static str,
    pub initrd: Option<&'static str>,
    pub cmdline: &'static str,
    pub entry_type: EntryType,
}

/// Boot entry with owned strings (for parsed config)
#[derive(Clone)]
pub struct OwnedBootEntry {
    pub name: String,
    pub kernel: String,
    pub initrd: Option<String>,
    pub cmdline: String,
    pub entry_type: EntryType,
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
}

/// Boot configuration
pub struct BootConfig {
    pub entries: [BootEntry; MAX_ENTRIES],
    pub entry_count: usize,
    pub default: usize,
    pub timeout: u32,
}

impl Default for BootEntry {
    fn default() -> Self {
        Self {
            name: "",
            kernel: "",
            initrd: None,
            cmdline: "",
            entry_type: EntryType::LinuxEfi,
        }
    }
}

impl Default for BootConfig {
    fn default() -> Self {
        // Create default boot configuration
        let mut entries: [BootEntry; MAX_ENTRIES] = core::array::from_fn(|_| BootEntry::default());

        // Default RavenLinux entry
        entries[0] = BootEntry {
            name: "RavenLinux",
            kernel: "\\EFI\\raven\\vmlinuz",
            initrd: Some("\\EFI\\raven\\initramfs.img"),
            cmdline: "root=LABEL=RAVEN_ROOT rw quiet splash",
            entry_type: EntryType::LinuxEfi,
        };

        // Recovery mode
        entries[1] = BootEntry {
            name: "RavenLinux (Recovery)",
            kernel: "\\EFI\\raven\\vmlinuz",
            initrd: Some("\\EFI\\raven\\initramfs.img"),
            cmdline: "root=LABEL=RAVEN_ROOT rw single",
            entry_type: EntryType::LinuxEfi,
        };

        // Auto-detect other operating systems would go here
        // In a full implementation, we'd scan for:
        // - \\EFI\\Microsoft\\Boot\\bootmgfw.efi (Windows)
        // - \\EFI\\*\\grubx64.efi (Other Linux)
        // - \\EFI\\*\\shimx64.efi (Secure Boot Linux)

        Self {
            entries,
            entry_count: 2,
            default: 0,
            timeout: 5,
        }
    }
}

/// Parsed boot configuration with owned strings
pub struct ParsedBootConfig {
    pub entries: Vec<OwnedBootEntry>,
    pub default: usize,
    pub timeout: u32,
}

impl BootConfig {
    /// Parse configuration from file contents
    pub fn parse(data: &[u8]) -> Result<ParsedBootConfig, ()> {
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

        let mut entries: Vec<OwnedBootEntry> = Vec::new();
        let mut default: usize = 0;
        let mut timeout: u32 = 5;

        // Current entry being parsed
        let mut current_entry: Option<OwnedBootEntry> = None;

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
                    if !entry.name.is_empty() && !entry.kernel.is_empty() {
                        entries.push(entry);
                    }
                }
                // Start new entry
                current_entry = Some(OwnedBootEntry {
                    name: String::new(),
                    kernel: String::new(),
                    initrd: None,
                    cmdline: String::new(),
                    entry_type: EntryType::LinuxEfi,
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
            if !entry.name.is_empty() && !entry.kernel.is_empty() {
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

        Ok(ParsedBootConfig {
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
    "\\EFI\\raven\\boot.conf",
    "\\EFI\\BOOT\\raven.conf",
    "\\raven\\boot.conf",
];
