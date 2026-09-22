//! Demand-triggered start: services that are not started at boot, and what
//! brings them up when the machine finally needs them.
//!
//! Five daemons on this laptop exist for hardware it usually does not have
//! attached. `cupsd` and `ipp-usb` are a printing stack; `avahi-daemon` is how
//! a network printer is found; `bluetoothd` and `obexd` are bluetooth and file
//! transfer over it. All five started on every boot, whether or not a printer
//! had ever been plugged in, and stayed resident for the life of the machine.
//! That is not a bug in any of them -- a supervisor that was told to start a
//! service starts it -- but it is five daemons' worth of memory and of boot
//! work spent on a machine that is not printing and has nothing paired.
//!
//! So a service definition can now say that it should be started when
//! something happens rather than at boot, and name the something. This module
//! is the half of that which watches: it owns one netlink socket on the
//! kernel's uevent broadcast, it walks sysfs once at startup for the devices
//! that were already plugged in before anybody was listening, and it answers
//! one question -- which services does the machine want started now.
//!
//! # This is demand-triggered start, not socket activation
//!
//! The distinction is the single most important thing in this file and it is
//! repeated in [`crate::config::DemandStart`] because it is the one a reader
//! will assume their way past.
//!
//! systemd's socket activation binds the listening socket *itself*, holds the
//! client's connection while the daemon starts, and passes the already-bound
//! descriptor to the daemon through `LISTEN_FDS`. The promise it makes is that
//! the first connection is never refused: from the client's side there is no
//! observable difference between a daemon that was running and one that was
//! started for it.
//!
//! Nothing here does any of that. Init does not bind anything on a service's
//! behalf, there is no `LISTEN_FDS`, no `LISTEN_PID`, no descriptor is passed,
//! and no connection is ever held. A service started by this module binds its
//! own socket, at its own speed, exactly as it does when it is started at boot
//! -- and a client that connected one microsecond before the trigger fired is
//! refused, exactly as it would be on a machine where the daemon had simply
//! not been started yet.
//!
//! That is why the trigger for each of these five services is a *device*
//! rather than a socket. A printer being plugged in happens seconds before
//! anybody prints, and the daemon is up long before the first connection; a
//! socket path being connected to happens after the connection has already
//! failed. Writing a demand trigger for a service whose correctness depends on
//! never refusing the first connection would be a mistake this mechanism has
//! no way to catch, and is the reason this section exists.
//!
//! # Why uevents, and why also a walk through sysfs
//!
//! The kernel broadcasts a message on a netlink socket every time a device
//! appears or disappears. `raven-ports watch` already listens to exactly that
//! socket -- see `ports.rs`, which opens it with `libc` and no udev library --
//! and this opens it the same way rather than inventing a second mechanism.
//! It is the right shape for the problem: the socket joins the main loop's
//! poll set beside the SIGCHLD pipe, the control listener and the readiness
//! watch, so a printer being plugged in wakes PID 1 in the same microsecond
//! the kernel says so and costs nothing at all while nothing is plugged in.
//!
//! What a broadcast cannot answer is what was already there. The events for
//! devices present at power-on are emitted by `raven-udev`'s coldplug, which
//! runs as the first service of the boot -- before this module's socket
//! exists, and before the main loop that would drain it. A machine with a
//! built-in bluetooth adapter would therefore never start `bluetoothd`, which
//! is the worst possible failure for this feature: silent, permanent, and
//! indistinguishable from the service being broken.
//!
//! So the already-plugged-in case is answered by reading sysfs instead. Every
//! device the kernel knows about has a `uevent` file holding the same
//! `KEY=VALUE` properties the broadcast carries, so the same rule is matched
//! by the same code against the same data, and the two answers cannot drift
//! apart. That walk happens once, when the main loop starts.
//!
//! # What this is not
//!
//! It is not udev and does not want to be. A rule here matches properties of
//! one device; it cannot walk up the device tree, read arbitrary sysfs
//! attributes, or run a program to decide. Where that is genuinely needed, the
//! answer is a udev rule that runs `raven-rc start`, which has always worked
//! and needs nothing from this file -- and which is what a service's
//! `daemon = "..."` demand field is for declaring.
//!
//! It is not a way to stop a service either. Nothing here notices a printer
//! being unplugged, and nothing stops a daemon that was started on demand. A
//! service that should go away when its hardware does needs an idle timeout of
//! its own, which is a decision about that daemon and not about the
//! supervisor.

use std::collections::HashMap;
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::{DeviceTrigger, InitConfig, ServiceConfig};

/// The uevent actions that count as "this device is here now".
///
/// `add` is the device appearing. `bind` is a driver attaching to a device
/// that was already there, which is a separate event and matters because a USB
/// interface can be enumerated before the module that drives it has loaded --
/// on a machine where the printer is plugged in while `usblp` or `ipp-usb`'s
/// class driver is still being modprobed, `add` is the only one that fires,
/// and on a machine where the module arrives afterwards it is `bind`.
///
/// `remove` and `unbind` are deliberately absent: nothing here stops a
/// service. `change` is absent too, and that one is a judgement rather than an
/// omission -- block devices and power supplies emit `change` constantly (a
/// battery's charge level is a `change` on the `power_supply` subsystem, every
/// few seconds), and a rule that fired on those would be asking PID 1 to
/// re-examine every demand-started service several times a minute forever, to
/// answer a question whose answer has not moved.
const TRIGGER_ACTIONS: &[&str] = &["add", "bind"];

/// How many broadcast messages one pass of the main loop will read.
///
/// The same reasoning as `readiness::Watcher::drain`'s bound. Plugging in a
/// USB hub with things already in it produces a burst of events -- one per
/// device, one per interface, one per driver binding -- and a drain with no
/// bound would let a device that is enumerating hold PID 1 inside this
/// function for as long as it kept talking. Anything not read this pass is
/// still in the socket's buffer and is read on the next one; a message that
/// was dropped because the buffer overflowed costs a demand-started service
/// its trigger, which is why the buffer below is made large rather than why
/// this number is made large.
const MAX_MESSAGES_PER_PASS: usize = 64;

/// The receive buffer asked for on the uevent socket, in bytes.
///
/// The kernel's default is small (the system-wide `net.core.rmem_default`,
/// 208KB on this machine) and a uevent message is up to a couple of kilobytes,
/// so the default holds a hundred or so. That is plenty for a hotplug and not
/// obviously plenty for a coldplug of a docked machine, and the cost of asking
/// for more is memory that is only ever allocated as messages actually arrive.
///
/// udev asks for 128MB via `SO_RCVBUFFORCE` on the same socket. This asks for
/// far less, and asks with plain `SO_RCVBUF`: the difference between the two
/// is that the forcing version ignores `net.core.rmem_max`, and a supervisor
/// quietly overriding a limit an operator set is a worse habit than a buffer
/// that is smaller than requested.
const RECV_BUFFER: libc::c_int = 1024 * 1024;

/// The most sysfs entries the startup walk will look at per directory.
///
/// A bound rather than a size: /sys/bus/usb/devices has a few dozen entries on
/// this machine and /sys/class/bluetooth has one, but this code runs on PID 1
/// on machines nobody here has seen, and "read every entry in a directory the
/// kernel populates" is exactly the kind of loop that should have a number on
/// it. Reaching the bound is logged, because a machine that does will have a
/// demand-started service that does not start and no other clue why.
const MAX_SCAN_ENTRIES: usize = 1024;

/// Said once. The socket needs `CAP_NET_ADMIN`, which PID 1 has, so a failure
/// here means either a kernel without `CONFIG_UEVENT_HELPER`/netlink or a
/// supervisor running somewhere it was not designed to -- a container, a test
/// harness, a session supervisor. None of those is worth a line per boot, and
/// all of them are worth the one line.
static SOCKET_REPORTED: AtomicBool = AtomicBool::new(false);

/// One kernel uevent: what happened, and everything the kernel said about the
/// device it happened to.
///
/// The properties are a `Vec` of pairs rather than a map because a uevent has
/// six or eight of them and is looked at once; building a `HashMap` to answer
/// three lookups costs more than the lookups.
struct Uevent {
    action: String,
    properties: Vec<(String, String)>,
}

/// What came back from one read of the socket.
///
/// Three answers rather than a `Result<Option<usize>>` because the middle one
/// has to be distinguishable from the other two at the call site and is not
/// an error: a message that was not the kernel's, or a read cut short by a
/// signal, means read the next one, where "nothing left" means stop. Folding
/// the first into the second is how a process that can write to a netlink
/// socket would have been able to keep a real hotplug from ever being seen.
enum Received {
    /// A kernel broadcast of this many bytes, at the front of the buffer.
    Message(usize),
    /// Something that is not one. Try again within the same pass.
    Ignored,
    /// The socket has nothing more to say this pass.
    Empty,
}

/// The kernel's uevent broadcast, and the state needed to read it.
pub struct Monitor {
    fd: OwnedFd,
    buf: Vec<u8>,
}

impl Monitor {
    /// Open the uevent socket, or `None` if it cannot be opened.
    ///
    /// `None` is not fatal and must not be: a supervisor that refused to boot
    /// because it could not watch for printers would be trading every service
    /// on the machine for one. What it does mean is that demand-started
    /// services have lost their device triggers for this boot, and
    /// [`report_configuration`] is what says so -- loudly, by name, because a
    /// service that will now never start is exactly the kind of thing that is
    /// otherwise discovered a week later by somebody trying to print.
    pub fn open() -> Option<Monitor> {
        match open_uevent_socket() {
            Ok(fd) => Some(Monitor {
                fd,
                // One buffer for the life of the monitor. A uevent is at most
                // a couple of kilobytes; this is sized so that a message is
                // never truncated, because a truncated message is silently a
                // rule that does not match rather than an error anybody sees.
                buf: vec![0u8; 16 * 1024],
            }),
            Err(e) => {
                if SOCKET_REPORTED.swap(true, Ordering::SeqCst) {
                    log::debug!("demand: uevent socket unavailable ({e})");
                } else {
                    log::warn!("Could not open the kernel uevent socket ({e});");
                    log::warn!("  services with a device trigger will not start by themselves");
                }
                None
            }
        }
    }

    /// Read whatever the kernel has said since the last pass and answer with
    /// the names of the services it should start.
    ///
    /// Non-blocking throughout, so it is safe to call on every pass of the
    /// main loop whether or not `poll` said the socket was readable -- which
    /// is what it does, because an unread socket stays readable and would turn
    /// every subsequent poll into a busy loop.
    ///
    /// Names may repeat within one call, and a name may be returned for a
    /// service that is already running: this function knows what the machine
    /// said, not what the supervisor is doing about it. Deciding is the
    /// caller's, which is also where the check for a service an operator
    /// stopped by hand lives.
    pub fn drain(&mut self, config: &InitConfig) -> Vec<String> {
        let mut wanted = Vec::new();
        for _ in 0..MAX_MESSAGES_PER_PASS {
            let len = match self.recv_one() {
                Received::Message(len) => len,
                // Not the kernel's, or interrupted: read the next one rather
                // than ending the pass. Ending it on a message somebody else
                // sent would let any process on the machine stop a real
                // hotplug being noticed simply by talking to this socket
                // first.
                Received::Ignored => continue,
                // Nothing left to read; the ordinary end of a pass.
                Received::Empty => break,
            };
            let Some(event) = parse_uevent(&self.buf[..len]) else {
                continue;
            };
            if !TRIGGER_ACTIONS.contains(&event.action.as_str()) {
                continue;
            }
            let Some(subsystem) = property(&event.properties, "SUBSYSTEM") else {
                // Every kernel uevent carries SUBSYSTEM. One without it is
                // either not from the kernel or is a message shape this code
                // does not understand, and either way there is no rule it can
                // be matched against.
                continue;
            };
            for cfg in config.services.iter().filter(|c| c.enabled) {
                if wanted.iter().any(|n| n == &cfg.name) {
                    continue;
                }
                if device_rules(cfg)
                    .iter()
                    .any(|rule| matches_device(rule, subsystem, &event.properties))
                {
                    log::info!(
                        "Service {} wanted: {} {} appeared",
                        cfg.name,
                        event.action,
                        describe_device(subsystem, &event.properties)
                    );
                    wanted.push(cfg.name.clone());
                }
            }
        }
        wanted
    }

    /// One message, or `None` when the socket has nothing more to give.
    ///
    /// The sender is checked, and that check is the reason this is not a bare
    /// `recv`. A netlink socket can be sent a unicast message by any process
    /// that knows the port id, and the messages this socket carries decide
    /// whether PID 1 starts a daemon. A message the kernel broadcast has a
    /// source port id of zero and a non-empty group mask; anything else was
    /// sent by a process, and a process that wants a service started has
    /// `raven-rc start` and the control socket's own permissions to go
    /// through.
    fn recv_one(&mut self) -> Received {
        // SAFETY: sockaddr_nl is plain data and zero is a valid initial value;
        // it is filled in by the kernel below.
        let mut from: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
        let mut from_len = std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t;
        // SAFETY: reading into a buffer of the length passed, on an owned fd,
        // with an address buffer and its length matching each other.
        let n = unsafe {
            libc::recvfrom(
                self.fd.as_raw_fd(),
                self.buf.as_mut_ptr().cast(),
                self.buf.len(),
                libc::MSG_DONTWAIT,
                std::ptr::addr_of_mut!(from).cast::<libc::sockaddr>(),
                &mut from_len,
            )
        };
        if n < 0 {
            let err = io::Error::last_os_error();
            return match err.kind() {
                io::ErrorKind::WouldBlock => Received::Empty,
                io::ErrorKind::Interrupted => Received::Ignored,
                _ => {
                    log::debug!("demand: reading the uevent socket: {err}");
                    Received::Empty
                }
            };
        }
        if from.nl_pid != 0 || from.nl_groups == 0 {
            log::debug!("demand: ignoring a netlink message from pid {}", from.nl_pid);
            return Received::Ignored;
        }
        Received::Message(n as usize)
    }
}

impl AsFd for Monitor {
    /// The fd the main loop adds to its poll set, beside the SIGCHLD
    /// self-pipe, the control socket and the readiness watch.
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

/// Bind a socket to the kernel's uevent broadcast group.
///
/// Group 1 is the kernel's own broadcasts; group 2 is the one libudev uses to
/// re-broadcast events it has processed, which this deliberately does not
/// join. The difference matters here more than it does in `raven-ports`, which
/// only prints what it hears: a udev-processed event arrives later, after
/// rules have run, and joining both groups would mean seeing most devices
/// twice and starting a service on whichever copy arrived first.
fn open_uevent_socket() -> io::Result<OwnedFd> {
    // SAFETY: plain socket creation; the descriptor is taken ownership of
    // immediately below, and CLOEXEC keeps it out of every service init forks.
    let fd = unsafe {
        libc::socket(
            libc::AF_NETLINK,
            libc::SOCK_RAW | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
            libc::NETLINK_KOBJECT_UEVENT,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the fd was just returned by socket() and is owned by nothing else.
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };

    // Best effort, and deliberately not checked: a receive buffer smaller than
    // asked for still works, and a supervisor that refused to watch for
    // printers because it could not have a megabyte would be trading the
    // feature for a preference.
    let want = RECV_BUFFER;
    // SAFETY: setsockopt with a pointer to a c_int that outlives the call and
    // with that type's own size, on an owned fd.
    unsafe {
        libc::setsockopt(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_RCVBUF,
            std::ptr::addr_of!(want).cast(),
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        );
    }

    // SAFETY: sockaddr_nl is plain data; zeroed is a valid initial value.
    let mut addr: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
    addr.nl_family = libc::AF_NETLINK as _;
    addr.nl_groups = 1;
    // SAFETY: binding an owned fd to a fully initialised address of the length
    // passed.
    let rc = unsafe {
        libc::bind(
            fd.as_raw_fd(),
            std::ptr::addr_of!(addr).cast::<libc::sockaddr>(),
            std::mem::size_of::<libc::sockaddr_nl>() as _,
        )
    };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(fd)
}

/// The kernel's wire format: `action@devpath\0KEY=VALUE\0KEY=VALUE\0...`.
///
/// libudev's re-broadcasts start with the literal `libudev` and have no `@`,
/// which is how they are told apart from the kernel's and skipped -- the same
/// test `raven-ports` makes on the same socket. This monitor does not join the
/// group they arrive on, so seeing one would be surprising; the check costs
/// nothing and means a surprise is ignored rather than mis-parsed.
fn parse_uevent(msg: &[u8]) -> Option<Uevent> {
    let mut fields = msg
        .split(|b| *b == 0)
        .filter_map(|f| std::str::from_utf8(f).ok())
        .filter(|f| !f.is_empty());
    let header = fields.next()?;
    let (action, _devpath) = header.split_once('@')?;
    let mut properties = Vec::new();
    for field in fields {
        if let Some((key, value)) = field.split_once('=') {
            properties.push((key.to_string(), value.to_string()));
        }
    }
    Some(Uevent {
        action: action.to_string(),
        properties,
    })
}

/// A device's uevent properties as sysfs keeps them.
///
/// The file holds the same `KEY=VALUE` lines the broadcast carries, minus the
/// two the kernel adds when it sends one: `ACTION`, which for a device that is
/// simply present is not a question, and `SUBSYSTEM`, which is implied by the
/// directory the device was found under and is supplied by the caller. That is
/// the whole reason the walk and the socket can share their matching code.
fn read_sysfs_uevent(dir: &Path) -> Option<Vec<(String, String)>> {
    let text = std::fs::read_to_string(dir.join("uevent")).ok()?;
    let mut properties = Vec::new();
    for line in text.lines() {
        if let Some((key, value)) = line.split_once('=') {
            properties.push((key.to_string(), value.to_string()));
        }
    }
    Some(properties)
}

/// Whether one rule matches one device.
///
/// `subsystem` is passed separately rather than looked for in `properties`
/// because the two sources of a device differ in exactly that one respect: a
/// broadcast carries `SUBSYSTEM=` and a sysfs `uevent` file does not. Having
/// the caller answer it is what lets both go through this function.
fn matches_device(rule: &DeviceTrigger, subsystem: &str, properties: &[(String, String)]) -> bool {
    if rule.subsystem.is_empty() || rule.subsystem != subsystem {
        return false;
    }
    if let Some(devtype) = rule.devtype.as_deref() {
        if property(properties, "DEVTYPE") != Some(devtype) {
            return false;
        }
    }
    rule.properties.iter().all(|(key, pattern)| {
        property(properties, key).is_some_and(|value| matches_pattern(pattern, value))
    })
}

/// Exact, or a prefix when the pattern ends in `*`.
///
/// The one case this exists for is a USB interface's `class/subclass/protocol`
/// triple: `INTERFACE = "7/*"` is "any printer", where `7/1/4` is specifically
/// one that speaks IPP over USB. See [`DeviceTrigger::properties`] for why
/// there is no more glob than this.
fn matches_pattern(pattern: &str, value: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => value.starts_with(prefix),
        None => pattern == value,
    }
}

fn property<'a>(properties: &'a [(String, String)], key: &str) -> Option<&'a str> {
    properties
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// The device rules a service declares, or nothing.
fn device_rules(cfg: &ServiceConfig) -> &[DeviceTrigger] {
    match cfg.demand.as_ref() {
        Some(demand) => &demand.device,
        None => &[],
    }
}

/// A device named the way a person would recognise it in a log line.
fn describe_device(subsystem: &str, properties: &[(String, String)]) -> String {
    let name = property(properties, "DEVNAME")
        .or_else(|| property(properties, "INTERFACE"))
        .or_else(|| property(properties, "PRODUCT"))
        .unwrap_or("device");
    format!("{subsystem} {name}")
}

/// Which demand-started services the machine already wants, before a single
/// broadcast has been heard.
///
/// This is the coldplug answer and the reason this module is not just a
/// socket. The events for hardware that was plugged in at power-on were
/// emitted by raven-udev's coldplug during `start_services`, long before the
/// main loop and this monitor existed; a machine whose bluetooth adapter is
/// soldered to the mainboard would otherwise never start `bluetoothd` at all.
///
/// It reads sysfs rather than asking the kernel to re-broadcast (which is what
/// writing `add` to a device's own `uevent` file does, and what `udevadm
/// trigger` is). Re-broadcasting would wake every other listener on the
/// machine -- raven-ports, and anything else with a uevent socket open -- with
/// events for devices that have not actually changed, to answer a question
/// this supervisor can answer by reading a file.
///
/// Called once, when the main loop starts. Calling it again would be harmless
/// -- it reads and decides nothing -- but the caller is what turns a name into
/// a start, and doing that twice on a service an operator has since stopped is
/// not harmless at all.
pub fn already_present(config: &InitConfig) -> Vec<String> {
    already_present_in(config, Path::new("/sys"))
}

/// [`already_present`], with the sysfs root named.
///
/// The argument exists for the tests, which plant a directory shaped like the
/// kernel's and prove that a rule finds what is in it -- the same shape
/// `sysctl::apply_all` takes its directories in, and for the same reason: a
/// test that needs no environment variable cannot race the rest of the binary
/// for one.
pub fn already_present_in(config: &InitConfig, sys: &Path) -> Vec<String> {
    let mut wanted: Vec<String> = Vec::new();
    for cfg in config.services.iter().filter(|c| c.enabled) {
        let rules = device_rules(cfg);
        if rules.is_empty() {
            continue;
        }
        for rule in rules {
            if wanted.iter().any(|n| n == &cfg.name) {
                break;
            }
            if let Some(found) = scan_subsystem(rule, sys) {
                log::info!("Service {} wanted: {} is already present", cfg.name, found);
                wanted.push(cfg.name.clone());
            }
        }
    }
    wanted
}

/// Look through a subsystem's devices for one this rule matches, and describe
/// the first that does.
///
/// Both directories are looked in because the kernel puts devices in both and
/// which one depends on the subsystem: `/sys/class/bluetooth/hci0` is where an
/// adapter is, and a USB interface -- the thing a printer rule matches on -- is
/// only ever under `/sys/bus/usb/devices`. Asking for both costs one failed
/// `read_dir` on whichever does not exist, and a subsystem with no devices at
/// all has neither, which is the common case on a laptop with no printer.
fn scan_subsystem(rule: &DeviceTrigger, sys: &Path) -> Option<String> {
    if rule.subsystem.is_empty() {
        return None;
    }
    let roots = [
        sys.join("class").join(&rule.subsystem),
        sys.join("bus").join(&rule.subsystem).join("devices"),
    ];
    for root in roots {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        let mut looked = 0usize;
        for entry in entries.flatten() {
            looked += 1;
            if looked > MAX_SCAN_ENTRIES {
                log::warn!(
                    "Stopped looking through {} after {} entries;",
                    root.display(),
                    MAX_SCAN_ENTRIES
                );
                log::warn!("  a service with a device trigger on this subsystem may not start");
                break;
            }
            let Some(properties) = read_sysfs_uevent(&entry.path()) else {
                continue;
            };
            if matches_device(rule, &rule.subsystem, &properties) {
                return Some(describe_device(&rule.subsystem, &properties));
            }
        }
    }
    None
}

/// What is watching for device triggers, as far as [`report_configuration`]
/// needs to know.
///
/// Three states rather than a boolean because the third produces a different
/// sentence and a different instruction. A machine that could not open the
/// uevent socket has something wrong with it and the operator should expect
/// the situation to be fixed; a session supervisor was never going to watch
/// the kernel's device broadcast -- it runs as somebody's login, the devices
/// on the machine are not its business, and a warning phrased as a fault
/// would send a person looking for a breakage that is a design decision.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Watching {
    /// The uevent socket is open and device rules will fire.
    Devices,
    /// It is not, and they will not.
    Nothing,
    /// This supervisor does not watch devices at all: `raven-init --user`.
    NotInThisMode,
}

/// Say, once at boot, what is demand-started and what will start it.
///
/// Two of these lines are warnings and the rest are not, and the split is the
/// point of the function. A service that is deliberately absent from the boot
/// is worth a line in the log because somebody will eventually look for it and
/// find it not running; a service that is deliberately absent and has *no way
/// of ever being started* is worth the console, because it is indistinguishable
/// from a broken daemon until somebody reads a definition carefully.
///
/// `watching` is what became of the uevent socket. When there is none, every
/// device rule on the machine is inert for this boot, and saying so once here
/// beside the affected service names is more use than the one line
/// [`Monitor::open`] printed about a socket.
pub fn report_configuration(config: &InitConfig, watching: Watching) {
    for cfg in config.services.iter().filter(|c| c.enabled) {
        let Some(demand) = cfg.demand.as_ref() else {
            continue;
        };
        let rules = demand.device.len();
        match (rules, demand.daemon.as_deref()) {
            (0, None) => {
                log::warn!(
                    "Service {} is marked started-on-demand but names no trigger;",
                    cfg.name
                );
                log::warn!("  it will start only for an explicit `raven-rc start {}`", cfg.name);
            }
            (0, Some(who)) => {
                log::info!(
                    "Service {} starts on demand, when {} asks for it",
                    cfg.name,
                    who
                );
            }
            (n, who) => match watching {
                Watching::Devices => {
                    log::info!(
                        "Service {} starts on demand, on {} device rule{}{}",
                        cfg.name,
                        n,
                        if n == 1 { "" } else { "s" },
                        match who {
                            Some(w) => format!(" or when {w} asks"),
                            None => String::new(),
                        }
                    );
                }
                Watching::Nothing => {
                    log::warn!(
                        "Service {} starts on demand, but the uevent socket is not open;",
                        cfg.name
                    );
                    log::warn!(
                        "  start it with `raven-rc start {}` until that is fixed",
                        cfg.name
                    );
                }
                Watching::NotInThisMode => {
                    log::warn!(
                        "Service {} names a device trigger, which a session supervisor",
                        cfg.name
                    );
                    log::warn!(
                        "  does not watch for; start it with `raven-rc --user start {}`",
                        cfg.name
                    );
                }
            },
        }
    }
}

/// Which of the names a trigger produced should actually be started.
///
/// Three services are dropped here rather than in the matching code, because
/// all three are facts about the supervisor rather than about the machine:
/// one that is already running (a second printer plugged into the same laptop
/// must not restart `cupsd` underneath the job it is printing), one an
/// operator stopped by hand (`raven-rc stop bluetoothd` means stopped, and a
/// device appearing is not an instruction to overrule somebody), and one whose
/// definition has since stopped being demand-started (a `raven-rc reload`
/// between the event and this call).
///
/// The list is returned rather than acted on so that the caller keeps its one
/// mutable borrow of the service map for the starting itself.
pub fn filter_startable(
    wanted: &[String],
    services: &HashMap<String, crate::service::Service>,
    config: &InitConfig,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in wanted {
        if out.iter().any(|n| n == name) {
            continue;
        }
        let Some(cfg) = config.services.iter().find(|c| &c.name == name) else {
            continue;
        };
        if !cfg.enabled || !cfg.is_demand_started() {
            continue;
        }
        if let Some(svc) = services.get(name) {
            if svc.is_running() {
                continue;
            }
            if svc.is_manually_stopped() {
                log::info!(
                    "Service {} not started on demand: it was stopped by request",
                    name
                );
                continue;
            }
        }
        out.push(name.clone());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DemandStart, ServiceConfig};

    fn props(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn printer_rule() -> DeviceTrigger {
        DeviceTrigger {
            subsystem: "usb".to_string(),
            devtype: Some("usb_interface".to_string()),
            properties: [("INTERFACE".to_string(), "7/*".to_string())]
                .into_iter()
                .collect(),
        }
    }

    #[test]
    fn a_kernel_uevent_is_parsed_into_an_action_and_its_properties() {
        let msg = b"add@/devices/pci0000:00/usb1/1-2/1-2:1.0\0\
                    ACTION=add\0SUBSYSTEM=usb\0DEVTYPE=usb_interface\0INTERFACE=7/1/4\0";
        let event = parse_uevent(msg).expect("a kernel uevent parses");
        assert_eq!(event.action, "add");
        assert_eq!(property(&event.properties, "SUBSYSTEM"), Some("usb"));
        assert_eq!(property(&event.properties, "INTERFACE"), Some("7/1/4"));
    }

    /// libudev re-broadcasts arrive on a group this monitor does not join, but
    /// a message with no `@` in its header must be ignored rather than have
    /// its first line treated as an action.
    #[test]
    fn a_message_that_is_not_a_kernel_uevent_is_ignored() {
        assert!(parse_uevent(b"libudev\0ACTION=add\0SUBSYSTEM=usb\0").is_none());
        assert!(parse_uevent(b"").is_none());
    }

    #[test]
    fn a_printer_rule_matches_a_printer_interface_and_not_a_hub() {
        let rule = printer_rule();
        assert!(matches_device(
            &rule,
            "usb",
            &props(&[("DEVTYPE", "usb_interface"), ("INTERFACE", "7/1/4")])
        ));
        // A USB hub: same subsystem, same devtype, class 9.
        assert!(!matches_device(
            &rule,
            "usb",
            &props(&[("DEVTYPE", "usb_interface"), ("INTERFACE", "9/0/0")])
        ));
        // The printer's own device node rather than its interface.
        assert!(!matches_device(
            &rule,
            "usb",
            &props(&[("DEVTYPE", "usb_device"), ("INTERFACE", "7/1/4")])
        ));
        // Right device, wrong subsystem.
        assert!(!matches_device(
            &rule,
            "bluetooth",
            &props(&[("DEVTYPE", "usb_interface"), ("INTERFACE", "7/1/4")])
        ));
    }

    /// A rule naming a property the device does not have must not match. It is
    /// the failure that is easiest to write by accident and hardest to see:
    /// the service simply never starts.
    #[test]
    fn a_rule_naming_a_property_the_device_lacks_does_not_match() {
        let rule = printer_rule();
        assert!(!matches_device(
            &rule,
            "usb",
            &props(&[("DEVTYPE", "usb_interface")])
        ));
    }

    #[test]
    fn a_trailing_star_matches_by_prefix_and_nothing_else_is_a_pattern() {
        assert!(matches_pattern("7/*", "7/1/4"));
        assert!(matches_pattern("7/*", "7/"));
        assert!(!matches_pattern("7/*", "9/0/0"));
        assert!(matches_pattern("7/1/4", "7/1/4"));
        // A `*` anywhere but the end is a literal asterisk, deliberately.
        assert!(!matches_pattern("7/*/4", "7/1/4"));
    }

    /// The whole point of the feature: a rule with no subsystem matches
    /// nothing at all, rather than everything.
    #[test]
    fn a_rule_with_no_subsystem_matches_nothing() {
        let rule = DeviceTrigger::default();
        assert!(!matches_device(&rule, "usb", &props(&[])));
        assert!(!matches_device(&rule, "", &props(&[])));
    }

    /// A sysfs `uevent` file has no SUBSYSTEM line -- the directory it was
    /// found in is what says which subsystem it is -- so the same rule has to
    /// match when the subsystem is supplied by the caller.
    #[test]
    fn a_sysfs_uevent_file_matches_the_same_rule_as_a_broadcast() {
        let dir = std::env::temp_dir().join(format!(
            "raven-demand-uevent-{}-{}",
            std::process::id(),
            line!()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        std::fs::write(
            dir.join("uevent"),
            "DEVTYPE=usb_interface\nDRIVER=usblp\nINTERFACE=7/1/2\nMODALIAS=usb:v03F0p0117\n",
        )
        .expect("write uevent");

        let properties = read_sysfs_uevent(&dir).expect("the file parses");
        assert!(matches_device(&printer_rule(), "usb", &properties));

        std::fs::remove_dir_all(&dir).ok();
    }

    fn demand_service(name: &str, demand: DemandStart) -> ServiceConfig {
        ServiceConfig {
            name: name.to_string(),
            exec: "/bin/true".to_string(),
            demand: Some(demand),
            ..ServiceConfig::default()
        }
    }

    #[test]
    fn a_service_with_no_demand_block_has_no_device_rules() {
        let plain = ServiceConfig {
            name: "dbus".to_string(),
            ..ServiceConfig::default()
        };
        assert!(device_rules(&plain).is_empty());
        assert!(!plain.is_demand_started());

        let triggered = demand_service(
            "ipp-usb",
            DemandStart {
                device: vec![printer_rule()],
                daemon: None,
            },
        );
        assert_eq!(device_rules(&triggered).len(), 1);
        assert!(triggered.is_demand_started());
    }

    /// An empty `[services.demand]` is still a demand-started service: it just
    /// has no trigger but `raven-rc start`. That is a legal thing to write and
    /// the thing `report_configuration` warns about.
    #[test]
    fn an_empty_demand_block_still_keeps_a_service_out_of_the_boot() {
        let cfg = demand_service("cupsd", DemandStart::default());
        assert!(cfg.is_demand_started());
        assert!(device_rules(&cfg).is_empty());
    }

    #[test]
    fn a_demand_block_round_trips_through_toml() {
        let text = r#"
            [[services]]
            name = "ipp-usb"
            exec = "/usr/bin/ipp-usb"

            [[services.demand.device]]
            subsystem = "usb"
            devtype = "usb_interface"
            properties = { INTERFACE = "7/1/4" }
        "#;
        let parsed: crate::config::InitConfig = toml::from_str(text).expect("parses");
        let svc = &parsed.services[0];
        assert!(svc.is_demand_started());
        let rules = device_rules(svc);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].subsystem, "usb");
        assert_eq!(rules[0].devtype.as_deref(), Some("usb_interface"));
        assert!(matches_device(
            &rules[0],
            "usb",
            &props(&[("DEVTYPE", "usb_interface"), ("INTERFACE", "7/1/4")])
        ));
    }

    /// `raven-rc stop` means stopped. A device appearing is not an argument
    /// with the person who typed it.
    ///
    /// The stopped service is built through `Service::adopt` rather than by
    /// starting one: a `ServiceSnapshot` with no pid and `manually_stopped`
    /// set is exactly the state being tested, and reaching it by forking a
    /// process would make a test about a decision into a test that also needs
    /// somewhere to put a log file.
    #[test]
    fn a_service_stopped_by_request_is_not_restarted_by_its_trigger() {
        use crate::service::{Service, ServiceSnapshot};

        let cfg = demand_service(
            "obexd",
            DemandStart {
                device: vec![],
                daemon: Some("raven-ports".to_string()),
            },
        );
        let config = InitConfig {
            services: vec![cfg.clone()],
            ..InitConfig::default_empty()
        };

        let mut services: HashMap<String, Service> = HashMap::new();
        let wanted = vec!["obexd".to_string()];
        assert_eq!(
            filter_startable(&wanted, &services, &config),
            vec!["obexd".to_string()],
            "a service that has never run is startable"
        );

        services.insert(
            "obexd".to_string(),
            Service::adopt(
                ServiceSnapshot {
                    config: cfg.clone(),
                    pid: None,
                    manually_stopped: true,
                    ..ServiceSnapshot::default()
                },
                cfg,
            ),
        );

        assert!(
            filter_startable(&wanted, &services, &config).is_empty(),
            "a service stopped by request stays stopped"
        );
    }

    /// The same name arriving twice in one burst of uevents -- a printer's
    /// interface and its device node, or two identical interfaces -- must
    /// produce one start.
    #[test]
    fn a_name_wanted_twice_is_started_once() {
        let cfg = demand_service(
            "ipp-usb",
            DemandStart {
                device: vec![printer_rule()],
                daemon: None,
            },
        );
        let config = InitConfig {
            services: vec![cfg],
            ..InitConfig::default_empty()
        };
        let services: HashMap<String, crate::service::Service> = HashMap::new();
        let wanted = vec!["ipp-usb".to_string(), "ipp-usb".to_string()];
        assert_eq!(
            filter_startable(&wanted, &services, &config),
            vec!["ipp-usb".to_string()]
        );
    }

    /// The coldplug half. A device that was already plugged in when init
    /// started has no broadcast to be found by, and if this walk did not find
    /// it the service would never start at all -- which is the failure this
    /// whole mechanism most needs not to have.
    ///
    /// Both directories the kernel uses are covered, because which one a
    /// device is in depends on its subsystem: a bluetooth adapter is under
    /// /sys/class and a USB interface is only ever under /sys/bus.
    #[test]
    fn a_device_already_plugged_in_is_found_by_the_startup_walk() {
        let sys = std::env::temp_dir().join(format!(
            "raven-demand-sys-{}-{}",
            std::process::id(),
            line!()
        ));
        let printer = sys.join("bus/usb/devices/2-1:1.0");
        std::fs::create_dir_all(&printer).expect("fake sysfs");
        std::fs::write(
            printer.join("uevent"),
            "DEVTYPE=usb_interface\nDRIVER=usblp\nINTERFACE=7/1/2\n",
        )
        .expect("write uevent");
        let hub = sys.join("bus/usb/devices/2-0:1.0");
        std::fs::create_dir_all(&hub).expect("fake sysfs");
        std::fs::write(hub.join("uevent"), "DEVTYPE=usb_interface\nINTERFACE=9/0/0\n")
            .expect("write uevent");
        let adapter = sys.join("class/bluetooth/hci0");
        std::fs::create_dir_all(&adapter).expect("fake sysfs");
        std::fs::write(adapter.join("uevent"), "DEVTYPE=host\n").expect("write uevent");

        let config = InitConfig {
            services: vec![
                demand_service(
                    "ipp-usb",
                    DemandStart {
                        device: vec![printer_rule()],
                        daemon: None,
                    },
                ),
                demand_service(
                    "bluetoothd",
                    DemandStart {
                        device: vec![DeviceTrigger {
                            subsystem: "bluetooth".to_string(),
                            devtype: Some("host".to_string()),
                            properties: HashMap::new(),
                        }],
                        daemon: None,
                    },
                ),
                demand_service(
                    "obexd",
                    DemandStart {
                        device: vec![DeviceTrigger {
                            subsystem: "sound".to_string(),
                            devtype: None,
                            properties: HashMap::new(),
                        }],
                        daemon: None,
                    },
                ),
            ],
            ..InitConfig::default_empty()
        };

        let mut found = already_present_in(&config, &sys);
        found.sort();
        assert_eq!(
            found,
            vec!["bluetoothd".to_string(), "ipp-usb".to_string()],
            "the printer and the adapter are there; nothing is a sound card"
        );

        std::fs::remove_dir_all(&sys).ok();
    }

    /// A subsystem with nothing in it, and a sysfs that is not there at all,
    /// are both ordinary answers rather than errors -- the common case on a
    /// laptop with no printer attached is every one of these directories being
    /// missing.
    #[test]
    fn a_subsystem_with_no_devices_starts_nothing() {
        let config = InitConfig {
            services: vec![demand_service(
                "ipp-usb",
                DemandStart {
                    device: vec![printer_rule()],
                    daemon: None,
                },
            )],
            ..InitConfig::default_empty()
        };
        assert!(already_present_in(&config, Path::new("/nonexistent/sys")).is_empty());
    }

    /// A name that is no longer demand-started -- the definition changed under
    /// a `raven-rc reload` between the event and the start -- is dropped
    /// rather than started behind the operator's back.
    #[test]
    fn a_name_that_is_no_longer_demand_started_is_dropped() {
        let plain = ServiceConfig {
            name: "cupsd".to_string(),
            exec: "/usr/bin/cupsd".to_string(),
            ..ServiceConfig::default()
        };
        let config = InitConfig {
            services: vec![plain],
            ..InitConfig::default_empty()
        };
        let services: HashMap<String, crate::service::Service> = HashMap::new();
        assert!(filter_startable(&["cupsd".to_string()], &services, &config).is_empty());
        assert!(filter_startable(&["nothing".to_string()], &services, &config).is_empty());
    }
}
