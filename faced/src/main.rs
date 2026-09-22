//! `raven-faced` -- the one process that touches the camera.
//!
//! # Why a daemon, and why this one
//!
//! For the reason `raven-fprintd` is one, and the argument is worth repeating
//! because it is the whole shape of this: the lock screen runs as the person
//! logged in. If it opened the camera itself and then told `ravend` that a
//! face had matched, that claim would carry no secret and *any* process
//! running as that person could make it. A lock screen exists to stop somebody
//! standing at the keyboard; a process already inside the session is not that
//! somebody, and it must not be able to become them.
//!
//! It is not `ravend` itself because the process holding `/etc/shadow` should
//! not also be the process that opens a video device, decodes JPEG from it and
//! runs two neural networks over the result. That is a large amount of
//! attack surface pointed at a lens, and this daemon is where it goes. It has
//! no account database, it cannot start a session, and it cannot unlock
//! anything. It answers one question -- whose stored face, if anybody's, is in
//! front of the camera right now -- and `ravend` decides what that is worth.
//!
//! # The socket
//!
//! `/run/raven-face/camera.sock`, in a `0700` directory owned by root, and
//! every connection's credentials are checked besides. Root only: this socket
//! reports matches, and anything that could ask it could ask about any
//! account.
//!
//! One line in, lines out, like `raven-fprintd`'s. See [`serve`] for the verbs.
//!
//! # No image leaves this process
//!
//! There is no verb that returns a frame, there is no preview, and nothing
//! this daemon writes to disk is an image -- an enrolment stores 128 numbers
//! per capture and never the capture. The login screen is the least trusted
//! process on the machine and it is the one thing that must never be handed
//! video of somebody's face.
//!
//! # One at a time
//!
//! There is one camera, so there is one connection being served. Another waits
//! in the listen backlog.

mod camera;
mod config;
mod liveness;
mod model;
mod store;
mod v4l2;

use std::io::{BufRead, BufReader, Write};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use camera::Camera;
use config::Config;
use liveness::{Challenge, Sample, Verdict};
use model::{Face, Models};

const SOCKET_DIR: &str = "/run/raven-face";
const SOCKET_PATH: &str = "/run/raven-face/camera.sock";

/// How long a camera that would not open is left alone before the next try.
///
/// `raven-fprintd`'s reasoning: opening a device that is not answering takes
/// seconds, and without this every request would try again and queue behind
/// the last one.
const RETRY_OPEN_AFTER: Duration = Duration::from_secs(10);

/// How long one `verify` waits for a face before giving up on the round.
///
/// Not a limit on the watch: `ravend` starts another round after a miss, and
/// the connection stays open for as long as somebody is looking at the screen.
/// This is how often the person gets told something.
const ROUND: Duration = Duration::from_secs(20);

/// How long to wait for any one frame.
const FRAME_WAIT: Duration = Duration::from_millis(400);

/// How often, at most, to send a correction. Faster than this and the line
/// under the field flickers between two pieces of advice.
const ADVICE_EVERY: Duration = Duration::from_millis(1200);

/// Good captures one enrolment wants. Matches `raven-face::ENROL_CAPTURES`.
const ENROL_CAPTURES: u8 = 5;

/// How far apart enrolment captures have to be, so five captures are five
/// looks at somebody rather than five copies of one frame.
const ENROL_SPACING: Duration = Duration::from_millis(220);

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // `raven-faced --selftest` opens the camera, looks for a face, and says
    // what it found. It binds no socket and stores nothing, so it can be run
    // by anybody in the `video` group -- which is the point: the thing most
    // worth checking on a new machine is whether the camera and the models
    // work at all, and doing that by watching a login screen fail tells you
    // nothing about which half is wrong.
    let selftest = std::env::args().any(|a| a == "--selftest");
    let result = if selftest { selftest_run() } else { run() };
    if let Err(e) = result {
        log::error!("{e}");
        std::process::exit(1);
    }
}

/// Open the camera, take a few frames, and report what the models make of
/// them. Nothing is stored and no socket is bound.
fn selftest_run() -> anyhow::Result<()> {
    let config = Config::load()?;
    let dir = config.models.clone().unwrap_or_else(model::model_dir);
    println!("models:   {}", dir.display());
    let models = Models::load(&dir)?;
    println!("          loaded");

    let mut camera = Camera::find(config.device.as_deref())?
        .ok_or_else(|| anyhow::anyhow!("no camera this can capture from"))?;
    println!(
        "camera:   {} ({})",
        camera.device().display(),
        if camera.is_infrared() {
            "infrared"
        } else {
            "visible light"
        }
    );
    camera.start()?;

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut frames = 0u32;
    let mut first: Option<[f32; store::DIM]> = None;
    while Instant::now() < deadline {
        let Some(frame) = camera.frame(FRAME_WAIT)? else {
            continue;
        };
        frames += 1;
        let started = Instant::now();
        let (face, crowded) = models.detect(&frame, config.detect_threshold)?;
        let detect_ms = started.elapsed().as_millis();
        let Some(face) = face else {
            if frames % 10 == 0 {
                println!("frame {frames}: no face ({detect_ms} ms)");
            }
            continue;
        };
        let started = Instant::now();
        let vector = models.embed(&frame, &face)?;
        println!(
            "frame {frames}: score {:.2}, fill {:.2}, off-centre {:.2}{}, \
             detect {detect_ms} ms, embed {} ms{}",
            face.score,
            face.fill(frame.width, frame.height),
            face.off_centre(frame.width, frame.height),
            if crowded { ", crowded" } else { "" },
            started.elapsed().as_millis(),
            match (&first, &vector) {
                (Some(a), Some(b)) => format!(", same as the first: {:.3}", store::similarity(a, b)),
                _ => String::new(),
            }
        );
        if first.is_none() {
            first = vector;
        }
        if frames > 40 {
            break;
        }
    }
    if first.is_none() {
        anyhow::bail!("no face was found in {frames} frames");
    }
    println!("ok        the camera and both models work");
    Ok(())
}

fn run() -> anyhow::Result<()> {
    // Before the socket, so a machine with a damaged config gets a daemon that
    // failed to start with a reason -- rather than one that is running, is
    // answering, and is using numbers nobody chose.
    let config = Config::load()?;

    let listener = listen()?;
    log::info!("listening on {SOCKET_PATH}");

    // The models are loaded eagerly and the camera is not, and the difference
    // is what each answer is worth. "There are no models" is a fixed fact about
    // this machine that a settings page needs at once; "there is no camera" can
    // change when somebody plugs one in, and must not need a restart.
    let models = match Models::load(&config.models.clone().unwrap_or_else(model::model_dir)) {
        Ok(models) => {
            log::info!("loaded the face models");
            Some(models)
        }
        Err(e) => {
            // Not fatal. A daemon that refused to start would answer nothing
            // at all, and "the models are not installed" is exactly the answer
            // the settings page needs to show somebody the way out.
            log::warn!("face unlock is unavailable: {e}");
            None
        }
    };

    let mut slot = Slot::new(config.device.clone());
    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(e) => {
                log::warn!("accept failed: {e}");
                continue;
            }
        };
        match peer_uid(&stream) {
            Ok(0) => {}
            Ok(uid) => {
                log::warn!("refused a connection from uid {uid}");
                let _ = writeln!(stream, "error this socket is root's");
                continue;
            }
            Err(e) => {
                log::warn!("cannot read peer credentials: {e}");
                continue;
            }
        }
        if let Err(e) = serve(stream, &mut slot, models.as_ref(), &config) {
            log::debug!("connection ended: {e}");
        }
        // The camera is put down between connections. A webcam with its light
        // on is a webcam somebody is looking at wondering why.
        slot.release();
    }
    Ok(())
}

fn listen() -> std::io::Result<UnixListener> {
    std::fs::create_dir_all(SOCKET_DIR)?;
    std::fs::set_permissions(SOCKET_DIR, std::fs::Permissions::from_mode(0o700))?;
    // A socket left by a daemon that was killed rather than stopped. Safe to
    // remove because the directory is root's: nothing else could have put a
    // file here to be clobbered.
    if Path::new(SOCKET_PATH).exists() {
        std::fs::remove_file(SOCKET_PATH)?;
    }
    let listener = UnixListener::bind(SOCKET_PATH)?;
    std::fs::set_permissions(SOCKET_PATH, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

fn peer_uid(stream: &UnixStream) -> std::io::Result<u32> {
    let mut creds: libc::ucred = v4l2::zeroed();
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: `stream` is a connected unix socket, and `creds`/`len` are a
    // valid ucred and its size, which is what SO_PEERCRED writes.
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            std::ptr::addr_of_mut!(creds).cast(),
            &mut len,
        )
    };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(creds.uid)
}

/// The camera, opened on demand, and how the last attempt to open it went.
struct Slot {
    prefer: Option<PathBuf>,
    camera: Option<Camera>,
    failed: Option<(Instant, String)>,
}

impl Slot {
    fn new(prefer: Option<PathBuf>) -> Self {
        Self {
            prefer,
            camera: None,
            failed: None,
        }
    }

    /// The open camera, opening one if there is not already one; `None` for a
    /// machine with no camera.
    fn open(&mut self) -> std::io::Result<Option<&mut Camera>> {
        if self.camera.is_none() {
            if let Some((at, why)) = &self.failed {
                if at.elapsed() < RETRY_OPEN_AFTER {
                    return Err(std::io::Error::other(why.clone()));
                }
            }
            match Camera::find(self.prefer.as_deref()) {
                Ok(camera) => {
                    self.camera = camera;
                    self.failed = None;
                }
                Err(e) => {
                    log::warn!("cannot open the camera: {e}");
                    self.failed = Some((Instant::now(), e.to_string()));
                    return Err(e);
                }
            }
        }
        Ok(self.camera.as_mut())
    }

    /// Stop streaming, keeping the device open and its buffers mapped.
    fn release(&mut self) {
        if let Some(camera) = &mut self.camera {
            camera.stop();
        }
    }
}

/// Serve one connection until it closes.
///
/// Verbs, one per line:
///
/// | Line | Answers |
/// |---|---|
/// | `status` | `ok present <device> <0\|1 infrared>`, `ok absent`, or `ok nomodel <why>` |
/// | `list <account>` | `look <id> <added> <label>` per stored look, then `ok` |
/// | `verify <account> <0\|1 flash>` | `flash`/`retry` lines, then `match <account>:<id>`, `nomatch` or `spoof` |
/// | `enrol <account> <0\|1 flash> <label>` | `flash`/`frame`/`retry` lines, then `ok <id> <added>` |
/// | `forget <account> <id>` | `ok` |
/// | `forget-all <account>` | `ok` |
///
/// Every error is a line beginning `error` and never a closed connection: a
/// caller that cannot tell a broken camera from a crashed daemon has to treat
/// both as the worst case.
fn serve(
    stream: UnixStream,
    slot: &mut Slot,
    models: Option<&Models>,
    config: &Config,
) -> std::io::Result<()> {
    // Three handles on the one connection, held at once: the reader owns its
    // buffer for the whole conversation, replies are written while that reader
    // still has it, and a wait for a face polls a third alongside the camera.
    let reader = BufReader::new(stream.try_clone()?);
    let cancel = stream.try_clone()?;
    let mut out = stream;

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let verb = parts.next().unwrap_or("");
        let result = match verb {
            "status" => status(&mut out, slot, models),
            "list" => match parts.next() {
                Some(account) => list(&mut out, account),
                None => writeln!(out, "error list wants an account"),
            },
            "forget" => match (parts.next(), parts.next().and_then(|w| w.parse().ok())) {
                (Some(account), Some(id)) => forget(&mut out, account, Some(id)),
                _ => writeln!(out, "error forget wants an account and a number"),
            },
            "forget-all" => match parts.next() {
                Some(account) => forget(&mut out, account, None),
                None => writeln!(out, "error forget-all wants an account"),
            },
            "verify" => {
                let account = parts.next().unwrap_or("").to_string();
                let flash = parts.next() == Some("1");
                if account.is_empty() {
                    writeln!(out, "error verify wants an account")?;
                    continue;
                }
                ready(slot, models).and_then(|(camera, models)| {
                    verify(&mut out, camera, models, config, &account, flash, &cancel)
                })
            }
            "enrol" | "enroll" => {
                let account = parts.next().unwrap_or("").to_string();
                let flash = parts.next() == Some("1");
                // The label is the rest of the line, and may be empty or
                // contain spaces.
                let label = line
                    .splitn(4, ' ')
                    .nth(3)
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if account.is_empty() {
                    writeln!(out, "error enrol wants an account")?;
                    continue;
                }
                ready(slot, models).and_then(|(camera, models)| {
                    enrol(&mut out, camera, models, config, &account, &label, flash, &cancel)
                })
            }
            other => {
                writeln!(out, "error unknown verb {other}")?;
                continue;
            }
        };
        if let Err(e) = result {
            writeln!(out, "error {e}")?;
            // A camera that failed may be unplugged, wedged, or merely
            // confused. Dropping it means the next request opens it again,
            // which recovers every one of those without a restart.
            slot.camera = None;
        }
    }
    Ok(())
}

/// The camera and the models, or an error saying which one is missing.
fn ready<'a>(
    slot: &'a mut Slot,
    models: Option<&'a Models>,
) -> std::io::Result<(&'a mut Camera, &'a Models)> {
    let models = models.ok_or_else(|| {
        std::io::Error::other("the face recognition models are not installed")
    })?;
    let camera = slot
        .open()?
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no camera"))?;
    Ok((camera, models))
}

fn status(out: &mut UnixStream, slot: &mut Slot, models: Option<&Models>) -> std::io::Result<()> {
    if models.is_none() {
        // Said before the camera is looked at, because it is the answer either
        // way: a camera this cannot run anything on is not a camera face
        // unlock has. Its own state on the wire, and not `absent`, because the
        // fix is installing a file and not buying hardware.
        return writeln!(out, "ok nomodel the face recognition models are not installed");
    }
    match slot.open() {
        Ok(Some(camera)) => writeln!(
            out,
            "ok present {} {}",
            camera.device().display(),
            u8::from(camera.is_infrared())
        ),
        Ok(None) => writeln!(out, "ok absent"),
        Err(e) => writeln!(out, "error {e}"),
    }
}

fn list(out: &mut UnixStream, account: &str) -> std::io::Result<()> {
    for look in store::load(Path::new(store::DIR), account)? {
        writeln!(out, "look {} {} {}", look.id, look.added, look.label)?;
    }
    writeln!(out, "ok")
}

fn forget(out: &mut UnixStream, account: &str, id: Option<u8>) -> std::io::Result<()> {
    let dir = Path::new(store::DIR);
    match id {
        Some(id) => store::forget(dir, account, id)?,
        None => store::forget_all(dir, account)?,
    }
    log::info!("forgot {account}'s face {id:?}");
    writeln!(out, "ok")
}

/// Whether the client has gone. Anything at all on the connection ends a wait.
fn hung_up(client: BorrowedFd<'_>) -> bool {
    let mut fds = libc::pollfd {
        fd: client.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: one valid pollfd, count 1, no wait.
    let r = unsafe { libc::poll(&mut fds, 1, 0) };
    r > 0 && fds.revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0
}

/// Why a frame could not be used, as the word the caller puts on screen.
fn unusable(
    face: Option<&Face>,
    crowded: bool,
    frame: &camera::Frame,
    config: &Config,
) -> Option<&'static str> {
    let Some(face) = face else {
        // A frame with no face in it at all, and almost no light in it either,
        // is a covered lens rather than an empty chair.
        let [b, g, r] = frame.mean_bgr(0, 0, frame.width as i32, frame.height as i32);
        return Some(if (b + g + r) / 3.0 < 0.03 {
            "covered"
        } else {
            "look"
        });
    };
    if crowded {
        return Some("alone");
    }
    let fill = face.fill(frame.width, frame.height);
    if fill < config.min_fill {
        return Some("far");
    }
    if fill > config.max_fill {
        return Some("near");
    }
    if face.is_turned_away() || face.off_centre(frame.width, frame.height) > 0.3 {
        return Some("angle");
    }
    // The box has to be entirely in the frame. A face half off the edge has
    // cells in the liveness grid that measure nothing, and a cell that
    // measures nothing looks exactly like one in deep shadow -- which is the
    // signal the relief test reads as "this has a nose".
    if face.x < 0.0
        || face.y < 0.0
        || face.x + face.width > frame.width as f32
        || face.y + face.height > frame.height as f32
    {
        return Some("look");
    }
    let [b, g, r] = frame.mean_bgr(
        face.x as i32,
        face.y as i32,
        face.width as i32,
        face.height as i32,
    );
    if (b + g + r) / 3.0 < 0.08 {
        return Some("dark");
    }
    None
}

/// One usable look at somebody, or the word for what to do differently.
struct Look {
    frame: camera::Frame,
    face: Face,
}

/// Wait for a usable frame, sending corrections at most every
/// [`ADVICE_EVERY`]. `Ok(None)` if the deadline passed or the client went.
fn wait_for_face(
    out: &mut UnixStream,
    camera: &mut Camera,
    models: &Models,
    config: &Config,
    client: BorrowedFd<'_>,
    deadline: Instant,
    said: &mut Option<(&'static str, Instant)>,
) -> std::io::Result<Option<Look>> {
    while Instant::now() < deadline {
        if hung_up(client) {
            return Ok(None);
        }
        let Some(frame) = camera.frame(FRAME_WAIT)? else {
            continue;
        };
        let (face, crowded) = models
            .detect(&frame, config.detect_threshold)
            .map_err(|e| std::io::Error::other(format!("the detector failed: {e}")))?;
        match unusable(face.as_ref(), crowded, &frame, config) {
            Some(word) => {
                let now = Instant::now();
                let due = said.map_or(true, |(last, at)| {
                    last != word || now.duration_since(at) > ADVICE_EVERY
                });
                if due {
                    writeln!(out, "retry {word}")?;
                    out.flush()?;
                    *said = Some((word, now));
                }
            }
            None => {
                return Ok(Some(Look {
                    frame,
                    face: face.expect("a usable frame has a face"),
                }));
            }
        }
    }
    Ok(None)
}

/// Run the liveness challenge, painting through `out` and watching `camera`.
///
/// The face's box is taken once, before the sequence starts, and reused for
/// every sample: detecting on each frame would cost more than the dwell time
/// and would move the measurement box under somebody who leaned, which is the
/// same signal the check is looking for. Somebody is asked to hold still, and
/// the sequence lasts less than a second.
fn challenge(
    out: &mut UnixStream,
    camera: &mut Camera,
    config: &Config,
    face: &Face,
    client: BorrowedFd<'_>,
) -> std::io::Result<Verdict> {
    let Some(challenge) = Challenge::new() else {
        return Err(std::io::Error::other("cannot read this machine's randomness"));
    };
    // For as long as this lives, the camera stops undoing the very thing being
    // measured. See `Camera::pin_exposure`.
    let _pinned = camera.pin_exposure();

    let mut samples = Vec::new();
    for (step, colour) in challenge.steps.iter().enumerate() {
        writeln!(out, "flash {} {} {}", colour[0], colour[1], colour[2])?;
        out.flush()?;
        let up = Instant::now();
        let until = up + liveness::DWELL;

        while Instant::now() < until {
            if hung_up(client) {
                let _ = writeln!(out, "flash off");
                return Ok(Verdict::Unsure("still"));
            }
            let Some(frame) = camera.frame(FRAME_WAIT)? else {
                continue;
            };
            // Frames from inside the settling window show the colour before
            // this one; counting them smears every step into its neighbour.
            if up.elapsed() < liveness::SETTLE {
                continue;
            }
            samples.push(Sample {
                step,
                cells: cells_of(&frame, face),
            });
        }
    }
    writeln!(out, "flash off")?;
    out.flush()?;

    Ok(liveness::judge(&challenge, &samples, config.limits()))
}

/// The face's box split into a grid, each cell's mean colour.
fn cells_of(frame: &camera::Frame, face: &Face) -> [[f32; 3]; liveness::GRID * liveness::GRID] {
    let mut cells = [[0f32; 3]; liveness::GRID * liveness::GRID];
    let cw = face.width / liveness::GRID as f32;
    let ch = face.height / liveness::GRID as f32;
    for row in 0..liveness::GRID {
        for col in 0..liveness::GRID {
            cells[row * liveness::GRID + col] = frame.mean_bgr(
                (face.x + col as f32 * cw) as i32,
                (face.y + row as f32 * ch) as i32,
                cw as i32,
                ch as i32,
            );
        }
    }
    cells
}

/// Wait for `account`'s face and say what was seen.
///
/// # The order the two questions are asked in
///
/// Identity first, then liveness, and the order is deliberate. A photograph of
/// somebody who is not this account's owner fails the comparison and is
/// answered `nomatch` -- the same answer any stranger gets, saying nothing
/// about whether the liveness check would have caught it. Only something that
/// *would otherwise have been let in* is reported as a spoof. So the one
/// person who learns anything from a `spoof` line is the account's own owner,
/// standing in bad light, who needs to know why their face is not working.
fn verify(
    out: &mut UnixStream,
    camera: &mut Camera,
    models: &Models,
    config: &Config,
    account: &str,
    flash: bool,
    client: &UnixStream,
) -> std::io::Result<()> {
    let looks = store::load(Path::new(store::DIR), account)?;
    if looks.is_empty() {
        return writeln!(out, "error no face is enrolled for that account");
    }
    // A colour camera with a screen that will not paint has no liveness check
    // worth the name. `ravend` refuses this case before it gets here; this is
    // the second lock on the same door, because the socket is reachable by
    // anything running as root and this daemon must not rely on its one caller
    // having been careful.
    if !flash && !camera.is_infrared() {
        return writeln!(
            out,
            "error a face cannot be checked for life on this machine without the screen"
        );
    }

    camera.start()?;
    let deadline = Instant::now() + ROUND;
    let mut said = None;

    let Some(look) = wait_for_face(out, camera, models, config, client.as_fd(), deadline, &mut said)?
    else {
        return writeln!(out, "nomatch");
    };

    let Some(seen) = models
        .embed(&look.frame, &look.face)
        .map_err(|e| std::io::Error::other(format!("the recogniser failed: {e}")))?
    else {
        return writeln!(out, "retry look");
    };

    let best = looks
        .iter()
        .flat_map(|stored| {
            stored
                .vectors
                .iter()
                .map(move |v| (stored.id, store::similarity(&seen, v)))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1));

    let Some((id, score)) = best else {
        return writeln!(out, "nomatch");
    };
    log::debug!("{account}: best match is look {id} at {score:.3}");
    if score < config.threshold {
        return writeln!(out, "nomatch");
    }

    // It is them. Now: is it them, or a picture of them?
    if camera.is_infrared() {
        // An infrared sensor sees a warm face and does not see a photograph of
        // one. There is nothing for a screen to add.
        log::info!("{account} matched look {id} at {score:.3} on the infrared camera");
        return writeln!(out, "match {account}:{id}");
    }
    match challenge(out, camera, config, &look.face, client.as_fd())? {
        Verdict::Live => {
            log::info!("{account} matched look {id} at {score:.3} and is live");
            writeln!(out, "match {account}:{id}")
        }
        Verdict::Spoof => {
            log::warn!("{account} matched look {id} at {score:.3} but failed the liveness check");
            writeln!(out, "spoof")
        }
        Verdict::Unsure(word) => {
            log::info!("{account} matched look {id} but the liveness check could not run: {word}");
            writeln!(out, "retry {word}")
        }
    }
}

/// Take enough good looks at `account` to store one template.
fn enrol(
    out: &mut UnixStream,
    camera: &mut Camera,
    models: &Models,
    config: &Config,
    account: &str,
    label: &str,
    flash: bool,
    client: &UnixStream,
) -> std::io::Result<()> {
    if !store::valid_account(account) {
        return writeln!(out, "error that is not an account name that can be stored");
    }
    camera.start()?;

    // Long enough for somebody to find the camera and sit still, and not so
    // long that a dialog nobody is in front of holds the device all afternoon.
    let deadline = Instant::now() + ROUND * 3;
    let mut said = None;
    let mut vectors = Vec::new();
    let mut last_face: Option<(camera::Frame, Face)> = None;
    let mut last_capture: Option<Instant> = None;

    while vectors.len() < usize::from(ENROL_CAPTURES) && Instant::now() < deadline {
        let Some(look) = wait_for_face(
            out,
            camera,
            models,
            config,
            client.as_fd(),
            deadline,
            &mut said,
        )?
        else {
            break;
        };
        // Spaced out, so five captures are five looks at somebody rather than
        // five copies of one frame -- a template built from one instant covers
        // one instant.
        if last_capture.is_some_and(|at| at.elapsed() < ENROL_SPACING) {
            continue;
        }
        let Some(vector) = models
            .embed(&look.frame, &look.face)
            .map_err(|e| std::io::Error::other(format!("the recogniser failed: {e}")))?
        else {
            continue;
        };
        vectors.push(vector);
        last_capture = Some(Instant::now());
        writeln!(out, "frame {} {ENROL_CAPTURES}", vectors.len())?;
        out.flush()?;
        last_face = Some((look.frame, look.face));
    }

    if vectors.len() < usize::from(ENROL_CAPTURES) {
        return writeln!(
            out,
            "error could not get a good enough look at you to store a face"
        );
    }

    // The liveness check on the way in, so that what gets stored is a person
    // and not whatever was held up to the lens. It costs nothing here -- the
    // password has already been taken, and somebody enrolling a photograph of
    // themselves is only making their own face unlock worse -- but a template
    // taken from a printed photo would be a template that a printed photo
    // matches especially well, which is the wrong thing to have on disk.
    if flash && !camera.is_infrared() {
        if let Some((_, face)) = &last_face {
            if let Verdict::Spoof = challenge(out, camera, config, face, client.as_fd())? {
                log::warn!("refused to enrol a face for {account}: it did not look live");
                return writeln!(
                    out,
                    "error that did not look like a live face, so it was not stored"
                );
            }
        }
    }

    let stored = store::save(Path::new(store::DIR), account, label, vectors)?;
    log::info!(
        "enrolled face {} for {account}{}",
        stored.id,
        if stored.label.is_empty() {
            String::new()
        } else {
            format!(" ({})", stored.label)
        }
    );
    writeln!(out, "ok {} {}", stored.id, stored.added)
}

