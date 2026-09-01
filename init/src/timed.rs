//! `raven-timed` -- the wall clock: what zone it reads in and who sets it.
//!
//! The kernel keeps time and stops there. It boots with whatever the RTC held,
//! never asks the network whether that is right, and has no idea what a
//! timezone is -- `/etc/localtime` is a userspace convention that libc reads
//! and nothing enforces. On most distributions the something that fills these
//! gaps is systemd-timesyncd and timedated behind a bus; here it is this, a
//! few hundred lines with no bus and one configuration file.
//!
//! Two jobs, and the split matters:
//!
//! * **Sync.** An SNTP client (RFC 4330 -- the stateless corner of NTP) asks
//!   the configured servers what time it is, and steps the clock when the
//!   answer disagrees by more than [`STEP_THRESHOLD`]. Stepping rather than
//!   slewing on purpose: adjtimex discipline is what chrony is for, and a
//!   desktop that is four hours wrong wants the four hours *now*, not smeared
//!   over a week. After a step the RTC is written back through the same
//!   `hwclock` init reads it with, so the correction survives a power cut.
//!
//! * **Zone.** `/etc/localtime` is a symlink into `/usr/share/zoneinfo`, and
//!   changing it is one atomic rename -- but a root-owned one, which is the
//!   whole reason the desktop needs this daemon for it.
//!
//! # The desktop's socket
//!
//! Same shape as raven-powerd's: `/run/raven-time/ctl`, group `video`, mode
//! 0660 -- `video` because the session already holds it for the DRM device,
//! so it names "whoever owns the screen" without inventing a group. One line
//! in, text out, close. The verbs:
//!
//! ```text
//! status            zone, whether sync is on, and the last sync's outcome
//! zone              the current zone, one word
//! zone Area/City    switch /etc/localtime; the reply is `Zone Area/City`
//! zones             every zone this machine's zoneinfo can name, one per line
//! sync              ask the servers now; `Synced +0.012s via 0.pool.ntp.org`
//! sync on|off       turn automatic sync on or off, persistently
//! set <unix-secs>   set the clock by hand, for the machine with no network
//! ```
//!
//! Every refusal begins with `error:`, as raven-powerd's do, so one client
//! can read both sockets with one rule.
//!
//! `sync on|off` is persisted into `/etc/raven/time.toml` -- unlike powerd's
//! profile override, which is deliberately session-only -- because "this
//! machine keeps its own time" is a fact about the machine, not about the
//! afternoon. The rewrite goes through `toml_edit`, so a hand-written comment
//! in the file survives the toggle.
//!
//! What the last sync did is also published at `/run/raven-time/status`, one
//! word (`synced` or `unsynced`), world-readable, so a bar that only wants to
//! know whether the clock can be trusted reads one file instead of holding
//! the socket.
//!
//! # Run by hand
//!
//! With arguments, this binary is its own client: `raven-timed status` (or
//! `zone`, `zones`, `sync`, `set`) sends the line to the running daemon and
//! prints the reply, which is how the CLI gets a time tool without a second
//! binary.

use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::{ToSocketAddrs, UdpSocket};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;

/// Where the desktop (and the CLI mode of this binary) asks.
const CTL_SOCKET_PATH: &str = "/run/raven-time/ctl";

/// The group that may write to [`CTL_SOCKET_PATH`]. The same group
/// raven-powerd uses for the same reason: the session already holds it for
/// the DRM device, so owning the screen and being allowed to set its clock
/// are one fact, not two.
const CTL_GROUP: &str = "video";

/// Where the last sync's outcome is published, one word, for anything that
/// only wants to read it. `synced` or `unsynced`; absent before the first
/// attempt, and gone after a boot like the rest of the directory.
const STATUS_MARKER: &str = "/run/raven-time/status";

/// Longest request we will read. The longest valid one is `zone` plus the
/// longest name in zoneinfo -- `America/Argentina/ComodRivadavia` and friends
/// stay well under this; the cap is for a client that never stops.
const CTL_MAX_REQUEST: u64 = 256;

/// How long one client may take to say its word. The panel writes the line in
/// the same call that connects, so this is only ever hit by something that
/// connected and then stalled.
const CTL_CLIENT_TIMEOUT: Duration = Duration::from_millis(500);

/// Policy, if anyone wrote any down.
const CONFIG_PATH: &str = "/etc/raven/time.toml";

/// Where zone names resolve. Also the prefix `/etc/localtime` must point
/// under for [`current_zone`] to have a name to report.
const ZONEINFO: &str = "/usr/share/zoneinfo";

/// The symlink that *is* the system timezone.
const LOCALTIME: &str = "/etc/localtime";

/// How far the clock may drift before a sync steps it. Under this the answer
/// is "close enough": without adjtimex discipline a small step every hour
/// would make the clock jitter for no one's benefit.
const STEP_THRESHOLD: f64 = 0.2;

/// How long to wait on one server before trying the next.
const SNTP_TIMEOUT: Duration = Duration::from_secs(5);

/// First retry after a failed sync. Doubles per failure up to
/// [`RETRY_CEILING`], because the common cause is "the network is not up
/// yet", which fixes itself in seconds, and the second most common is "there
/// is no network", which does not.
const RETRY_FLOOR: Duration = Duration::from_secs(10);
const RETRY_CEILING: Duration = Duration::from_secs(300);

/// Seconds between the NTP epoch (1900) and the Unix one (1970).
const NTP_UNIX_OFFSET: f64 = 2_208_988_800.0;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct SyncConfig {
    /// Ask the servers at all. `sync on|off` on the socket rewrites this.
    enabled: bool,
    /// Tried in order until one answers. Hostnames or addresses; port 123
    /// unless the entry carries its own.
    servers: Vec<String>,
    /// How long after a successful sync the next one happens.
    interval_minutes: u64,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            servers: vec![
                "0.pool.ntp.org".to_string(),
                "1.pool.ntp.org".to_string(),
                "2.pool.ntp.org".to_string(),
            ],
            interval_minutes: 60,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Config {
    sync: SyncConfig,
}

impl Config {
    /// Read the file, or fall back to the defaults.
    ///
    /// A missing file is normal and silent. A *broken* file is loud and still
    /// falls back, because the alternative is a daemon that refuses to start
    /// and a clock that stops being kept over a typo.
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
                log::error!("  Using defaults. Fix the file and restart raven-timed.");
                Self::default()
            }
        }
    }
}

/// Rewrite `sync.enabled` in the config file, keeping whatever else -- and
/// whatever comments -- the file holds. A missing file becomes a minimal one:
/// the shipped template lives in the image, but a machine that predates it
/// still deserves a toggle that sticks.
fn persist_enabled(value: bool) -> Result<(), String> {
    let text = match fs::read_to_string(CONFIG_PATH) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("cannot read {}: {}", CONFIG_PATH, e)),
    };

    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e| format!("{} is not valid TOML: {}", CONFIG_PATH, e))?;
    doc["sync"]["enabled"] = toml_edit::value(value);

    fs::write(CONFIG_PATH, doc.to_string()).map_err(|e| format!("cannot write {}: {}", CONFIG_PATH, e))
}

// ---------------------------------------------------------------------------
// SNTP
// ---------------------------------------------------------------------------

/// One successful exchange: how far off we were, and who said so.
#[derive(Debug, Clone)]
struct SyncOutcome {
    /// Seconds the server thinks we are behind (positive) or ahead (negative).
    offset: f64,
    server: String,
    /// When, on the corrected clock.
    at: u64,
}

/// Ask one server. Returns the measured offset in seconds.
///
/// RFC 4330: one 48-byte datagram out, one back. We send LI=0 VN=4 Mode=3
/// (client) with our transmit time in the transmit field; the server echoes
/// it in *originate* and adds its receive (T2) and transmit (T3) stamps. With
/// our send (T1) and arrival (T4) times the offset is
/// `((T2 - T1) + (T3 - T4)) / 2`, which cancels a symmetric network delay.
fn sntp_query(server: &str) -> Result<f64, String> {
    let address = if server.contains(':') {
        server.to_string()
    } else {
        format!("{}:123", server)
    };
    // getaddrinfo, which musl does in-process from /etc/resolv.conf: a static
    // binary resolving names must not need nss modules.
    let address = address
        .to_socket_addrs()
        .map_err(|e| format!("{}: cannot resolve: {}", server, e))?
        .next()
        .ok_or_else(|| format!("{}: resolves to nothing", server))?;

    let socket = UdpSocket::bind(if address.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" })
        .map_err(|e| format!("cannot bind a UDP socket: {}", e))?;
    socket
        .set_read_timeout(Some(SNTP_TIMEOUT))
        .map_err(|e| e.to_string())?;
    socket
        .connect(address)
        .map_err(|e| format!("{}: {}", server, e))?;

    let mut request = [0u8; 48];
    request[0] = 0x23; // LI 0, version 4, mode 3 (client)
    let t1 = unix_now();
    write_ntp_timestamp(&mut request[40..48], t1);
    socket
        .send(&request)
        .map_err(|e| format!("{}: send: {}", server, e))?;

    let mut reply = [0u8; 48];
    let read = socket
        .recv(&mut reply)
        .map_err(|e| format!("{}: no reply: {}", server, e))?;
    let t4 = unix_now();

    if read < 48 {
        return Err(format!("{}: short reply ({} bytes)", server, read));
    }
    // Mode must be server; a reply to a mode-3 request that is not mode 4 is
    // not for us.
    if reply[0] & 0x07 != 4 {
        return Err(format!("{}: reply mode {} is not server", server, reply[0] & 0x07));
    }
    // Stratum 0 is a kiss-of-death code, and LI 3 is a server that admits its
    // own clock is not synchronized. Neither may set ours.
    if reply[1] == 0 {
        return Err(format!("{}: kiss-of-death", server));
    }
    if reply[0] >> 6 == 3 {
        return Err(format!("{}: server is unsynchronized", server));
    }
    // The originate stamp must be the transmit stamp we sent. Anything else
    // is a stale or forged datagram, and UDP will happily deliver both.
    if read_ntp_timestamp(&reply[24..32]) != read_ntp_timestamp(&request[40..48]) {
        return Err(format!("{}: originate stamp is not ours", server));
    }

    let t2 = ntp_to_unix(read_ntp_timestamp(&reply[32..40]));
    let t3 = ntp_to_unix(read_ntp_timestamp(&reply[40..48]));
    Ok(((t2 - t1) + (t3 - t4)) / 2.0)
}

/// The system clock, as seconds since the Unix epoch.
fn unix_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Eight bytes of NTP timestamp: 32 bits of seconds since 1900, 32 of
/// fraction. Read raw; [`ntp_to_unix`] is where it becomes a time.
fn read_ntp_timestamp(bytes: &[u8]) -> u64 {
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&bytes[..8]);
    u64::from_be_bytes(raw)
}

fn ntp_to_unix(raw: u64) -> f64 {
    let seconds = (raw >> 32) as f64;
    let fraction = (raw & 0xFFFF_FFFF) as f64 / 4_294_967_296.0;
    seconds + fraction - NTP_UNIX_OFFSET
}

fn write_ntp_timestamp(bytes: &mut [u8], unix: f64) {
    let ntp = unix + NTP_UNIX_OFFSET;
    let seconds = ntp as u64;
    let fraction = ((ntp - seconds as f64) * 4_294_967_296.0) as u64;
    bytes[..8].copy_from_slice(&((seconds << 32) | fraction).to_be_bytes());
}

/// Step the clock by `offset` seconds and write the RTC to match.
///
/// `clock_settime` rather than `date`, because the daemon is the thing a
/// shell tool would be shelling out to. The RTC write goes through the same
/// `hwclock --utc` init reads it back with at boot; if the binary is missing
/// the step still stands, it just does not survive a power cut.
fn step_clock(offset: f64) -> Result<(), String> {
    let target = unix_now() + offset;
    let spec = libc::timespec {
        tv_sec: target as libc::time_t,
        tv_nsec: ((target - target.floor()) * 1e9) as libc::c_long,
    };
    // SAFETY: a plain syscall on a stack value; fails with EPERM if we are
    // not root, which is reported, not ignored.
    if unsafe { libc::clock_settime(libc::CLOCK_REALTIME, &spec) } != 0 {
        return Err(format!(
            "clock_settime: {}",
            std::io::Error::last_os_error()
        ));
    }

    let rtc = Command::new("/sbin/hwclock")
        .args(["--systohc", "--utc"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if !rtc.map(|s| s.success()).unwrap_or(false) {
        log::warn!("Clock stepped, but the RTC was not written (hwclock failed)");
    }
    Ok(())
}

/// Try every configured server in order; step the clock if the first one that
/// answers says we are off by more than [`STEP_THRESHOLD`].
fn sync_once(servers: &[String]) -> Result<SyncOutcome, String> {
    if servers.is_empty() {
        return Err("no servers configured".to_string());
    }

    let mut last_error = String::new();
    for server in servers {
        match sntp_query(server) {
            Ok(offset) => {
                if offset.abs() >= STEP_THRESHOLD {
                    // Measured before stepped, so a step that fails -- EPERM
                    // when run by hand -- still leaves the offset in the log.
                    log::debug!("{} says we are {:+.3}s off", server, offset);
                    step_clock(offset)?;
                    log::info!("Stepped the clock {:+.3}s (from {})", offset, server);
                } else {
                    log::debug!("Clock within {:+.3}s of {}; left alone", offset, server);
                }
                return Ok(SyncOutcome {
                    offset,
                    server: server.clone(),
                    at: unix_now() as u64,
                });
            }
            Err(e) => {
                log::debug!("{}", e);
                last_error = e;
            }
        }
    }
    Err(last_error)
}

// ---------------------------------------------------------------------------
// Zones
// ---------------------------------------------------------------------------

/// The zone `/etc/localtime` names, or `UTC` when it names nothing readable.
///
/// Read from the symlink rather than kept in memory, so an admin who moves
/// the link by hand is answered truthfully.
fn current_zone() -> String {
    let Ok(target) = fs::read_link(LOCALTIME) else {
        return "UTC".to_string();
    };
    let target = target.to_string_lossy();
    match target.split_once("zoneinfo/") {
        Some((_, zone)) => zone.to_string(),
        None => "UTC".to_string(),
    }
}

/// Is `name` a zone this machine's zoneinfo actually holds?
///
/// The name walks into the filesystem, so the character check is load-bearing:
/// nothing with a `..`, a leading slash or a character outside the zoneinfo
/// alphabet gets near a path. What survives must then exist and carry the
/// TZif magic, which is what tells `America/New_York` apart from `zone.tab`.
fn validate_zone(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 100 {
        return Err("error: not a zone name".to_string());
    }
    let ok = |c: char| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '+' | '.');
    if !name.chars().all(ok)
        || name.starts_with('/')
        || name.split('/').any(|part| part.is_empty() || part.starts_with('.'))
    {
        return Err(format!("error: {:?} is not a zone name", name));
    }

    let path = Path::new(ZONEINFO).join(name);
    let mut magic = [0u8; 4];
    let readable = fs::File::open(&path)
        .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut magic))
        .is_ok();
    if !readable || &magic != b"TZif" {
        return Err(format!("error: no zone {:?} in {}", name, ZONEINFO));
    }
    Ok(())
}

/// Point `/etc/localtime` at a zone, atomically: the new link is made beside
/// the old one and renamed over it, so no reader ever finds the path missing.
fn set_zone(name: &str) -> Result<(), String> {
    validate_zone(name)?;

    let staging = format!("{}.new", LOCALTIME);
    let _ = fs::remove_file(&staging);
    std::os::unix::fs::symlink(format!("{}/{}", ZONEINFO, name), &staging)
        .map_err(|e| format!("error: cannot make the link: {}", e))?;
    fs::rename(&staging, LOCALTIME).map_err(|e| {
        let _ = fs::remove_file(&staging);
        format!("error: cannot replace {}: {}", LOCALTIME, e)
    })?;
    log::info!("Timezone set to {}", name);
    Ok(())
}

/// Every zone name under [`ZONEINFO`], sorted.
///
/// The convention that separates `America/New_York` from `zone.tab`,
/// `leapseconds` and the `posix`/`right` duplicate trees is that real zones'
/// path components begin with an uppercase letter -- `UTC` included -- and
/// the clutter's do not. Checking the magic on hundreds of files would say
/// the same thing slower.
fn list_zones() -> Vec<String> {
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                continue;
            }
            let full = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", prefix, name)
            };
            let path = entry.path();
            if path.is_dir() {
                walk(&path, &full, out);
            } else {
                out.push(full);
            }
        }
    }

    let mut zones = Vec::new();
    walk(Path::new(ZONEINFO), "", &mut zones);
    zones.sort();
    zones
}

// ---------------------------------------------------------------------------
// State shared between the sync loop and the socket
// ---------------------------------------------------------------------------

struct Shared {
    enabled: bool,
    last: Option<SyncOutcome>,
    /// Bumped by `sync on|off` so the loop re-decides its wait immediately
    /// instead of at the end of the interval it was sleeping through.
    generation: u64,
}

struct State {
    shared: Mutex<Shared>,
    wake: Condvar,
    servers: Vec<String>,
    interval: Duration,
}

/// Keep [`STATUS_MARKER`] saying whether the last attempt worked. Made 0644
/// explicitly rather than trusting the umask, because a reader that cannot
/// read the marker is the whole feature not working.
fn publish_status(synced: bool) {
    let word = if synced { "synced\n" } else { "unsynced\n" };
    match fs::write(STATUS_MARKER, word) {
        Ok(()) => {
            fs::set_permissions(STATUS_MARKER, fs::Permissions::from_mode(0o644)).ok();
        }
        Err(e) => log::debug!("Cannot publish {}: {}", STATUS_MARKER, e),
    }
}

/// The sync loop: sync, wait, repeat; back off while it fails; block while
/// it is turned off.
fn run_sync_loop(state: Arc<State>) {
    let mut retry = RETRY_FLOOR;
    loop {
        let (enabled, generation) = {
            let shared = state.shared.lock().unwrap_or_else(|e| e.into_inner());
            (shared.enabled, shared.generation)
        };

        let wait = if !enabled {
            // Nothing to do until someone turns sync back on; a day is only
            // a backstop under the condvar wake.
            Duration::from_secs(86_400)
        } else {
            match sync_once(&state.servers) {
                Ok(outcome) => {
                    publish_status(true);
                    let mut shared = state.shared.lock().unwrap_or_else(|e| e.into_inner());
                    shared.last = Some(outcome);
                    retry = RETRY_FLOOR;
                    state.interval
                }
                Err(e) => {
                    log::warn!("Sync failed: {}", e);
                    publish_status(false);
                    let wait = retry;
                    retry = (retry * 2).min(RETRY_CEILING);
                    wait
                }
            }
        };

        // Sleep the interval out, but leave early if a toggle bumped the
        // generation -- that is what makes `sync on` sync now, not next hour.
        let mut shared = state.shared.lock().unwrap_or_else(|e| e.into_inner());
        let deadline = std::time::Instant::now() + wait;
        while shared.generation == generation {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                break;
            }
            let (guard, _) = state
                .wake
                .wait_timeout(shared, left)
                .unwrap_or_else(|e| e.into_inner());
            shared = guard;
        }
    }
}

// ---------------------------------------------------------------------------
// The socket
// ---------------------------------------------------------------------------

/// One line from the socket, in full.
#[derive(Debug, PartialEq)]
enum CtlRequest {
    Status,
    ZoneQuery,
    SetZone(String),
    Zones,
    SyncNow,
    SyncEnable(bool),
    SetTime(i64),
}

impl CtlRequest {
    /// Strict on purpose, as powerd's parser is: every spelling accepted here
    /// is one more a client could send by mistake.
    fn parse(line: &str) -> Option<Self> {
        let mut words = line.split_whitespace();
        match (words.next(), words.next(), words.next()) {
            (Some("status"), None, None) => Some(CtlRequest::Status),
            (Some("zone"), None, None) => Some(CtlRequest::ZoneQuery),
            (Some("zone"), Some(name), None) => Some(CtlRequest::SetZone(name.to_string())),
            (Some("zones"), None, None) => Some(CtlRequest::Zones),
            (Some("sync"), None, None) => Some(CtlRequest::SyncNow),
            (Some("sync"), Some("on"), None) => Some(CtlRequest::SyncEnable(true)),
            (Some("sync"), Some("off"), None) => Some(CtlRequest::SyncEnable(false)),
            (Some("set"), Some(seconds), None) => {
                seconds.parse().ok().map(CtlRequest::SetTime)
            }
            _ => None,
        }
    }
}

/// Answer one client. The work happens on the client's thread -- there is no
/// cooldown and no main-loop policy here, so nothing needs marshalling the
/// way powerd's requests do; the shared mutex is the only coordination a
/// `sync` needs against the loop.
fn serve(request: CtlRequest, state: &Arc<State>) -> String {
    match request {
        CtlRequest::Status => {
            let shared = state.shared.lock().unwrap_or_else(|e| e.into_inner());
            let last = match &shared.last {
                Some(outcome) => format!(
                    "last {} {:+.3}s {}",
                    rfc3339(outcome.at),
                    outcome.offset,
                    outcome.server
                ),
                None => "last never".to_string(),
            };
            format!(
                "zone {}\nsync {}\n{}\n",
                current_zone(),
                if shared.enabled { "on" } else { "off" },
                last
            )
        }
        CtlRequest::ZoneQuery => format!("{}\n", current_zone()),
        CtlRequest::SetZone(name) => match set_zone(&name) {
            Ok(()) => format!("Zone {}\n", name),
            Err(e) => format!("{}\n", e),
        },
        CtlRequest::Zones => {
            let mut out = list_zones().join("\n");
            out.push('\n');
            out
        }
        CtlRequest::SyncNow => match sync_once(&state.servers) {
            Ok(outcome) => {
                publish_status(true);
                let reply = format!("Synced {:+.3}s via {}\n", outcome.offset, outcome.server);
                let mut shared = state.shared.lock().unwrap_or_else(|e| e.into_inner());
                shared.last = Some(outcome);
                reply
            }
            Err(e) => {
                publish_status(false);
                format!("error: sync failed: {}\n", e)
            }
        },
        CtlRequest::SyncEnable(value) => {
            if let Err(e) = persist_enabled(value) {
                // The toggle still applies for this boot; the file being
                // unwritable is worth telling the client about anyway.
                log::warn!("Sync {} not persisted: {}", if value { "on" } else { "off" }, e);
            }
            let mut shared = state.shared.lock().unwrap_or_else(|e| e.into_inner());
            shared.enabled = value;
            shared.generation += 1;
            drop(shared);
            state.wake.notify_all();
            log::info!("Automatic sync turned {}", if value { "on" } else { "off" });
            format!("Sync {}\n", if value { "on" } else { "off" })
        }
        CtlRequest::SetTime(seconds) => {
            if !(0..=253_402_300_799).contains(&seconds) {
                // Anything outside year 9999 is a client bug, not a request.
                return "error: not a time\n".to_string();
            }
            match step_clock(seconds as f64 - unix_now()) {
                Ok(()) => "Time set\n".to_string(),
                Err(e) => format!("error: {}\n", e),
            }
        }
    }
}

/// Bind, chown, tighten -- the same order and the same reasoning as powerd's
/// socket: group before mode, so there is no moment at which the file is
/// writable by a group it is not yet owned by.
fn bind_ctl_socket(path: &str) -> std::io::Result<UnixListener> {
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
            log::info!("Control socket listening on {} (group {})", path, CTL_GROUP);
        }
        None => {
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
            log::warn!("No group '{}' in /etc/group; {} is root-only", CTL_GROUP, path);
        }
    }

    Ok(listener)
}

/// The gid of a group, from `/etc/group`. Same hand parser powerd and init
/// use, for the same reason: a static binary must not need nss modules to
/// find a gid.
fn group_id(name: &str) -> Option<u32> {
    let text = fs::read_to_string("/etc/group").ok()?;
    text.lines().find_map(|line| {
        let mut fields = line.split(':');
        if fields.next()? != name {
            return None;
        }
        fields.next()?;
        fields.next()?.trim().parse().ok()
    })
}

fn accept_client(stream: UnixStream, state: &Arc<State>) {
    stream.set_read_timeout(Some(CTL_CLIENT_TIMEOUT)).ok();
    // A `sync` can spend SNTP_TIMEOUT per server before it has anything to
    // say; the write timeout only guards the reply itself.
    stream.set_write_timeout(Some(CTL_CLIENT_TIMEOUT)).ok();

    let mut line = String::new();
    let Ok(reader) = stream.try_clone() else {
        return;
    };
    if let Err(e) = BufReader::new(reader)
        .take(CTL_MAX_REQUEST)
        .read_line(&mut line)
    {
        log::warn!("{}: could not read request: {}", CTL_SOCKET_PATH, e);
        return;
    }

    let reply_text = match CtlRequest::parse(&line) {
        Some(request) => serve(request, state),
        None => {
            log::warn!("{}: unknown command {:?}", CTL_SOCKET_PATH, line.trim());
            "error: unknown command\n".to_string()
        }
    };

    let mut stream = stream;
    if let Err(e) = stream
        .write_all(reply_text.as_bytes())
        .and_then(|()| stream.flush())
    {
        log::debug!("{}: reply not delivered: {}", CTL_SOCKET_PATH, e);
    }
}

// ---------------------------------------------------------------------------
// RFC 3339, by hand
// ---------------------------------------------------------------------------

/// `2026-09-01T12:34:56Z` from Unix seconds. Hinnant's civil-from-days,
/// written out rather than pulled from a crate because it is a dozen lines
/// and this crate is PID 1's.
fn rfc3339(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let secs = unix % 86_400;

    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year,
        month,
        day,
        secs / 3600,
        (secs / 60) % 60,
        secs % 60
    )
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    // With arguments this binary is a client of the daemon, not the daemon.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        std::process::exit(run_client(&args.join(" ")));
    }

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = Config::load();
    log::info!(
        "raven-timed: zone {}, sync {} ({} servers, every {} min)",
        current_zone(),
        if config.sync.enabled { "on" } else { "off" },
        config.sync.servers.len(),
        config.sync.interval_minutes
    );

    let state = Arc::new(State {
        shared: Mutex::new(Shared {
            enabled: config.sync.enabled,
            last: None,
            generation: 0,
        }),
        wake: Condvar::new(),
        servers: config.sync.servers,
        interval: Duration::from_secs(config.sync.interval_minutes.max(1) * 60),
    });

    {
        let state = Arc::clone(&state);
        thread::Builder::new()
            .name("timed-sync".to_string())
            .stack_size(64 * 1024)
            .spawn(move || run_sync_loop(state))
            .expect("cannot start the sync loop");
    }

    let listener = match bind_ctl_socket(CTL_SOCKET_PATH) {
        Ok(listener) => listener,
        Err(e) => {
            // Unlike powerd there is no second job to fall back to that needs
            // the process alive-but-deaf; without the socket this daemon is
            // only the sync loop, and that is still worth running.
            log::warn!("Not listening on {}: {}", CTL_SOCKET_PATH, e);
            loop {
                thread::sleep(Duration::from_secs(3600));
            }
        }
    };

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = Arc::clone(&state);
                thread::Builder::new()
                    .name("timed-ctl".to_string())
                    .stack_size(64 * 1024)
                    .spawn(move || accept_client(stream, &state))
                    .ok();
            }
            Err(e) => log::warn!("{}: accept failed: {}", CTL_SOCKET_PATH, e),
        }
    }
}

/// Send one line to the running daemon, print the reply, and exit the way a
/// shell expects: 0 unless the daemon said `error:` or could not be reached.
fn run_client(line: &str) -> i32 {
    let mut stream = match UnixStream::connect(CTL_SOCKET_PATH) {
        Ok(stream) => stream,
        Err(e) => {
            eprintln!("raven-timed is not running ({}: {})", CTL_SOCKET_PATH, e);
            return 1;
        }
    };

    if stream
        .write_all(format!("{}\n", line).as_bytes())
        .and_then(|()| stream.flush())
        .is_err()
    {
        eprintln!("could not send the request");
        return 1;
    }

    let mut reply = String::new();
    if stream.read_to_string(&mut reply).is_err() {
        eprintln!("no reply");
        return 1;
    }
    print!("{}", reply);
    i32::from(reply.starts_with("error"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_socket_knows_its_verbs() {
        assert_eq!(CtlRequest::parse("status\n"), Some(CtlRequest::Status));
        assert_eq!(CtlRequest::parse("zone\n"), Some(CtlRequest::ZoneQuery));
        assert_eq!(
            CtlRequest::parse("zone America/New_York\n"),
            Some(CtlRequest::SetZone("America/New_York".to_string()))
        );
        assert_eq!(CtlRequest::parse("zones\n"), Some(CtlRequest::Zones));
        assert_eq!(CtlRequest::parse("sync\n"), Some(CtlRequest::SyncNow));
        assert_eq!(CtlRequest::parse("sync on\n"), Some(CtlRequest::SyncEnable(true)));
        assert_eq!(CtlRequest::parse("sync off\n"), Some(CtlRequest::SyncEnable(false)));
        assert_eq!(
            CtlRequest::parse("set 1756728000\n"),
            Some(CtlRequest::SetTime(1_756_728_000))
        );
    }

    #[test]
    fn the_socket_rejects_near_misses() {
        assert_eq!(CtlRequest::parse("zone America/New_York extra\n"), None);
        assert_eq!(CtlRequest::parse("sync maybe\n"), None);
        assert_eq!(CtlRequest::parse("set noon\n"), None);
        assert_eq!(CtlRequest::parse("set\n"), None);
        assert_eq!(CtlRequest::parse("timezone UTC\n"), None);
        assert_eq!(CtlRequest::parse("\n"), None);
        assert_eq!(CtlRequest::parse(""), None);
    }

    #[test]
    fn a_zone_name_cannot_walk_the_filesystem() {
        // The validator is the only thing between a socket line and a path.
        assert!(validate_zone("../../../etc/shadow").is_err());
        assert!(validate_zone("/etc/shadow").is_err());
        assert!(validate_zone("America/../../etc/shadow").is_err());
        assert!(validate_zone("America//New_York").is_err());
        assert!(validate_zone(".hidden").is_err());
        assert!(validate_zone("").is_err());
        assert!(validate_zone("Amer ica").is_err());
    }

    // NTP timestamps: 1 Jan 1970 00:00 UTC is exactly the epoch offset into
    // the NTP era, and the fraction field is binary, not decimal.

    #[test]
    fn the_unix_epoch_reads_as_zero() {
        let raw = (2_208_988_800u64) << 32;
        assert_eq!(ntp_to_unix(raw), 0.0);
    }

    #[test]
    fn half_a_second_is_half_the_fraction_field() {
        let raw = ((2_208_988_800u64 + 1) << 32) | 0x8000_0000;
        let unix = ntp_to_unix(raw);
        assert!((unix - 1.5).abs() < 1e-6);
    }

    #[test]
    fn a_timestamp_survives_the_round_trip() {
        let mut bytes = [0u8; 8];
        write_ntp_timestamp(&mut bytes, 1_756_728_000.125);
        let back = ntp_to_unix(read_ntp_timestamp(&bytes));
        assert!((back - 1_756_728_000.125).abs() < 1e-6);
    }

    #[test]
    fn rfc3339_agrees_with_the_calendar() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        // date -u -d @1788264000
        assert_eq!(rfc3339(1_788_264_000), "2026-09-01T12:00:00Z");
        // A leap year's extra day.
        assert_eq!(rfc3339(1_709_164_800), "2024-02-29T00:00:00Z");
    }

    #[test]
    fn defaults_keep_time_and_name_the_pool() {
        let config = Config::default();
        assert!(config.sync.enabled);
        assert_eq!(config.sync.servers.len(), 3);
        assert_eq!(config.sync.interval_minutes, 60);
    }

    #[test]
    fn config_overrides_only_what_it_names() {
        let config: Config = toml::from_str("[sync]\nenabled = false\n").unwrap();
        assert!(!config.sync.enabled);
        // Untouched by a file that says nothing about them.
        assert_eq!(config.sync.servers.len(), 3);
    }

    /// The file this repo ships must deserialize with the schema the daemon
    /// actually uses. Without this, a renamed key is only discovered on a
    /// machine whose clock has quietly stopped being kept.
    #[test]
    fn the_shipped_config_parses() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../etc/raven/time.toml");
        let text = std::fs::read_to_string(&path).expect("etc/raven/time.toml is readable");
        let config: Config = toml::from_str(&text).expect("etc/raven/time.toml parses");
        assert!(config.sync.enabled);
        assert!(!config.sync.servers.is_empty());
    }
}
