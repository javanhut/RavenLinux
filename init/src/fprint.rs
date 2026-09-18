//! The Elan match-on-chip fingerprint sensor, over usbfs.
//!
//! RavenLinux ships no `libfprint` and no `libusb`, for the reason it ships no
//! `wpa_supplicant`: the image is static musl binaries, and the stack under
//! the one operation that has to work when everything else has gone wrong
//! should not be the largest C dependency on the machine. So this talks to the
//! device the way `libusb` itself does — `ioctl` on a node under
//! `/dev/bus/usb` — with `libc` and nothing else.
//!
//! # Provenance, and the licence question that comes with it
//!
//! The wire protocol below is Elan's, and the only public description of it is
//! `libfprint`'s `elanmoc` driver, which is **LGPL-2.1-or-later** and copyright
//! Elan Microelectronics. What is taken from it here are interface facts —
//! endpoint numbers, command bytes, response codes, how many frames a template
//! wants — and the code itself is written fresh against those facts; none of
//! it is a translation of theirs. That is the ordinary basis for
//! reimplementing a driver, and it is not a lawyer's opinion. If RavenLinux
//! wants certainty, the alternative is to ship this file under LGPL-2.1+ with
//! attribution rather than under the repository's MIT, and that is a decision
//! for whoever owns the licensing and not for this comment.
//!
//! # Match on chip
//!
//! The template never leaves the sensor. Enrolment feeds it frames until it
//! says it has enough; verification asks it a question and it answers with the
//! index of whichever stored finger matched. No image of a fingerprint is ever
//! read into this process, which is the property worth protecting: a host that
//! cannot read a template cannot leak one. Nothing below ever asks for one.
//!
//! # Cancellation
//!
//! Waiting for a finger has no timeout — somebody may be away from the machine
//! for an hour — so the read cannot be a blocking `ioctl`, or the daemon could
//! never be told to stop. Instead the URB is submitted, the file descriptor is
//! polled alongside a pipe the caller can write to, and a wake on the pipe
//! discards the URB. See [`Sensor::wait`].

use std::io;
use std::os::fd::{AsRawFd, BorrowedFd, OwnedFd, RawFd};
use std::path::PathBuf;

/// Elan's USB vendor id.
const ELAN_VENDOR: u16 = 0x04f3;

/// The product ids this driver claims.
///
/// Every one of them is a match-on-chip sensor speaking the protocol below.
/// `0x0c00` is the one in the machine this was written on; the rest are the
/// list `libfprint` carries, kept because a driver that works on one Elan
/// laptop and not the next one is not worth having.
const ELAN_PRODUCTS: &[u16] = &[
    0x0c00, 0x0c01, 0x0c02, 0x0c03, 0x0c04, 0x0c05, 0x0c06, 0x0c07, 0x0c08,
    0x0c09, 0x0c0a, 0x0c0b, 0x0c0c, 0x0c0d, 0x0c0e, 0x0c0f, 0x0c10, 0x0c11,
    0x0c12, 0x0c13, 0x0c14, 0x0c15, 0x0c16, 0x0c17, 0x0c18, 0x0c19, 0x0c1a,
    0x0c1b, 0x0c1c, 0x0c1d, 0x0c1e, 0x0c1f, 0x0c20, 0x0c21, 0x0c22, 0x0c23,
    0x0c24, 0x0c25, 0x0c26, 0x0c27, 0x0c28, 0x0c29, 0x0c2a, 0x0c2b, 0x0c2c,
    0x0c2d, 0x0c2e, 0x0c2f, 0x0c30, 0x0c31, 0x0c32, 0x0c33, 0x0c3d, 0x0c42,
    0x0c4b, 0x0c4d, 0x0c4f, 0x0c58, 0x0c63, 0x0c6e, 0x0c71, 0x0c72, 0x0c7d,
    0x0c82, 0x0c88, 0x0c8c, 0x0c8d, 0x0c99, 0x0c9c, 0x0ca5, 0x0caf, 0x0cb0,
];

/// Bulk endpoints. Out for every command; the ordinary in for a command that
/// answers at once, and a second in for one that answers when a finger lands.
const EP_CMD_OUT: u8 = 0x01;
const EP_CMD_IN: u8 = 0x83;
const EP_FINGER_IN: u8 = 0x84;

/// How long a command that is not waiting for a finger may take.
const CMD_TIMEOUT_MS: u32 = 5000;

/// The interface the sensor's endpoints live on.
const INTERFACE: u32 = 0;

/// The largest response any command below asks for.
const MAX_RESPONSE: usize = 128;

/// The name stored with a finger: two bytes this daemon keeps zero, a length,
/// and up to 92 bytes of label.
///
/// The chip takes it and never gives it back — see the module note on what
/// this firmware does not implement — so it is written for the sake of a
/// system that can read it, and this daemon reads the labels from its own
/// roster instead.
const USER_DATA_LEN: usize = 95;
const MAX_NAME: usize = 92;
/// A command that carries a name: `40 ff <op>`, two bytes the chip fills in
/// itself, the name, and zeroes to the length it reads.
const NAMED_CMD_LEN: usize = 128;
/// Where the name begins in one.
const NAME_AT: usize = 5;

/// Slots the sensor holds.
pub const MAX_ENROLLED: u8 = 10;

/// Frames this firmware wants before a template is complete.
const ENROL_FRAMES: u8 = 9;

/// The sensor's own "that reading was fine" code.
const MSG_OK: u8 = 0x00;
/// What the calibration status poll answers once the sensor is ready. Not
/// [`MSG_OK`]: that poll has its own vocabulary, and libfprint's elanmoc driver
/// waits for this value on the same command.
const STATUS_CALIBRATED: u8 = 0x03;
/// A clean reading of a finger it does not know.
const MSG_NO_MATCH: u8 = 0xfd;
/// A reading it could not use: too little of the finger, or a dirty sensor.
const MSG_AREA_NOT_ENOUGH: u8 = 0xfe;
const MSG_DIRTY: u8 = 0xfb;
/// Off-centre, by direction. All four mean the same correction.
const MSG_TOO_HIGH: u8 = 0x41;
const MSG_TOO_LEFT: u8 = 0x42;
const MSG_TOO_LOW: u8 = 0x43;
const MSG_TOO_RIGHT: u8 = 0x44;

// -----------------------------------------------------------------------------
// usbfs
// -----------------------------------------------------------------------------

const IOC_NONE: u32 = 0;
const IOC_WRITE: u32 = 1;
const IOC_READ: u32 = 2;

/// `_IOC` from `<asm/ioctl.h>`, which is where every number below comes from.
/// Written out rather than hard-coded so that each constant reads as the macro
/// the kernel header spells it with.
const fn ioc(dir: u32, ty: u32, nr: u32, size: u32) -> u32 {
    (dir << 30) | (size << 16) | (ty << 8) | nr
}

const USBDEVFS_CLAIMINTERFACE: u32 = ioc(IOC_READ, b'U' as u32, 15, 4);
const USBDEVFS_RELEASEINTERFACE: u32 = ioc(IOC_READ, b'U' as u32, 16, 4);
const USBDEVFS_SUBMITURB: u32 =
    ioc(IOC_READ, b'U' as u32, 10, std::mem::size_of::<Urb>() as u32);
const USBDEVFS_DISCARDURB: u32 = ioc(IOC_NONE, b'U' as u32, 11, 0);
const USBDEVFS_REAPURBNDELAY: u32 =
    ioc(IOC_WRITE, b'U' as u32, 13, std::mem::size_of::<*mut Urb>() as u32);
const USBDEVFS_CLEAR_HALT: u32 = ioc(IOC_READ, b'U' as u32, 21, 4);

const URB_TYPE_BULK: u8 = 3;

/// `struct usbdevfs_urb`, laid out to match `<linux/usbdevice_fs.h>`.
///
/// `repr(C)` and in the header's field order, including the anonymous union —
/// which this spells as the `number_of_packets` arm, the only one a bulk
/// transfer uses. The trailing `iso_frame_desc[]` is a flexible array a bulk
/// URB never carries and is left off.
#[repr(C)]
#[derive(Debug)]
struct Urb {
    kind: u8,
    endpoint: u8,
    status: i32,
    flags: u32,
    buffer: *mut u8,
    buffer_length: i32,
    actual_length: i32,
    start_frame: i32,
    number_of_packets: i32,
    error_count: i32,
    signr: u32,
    usercontext: *mut std::ffi::c_void,
}

impl Urb {
    fn bulk(endpoint: u8, buffer: *mut u8, len: usize) -> Self {
        Self {
            kind: URB_TYPE_BULK,
            endpoint,
            status: 0,
            flags: 0,
            buffer,
            buffer_length: len as i32,
            actual_length: 0,
            start_frame: 0,
            number_of_packets: 0,
            error_count: 0,
            signr: 0,
            usercontext: std::ptr::null_mut(),
        }
    }
}

/// The type `ioctl`'s request argument has, which is not the same on both of
/// the libcs this crate is built against.
///
/// glibc takes `unsigned long`, musl takes `int`. `imlazy dev` builds natively
/// against the host's glibc and the ISO stage cross-builds against musl, so a
/// file that hard-coded either would compile in one of those and not the other.
/// The `as` cast keeps the bit pattern either way -- a request with the
/// direction bits set is a negative `int` and the same 32 bits.
#[cfg(target_env = "musl")]
type IoctlRequest = libc::c_int;
#[cfg(not(target_env = "musl"))]
type IoctlRequest = libc::c_ulong;

fn ioctl_ptr(fd: RawFd, request: u32, arg: *mut std::ffi::c_void) -> io::Result<i32> {
    // SAFETY: `fd` is a usbfs file descriptor this process owns, `request` is
    // one of the constants above, and `arg` points at a value of the type that
    // request names -- each call site passes the struct the header pairs with
    // the number.
    let rc = unsafe { libc::ioctl(fd, request as IoctlRequest, arg) };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(rc)
}

fn ioctl_u32(fd: RawFd, request: u32, mut value: u32) -> io::Result<i32> {
    ioctl_ptr(fd, request, std::ptr::addr_of_mut!(value).cast())
}

// -----------------------------------------------------------------------------
// Finding the device
// -----------------------------------------------------------------------------

/// Where a sensor is, as a path under `/dev/bus/usb` and the ids that found it.
#[derive(Debug, Clone)]
pub struct Found {
    pub node: PathBuf,
    pub product: u16,
}

/// Look for an Elan match-on-chip sensor.
///
/// Walks `/sys/bus/usb/devices` rather than `/dev/bus/usb`, because sysfs is
/// where the ids are: the device nodes are named by bus and address and say
/// nothing about what they are, so finding one by opening every node and
/// reading its descriptors would mean opening every USB device on the machine
/// as root. Reading two text files each is cheaper and touches nothing.
pub fn find() -> io::Result<Option<Found>> {
    let entries = match std::fs::read_dir("/sys/bus/usb/devices") {
        Ok(entries) => entries,
        // No USB at all is not an error here; it is a machine with no sensor.
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        let hex = |name: &str| -> Option<u16> {
            let text = std::fs::read_to_string(dir.join(name)).ok()?;
            u16::from_str_radix(text.trim(), 16).ok()
        };
        let dec = |name: &str| -> Option<u32> {
            let text = std::fs::read_to_string(dir.join(name)).ok()?;
            text.trim().parse().ok()
        };
        // An interface directory has no idVendor; only whole devices do, so
        // this skips them without having to recognise their naming.
        let (Some(vendor), Some(product)) = (hex("idVendor"), hex("idProduct")) else {
            continue;
        };
        if vendor != ELAN_VENDOR || !ELAN_PRODUCTS.contains(&product) {
            continue;
        }
        let (Some(bus), Some(dev)) = (dec("busnum"), dec("devnum")) else {
            continue;
        };
        return Ok(Some(Found {
            node: PathBuf::from(format!("/dev/bus/usb/{bus:03}/{dev:03}")),
            product,
        }));
    }
    Ok(None)
}

// -----------------------------------------------------------------------------
// What a reading came to
// -----------------------------------------------------------------------------

/// The outcome of one presentation of a finger.
///
/// The distinction that carries all the weight is between a reading that was no
/// good and a reading that was good and was not you. Everything in [`Self::Retry`]
/// is the first — ask again, count nothing — and only [`Self::NoMatch`] is the
/// second. Conflating them spends an unlock's whole budget on a wet thumb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scan {
    /// Matched, and the sensor's index for the finger it matched.
    Match(u8),
    /// A clean reading of a finger the sensor does not hold.
    NoMatch,
    /// A frame the sensor accepted towards a template.
    Frame,
    /// Unusable. Say which correction and ask again.
    Retry(Retry),
}

/// Why a reading was no good.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retry {
    /// Off centre — the sensor says which way, and the correction is the same
    /// whichever it is.
    Centre,
    /// Not enough finger on the sensor.
    Area,
    /// Wet, dry or dirty; the sensor cannot tell which and neither can this.
    Dirty,
    /// Something else the sensor did not explain.
    Unknown(u8),
}

impl Retry {
    fn of(code: u8) -> Self {
        match code {
            MSG_TOO_HIGH | MSG_TOO_LOW | MSG_TOO_LEFT | MSG_TOO_RIGHT => Self::Centre,
            MSG_AREA_NOT_ENOUGH => Self::Area,
            MSG_DIRTY => Self::Dirty,
            other => Self::Unknown(other),
        }
    }

    /// What to tell the person, in the imperative and without blaming them.
    pub fn advice(self) -> &'static str {
        match self {
            Self::Centre => "move your finger to the middle of the sensor",
            Self::Area => "cover more of the sensor",
            Self::Dirty => "wipe the sensor and try again",
            Self::Unknown(_) => "try again",
        }
    }
}

/// The name to store with a finger, in the shape the chip takes it.
///
/// Two bytes kept zero, the length, then the label. Nothing in it is a
/// fingerprint — the template is on the chip and is never any of this daemon's
/// business — so what goes here is the account and the finger, for whatever
/// can read it. Truncated rather than refused: a long username is not a reason
/// to refuse to enrol a finger.
fn user_data(label: &[u8]) -> [u8; USER_DATA_LEN] {
    let mut out = [0u8; USER_DATA_LEN];
    let len = label.len().min(MAX_NAME);
    out[2] = len as u8;
    out[3..3 + len].copy_from_slice(&label[..len]);
    out
}

// -----------------------------------------------------------------------------
// The sensor
// -----------------------------------------------------------------------------

/// An open, claimed Elan sensor.
#[derive(Debug)]
pub struct Sensor {
    fd: OwnedFd,
    /// Frames the chip wants before a template is complete. The same number
    /// the enrolment command carries, kept here because the progress a caller
    /// shows has to be the progress the sensor is working to.
    stages: u8,
    /// Fingers currently stored, as of the last time anything asked.
    enrolled: u8,
    firmware: u16,
}

impl Sensor {
    /// Open the sensor, claim its interface and bring it up.
    pub fn open() -> io::Result<Option<Self>> {
        let Some(found) = find()? else {
            return Ok(None);
        };
        Self::open_at(&found).map(Some)
    }

    fn open_at(found: &Found) -> io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&found.node)?;
        let fd = OwnedFd::from(file);
        ioctl_u32(fd.as_raw_fd(), USBDEVFS_CLAIMINTERFACE, INTERFACE)?;
        log::debug!(
            "claimed {:04x}:{:04x} at {}",
            ELAN_VENDOR,
            found.product,
            found.node.display()
        );

        let mut sensor = Self {
            fd,
            stages: ENROL_FRAMES,
            enrolled: 0,
            firmware: 0,
        };
        sensor.bring_up()?;
        Ok(sensor)
    }

    /// The sequence the sensor expects before it will do anything: wait for
    /// calibration, then read back what it is and what it holds.
    fn bring_up(&mut self) -> io::Result<()> {
        self.wait_calibrated()?;
        let version = self.command(&[0x40, 0x19], 2)?;
        self.firmware = u16::from(version[0]) << 8 | u16::from(version[1]);
        self.enrolled = self.read_enrolled_count()?;
        Ok(())
    }

    /// Poll the calibration status until the sensor says it is ready.
    ///
    /// Bounded. A sensor that never finishes calibrating is a broken sensor,
    /// and a daemon that waits for it forever is a daemon that never starts.
    fn wait_calibrated(&mut self) -> io::Result<()> {
        let mut last = None;
        for _ in 0..500 {
            let status = self.command(&[0x40, 0xff, 0x00], 2)?;
            // Logged on change rather than per poll: the byte is what says
            // whether a sensor is slow or speaking a different protocol, and
            // five hundred copies of it say nothing more.
            if last != Some(status[1]) {
                log::debug!("calibration status {:02x}", status[1]);
                last = Some(status[1]);
            }
            if status[1] == STATUS_CALIBRATED {
                log::info!("the sensor is calibrated");
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!(
                "the fingerprint sensor never finished calibrating (last status {:02x}, waiting for {STATUS_CALIBRATED:02x})",
                last.unwrap_or_default()
            ),
        ))
    }

    /// The mode change is not reliably acknowledged: the `0c00` accepts it and
    /// answers nothing, so an answer is read if one comes and not required.
    /// Whether the sensor took it shows in the next command, which does answer.
    fn read_enrolled_count(&mut self) -> io::Result<u8> {
        let response = self.command(&[0x40, 0xff, 0x04], 2)?;
        Ok(response[1].min(MAX_ENROLLED))
    }

    /// Frames a template wants.
    pub fn stages(&self) -> u8 {
        self.stages
    }

    /// How many fingers are stored.
    pub fn enrolled(&self) -> u8 {
        self.enrolled
    }

    /// The firmware version, for a log line.
    pub fn firmware(&self) -> u16 {
        self.firmware
    }

    /// How many fingers the sensor holds, asked afresh.
    pub fn count(&mut self) -> io::Result<u8> {
        self.enrolled = self.read_enrolled_count()?;
        Ok(self.enrolled)
    }

    /// Wait for a finger and ask whether it is one the sensor knows.
    ///
    /// Blocks until somebody touches the sensor, `cancel` becomes readable, or
    /// the device goes away. See the module note on cancellation.
    pub fn verify(&mut self, cancel: BorrowedFd<'_>) -> io::Result<Option<Scan>> {
        self.send(&[0x40, 0xff, 0x03])?;
        let Some(response) = self.wait(EP_FINGER_IN, 2, cancel)? else {
            return Ok(None);
        };
        Ok(Some(match response[1] {
            MSG_NO_MATCH => Scan::NoMatch,
            // A match answers with the slot the finger is in, and every slot
            // number is below the count the sensor holds. The codes that mean
            // something went wrong all have the top nibble set.
            slot if slot < MAX_ENROLLED => Scan::Match(slot),
            other => Scan::Retry(Retry::of(other)),
        }))
    }

    /// Take one frame towards a template.
    ///
    /// `frame` is how many good ones the chip already has: it accumulates, and
    /// this tells it where the caller believes it is so the two cannot drift.
    pub fn enrol_frame(&mut self, frame: u8, cancel: BorrowedFd<'_>) -> io::Result<Option<Scan>> {
        // The sensor is told the slot the template is being built for, how
        // many frames the whole enrolment takes, and which one this is.
        self.send(&[0x40, 0xff, 0x01, self.enrolled, ENROL_FRAMES, frame, 0x00])?;
        let Some(response) = self.wait(EP_FINGER_IN, 2, cancel)? else {
            return Ok(None);
        };
        if response[0] != 0x40 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the sensor answered an enrolment frame with something else",
            ));
        }
        Ok(Some(match response[1] {
            MSG_OK => Scan::Frame,
            other => Scan::Retry(Retry::of(other)),
        }))
    }

    /// Tell the sensor the template is complete and store it under `label`.
    ///
    /// Answers with the slot it went into, which is the count of what was
    /// already stored: the chip fills its slots in order, and that number is
    /// the only handle anything has on the finger afterwards. A caller that
    /// wants to know whose finger it was later has to write it down — see the
    /// module note.
    pub fn enrol_commit(&mut self, label: &[u8]) -> io::Result<u8> {
        let slot = self.enrolled;
        let mut cmd = [0u8; NAMED_CMD_LEN];
        cmd[..3].copy_from_slice(&[0x40, 0xff, 0x11]);
        cmd[NAME_AT..NAME_AT + USER_DATA_LEN].copy_from_slice(&user_data(label));
        let response = self.command(&cmd, 2)?;
        if response[1] != MSG_OK {
            return Err(io::Error::other(format!(
                "the sensor refused to store the finger (0x{:02x})",
                response[1]
            )));
        }
        self.enrolled = self.read_enrolled_count()?;
        Ok(slot)
    }

    /// Abandon a template the sensor is part way through building.
    ///
    /// This takes it out of the waiting-for-a-finger state, which is where a
    /// fresh enrolment expects to find it. A part-built template left behind
    /// is a slot gone from a sensor that has ten.
    pub fn enrol_abandon(&mut self) -> io::Result<()> {
        self.command(&[0x40, 0xff, 0x02], 2)?;
        Ok(())
    }

    /// Forget every stored finger.
    ///
    /// All of them or none: this firmware has no per-finger delete. Both
    /// commands that would remove one -- `40 ff 13` in either shape -- are
    /// refused, and `40 ff 05` takes the whole store whatever slot it is
    /// given. Worse, a refused delete leaves the chip counting fingers that no
    /// longer match anything, so the only way back to a sensor that behaves is
    /// to clear it and enrol again.
    pub fn forget_all(&mut self) -> io::Result<()> {
        let response = self.command(&[0x40, 0xff, 0x05, 0x00, 0x00], 2)?;
        if response[1] != MSG_OK {
            return Err(io::Error::other(format!(
                "the sensor refused to clear itself (0x{:02x})",
                response[1]
            )));
        }
        self.enrolled = self.read_enrolled_count()?;
        if self.enrolled != 0 {
            return Err(io::Error::other(format!(
                "the sensor still holds {} fingers after being cleared",
                self.enrolled
            )));
        }
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Transport
    // -------------------------------------------------------------------------

    /// Send a command and read its answer, both within [`CMD_TIMEOUT_MS`].
    ///
    /// For everything that does not wait for a finger. The ones that do go
    /// through [`Self::wait`] instead, which can be cancelled.
    fn command(&mut self, cmd: &[u8], response_len: usize) -> io::Result<Vec<u8>> {
        self.send(cmd).map_err(|e| named(e, "sending", cmd))?;
        if response_len == 0 {
            return Ok(Vec::new());
        }
        self.read_bulk(EP_CMD_IN, response_len, CMD_TIMEOUT_MS)
            .map_err(|e| named(e, "reading the answer to", cmd))
    }

    fn send(&mut self, cmd: &[u8]) -> io::Result<()> {
        let mut buffer = cmd.to_vec();
        let written = self.transfer(EP_CMD_OUT, &mut buffer, CMD_TIMEOUT_MS)?;
        if written != cmd.len() {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "the sensor took only part of a command",
            ));
        }
        Ok(())
    }

    fn read_bulk(&mut self, endpoint: u8, len: usize, timeout: u32) -> io::Result<Vec<u8>> {
        if len > MAX_RESPONSE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "response larger than any command asks for",
            ));
        }
        let mut buffer = vec![0u8; len];
        let read = self.transfer(endpoint, &mut buffer, timeout)?;
        if read < 2 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the sensor answered with less than a status",
            ));
        }
        buffer.truncate(read);
        Ok(buffer)
    }

    /// One bulk transfer with a timeout, through submit-and-reap rather than
    /// the blocking `USBDEVFS_BULK`, so that every transfer in this file goes
    /// the same way and only the waiting differs.
    fn transfer(&mut self, endpoint: u8, buffer: &mut [u8], timeout: u32) -> io::Result<usize> {
        let mut urb = Urb::bulk(endpoint, buffer.as_mut_ptr(), buffer.len());
        self.submit(&mut urb)?;
        match self.reap(timeout_poll(timeout), None) {
            Ok(Some(actual)) => Ok(actual),
            Ok(None) => {
                self.discard(&mut urb);
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "the fingerprint sensor did not answer",
                ))
            }
            Err(e) => {
                self.discard(&mut urb);
                Err(e)
            }
        }
    }

    /// Wait for a finger: a transfer with no deadline that a write to `cancel`
    /// ends.
    ///
    /// `Ok(None)` means cancelled. Nothing was read and nothing is pending —
    /// the URB is discarded and reaped before returning, so the next command
    /// does not find somebody else's completion waiting for it.
    fn wait(
        &mut self,
        endpoint: u8,
        len: usize,
        cancel: BorrowedFd<'_>,
    ) -> io::Result<Option<Vec<u8>>> {
        let mut buffer = vec![0u8; len];
        let mut urb = Urb::bulk(endpoint, buffer.as_mut_ptr(), buffer.len());
        self.submit(&mut urb)?;
        match self.reap(-1, Some(cancel.as_raw_fd())) {
            Ok(Some(actual)) => {
                if actual < 2 {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "the sensor answered a finger with less than a status",
                    ));
                }
                buffer.truncate(actual);
                Ok(Some(buffer))
            }
            Ok(None) => {
                self.discard(&mut urb);
                Ok(None)
            }
            Err(e) => {
                self.discard(&mut urb);
                Err(e)
            }
        }
    }

    fn submit(&mut self, urb: &mut Urb) -> io::Result<()> {
        ioctl_ptr(
            self.fd.as_raw_fd(),
            USBDEVFS_SUBMITURB,
            (urb as *mut Urb).cast(),
        )?;
        Ok(())
    }

    /// Wait for the submitted URB to complete.
    ///
    /// `timeout` is milliseconds, or -1 for no deadline. `cancel`, when given,
    /// is a descriptor whose readability means stop. Returns the number of
    /// bytes transferred, or `None` for a timeout or a cancel.
    fn reap(&mut self, timeout: i32, cancel: Option<RawFd>) -> io::Result<Option<usize>> {
        let mut fds = [
            libc::pollfd {
                fd: self.fd.as_raw_fd(),
                // usbfs signals a completed URB as writability, not
                // readability. Polling for POLLIN here waits forever on a
                // transfer that finished before the poll began.
                events: libc::POLLOUT,
                revents: 0,
            },
            libc::pollfd {
                fd: cancel.unwrap_or(-1),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let count = if cancel.is_some() { 2 } else { 1 };
        loop {
            // SAFETY: `fds` is a valid array of `count` pollfds that outlives
            // the call.
            let rc = unsafe { libc::poll(fds.as_mut_ptr(), count, timeout) };
            if rc < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(err);
            }
            if rc == 0 {
                return Ok(None);
            }
            if cancel.is_some() && fds[1].revents != 0 {
                return Ok(None);
            }
            if fds[0].revents == 0 {
                continue;
            }
            let mut done: *mut Urb = std::ptr::null_mut();
            match ioctl_ptr(
                self.fd.as_raw_fd(),
                USBDEVFS_REAPURBNDELAY,
                std::ptr::addr_of_mut!(done).cast(),
            ) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
            if done.is_null() {
                continue;
            }
            // SAFETY: the pointer came back from REAPURB and names the URB
            // submitted above, which is still alive in the caller's frame.
            let (status, actual) = unsafe { ((*done).status, (*done).actual_length) };
            if status != 0 {
                // A stalled endpoint is recoverable and worth recovering: the
                // alternative is a sensor that stays wedged until the machine
                // is rebooted.
                if status == -libc::EPIPE {
                    let _ = ioctl_u32(
                        self.fd.as_raw_fd(),
                        USBDEVFS_CLEAR_HALT,
                        u32::from(EP_CMD_IN),
                    );
                }
                return Err(io::Error::from_raw_os_error(-status));
            }
            return Ok(Some(actual.max(0) as usize));
        }
    }

    /// Ask the kernel to unlink a URB and then collect it.
    ///
    /// Best effort by design: every caller is already on its way out with an
    /// error or a cancellation, and there is nothing useful to do about a
    /// discard that fails. What matters is that the completion is drained, so
    /// that the next transfer does not reap this one's.
    fn discard(&mut self, urb: &mut Urb) {
        let _ = ioctl_ptr(
            self.fd.as_raw_fd(),
            USBDEVFS_DISCARDURB,
            (urb as *mut Urb).cast(),
        );
        let _ = self.reap(200, None);
    }
}

impl Drop for Sensor {
    fn drop(&mut self) {
        let _ = ioctl_u32(self.fd.as_raw_fd(), USBDEVFS_RELEASEINTERFACE, INTERFACE);
    }
}


/// An error that says which command it came from and which half of it failed,
/// because "did not answer" is only useful once it says which step of the
/// bring-up went unanswered.
fn named(e: io::Error, doing: &str, cmd: &[u8]) -> io::Error {
    let hex: Vec<String> = cmd.iter().map(|b| format!("{b:02x}")).collect();
    io::Error::new(e.kind(), format!("{e} ({doing} command {})", hex.join(" ")))
}

/// A command timeout as a `poll` timeout.
fn timeout_poll(ms: u32) -> i32 {
    i32::try_from(ms).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ioctl numbers are arithmetic on macros from a kernel header, and
    /// getting one wrong is a driver that fails in a way that looks like
    /// hardware. These are the values `<linux/usbdevice_fs.h>` produces on
    /// 64-bit Linux.
    #[test]
    fn the_ioctl_numbers_are_the_kernels() {
        assert_eq!(USBDEVFS_CLAIMINTERFACE, 0x8004_550f);
        assert_eq!(USBDEVFS_RELEASEINTERFACE, 0x8004_5510);
        assert_eq!(USBDEVFS_DISCARDURB, 0x0000_550b);
        assert_eq!(USBDEVFS_CLEAR_HALT, 0x8004_5515);
        // _IOR('U', 10, struct usbdevfs_urb): 56 bytes on 64-bit, so the
        // number carries 56 in its size field. This is the one that would go
        // wrong silently if `Urb` below drifted from the header.
        assert_eq!(std::mem::size_of::<Urb>(), 56);
        assert_eq!(USBDEVFS_SUBMITURB, 0x8038_550a);
        // _IOW('U', 13, void *)
        assert_eq!(USBDEVFS_REAPURBNDELAY, 0x4008_550d);
    }

    /// The kernel reads and writes these fields by offset. A `repr(C)` struct
    /// whose fields are in the header's order gets them right, and these are
    /// the offsets `offsetof` gives on 64-bit Linux -- so a field added,
    /// removed or reordered fails here rather than in a driver that reports
    /// the wrong number of bytes transferred.
    #[test]
    fn the_urb_is_laid_out_like_the_kernels() {
        let urb = Urb::bulk(0x83, std::ptr::null_mut(), 0);
        let base = std::ptr::addr_of!(urb) as usize;
        let offset = |field: *const u8| field as usize - base;
        assert_eq!(offset(std::ptr::addr_of!(urb.status).cast()), 4);
        assert_eq!(offset(std::ptr::addr_of!(urb.flags).cast()), 8);
        assert_eq!(offset(std::ptr::addr_of!(urb.buffer).cast()), 16);
        assert_eq!(offset(std::ptr::addr_of!(urb.buffer_length).cast()), 24);
        assert_eq!(offset(std::ptr::addr_of!(urb.actual_length).cast()), 28);
        assert_eq!(offset(std::ptr::addr_of!(urb.usercontext).cast()), 48);
    }
    /// The name goes to the chip as a length and then the label, at the
    /// offset the store command reads it from. Nothing reads it back -- this
    /// firmware has no command that returns it -- so the shape is checked here
    /// or nowhere.
    #[test]
    fn a_name_is_stored_as_a_length_and_the_label() {
        let data = user_data(b"javanstorm:right-index");
        assert_eq!(data[0..2], [0, 0]);
        assert_eq!(data[2], 22);
        assert_eq!(&data[3..25], b"javanstorm:right-index");
        assert!(data[25..].iter().all(|b| *b == 0));
    }

    /// A long account name must cost a truncated label and not a refusal to
    /// enrol.
    #[test]
    fn an_over_long_name_is_truncated_rather_than_refused() {
        let data = user_data(&vec![b'a'; 200]);
        assert_eq!(data.len(), USER_DATA_LEN);
        assert_eq!(data[2], MAX_NAME as u8);
    }

    /// The store command is the length the chip reads, with the name where it
    /// looks for it: a byte out either way and the finger is stored under a
    /// name nothing will recognise.
    #[test]
    fn the_store_command_puts_the_name_where_the_chip_reads_it() {
        let mut cmd = [0u8; NAMED_CMD_LEN];
        cmd[..3].copy_from_slice(&[0x40, 0xff, 0x11]);
        cmd[NAME_AT..NAME_AT + USER_DATA_LEN].copy_from_slice(&user_data(b"a:b"));
        assert_eq!(cmd.len(), 128);
        assert_eq!(cmd[3..5], [0, 0]);
        assert_eq!(cmd[NAME_AT + 2], 3);
        assert_eq!(&cmd[NAME_AT + 3..NAME_AT + 6], b"a:b");
    }

    /// The four off-centre codes are one correction, and a code nobody
    /// documented is still a retry rather than a failed match.
    #[test]
    fn the_retry_codes_map_to_advice() {
        for code in [MSG_TOO_HIGH, MSG_TOO_LOW, MSG_TOO_LEFT, MSG_TOO_RIGHT] {
            assert_eq!(Retry::of(code), Retry::Centre);
        }
        assert_eq!(Retry::of(MSG_AREA_NOT_ENOUGH), Retry::Area);
        assert_eq!(Retry::of(MSG_DIRTY), Retry::Dirty);
        assert_eq!(Retry::of(0x77), Retry::Unknown(0x77));
    }

    /// The count the enrolment asks the person for and the count it tells the
    /// sensor are the same number, or the chip is still waiting for frames
    /// when the caller believes the template is done.
    #[test]
    fn a_template_takes_the_frames_the_sensor_is_told_it_does() {
        // Nine is what this firmware was seen to take, frame by frame, before
        // it would accept a commit.
        assert_eq!(ENROL_FRAMES, 9);
    }

    /// Finding a sensor must never fail on a machine that has none — a desktop
    /// with no USB fingerprint reader is the common case, not an error.
    #[test]
    fn looking_for_a_sensor_is_safe_on_a_machine_without_one() {
        assert!(find().is_ok());
    }
}
