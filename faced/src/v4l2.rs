//! The V4L2 ioctls a webcam needs, and nothing else.
//!
//! No binding crate between this and the kernel, for the reason RavenCamera's
//! sibling of this file gives: what a camera needs is a dozen `repr(C)` structs
//! out of `<linux/videodev2.h>` that have not changed in a decade, and a
//! binding crate would be a second thing to audit inside the process that
//! decides who may log in. The structs are the kernel's ABI -- their sizes are
//! part of the ioctl request numbers, so a field in the wrong place does not
//! misbehave subtly, it fails every call with `ENOTTY`. There are tests for
//! exactly that below.
//!
//! What is *not* here, and is in RavenCamera's: format enumeration beyond what
//! is needed to choose one, frame-rate enumeration, menu controls. This daemon
//! captures one size at one rate and sets three controls.

use std::ffi::CStr;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

pub const BUF_TYPE_VIDEO_CAPTURE: u32 = 1;
pub const MEMORY_MMAP: u32 = 1;

pub const CAP_VIDEO_CAPTURE: u32 = 0x0000_0001;
pub const CAP_STREAMING: u32 = 0x0400_0000;
pub const CAP_DEVICE_CAPS: u32 = 0x8000_0000;
/// A node that only carries metadata, which several laptops expose beside the
/// real camera. Capturing from one gets you no pixels and no error.
pub const CAP_META_CAPTURE: u32 = 0x0080_0000;

pub const fn fourcc(code: &[u8; 4]) -> u32 {
    (code[0] as u32) | ((code[1] as u32) << 8) | ((code[2] as u32) << 16) | ((code[3] as u32) << 24)
}

pub const PIX_MJPEG: u32 = fourcc(b"MJPG");
pub const PIX_YUYV: u32 = fourcc(b"YUYV");
/// Greyscale, 8 bits. What an infrared camera almost always delivers.
pub const PIX_GREY: u32 = fourcc(b"GREY");

/// The controls this daemon touches, and no others.
///
/// All three are pinned for the duration of a liveness challenge. See
/// [`crate::camera::Camera::pin_exposure`] for why: a camera left on automatic
/// corrects for the light this machine is deliberately throwing at somebody's
/// face, which erases the one signal the challenge is measuring.
pub const CID_AUTO_WHITE_BALANCE: u32 = 0x0098_090c;
pub const CID_EXPOSURE_AUTO: u32 = 0x009a_0901;
pub const CID_EXPOSURE_ABSOLUTE: u32 = 0x009a_0902;
/// `V4L2_EXPOSURE_MANUAL`.
pub const EXPOSURE_MANUAL: i32 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Capability {
    pub driver: [u8; 16],
    pub card: [u8; 32],
    pub bus_info: [u8; 32],
    pub version: u32,
    pub capabilities: u32,
    pub device_caps: u32,
    pub reserved: [u32; 3],
}

impl Capability {
    /// What this node can do, preferring the per-device capabilities over the
    /// whole driver's: a driver that exposes two nodes reports the union in
    /// `capabilities`, so a metadata node looks like a capture node there.
    pub fn caps(&self) -> u32 {
        if self.capabilities & CAP_DEVICE_CAPS != 0 {
            self.device_caps
        } else {
            self.capabilities
        }
    }

    pub fn can_capture(&self) -> bool {
        let caps = self.caps();
        caps & CAP_VIDEO_CAPTURE != 0
            && caps & CAP_STREAMING != 0
            // Stated rather than implied. A node that is both -- which no
            // driver should report and some do -- is the metadata half, and
            // capturing from it gets you no pixels and no error either.
            && caps & CAP_META_CAPTURE == 0
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FmtDesc {
    pub index: u32,
    pub kind: u32,
    pub flags: u32,
    pub description: [u8; 32],
    pub pixelformat: u32,
    pub mbus_code: u32,
    pub reserved: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PixFormat {
    pub width: u32,
    pub height: u32,
    pub pixelformat: u32,
    pub field: u32,
    pub bytesperline: u32,
    pub sizeimage: u32,
    pub colorspace: u32,
    pub private: u32,
    pub flags: u32,
    pub ycbcr_enc: u32,
    pub quantization: u32,
    pub xfer_func: u32,
}

/// `struct v4l2_format`. The union is 200 bytes and, because some of its
/// members hold pointers, 8-aligned -- hence the padding after `kind`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Format {
    pub kind: u32,
    _pad: u32,
    pub pix: PixFormat,
    _rest: [u8; 200 - std::mem::size_of::<PixFormat>()],
}

impl Format {
    pub fn capture(pix: PixFormat) -> Self {
        Self {
            kind: BUF_TYPE_VIDEO_CAPTURE,
            _pad: 0,
            pix,
            _rest: [0; 200 - std::mem::size_of::<PixFormat>()],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RequestBuffers {
    pub count: u32,
    pub kind: u32,
    pub memory: u32,
    pub capabilities: u32,
    pub flags: u8,
    pub reserved: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Timecode {
    pub kind: u32,
    pub flags: u32,
    pub frames: u8,
    pub seconds: u8,
    pub minutes: u8,
    pub hours: u8,
    pub userbits: [u8; 4],
}

/// `struct v4l2_buffer`, 64-bit layout. `m` is a union of a u32 offset, an
/// unsigned long, a pointer and an int; for mmap buffers only the offset in
/// its low four bytes matters.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Buffer {
    pub index: u32,
    pub kind: u32,
    pub bytesused: u32,
    pub flags: u32,
    pub field: u32,
    pub timestamp: libc::timeval,
    pub timecode: Timecode,
    pub sequence: u32,
    pub memory: u32,
    pub m: u64,
    pub length: u32,
    pub reserved2: u32,
    pub request_fd: i32,
}

impl Buffer {
    pub fn mmap(index: u32) -> Self {
        Self {
            index,
            kind: BUF_TYPE_VIDEO_CAPTURE,
            bytesused: 0,
            flags: 0,
            field: 0,
            timestamp: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            timecode: Timecode::default(),
            sequence: 0,
            memory: MEMORY_MMAP,
            m: 0,
            length: 0,
            reserved2: 0,
            request_fd: 0,
        }
    }

    /// The buffer's mmap offset.
    pub fn offset(&self) -> u32 {
        self.m as u32
    }

    /// When the kernel says this frame was captured.
    ///
    /// The camera's own clock, and the reason the liveness check can say
    /// anything at all about *when* a colour arrived: a timestamp taken after
    /// the frame was dequeued would carry this daemon's scheduling delay, and
    /// a delay is exactly what it is trying to measure.
    pub fn captured_at(&self) -> std::time::Duration {
        std::time::Duration::new(
            self.timestamp.tv_sec.max(0) as u64,
            (self.timestamp.tv_usec.max(0) as u32).saturating_mul(1000),
        )
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct QueryCtrl {
    pub id: u32,
    pub kind: u32,
    pub name: [u8; 32],
    pub minimum: i32,
    pub maximum: i32,
    pub step: i32,
    pub default_value: i32,
    pub flags: u32,
    pub reserved: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Control {
    pub id: u32,
    pub value: i32,
}

const fn ioc(dir: u32, nr: u32, size: usize) -> libc::c_ulong {
    ((dir << 30) | ((size as u32) << 16) | ((b'V' as u32) << 8) | nr) as libc::c_ulong
}
const fn ior<T>(nr: u32) -> libc::c_ulong {
    ioc(2, nr, std::mem::size_of::<T>())
}
const fn iow<T>(nr: u32) -> libc::c_ulong {
    ioc(1, nr, std::mem::size_of::<T>())
}
const fn iowr<T>(nr: u32) -> libc::c_ulong {
    ioc(3, nr, std::mem::size_of::<T>())
}

pub const VIDIOC_QUERYCAP: libc::c_ulong = ior::<Capability>(0);
pub const VIDIOC_ENUM_FMT: libc::c_ulong = iowr::<FmtDesc>(2);
pub const VIDIOC_S_FMT: libc::c_ulong = iowr::<Format>(5);
pub const VIDIOC_REQBUFS: libc::c_ulong = iowr::<RequestBuffers>(8);
pub const VIDIOC_QUERYBUF: libc::c_ulong = iowr::<Buffer>(9);
pub const VIDIOC_QBUF: libc::c_ulong = iowr::<Buffer>(15);
pub const VIDIOC_DQBUF: libc::c_ulong = iowr::<Buffer>(17);
pub const VIDIOC_STREAMON: libc::c_ulong = iow::<libc::c_int>(18);
pub const VIDIOC_STREAMOFF: libc::c_ulong = iow::<libc::c_int>(19);
pub const VIDIOC_G_CTRL: libc::c_ulong = iowr::<Control>(27);
pub const VIDIOC_S_CTRL: libc::c_ulong = iowr::<Control>(28);
pub const VIDIOC_QUERYCTRL: libc::c_ulong = iowr::<QueryCtrl>(36);

/// A zeroed `T`. Every struct in this module is plain old data for which all
/// zero bytes are a valid value, which is how the kernel's own documentation
/// says to prepare them.
pub fn zeroed<T: Copy>() -> T {
    // SAFETY: only instantiated with the repr(C) integer-and-array structs
    // above (and libc integers), for which the all-zero bit pattern is valid.
    unsafe { std::mem::zeroed() }
}

/// `ioctl(fd, request, arg)`, retried on `EINTR`.
///
/// # Safety
/// `request` must be one of the constants above, and `T` the struct that
/// request's size field was computed from.
unsafe fn ioctl<T>(fd: RawFd, request: libc::c_ulong, arg: &mut T) -> io::Result<()> {
    loop {
        // SAFETY: the caller guarantees `arg` is the struct the kernel will
        // read and write for `request`, so its size matches what the kernel
        // copies, and it is a live exclusive reference for the call.
        let r = unsafe { libc::ioctl(fd, request as _, arg as *mut T) };
        if r != -1 {
            return Ok(());
        }
        let e = io::Error::last_os_error();
        if e.kind() != io::ErrorKind::Interrupted {
            return Err(e);
        }
    }
}

/// An open video node.
#[derive(Debug)]
pub struct Node {
    file: File,
}

impl Node {
    /// Open `path` non-blocking: capture waits on `poll`, so a client hanging
    /// up never sits behind a camera that has stopped sending frames.
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(path)?;
        Ok(Self { file })
    }

    pub fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    pub fn query_cap(&self) -> io::Result<Capability> {
        let mut cap: Capability = zeroed();
        // SAFETY: VIDIOC_QUERYCAP takes a v4l2_capability.
        unsafe { ioctl(self.fd(), VIDIOC_QUERYCAP, &mut cap)? };
        Ok(cap)
    }

    /// Every pixel format this node offers, in the order it offers them.
    pub fn formats(&self) -> Vec<u32> {
        let mut found = Vec::new();
        for index in 0..32 {
            let mut desc: FmtDesc = zeroed();
            desc.index = index;
            desc.kind = BUF_TYPE_VIDEO_CAPTURE;
            // SAFETY: VIDIOC_ENUM_FMT takes a v4l2_fmtdesc.
            if unsafe { ioctl(self.fd(), VIDIOC_ENUM_FMT, &mut desc) }.is_err() {
                break;
            }
            found.push(desc.pixelformat);
        }
        found
    }

    /// Ask for a size and a format; the driver answers with what it will
    /// actually give, which is what this returns.
    pub fn set_format(&self, width: u32, height: u32, pixelformat: u32) -> io::Result<PixFormat> {
        let mut fmt = Format::capture(PixFormat {
            width,
            height,
            pixelformat,
            field: 1, // V4L2_FIELD_NONE
            ..PixFormat::default()
        });
        // SAFETY: VIDIOC_S_FMT takes a v4l2_format.
        unsafe { ioctl(self.fd(), VIDIOC_S_FMT, &mut fmt)? };
        Ok(fmt.pix)
    }

    /// Whether a control exists and can be set right now.
    pub fn has_control(&self, id: u32) -> bool {
        has_control(self.fd(), id)
    }

    pub fn control(&self, id: u32) -> io::Result<i32> {
        control(self.fd(), id)
    }

    pub fn set_control(&self, id: u32, value: i32) -> io::Result<()> {
        set_control(self.fd(), id, value)
    }

    /// A second descriptor for the same node.
    ///
    /// For the exposure guard, which has to be able to put the controls back
    /// without holding a borrow on the camera for the whole of a liveness
    /// challenge -- the challenge is busy capturing frames through it.
    pub fn dup(&self) -> io::Result<std::os::fd::OwnedFd> {
        self.file.try_clone().map(Into::into)
    }

    pub fn request_buffers(&self, count: u32) -> io::Result<u32> {
        let mut req = RequestBuffers {
            count,
            kind: BUF_TYPE_VIDEO_CAPTURE,
            memory: MEMORY_MMAP,
            ..RequestBuffers::default()
        };
        // SAFETY: VIDIOC_REQBUFS takes a v4l2_requestbuffers.
        unsafe { ioctl(self.fd(), VIDIOC_REQBUFS, &mut req)? };
        Ok(req.count)
    }

    pub fn query_buffer(&self, index: u32) -> io::Result<Buffer> {
        let mut buf = Buffer::mmap(index);
        // SAFETY: VIDIOC_QUERYBUF takes a v4l2_buffer.
        unsafe { ioctl(self.fd(), VIDIOC_QUERYBUF, &mut buf)? };
        Ok(buf)
    }

    pub fn queue(&self, index: u32) -> io::Result<()> {
        let mut buf = Buffer::mmap(index);
        // SAFETY: VIDIOC_QBUF takes a v4l2_buffer.
        unsafe { ioctl(self.fd(), VIDIOC_QBUF, &mut buf) }
    }

    /// Take the next filled buffer, or `WouldBlock` if there is not one yet.
    pub fn dequeue(&self) -> io::Result<Buffer> {
        let mut buf = Buffer::mmap(0);
        // SAFETY: VIDIOC_DQBUF takes a v4l2_buffer.
        unsafe { ioctl(self.fd(), VIDIOC_DQBUF, &mut buf)? };
        Ok(buf)
    }

    pub fn stream_on(&self) -> io::Result<()> {
        let mut kind: libc::c_int = BUF_TYPE_VIDEO_CAPTURE as libc::c_int;
        // SAFETY: VIDIOC_STREAMON takes an int.
        unsafe { ioctl(self.fd(), VIDIOC_STREAMON, &mut kind) }
    }

    pub fn stream_off(&self) -> io::Result<()> {
        let mut kind: libc::c_int = BUF_TYPE_VIDEO_CAPTURE as libc::c_int;
        // SAFETY: VIDIOC_STREAMOFF takes an int.
        unsafe { ioctl(self.fd(), VIDIOC_STREAMOFF, &mut kind) }
    }

    /// Wait up to `timeout` for a frame. `false` if none arrived.
    pub fn wait_readable(&self, timeout: std::time::Duration) -> io::Result<bool> {
        let mut fds = libc::pollfd {
            fd: self.fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let ms = timeout.as_millis().min(i32::MAX as u128) as libc::c_int;
        // SAFETY: one valid pollfd, count 1.
        let r = unsafe { libc::poll(&mut fds, 1, ms) };
        match r {
            -1 => {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::Interrupted {
                    Ok(false)
                } else {
                    Err(e)
                }
            }
            0 => Ok(false),
            _ => Ok(fds.revents & libc::POLLIN != 0),
        }
    }
}

/// Whether a control exists and can be set right now.
pub fn has_control(fd: RawFd, id: u32) -> bool {
    let mut q: QueryCtrl = zeroed();
    q.id = id;
    // SAFETY: VIDIOC_QUERYCTRL takes a v4l2_queryctrl.
    if unsafe { ioctl(fd, VIDIOC_QUERYCTRL, &mut q) }.is_err() {
        return false;
    }
    // V4L2_CTRL_FLAG_DISABLED
    q.flags & 0x0001 == 0
}

pub fn control(fd: RawFd, id: u32) -> io::Result<i32> {
    let mut c = Control { id, value: 0 };
    // SAFETY: VIDIOC_G_CTRL takes a v4l2_control.
    unsafe { ioctl(fd, VIDIOC_G_CTRL, &mut c)? };
    Ok(c.value)
}

pub fn set_control(fd: RawFd, id: u32, value: i32) -> io::Result<()> {
    let mut c = Control { id, value };
    // SAFETY: VIDIOC_S_CTRL takes a v4l2_control.
    unsafe { ioctl(fd, VIDIOC_S_CTRL, &mut c) }
}

/// One mmap'd capture buffer, unmapped when it is dropped.
#[derive(Debug)]
pub struct Mapping {
    ptr: *mut libc::c_void,
    len: usize,
}

// SAFETY: the mapping is owned exclusively by this struct, which hands out
// only shared slices of it, and `munmap` may be called from any thread.
unsafe impl Send for Mapping {}

impl Mapping {
    pub fn new(fd: RawFd, offset: u32, len: usize) -> io::Result<Self> {
        // SAFETY: fd is an open video node and (offset, len) came from
        // VIDIOC_QUERYBUF for one of its buffers, which is what the kernel
        // documents as the argument to mmap on a V4L2 node.
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                libc::off_t::from(offset),
            )
        };
        if ptr == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { ptr, len })
    }

    /// The buffer's first `used` bytes.
    pub fn bytes(&self, used: usize) -> &[u8] {
        // SAFETY: the mapping is `self.len` readable bytes for as long as
        // `self` lives, and the slice is clamped to it.
        unsafe { std::slice::from_raw_parts(self.ptr.cast::<u8>(), used.min(self.len)) }
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: `ptr`/`len` are the mapping this struct made and has not
        // unmapped, and nothing else holds a slice of it -- `bytes` borrows
        // from `&self`.
        unsafe { libc::munmap(self.ptr, self.len) };
    }
}

/// A NUL-terminated byte array as text.
pub fn cstr(bytes: &[u8]) -> String {
    CStr::from_bytes_until_nul(bytes)
        .map(|c| c.to_string_lossy().into_owned())
        .unwrap_or_else(|_| String::from_utf8_lossy(bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    /// The sizes `<linux/videodev2.h>` gives on x86_64 and aarch64. If one of
    /// these is wrong every ioctl using it fails with `ENOTTY`, because the
    /// size is part of the request number -- so this test is the difference
    /// between a typo and a camera that "is not supported".
    #[test]
    fn structs_match_the_kernel_abi() {
        assert_eq!(size_of::<Capability>(), 104);
        assert_eq!(size_of::<FmtDesc>(), 64);
        assert_eq!(size_of::<Format>(), 208);
        assert_eq!(size_of::<RequestBuffers>(), 20);
        assert_eq!(size_of::<Buffer>(), 88);
        assert_eq!(size_of::<QueryCtrl>(), 68);
        assert_eq!(size_of::<Control>(), 8);
    }

    #[test]
    fn request_numbers_match_the_header() {
        assert_eq!(VIDIOC_QUERYCAP, 0x8068_5600);
        assert_eq!(VIDIOC_S_FMT, 0xc0d0_5605);
        assert_eq!(VIDIOC_REQBUFS, 0xc014_5608);
        assert_eq!(VIDIOC_QBUF, 0xc058_560f);
        assert_eq!(VIDIOC_DQBUF, 0xc058_5611);
        assert_eq!(VIDIOC_STREAMON, 0x4004_5612);
        assert_eq!(VIDIOC_S_CTRL, 0xc008_561c);
    }

    /// A driver that reports the union of both its nodes' capabilities must
    /// not make a metadata node look like a camera.
    #[test]
    fn device_caps_win_over_the_drivers_union() {
        let mut cap: Capability = zeroed();
        cap.capabilities = CAP_DEVICE_CAPS | CAP_VIDEO_CAPTURE | CAP_STREAMING | CAP_META_CAPTURE;
        cap.device_caps = CAP_META_CAPTURE;
        assert!(!cap.can_capture(), "a metadata node is not a camera");

        cap.device_caps = CAP_VIDEO_CAPTURE | CAP_STREAMING;
        assert!(cap.can_capture());
    }

    #[test]
    fn fourccs_are_little_endian_words() {
        assert_eq!(PIX_MJPEG, u32::from_le_bytes(*b"MJPG"));
        assert_eq!(PIX_GREY, u32::from_le_bytes(*b"GREY"));
    }
}
