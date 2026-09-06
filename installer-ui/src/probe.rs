//! Reading `raven-install --probe`.
//!
//! Every fact this program shows about the machine comes from here, and every
//! one of them was produced by the installer's own preflight and source
//! lookup rather than by a second implementation of the same checks. That is
//! the point: when raven-install learns to refuse a machine, this refuses it
//! too, without being edited.

use std::collections::HashMap;
use std::process::Command;

/// One partition on a candidate disk, as `lsblk` reported it.
#[derive(Debug, Clone, Default)]
pub struct Partition {
    pub name: String,
    pub size: String,
    pub fstype: String,
    pub label: String,
}

/// A disk the installer could be pointed at.
#[derive(Debug, Clone, Default)]
pub struct Disk {
    pub dev: String,
    pub size: String,
    pub bytes: u64,
    pub model: String,
    pub removable: bool,
    /// The disk this machine booted from. Never installable, because the copy
    /// would be reading from a device it is erasing.
    pub live_media: bool,
    /// Something on it looks like a Windows installation.
    pub windows: bool,
    pub parts: Vec<Partition>,
}

impl Disk {
    /// What the disk-choosing page puts under the device name.
    pub fn subtitle(&self) -> String {
        let mut s = self.size.clone();
        if !self.model.is_empty() {
            s.push_str(", ");
            s.push_str(&self.model);
        }
        if self.removable {
            s.push_str(" (removable)");
        }
        s
    }

    pub fn holds_summary(&self) -> Option<String> {
        if self.parts.is_empty() {
            return None;
        }
        let rows: Vec<String> = self
            .parts
            .iter()
            .map(|p| {
                let mut r = format!("{}  {}", p.name, p.size);
                if !p.fstype.is_empty() {
                    r.push_str(&format!("  {}", p.fstype));
                }
                if !p.label.is_empty() {
                    r.push_str(&format!("  “{}”", p.label));
                }
                r
            })
            .collect();
        Some(rows.join("\n"))
    }
}

#[derive(Debug, Clone, Default)]
pub struct Probe {
    pub protocol: String,
    pub installer_version: String,
    pub euid: u32,
    /// False when the installer would refuse this machine. `errors` says why.
    pub ok: bool,
    pub errors: Vec<String>,

    pub firmware: String,
    pub secureboot: String,
    pub tools_missing: Vec<String>,
    pub filesystems: Vec<String>,
    pub chpasswd: bool,
    pub mkswap: bool,
    pub efibootmgr: bool,

    pub source_kind: String,
    pub source_root: String,
    pub source_kernel: String,
    pub source_initrd: String,
    pub source_efi: String,
    pub source_size_mb: u64,
    pub initrd_root_support: String,
    pub has_desktop: bool,

    pub mem_total_mb: u64,
    pub swap_suggested: String,
    pub esp_size_default: String,

    pub disks: Vec<Disk>,
    pub timezones: Vec<String>,
    pub profiles: Vec<String>,
    pub answer_keys: Vec<String>,
    pub answers_required: Vec<String>,
}

/// The protocol version this program was written against. A newer installer
/// is refused rather than guessed at: the records it emits are the ones that
/// decide which disk gets erased.
pub const SUPPORTED_PROTOCOL: &str = "1";

impl Probe {
    /// Warnings worth showing on the welcome page. Not blockers -- the install
    /// proceeds -- but each one is a thing the person will otherwise discover
    /// at the next boot, which is the worst time to discover it.
    pub fn warnings(&self) -> Vec<String> {
        let mut w = Vec::new();
        if self.secureboot == "on" {
            w.push(
                "Secure Boot is enabled. RavenBoot is unsigned, so the installed \
                 system will not boot until you turn Secure Boot off in the \
                 firmware setup."
                    .to_string(),
            );
        }
        if self.initrd_root_support == "no" {
            w.push(
                "This initramfs was built before it could boot from a disk. It \
                 will look for the live squashfs instead of the new root \
                 partition. Rebuild with 'imlazy initramfs && imlazy iso' if the \
                 install does not boot."
                    .to_string(),
            );
        }
        if !self.chpasswd {
            w.push(
                "chpasswd is missing from this image, so no password can be set \
                 during the install. You will have to set one from the rescue \
                 boot entry."
                    .to_string(),
            );
        }
        w
    }

    /// The disks a person may actually pick.
    pub fn installable_disks(&self) -> Vec<&Disk> {
        self.disks.iter().filter(|d| !d.live_media).collect()
    }

    /// Is this disk big enough for the tree that would be copied onto it, plus
    /// the ESP and whatever swap was asked for? Answered in MB against the
    /// same `source_size_mb` the installer measures its progress with.
    pub fn disk_too_small(&self, disk: &Disk, swap_mb: u64, esp_mb: u64) -> bool {
        if disk.bytes == 0 || self.source_size_mb == 0 {
            return false;
        }
        let have_mb = disk.bytes / (1024 * 1024);
        // A tenth over the copied size, so the installed system has somewhere
        // to put a log before it runs out.
        let need_mb = self.source_size_mb + self.source_size_mb / 10 + swap_mb + esp_mb;
        have_mb < need_mb
    }
}

/// Parse `<n>G`, `<n>M` or a bare number of megabytes. Used for the two size
/// fields a person can type into.
pub fn size_to_mb(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (num, mult) = match s.chars().last().unwrap().to_ascii_uppercase() {
        'G' => (&s[..s.len() - 1], 1024),
        'M' => (&s[..s.len() - 1], 1),
        'T' => (&s[..s.len() - 1], 1024 * 1024),
        _ => (s, 1),
    };
    num.trim().parse::<u64>().ok().map(|n| n * mult)
}

/// Run the probe and parse it. `argv` is the command to run it with, which is
/// how the privilege escalation gets in front of it.
pub fn run(argv: &[String]) -> Result<Probe, String> {
    let (head, tail) = argv.split_first().ok_or("empty probe command")?;
    let out = Command::new(head)
        .args(tail)
        .output()
        .map_err(|e| format!("could not run {head}: {e}"))?;

    if !out.status.success() && out.stdout.is_empty() {
        let err = String::from_utf8_lossy(&out.stderr);
        let tail: Vec<&str> = err.lines().rev().take(6).collect();
        let mut tail = tail;
        tail.reverse();
        return Err(format!(
            "raven-install --probe failed ({}).\n\n{}",
            out.status,
            tail.join("\n")
        ));
    }

    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let p = parse(&text);

    if p.protocol.is_empty() {
        return Err(
            "raven-install --probe produced nothing this program recognises. \
             The installer on this image may be older than the graphical \
             front-end; run raven-install in a terminal instead."
                .to_string(),
        );
    }
    if p.protocol != SUPPORTED_PROTOCOL {
        return Err(format!(
            "raven-install speaks protocol {} and this front-end reads {}. \
             Refusing to guess at what its records mean -- run raven-install in \
             a terminal instead.",
            p.protocol, SUPPORTED_PROTOCOL
        ));
    }
    Ok(p)
}

pub fn parse(text: &str) -> Probe {
    let mut p = Probe::default();
    let mut disk: Option<Disk> = None;
    // Everything that is not a repeating record, so single-valued keys need no
    // match arm of their own below.
    let mut single: HashMap<&str, String> = HashMap::new();

    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "disk.begin" => {
                disk = Some(Disk {
                    dev: value.to_string(),
                    ..Default::default()
                });
            }
            "disk.end" => {
                if let Some(d) = disk.take() {
                    p.disks.push(d);
                }
            }
            _ if key.starts_with("disk.") => {
                let Some(d) = disk.as_mut() else { continue };
                match key {
                    "disk.size" => d.size = value.to_string(),
                    "disk.bytes" => d.bytes = value.parse().unwrap_or(0),
                    "disk.model" => d.model = value.to_string(),
                    "disk.removable" => d.removable = value == "1",
                    "disk.live_media" => d.live_media = value == "1",
                    "disk.windows" => d.windows = value == "1",
                    "disk.part" => {
                        let f: Vec<&str> = value.splitn(4, '|').collect();
                        d.parts.push(Partition {
                            name: f.first().unwrap_or(&"").to_string(),
                            size: f.get(1).unwrap_or(&"").to_string(),
                            fstype: f.get(2).unwrap_or(&"").to_string(),
                            label: f.get(3).unwrap_or(&"").to_string(),
                        });
                    }
                    _ => {}
                }
            }
            "probe.error" => p.errors.push(value.to_string()),
            "preflight.fs" => p.filesystems.push(value.to_string()),
            "tz" => p.timezones.push(value.to_string()),
            "profile" => p.profiles.push(value.to_string()),
            "answers.key" => p.answer_keys.push(value.to_string()),
            "answers.required" => p.answers_required.push(value.to_string()),
            _ => {
                single.insert(
                    // Leaked into a 'static str only for the map key; the set of
                    // keys is fixed and tiny, so this is a handful of bytes for
                    // the lifetime of a program that runs once.
                    Box::leak(key.to_string().into_boxed_str()),
                    value.to_string(),
                );
            }
        }
    }

    let s = |k: &str| single.get(k).cloned().unwrap_or_default();
    let b = |k: &str| single.get(k).map(|v| v == "1").unwrap_or(false);
    let n = |k: &str| {
        single
            .get(k)
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
    };

    p.protocol = s("probe.protocol");
    p.installer_version = s("probe.installer_version");
    p.euid = n("probe.euid") as u32;
    p.ok = b("probe.ok");
    p.firmware = s("preflight.firmware");
    p.secureboot = s("preflight.secureboot");
    p.tools_missing = s("preflight.tools_missing")
        .split_whitespace()
        .map(str::to_string)
        .collect();
    p.chpasswd = b("preflight.chpasswd");
    p.mkswap = b("preflight.mkswap");
    p.efibootmgr = b("preflight.efibootmgr");
    p.source_kind = s("source.kind");
    p.source_root = s("source.root");
    p.source_kernel = s("source.kernel");
    p.source_initrd = s("source.initrd");
    p.source_efi = s("source.efi");
    p.source_size_mb = n("source.size_mb");
    p.initrd_root_support = s("source.initrd_root_support");
    p.has_desktop = b("source.has_desktop");
    p.mem_total_mb = n("mem.total_mb");
    p.swap_suggested = s("swap.suggested");
    p.esp_size_default = s("esp.size_default");
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
probe.protocol=1
probe.installer_version=1.0
probe.euid=0
probe.ok=1
preflight.firmware=uefi
preflight.secureboot=off
preflight.tools_missing=
preflight.fs=ext4
preflight.fs=btrfs
preflight.chpasswd=1
source.kind=squashfs
source.size_mb=1400
source.has_desktop=1
mem.total_mb=16000
swap.suggested=16G
esp.size_default=512M
disk.begin=/dev/nvme0n1
disk.size=476.9G
disk.bytes=512110190592
disk.model=INTEL SSD
disk.removable=0
disk.live_media=0
disk.part=nvme0n1p1|512M|vfat|RAVEN ESP
disk.windows=0
disk.end=/dev/nvme0n1
disk.begin=/dev/sda
disk.size=28.9G
disk.bytes=31000000000
disk.live_media=1
disk.windows=0
disk.end=/dev/sda
tz=UTC
tz=America/New_York
profile=minimal
answers.key=disk
answers.required=disk
probe.end=1
";

    #[test]
    fn parses_scalars_and_lists() {
        let p = parse(SAMPLE);
        assert_eq!(p.protocol, "1");
        assert!(p.ok);
        assert_eq!(p.filesystems, vec!["ext4", "btrfs"]);
        assert_eq!(p.timezones, vec!["UTC", "America/New_York"]);
        assert_eq!(p.source_size_mb, 1400);
        assert!(p.has_desktop);
        assert!(p.tools_missing.is_empty());
    }

    #[test]
    fn groups_partitions_under_their_disk() {
        let p = parse(SAMPLE);
        assert_eq!(p.disks.len(), 2);
        assert_eq!(p.disks[0].dev, "/dev/nvme0n1");
        assert_eq!(p.disks[0].parts.len(), 1);
        // A label with a space survives the pipe-separated encoding.
        assert_eq!(p.disks[0].parts[0].label, "RAVEN ESP");
        assert_eq!(p.disks[1].parts.len(), 0);
    }

    #[test]
    fn the_live_stick_is_not_offered() {
        let p = parse(SAMPLE);
        let offered = p.installable_disks();
        assert_eq!(offered.len(), 1);
        assert_eq!(offered[0].dev, "/dev/nvme0n1");
    }

    #[test]
    fn sizes() {
        assert_eq!(size_to_mb("8G"), Some(8192));
        assert_eq!(size_to_mb("512M"), Some(512));
        assert_eq!(size_to_mb("1T"), Some(1024 * 1024));
        assert_eq!(size_to_mb("nonsense"), None);
        assert_eq!(size_to_mb(""), None);
    }

    #[test]
    fn a_disk_smaller_than_the_source_is_rejected() {
        let p = parse(SAMPLE);
        let tiny = Disk {
            bytes: 900 * 1024 * 1024,
            ..Default::default()
        };
        assert!(p.disk_too_small(&tiny, 0, 512));
        assert!(!p.disk_too_small(&p.disks[0], 16384, 512));
    }
}
