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

/// A partition that could give up space for an install alongside what is
/// already on the disk. One `part.begin`/`part.end` bracket from the probe.
///
/// Separate from `Partition` above, which is the lsblk listing the disk page
/// shows. These carry the answers to a different question -- can this one be
/// shrunk, by how much, and if not why not -- and the installer pays a
/// resize2fs or ntfsresize probe for each of them.
#[derive(Debug, Clone, Default)]
pub struct ShrinkCandidate {
    pub dev: String,
    pub size_bytes: u64,
    pub fstype: String,
    pub label: String,
    /// What the installer thinks is on it: "Windows", "Windows Recovery",
    /// "ext4 (ubuntu)". Its own words, from part_os_hint.
    pub os: String,
    /// The filesystem can be made smaller at all, by this image, right now.
    /// A fact about the partition: nothing on screen changes it.
    pub resizable: bool,
    /// Resizable *and* able to spare what the install needs. The probe decided
    /// this at the swap it would have chosen itself, so it moves when the swap
    /// switch does -- `Disk::shrinkable_for` is the one to ask.
    pub shrinkable: bool,
    /// Why not, when `shrinkable` is false. Shown rather than hidden: "this
    /// partition is greyed out" with no reason is the thing people file bugs
    /// about.
    pub why: String,
    /// What the filesystem says it needs, from its own resizer.
    pub used_bytes: u64,
    /// The most it could give up while keeping `used_bytes` plus headroom.
    pub spare_bytes: u64,
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

    /// The installer could put RavenLinux on this disk without erasing it.
    pub alongside: bool,
    /// Why not, when it could not. The installer's own sentence.
    pub alongside_why: String,
    /// The EFI System Partition already on the disk, which an alongside
    /// install reuses rather than replaces.
    pub esp: String,
    /// The largest unallocated run on the disk. When this is big enough on its
    /// own, nothing has to be shrunk.
    pub free_bytes: u64,
    pub min_root_bytes: u64,
    pub min_headroom_bytes: u64,
    /// The swap partition the installer would create, which comes out of the
    /// same gap as the root. Zero when swap is off.
    pub swap_bytes: u64,
    /// `min_root_bytes + swap_bytes` -- what the gap actually has to hold, and
    /// the figure the probe judged `alongside` by.
    pub min_total_bytes: u64,
    pub candidates: Vec<ShrinkCandidate>,
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

    /// What an alongside install has to find on this disk, for a given swap.
    ///
    /// The probe answered this for the swap the installer would have picked on
    /// its own, and `min_total_bytes` is that answer. The swap switch is two
    /// groups further down the same page, though, so by the time anything is
    /// on screen the person may have turned swap off -- and then the figure to
    /// judge by is the root alone. Recomputing beats re-running the probe: the
    /// two numbers it takes are both already here.
    pub fn min_total_for(&self, swap_bytes: u64) -> u64 {
        self.min_root_bytes.saturating_add(swap_bytes)
    }

    /// True when the disk already has room and no filesystem needs touching.
    /// This is the safe case and it is worth saying so on screen: it is the
    /// difference between "we will resize Windows" and "we will use the space
    /// you already left".
    ///
    /// `swap_bytes` is part of the question because swap is carved out of the
    /// same unallocated run as the root: a 20 GiB hole is room for RavenLinux
    /// and is not room for RavenLinux plus a 16 GiB swap.
    pub fn has_free_room(&self, swap_bytes: u64) -> bool {
        self.min_root_bytes > 0 && self.free_bytes >= self.min_total_for(swap_bytes)
    }

    /// The partitions that could give up `min_total_for(swap_bytes)`.
    ///
    /// `shrinkable` is the probe's verdict at its own swap figure; this is the
    /// same verdict re-taken at the swap actually chosen, so turning swap off
    /// brings back a partition that was too small with it on.
    pub fn shrinkable_for(&self, swap_bytes: u64) -> Vec<&ShrinkCandidate> {
        let need = self.min_total_for(swap_bytes);
        self.candidates
            .iter()
            .filter(|c| c.resizable && c.spare_bytes >= need)
            .collect()
    }

    /// Can RavenLinux go on this disk without erasing it, at this swap size?
    ///
    /// The probe's own `alongside` is this question answered at the probe's
    /// own swap figure. Re-asking it here is what keeps the disk page honest
    /// in both directions: a disk the probe ruled out only on size comes back
    /// when swap is turned off, and one it allowed stops being offered if the
    /// swap it was judged against grows.
    pub fn alongside_for(&self, swap_bytes: u64) -> bool {
        // Unreadable, no partition table, not GPT, no ESP: the probe stopped
        // before it reported any of the numbers below, and not one of those
        // refusals is about size, so nothing here can overturn them.
        if self.min_root_bytes == 0 {
            return self.alongside;
        }
        self.free_bytes >= self.min_total_for(swap_bytes)
            || !self.shrinkable_for(swap_bytes).is_empty()
    }

    /// Why `alongside_for` said no, in the installer's own words where they
    /// still apply and in the same shape where the figure has moved.
    pub fn alongside_why_for(&self, swap_bytes: u64) -> String {
        if self.min_root_bytes == 0 {
            return self.alongside_why.clone();
        }
        let mut s = format!(
            "no free space and nothing that can give up {}",
            human_bytes(self.min_total_for(swap_bytes))
        );
        // The lever, named only when pulling it would actually help. Offering
        // "turn swap off" on a disk that is full either way is advice that
        // costs the person a page of clicking to find out it was wrong.
        if swap_bytes > 0 && self.alongside_for(0) {
            s.push_str(&format!(
                " -- {} of that is the swap partition, which you can turn off under Layout below",
                human_bytes(swap_bytes)
            ));
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
    /// The two resizers. Not required for an install; required for an install
    /// alongside a filesystem of the matching kind.
    pub resize2fs: bool,
    pub ntfsresize: bool,

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

/// Bytes as the installer prints them, so the same number reads the same way
/// on the confirmation screen and on the page that produced it. Binary units,
/// because that is what the partition table is in and what sfdisk means by G.
pub fn human_bytes(b: u64) -> String {
    const U: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{b} B")
    } else {
        format!("{v:.1} {}", U[i])
    }
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
    // part.* records arrive inside a disk.begin/disk.end bracket and belong to
    // the disk that is open. They are not named disk.* because they are about
    // one partition rather than the disk, so they need their own arms below.
    let mut cand: Option<ShrinkCandidate> = None;
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
                // A `part.begin` with no `part.end` would otherwise leak into
                // the next disk. The probe always closes them, so this is
                // about what happens when it one day does not.
                cand = None;
                if let Some(d) = disk.take() {
                    p.disks.push(d);
                }
            }
            "part.begin" => {
                cand = Some(ShrinkCandidate {
                    dev: value.to_string(),
                    ..Default::default()
                });
            }
            "part.end" => {
                if let (Some(c), Some(d)) = (cand.take(), disk.as_mut()) {
                    d.candidates.push(c);
                }
            }
            _ if key.starts_with("part.") => {
                let Some(c) = cand.as_mut() else { continue };
                match key {
                    "part.size_bytes" => c.size_bytes = value.parse().unwrap_or(0),
                    "part.fstype" => c.fstype = value.to_string(),
                    "part.label" => c.label = value.to_string(),
                    "part.os" => c.os = value.to_string(),
                    "part.resizable" => c.resizable = value == "1",
                    "part.shrinkable" => c.shrinkable = value == "1",
                    "part.why" => c.why = value.to_string(),
                    "part.used_bytes" => c.used_bytes = value.parse().unwrap_or(0),
                    "part.spare_bytes" => c.spare_bytes = value.parse().unwrap_or(0),
                    _ => {}
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
                    "disk.alongside" => d.alongside = value == "1",
                    "disk.alongside_why" => d.alongside_why = value.to_string(),
                    "disk.esp" => d.esp = value.to_string(),
                    "disk.free_bytes" => d.free_bytes = value.parse().unwrap_or(0),
                    "disk.min_root_bytes" => d.min_root_bytes = value.parse().unwrap_or(0),
                    "disk.min_headroom_bytes" => {
                        d.min_headroom_bytes = value.parse().unwrap_or(0)
                    }
                    "disk.swap_bytes" => d.swap_bytes = value.parse().unwrap_or(0),
                    "disk.min_total_bytes" => d.min_total_bytes = value.parse().unwrap_or(0),
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
    p.resize2fs = b("preflight.resize2fs");
    p.ntfsresize = b("preflight.ntfsresize");
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
    fn alongside_records_land_on_the_right_disk() {
        // part.* records are not disk.* records and arrive between a
        // disk.begin and its disk.end. Getting that wrong would attach a
        // Windows partition to the wrong disk, on the page that decides which
        // one gets resized.
        let p = parse(
            "disk.begin=/dev/sda\n\
             disk.alongside=1\n\
             disk.esp=/dev/sda1\n\
             disk.free_bytes=0\n\
             disk.min_root_bytes=12884901888\n\
             disk.swap_bytes=17179869184\n\
             disk.min_total_bytes=30064771072\n\
             part.begin=/dev/sda3\n\
             part.size_bytes=66571993088\n\
             part.fstype=ntfs\n\
             part.os=Windows\n\
             part.resizable=1\n\
             part.shrinkable=1\n\
             part.used_bytes=21474836480\n\
             part.spare_bytes=40802189312\n\
             part.end=/dev/sda3\n\
             part.begin=/dev/sda4\n\
             part.fstype=xfs\n\
             part.resizable=0\n\
             part.shrinkable=0\n\
             part.why=xfs cannot be shrunk here\n\
             part.end=/dev/sda4\n\
             disk.end=/dev/sda\n\
             disk.begin=/dev/sdb\n\
             disk.alongside=0\n\
             disk.alongside_why=no EFI System Partition\n\
             disk.end=/dev/sdb\n",
        );

        assert_eq!(p.disks.len(), 2);
        let a = &p.disks[0];
        assert!(a.alongside);
        assert_eq!(a.esp, "/dev/sda1");
        assert!(!a.has_free_room(0), "0 free is not room for a 12 GiB root");
        assert_eq!(a.candidates.len(), 2);
        // 38 GiB to spare covers a 12 GiB root and a 16 GiB swap either way.
        assert_eq!(a.shrinkable_for(a.swap_bytes).len(), 1);
        assert_eq!(a.shrinkable_for(a.swap_bytes)[0].dev, "/dev/sda3");
        assert_eq!(a.shrinkable_for(a.swap_bytes)[0].os, "Windows");
        assert_eq!(a.candidates[1].why, "xfs cannot be shrunk here");

        // The second disk gets none of the first disk's partitions.
        let b = &p.disks[1];
        assert!(!b.alongside);
        assert!(b.candidates.is_empty());
        assert_eq!(b.alongside_why, "no EFI System Partition");
    }

    #[test]
    fn free_space_alone_is_enough_room() {
        let p = parse(
            "disk.begin=/dev/sda\n\
             disk.alongside=1\n\
             disk.free_bytes=68719476736\n\
             disk.min_root_bytes=12884901888\n\
             disk.swap_bytes=17179869184\n\
             disk.min_total_bytes=30064771072\n\
             disk.end=/dev/sda\n",
        );
        // 64 GiB is room for the root and the swap that goes beside it.
        assert!(p.disks[0].has_free_room(p.disks[0].swap_bytes));
    }

    #[test]
    fn swap_comes_out_of_the_same_hole_as_the_root() {
        // The bug this guards: the probe used to answer "is there room" about
        // the root alone, while plan_alongside_partitions put swap in the same
        // gap. On a 16 GB machine that offered an install needing 12 GiB and
        // then asked for 28.
        let p = parse(
            "disk.begin=/dev/sda\n\
             disk.alongside=0\n\
             disk.esp=/dev/sda1\n\
             disk.free_bytes=21474836480\n\
             disk.min_root_bytes=12884901888\n\
             disk.swap_bytes=17179869184\n\
             disk.min_total_bytes=30064771072\n\
             disk.alongside_why=no free space and nothing that can give up 28.0 GiB\n\
             disk.end=/dev/sda\n",
        );
        let d = &p.disks[0];

        // 20 GiB of free space: a root fits, a root plus a 16 GiB swap does not.
        assert_eq!(d.min_total_for(d.swap_bytes), 30064771072);
        assert!(!d.has_free_room(d.swap_bytes));
        assert!(d.has_free_room(0));

        // And so the whole disk swings on the swap switch.
        assert!(!d.alongside_for(d.swap_bytes));
        assert!(d.alongside_for(0));

        // The refusal has to name the lever, or "28.0 GiB" is a number nobody
        // can account for on a machine with 16 GB of memory.
        let why = d.alongside_why_for(d.swap_bytes);
        assert!(why.contains("16.0 GiB"), "{why}");
        assert!(why.contains("turn off"), "{why}");
    }

    #[test]
    fn a_partition_too_small_for_swap_comes_back_without_it() {
        // 20 GiB of spare on the only resizable partition: enough for a root,
        // not enough for a root and a 16 GiB swap.
        let p = parse(
            "disk.begin=/dev/sda\n\
             disk.alongside=0\n\
             disk.esp=/dev/sda1\n\
             disk.free_bytes=0\n\
             disk.min_root_bytes=12884901888\n\
             disk.swap_bytes=17179869184\n\
             disk.min_total_bytes=30064771072\n\
             part.begin=/dev/sda3\n\
             part.size_bytes=66571993088\n\
             part.os=Windows\n\
             part.resizable=1\n\
             part.shrinkable=0\n\
             part.why=only 20.0 GiB to spare; this install needs 28.0 GiB\n\
             part.used_bytes=21474836480\n\
             part.spare_bytes=21474836480\n\
             part.end=/dev/sda3\n\
             disk.end=/dev/sda\n",
        );
        let d = &p.disks[0];
        assert!(d.shrinkable_for(d.swap_bytes).is_empty());
        assert_eq!(d.shrinkable_for(0).len(), 1);
        assert!(!d.alongside_for(d.swap_bytes));
        assert!(d.alongside_for(0));
    }

    #[test]
    fn an_early_refusal_is_not_about_size() {
        // No ESP, no GPT, unreadable: the probe stops before it reports any of
        // the numbers, and turning swap off cannot bring those disks back.
        let p = parse(
            "disk.begin=/dev/sdb\n\
             disk.alongside=0\n\
             disk.alongside_why=no EFI System Partition\n\
             disk.end=/dev/sdb\n",
        );
        let d = &p.disks[0];
        assert!(!d.alongside_for(0));
        assert_eq!(d.alongside_why_for(0), "no EFI System Partition");
    }

    #[test]
    fn bytes_read_the_way_the_installer_prints_them() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(1023), "1023 B");
        assert_eq!(human_bytes(1024), "1.0 KiB");
        assert_eq!(human_bytes(68719476736), "64.0 GiB");
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
