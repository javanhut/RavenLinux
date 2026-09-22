//! Boot configuration handling

use alloc::string::String;
use alloc::vec::Vec;
use core::str;
use uefi::Handle;

extern crate alloc;

/// Maximum number of boot entries
pub const MAX_ENTRIES: usize = 16;

/// Maximum submenu depth
pub const MAX_SUBMENU_DEPTH: usize = 3;

/// Most snapshot entries the "Snapshots >" submenu will show.
///
/// This is a separate budget from [`MAX_ENTRIES`] and that is the whole point
/// of putting snapshots behind a submenu rather than in the top-level list.
/// The top level is nearly full before a single snapshot exists --
/// `raven-install` writes seven fixed entries plus a Desktop one, and
/// `detect_other_os` appends whatever else it finds on the machine's disks --
/// and the parser here does not complain when it runs out of room, it just
/// stops pushing. A flat snapshot list would therefore quietly shorten the
/// fixed part of the menu from the bottom, and the entry at the bottom is the
/// rescue shell: the one thing `raven-install`'s own closing text tells people
/// to fall back to when a machine will not start. A submenu costs exactly one
/// top-level slot no matter how many snapshots are in it.
///
/// Twelve is roughly twice the default retention (`raven-snapshot` keeps five)
/// so that pinned snapshots have somewhere to accumulate, and still short
/// enough to fit on one screen of the menu.
pub const MAX_SNAPSHOT_ENTRIES: usize = 12;

/// Where the snapshot entries are read from, most 8.3-safe name first.
///
/// This is a second file and not more `[entry]` blocks in `boot.cfg` because
/// the two have different authors. `boot.cfg` is written once by
/// `raven-install` and describes the machine: which UUID the root is, which
/// container holds it, where to resume from. The snapshot list is rewritten by
/// `raven-snapshot` every time a snapshot is taken or pruned, which on a
/// machine that updates weekly is often. Keeping them apart means the tool
/// that regenerates the volatile half never has to read, edit and write back
/// the file that a machine's ability to boot at all depends on -- and that a
/// truncated write, a full ESP or a power cut in the middle of `rvn update`
/// costs the snapshot menu and nothing else.
///
/// A missing file is the normal case, not an error: an ext4 machine has no
/// snapshots and never will, and no part of this may treat that as a fault.
pub const SNAPSHOT_CONFIG_PATHS: &[&str] =
    &["\\EFI\\raven\\snaps.cfg", "\\EFI\\raven\\snapshots.cfg"];

/// The label of the generated submenu.
pub const SNAPSHOT_MENU_NAME: &str = "Snapshots >";

/// A single boot entry
#[derive(Clone)]
pub struct BootEntry {
    pub name: String,
    pub kernel: String,
    pub initrd: Option<String>,
    pub cmdline: String,
    pub entry_type: EntryType,
    /// Which filesystem `kernel` is a path on.
    ///
    /// `None` means the volume RavenBoot was itself loaded from. Every entry
    /// boot.cfg can describe is one of those -- boot.cfg lives on that volume
    /// and its paths are relative to it -- so the parser never sets this.
    ///
    /// `Some(h)` comes from the scan for other operating systems, which looks
    /// at every filesystem the firmware can see rather than only at ours. A
    /// Windows on the machine's second disk has its own ESP, and booting it
    /// means reading from that ESP and handing the firmware a device path
    /// that points into it: Windows Boot Manager finds its BCD store next to
    /// itself, so a loader started against the wrong volume does not boot.
    pub volume: Option<Handle>,
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
    /// Ask firmware to open its setup interface after reboot
    FirmwareSetup,
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
            volume: None,
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
            volume: None,
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
            volume: None,
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
            volume: None,
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
            volume: None,
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
            volume: None,
            children: Vec::new(),
        }
    }

    /// Create a firmware settings entry.
    pub fn firmware_setup() -> Self {
        Self {
            name: String::from("System UEFI Settings"),
            kernel: String::new(),
            initrd: None,
            cmdline: String::new(),
            entry_type: EntryType::FirmwareSetup,
            volume: None,
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
            cmdline: String::from(
                "rdinit=/init quiet loglevel=3 console=ttyS0,115200 console=tty0",
            ),
            entry_type: EntryType::LinuxEfi,
            volume: None,
            children: Vec::new(),
        });
        let mut serial_entries: Vec<BootEntry> = Vec::new();

        // Serial-first entries for headless debugging (make ttyS0 the primary console)
        serial_entries.push(BootEntry {
            name: String::from("Raven Linux (Serial Console)"),
            kernel: String::from("\\EFI\\raven\\vmlinuz"),
            initrd: Some(String::from("\\EFI\\raven\\initrd.img")),
            cmdline: String::from(
                "rdinit=/init quiet loglevel=3 raven.console=serial console=tty0 console=ttyS0,115200",
            ),
            entry_type: EntryType::LinuxEfi,
            volume: None,
            children: Vec::new(),
        });

        serial_entries.push(BootEntry {
            name: String::from("Raven Linux (Serial Console, Verbose)"),
            kernel: String::from("\\EFI\\raven\\vmlinuz"),
            initrd: Some(String::from("\\EFI\\raven\\initrd.img")),
            cmdline: String::from(
                "rdinit=/init loglevel=7 raven.console=serial console=tty0 console=ttyS0,115200",
            ),
            entry_type: EntryType::LinuxEfi,
            volume: None,
            children: Vec::new(),
        });
        serial_entries.push(BootEntry::back());

        entries.push(BootEntry::submenu("Raven Linux (Serial) >", serial_entries));
        // Graphical submenu
        let mut graphical_entries: Vec<BootEntry> = Vec::new();

        graphical_entries.push(BootEntry {
            name: String::from("Raven Desktop (Huginn)"),
            kernel: String::from("\\EFI\\raven\\vmlinuz"),
            initrd: Some(String::from("\\EFI\\raven\\initrd.img")),
            // raven.wayland names the compositor binary. raven-init passes it
            // to /bin/raven-wayland-session as RAVEN_WAYLAND_COMPOSITOR; the
            // launcher still accepts the old "raven" and maps it to huginn.
            //
            // raven.user=root: the live desktop runs as root. Without it
            // raven-init picks the lowest-uid regular account -- the `raven`
            // placeholder, which exists so the image has a non-root user and
            // not because anyone is meant to be it -- and the desktop's job
            // here is installing the machine, which is root's work.
            //
            // These defaults are only ever read when the ESP carries no
            // boot.cfg, and raven-install always writes one. So this is the
            // live ISO's menu and nothing else: an installed disk boots the
            // entries that installer generated, which carry no raven.user.
            cmdline: String::from("rdinit=/init quiet loglevel=3 raven.graphics=wayland raven.wayland=huginn raven.user=root console=ttyS0,115200 console=tty0"),
            entry_type: EntryType::LinuxEfi,
            volume: None,
            children: Vec::new(),
        });

        graphical_entries.push(BootEntry {
            name: String::from("X11"),
            kernel: String::from("\\EFI\\raven\\vmlinuz"),
            initrd: Some(String::from("\\EFI\\raven\\initrd.img")),
            cmdline: String::from("rdinit=/init quiet loglevel=3 raven.graphics=x11 console=ttyS0,115200 console=tty0"),
            entry_type: EntryType::LinuxEfi,
            volume: None,
            children: Vec::new(),
        });

        graphical_entries.push(BootEntry::back());

        entries.push(BootEntry::submenu(
            "Raven Linux (Graphical) >",
            graphical_entries,
        ));

        let mut recovery_entries: Vec<BootEntry> = Vec::new();
        // Recovery mode
        recovery_entries.push(BootEntry {
            name: String::from("Raven Linux (Recovery)"),
            kernel: String::from("\\EFI\\raven\\vmlinuz"),
            initrd: Some(String::from("\\EFI\\raven\\initrd.img")),
            cmdline: String::from("rdinit=/init single console=ttyS0,115200 console=tty0"),
            entry_type: EntryType::LinuxEfi,
            volume: None,
            children: Vec::new(),
        });

        recovery_entries.push(BootEntry {
            name: String::from("Raven Linux (Recovery, Serial Console)"),
            kernel: String::from("\\EFI\\raven\\vmlinuz"),
            initrd: Some(String::from("\\EFI\\raven\\initrd.img")),
            cmdline: String::from(
                "rdinit=/init single raven.console=serial console=tty0 console=ttyS0,115200",
            ),
            entry_type: EntryType::LinuxEfi,
            volume: None,
            children: Vec::new(),
        });
        recovery_entries.push(BootEntry::back());
        entries.push(BootEntry::submenu("Recovery >", recovery_entries));
        // System submenu
        let mut system_entries: Vec<BootEntry> = Vec::new();
        system_entries.push(BootEntry::firmware_setup());
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
                    volume: None,
                    children: Vec::new(),
                });
                continue;
            }

            // Parse key = value pairs
            if let Some((key, value)) = parse_key_value(line) {
                if let Some(ref mut entry) = current_entry {
                    // Entry-level settings
                    apply_entry_key(entry, key, value);
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

/// Apply one `key = value` line to the entry being built.
///
/// Lifted out of [`BootConfig::parse`] unchanged so that the snapshot list
/// below reads entries with exactly the same rules -- one spelling of `type`,
/// one set of aliases, one catch-all. A second copy of this match would be a
/// second dialect of boot.cfg, and the day they drifted apart would be the day
/// an entry booted differently depending on which file it was written in.
fn apply_entry_key(entry: &mut BootEntry, key: &str, value: String) {
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
                // The enum and the dispatch in main.rs have
                // always had this; the parser had no spelling
                // for it, so `type = firmware-setup` fell
                // through the catch-all below and became a
                // Linux entry that tried to boot the literal
                // string in `kernel`.
                "firmware-setup" | "uefi-setup" | "firmware" => EntryType::FirmwareSetup,
                _ => EntryType::LinuxEfi,
            };
        }
        _ => {}
    }
}

/// Parse the generated snapshot list into a single submenu entry.
///
/// The file is the same flat `[entry]` dialect `boot.cfg` uses, and
/// deliberately so: `raven-snapshot` builds each entry by copying the machine's
/// own boot entry and changing `rootflags=subvol=` to name a snapshot, so what
/// it writes is a boot.cfg entry in every respect but which subvolume it
/// mounts. Nothing new had to be invented here, and nothing new can go wrong
/// here that could not already go wrong in boot.cfg.
///
/// What this adds is the wrapper. The file has no syntax for a submenu -- the
/// parser above has none either, and giving it one would mean nesting, an end
/// marker and a depth check in the one piece of code whose failure mode is a
/// machine that does not start. Instead the *meaning* of this particular file
/// is the submenu: everything in it is a snapshot, so everything in it goes one
/// level down, under [`SNAPSHOT_MENU_NAME`], with a `< Back` at the bottom like
/// every other submenu in the menu.
///
/// Returns `None` when there is nothing to show -- no file, an unreadable one,
/// no usable entries. Every one of those is the ordinary state of an ext4
/// machine, a machine whose ESP was never mounted when the snapshot was taken,
/// or a machine that has simply never taken one, and none of them is worth a
/// warning on a screen somebody is waiting five seconds to get past.
pub fn parse_snapshot_menu(data: &[u8]) -> Option<BootEntry> {
    let text = str::from_utf8(data).ok()?;

    let mut children: Vec<BootEntry> = Vec::new();
    let mut current_entry: Option<BootEntry> = None;

    // Pushes the finished entry if it is complete and there is still room.
    // A snapshot entry with no name or no kernel is dropped for the same
    // reason boot.cfg drops one: there is nothing to draw and nothing to boot.
    fn flush(current: &mut Option<BootEntry>, children: &mut Vec<BootEntry>) {
        if let Some(entry) = current.take() {
            if !entry.name.is_empty()
                && !entry.kernel.is_empty()
                && children.len() < MAX_SNAPSHOT_ENTRIES
            {
                children.push(entry);
            }
        }
    }

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if line == "[entry]" {
            flush(&mut current_entry, &mut children);
            current_entry = Some(BootEntry::default());
            continue;
        }

        // There are no global settings in this file. `timeout` and `default`
        // belong to the menu as a whole, which this is not part of: a snapshot
        // is never the default and a submenu never counts down.
        if let Some((key, value)) = parse_key_value(line) {
            if let Some(ref mut entry) = current_entry {
                apply_entry_key(entry, key, value);
            }
        }
    }
    flush(&mut current_entry, &mut children);

    if children.is_empty() {
        return None;
    }

    children.push(BootEntry::back());
    Some(BootEntry::submenu(SNAPSHOT_MENU_NAME, children))
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
