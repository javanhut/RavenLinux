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
//! # The desktop's socket
//!
//! Buttons and the lid are not the only way a person asks a laptop to sleep;
//! a quick-settings panel wants a "Suspend" row too, and the session it runs
//! in is an unprivileged user with no business talking to PID 1. So this
//! daemon listens a second time, on `/run/raven-power/ctl`, for the desktop.
//! That socket is group `video` and mode 0660 -- `video` because the session
//! already holds it for the DRM device, so it names exactly "whoever owns the
//! screen" without inventing a group -- and it takes one line. `suspend`,
//! `hibernate`, `poweroff` and `reboot` go down the same [`perform`] path as a
//! lid close.
//! `profile performance|balanced|power-saver` holds the machine in one preset
//! regardless of the supply, until `profile auto` hands the choice back to the
//! config or the daemon restarts; a bare `profile` asks what the machine is in
//! and why. Init's own socket stays root-only: what the desktop gets is a
//! verb into the daemon that already decides these things, never a channel
//! into PID 1.
//!
//! What was last *applied* is also published at `/run/raven-power/profile`,
//! world-readable, so a bar that only wants to paint a leaf when the machine
//! is in `power-saver` can read one word instead of holding the socket.
//!
//! # Wake
//!
//! Waking is not this daemon's job and cannot be: nothing in userspace runs
//! while the machine is asleep. The power button and the lid wake the machine
//! because ACPI arms them as wakeup sources, and this daemon's only
//! contribution is to make sure they are actually armed -- see
//! [`arm_wakeup_source`]. After that, the press or the lid opening is a
//! firmware event, and Linux is running again before any of this code is.
//!
//! # The command line
//!
//! With no arguments this is the daemon, which is how `init.toml` starts it
//! and is the only way it has ever been run. With arguments it is instead a
//! client of its own control socket -- `raven-powerd status` prints the
//! preset in force and what the buttons and the lid are set to do, and
//! `raven-powerd profile [<preset>|auto]` asks and answers the same question
//! the desktop panel does -- and it exits as soon as it has an answer.
//!
//! It is here rather than in a binary of its own because the alternative was
//! to have no way at all to ask a machine its power policy from a terminal.
//! `/usr/bin/raven-power` is a GTK window from a separate repository, not a
//! command: run it without arguments and it opens on the screen and stays
//! open, which from a terminal is indistinguishable from a hang and is what
//! sent somebody looking for one. It was never going to answer `raven-power`
//! the way `raven-rc` answers, and a settings window is the wrong place to
//! look when the question is why a machine on mains is running at 800MHz.
//!
//! Every client path in here is bounded. A daemon that is not running, a
//! socket left behind by one that died, and a daemon that has stopped
//! answering are three different messages and none of them is a wait -- see
//! [`ask_powerd`], which is where the bound lives and why it is a thread
//! rather than a socket option.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;

mod profile;
mod sleepmark;

/// Where raven-init listens. Must match control::SOCKET_PATH.
const SOCKET_PATH: &str = "/run/raven-init.sock";

/// Where the desktop asks us. Under the directory init already publishes the
/// sleep marker in (see `sleepmark::RUN_DIR`), so a session that watches one can
/// find the other without a second path to know.
const CTL_SOCKET_PATH: &str = "/run/raven-power/ctl";

/// The group that may write to [`CTL_SOCKET_PATH`]. The same group the session
/// needs for the DRM device, so owning the screen and being allowed to put it
/// to sleep are one fact, not two.
const CTL_GROUP: &str = "video";

/// Where the profile that was last applied is published, one word, for
/// anything that only wants to read it. Written by [`publish_profile`];
/// world-readable, root-writable, and gone after a boot like the rest of the
/// directory -- its absence means no profile has been applied.
const PROFILE_MARKER: &str = "/run/raven-power/profile";

/// Longest request we will read from the desktop. The longest valid one is
/// `profile power-saver` -- nineteen bytes plus a newline; the cap is for a
/// client that never stops.
const CTL_MAX_REQUEST: u64 = 256;

/// How long one desktop client may take to say its word. The panel writes the
/// line in the same call that connects, so this is only ever hit by something
/// that connected and then stalled.
const CTL_CLIENT_TIMEOUT: Duration = Duration::from_millis(500);

/// Policy, if anyone wrote any down.
const CONFIG_PATH: &str = "/etc/raven/power.toml";

/// How often we look for input devices that were not there before.
///
/// Ten seconds because the thing this catches is a keyboard being plugged in,
/// and nobody plugs in a keyboard and reaches for its power key inside ten
/// seconds. Cheap enough that watching `/dev/input` with inotify would buy
/// nothing but another failure mode.
const RESCAN_INTERVAL: Duration = Duration::from_secs(10);

/// How often we look at the power supply.
///
/// Pulling the cord is a uevent, and udev would tell us, but a netlink
/// listener is more machinery than reading two sysfs files every few
/// seconds costs. Three seconds is well inside how long it takes to notice
/// a laptop feels different. This is also what re-applies the profile after
/// a resume, where firmware may have put the platform back the way it likes.
const SUPPLY_POLL: Duration = Duration::from_secs(3);

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

/// Longest the command line waits for the daemon, at every step of the
/// exchange and for the exchange as a whole.
///
/// Three seconds rather than one because the daemon answers from its own main
/// loop, and that loop can be inside [`REPLY_TIMEOUT`] of its own -- two
/// seconds waiting on init for a suspend somebody asked for a moment earlier.
/// A client that gave up in one second would report a wedged daemon every
/// time a person pressed the power button and then typed a command.
///
/// Three seconds rather than ten because this is a command somebody is
/// waiting on at a prompt. Past about three seconds the honest answer is that
/// the daemon is not answering, and saying so is more use than another seven
/// seconds of nothing.
const CLI_TIMEOUT: Duration = Duration::from_secs(3);

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
    /// Write the session to swap and power off, to be resumed at the next
    /// boot.
    ///
    /// Not the default for anything and never will be. It is here because a
    /// machine that hibernates on a lid close is a real configuration -- a
    /// laptop carried in a bag all day survives it and a suspended one does
    /// not -- and until this existed there was no way to ask for it at all.
    /// Whether the machine can actually resume is init's question, asked in
    /// `power::hibernate`; a `close = "hibernate"` on a machine with no resume
    /// device is refused there and logged, and the lid does nothing.
    Hibernate,
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
            PowerAction::Hibernate => Some("hibernate"),
            PowerAction::Poweroff => Some("poweroff"),
            PowerAction::Reboot => Some("reboot"),
            PowerAction::Ignore => None,
        }
    }

    /// What init says back once it has agreed. Kept in step with the replies
    /// `control.rs` produces, so a client of the desktop socket reads the same
    /// words a client of init's would.
    fn acknowledgement(self) -> &'static str {
        match self {
            PowerAction::Suspend => "Suspending\n",
            PowerAction::Hibernate => "Hibernating\n",
            PowerAction::Poweroff => "Powering off\n",
            PowerAction::Reboot => "Rebooting\n",
            PowerAction::Ignore => "Ignored\n",
        }
    }

    /// What this action does, for a person reading `raven-powerd status`.
    ///
    /// Not [`PowerAction::request`]: that is the protocol's word and `Ignore`
    /// has none, which is exactly the case a status line most needs to state
    /// out loud. A lid configured to do nothing is the commonest reason
    /// somebody comes looking, and "nothing" is the answer, not a blank.
    fn describe(self) -> &'static str {
        match self {
            PowerAction::Suspend => "suspend",
            PowerAction::Hibernate => "hibernate",
            PowerAction::Poweroff => "power off",
            PowerAction::Reboot => "reboot",
            PowerAction::Ignore => "nothing",
        }
    }

    /// One line from the desktop socket, or `None` if it is not one of the
    /// four words.
    ///
    /// Stricter than init's parser on purpose: there is no `sleep` alias and
    /// no `halt`, because the only client is a panel with a handful of fixed
    /// rows, and every spelling accepted here is one more the panel could send
    /// by mistake. `Ignore` is not a request either -- it is a thing a config
    /// says, not a thing a person asks for.
    fn parse_request(line: &str) -> Option<Self> {
        match line.trim() {
            "suspend" => Some(PowerAction::Suspend),
            "hibernate" => Some(PowerAction::Hibernate),
            "poweroff" => Some(PowerAction::Poweroff),
            "reboot" => Some(PowerAction::Reboot),
            _ => None,
        }
    }
}

/// One line from the desktop socket, in full.
#[derive(Debug)]
enum CtlRequest {
    /// `suspend`, `hibernate`, `poweroff` or `reboot`: the words the socket
    /// takes as an instruction to change the machine's power state.
    Action(PowerAction),
    /// `profile <preset>`: hold the machine in one preset whatever the supply
    /// says, until `profile auto` or a restart of this daemon. Deliberately
    /// not persisted: a choice made from a panel is a choice about *now*, and
    /// a machine that boots into last Tuesday's "performance" is a machine
    /// that runs hot for no reason anyone remembers.
    SetProfile(profile::Preset),
    /// `profile auto`: back to the config's ac/battery mapping.
    ProfileAuto,
    /// `profile`, bare: what is the machine in right now, and why.
    ProfileQuery,
}

impl CtlRequest {
    /// Same strictness as [`PowerAction::parse_request`], for the same reason:
    /// every spelling accepted here is one more a client could send by
    /// mistake. `profile` takes exactly zero or one word, and the word is a
    /// preset's public name or `auto`.
    fn parse(line: &str) -> Option<Self> {
        let mut words = line.split_whitespace();
        match (words.next(), words.next(), words.next()) {
            (Some("profile"), None, None) => Some(CtlRequest::ProfileQuery),
            (Some("profile"), Some("auto"), None) => Some(CtlRequest::ProfileAuto),
            (Some("profile"), Some(word), None) => {
                profile::Preset::parse(word).map(CtlRequest::SetProfile)
            }
            _ => PowerAction::parse_request(line).map(CtlRequest::Action),
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
    /// The CPU and platform power policy on mains and on battery. See
    /// [`profile`].
    profile: profile::Profile,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            buttons: Buttons::default(),
            lid: Lid::default(),
            manage_wakeup: true,
            profile: profile::Profile::default(),
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

#[derive(Debug)]
enum Signal {
    PowerPressed,
    SleepPressed,
    /// `true` when the lid just closed.
    Lid(bool),
    /// The desktop asked, over [`CTL_SOCKET_PATH`]. The stream comes with it
    /// because the reply is the main loop's to write: only it knows whether
    /// the cooldown applies, what the profile override is, and only it
    /// performs the action.
    Request(CtlRequest, UnixStream),
}

/// Argument dispatch, and the one place that decides whether this process is
/// a daemon or a command.
///
/// No arguments is the daemon, unconditionally and forever: `init.toml` execs
/// `/usr/bin/raven-powerd` with an empty `args` list, so anything else here
/// would be a machine whose lid stops working the first time somebody adds a
/// verb. Every command is therefore a word, and there is no bare `raven-powerd`
/// that prints status -- see [`USAGE`], which says so to the person who
/// expected one.
///
/// The logger is set up differently on the two paths. The daemon logs at
/// `info` into its service log, as it always has. A command gets `warn`, so
/// that a `power.toml` which does not parse is still complained about on
/// stderr -- [`Config::load`] reports that at `error` -- while the "no
/// /etc/raven/power.toml; using defaults" line, which is an `info` and is
/// normal, stays out of the way of the output the person asked for.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let invocation = invocation(&args);

    if invocation == Invocation::Daemon {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
        run_daemon();
        return;
    }

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    std::process::exit(run_command(invocation));
}

fn run_daemon() {
    let config = Config::load();
    log::info!(
        "raven-powerd: power={:?} sleep={:?} lid-close={:?} profile={}",
        config.buttons.power,
        config.buttons.sleep,
        config.lid.close,
        if config.profile.manage {
            format!("ac:{:?}/battery:{:?}", config.profile.ac, config.profile.battery)
        } else {
            "unmanaged".to_string()
        }
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

    listen_for_desktop(&tx);

    let mut lid_closed = false;
    let mut last_action: Option<Instant> = None;
    let mut last_scan = Instant::now();

    // The profile is applied at start and then whenever the supply changes,
    // and re-applied at every poll regardless: the writes are no-ops when
    // nothing has moved, and after a resume something usually has. A desktop
    // override rides the same poll, which is what keeps it stuck through a
    // resume that undid it.
    let mut supply: Option<profile::Supply> = None;
    let mut last_supply_poll = Instant::now() - SUPPLY_POLL;
    let mut profile_override: Option<profile::Preset> = None;
    let mut published_profile: Option<profile::Preset> = None;

    loop {
        let managing = config.profile.manage || profile_override.is_some();
        if managing && last_supply_poll.elapsed() >= SUPPLY_POLL {
            let now = profile::supply();
            if supply != Some(now) {
                log::info!(
                    "On {}",
                    match now {
                        profile::Supply::Mains => "mains",
                        profile::Supply::Battery => "battery",
                    }
                );
                supply = Some(now);
            }
            let preset = profile_override.unwrap_or(match now {
                profile::Supply::Mains => config.profile.ac,
                profile::Supply::Battery => config.profile.battery,
            });
            profile::apply(preset);
            publish_profile(preset, &mut published_profile);
            last_supply_poll = Instant::now();
        }

        let wait = if managing { SUPPLY_POLL } else { RESCAN_INTERVAL };
        match rx.recv_timeout(wait) {
            Ok(Signal::Request(request, stream)) => {
                serve_ctl(
                    request,
                    stream,
                    &config.profile,
                    &mut last_action,
                    &mut profile_override,
                    &mut published_profile,
                );
            }
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
                    // Taken by the arm above; here only so the match is total.
                    Signal::Request(..) => None,
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
        log::error!(
            "Cannot read {}: no suspend support in this kernel",
            STATE_PATH
        );
        return false;
    };

    let Some(state) = ["mem", "freeze"]
        .into_iter()
        .find(|s| offered.split_whitespace().any(|o| o == *s))
    else {
        log::error!(
            "{} offers '{}', no state we use",
            STATE_PATH,
            offered.trim()
        );
        return false;
    };

    // The same handshake init does: without it the fallback is a suspend
    // with the desktop still on the glass, shown again the moment the lid
    // opens. See `sleepmark`.
    sleepmark::prepare();

    unsafe { libc::sync() };

    let result = fs::write(STATE_PATH, state);
    sleepmark::awake();

    match result {
        Ok(()) => true,
        Err(e) => {
            log::error!("Suspend refused: {}", e);
            false
        }
    }
}

// ---------------------------------------------------------------------------
// The desktop's socket
// ---------------------------------------------------------------------------

/// Bind [`CTL_SOCKET_PATH`] and leave a thread accepting on it.
///
/// A thread rather than a non-blocking accept from the main loop, because the
/// main loop is not a poll over file descriptors: it blocks in `recv_timeout`
/// on the channel the device threads feed, for up to [`RESCAN_INTERVAL`] at
/// a time. A socket checked only when that returns would answer a panel ten
/// seconds late. So the socket is one more feeder of the same channel, and the
/// accept thread does exactly what a device thread does -- block, decode, send
/// -- with the connection riding along in the [`Signal`] so the main loop can
/// answer it. Policy stays where it was: the cooldown and [`perform`] run on
/// the main thread for a desktop request as they do for a lid close.
///
/// Failing to bind is logged and not fatal: the buttons and the lid still work
/// on a machine whose quick settings cannot sleep it, and that is better than
/// the other way round.
fn listen_for_desktop(tx: &Sender<Signal>) {
    let listener = match bind_ctl_socket(CTL_SOCKET_PATH) {
        Ok(listener) => listener,
        Err(e) => {
            log::warn!("Not listening on {}: {}", CTL_SOCKET_PATH, e);
            return;
        }
    };

    let tx = tx.clone();
    thread::Builder::new()
        .name("powerd-ctl".to_string())
        .stack_size(64 * 1024)
        .spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => accept_desktop_client(stream, &tx),
                    Err(e) => log::warn!("{}: accept failed: {}", CTL_SOCKET_PATH, e),
                }
            }
        })
        .ok();
}

/// Create the socket file with the ownership the module docs describe.
///
/// Group first, mode second: between `bind` and `set_permissions` the file
/// carries the umask's mode, which is at most 0755 and admits nobody to
/// write. Tightening to 0660 after the chown means there is no moment at
/// which the socket is writable by a group it is not yet owned by.
fn bind_ctl_socket(path: &str) -> std::io::Result<UnixListener> {
    // Normally made by init when it publishes the marker at boot, but this
    // daemon is a service and need not start second. The same 0755 init uses,
    // so whichever of the two gets there first leaves the same directory.
    if let Some(dir) = Path::new(path).parent() {
        if !dir.is_dir() {
            fs::create_dir_all(dir)?;
            fs::set_permissions(dir, fs::Permissions::from_mode(0o755)).ok();
        }
    }

    if Path::new(path).exists() {
        // Left by a previous run of this daemon; nothing else owns the path.
        fs::remove_file(path)?;
    }

    let listener = UnixListener::bind(path)?;

    match group_id(CTL_GROUP) {
        Some(gid) => {
            nix::unistd::chown(path, None, Some(nix::unistd::Gid::from_raw(gid)))
                .map_err(std::io::Error::from)?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o660))?;
            log::info!(
                "Desktop control socket listening on {} (group {})",
                path,
                CTL_GROUP
            );
        }
        None => {
            // No such group means no session that could use it. Root-only
            // then, which is the same as not listening but leaves the reason
            // in the log.
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
            log::warn!(
                "No group '{}' in /etc/group; {} is root-only",
                CTL_GROUP,
                path
            );
        }
    }

    Ok(listener)
}

/// The gid of a group, from `/etc/group`.
fn group_id(name: &str) -> Option<u32> {
    let text = fs::read_to_string("/etc/group").ok()?;
    parse_group_id(&text, name)
}

/// One `/etc/group` line is `name:passwd:gid:members`. Same hand parser init
/// uses for the same reason: this crate links no libc name-service code, and
/// a static binary must not start depending on nss modules to find a gid.
fn parse_group_id(group: &str, name: &str) -> Option<u32> {
    group.lines().find_map(|line| {
        let mut fields = line.split(':');
        if fields.next()? != name {
            return None;
        }
        fields.next()?;
        fields.next()?.trim().parse().ok()
    })
}

/// Read one desktop client's line and either answer it here or hand it on.
///
/// Only a request that is not a request is answered from this thread: an
/// unknown word never reaches the main loop, so nothing about it can be
/// mistaken for a button. A real one is passed along with its stream, and the
/// main loop writes the reply.
fn accept_desktop_client(stream: UnixStream, tx: &Sender<Signal>) {
    stream.set_read_timeout(Some(CTL_CLIENT_TIMEOUT)).ok();
    stream.set_write_timeout(Some(CTL_CLIENT_TIMEOUT)).ok();

    let mut line = String::new();
    let Ok(reader) = stream.try_clone() else {
        return;
    };
    // Capped before it is parsed, so a client that streams forever costs a
    // timeout and a bounded buffer, not the daemon's memory.
    if let Err(e) = BufReader::new(reader)
        .take(CTL_MAX_REQUEST)
        .read_line(&mut line)
    {
        log::warn!("{}: could not read request: {}", CTL_SOCKET_PATH, e);
        return;
    }

    match CtlRequest::parse(&line) {
        Some(request) => {
            // If the main loop is gone the client sees a close with no reply,
            // which is the honest answer.
            let _ = tx.send(Signal::Request(request, stream));
        }
        None => {
            log::warn!("{}: unknown command {:?}", CTL_SOCKET_PATH, line.trim());
            reply(stream, "error: unknown command\n");
        }
    }
}

/// Answer one desktop request, from the main loop.
///
/// The power actions keep their cooldown and their [`serve_request`] path.
/// The profile verbs have neither: nothing about switching a governor twice
/// in five seconds needs guarding, and the reply is always written after the
/// work because none of the work can take the machine away mid-reply.
fn serve_ctl(
    request: CtlRequest,
    stream: UnixStream,
    profile_config: &profile::Profile,
    last_action: &mut Option<Instant>,
    profile_override: &mut Option<profile::Preset>,
    published: &mut Option<profile::Preset>,
) {
    match request {
        CtlRequest::Action(action) => {
            log::info!(
                "Desktop asked to {}",
                action.request().unwrap_or("do nothing")
            );
            serve_request(action, stream, last_action);
        }
        CtlRequest::SetProfile(preset) => {
            log::info!("Desktop set profile {}", preset.name());
            *profile_override = Some(preset);
            profile::apply(preset);
            publish_profile(preset, published);
            reply(stream, &format!("Profile {}\n", preset.name()));
        }
        CtlRequest::ProfileAuto => {
            log::info!("Desktop returned the profile to auto");
            *profile_override = None;
            if profile_config.manage {
                let preset = configured_preset(profile_config);
                profile::apply(preset);
                publish_profile(preset, published);
            } else {
                // Nothing is managing the governor now, and the marker must
                // not claim otherwise.
                *published = None;
                let _ = fs::remove_file(PROFILE_MARKER);
            }
            reply(stream, "Profile auto\n");
        }
        CtlRequest::ProfileQuery => {
            // First word the preset, the parenthesis for people; a client
            // parses the word and shows the rest or does not.
            let text = match (*profile_override, profile_config.manage) {
                (Some(preset), _) => format!("{} (override)\n", preset.name()),
                (None, true) => format!("{} (auto)\n", configured_preset(profile_config).name()),
                (None, false) => "unmanaged\n".to_string(),
            };
            reply(stream, &text);
        }
    }
}

/// The preset the config maps the supply to, right now. Read fresh rather
/// than from the poll's cache: an answer about the present should not be up
/// to [`SUPPLY_POLL`] stale.
fn configured_preset(profile_config: &profile::Profile) -> profile::Preset {
    match profile::supply() {
        profile::Supply::Mains => profile_config.ac,
        profile::Supply::Battery => profile_config.battery,
    }
}

/// Keep [`PROFILE_MARKER`] saying what was last applied.
///
/// Written only on change -- this sits on the same poll as the sysfs writes
/// -- and made 0644 explicitly rather than trusting the umask, because a
/// reader that cannot read the marker is the whole feature not working.
fn publish_profile(preset: profile::Preset, published: &mut Option<profile::Preset>) {
    if *published == Some(preset) {
        return;
    }
    match fs::write(PROFILE_MARKER, format!("{}\n", preset.name())) {
        Ok(()) => {
            fs::set_permissions(PROFILE_MARKER, fs::Permissions::from_mode(0o644)).ok();
            *published = Some(preset);
        }
        // Once per attempted change, not per poll, since `published` is only
        // left unset on failure -- but the next change will complain again,
        // which is right: the file being wrong is worth two lines.
        Err(e) => log::warn!("Cannot publish {}: {}", PROFILE_MARKER, e),
    }
}

/// Carry out a desktop power request and answer it, from the main loop.
///
/// The order differs by verb, and the difference is the point. For a suspend
/// the reply is written and the stream closed *before* [`perform`], for the
/// reason init's control socket gives: a client blocked on a read for the
/// length of a sleep looks like a hang and, if the machine never resumes, is
/// one. A power off or a reboot is answered afterwards, because init can still
/// refuse and the panel should hear that rather than a "Powering off" that was
/// a guess.
fn serve_request(action: PowerAction, stream: UnixStream, last_action: &mut Option<Instant>) {
    if let Some(at) = *last_action {
        if at.elapsed() < COOLDOWN {
            // The same window that keeps the wake press from re-suspending
            // the machine. The desktop cannot be told apart from it here, and
            // a person pressing twice inside five seconds can press again.
            log::debug!("Refused: within {:?} of the last action", COOLDOWN);
            reply(stream, "error: too soon after the last action\n");
            return;
        }
    }

    if action == PowerAction::Suspend {
        reply(stream, action.acknowledgement());
        if perform(action) {
            *last_action = Some(Instant::now());
        }
        return;
    }

    if perform(action) {
        *last_action = Some(Instant::now());
        reply(stream, action.acknowledgement());
    } else {
        reply(
            stream,
            "error: init did not agree; see the raven-powerd log\n",
        );
    }
}

/// Write one line and close. A client that went away is not an error worth
/// more than a debug line; the action was theirs to ask for, not to wait on.
fn reply(mut stream: UnixStream, text: &str) {
    if let Err(e) = stream
        .write_all(text.as_bytes())
        .and_then(|()| stream.flush())
    {
        log::debug!("{}: reply not delivered: {}", CTL_SOCKET_PATH, e);
    }
    // Dropping `stream` closes it, which is what ends the reply.
}

// ---------------------------------------------------------------------------
// The command line
// ---------------------------------------------------------------------------

/// What this process was asked to be.
#[derive(Debug, PartialEq, Eq)]
enum Invocation {
    /// No arguments: the daemon, as `init.toml` runs it.
    Daemon,
    /// `status`: the preset in force and the button and lid policy.
    Status,
    /// `profile`: which preset is in force, and why.
    ProfileQuery,
    /// `profile <preset>`: hold the machine in one preset for this session.
    ProfileSet(profile::Preset),
    /// `profile auto`: hand the choice back to `power.toml`.
    ProfileAuto,
    /// `help`, `--help`, `-h`.
    Help,
    /// Something else, kept whole so the error can quote what was typed.
    Unknown(String),
}

/// Decide what the arguments mean, without doing any of it.
///
/// Separated from [`main`] so the table below is a thing the tests can read.
/// The strictness matches [`CtlRequest::parse`] and is there for the same
/// reason: `profile` takes exactly zero or one word, and the word is a
/// preset's public name or `auto`. A near miss is [`Invocation::Unknown`] and
/// gets the usage text, never a socket connection -- a typo should cost a
/// message, not a governor.
fn invocation(args: &[String]) -> Invocation {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        [] => Invocation::Daemon,
        ["help"] | ["--help"] | ["-h"] => Invocation::Help,
        ["status"] => Invocation::Status,
        ["profile"] => Invocation::ProfileQuery,
        ["profile", "auto"] => Invocation::ProfileAuto,
        ["profile", word] => match profile::Preset::parse(word) {
            Some(preset) => Invocation::ProfileSet(preset),
            None => Invocation::Unknown(format!("profile {}", word)),
        },
        _ => Invocation::Unknown(words.join(" ")),
    }
}

/// What `raven-powerd help` prints, and what an unknown word prints on stderr.
///
/// Built rather than written down, for one line of it: the presets come from
/// [`profile::Preset::ALL`], so a fourth preset is offered here the moment it
/// exists instead of being a help text somebody forgot. Everything else is
/// fixed and the cost is one allocation on a path that is about to exit.
///
/// It names `/usr/bin/raven-power` at the end on purpose. That is the GTK
/// window, it is a different program from a different repository, and the
/// reason this paragraph exists is that running it from a terminal looks like
/// a hang -- it opens a window and then, quite correctly, does not return.
/// Somebody who reaches this text was probably looking for that.
fn usage() -> String {
    let presets: Vec<&str> = profile::Preset::ALL.iter().map(|p| p.name()).collect();
    USAGE.replace("<presets>", &presets.join(", "))
}

const USAGE: &str = "\
raven-powerd -- the power button, the sleep button and the lid.

With no arguments this is the daemon itself; that is how init starts it and
it does not return. The verbs below are a client of its control socket and
answer at once.

  raven-powerd status              What the machine's power policy is now
  raven-powerd profile             Which preset is in force, and why
  raven-powerd profile <preset>    Hold the machine in one preset
  raven-powerd profile auto        Hand the choice back to power.toml
  raven-powerd help                This text

<preset> is one of: <presets>.

A preset set here lasts until `profile auto`, until the daemon restarts, or
until the machine reboots. It is deliberately not written to a file: a choice
made at a prompt is a choice about now, and a laptop that boots into last
Tuesday's `performance` runs hot for a reason nobody remembers. To change
what the machine does by default, edit the [profile] table in
/etc/raven/power.toml and `raven-rc restart powerd`.

/usr/bin/raven-power is something else: the graphical Raven Power window. It
opens on the screen and stays open, which from a terminal looks like a hang.
";

/// Run one command and return the process's exit status.
///
/// Three statuses, and they mean what they mean everywhere else in this tree:
/// 0 the question was answered, 1 the machine could not answer it, 2 the
/// command was not one we have. The split matters to a shell script deciding
/// whether to retry.
fn run_command(invocation: Invocation) -> i32 {
    match invocation {
        // Handled in main before we get here; kept so the match is total and
        // so adding a variant is a compile error rather than a silent arm.
        Invocation::Daemon => 0,
        Invocation::Help => {
            print!("{}", usage());
            0
        }
        Invocation::Status => command_status(),
        Invocation::ProfileQuery => command_profile("profile"),
        Invocation::ProfileAuto => command_profile("profile auto"),
        Invocation::ProfileSet(preset) => command_profile(&format!("profile {}", preset.name())),
        Invocation::Unknown(what) => {
            eprintln!("raven-powerd: not a command: {}", what);
            eprintln!();
            eprint!("{}", usage());
            2
        }
    }
}

/// `profile`, `profile auto` and `profile <preset>`: one line in, one line out.
///
/// The daemon's reply is printed verbatim rather than reworded. It is already
/// written for a person -- "balanced (auto)", "performance (override)" -- and
/// it is the same text the desktop panel reads, so two clients cannot end up
/// describing the same machine differently.
fn command_profile(request: &str) -> i32 {
    match ask_powerd(request) {
        Ok(reply) => {
            print!("{}", with_newline(reply.trim_end()));
            // The daemon refuses in init's words, beginning `error:`. A
            // refusal printed with a zero status is a refusal a script does
            // not notice.
            if reply_is_error(&reply) {
                1
            } else {
                0
            }
        }
        Err(e) => {
            eprint!("{}", e.explain());
            1
        }
    }
}

/// `status`: everything this daemon decides, on one screen.
///
/// The preset comes from the daemon because only the daemon knows whether a
/// desktop override is holding one; the buttons, the lid and the ac/battery
/// mapping come from `power.toml`, which is where they live and which can be
/// read whether or not anything is running. That split is why a stopped
/// daemon still gets you most of this page, with one line saying which part
/// is missing and why.
fn command_status() -> i32 {
    let config = Config::load();
    let (in_force, summary, trouble) = profile_in_force();

    row("profile", &in_force);
    if let Some(summary) = summary {
        // Indented under the preset, unlabelled: it is the same fact said
        // again for somebody who does not already know what the word buys.
        row("", summary);
    }
    row(
        "supply",
        match profile::supply() {
            profile::Supply::Mains => "mains",
            profile::Supply::Battery => "battery",
        },
    );

    // What the config maps each supply to. Said even when `manage = false`,
    // because the commonest reason to run this is that the machine is not
    // doing what the file says and the answer is that nothing is applying it.
    let unmanaged = if config.profile.manage {
        ""
    } else {
        "   (not applied: manage = false)"
    };
    row(
        "on mains",
        &format!("{}{}", config.profile.ac.name(), unmanaged),
    );
    row(
        "on battery",
        &format!("{}{}", config.profile.battery.name(), unmanaged),
    );

    row("power button", config.buttons.power.describe());
    row("sleep button", config.buttons.sleep.describe());
    row("lid close", config.lid.close.describe());
    row(
        "wakeup",
        if config.manage_wakeup {
            "power button and lid armed at start"
        } else {
            "left to /proc/acpi/wakeup (manage_wakeup = false)"
        },
    );
    row("config", CONFIG_PATH);

    match trouble {
        Some(e) => {
            eprintln!();
            eprint!("{}", e.explain());
            1
        }
        None => 0,
    }
}

/// One `label  value` line. The width fits the longest label above with room
/// to spare, so a label added later does not silently run into its value.
fn row(label: &str, value: &str) {
    println!("{:<15}{}", label, value);
}

/// The preset in force, its one-line summary, and whatever went wrong asking.
///
/// The fallback when the daemon does not answer is `/run/raven-power/profile`,
/// the marker the daemon writes on every change. It is the last preset that
/// was actually applied, and since the governor it set is still set, it is
/// very probably still true -- so saying it, labelled as what it is, beats
/// saying "unknown" to somebody who only wanted to know whether the machine
/// is in power-saver. The marker lives in `/run` and so cannot outlive a boot
/// and start lying about a machine that has been restarted.
fn profile_in_force() -> (String, Option<&'static str>, Option<CtlError>) {
    match ask_powerd("profile") {
        Ok(reply) => {
            let text = reply.trim().to_string();
            // The reply's first word is the preset and the rest is for
            // people, exactly as `serve_ctl` writes it. `unmanaged` is a
            // first word that is not a preset, and has no summary.
            let first = text.split_whitespace().next().unwrap_or_default();
            let summary = profile::Preset::parse(first).map(profile::Preset::summary);
            (text, summary, None)
        }
        Err(e) => match last_applied_preset() {
            Some(preset) => (
                format!("{} (last applied; the daemon is not answering)", preset.name()),
                Some(preset.summary()),
                Some(e),
            ),
            None => ("unknown".to_string(), None, Some(e)),
        },
    }
}

/// Read [`PROFILE_MARKER`]. `None` when it is absent, unreadable, or holds a
/// word this build does not know -- all of which mean the same thing to the
/// caller, which is that it has to say so.
fn last_applied_preset() -> Option<profile::Preset> {
    let text = fs::read_to_string(PROFILE_MARKER).ok()?;
    profile::Preset::parse(text.trim())
}

/// Print with a trailing newline, once, however the daemon punctuated it.
fn with_newline(text: &str) -> String {
    format!("{}\n", text)
}

/// Why a request to the daemon did not produce an answer.
///
/// Four cases and not one, because they want four different things done about
/// them and a client that cannot tell them apart sends everybody to the same
/// wrong place.
#[derive(Debug)]
enum CtlError {
    /// No socket, or a socket file with nothing listening on it: the daemon
    /// is not running.
    NotRunning,
    /// The socket is there and we are not in group `video`.
    Denied,
    /// It accepted, or it did not, and either way nothing came back inside
    /// [`CLI_TIMEOUT`].
    Timeout,
    /// Anything else, quoted rather than interpreted.
    Other(std::io::Error),
}

impl CtlError {
    /// The operator-facing text, ending in the command that fixes it -- the
    /// shape every error in this tree takes (see `control.rs`'s start
    /// failures and `rc.rs`'s connection failures).
    fn explain(&self) -> String {
        match self {
            CtlError::NotRunning => format!(
                "raven-powerd is not answering on {}.\n\
                 \x20 It is the daemon that decides the profile, so there is nothing else\n\
                 \x20 to ask which one is in force.\n\
                 \x20 Start it with `raven-rc start powerd`, or find out why it stopped\n\
                 \x20 with `raven-rc logs powerd`.\n",
                CTL_SOCKET_PATH
            ),
            CtlError::Denied => format!(
                "{}: permission denied.\n\
                 \x20 The socket is group `{}` and mode 0660 -- whoever owns the screen may\n\
                 \x20 put the machine to sleep, and nobody else.\n\
                 \x20 Add yourself to that group and log in again, or run this as root.\n",
                CTL_SOCKET_PATH, CTL_GROUP
            ),
            CtlError::Timeout => format!(
                "raven-powerd did not answer within {}s.\n\
                 \x20 Something is listening on {} -- so the\n\
                 \x20 daemon is there -- but its main loop has not reached the request.\n\
                 \x20 Read `raven-rc logs powerd`; if it is wedged, `raven-rc restart powerd`.\n",
                CLI_TIMEOUT.as_secs(),
                CTL_SOCKET_PATH
            ),
            CtlError::Other(e) => format!(
                "Could not ask raven-powerd on {}: {}\n\
                 \x20 See `raven-rc status powerd`.\n",
                CTL_SOCKET_PATH, e
            ),
        }
    }

    /// Which of the four an `io::Error` from the exchange is.
    ///
    /// `ConnectionRefused` is grouped with `NotFound` deliberately: it is the
    /// socket file a daemon left behind when it died without unlinking, and
    /// to the person at the prompt that is the same fact as no file at all --
    /// the daemon is not running. `bind_ctl_socket` removes such a file on
    /// the next start, so there is nothing for them to clean up either.
    fn from_io(e: std::io::Error) -> Self {
        match e.kind() {
            ErrorKind::NotFound | ErrorKind::ConnectionRefused => CtlError::NotRunning,
            ErrorKind::PermissionDenied => CtlError::Denied,
            ErrorKind::TimedOut | ErrorKind::WouldBlock => CtlError::Timeout,
            _ => CtlError::Other(e),
        }
    }
}

/// Send one request to the daemon and return its reply, or give up.
///
/// # Why a thread
///
/// The read and the write are bounded by `SO_RCVTIMEO` and `SO_SNDTIMEO`, but
/// the `connect` is not: `std` has no connect timeout for a Unix socket, and
/// `connect` on `AF_UNIX` really can block -- it waits when the listener's
/// backlog is full, and a daemon whose accept thread has stopped accepting is
/// exactly the state in which somebody types this command. Setting the socket
/// non-blocking to dodge that would mean hand-rolling the connect, the
/// `SO_ERROR` check and the poll, in a binary that has no other reason to
/// touch raw file descriptors.
///
/// So the whole exchange happens on a thread and this waits on a channel for
/// [`CLI_TIMEOUT`]. When the wait expires we return and the process exits; the
/// thread goes with it. Nothing is leaked that outlives the command, because
/// the command is the process. That is only true of a short-lived client --
/// do not copy this shape into the daemon, where a thread stuck in `connect`
/// would accumulate once per attempt forever.
fn ask_powerd(request: &str) -> Result<String, CtlError> {
    ask_powerd_at(Path::new(CTL_SOCKET_PATH), request, CLI_TIMEOUT)
}

/// [`ask_powerd`], with the socket and the bound named rather than assumed.
///
/// Both are arguments for the tests' sake -- a temporary socket and a bound
/// of a few hundred milliseconds, rather than an environment variable that
/// would also be read by the daemon and by every other client on the machine.
/// Nothing in the shipped binary calls this with anything but the two
/// constants above.
fn ask_powerd_at(path: &Path, request: &str, timeout: Duration) -> Result<String, CtlError> {
    let (tx, rx) = mpsc::channel::<std::io::Result<String>>();
    let owned = request.to_string();
    let owned_path = path.to_path_buf();

    let spawned = thread::Builder::new()
        .name("powerd-ask".to_string())
        .stack_size(64 * 1024)
        .spawn(move || {
            // The receiver is gone if we timed out; that send failing is the
            // expected end of this thread and not worth a word.
            let _ = tx.send(ctl_exchange(&owned_path, &owned, timeout));
        });

    if spawned.is_err() {
        // A machine that cannot make a thread will not do better for being
        // told again. Fall back to doing it here, which is bounded by the
        // socket timeouts and unbounded only in `connect`.
        return ctl_exchange(path, request, timeout).map_err(CtlError::from_io);
    }

    match rx.recv_timeout(timeout) {
        Ok(Ok(reply)) => Ok(reply),
        Ok(Err(e)) => Err(CtlError::from_io(e)),
        // Timeout is the case this function exists for. Disconnected means
        // the thread ended without sending, which it has no path to do --
        // reporting it as a timeout keeps the impossible case from needing a
        // message of its own that nobody would ever read.
        Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => {
            Err(CtlError::Timeout)
        }
    }
}

/// One request, one reply, on the caller's thread.
///
/// The same shape as [`ask_init`], down to reading to end-of-file: the daemon
/// closes the stream after answering (see [`reply`]), so a short read is the
/// end of the message and not a truncation.
fn ctl_exchange(path: &Path, request: &str, timeout: Duration) -> std::io::Result<String> {
    let mut stream = UnixStream::connect(path)?;
    stream.set_write_timeout(Some(timeout))?;
    stream.set_read_timeout(Some(timeout))?;

    stream.write_all(request.as_bytes())?;
    // The daemon reads a line, so the newline is not decoration.
    stream.write_all(b"\n")?;
    stream.flush()?;

    let mut reply = String::new();
    stream.read_to_string(&mut reply)?;
    Ok(reply)
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
    let caps = PathBuf::from("/sys/class/input")
        .join(name)
        .join("device/capabilities");

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

    /// A lid or a button can be set to hibernate.
    ///
    /// This is the whole user-visible half of the change that made
    /// hibernation reachable: before it, `close = "hibernate"` was a value
    /// serde had never heard of, so the file failed to parse, the daemon fell
    /// back to its defaults, and the lid suspended -- with one `info` line to
    /// say why. Asserting the request word as well as the variant is what ties
    /// this to the verb `control::dispatch` answers; a rename on either side
    /// that did not happen on both would be a lid that asks init for something
    /// it does not know.
    #[test]
    fn a_lid_or_a_button_can_be_set_to_hibernate() {
        let config: Config =
            toml::from_str("[lid]\nclose = \"hibernate\"\n").expect("hibernate is a value");
        assert_eq!(config.lid.close, PowerAction::Hibernate);
        assert_eq!(config.lid.close.request(), Some("hibernate"));

        let config: Config =
            toml::from_str("[buttons]\npower = \"hibernate\"\n").expect("hibernate is a value");
        assert_eq!(config.buttons.power, PowerAction::Hibernate);

        // It is nobody's default, on any machine. A lid that hibernates has to
        // have been asked for, because a machine that cannot resume loses the
        // session and this daemon is not the thing that knows whether it can.
        assert_eq!(Lid::default().close, PowerAction::Suspend);
        assert_eq!(Buttons::default().power, PowerAction::Suspend);
        assert_eq!(Buttons::default().sleep, PowerAction::Suspend);
    }

    /// The file this repo ships must deserialize with the schema the daemon
    /// actually uses. Without this, a renamed key is only discovered on a
    /// machine whose lid has stopped working.
    #[test]
    fn the_shipped_config_parses() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../etc/raven/power.toml");
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
        assert_eq!(PowerAction::Hibernate.request(), Some("hibernate"));
        assert_eq!(PowerAction::Poweroff.request(), Some("poweroff"));
        assert_eq!(PowerAction::Reboot.request(), Some("reboot"));
        assert_eq!(PowerAction::Ignore.request(), None);
    }

    #[test]
    fn the_ctl_socket_knows_its_verbs() {
        assert!(matches!(
            CtlRequest::parse("suspend\n"),
            Some(CtlRequest::Action(PowerAction::Suspend))
        ));
        assert!(matches!(
            CtlRequest::parse("profile\n"),
            Some(CtlRequest::ProfileQuery)
        ));
        assert!(matches!(
            CtlRequest::parse("profile auto\n"),
            Some(CtlRequest::ProfileAuto)
        ));
        assert!(matches!(
            CtlRequest::parse("profile power-saver\n"),
            Some(CtlRequest::SetProfile(profile::Preset::PowerSaver))
        ));
        assert!(matches!(
            CtlRequest::parse("  profile performance  \n"),
            Some(CtlRequest::SetProfile(profile::Preset::Performance))
        ));
    }

    #[test]
    fn the_ctl_socket_rejects_near_misses() {
        // Every spelling accepted is one more a client sends by mistake.
        assert!(CtlRequest::parse("profile banana\n").is_none());
        assert!(CtlRequest::parse("profile eco\n").is_none());
        assert!(CtlRequest::parse("profiles\n").is_none());
        assert!(CtlRequest::parse("profile power-saver now\n").is_none());
        assert!(CtlRequest::parse("sleep\n").is_none());
        assert!(CtlRequest::parse("\n").is_none());
    }

    // The desktop socket takes exactly the three words below, and what it
    // reads is a line, so the newline the client sends must not count.

    #[test]
    fn the_three_verbs_parse_to_their_actions() {
        assert_eq!(
            PowerAction::parse_request("suspend\n"),
            Some(PowerAction::Suspend)
        );
        assert_eq!(
            PowerAction::parse_request("poweroff\n"),
            Some(PowerAction::Poweroff)
        );
        assert_eq!(
            PowerAction::parse_request("reboot\n"),
            Some(PowerAction::Reboot)
        );
    }

    #[test]
    fn trailing_whitespace_is_not_part_of_the_verb() {
        assert_eq!(
            PowerAction::parse_request("suspend \r\n"),
            Some(PowerAction::Suspend)
        );
        assert_eq!(
            PowerAction::parse_request("reboot"),
            Some(PowerAction::Reboot)
        );
    }

    #[test]
    fn an_unknown_word_is_not_a_request() {
        assert_eq!(PowerAction::parse_request("halt\n"), None);
        assert_eq!(PowerAction::parse_request("sleep\n"), None);
        assert_eq!(PowerAction::parse_request("ignore\n"), None);
        assert_eq!(PowerAction::parse_request("suspend now\n"), None);
        assert_eq!(PowerAction::parse_request("\n"), None);
        assert_eq!(PowerAction::parse_request(""), None);
    }

    /// A request's word and its acknowledgement are the pair init uses, so a
    /// client reading the desktop socket sees what a client of init's would.
    #[test]
    fn a_request_is_acknowledged_in_init_words() {
        for (word, ack) in [
            ("suspend", "Suspending\n"),
            ("hibernate", "Hibernating\n"),
            ("poweroff", "Powering off\n"),
            ("reboot", "Rebooting\n"),
        ] {
            let action = PowerAction::parse_request(word).unwrap();
            assert_eq!(action.request(), Some(word));
            assert_eq!(action.acknowledgement(), ack);
            assert!(!reply_is_error(ack));
        }
    }

    // -----------------------------------------------------------------
    // The command line
    // -----------------------------------------------------------------

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    /// The one property nothing may ever break: `init.toml` execs this with
    /// an empty `args` list, so no arguments has to stay the daemon. A verb
    /// added carelessly here is a machine whose lid stops working.
    #[test]
    fn no_arguments_is_still_the_daemon() {
        assert_eq!(invocation(&argv(&[])), Invocation::Daemon);
    }

    #[test]
    fn the_command_line_knows_its_verbs() {
        assert_eq!(invocation(&argv(&["status"])), Invocation::Status);
        assert_eq!(invocation(&argv(&["profile"])), Invocation::ProfileQuery);
        assert_eq!(invocation(&argv(&["profile", "auto"])), Invocation::ProfileAuto);
        assert_eq!(
            invocation(&argv(&["profile", "performance"])),
            Invocation::ProfileSet(profile::Preset::Performance)
        );
        assert_eq!(
            invocation(&argv(&["profile", "power-saver"])),
            Invocation::ProfileSet(profile::Preset::PowerSaver)
        );
        for word in ["help", "--help", "-h"] {
            assert_eq!(invocation(&argv(&[word])), Invocation::Help, "{}", word);
        }
    }

    /// A near miss must not reach the socket. Every one of these used to be
    /// the shape of a typo that would otherwise have set a governor or, on
    /// the argument-less path, silently forked a second daemon.
    #[test]
    fn a_near_miss_is_a_usage_error_and_never_a_request() {
        for words in [
            vec!["profile", "banana"],
            vec!["profile", "eco"],
            vec!["profile", "performance", "now"],
            vec!["profiles"],
            vec!["suspend"],
            vec!["status", "powerd"],
            vec!["--daemon"],
            vec![""],
        ] {
            assert!(
                matches!(invocation(&argv(&words)), Invocation::Unknown(_)),
                "{:?} should not be a command",
                words
            );
        }
    }

    /// Every preset the daemon will accept is a preset the help text offers,
    /// and every word the help text offers parses back. This is the check
    /// that a fourth preset cannot be added to `profile.rs` and forgotten
    /// here.
    #[test]
    fn the_usage_text_offers_exactly_the_presets_that_exist() {
        let text = usage();
        for preset in profile::Preset::ALL {
            assert!(
                text.contains(preset.name()),
                "usage does not mention {}",
                preset.name()
            );
            assert_eq!(
                invocation(&argv(&["profile", preset.name()])),
                Invocation::ProfileSet(preset)
            );
        }
        assert!(!text.contains("<presets>"), "the preset list was not filled in");
        // The paragraph that exists to stop the next person losing an
        // afternoon to a window that will not return.
        assert!(text.contains("/usr/bin/raven-power"));
    }

    /// The four failures are four messages, each naming the socket and ending
    /// in something to type. A client that cannot tell them apart sends
    /// everybody to the same wrong place.
    #[test]
    fn every_failure_to_reach_the_daemon_says_what_to_do() {
        let errors = [
            CtlError::NotRunning,
            CtlError::Denied,
            CtlError::Timeout,
            CtlError::Other(std::io::Error::new(ErrorKind::BrokenPipe, "gone")),
        ];
        for e in &errors {
            let text = e.explain();
            assert!(text.contains(CTL_SOCKET_PATH), "{:?} does not name the socket", e);
            assert!(text.ends_with('\n'), "{:?} does not end in a newline", e);
            assert!(text.contains('`'), "{:?} names no command to run", e);
        }
        assert!(errors[0].explain().contains("raven-rc start powerd"));
        assert!(errors[1].explain().contains(CTL_GROUP));
    }

    /// A socket file left behind by a daemon that died is the same fact, to
    /// the person at the prompt, as no socket at all.
    #[test]
    fn a_stale_socket_reads_as_a_daemon_that_is_not_running() {
        let cases = [
            (ErrorKind::NotFound, "NotRunning"),
            (ErrorKind::ConnectionRefused, "NotRunning"),
            (ErrorKind::PermissionDenied, "Denied"),
            (ErrorKind::WouldBlock, "Timeout"),
            (ErrorKind::TimedOut, "Timeout"),
        ];
        for (kind, expected) in cases {
            let got = match CtlError::from_io(std::io::Error::new(kind, "x")) {
                CtlError::NotRunning => "NotRunning",
                CtlError::Denied => "Denied",
                CtlError::Timeout => "Timeout",
                CtlError::Other(_) => "Other",
            };
            assert_eq!(got, expected, "{:?}", kind);
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "raven-powerd-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a temporary directory");
        dir
    }

    /// A daemon that is not running costs a message, not the terminal. This
    /// is the reported bug's shape: something asks the power socket a
    /// question and nothing is there to answer it.
    #[test]
    fn asking_a_daemon_that_is_not_there_answers_instead_of_waiting() {
        let dir = scratch("absent");
        let started = Instant::now();
        let result = ask_powerd_at(&dir.join("ctl"), "profile", Duration::from_millis(500));
        assert!(matches!(result, Err(CtlError::NotRunning)), "{:?}", result);
        assert!(started.elapsed() < Duration::from_millis(500));
        let _ = fs::remove_dir_all(&dir);
    }

    /// A socket somebody is listening on but nobody is answering on is the
    /// case the bound exists for: `connect` succeeds, the write succeeds, and
    /// the reply never comes. Before there was a timeout this is where a
    /// client sat for as long as anyone was willing to wait.
    #[test]
    fn a_daemon_that_never_answers_is_given_up_on() {
        let dir = scratch("silent");
        let path = dir.join("ctl");
        // Bound and never accepted: the connection sits in the backlog.
        let listener = UnixListener::bind(&path).expect("bind");

        let started = Instant::now();
        let result = ask_powerd_at(&path, "profile", Duration::from_millis(300));
        assert!(matches!(result, Err(CtlError::Timeout)), "{:?}", result);
        // The bound is the bound, not a suggestion.
        assert!(started.elapsed() < Duration::from_secs(3), "took {:?}", started.elapsed());

        drop(listener);
        let _ = fs::remove_dir_all(&dir);
    }

    /// And the case that has to keep working: a daemon that answers is
    /// answered, with the reply it wrote.
    #[test]
    fn a_daemon_that_answers_is_read_verbatim() {
        let dir = scratch("answers");
        let path = dir.join("ctl");
        let listener = UnixListener::bind(&path).expect("bind");

        let server = thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut line = String::new();
                let Ok(peer) = stream.try_clone() else { return };
                let _ = BufReader::new(peer).read_line(&mut line);
                assert_eq!(line, "profile\n", "the newline is part of the request");
                let _ = stream.write_all(b"performance (override)\n");
            }
        });

        let reply = ask_powerd_at(&path, "profile", Duration::from_secs(2));
        assert_eq!(reply.ok().as_deref(), Some("performance (override)\n"));

        let _ = server.join();
        let _ = fs::remove_dir_all(&dir);
    }

    /// The reply is printed as the daemon wrote it, punctuated once.
    #[test]
    fn a_reply_gets_exactly_one_newline() {
        assert_eq!(with_newline("balanced (auto)"), "balanced (auto)\n");
        assert_eq!(with_newline(""), "\n");
    }

    #[test]
    fn a_group_is_found_by_name_and_only_by_name() {
        let group = "root:x:0:\nvideo:x:91:javan\nvideoish:x:92:\n";
        assert_eq!(parse_group_id(group, "video"), Some(91));
        assert_eq!(parse_group_id(group, "root"), Some(0));
        assert_eq!(parse_group_id(group, "input"), None);
    }
}
