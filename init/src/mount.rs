//! raven-mount: removable storage, mounted where a desktop can see it.
//!
//! Plugging in a USB drive produced a `/dev/sdb1` and nothing else. The kernel
//! had done its whole job -- the device was there, the filesystem drivers were
//! in -- and the machine still looked broken, because nothing in userspace
//! turned a block device into a directory. This is the piece that was missing.
//!
//! It is deliberately not udisks2. udisks2 answers to polkit, and `etc/raven/
//! init.toml` says why polkitd is not here: nothing in Raven consults it, so
//! it was a root process for nothing. Bringing it back to run a second root
//! process that mounts disks is a lot of machinery for one decision -- "is
//! this a removable volume, and where should it appear" -- which is the only
//! thing this file actually contains.
//!
//! What it owns, in the same spirit as `raven-dhcp` wrapping `dhcpcd`: the
//! policy, not the mechanism. `blkid` identifies the filesystem, `mount(2)`
//! mounts it. The decisions here are which devices are eligible (removable,
//! and not part of any disk the system is already running from), where they
//! land (`/media/<label>`), what options they get (`nosuid,nodev`, and an
//! owner for the filesystems that cannot store one), and what happens when
//! the drive is pulled without being unmounted first.
//!
//! RavenFileManager needs nothing new for this. Its sidebar is GIO's
//! `VolumeMonitor`, whose unix backend enumerates `/proc/mounts`, so a volume
//! mounted here shows up in the Devices list on its own.
//!
//! Like `raven-ports`, this is `std` plus `libc` for the uevent socket and
//! `nix` for `mount(2)`: no udev library, no D-Bus.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::process::Command;

use nix::mount::{mount, umount2, MntFlags, MsFlags};

mod user;

/// Where removable volumes appear. FHS 3.0 reserves /media for exactly this,
/// and `scripts/lib/skeleton.sh` already creates it.
const MEDIA_ROOT: &str = "/media";

/// One file per volume this daemon mounted, named after the kernel device.
/// It is the record of what is *ours*: a mountpoint someone made by hand is
/// never unmounted, and a /media directory we did not create is never removed.
/// On tmpfs, so a crash and reboot starts from an honest empty state.
const STATE_DIR: &str = "/run/raven-mount";

/// Filesystems that are a container for something else rather than something
/// to mount. Encrypted volumes are listed but not opened -- there is no
/// passphrase prompt here, and guessing at one would be worse than saying so.
const NOT_MOUNTABLE: &[&str] = &[
    "swap",
    "crypto_LUKS",
    "LVM2_member",
    "linux_raid_member",
    "zfs_member",
    "bcache",
    "DM_snapshot_cow",
];

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let (command, rest): (&str, Vec<&str>) = match args.split_first() {
        None => ("list", Vec::new()),
        Some((first, tail)) => (first.as_str(), tail.iter().map(String::as_str).collect()),
    };

    let code = match command {
        "list" | "status" => {
            list();
            0
        }
        "watch" => match watch(rest.contains(&"--automount")) {
            Ok(()) => 0,
            Err(e) => {
                log::error!("raven-mount watch: {e}");
                1
            }
        },
        "mount" => match rest.first() {
            Some(target) => mount_one(target),
            None => {
                eprintln!("raven-mount mount: needs a device, e.g. /dev/sdb1");
                2
            }
        },
        "unmount" | "umount" | "eject" => {
            if rest.contains(&"--all") {
                unmount_all()
            } else {
                match rest.first() {
                    Some(target) => unmount_one(target, command == "eject"),
                    None => {
                        eprintln!("raven-mount {command}: needs a device or mountpoint, or --all");
                        2
                    }
                }
            }
        }
        "-h" | "--help" | "help" => {
            usage();
            0
        }
        other => {
            eprintln!("raven-mount: unknown command '{other}'");
            usage();
            2
        }
    };
    std::process::exit(code);
}

fn usage() {
    eprintln!(
        "usage: raven-mount [list]\n\
         \x20      raven-mount watch [--automount]\n\
         \x20      raven-mount mount <device>\n\
         \x20      raven-mount unmount <device|mountpoint> | --all\n\
         \x20      raven-mount eject <device>\n\n\
         list       every removable volume, its filesystem and where it is\n\
         watch      act on drives as they are plugged and pulled (root)\n\
         --automount  mount what appears, rather than only reporting it\n\
         mount      mount one volume by hand, under {MEDIA_ROOT}\n\
         unmount    flush and unmount; eject also spins the device down"
    );
}

// ---------------------------------------------------------------------------
// What is out there
// ---------------------------------------------------------------------------

/// A block device far enough identified to decide what to do with it.
#[derive(Debug, Clone)]
struct Volume {
    /// Kernel name, as in /sys/class/block: `sdb1`, `mmcblk0p1`, `sr0`.
    kname: String,
    dev: PathBuf,
    /// The whole-disk device this sits on; equal to `kname` for a whole disk.
    disk: String,
    fstype: Option<String>,
    label: Option<String>,
    uuid: Option<String>,
    /// Size in bytes, from sysfs's 512-byte sector count.
    size: u64,
    removable: bool,
    /// Where it is mounted now, whoever mounted it.
    mounted_at: Option<PathBuf>,
}

impl Volume {
    /// The name to show a person: the label if it has one, else the device.
    fn display(&self) -> String {
        match &self.label {
            Some(l) => format!("{} ({})", l, self.kname),
            None => self.kname.clone(),
        }
    }

    /// Whether this is something we would mount if asked to automount.
    ///
    /// Mounted already, not removable, no filesystem, or a filesystem that is
    /// a container rather than a tree -- each is a reason to leave it alone.
    fn is_candidate(&self, protected: &BTreeSet<String>) -> bool {
        if self.mounted_at.is_some() || !self.removable || protected.contains(&self.disk) {
            return false;
        }
        match self.fstype.as_deref() {
            None | Some("") => false,
            Some(t) => !NOT_MOUNTABLE.contains(&t),
        }
    }
}

/// Every block device the kernel is showing, with what `blkid` makes of it.
fn volumes() -> Vec<Volume> {
    let mounts = mounted_by_devno();
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/block") else {
        return out;
    };
    for entry in entries.flatten() {
        let kname = entry.file_name().to_string_lossy().into_owned();
        // Virtual block devices are never removable media, and probing them
        // costs a blkid run each. dm- is left out too: a mapper device is the
        // inside of a container this does not open.
        if kname.starts_with("loop")
            || kname.starts_with("ram")
            || kname.starts_with("zram")
            || kname.starts_with("dm-")
            || kname.starts_with("md")
        {
            continue;
        }
        let sysfs = entry.path();
        let is_partition = sysfs.join("partition").is_file();
        let disk = if is_partition {
            parent_disk(&sysfs).unwrap_or_else(|| kname.clone())
        } else {
            kname.clone()
        };
        // A whole disk that has partitions is not itself a volume; its
        // partitions are, and they are separate entries in this same loop.
        if !is_partition && has_partitions(&sysfs, &kname) {
            continue;
        }
        let dev = PathBuf::from("/dev").join(&kname);
        let devno = read_trim(sysfs.join("dev"));
        let probe = blkid(&dev);
        out.push(Volume {
            mounted_at: devno.as_deref().and_then(|d| mounts.get(d).cloned()),
            kname,
            dev,
            removable: is_removable(&sysfs, &disk),
            size: read_trim(sysfs.join("size"))
                .and_then(|s| s.parse::<u64>().ok())
                .map(|sectors| sectors * 512)
                .unwrap_or(0),
            fstype: probe.get("TYPE").cloned(),
            label: probe.get("LABEL").cloned(),
            uuid: probe.get("UUID").cloned(),
            disk,
        });
    }
    out.sort_by(|a, b| a.kname.cmp(&b.kname));
    out
}

/// The whole-disk name behind a partition's sysfs directory.
///
/// /sys/class/block/sdb1 is a symlink into the device tree, where the
/// partition is a child of its disk, so the disk is the parent's name.
fn parent_disk(sysfs: &Path) -> Option<String> {
    let real = fs::canonicalize(sysfs).ok()?;
    let parent = real.parent()?;
    // Guard against a partition whose parent is not itself a block device.
    parent
        .join("dev")
        .is_file()
        .then(|| parent.file_name()?.to_str().map(String::from))
        .flatten()
}

/// Whether a disk has a partition table the kernel has read.
fn has_partitions(sysfs: &Path, kname: &str) -> bool {
    let Ok(entries) = fs::read_dir(sysfs) else {
        return false;
    };
    entries.flatten().any(|e| {
        let name = e.file_name();
        let name = name.to_string_lossy();
        name.starts_with(kname) && e.path().join("partition").is_file()
    })
}

/// Whether the disk behind a device is removable media.
///
/// `removable` covers USB sticks, card readers and optical drives. It does
/// not cover every USB enclosure -- a disk in one often reports 0, because
/// the *disk* is fixed even though the enclosure is not -- so the transport
/// is the second test: anything reached over USB or an SD/MMC host is
/// removable regardless of what the disk claims about itself.
fn is_removable(sysfs: &Path, disk: &str) -> bool {
    let disk_sysfs = PathBuf::from("/sys/class/block").join(disk);
    if read_trim(disk_sysfs.join("removable")).as_deref() == Some("1") {
        return true;
    }
    let path = fs::canonicalize(sysfs)
        .or_else(|_| fs::canonicalize(&disk_sysfs))
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.contains("/usb") || path.contains("/mmc_host/") || path.contains("/ieee1394")
}

/// `blkid -o export`, parsed. Absent or unreadable is an empty map, which
/// reads downstream as "no filesystem here", which is the right conclusion.
fn blkid(dev: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    // blkid moved between /sbin and /usr/bin across distributions; the image
    // keeps it (stage2-native.sh), but not always at one path.
    for bin in ["/usr/bin/blkid", "/sbin/blkid", "/usr/sbin/blkid", "blkid"] {
        let Ok(result) = Command::new(bin).arg("-o").arg("export").arg(dev).output() else {
            continue;
        };
        if !result.status.success() {
            // blkid exits 2 for "no filesystem", which is an answer, not an
            // error; either way there is nothing to parse.
            return out;
        }
        for line in String::from_utf8_lossy(&result.stdout).lines() {
            if let Some((key, value)) = line.split_once('=') {
                out.insert(key.to_string(), value.to_string());
            }
        }
        return out;
    }
    out
}

// ---------------------------------------------------------------------------
// What must not be touched
// ---------------------------------------------------------------------------

/// Mountpoints by `major:minor`, straight from the kernel's own view.
fn mounted_by_devno() -> BTreeMap<String, PathBuf> {
    let mut out = BTreeMap::new();
    let Ok(info) = fs::read_to_string("/proc/self/mountinfo") else {
        return out;
    };
    for line in info.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        // mountinfo: id parent major:minor root mountpoint ...
        if fields.len() < 5 {
            continue;
        }
        // The mountpoint is octal-escaped for spaces and tabs.
        out.entry(fields[2].to_string())
            .or_insert_with(|| PathBuf::from(unescape_mount(fields[4])));
    }
    out
}

/// mountinfo escapes space, tab, newline and backslash as \\040 and friends.
fn unescape_mount(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut chars = field.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let digits: String = chars.clone().take(3).collect();
        match u32::from_str_radix(&digits, 8)
            .ok()
            .and_then(char::from_u32)
        {
            Some(decoded) if digits.len() == 3 => {
                out.push(decoded);
                for _ in 0..3 {
                    chars.next();
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// Disks the system is running from, which automount must never touch.
///
/// Every mounted filesystem's device maps back to a disk, and that whole disk
/// is off limits -- not only the partition that is mounted. A live USB is the
/// case that makes this matter: its squashfs is a loop device, so the USB
/// stick itself is never directly mounted, and without following the loop
/// back to its backing file the daemon would happily mount the stick it is
/// running from and then unmount it when the user pulled something else.
fn protected_disks() -> BTreeSet<String> {
    let mounts = mounted_by_devno();
    let mut disks = BTreeSet::new();
    for devno in mounts.keys() {
        if let Some(disk) = disk_for_devno(devno) {
            disks.insert(disk);
        }
    }
    // Loop devices: resolve the backing file to the filesystem holding it.
    if let Ok(entries) = fs::read_dir("/sys/class/block") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with("loop") {
                continue;
            }
            let Some(devno) = read_trim(entry.path().join("dev")) else {
                continue;
            };
            if !mounts.contains_key(&devno) {
                continue;
            }
            let Some(backing) = read_trim(entry.path().join("loop/backing_file")) else {
                continue;
            };
            if let Some(disk) = disk_holding_path(Path::new(&backing), &mounts) {
                disks.insert(disk);
            }
        }
    }
    disks
}

/// The whole-disk name for a `major:minor`, via /sys/dev/block.
fn disk_for_devno(devno: &str) -> Option<String> {
    let link = PathBuf::from("/sys/dev/block").join(devno);
    let real = fs::canonicalize(&link).ok()?;
    if real.join("partition").is_file() {
        return parent_disk(&real);
    }
    real.file_name()?.to_str().map(String::from)
}

/// Which disk holds a given path, by finding the longest mountpoint that is a
/// prefix of it and mapping that mount's device back to its disk.
fn disk_holding_path(path: &Path, mounts: &BTreeMap<String, PathBuf>) -> Option<String> {
    let mut best: Option<(usize, &String)> = None;
    for (devno, mountpoint) in mounts {
        if path.starts_with(mountpoint) {
            let depth = mountpoint.components().count();
            if best.is_none_or(|(d, _)| depth > d) {
                best = Some((depth, devno));
            }
        }
    }
    disk_for_devno(best?.1)
}

// ---------------------------------------------------------------------------
// Mounting
// ---------------------------------------------------------------------------

/// Who a filesystem without its own ownership should belong to.
///
/// vfat, exfat, ntfs and hfs+ store no uid, so the kernel assigns one at
/// mount time; leave it at the default and the drive is root's and the person
/// at the keyboard cannot write to it. This picks the same account the
/// graphical session runs as, by the same rule `overrides.rs` uses -- the
/// `raven.user=` on the kernel command line, else the lowest-uid regular
/// account -- so the two cannot disagree about whose machine this is.
fn session_owner() -> (u32, u32) {
    let named = fs::read_to_string("/proc/cmdline")
        .ok()
        .and_then(|cmdline| {
            cmdline
                .split_whitespace()
                .find_map(|arg| arg.strip_prefix("raven.user=").map(String::from))
        })
        .and_then(|name| user::by_name(&name).ok());
    match named.or_else(user::first_regular) {
        Some(account) => (account.uid, account.gid),
        None => (0, 0),
    }
}

/// Mount flags and option string for a filesystem type.
///
/// `nosuid` and `nodev` are not negotiable on removable media: a setuid
/// binary or a device node on a stick someone handed you is the oldest trick
/// there is. `noatime` is for the flash the stick is made of.
fn options(fstype: &str, uid: u32, gid: u32) -> (MsFlags, String) {
    let base = MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOATIME;
    // fmask/dmask rather than umask: files come out 0644 and directories
    // 0755, instead of directories inheriting the file mask and ending up
    // without the execute bit that makes them possible to enter.
    let owned = format!("uid={uid},gid={gid},fmask=0133,dmask=0022");
    match fstype {
        "vfat" | "msdos" => (base, format!("{owned},utf8=1,flush")),
        "exfat" => (base, format!("{owned},iocharset=utf8")),
        "ntfs3" | "ntfs" => (base, format!("{owned},windows_names,nls=utf8")),
        "hfsplus" | "hfs" => (base, format!("uid={uid},gid={gid},nls=utf8")),
        // Optical media and disc images are read-only by construction; asking
        // for rw earns an EROFS and a confusing log line.
        "iso9660" | "udf" => (
            base | MsFlags::MS_RDONLY,
            format!("uid={uid},gid={gid},iocharset=utf8"),
        ),
        // ext4, btrfs, xfs, f2fs: real ownership on disk, so imposing one
        // here would be wrong -- the uids in the filesystem are the answer.
        _ => (base, String::new()),
    }
}

/// Mount one volume, returning where it landed.
///
/// The ladder of attempts is the point. A filesystem that rejects an option
/// fails the whole mount with EINVAL, and the set of options each driver
/// accepts moves between kernel versions; a drive that is unclean or
/// hardware-write-protected fails with EROFS however it is asked. Rather than
/// predict either, try the best case and give ground one step at a time, so
/// the outcome is a read-only mount rather than no mount.
fn mount_volume(volume: &Volume, owner: (u32, u32)) -> io::Result<PathBuf> {
    let fstype = volume.fstype.clone().unwrap_or_default();
    if fstype.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{}: no filesystem to mount", volume.kname),
        ));
    }
    let target = allocate_mountpoint(volume)?;
    let (flags, data) = options(&fstype, owner.0, owner.1);
    let minimal = if data.is_empty() {
        String::new()
    } else {
        format!("uid={},gid={}", owner.0, owner.1)
    };

    let mut attempts: Vec<(MsFlags, String)> = vec![
        (flags, data.clone()),
        (flags, minimal.clone()),
        (flags, String::new()),
        (flags | MsFlags::MS_RDONLY, data),
        (flags | MsFlags::MS_RDONLY, minimal),
        (flags | MsFlags::MS_RDONLY, String::new()),
    ];
    attempts.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);

    let mut last: Option<nix::Error> = None;
    for (flags, data) in attempts {
        let data_arg = (!data.is_empty()).then_some(data.as_str());
        match mount(
            Some(volume.dev.as_path()),
            &target,
            Some(fstype.as_str()),
            flags,
            data_arg,
        ) {
            Ok(()) => {
                if flags.contains(MsFlags::MS_RDONLY) {
                    log::info!(
                        "{} mounted read-only at {}",
                        volume.display(),
                        target.display()
                    );
                } else {
                    log::info!("{} mounted at {}", volume.display(), target.display());
                }
                record_ours(&volume.kname, &target);
                return Ok(target);
            }
            Err(e) => last = Some(e),
        }
    }
    // Leave no empty directory behind for a mount that never happened.
    let _ = fs::remove_dir(&target);
    Err(io::Error::from(last.unwrap_or(nix::Error::EINVAL)))
}

/// Create and return a free directory under /media for this volume.
fn allocate_mountpoint(volume: &Volume) -> io::Result<PathBuf> {
    fs::create_dir_all(MEDIA_ROOT)?;
    let base = volume
        .label
        .as_deref()
        .and_then(sanitize)
        .or_else(|| {
            let uuid = volume.uuid.as_deref()?;
            let short: String = uuid.chars().filter(|c| *c != '-').take(8).collect();
            sanitize(&format!(
                "{}-{short}",
                volume.fstype.as_deref().unwrap_or("disk")
            ))
        })
        .unwrap_or_else(|| volume.kname.clone());

    for suffix in 0..64 {
        let name = if suffix == 0 {
            base.clone()
        } else {
            format!("{base}-{}", suffix + 1)
        };
        let candidate = Path::new(MEDIA_ROOT).join(&name);
        // create_dir fails with AlreadyExists rather than racing another
        // mounter for the same name.
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("{MEDIA_ROOT}/{base}: no free name"),
    ))
}

/// A filesystem label is arbitrary bytes; a path component is not.
///
/// Anything outside a conservative set becomes an underscore, a leading dot
/// is dropped so a label cannot produce a hidden directory, and the result is
/// capped well short of NAME_MAX. `None` means the label was unusable and the
/// caller should fall back to the UUID or the device name.
fn sanitize(label: &str) -> Option<String> {
    let cleaned: String = label
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches(['_', '.']).to_string();
    if cleaned.is_empty() {
        return None;
    }
    Some(cleaned.chars().take(64).collect())
}

// ---------------------------------------------------------------------------
// What we mounted, so we only unmount that
// ---------------------------------------------------------------------------

fn record_ours(kname: &str, target: &Path) {
    if fs::create_dir_all(STATE_DIR).is_err() {
        return;
    }
    let _ = fs::write(
        Path::new(STATE_DIR).join(kname),
        target.to_string_lossy().as_bytes(),
    );
}

fn forget(kname: &str) {
    let _ = fs::remove_file(Path::new(STATE_DIR).join(kname));
}

fn ours(kname: &str) -> Option<PathBuf> {
    let recorded = fs::read_to_string(Path::new(STATE_DIR).join(kname)).ok()?;
    let path = PathBuf::from(recorded.trim());
    path.starts_with(MEDIA_ROOT).then_some(path)
}

/// Every mountpoint this daemon is responsible for.
fn all_ours() -> Vec<(String, PathBuf)> {
    let Ok(entries) = fs::read_dir(STATE_DIR) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let kname = e.file_name().to_string_lossy().into_owned();
            let path = ours(&kname)?;
            Some((kname, path))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Unmounting
// ---------------------------------------------------------------------------

/// Unmount a path we mounted, flush it, and tidy the directory away.
///
/// `detach` is for the drive that is already gone: the device the filesystem
/// was on no longer exists, so a clean unmount can block forever waiting to
/// write back, and MNT_DETACH is the only thing that returns. For a volume
/// the user asked to unmount, the clean path is tried first and the lazy one
/// is the fallback, because a lazy unmount on live hardware means the writes
/// are still in flight when the person pulls the stick.
fn unmount_at(target: &Path, detach: bool) -> io::Result<()> {
    // SAFETY: sync(2) takes no arguments and cannot fail.
    unsafe { libc::sync() };
    if !detach {
        match umount2(target, MntFlags::empty()) {
            Ok(()) => {
                tidy(target);
                return Ok(());
            }
            Err(nix::Error::EBUSY) => {
                log::warn!(
                    "{}: still in use; detaching -- files open on it will fail",
                    target.display()
                );
            }
            Err(nix::Error::EINVAL) => {
                // Not a mountpoint any more. Nothing to do, and the directory
                // still deserves clearing.
                tidy(target);
                return Ok(());
            }
            Err(e) => return Err(io::Error::from(e)),
        }
    }
    match umount2(target, MntFlags::MNT_DETACH) {
        Ok(()) | Err(nix::Error::EINVAL) => {
            tidy(target);
            Ok(())
        }
        Err(e) => Err(io::Error::from(e)),
    }
}

/// Remove an empty mountpoint under /media. Anything still in the directory
/// means it is not ours to remove, so the failure is ignored on purpose.
fn tidy(target: &Path) {
    if target.starts_with(MEDIA_ROOT) && target != Path::new(MEDIA_ROOT) {
        let _ = fs::remove_dir(target);
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn list() {
    let protected = protected_disks();
    let volumes = volumes();
    let removable: Vec<&Volume> = volumes.iter().filter(|v| v.removable).collect();
    if removable.is_empty() {
        println!("No removable volumes.");
        return;
    }
    println!(
        "{:<12} {:<9} {:>8}  {:<20} MOUNTED",
        "DEVICE", "TYPE", "SIZE", "LABEL"
    );
    for volume in removable {
        let state = match (&volume.mounted_at, ours(&volume.kname)) {
            (Some(at), Some(_)) => at.display().to_string(),
            (Some(at), None) => format!("{} (not ours)", at.display()),
            (None, _) => match volume.fstype.as_deref() {
                Some(t) if NOT_MOUNTABLE.contains(&t) => format!("-  ({t}, not opened here)"),
                None | Some("") => "-  (no filesystem)".to_string(),
                _ if protected.contains(&volume.disk) => "-  (system disk)".to_string(),
                _ => "-".to_string(),
            },
        };
        println!(
            "{:<12} {:<9} {:>8}  {:<20} {}",
            volume.kname,
            volume.fstype.as_deref().unwrap_or("-"),
            human(volume.size),
            volume.label.as_deref().unwrap_or("-"),
            state
        );
    }
}

fn human(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes}{}", UNITS[0])
    } else {
        format!("{value:.1}{}", UNITS[unit])
    }
}

/// Resolve what the user typed -- /dev/sdb1, sdb1, or a mountpoint -- to a
/// volume.
fn find(target: &str) -> Option<Volume> {
    let kname = target.trim_start_matches("/dev/");
    let volumes = volumes();
    volumes
        .iter()
        .find(|v| v.kname == kname)
        .or_else(|| {
            let path = fs::canonicalize(target).ok()?;
            volumes
                .iter()
                .find(|v| v.mounted_at.as_deref() == Some(&path))
        })
        .cloned()
}

fn mount_one(target: &str) -> i32 {
    let Some(volume) = find(target) else {
        eprintln!("raven-mount: no block device matching '{target}'");
        return 1;
    };
    if let Some(at) = &volume.mounted_at {
        println!(
            "{} is already mounted at {}",
            volume.display(),
            at.display()
        );
        return 0;
    }
    let protected = protected_disks();
    if protected.contains(&volume.disk) {
        eprintln!(
            "raven-mount: {} is on {}, which the running system is using",
            volume.kname, volume.disk
        );
        return 1;
    }
    match mount_volume(&volume, session_owner()) {
        Ok(path) => {
            println!("{}", path.display());
            0
        }
        Err(e) => {
            eprintln!("raven-mount: {}: {e}", volume.kname);
            1
        }
    }
}

fn unmount_one(target: &str, eject: bool) -> i32 {
    // Accept a mountpoint directly, so `raven-mount unmount /media/TRAVEL`
    // works even for a volume whose device has already been pulled.
    let by_path = fs::canonicalize(target)
        .ok()
        .filter(|p| p.starts_with(MEDIA_ROOT));
    let volume = find(target);
    let mountpoint = volume
        .as_ref()
        .and_then(|v| v.mounted_at.clone())
        .or(by_path);

    let Some(mountpoint) = mountpoint else {
        eprintln!("raven-mount: nothing mounted for '{target}'");
        return 1;
    };
    if let Err(e) = unmount_at(&mountpoint, false) {
        eprintln!("raven-mount: {}: {e}", mountpoint.display());
        return 1;
    }
    if let Some(volume) = &volume {
        forget(&volume.kname);
        if eject {
            spin_down(&volume.disk);
        }
    }
    println!("{} unmounted", mountpoint.display());
    0
}

fn unmount_all() -> i32 {
    let mut failures = 0;
    for (kname, path) in all_ours() {
        match unmount_at(&path, false) {
            Ok(()) => {
                forget(&kname);
                println!("{} unmounted", path.display());
            }
            Err(e) => {
                eprintln!("raven-mount: {}: {e}", path.display());
                failures += 1;
            }
        }
    }
    i32::from(failures > 0)
}

/// Tell the disk it is safe to power down, so the light goes out and the
/// person knows the stick can come out. Best effort: not every device has the
/// attribute, and failing to spin down is not a failure to unmount.
fn spin_down(disk: &str) {
    let path = PathBuf::from("/sys/block").join(disk).join("device/state");
    let _ = fs::write(path, "offline\n");
}

// ---------------------------------------------------------------------------
// watch: the uevent socket
// ---------------------------------------------------------------------------

fn watch(automount: bool) -> io::Result<()> {
    let uevent = netlink_uevent_socket()?;
    log::info!(
        "raven-mount: watching block devices{}",
        if automount {
            "; removable volumes will be mounted under /media"
        } else {
            " (reporting only; --automount to mount them)"
        }
    );

    // A drive plugged in before this started, or before the machine booted,
    // is as much a hotplug as one arriving now -- there is simply no uevent
    // left to hear. Sweep once so the two cases behave the same.
    if automount {
        sweep();
    }

    let mut buf = vec![0u8; 16 * 1024];
    loop {
        let len = match recv(&uevent, &mut buf) {
            Ok(len) => len,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        let Some(event) = parse_uevent(&buf[..len]) else {
            continue;
        };
        if event.subsystem != "block" {
            continue;
        }
        match event.action.as_str() {
            // `change` is how a card reader announces a card, and how an
            // optical drive announces a disc: the device node existed all
            // along and only now has a filesystem behind it.
            "add" | "change" => {
                if automount {
                    handle_arrival(&event.kname);
                } else {
                    log::info!("block {}: {}", event.action, event.kname);
                }
            }
            "remove" => handle_departure(&event.kname),
            _ => {}
        }
    }
}

/// Mount every removable volume that is currently eligible.
fn sweep() {
    let protected = protected_disks();
    let owner = session_owner();
    for volume in volumes() {
        if volume.is_candidate(&protected) {
            if let Err(e) = mount_volume(&volume, owner) {
                log::warn!("{}: {e}", volume.kname);
            }
        }
    }
}

fn handle_arrival(kname: &str) {
    let protected = protected_disks();
    let owner = session_owner();
    // Re-read rather than trusting the uevent's fields: the event fires when
    // the device appears, which can be before the partition table is read or
    // before blkid can see a filesystem on it.
    let Some(volume) = volumes().into_iter().find(|v| v.kname == kname) else {
        return;
    };
    if !volume.is_candidate(&protected) {
        return;
    }
    if let Err(e) = mount_volume(&volume, owner) {
        log::warn!("{}: {e}", volume.kname);
    }
}

/// A device went away. If it was one of ours, get the mountpoint out of the
/// way; the filesystem is already unreachable either way, and leaving it
/// mounted leaves a stale entry in every file manager on the machine.
fn handle_departure(kname: &str) {
    let Some(mountpoint) = ours(kname) else {
        return;
    };
    log::info!("{kname} was removed; detaching {}", mountpoint.display());
    if let Err(e) = unmount_at(&mountpoint, true) {
        log::warn!("{}: {e}", mountpoint.display());
    }
    forget(kname);
}

struct Uevent {
    action: String,
    subsystem: String,
    kname: String,
}

/// A uevent is `action@devpath\0KEY=VALUE\0...`.
fn parse_uevent(msg: &[u8]) -> Option<Uevent> {
    let mut fields = msg
        .split(|b| *b == 0)
        .filter_map(|f| std::str::from_utf8(f).ok());
    let header = fields.next()?;
    // udev's own broadcasts start with "libudev"; the kernel's have the @.
    let (action, devpath) = header.split_once('@')?;
    let mut subsystem = String::new();
    let mut kname = devpath.rsplit('/').next().unwrap_or_default().to_string();
    for field in fields {
        match field.split_once('=') {
            Some(("SUBSYSTEM", v)) => subsystem = v.to_string(),
            Some(("DEVNAME", v)) => kname = v.trim_start_matches("/dev/").to_string(),
            _ => {}
        }
    }
    Some(Uevent {
        action: action.to_string(),
        subsystem,
        kname,
    })
}

fn netlink_uevent_socket() -> io::Result<OwnedFd> {
    // SAFETY: plain socket creation; the fd is owned below.
    let fd = unsafe {
        libc::socket(
            libc::AF_NETLINK,
            libc::SOCK_RAW | libc::SOCK_CLOEXEC,
            libc::NETLINK_KOBJECT_UEVENT,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the fd was just returned by socket() and is not shared.
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    // SAFETY: sockaddr_nl is plain data; zeroed is a valid initial value.
    let mut addr: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
    addr.nl_family = libc::AF_NETLINK as _;
    addr.nl_groups = 1;
    // SAFETY: binding an owned fd to a fully initialised address of the
    // length passed.
    let rc = unsafe {
        libc::bind(
            fd.as_raw_fd(),
            std::ptr::addr_of!(addr).cast::<libc::sockaddr>(),
            std::mem::size_of::<libc::sockaddr_nl>() as _,
        )
    };
    if rc < 0 {
        let err = io::Error::last_os_error();
        if err.kind() == io::ErrorKind::PermissionDenied {
            return Err(io::Error::new(
                err.kind(),
                "the kernel uevent socket needs root; run as root or as the `mount` service",
            ));
        }
        return Err(err);
    }
    Ok(fd)
}

fn recv(fd: &OwnedFd, buf: &mut [u8]) -> io::Result<usize> {
    // SAFETY: reading into a buffer of the length passed, on an owned fd.
    let n = unsafe { libc::recv(fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len(), 0) };
    if n < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(n as usize)
}

// ---------------------------------------------------------------------------

fn read_trim(path: PathBuf) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_become_safe_path_components() {
        assert_eq!(sanitize("TRAVEL").as_deref(), Some("TRAVEL"));
        assert_eq!(sanitize("My Backup").as_deref(), Some("My_Backup"));
        // A label cannot climb out of /media or hide the mountpoint.
        assert_eq!(sanitize("../../etc").as_deref(), Some("etc"));
        assert_eq!(sanitize(".hidden").as_deref(), Some("hidden"));
        assert_eq!(sanitize("/").as_deref(), None);
        assert_eq!(sanitize("   ").as_deref(), None);
        assert_eq!(sanitize("").as_deref(), None);
        assert_eq!(sanitize("a".repeat(200).as_str()).unwrap().len(), 64);
    }

    #[test]
    fn mountinfo_escapes_are_decoded() {
        assert_eq!(unescape_mount("/media/My\\040Disk"), "/media/My Disk");
        assert_eq!(unescape_mount("/media/plain"), "/media/plain");
        // A lone backslash is kept rather than eating the next characters.
        assert_eq!(unescape_mount("/media/a\\b"), "/media/a\\b");
    }

    #[test]
    fn removable_filesystems_get_an_owner_and_fixed_filesystems_do_not() {
        let (flags, data) = options("vfat", 1000, 1000);
        assert!(data.contains("uid=1000"));
        assert!(flags.contains(MsFlags::MS_NOSUID | MsFlags::MS_NODEV));
        assert!(!flags.contains(MsFlags::MS_RDONLY));

        let (_, data) = options("ext4", 1000, 1000);
        assert!(data.is_empty(), "ext4 stores its own ownership");

        let (flags, _) = options("iso9660", 1000, 1000);
        assert!(flags.contains(MsFlags::MS_RDONLY));
    }

    #[test]
    fn containers_are_not_mounted() {
        for fstype in ["swap", "crypto_LUKS", "LVM2_member"] {
            assert!(NOT_MOUNTABLE.contains(&fstype));
        }
    }

    #[test]
    fn sizes_read_the_way_a_person_expects() {
        assert_eq!(human(512), "512B");
        assert_eq!(human(1024), "1.0K");
        assert_eq!(human(32 * 1024 * 1024 * 1024), "32.0G");
    }
}
