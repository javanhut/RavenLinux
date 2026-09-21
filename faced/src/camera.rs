//! Finding a camera, streaming from it, and turning what comes out into
//! pixels the models can be handed.
//!
//! # Which node
//!
//! A laptop with a camera has between one and four `/dev/video*` nodes, and
//! only some of them are cameras: the same device usually exposes a metadata
//! node beside its capture node, and a machine with Windows Hello hardware
//! exposes a second, infrared camera that looks exactly like the first to
//! anything that only reads the node's name.
//!
//! So nodes are chosen by what they answer rather than by what they are
//! called, and an infrared one wins when there is one. An IR sensor sees a
//! warm face and does not see a photograph of one, which is a better liveness
//! check than this daemon can build out of a screen -- see [`crate::liveness`].
//!
//! # Colour order
//!
//! Frames come out of here as BGR, not RGB, because both models were trained
//! on OpenCV's output and OpenCV is BGR. Converting once here rather than
//! twice at the model boundary means there is one place to get it wrong, and
//! getting it wrong is not a crash -- it is a recogniser that quietly performs
//! worse than it should, which is the kind of bug that ships.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::v4l2::{self, Mapping, Node, PixFormat};

/// What the camera is asked for. Plenty for a face that fills a good part of
/// the frame, and small enough that a forward pass is not felt.
const WANT_WIDTH: u32 = 640;
const WANT_HEIGHT: u32 = 480;

/// Capture buffers. Four is the usual floor for a UVC camera to stream
/// smoothly, and more would only add latency between a flash going up and the
/// frame that saw it being dequeued.
const BUFFERS: u32 = 4;

/// One frame, as BGR bytes.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// `width * height * 3` bytes, blue first.
    pub bgr: Vec<u8>,
    /// When the *kernel* says this was captured, not when it was decoded.
    pub at: Duration,
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the pixels. A `{:?}` on a frame of somebody's face would put
        // it in the journal, and this is the process that has the faces.
        f.debug_struct("Frame")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("at", &self.at)
            .finish_non_exhaustive()
    }
}

impl Frame {
    /// The mean brightness of a box, 0.0 to 1.0.
    ///
    /// The liveness check's whole measurement: how bright the face is while a
    /// given colour is on the screen. Per channel, because a red flash and a
    /// blue one of the same luminance must not look alike.
    pub fn mean_bgr(&self, x: i32, y: i32, w: i32, h: i32) -> [f32; 3] {
        let x0 = x.clamp(0, self.width as i32);
        let y0 = y.clamp(0, self.height as i32);
        let x1 = (x + w).clamp(0, self.width as i32);
        let y1 = (y + h).clamp(0, self.height as i32);
        let mut sums = [0u64; 3];
        let mut count = 0u64;
        for row in y0..y1 {
            let base = (row as usize) * (self.width as usize) * 3;
            for col in x0..x1 {
                let i = base + (col as usize) * 3;
                sums[0] += u64::from(self.bgr[i]);
                sums[1] += u64::from(self.bgr[i + 1]);
                sums[2] += u64::from(self.bgr[i + 2]);
                count += 1;
            }
        }
        if count == 0 {
            return [0.0; 3];
        }
        let n = count as f32 * 255.0;
        [
            sums[0] as f32 / n,
            sums[1] as f32 / n,
            sums[2] as f32 / n,
        ]
    }
}

/// An open camera.
#[derive(Debug)]
pub struct Camera {
    node: Node,
    device: PathBuf,
    format: PixFormat,
    infrared: bool,
    maps: Vec<Mapping>,
    streaming: bool,
}

impl Camera {
    /// Find and open the camera this machine should use for faces.
    ///
    /// `Ok(None)` for a machine with no camera, which is ordinary and not an
    /// error -- most machines running this have none.
    pub fn find(prefer: Option<&Path>) -> io::Result<Option<Self>> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(path) = prefer {
            candidates.push(path.to_path_buf());
        } else {
            let mut nodes: Vec<PathBuf> = std::fs::read_dir("/dev")
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("video"))
                })
                .collect();
            // Numerically, so video10 does not come before video2 and change
            // which camera a machine uses between boots.
            nodes.sort_by_key(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|n| n.trim_start_matches("video").parse::<u32>().ok())
                    .unwrap_or(u32::MAX)
            });
            candidates = nodes;
        }

        let mut fallback = None;
        for path in candidates {
            let camera = match Self::open(&path) {
                Ok(Some(camera)) => camera,
                Ok(None) => continue,
                Err(e) => {
                    log::debug!("{}: {e}", path.display());
                    continue;
                }
            };
            // An infrared camera is taken the moment one is found: nothing
            // later in the list can be better.
            if camera.infrared {
                log::info!("using the infrared camera at {}", path.display());
                return Ok(Some(camera));
            }
            if fallback.is_none() {
                fallback = Some(camera);
            }
        }
        if let Some(camera) = &fallback {
            log::info!("using the camera at {}", camera.device.display());
        }
        Ok(fallback)
    }

    /// Open one node, if it is a camera at all. `Ok(None)` if it is not.
    fn open(path: &Path) -> io::Result<Option<Self>> {
        let node = Node::open(path)?;
        let cap = node.query_cap()?;
        if !cap.can_capture() {
            return Ok(None);
        }

        let offered = node.formats();
        // Greyscale and nothing else is how an infrared camera presents. A
        // colour camera offers MJPEG or YUYV as well, whatever else it has.
        let infrared = !offered.is_empty()
            && offered
                .iter()
                .all(|f| *f == v4l2::PIX_GREY)
            || v4l2::cstr(&cap.card).to_lowercase().contains(" ir");

        // MJPEG first: it is what a USB camera can deliver at full size
        // without saturating the bus, and its decoder is in this binary
        // already. YUYV is the universal fallback. GREY is the IR case.
        let order = [v4l2::PIX_MJPEG, v4l2::PIX_YUYV, v4l2::PIX_GREY];
        let chosen = order
            .into_iter()
            .find(|want| offered.contains(want))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::Unsupported,
                    "the camera offers no format this can decode",
                )
            })?;
        let format = node.set_format(WANT_WIDTH, WANT_HEIGHT, chosen)?;
        if format.pixelformat != chosen {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "the camera would not give the format it said it had",
            ));
        }

        Ok(Some(Self {
            node,
            device: path.to_path_buf(),
            format,
            infrared,
            maps: Vec::new(),
            streaming: false,
        }))
    }

    pub fn device(&self) -> &Path {
        &self.device
    }

    pub fn is_infrared(&self) -> bool {
        self.infrared
    }

    /// Start streaming, mapping the buffers if that has not been done.
    pub fn start(&mut self) -> io::Result<()> {
        if self.streaming {
            return Ok(());
        }
        if self.maps.is_empty() {
            let count = self.node.request_buffers(BUFFERS)?;
            if count == 0 {
                return Err(io::Error::other("the camera gave no capture buffers"));
            }
            for index in 0..count {
                let buf = self.node.query_buffer(index)?;
                self.maps.push(Mapping::new(
                    self.node.fd(),
                    buf.offset(),
                    buf.length as usize,
                )?);
            }
        }
        for index in 0..self.maps.len() as u32 {
            self.node.queue(index)?;
        }
        self.node.stream_on()?;
        self.streaming = true;
        Ok(())
    }

    /// Stop streaming. The buffers stay mapped, so starting again is cheap --
    /// which matters, because a watch that reconnects mid-sequence would
    /// otherwise pay for the whole set-up while somebody is standing there.
    pub fn stop(&mut self) {
        if self.streaming {
            if let Err(e) = self.node.stream_off() {
                log::warn!("cannot stop the camera: {e}");
            }
            self.streaming = false;
        }
    }

    /// The next frame, or `None` if none arrived within `timeout`.
    pub fn frame(&mut self, timeout: Duration) -> io::Result<Option<Frame>> {
        if !self.streaming {
            self.start()?;
        }
        if !self.node.wait_readable(timeout)? {
            return Ok(None);
        }
        let buf = match self.node.dequeue() {
            Ok(buf) => buf,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(None),
            Err(e) => return Err(e),
        };
        let index = buf.index;
        let used = buf.bytesused as usize;
        let at = buf.captured_at();

        let map = self
            .maps
            .get(index as usize)
            .ok_or_else(|| io::Error::other("the camera returned a buffer it was not given"))?;
        let decoded = decode(
            map.bytes(used),
            self.format.pixelformat,
            self.format.width,
            self.format.height,
            self.format.bytesperline,
        );

        // Requeued whatever the decode did: a buffer not given back is a
        // buffer the camera runs out of, and a decoder that fails on one
        // frame must not stop the stream.
        if let Err(e) = self.node.queue(index) {
            log::warn!("cannot requeue a capture buffer: {e}");
        }

        Ok(decoded.map(|bgr| Frame {
            width: self.format.width,
            height: self.format.height,
            bgr,
            at,
        }))
    }

    /// Stop the camera correcting for the light this machine is about to throw
    /// at somebody, for as long as the returned guard lives.
    ///
    /// **This is load-bearing, not tuning.** A camera on automatic exposure
    /// and automatic white balance does exactly what the liveness check is
    /// measuring, in reverse: the screen goes green, the camera decides the
    /// scene has a green cast and takes the green back out, and the face
    /// reflects the sequence perfectly while the *frames* show almost nothing.
    /// The first version of this measured a beautifully flat signal off a live
    /// face and could not tell it from a photograph.
    ///
    /// A camera without manual exposure is not refused -- the check still has
    /// the anti-spoofing model, and the correction takes a few frames, so a
    /// short sequence still leaves a signal. It is logged, because it is the
    /// first thing to suspect on a machine where live faces keep failing.
    pub fn pin_exposure(&self) -> Pinned {
        let mut restore = Vec::new();
        for (id, manual) in [
            (v4l2::CID_EXPOSURE_AUTO, v4l2::EXPOSURE_MANUAL),
            (v4l2::CID_AUTO_WHITE_BALANCE, 0),
        ] {
            if !self.node.has_control(id) {
                log::info!(
                    "the camera has no control {id:#x}; the liveness check will be working \
                     against its own automatic correction"
                );
                continue;
            }
            match self.node.control(id) {
                Ok(was) => {
                    if let Err(e) = self.node.set_control(id, manual) {
                        log::warn!("cannot pin control {id:#x}: {e}");
                    } else {
                        restore.push((id, was));
                    }
                }
                Err(e) => log::warn!("cannot read control {id:#x}: {e}"),
            }
        }
        // A descriptor of its own, so the guard can live across the whole
        // challenge while the challenge is capturing through the camera.
        match self.node.dup() {
            Ok(fd) => Pinned {
                fd: Some(fd),
                restore,
            },
            Err(e) => {
                log::warn!("cannot hold a second handle on the camera: {e}");
                Pinned {
                    fd: None,
                    restore: Vec::new(),
                }
            }
        }
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Controls put back when this is dropped.
#[derive(Debug)]
pub struct Pinned {
    fd: Option<std::os::fd::OwnedFd>,
    restore: Vec<(u32, i32)>,
}

impl Drop for Pinned {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        let Some(fd) = &self.fd else {
            return;
        };
        for (id, was) in self.restore.drain(..) {
            if let Err(e) = v4l2::set_control(fd.as_raw_fd(), id, was) {
                log::warn!("cannot put control {id:#x} back to {was}: {e}");
            }
        }
    }
}

/// One frame's bytes as BGR, or `None` for a frame that would not decode.
fn decode(
    bytes: &[u8],
    pixelformat: u32,
    width: u32,
    height: u32,
    stride: u32,
) -> Option<Vec<u8>> {
    match pixelformat {
        v4l2::PIX_MJPEG => decode_mjpeg(bytes, width, height),
        v4l2::PIX_YUYV => Some(decode_yuyv(bytes, width, height, stride)),
        v4l2::PIX_GREY => Some(decode_grey(bytes, width, height, stride)),
        _ => None,
    }
}

fn decode_mjpeg(bytes: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    use zune_jpeg::JpegDecoder;
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    use zune_jpeg::zune_core::options::DecoderOptions;

    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB);
    let mut decoder = JpegDecoder::new_with_options(bytes, options);
    let rgb = decoder.decode().ok()?;
    let (w, h) = decoder.dimensions()?;
    if w != width as usize || h != height as usize {
        log::debug!("a frame decoded to {w}x{h}, not {width}x{height}");
        return None;
    }
    if rgb.len() < (width as usize) * (height as usize) * 3 {
        return None;
    }
    // In place: RGB and BGR differ by a swap of two of every three bytes.
    let mut bgr = rgb;
    for px in bgr.chunks_exact_mut(3) {
        px.swap(0, 2);
    }
    Some(bgr)
}

/// YUYV 4:2:2 to BGR. Two pixels share a chroma pair.
fn decode_yuyv(bytes: &[u8], width: u32, height: u32, stride: u32) -> Vec<u8> {
    let stride = if stride == 0 { width * 2 } else { stride } as usize;
    let (width, height) = (width as usize, height as usize);
    let mut out = vec![0u8; width * height * 3];
    for row in 0..height {
        let line = match bytes.get(row * stride..row * stride + width * 2) {
            Some(line) => line,
            None => break,
        };
        for pair in 0..width / 2 {
            let y0 = f32::from(line[pair * 4]);
            let u = f32::from(line[pair * 4 + 1]) - 128.0;
            let y1 = f32::from(line[pair * 4 + 2]);
            let v = f32::from(line[pair * 4 + 3]) - 128.0;
            for (i, y) in [y0, y1].into_iter().enumerate() {
                let o = (row * width + pair * 2 + i) * 3;
                out[o] = clamp8(y + 1.772 * u);
                out[o + 1] = clamp8(y - 0.344_136 * u - 0.714_136 * v);
                out[o + 2] = clamp8(y + 1.402 * v);
            }
        }
    }
    out
}

/// Greyscale to BGR: the same byte three times, so everything downstream can
/// treat an infrared frame exactly like a colour one.
fn decode_grey(bytes: &[u8], width: u32, height: u32, stride: u32) -> Vec<u8> {
    let stride = if stride == 0 { width } else { stride } as usize;
    let (width, height) = (width as usize, height as usize);
    let mut out = vec![0u8; width * height * 3];
    for row in 0..height {
        let line = match bytes.get(row * stride..row * stride + width) {
            Some(line) => line,
            None => break,
        };
        for (col, grey) in line.iter().enumerate() {
            let o = (row * width + col) * 3;
            out[o] = *grey;
            out[o + 1] = *grey;
            out[o + 2] = *grey;
        }
    }
    out
}

fn clamp8(v: f32) -> u8 {
    v.clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Grey in, grey out, on every channel: an infrared frame has to reach the
    /// models looking like an ordinary one.
    #[test]
    fn grey_becomes_three_equal_channels() {
        let bgr = decode_grey(&[0, 128, 255, 64], 2, 2, 2);
        assert_eq!(bgr, vec![0, 0, 0, 128, 128, 128, 255, 255, 255, 64, 64, 64]);
    }

    /// A short buffer is a truncated frame, not a panic. A camera that sends
    /// one is a camera that just got unplugged.
    #[test]
    fn a_short_buffer_does_not_panic() {
        let _ = decode_grey(&[1, 2], 4, 4, 4);
        let _ = decode_yuyv(&[1, 2, 3], 4, 4, 8);
    }

    /// Mid-grey YUYV with no chroma is mid-grey BGR. This is the test that
    /// catches the coefficients being pasted in the wrong order.
    #[test]
    fn neutral_yuyv_is_neutral_bgr() {
        let bgr = decode_yuyv(&[128, 128, 128, 128], 2, 1, 4);
        for channel in &bgr {
            assert!((i32::from(*channel) - 128).abs() <= 1, "{bgr:?}");
        }
    }

    /// A red pixel must come out with red in the *last* byte. Getting this
    /// backwards does not crash; it quietly makes the recogniser worse.
    #[test]
    fn the_byte_order_is_blue_green_red() {
        // Y=82, U=90, V=240 is BT.601 red.
        let bgr = decode_yuyv(&[82, 90, 82, 240], 2, 1, 4);
        assert!(bgr[2] > 200, "red belongs in the third byte: {bgr:?}");
        assert!(bgr[0] < 60, "not the first: {bgr:?}");
    }

    #[test]
    fn a_mean_over_an_empty_box_is_not_a_division_by_zero() {
        let frame = Frame {
            width: 2,
            height: 2,
            bgr: vec![255; 12],
            at: Duration::ZERO,
        };
        assert_eq!(frame.mean_bgr(0, 0, 0, 0), [0.0; 3]);
        // Entirely off the frame: nothing overlaps, so there is nothing to
        // average. `unusable` refuses a face whose box leaves the frame, so
        // the liveness grid never reads one of these.
        assert_eq!(frame.mean_bgr(-10, -10, 4, 4), [0.0; 3]);
        // Partly on it: the part that is on it.
        assert_eq!(frame.mean_bgr(-1, -1, 4, 4), [1.0; 3]);
        assert_eq!(frame.mean_bgr(0, 0, 100, 100), [1.0; 3]);
    }

    /// A debug print of a frame must not contain the frame.
    #[test]
    fn a_frame_does_not_print_its_pixels() {
        let frame = Frame {
            width: 1,
            height: 1,
            bgr: vec![7, 8, 9],
            at: Duration::ZERO,
        };
        let rendered = format!("{frame:?}");
        assert!(!rendered.contains('7'), "{rendered}");
    }
}
