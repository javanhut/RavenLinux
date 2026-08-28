//! `raven-powerd` -- what the power button, the sleep button and the lid do.
//!
//! The kernel turns all three into ordinary evdev events and then stops. It
//! does not suspend on a lid close, and the only reason a bare Linux machine
//! powers off when you hold the power button is that the firmware cut the
//! rails. Something in userspace has to read those events and decide. On most
//! distributions that something is logind, or acpid running shell fragments;
//! here it is this, a few hundred lines with no bus, no scripts and one
//! configuration file.
//!
//! # Why a separate daemon
//!
//! It could have lived in PID 1 -- the main loop already ticks ten times a
//! second and could read a few file descriptors while it is there. It does not
//! for two reasons. Watching input devices means opening whatever `/dev/input`
//! happens to hold, including hot-plugged hardware, and that is a surface PID 1
//! should not grow. And this is *policy*: what the lid means is the kind of
//! thing a person edits and reloads, which wants a daemon you can restart, not
//! a process you cannot.
//!
//! The mechanism stays in init: this daemon decides, then asks over
//! `/run/raven-init.sock`, exactly as `raven-rc suspend` does.
//!
//! # Wake
//!
//! Waking is not this daemon's job and cannot be: nothing in userspace runs
//! while the machine is asleep. The power button and the lid wake the machine
//! because ACPI arms them as wakeup sources, and this daemon's only
//! contribution is to make sure they are actually armed -- see
//! [`arm_wakeup_source`]. After that, the press or the lid opening is a
//! firmware event, and Linux is running again before any of this code is.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;

/// Where raven-init listens. Must match control::SOCKET_PATH.
const SOCKET_PATH: &str = "/run/raven-init.sock";

/// Policy, if anyone wrote any down.
const CONFIG_PATH: &str = "/etc/raven/power.toml";

/// How often we look for input devices that were not there before.
///
/// Ten seconds because the thing this catches is a keyboard being plugged in,
/// and nobody plugs in a keyboard and reaches for its power key inside ten
/// seconds. Cheap enough that watching `/dev/input` with inotify would buy
/// nothing but another failure mode.
const RESCAN_INTERVAL: Duration = Duration::from_secs(10);

/// How long an action suppresses the next one.
///
/// This is the guard against the most obvious way for a power daemon to be
/// unusable: the press that *wakes* the machine is a real evdev event, queued
/// while we were frozen and delivered the moment we resume. Without this, every
/// wake immediately suspends again.
///
/// It works because [`Instant`] is `CLOCK_MONOTONIC`, which on Linux does not
/// advance while the machine is suspended. Five seconds measured on that clock
/// is five seconds of *awake* time, so the window opened just before the
/// suspend is still open just after the resume, however long the machine slept.
/// Anything measured on the wall clock would have expired mid-sleep and let the
/// wake press through.
const COOLDOWN: Duration = Duration::from_secs(5);

/// Longest we will wait on init for a reply. It answers from its main loop,
/// which ticks every 100ms.
const REPLY_TIMEOUT: Duration = Duration::from_secs(2);

// ---------------------------------------------------------------------------
// evdev
// ---------------------------------------------------------------------------
//
// `struct input_event` is a timeval followed by type, code and value. On a
// 64-bit kernel that is 8 + 8 + 2 + 2 + 4 = 24 bytes, and the layout is ABI --
// it is what `read(2)` on an event device returns and it does not change.

const EVENT_SIZE: usize = 24;

/// `EV_KEY` -- a key or button changed state.
const EV_KEY: u16 = 0x01;
/// `EV_SW` -- a switch changed state. The lid is one.
const EV_SW: u16 = 0x05;

/// `KEY_POWER`, the ACPI power button and the key of the same name on a
/// keyboard.
const KEY_POWER: u16 = 116;
/// `KEY_SLEEP`, the ACPI sleep button and Fn+F-something on many laptops.
const KEY_SLEEP: u16 = 142;
/// `SW_LID` -- 1 is closed, 0 is open.
const SW_LID: u16 = 0x00;
/// `KEY_A`. Nothing here cares about the letter A; it is the cheapest test for
/// "this device is a keyboard, not a button". See [`arm_wakeup_sources`].
const KEY_A: u16 = 30;

/// A key event's `value`: 0 release, 1 press, 2 autorepeat. We act on presses.
const KEY_PRESSED: i32 = 1;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// What a button or the lid should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum PowerAction {
    /// Suspend to RAM.
    Suspend,
    /// Orderly shutdown, then power off.
    Poweroff,
    /// Orderly shutdown, then reboot.
    Reboot,
    /// Do nothing. For a machine that lives on a desk with the lid shut.
    Ignore,
}

impl PowerAction {
    /// The word init's control socket knows this action by.
    fn request(self) -> Option<&'static str> {
        match self {
            PowerAction::Suspend => Some("suspend"),
            PowerAction::Poweroff => Some("poweroff"),
            PowerAction::Reboot => Some("reboot"),
            PowerAction::Ignore => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct Buttons {
    /// `KEY_POWER`. Sleep rather than power off, because a laptop's power
    /// button is pressed far more often to put the machine down for an hour
    /// than to shut it down, and the firmware still cuts power if you hold it.
    power: PowerAction,
    /// `KEY_SLEEP`.
    sleep: PowerAction,
}

impl Default for Buttons {
    fn default() -> Self {
        Self {
            power: PowerAction::Suspend,
            sleep: PowerAction::Suspend,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct Lid {
    /// What closing the lid does.
    ///
    /// There is no `open` counterpart, and there is nothing to configure for
    /// it: the machine is asleep when the lid opens, so the wake is ACPI's, and
    /// by the time anything here could have an opinion the screen is already
    /// coming back.
    close: PowerAction,
}

impl Default for Lid {
    fn default() -> Self {
        Self {
            close: PowerAction::Suspend,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct Config {
    buttons: Buttons,
    lid: Lid,
    /// Arm the power button and the lid as ACPI wakeup sources at start.
    ///
    /// On by default: a lid that sleeps the machine and then cannot wake it is
    /// worse than one that does nothing. Turn it off if you are managing
    /// `/proc/acpi/wakeup` by hand.
    manage_wakeup: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            buttons: Buttons::default(),
            lid: Lid::default(),
            manage_wakeup: true,
        }
    }
}

impl Config {
    /// Read the file, or fall back to the defaults.
    ///
    /// A missing file is normal and silent. A *broken* file is loud and still
    /// falls back, because the alternative is a daemon that refuses to start
    /// and a laptop whose lid stops working over a typo.
    fn load() -> Self {
        let text = match fs::read_to_string(CONFIG_PATH) {
            Ok(text) => text,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                log::info!("No {}; using defaults", CONFIG_PATH);
                return Self::default();
            }
            Err(e) => {
                log::warn!("Cannot read {}: {}; using defaults", CONFIG_PATH, e);
                return Self::default();
            }
        };

        match toml::from_str::<Config>(&text) {
            Ok(config) => config,
            Err(e) => {
                log::error!("{} is not valid: {}", CONFIG_PATH, e);
                log::error!("  Using defaults. Fix the file and restart raven-powerd.");
                Self::default()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// What the reader threads report
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Signal {
    PowerPressed,
    SleepPressed,
    /// `true` when the lid just closed.
    Lid(bool),
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = Config::load();
    log::info!(
        "raven-powerd: power={:?} sleep={:?} lid-close={:?}",
        config.buttons.power,
        config.buttons.sleep,
        config.lid.close
    );

    let (tx, rx) = mpsc::channel::<Signal>();
    // Held for the lifetime of the process so `recv` can never see every sender
    // dropped and give up on a machine that simply has no input devices yet.
    let device_tx = tx.clone();

    let watched: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));

    if config.manage_wakeup {
        arm_wakeup_sources();
    }

    let found = scan_devices(&device_tx, &watched);
    if found == 0 {
        // Not fatal. A machine with no power button and no lid is a virtual
        // machine, and it is allowed to run this daemon and do nothing.
        log::warn!(
            "No power button, sleep button or lid switch in /dev/input. \
             Nothing to watch (are we root?)"
        );
    }

    let mut lid_closed = false;
    let mut last_action: Option<Instant> = None;
    let mut last_scan = Instant::now();

    loop {
        match rx.recv_timeout(RESCAN_INTERVAL) {
            Ok(signal) => {
                let action = match signal {
                    Signal::PowerPressed => {
                        log::info!("Power button pressed");
                        Some(config.buttons.power)
                    }
                    Signal::SleepPressed => {
                        log::info!("Sleep button pressed");
                        Some(config.buttons.sleep)
                    }
                    // Only a transition counts. A driver that re-announces
                    // "closed" on resume -- and several do -- must not put the
                    // machine straight back to sleep.
                    Signal::Lid(true) if !lid_closed => {
                        lid_closed = true;
                        log::info!("Lid closed");
                        Some(config.lid.close)
                    }
                    Signal::Lid(true) => None,
                    Signal::Lid(false) => {
                        lid_closed = false;
                        log::info!("Lid opened");
                        None
                    }
                };

                if let Some(action) = action {
                    if let Some(at) = last_action {
                        if at.elapsed() < COOLDOWN {
                            log::debug!("Ignored: within {:?} of the last action", COOLDOWN);
                            continue;
                        }
                    }
                    if perform(action) {
                        last_action = Some(Instant::now());
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                // Impossible while `tx` is alive, and this is what keeps it so.
                let _ = &tx;
                break;
            }
        }

        if last_scan.elapsed() >= RESCAN_INTERVAL {
            scan_devices(&device_tx, &watched);
            last_scan = Instant::now();
        }
    }
}

/// Carry out one action. Returns whether it was actually requested.
fn perform(action: PowerAction) -> bool {
    let Some(request) = action.request() else {
        log::info!("Configured to ignore this");
        return false;
    };

    log::info!("Asking init to {}", request);

    match ask_init(request) {
        Ok(reply) if reply_is_error(&reply) => {
            // An init too old to know the verb answers "error: unknown
            // command 'suspend'". It is reachable, so this is not the socket
            // failing -- but the machine is no more asleep than if it were,
            // and falling through to the same recovery is what makes an
            // upgraded raven-powerd work against an init that has not been
            // restarted yet.
            log::error!("raven-init refused: {}", reply.trim());
            if action == PowerAction::Suspend {
                log::warn!("Suspending directly; sleep hooks will not run");
                return suspend_directly();
            }
            false
        }
        Ok(reply) => {
            log::info!("init: {}", reply.trim());
            true
        }
        Err(e) => {
            log::error!("Could not reach raven-init: {}", e);
            // Only suspend has a safe fallback. Powering the machine off
            // behind init's back skips the service stop, the filesystem
            // quiesce and every shutdown hook -- that is a corrupted
            // filesystem, not a graceful degradation, so it does not happen.
            if action == PowerAction::Suspend {
                log::warn!("Suspending directly; sleep hooks will not run");
                return suspend_directly();
            }
            log::error!("  {} needs init; not doing it by hand", request);
            false
        }
    }
}

/// Did init refuse?
///
/// The control protocol is text for people, so this is a prefix check rather
/// than a status code. Every refusal `control.rs` produces begins with
/// `error:`, and every success begins with a verb in the present participle
/// -- "Suspending", "Powering off".
fn reply_is_error(reply: &str) -> bool {
    reply.trim_start().starts_with("error")
}

/// Send one request to init and return its reply.
fn ask_init(request: &str) -> Result<String, std::io::Error> {
    let mut stream = UnixStream::connect(SOCKET_PATH)?;
    stream.set_write_timeout(Some(REPLY_TIMEOUT))?;
    stream.set_read_timeout(Some(REPLY_TIMEOUT))?;

    stream.write_all(request.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;

    let mut reply = String::new();
    // init closes the stream after answering, so a short read is the end of
    // the message and not a truncation.
    stream.read_to_string(&mut reply)?;
    Ok(reply)
}

/// The write init would have done, for when init is not there to do it.
fn suspend_directly() -> bool {
    const STATE_PATH: &str = "/sys/power/state";

    let Ok(offered) = fs::read_to_string(STATE_PATH) else {
        log::error!("Cannot read {}: no suspend support in this kernel", STATE_PATH);
        return false;
    };

    let Some(state) = ["mem", "freeze"]
        .into_iter()
        .find(|s| offered.split_whitespace().any(|o| o == *s))
    else {
        log::error!("{} offers '{}', no state we use", STATE_PATH, offered.trim());
        return false;
    };

    unsafe { libc::sync() };

    match fs::write(STATE_PATH, state) {
        Ok(()) => true,
        Err(e) => {
            log::error!("Suspend refused: {}", e);
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Devices
// ---------------------------------------------------------------------------

/// Open every input device that reports a power key, a sleep key or a lid, and
/// leave a thread reading each one. Returns how many are being watched.
fn scan_devices(tx: &Sender<Signal>, watched: &Arc<Mutex<HashSet<PathBuf>>>) -> usize {
    let Ok(entries) = fs::read_dir("/dev/input") else {
        log::warn!("No /dev/input; is udev running?");
        return 0;
    };

    let mut nodes: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("event"))
        })
        .collect();
    // Deterministic order, so the log reads the same way on every boot.
    nodes.sort();

    for node in nodes {
        {
            let open = watched.lock().unwrap_or_else(|e| e.into_inner());
            if open.contains(&node) {
                continue;
            }
        }

        let Some(interest) = device_interest(&node) else {
            continue;
        };

        let file = match fs::File::open(&node) {
            Ok(file) => file,
            Err(e) => {
                log::warn!("Cannot open {:?}: {}", node, e);
                continue;
            }
        };

        log::info!(
            "Watching {:?} ({}){}",
            node,
            device_name(&node).unwrap_or_else(|| "unnamed".to_string()),
            if interest.lid { " [lid]" } else { "" }
        );

        watched
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(node.clone());

        let tx = tx.clone();
        let watched = Arc::clone(watched);
        let path = node.clone();
        thread::Builder::new()
            .name("powerd-input".to_string())
            // A thread that does nothing but block in read(2) on 24-byte
            // records has no use for the default 2 MiB, and there is one per
            // device.
            .stack_size(64 * 1024)
            .spawn(move || {
                read_device(file, &path, &tx);
                // Dropped from the set on the way out, so a device that comes
                // back -- a keyboard replugged, a driver reloaded -- is picked
                // up again by the next scan.
                watched
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&path);
                log::info!("Stopped watching {:?}", path);
            })
            .ok();
    }

    watched.lock().unwrap_or_else(|e| e.into_inner()).len()
}

/// What a device can tell us. A device that can tell us nothing is not opened.
struct Interest {
    lid: bool,
    /// A general keyboard rather than a dedicated button. Watched the same
    /// way, treated differently when arming wakeup sources.
    keyboard: bool,
}

/// Decide from sysfs, before opening anything, whether a device is worth a
/// file descriptor and a thread.
///
/// The capability bitmaps are the kernel's own answer to "what can this device
/// report", so this needs no device names and no quirk list: a keyboard with a
/// power key is included because it says it has one, and the sixteen audio
/// jack-detect devices on a modern laptop are excluded because they say they
/// have not.
fn device_interest(node: &Path) -> Option<Interest> {
    let name = node.file_name()?.to_str()?;
    let caps = PathBuf::from("/sys/class/input").join(name).join("device/capabilities");

    let keys = fs::read_to_string(caps.join("key")).unwrap_or_default();
    let switches = fs::read_to_string(caps.join("sw")).unwrap_or_default();

    let has_buttons = bit_is_set(&keys, KEY_POWER) || bit_is_set(&keys, KEY_SLEEP);
    let has_lid = bit_is_set(&switches, SW_LID);

    if has_buttons || has_lid {
        Some(Interest {
            lid: has_lid,
            keyboard: bit_is_set(&keys, KEY_A),
        })
    } else {
        None
    }
}

/// The device's human name, for the log.
fn device_name(node: &Path) -> Option<String> {
    let name = node.file_name()?.to_str()?;
    fs::read_to_string(format!("/sys/class/input/{}/device/name", name))
        .ok()
        .map(|n| n.trim().to_string())
}

/// Is bit `bit` set in a sysfs capability bitmap?
///
/// The format is a run of hex words, least significant *last*, e.g.
/// `"10000 0 0 ... 0"`. Written by hand rather than pulled from a crate
/// because it is eight lines and this crate is PID 1's.
fn bit_is_set(bitmap: &str, bit: u16) -> bool {
    let words: Vec<&str> = bitmap.split_whitespace().collect();
    if words.is_empty() {
        return false;
    }

    let bit = usize::from(bit);
    let index_from_end = bit / 64;
    if index_from_end >= words.len() {
        return false;
    }

    let word = words[words.len() - 1 - index_from_end];
    u64::from_str_radix(word, 16)
        .map(|value| value >> (bit % 64) & 1 == 1)
        .unwrap_or(false)
}

/// Block reading one device, forwarding the events we care about.
///
/// Returns when the device goes away, which for a hot-plugged keyboard is
/// `ENODEV` on the next read.
fn read_device(mut file: fs::File, path: &Path, tx: &Sender<Signal>) {
    // The kernel returns whole events and will fill as many as fit, so reading
    // a batch costs one syscall for a burst instead of one per event.
    let mut buffer = [0u8; EVENT_SIZE * 16];

    loop {
        let read = match file.read(&mut buffer) {
            Ok(0) => return,
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => {
                log::debug!("{:?}: {}", path, e);
                return;
            }
        };

        // `as_chunks` rather than `chunks_exact`: the size is a constant, so
        // the compiler gets fixed-size arrays and the remainder -- a partial
        // record the kernel never produces -- is handed back separately
        // instead of being silently dropped by the iterator.
        let (events, _partial) = buffer[..read].as_chunks::<EVENT_SIZE>();
        for chunk in events {
            // Past the timeval: type, code, value, native-endian.
            let kind = u16::from_ne_bytes([chunk[16], chunk[17]]);
            let code = u16::from_ne_bytes([chunk[18], chunk[19]]);
            let value = i32::from_ne_bytes([chunk[20], chunk[21], chunk[22], chunk[23]]);

            let signal = match (kind, code) {
                (EV_KEY, KEY_POWER) if value == KEY_PRESSED => Signal::PowerPressed,
                (EV_KEY, KEY_SLEEP) if value == KEY_PRESSED => Signal::SleepPressed,
                (EV_SW, SW_LID) => Signal::Lid(value != 0),
                _ => continue,
            };

            if tx.send(signal).is_err() {
                // The main thread is gone; so is the reason to keep reading.
                return;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Wakeup sources
// ---------------------------------------------------------------------------

/// Make sure everything we suspend *from* can also wake us.
///
/// For each device we are willing to watch, walk up sysfs to the first
/// ancestor with a `power/wakeup` attribute -- for the lid that is the
/// `PNP0C0D` ACPI node two levels above the input device -- and enable it.
/// Writing `enabled` is idempotent; this is not `/proc/acpi/wakeup`, which
/// toggles and would turn wake *off* on the second boot.
///
/// Keyboards are excluded, and that exclusion is what [`KEY_A`] is for. A
/// laptop keyboard advertises `KEY_POWER`, so without the test this would arm
/// `i8042` -- and an i8042 armed as a wakeup source is a well-known way to end
/// up with a machine that resumes a second after it suspends, because the
/// controller signals on its own during the transition. The three devices that
/// matter here are the ACPI power button, the sleep button and the lid, and
/// none of them is a keyboard.
fn arm_wakeup_sources() {
    let Ok(entries) = fs::read_dir("/dev/input") else {
        return;
    };

    let mut armed: HashMap<PathBuf, bool> = HashMap::new();

    for entry in entries.filter_map(|e| e.ok()) {
        let node = entry.path();
        let is_event = node
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("event"));
        // Buttons and the lid only; see this function's documentation.
        let arm = match device_interest(&node) {
            Some(interest) => !interest.keyboard,
            None => false,
        };
        if !is_event || !arm {
            continue;
        }

        let Some(name) = node.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let sysfs = PathBuf::from("/sys/class/input").join(name).join("device");
        let Ok(sysfs) = fs::canonicalize(&sysfs) else {
            continue;
        };

        if let Some((attribute, state)) = arm_wakeup_source(&sysfs) {
            armed.insert(attribute, state);
        }
    }

    for (attribute, was_already_on) in armed {
        if was_already_on {
            log::debug!("Wakeup already enabled: {:?}", attribute);
        } else {
            log::info!("Enabled as a wakeup source: {:?}", attribute);
        }
    }
}

/// Enable the nearest `power/wakeup` at or above `start`.
///
/// Returns the attribute touched and whether it was already on, or `None` if
/// no ancestor has one -- which is the normal answer for, say, a USB keyboard
/// on a port that is not wake-capable.
fn arm_wakeup_source(start: &Path) -> Option<(PathBuf, bool)> {
    // Four levels is enough for input -> inputN -> the ACPI or platform device
    // -> its bus, and short enough that a symlink loop cannot cost anything.
    let mut current = Some(start);

    for _ in 0..4 {
        let dir = current?;
        let attribute = dir.join("power/wakeup");

        if let Ok(state) = fs::read_to_string(&attribute) {
            let state = state.trim();
            // "disabled" means capable but off. A device that cannot wake the
            // machine has no file here at all, so there is nothing to force.
            if state == "enabled" {
                return Some((attribute, true));
            }
            if let Err(e) = fs::write(&attribute, "enabled") {
                log::warn!("Cannot arm {:?}: {}", attribute, e);
                return None;
            }
            return Some((attribute, false));
        }

        current = dir.parent();
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // The bitmaps below are the shape sysfs actually produces: hex words,
    // space separated, least significant word last.

    #[test]
    fn bit_zero_is_the_last_word() {
        assert!(bit_is_set("0 0 1", 0));
        assert!(!bit_is_set("1 0 0", 0));
    }

    #[test]
    fn a_lid_switch_advertises_sw_lid() {
        // What /sys/class/input/event0/device/capabilities/sw reads on a laptop.
        assert!(bit_is_set("1", SW_LID));
        assert!(!bit_is_set("0", SW_LID));
    }

    #[test]
    fn key_power_is_found_in_its_own_word() {
        // KEY_POWER is 116, so word 1 counting from the end, bit 52.
        let bitmap = format!("{:x} 0", 1u64 << 52);
        assert!(bit_is_set(&bitmap, KEY_POWER));
        assert!(!bit_is_set(&bitmap, KEY_SLEEP));
    }

    #[test]
    fn a_short_bitmap_is_not_read_past_its_end() {
        assert!(!bit_is_set("1", KEY_POWER));
        assert!(!bit_is_set("", KEY_POWER));
    }

    #[test]
    fn rubbish_in_a_bitmap_is_not_a_panic() {
        assert!(!bit_is_set("zzzz", 0));
    }

    #[test]
    fn a_keyboard_is_told_apart_from_a_button() {
        // The AT keyboard's bitmap claims KEY_A; the ACPI power button's does
        // not. Both are watched; only the second is armed to wake the machine.
        // Least significant word last: KEY_POWER (116) lands in the middle
        // word, KEY_A (30) in the last one.
        let keyboard = format!("0 {:x} {:x}", 1u64 << 52, 1u64 << 30);
        let button = format!("0 {:x} 0", 1u64 << 52);
        assert!(bit_is_set(&keyboard, KEY_A));
        assert!(!bit_is_set(&button, KEY_A));
        assert!(bit_is_set(&keyboard, KEY_POWER));
        assert!(bit_is_set(&button, KEY_POWER));
    }

    #[test]
    fn defaults_sleep_rather_than_power_off() {
        let config = Config::default();
        assert_eq!(config.buttons.power, PowerAction::Suspend);
        assert_eq!(config.lid.close, PowerAction::Suspend);
        assert!(config.manage_wakeup);
    }

    #[test]
    fn config_overrides_only_what_it_names() {
        let config: Config = toml::from_str("[lid]\nclose = \"ignore\"\n").unwrap();
        assert_eq!(config.lid.close, PowerAction::Ignore);
        // Untouched by a file that says nothing about it.
        assert_eq!(config.buttons.power, PowerAction::Suspend);
    }

    /// The file this repo ships must deserialize with the schema the daemon
    /// actually uses. Without this, a renamed key is only discovered on a
    /// machine whose lid has stopped working.
    #[test]
    fn the_shipped_config_parses() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../etc/raven/power.toml");
        let text = std::fs::read_to_string(&path).expect("etc/raven/power.toml is readable");
        let config: Config = toml::from_str(&text).expect("etc/raven/power.toml parses");

        // And it must say what its comments claim it says.
        assert_eq!(config.buttons.power, PowerAction::Suspend);
        assert_eq!(config.buttons.sleep, PowerAction::Suspend);
        assert_eq!(config.lid.close, PowerAction::Suspend);
        assert!(config.manage_wakeup);
    }

    #[test]
    fn a_refusal_is_told_apart_from_an_acknowledgement() {
        // What control.rs actually sends back, both ways.
        assert!(reply_is_error("error: unknown command 'suspend'\n"));
        assert!(!reply_is_error("Suspending\n"));
        assert!(!reply_is_error("Powering off\n"));
    }

    #[test]
    fn every_action_but_ignore_has_a_request() {
        assert_eq!(PowerAction::Suspend.request(), Some("suspend"));
        assert_eq!(PowerAction::Poweroff.request(), Some("poweroff"));
        assert_eq!(PowerAction::Reboot.request(), Some("reboot"));
        assert_eq!(PowerAction::Ignore.request(), None);
    }
}
