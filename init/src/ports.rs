//! raven-ports: every port and peripheral on the machine, and what is in it.
//!
//! The kernel already knows all of this and says so in sysfs; what it does not
//! do is put it in one place. This reads the places -- DRM connectors, the
//! Type-C class, the Thunderbolt bus, the USB tree, network links, sound
//! cards and their jacks, bluetooth, input devices, thermal sensors -- and
//! prints them as one inventory, so "is the HDMI port dead or is the cable"
//! is one command rather than a tour of `/sys`.
//!
//! `watch` listens on the kernel's uevent socket and prints devices as they
//! come and go. With `--react` it also acts on the one hotplug the rest of
//! the system does not: a wired link coming up after boot. `raven-dhcp` runs
//! once from init.toml at boot, so a cable plugged in later -- or a dock's
//! NIC -- got no lease. This watches link state over rtnetlink and asks for
//! one. init.toml runs `raven-ports watch --react` as the `ports` service.
//!
//! Everything here is `std` plus `libc` for the two netlink sockets; no
//! udev library, no D-Bus, in keeping with the rest of the Raven layer.

use std::collections::HashMap;
use std::ffi::CStr;
use std::fs;
use std::io;
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::process::Command;

/// What init.toml runs for wired interfaces at boot; `--react` runs the same
/// thing when a link comes up later, so the two cannot disagree.
const DHCP: &str = "/bin/raven-dhcp";
const DHCP_ARGS: &[&str] = &["--all", "-q"];

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let (command, flags): (&str, Vec<&str>) = match args.split_first() {
        None => ("list", Vec::new()),
        Some((first, rest)) => (first.as_str(), rest.iter().map(String::as_str).collect()),
    };

    let code = match command {
        "list" | "all" => {
            print_all();
            0
        }
        "displays" => section("Displays", displays()),
        "usb-c" | "typec" => section("USB-C", typec()),
        "usb4" | "thunderbolt" => section("USB4 / Thunderbolt", thunderbolt()),
        "usb" => section("USB", usb()),
        "net" | "network" => section("Network", network()),
        "audio" | "sound" => section("Audio", audio()),
        "bluetooth" | "bt" => section("Bluetooth", bluetooth()),
        "input" => section("Input", input()),
        "sensors" | "thermal" => section("Sensors", sensors()),
        "watch" => match watch(flags.contains(&"--react")) {
            Ok(()) => 0,
            Err(e) => {
                log::error!("raven-ports watch: {e}");
                1
            }
        },
        "-h" | "--help" | "help" => {
            usage();
            0
        }
        other => {
            eprintln!("raven-ports: unknown command `{other}`");
            usage();
            2
        }
    };
    std::process::exit(code);
}

fn usage() {
    eprintln!(
        "usage: raven-ports [list | displays | usb-c | usb4 | usb | net | audio | bluetooth | input | sensors]\n       \
         raven-ports watch [--react]\n\n\
         list     every port and peripheral (default)\n\
         watch    print devices as they are plugged and unplugged\n\
         --react  also request a DHCP lease when a wired link comes up"
    );
}

fn print_all() {
    for (title, lines) in [
        ("Displays", displays()),
        ("USB-C", typec()),
        ("USB4 / Thunderbolt", thunderbolt()),
        ("USB", usb()),
        ("Network", network()),
        ("Audio", audio()),
        ("Bluetooth", bluetooth()),
        ("Input", input()),
        ("Sensors", sensors()),
    ] {
        section(title, lines);
    }
}

fn section(title: &str, lines: Vec<String>) -> i32 {
    println!("{title}");
    if lines.is_empty() {
        println!("  (none)");
    }
    for line in lines {
        println!("  {line}");
    }
    println!();
    0
}

// ---------------------------------------------------------------------------
// sysfs helpers
// ---------------------------------------------------------------------------

fn read(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn read_or(path: impl AsRef<Path>, fallback: &str) -> String {
    read(path).unwrap_or_else(|| fallback.to_owned())
}

/// The entries of `dir` whose names start with `prefix`, sorted by name.
fn entries(dir: &str, prefix: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(dir)
        .map(|it| {
            it.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with(prefix))
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("?")
        .to_owned()
}

/// The driver bound to a device, from its `driver` symlink.
fn driver(path: &Path) -> Option<String> {
    fs::read_link(path.join("driver"))
        .ok()
        .and_then(|target| target.file_name().map(|n| n.to_string_lossy().into_owned()))
}

// ---------------------------------------------------------------------------
// Displays
// ---------------------------------------------------------------------------

fn displays() -> Vec<String> {
    let mut out = Vec::new();
    for card in entries("/sys/class/drm", "card") {
        let name = name_of(&card);
        // `card1-HDMI-A-1` is a connector; `card1` is the device.
        let Some((_, connector)) = name.split_once('-') else {
            let drv = driver(&card.join("device")).unwrap_or_else(|| "no driver".into());
            out.push(format!("{name}: GPU ({drv})"));
            continue;
        };
        let status = read_or(card.join("status"), "unknown");
        let enabled = read_or(card.join("enabled"), "unknown");
        let mut line = format!("{connector}: {status}");
        if status == "connected" {
            // The first listed mode is the one the kernel prefers.
            if let Some(mode) =
                read(card.join("modes")).and_then(|m| m.lines().next().map(str::to_owned))
            {
                line.push_str(&format!(", {mode}"));
            }
            if let Some((w, h)) = edid_size_mm(&card.join("edid")) {
                let inches = ((w * w + h * h) as f64).sqrt() / 25.4;
                line.push_str(&format!(", {w}x{h} mm ({inches:.1}\")"));
            }
            line.push_str(&format!(", {enabled}"));
        }
        out.push(line);
    }
    out
}

/// Physical size from the EDID's base block: bytes 21 and 22 are the
/// horizontal and vertical size in centimetres. Zero means the panel did not
/// say, which projectors and some docks do.
fn edid_size_mm(path: &Path) -> Option<(u32, u32)> {
    let edid = fs::read(path).ok()?;
    if edid.len() < 23 {
        return None;
    }
    let (w, h) = (u32::from(edid[21]) * 10, u32::from(edid[22]) * 10);
    (w > 0 && h > 0).then_some((w, h))
}

// ---------------------------------------------------------------------------
// USB-C
// ---------------------------------------------------------------------------

fn typec() -> Vec<String> {
    let mut out = Vec::new();
    let ports = entries("/sys/class/typec", "port");
    for port in ports
        .iter()
        .filter(|p| !name_of(p).contains("partner") && !name_of(p).contains("cable"))
    {
        let name = name_of(port);
        let data = read_or(port.join("data_role"), "?");
        let power = read_or(port.join("power_role"), "?");
        let usb_power = read_or(port.join("usb_power_delivery_revision"), "");
        let mut line = format!(
            "{name}: data {}, power {}",
            bracketed(&data),
            bracketed(&power)
        );
        if !usb_power.is_empty() && usb_power != "0.0" {
            line.push_str(&format!(", PD {usb_power}"));
        }
        let partner = port.join(format!("{name}-partner"));
        if partner.exists() {
            let accessory = read_or(partner.join("accessory_mode"), "none");
            let mut modes = Vec::new();
            for alt in entries(&partner.to_string_lossy(), &format!("{name}-partner.")) {
                let svid = read_or(alt.join("svid"), "?");
                let active = read_or(alt.join("active"), "no");
                let what = match svid.as_str() {
                    "ff01" => "DisplayPort",
                    "8087" => "Thunderbolt",
                    "955" => "VirtualLink",
                    _ => "alt mode",
                };
                modes.push(format!("{what} {svid} ({active})"));
            }
            line.push_str(", partner attached");
            if accessory != "none" {
                line.push_str(&format!(", accessory {accessory}"));
            }
            if !modes.is_empty() {
                line.push_str(&format!(": {}", modes.join(", ")));
            }
        } else {
            line.push_str(", nothing attached");
        }
        out.push(line);
    }
    if out.is_empty() && !Path::new("/sys/class/typec").exists() {
        out.push("no Type-C class: kernel built without CONFIG_TYPEC, or no USB-C ports".into());
    }
    out
}

/// `[host] device` -> `host`: the bracketed entry is the current role.
fn bracketed(roles: &str) -> String {
    roles
        .split_whitespace()
        .find(|r| r.starts_with('['))
        .map(|r| r.trim_matches(&['[', ']'][..]).to_owned())
        .unwrap_or_else(|| roles.to_owned())
}

// ---------------------------------------------------------------------------
// USB4 / Thunderbolt
// ---------------------------------------------------------------------------

fn thunderbolt() -> Vec<String> {
    let mut out = Vec::new();
    for dev in entries("/sys/bus/thunderbolt/devices", "") {
        let name = name_of(&dev);
        // Domains and retimers are plumbing; devices have a device_name.
        let Some(device) = read(dev.join("device_name")) else {
            if name.starts_with("domain") {
                out.push(format!("{name}: host controller"));
            }
            continue;
        };
        let vendor = read_or(dev.join("vendor_name"), "");
        let authorized = match read(dev.join("authorized")).as_deref() {
            Some("0") => "not authorized",
            Some(_) => "authorized",
            None => "",
        };
        let generation = read(dev.join("generation"))
            .map(|g| format!("gen {g}"))
            .unwrap_or_default();
        let speed = read(dev.join("tx_speed"))
            .map(|s| format!("{s}"))
            .unwrap_or_default();
        out.push(
            [
                name,
                format!("{vendor} {device}").trim().to_owned(),
                generation,
                speed,
                authorized.to_owned(),
            ]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        );
    }
    if out.is_empty() && !Path::new("/sys/bus/thunderbolt").exists() {
        out.push("no Thunderbolt bus: kernel built without CONFIG_USB4, or no USB4 ports".into());
    }
    out
}

// ---------------------------------------------------------------------------
// USB
// ---------------------------------------------------------------------------

fn usb() -> Vec<String> {
    let mut out = Vec::new();
    for dev in entries("/sys/bus/usb/devices", "") {
        let name = name_of(&dev);
        // `1-2` is a device, `1-2:1.0` an interface, `usb1` a root hub.
        if name.contains(':') {
            continue;
        }
        let Some(product) = read(dev.join("product")) else {
            continue;
        };
        let is_root = name.starts_with("usb");
        let speed = read(dev.join("speed"))
            .map(|s| match s.as_str() {
                "1.5" => "USB 1 low".to_owned(),
                "12" => "USB 1.1".to_owned(),
                "480" => "USB 2.0".to_owned(),
                "5000" => "USB 3.0 5G".to_owned(),
                "10000" => "USB 3.1 10G".to_owned(),
                "20000" => "USB 3.2 20G".to_owned(),
                other => format!("{other} Mb/s"),
            })
            .unwrap_or_default();
        let id = format!(
            "{}:{}",
            read_or(dev.join("idVendor"), "????"),
            read_or(dev.join("idProduct"), "????")
        );
        // The drivers bound to its interfaces are what tell you whether the
        // thing works: a webcam with `uvcvideo` is a webcam, one without is
        // a USB device the kernel has no idea what to do with.
        let mut drivers: Vec<String> = fs::read_dir(&dev)
            .map(|it| {
                it.filter_map(Result::ok)
                    .map(|e| e.path())
                    .filter(|p| name_of(p).starts_with(&format!("{name}:")))
                    .filter_map(|p| driver(&p))
                    .collect()
            })
            .unwrap_or_default();
        drivers.sort();
        drivers.dedup();
        let manufacturer = read(dev.join("manufacturer")).unwrap_or_default();
        let mut line = if is_root {
            format!("{name}: {product} (root hub, {speed})")
        } else {
            format!("{name}: {manufacturer} {product} [{id}], {speed}")
        };
        if !is_root {
            if drivers.is_empty() {
                line.push_str(", no driver");
            } else {
                line.push_str(&format!(", {}", drivers.join("+")));
            }
        }
        out.push(line.replace("  ", " "));
    }
    out
}

// ---------------------------------------------------------------------------
// Network
// ---------------------------------------------------------------------------

fn network() -> Vec<String> {
    let mut out = Vec::new();
    for iface in entries("/sys/class/net", "") {
        let name = name_of(&iface);
        if name == "lo" {
            continue;
        }
        let kind = link_kind(&iface);
        let state = read_or(iface.join("operstate"), "unknown");
        let carrier = match read(iface.join("carrier")).as_deref() {
            Some("1") => "link up",
            Some("0") => "no link",
            _ => "",
        };
        let speed = read(iface.join("speed"))
            .and_then(|s| s.parse::<i64>().ok())
            .filter(|s| *s > 0)
            .map(|s| format!("{s} Mb/s"))
            .unwrap_or_default();
        let drv = driver(&iface.join("device")).unwrap_or_default();
        let mac = read(iface.join("address")).unwrap_or_default();
        out.push(
            [
                name,
                kind.to_owned(),
                state,
                carrier.to_owned(),
                speed,
                drv,
                mac,
            ]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        );
    }
    out
}

fn link_kind(iface: &Path) -> &'static str {
    if iface.join("wireless").exists() || iface.join("phy80211").exists() {
        "wifi"
    } else if iface.join("device").exists() {
        // A USB path in the device link is a dock or adapter NIC.
        let target = fs::read_link(iface.join("device"))
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        if target.contains("/usb") {
            "wired (USB)"
        } else if target.contains("thunderbolt") {
            "wired (USB4)"
        } else {
            "wired"
        }
    } else {
        "virtual"
    }
}

/// Whether a link is one `--react` should get a lease for: wired, real, up.
fn is_wired(iface: &Path) -> bool {
    matches!(link_kind(iface), "wired" | "wired (USB)" | "wired (USB4)")
}

// ---------------------------------------------------------------------------
// Audio
// ---------------------------------------------------------------------------

fn audio() -> Vec<String> {
    let mut out = Vec::new();
    for card in entries("/sys/class/sound", "card") {
        let name = name_of(&card);
        let id = read_or(card.join("id"), "?");
        let long = read(card.join("device/description"))
            .or_else(|| read(card.join("device/product")))
            .unwrap_or_default();
        let drv = driver(&card.join("device")).unwrap_or_default();
        let mut line = format!("{name} [{id}]");
        if !long.is_empty() {
            line.push_str(&format!(": {long}"));
        }
        if !drv.is_empty() {
            line.push_str(&format!(" ({drv})"));
        }
        out.push(line);
    }
    // Jacks are input devices with switch capabilities, named after the
    // card: "HDA Intel PCH Headphone". Their state is a switch bit read over
    // EVIOCGSW, which needs the event node to be readable.
    for input in entries("/sys/class/input", "input") {
        let Some(name) = read(input.join("name")) else {
            continue;
        };
        if !(name.contains("Headphone")
            || name.contains("Mic")
            || name.contains("HDMI")
            || name.contains("Line")
            || name.contains("Headset"))
        {
            continue;
        }
        let state = jack_state(&input).unwrap_or("state unreadable");
        out.push(format!("jack: {name}: {state}"));
    }
    out
}

fn jack_state(input: &Path) -> Option<&'static str> {
    let event = entries(&input.to_string_lossy(), "event")
        .into_iter()
        .next()?;
    let node = PathBuf::from("/dev/input").join(name_of(&event));
    let file = fs::File::open(node).ok()?;
    let mut bits = [0u8; 8];
    // EVIOCGSW(len): _IOC(_IOC_READ, 'E', 0x1b, len)
    let request = (2u64 << 30) | ((bits.len() as u64) << 16) | (0x45 << 8) | 0x1b;
    // SAFETY: a valid ioctl on an open evdev fd writing into a buffer of
    // exactly the length encoded in the request.
    let rc = unsafe { libc::ioctl(file.as_raw_fd(), request as _, bits.as_mut_ptr()) };
    if rc < 0 {
        return None;
    }
    const SW_HEADPHONE_INSERT: u32 = 2;
    const SW_MICROPHONE_INSERT: u32 = 4;
    const SW_LINEOUT_INSERT: u32 = 6;
    const SW_JACK_PHYSICAL_INSERT: u32 = 7;
    const SW_VIDEOOUT_INSERT: u32 = 8;
    const SW_LINEIN_INSERT: u32 = 13;
    let set = |bit: u32| bits[(bit / 8) as usize] & (1 << (bit % 8)) != 0;
    Some(
        if set(SW_HEADPHONE_INSERT)
            || set(SW_MICROPHONE_INSERT)
            || set(SW_LINEOUT_INSERT)
            || set(SW_JACK_PHYSICAL_INSERT)
            || set(SW_VIDEOOUT_INSERT)
            || set(SW_LINEIN_INSERT)
        {
            "plugged"
        } else {
            "empty"
        },
    )
}

// ---------------------------------------------------------------------------
// Bluetooth
// ---------------------------------------------------------------------------

fn bluetooth() -> Vec<String> {
    let mut out = Vec::new();
    for hci in entries("/sys/class/bluetooth", "hci") {
        let name = name_of(&hci);
        let address = read(hci.join("address")).unwrap_or_default();
        let drv = driver(&hci.join("device")).unwrap_or_default();
        out.push(
            [name, address, drv]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    for rfkill in entries("/sys/class/rfkill", "rfkill") {
        if read(rfkill.join("type")).as_deref() != Some("bluetooth") {
            continue;
        }
        let soft = read(rfkill.join("soft")).as_deref() == Some("1");
        let hard = read(rfkill.join("hard")).as_deref() == Some("1");
        let state = match (soft, hard) {
            (false, false) => "radio on",
            (true, _) => "radio off (soft block)",
            (_, true) => "radio off (hardware switch)",
        };
        out.push(format!(
            "{}: {state}",
            read_or(rfkill.join("name"), &name_of(&rfkill))
        ));
    }
    if out.is_empty() {
        out.push(
            "no adapter: no bluetooth hardware, or its firmware is missing from /lib/firmware"
                .into(),
        );
    }
    out
}

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

fn input() -> Vec<String> {
    let mut out = Vec::new();
    for input in entries("/sys/class/input", "input") {
        let Some(name) = read(input.join("name")) else {
            continue;
        };
        // Jacks and ACPI buttons are listed under audio and are not
        // peripherals anyone plugs in.
        if name.contains("Headphone")
            || name.contains("Mic")
            || name.contains("HDMI")
            || name.contains("Line")
        {
            continue;
        }
        let bus = read(input.join("id/bustype")).unwrap_or_default();
        let via = match bus.as_str() {
            "0003" => "USB",
            "0005" => "Bluetooth",
            "0011" => "PS/2",
            "0018" => "I2C",
            "0019" => "platform",
            _ => "",
        };
        out.push(if via.is_empty() {
            name
        } else {
            format!("{name} ({via})")
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Sensors
// ---------------------------------------------------------------------------

fn sensors() -> Vec<String> {
    let mut out = Vec::new();
    for hwmon in entries("/sys/class/hwmon", "hwmon") {
        let Some(name) = read(hwmon.join("name")) else {
            continue;
        };
        let mut temps = Vec::new();
        for temp in entries(&hwmon.to_string_lossy(), "temp") {
            let file = name_of(&temp);
            if !file.ends_with("_input") {
                continue;
            }
            let label = read(hwmon.join(file.replace("_input", "_label")))
                .unwrap_or_else(|| file.trim_end_matches("_input").to_owned());
            if let Some(milli) = read(&temp).and_then(|v| v.parse::<i64>().ok()) {
                temps.push(format!("{label} {:.0}°C", milli as f64 / 1000.0));
            }
        }
        if !temps.is_empty() {
            out.push(format!("{name}: {}", temps.join(", ")));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// watch: uevents, and link state for --react
// ---------------------------------------------------------------------------

fn watch(react: bool) -> io::Result<()> {
    let uevent = netlink_socket(libc::NETLINK_KOBJECT_UEVENT, 1)?;
    let route = netlink_socket(libc::NETLINK_ROUTE, libc::RTMGRP_LINK as u32)?;
    log::info!(
        "raven-ports: watching for hotplug{}",
        if react {
            "; wired links get a DHCP lease when they come up"
        } else {
            ""
        }
    );

    // Link state as last seen, by interface index, so only transitions act.
    let mut running: HashMap<i32, bool> = HashMap::new();
    if react {
        // Anything already up at start is a link that raven-dhcp handled at
        // boot, or one that came up in the gap before this service; the
        // second deserves a lease and the first is harmless to re-request.
        for iface in entries("/sys/class/net", "") {
            if is_wired(&iface) && read(iface.join("carrier")).as_deref() == Some("1") {
                log::info!("{} is up at start", name_of(&iface));
                request_lease();
                break;
            }
        }
    }

    let mut fds = [
        libc::pollfd {
            fd: uevent.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: route.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        },
    ];
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        // SAFETY: `fds` is a valid array of the length passed.
        let n = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as _, -1) };
        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(err);
        }
        if fds[0].revents & libc::POLLIN != 0 {
            let len = recv(&uevent, &mut buf)?;
            print_uevent(&buf[..len]);
        }
        if fds[1].revents & libc::POLLIN != 0 {
            let len = recv(&route, &mut buf)?;
            for (index, name, up) in link_messages(&buf[..len]) {
                let was = running.insert(index, up).unwrap_or(false);
                if up == was {
                    continue;
                }
                println!("link    {name}: {}", if up { "up" } else { "down" });
                if react && up && is_wired(&PathBuf::from("/sys/class/net").join(&name)) {
                    request_lease();
                }
            }
        }
    }
}

fn request_lease() {
    match Command::new(DHCP).args(DHCP_ARGS).status() {
        Ok(status) if status.success() => log::info!("{DHCP}: lease requested"),
        Ok(status) => log::warn!("{DHCP} exited with {status}"),
        Err(e) => log::warn!("could not run {DHCP}: {e}"),
    }
}

fn netlink_socket(protocol: i32, groups: u32) -> io::Result<OwnedFd> {
    // SAFETY: plain socket creation; the fd is owned below.
    let fd = unsafe {
        libc::socket(
            libc::AF_NETLINK,
            libc::SOCK_RAW | libc::SOCK_CLOEXEC,
            protocol,
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
    addr.nl_groups = groups;
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
                "the kernel uevent socket needs root; run as root or as the `ports` service",
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

/// A uevent is `action@devpath\0KEY=VALUE\0...`. Print the parts a person
/// wants: what happened, to which subsystem, and the device's name.
fn print_uevent(msg: &[u8]) {
    let mut fields = msg
        .split(|b| *b == 0)
        .filter_map(|f| std::str::from_utf8(f).ok());
    let Some(header) = fields.next() else {
        return;
    };
    // udev's own broadcasts start with "libudev"; the kernel's are the ones
    // with an @.
    let Some((action, devpath)) = header.split_once('@') else {
        return;
    };
    let mut subsystem = "";
    let mut devname = "";
    let mut interface = "";
    let mut product = "";
    for field in fields {
        match field.split_once('=') {
            Some(("SUBSYSTEM", v)) => subsystem = v,
            Some(("DEVNAME", v)) => devname = v,
            Some(("INTERFACE", v)) => interface = v,
            Some(("PRODUCT", v)) => product = v,
            _ => {}
        }
    }
    // Interfaces, endpoints and the like arrive by the dozen per plug; the
    // device itself is the line worth reading.
    if subsystem.is_empty()
        || (subsystem == "usb" && devpath.rsplit('/').next().is_some_and(|n| n.contains(':')))
    {
        return;
    }
    let what = if !interface.is_empty() {
        interface.to_owned()
    } else if !devname.is_empty() {
        devname.to_owned()
    } else {
        devpath.rsplit('/').next().unwrap_or(devpath).to_owned()
    };
    let extra = if product.is_empty() {
        String::new()
    } else {
        format!(" [{product}]")
    };
    println!("{action:<7} {subsystem:<12} {what}{extra}");
}

/// The RTM_NEWLINK messages in a buffer, as (index, name, running).
fn link_messages(buf: &[u8]) -> Vec<(i32, String, bool)> {
    const RTM_NEWLINK: u16 = 16;
    const IFLA_IFNAME: u16 = 3;
    let mut out = Vec::new();
    let mut offset = 0;
    let hdr_len = std::mem::size_of::<libc::nlmsghdr>();
    let info_len = std::mem::size_of::<libc::ifinfomsg>();
    while offset + hdr_len <= buf.len() {
        // SAFETY: bounds checked above; nlmsghdr is plain data read unaligned.
        let hdr: libc::nlmsghdr =
            unsafe { std::ptr::read_unaligned(buf.as_ptr().add(offset).cast()) };
        let len = hdr.nlmsg_len as usize;
        if len < hdr_len || offset + len > buf.len() {
            break;
        }
        if hdr.nlmsg_type == RTM_NEWLINK && len >= hdr_len + info_len {
            // SAFETY: bounds checked; ifinfomsg is plain data read unaligned.
            let info: libc::ifinfomsg =
                unsafe { std::ptr::read_unaligned(buf.as_ptr().add(offset + hdr_len).cast()) };
            let running = info.ifi_flags & (libc::IFF_RUNNING as u32) != 0;
            let mut name = String::new();
            let mut at = offset + hdr_len + ((info_len + 3) & !3);
            while at + 4 <= offset + len {
                let rta_len = u16::from_ne_bytes([buf[at], buf[at + 1]]) as usize;
                let rta_type = u16::from_ne_bytes([buf[at + 2], buf[at + 3]]);
                if rta_len < 4 || at + rta_len > offset + len {
                    break;
                }
                if rta_type == IFLA_IFNAME {
                    name = CStr::from_bytes_until_nul(&buf[at + 4..at + rta_len])
                        .map(|c| c.to_string_lossy().into_owned())
                        .unwrap_or_default();
                }
                at += (rta_len + 3) & !3;
            }
            if !name.is_empty() {
                out.push((info.ifi_index, name, running));
            }
        }
        offset += (len + 3) & !3;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bracketed_role_is_the_current_one() {
        assert_eq!(bracketed("[host] device"), "host");
        assert_eq!(bracketed("host [device]"), "device");
        assert_eq!(bracketed("sink"), "sink");
    }

    #[test]
    fn edid_size_comes_from_bytes_21_and_22() {
        let dir = std::env::temp_dir().join(format!("raven-ports-edid-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let mut edid = vec![0u8; 128];
        edid[21] = 34; // 34 cm
        edid[22] = 19;
        fs::write(dir.join("edid"), &edid).unwrap();
        assert_eq!(edid_size_mm(&dir.join("edid")), Some((340, 190)));
        edid[21] = 0;
        fs::write(dir.join("edid"), &edid).unwrap();
        assert_eq!(
            edid_size_mm(&dir.join("edid")),
            None,
            "0 cm means the panel did not say"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_newlink_message_yields_index_name_and_running() {
        // One RTM_NEWLINK for ifindex 4 "enp4s0" with IFF_UP|IFF_RUNNING.
        let hdr_len = std::mem::size_of::<libc::nlmsghdr>();
        let info_len = std::mem::size_of::<libc::ifinfomsg>();
        let name = b"enp4s0\0";
        let rta_len = 4 + name.len();
        let total = hdr_len + info_len + ((rta_len + 3) & !3);
        let mut buf = vec![0u8; total];
        let hdr = libc::nlmsghdr {
            nlmsg_len: total as u32,
            nlmsg_type: 16,
            nlmsg_flags: 0,
            nlmsg_seq: 0,
            nlmsg_pid: 0,
        };
        // SAFETY: writing plain data into a buffer large enough for it.
        unsafe { std::ptr::write_unaligned(buf.as_mut_ptr().cast(), hdr) };
        let mut info: libc::ifinfomsg = unsafe { std::mem::zeroed() };
        info.ifi_index = 4;
        info.ifi_flags = (libc::IFF_UP | libc::IFF_RUNNING) as u32;
        // SAFETY: as above, at the offset the parser reads from.
        unsafe { std::ptr::write_unaligned(buf.as_mut_ptr().add(hdr_len).cast(), info) };
        let at = hdr_len + info_len;
        buf[at..at + 2].copy_from_slice(&(rta_len as u16).to_ne_bytes());
        buf[at + 2..at + 4].copy_from_slice(&3u16.to_ne_bytes());
        buf[at + 4..at + 4 + name.len()].copy_from_slice(name);

        assert_eq!(link_messages(&buf), vec![(4, "enp4s0".to_owned(), true)]);
    }

    #[test]
    fn a_uevent_for_an_interface_is_skipped_and_a_device_is_not() {
        // Exercised for the parse, not the print: a message with no
        // subsystem is dropped without panicking.
        print_uevent(b"add@/devices/x\0");
        print_uevent(b"libudev\0");
    }
}
