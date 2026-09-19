//! Manual partitioning: the arithmetic behind the partition editor.
//!
//! Nothing here touches a disk. It turns the probe's view of a partition table
//! plus the plan in `Answers` into what the editor draws -- partitions, new
//! partitions and free space, in disk order -- and says what is wrong with the
//! plan. raven-install's plan_manual checks all of it again and is the copy
//! that decides; this one exists so the person hears about a missing root
//! partition on the page where they can add one.

use crate::answers::{Answers, NewPart};
use crate::probe::{Disk, Probe, Slot, GUID_ESP};

/// The smallest ESP either side will create: RavenBoot, a kernel and an
/// initramfs, with room for a second kernel. MANUAL_MIN_ESP in raven-install.
pub const MIN_ESP_BYTES: u64 = 200 * 1024 * 1024;

/// Free space smaller than this is not worth a row.
const MIN_FREE_BYTES: u64 = 16 * 1024 * 1024;

pub const ROLES: &[(&str, &str)] = &[
    ("root", "/ (root)"),
    ("home", "/home"),
    ("swap", "Swap"),
    ("esp", "EFI System Partition"),
];

pub fn role_label(role: &str) -> &'static str {
    ROLES.iter().find(|(r, _)| *r == role).map_or("", |(_, l)| l)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Segment {
    /// An existing partition that is being kept (whatever it is used for).
    Existing(Slot),
    /// A partition this plan creates; the index into `manual_new`.
    New(usize, NewPart),
    /// Unallocated, aligned, and big enough to put something in.
    Free { start: u64, sectors: u64 },
}

impl Segment {
    pub fn start(&self) -> u64 {
        match self {
            Segment::Existing(s) => s.start,
            Segment::New(_, n) => n.start,
            Segment::Free { start, .. } => *start,
        }
    }
}

fn align(d: &Disk) -> u64 {
    d.align_sectors.max(1)
}

fn sector(d: &Disk) -> u64 {
    if d.sector_size == 0 { 512 } else { d.sector_size }
}

pub fn bytes(d: &Disk, sectors: u64) -> u64 {
    sectors.saturating_mul(sector(d))
}

/// Sectors for a size in bytes, rounded down to the disk's alignment.
pub fn sectors_for(d: &Disk, b: u64) -> u64 {
    let s = b / sector(d);
    s / align(d) * align(d)
}

/// The table as the plan leaves it, in disk order.
pub fn segments(d: &Disk, a: &Answers) -> Vec<Segment> {
    let mut out: Vec<Segment> = d
        .slots
        .iter()
        .filter(|s| !a.manual_delete.contains(&s.dev))
        .cloned()
        .map(Segment::Existing)
        .collect();
    out.extend(
        a.manual_new
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, n)| Segment::New(i, n)),
    );
    out.sort_by_key(Segment::start);

    // Free space is what lies between them, trimmed to the alignment every
    // new partition has to start and end on.
    let al = align(d);
    let mut spans: Vec<(u64, u64)> = out
        .iter()
        .map(|s| match s {
            Segment::Existing(sl) => (sl.start, sl.sectors),
            Segment::New(_, n) => (n.start, n.sectors),
            Segment::Free { .. } => unreachable!(),
        })
        .collect();
    spans.sort();
    let mut free = Vec::new();
    let mut cursor = d.first_lba;
    let mut gap = |from: u64, to_incl: u64| {
        let s = from.div_ceil(al) * al;
        let e = (to_incl + 1) / al * al; // exclusive
        if e > s && (e - s) * sector(d) >= MIN_FREE_BYTES {
            free.push(Segment::Free { start: s, sectors: e - s });
        }
    };
    for (start, sectors) in spans {
        if start > cursor {
            gap(cursor, start - 1);
        }
        cursor = cursor.max(start + sectors);
    }
    if d.last_lba >= cursor && d.last_lba > 0 {
        gap(cursor, d.last_lba);
    }
    out.extend(free);
    out.sort_by_key(Segment::start);
    out
}

/// The choices the "Use as" menu offers for one existing partition, as
/// (label, value). The value is "" for leave, "delete", or "role:action".
pub fn choices(s: &Slot) -> Vec<(String, String)> {
    let mut v = vec![("Leave as it is".to_string(), String::new())];
    v.push(("/ (root) — format".into(), "root:format".into()));
    if matches!(s.fstype.as_str(), "ext2" | "ext3" | "ext4" | "xfs" | "btrfs") {
        v.push(("/home — keep its files".into(), "home:keep".into()));
    }
    v.push(("/home — format".into(), "home:format".into()));
    if s.fstype == "swap" {
        v.push(("Swap — keep".into(), "swap:keep".into()));
    }
    v.push(("Swap — format".into(), "swap:format".into()));
    if s.fstype == "vfat" {
        let label = match s.free_bytes {
            Some(f) if f < crate::probe::ESP_MIN_FREE => format!(
                "EFI System Partition — keep (only {} free)",
                crate::probe::human_bytes(f)
            ),
            _ => "EFI System Partition — keep (shared)".into(),
        };
        v.push((label, "esp:keep".into()));
    }
    if s.sectors.saturating_mul(512) >= MIN_ESP_BYTES || s.type_guid == GUID_ESP {
        v.push(("EFI System Partition — format".into(), "esp:format".into()));
    }
    v.push(("Delete".into(), "delete".into()));
    v
}

/// What the plan currently says about one existing partition, in the same
/// form `choices` uses.
pub fn current_choice(a: &Answers, dev: &str) -> String {
    if a.manual_delete.iter().any(|d| d == dev) {
        return "delete".into();
    }
    a.manual_use
        .iter()
        .find(|u| u.dev == dev)
        .map(|u| format!("{}:{}", u.role, u.action))
        .unwrap_or_default()
}

/// Apply a menu choice for one existing partition.
pub fn set_choice(a: &mut Answers, dev: &str, value: &str) {
    a.manual_delete.retain(|d| d != dev);
    a.manual_use.retain(|u| u.dev != dev);
    match value {
        "" => {}
        "delete" => a.manual_delete.push(dev.to_string()),
        other => {
            if let Some((role, action)) = other.split_once(':') {
                a.manual_use.push(crate::answers::UsePart {
                    dev: dev.to_string(),
                    role: role.to_string(),
                    action: action.to_string(),
                });
            }
        }
    }
}

/// Undo a delete. A new partition that was put into the freed space would
/// overlap it again, so those go too.
pub fn undelete(a: &mut Answers, d: &Disk, dev: &str) {
    a.manual_delete.retain(|x| x != dev);
    if let Some(s) = d.slots.iter().find(|s| s.dev == dev) {
        let (s0, s1) = (s.start, s.start + s.sectors);
        a.manual_new
            .retain(|n| n.start + n.sectors <= s0 || n.start >= s1);
    }
}

/// RavenLinux gets an ESP of its own while another one stays on the disk.
/// The other OS's boot entry comes first, so ours is the only way in and the
/// boot-entry switch is not optional. plan_manual's ESP_OWN.
pub fn own_esp_beside_another(d: &Disk, a: &Answers) -> bool {
    let reused = a.manual_use.iter().any(|u| u.role == "esp" && u.action == "keep");
    if reused {
        return false;
    }
    let ours = a.manual_use.iter().find(|u| u.role == "esp").map(|u| u.dev.clone());
    d.slots.iter().any(|s| {
        s.type_guid == GUID_ESP
            && !a.manual_delete.contains(&s.dev)
            && Some(&s.dev) != ours.as_ref()
    })
}

/// Why this plan cannot be installed, first thing to fix first.
pub fn problems(d: &Disk, a: &Answers, p: &Probe) -> Vec<String> {
    let mut v = Vec::new();
    if d.label != "gpt" {
        v.push(format!(
            "{} has no GPT partition table; manual partitioning edits one. Erase the disk instead.",
            d.dev
        ));
        return v;
    }
    let count = |role: &str| {
        a.manual_use.iter().filter(|u| u.role == role).count()
            + a.manual_new.iter().filter(|n| n.role == role).count()
    };
    match count("root") {
        0 => v.push("Choose a partition for / (root), or create one in free space.".into()),
        1 => {}
        _ => v.push("Only one partition can be / (root).".into()),
    }
    match count("esp") {
        0 => v.push(
            "Choose an EFI System Partition: keep the one already there, or create one.".into(),
        ),
        1 => {}
        _ => v.push("Only one partition can be the EFI System Partition.".into()),
    }
    if count("home") > 1 {
        v.push("Only one partition can be /home.".into());
    }
    if count("swap") > 1 {
        v.push("Only one partition can be swap.".into());
    }

    let root_bytes = a
        .manual_use
        .iter()
        .filter(|u| u.role == "root")
        .filter_map(|u| d.slots.iter().find(|s| s.dev == u.dev))
        .map(|s| bytes(d, s.sectors))
        .chain(
            a.manual_new
                .iter()
                .filter(|n| n.role == "root")
                .map(|n| bytes(d, n.sectors)),
        )
        .next();
    if let Some(rb) = root_bytes {
        let need = (p.source_size_mb + p.source_size_mb / 10) * 1024 * 1024;
        if rb < need {
            v.push(format!(
                "The root partition is {}; the system alone needs {}.",
                crate::probe::human_bytes(rb),
                crate::probe::human_bytes(need)
            ));
        }
    }
    for u in a.manual_use.iter().filter(|u| u.role == "esp" && u.action == "keep") {
        if let Some(f) = d.slots.iter().find(|s| s.dev == u.dev).and_then(|s| s.free_bytes) {
            if f < crate::probe::ESP_MIN_FREE {
                v.push(format!(
                    "{} has only {} free; RavenLinux needs {} there. Create a new EFI \
                     System Partition in free space instead -- a boot entry will point \
                     the firmware at it.",
                    u.dev,
                    crate::probe::human_bytes(f),
                    crate::probe::human_bytes(crate::probe::ESP_MIN_FREE)
                ));
            }
        }
    }
    for n in a.manual_new.iter().filter(|n| n.role == "esp") {
        if bytes(d, n.sectors) < MIN_ESP_BYTES {
            v.push(format!(
                "A new EFI System Partition needs at least {}.",
                crate::probe::human_bytes(MIN_ESP_BYTES)
            ));
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disk() -> Disk {
        let slot = |dev: &str, start, sectors, t: &str, fs: &str| Slot {
            dev: dev.into(),
            start,
            sectors,
            type_guid: t.into(),
            fstype: fs.into(),
            ..Default::default()
        };
        Disk {
            dev: "/dev/sda".into(),
            label: "gpt".into(),
            sector_size: 512,
            first_lba: 2048,
            last_lba: 134217694,
            align_sectors: 2048,
            slots: vec![
                slot("/dev/sda1", 2048, 1048576, GUID_ESP, "vfat"),
                slot("/dev/sda2", 1050624, 62914560, "EBD0A0A2", "ntfs"),
                slot("/dev/sda3", 63965184, 20971520, "0FC63DAF", "ext4"),
            ],
            ..Default::default()
        }
    }

    fn manual() -> Answers {
        Answers { mode: "manual".into(), ..Default::default() }
    }

    fn free(segs: &[Segment]) -> Vec<(u64, u64)> {
        segs.iter()
            .filter_map(|s| match s {
                Segment::Free { start, sectors } => Some((*start, *sectors)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn free_space_is_what_is_left_aligned() {
        let d = disk();
        let a = manual();
        let f = free(&segments(&d, &a));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].0, 63965184 + 20971520);
        assert_eq!(f[0].0 % 2048, 0);
        assert!(f[0].0 + f[0].1 - 1 <= d.last_lba);
    }

    #[test]
    fn deleting_merges_into_the_free_space_after_it() {
        let d = disk();
        let mut a = manual();
        set_choice(&mut a, "/dev/sda3", "delete");
        let f = free(&segments(&d, &a));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].0, 63965184);
    }

    #[test]
    fn a_new_partition_splits_the_free_space() {
        let d = disk();
        let mut a = manual();
        let start = 63965184 + 20971520;
        a.manual_new.push(NewPart { start, sectors: 41943040, role: "root".into() });
        let segs = segments(&d, &a);
        let f = free(&segs);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].0, start + 41943040);
        assert!(segs.iter().any(|s| matches!(s, Segment::New(0, _))));
    }

    #[test]
    fn undelete_drops_what_was_built_in_the_hole() {
        let d = disk();
        let mut a = manual();
        set_choice(&mut a, "/dev/sda3", "delete");
        a.manual_new.push(NewPart { start: 63965184, sectors: 2048, role: "swap".into() });
        a.manual_new.push(NewPart { start: 90000000, sectors: 2048, role: "home".into() });
        undelete(&mut a, &d, "/dev/sda3");
        assert!(a.manual_delete.is_empty());
        assert_eq!(a.manual_new.len(), 1);
        assert_eq!(a.manual_new[0].role, "home");
    }

    #[test]
    fn plan_needs_one_root_and_one_esp() {
        let d = disk();
        let p = Probe { source_size_mb: 1400, ..Default::default() };
        let mut a = manual();
        assert_eq!(problems(&d, &a, &p).len(), 2);
        set_choice(&mut a, "/dev/sda1", "esp:keep");
        set_choice(&mut a, "/dev/sda3", "root:format");
        assert!(problems(&d, &a, &p).is_empty(), "{:?}", problems(&d, &a, &p));
        a.manual_new.push(NewPart { start: 90000000, sectors: 41943040, role: "root".into() });
        assert!(problems(&d, &a, &p).iter().any(|m| m.contains("Only one")));
    }

    #[test]
    fn root_too_small_is_refused() {
        let d = disk();
        let p = Probe { source_size_mb: 20000, ..Default::default() };
        let mut a = manual();
        set_choice(&mut a, "/dev/sda1", "esp:keep");
        set_choice(&mut a, "/dev/sda3", "root:format");
        assert!(problems(&d, &a, &p).iter().any(|m| m.contains("system alone needs")));
    }

    #[test]
    fn choices_depend_on_what_is_there() {
        let d = disk();
        let esp: Vec<String> = choices(&d.slots[0]).into_iter().map(|c| c.1).collect();
        assert!(esp.contains(&"esp:keep".to_string()));
        let ntfs: Vec<String> = choices(&d.slots[1]).into_iter().map(|c| c.1).collect();
        assert!(!ntfs.contains(&"home:keep".to_string()));
        assert!(!ntfs.contains(&"esp:keep".to_string()));
        let ext4: Vec<String> = choices(&d.slots[2]).into_iter().map(|c| c.1).collect();
        assert!(ext4.contains(&"home:keep".to_string()));
    }

    #[test]
    fn answers_file_carries_the_plan() {
        let mut a = manual();
        set_choice(&mut a, "/dev/sda1", "esp:keep");
        set_choice(&mut a, "/dev/sda3", "delete");
        a.manual_new.push(NewPart { start: 63965184, sectors: 2048, role: "root".into() });
        let f = a.to_file();
        assert!(f.contains("manual_delete=/dev/sda3\n"), "{f}");
        assert!(f.contains("manual_new=63965184:2048:root\n"), "{f}");
        assert!(f.contains("manual_use=/dev/sda1:esp:keep\n"), "{f}");
        a.mode = "wipe".into();
        assert!(!a.to_file().contains("manual_"));
    }

    #[test]
    fn a_full_shared_esp_is_refused_and_a_new_one_needs_an_entry() {
        let mut d = disk();
        d.slots[0].free_bytes = Some(40 * 1024 * 1024);
        let p = Probe { source_size_mb: 1400, ..Default::default() };
        let mut a = manual();
        set_choice(&mut a, "/dev/sda1", "esp:keep");
        set_choice(&mut a, "/dev/sda3", "root:format");
        assert!(problems(&d, &a, &p).iter().any(|m| m.contains("only 40.0 MiB free")));
        assert!(!own_esp_beside_another(&d, &a));
        set_choice(&mut a, "/dev/sda1", "");
        a.manual_new.push(NewPart { start: 90000000, sectors: 1048576, role: "esp".into() });
        assert!(problems(&d, &a, &p).is_empty(), "{:?}", problems(&d, &a, &p));
        assert!(own_esp_beside_another(&d, &a));
    }
}
