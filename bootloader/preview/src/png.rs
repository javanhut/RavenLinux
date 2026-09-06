//! A minimal PNG reader and writer.
//!
//! The container is hand-rolled — four chunks and a CRC is not worth a
//! dependency — but the pixel data is zlib, and that is [`flate2`]'s.
//!
//! It was not, at first. The first version wrote *stored* (uncompressed)
//! deflate blocks, which zlib permits and which needs no compressor at all,
//! on the reasoning that these are build artifacts looked at once and deleted.
//! That was true of four 1080p previews and false by the time there were twelve
//! including 4K: the directory reached 81MB, a single 3840x2160 render being
//! 24MB of a file that compresses to about two hundred kilobytes.
//!
//! The same writer also produces the shipped GRUB theme in `configs/grub/theme`,
//! which is checked in, so the waste was not confined to a scratch directory.
//! `flate2` is pure Rust (miniz_oxide), host-side only, and reaches nothing that
//! ships in the `.efi`.

use std::io::{Read, Write};
use std::path::Path;

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;

use crate::gfx::Canvas;

pub fn write(path: &Path, canvas: &Canvas) -> std::io::Result<()> {
    write_region(
        path,
        canvas,
        0,
        0,
        canvas.width() as usize,
        canvas.height() as usize,
    )
}

/// One rectangle of `canvas`, as its own PNG.
///
/// The nine-slice cutter in `grub.rs` needs this: the tiles have to come out of
/// a single rendering so that every seam is the same anti-aliased edge at the
/// same subpixel phase, which drawing each tile separately would not give.
pub fn write_region(
    path: &Path,
    canvas: &Canvas,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) -> std::io::Result<()> {
    let stride = canvas.width() as usize;
    let pixels = canvas.pixels();

    // Each row is prefixed with its filter type, 0 = none.
    let mut raw = Vec::with_capacity((width * 3 + 1) * height);
    for row in 0..height {
        raw.push(0);
        for col in 0..width {
            let pixel = pixels[(y + row) * stride + (x + col)];
            raw.push(pixel.red);
            raw.push(pixel.green);
            raw.push(pixel.blue);
        }
    }

    let width = width as u32;
    let height = height as u32;

    let mut png: Vec<u8> = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit, truecolour RGB
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"IDAT", &deflate(&raw)?);
    chunk(&mut png, b"IEND", &[]);

    std::fs::File::create(path)?.write_all(&png)
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// `data` as a zlib stream.
///
/// Default compression rather than `best`: these are flat-coloured screens of
/// large uniform runs, where the two land within a few percent of each other and
/// the default is several times quicker. The preview regenerates on every edit,
/// so its speed is part of the loop being fast.
fn deflate(data: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    encoder.finish()
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------
//
// Only the subset `write` above produces: 8-bit truecolour, no interlacing, and
// filter 0 on every row. That is not a PNG decoder and must not be pointed at a
// PNG from anywhere else -- it exists so `grubsim` can load the theme's own
// tiles back and compose them, which is the only way to check the nine-slice cut
// without running GRUB.
//
// The zlib framing is `flate2`'s, so this half is not restricted to what the
// writer happens to emit; the filter and colour-type checks below are.

/// A decoded image, RGB.
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<u8>,
}

impl Image {
    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> (u8, u8, u8) {
        let i = (y * self.width + x) * 3;
        (self.rgb[i], self.rgb[i + 1], self.rgb[i + 2])
    }
}

pub fn read(path: &Path) -> std::io::Result<Image> {
    let data = std::fs::read(path)?;
    let bad = |what: &str| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{}: {what}", path.display()),
        )
    };

    if data.len() < 8 || &data[..8] != b"\x89PNG\r\n\x1a\n" {
        return Err(bad("not a PNG"));
    }

    let mut width = 0usize;
    let mut height = 0usize;
    let mut idat: Vec<u8> = Vec::new();
    let mut at = 8;

    while at + 8 <= data.len() {
        let len = u32::from_be_bytes(data[at..at + 4].try_into().unwrap()) as usize;
        let kind = &data[at + 4..at + 8];
        let body = data
            .get(at + 8..at + 8 + len)
            .ok_or_else(|| bad("truncated chunk"))?;

        match kind {
            b"IHDR" => {
                width = u32::from_be_bytes(body[0..4].try_into().unwrap()) as usize;
                height = u32::from_be_bytes(body[4..8].try_into().unwrap()) as usize;
                if body[8] != 8 || body[9] != 2 {
                    return Err(bad("not 8-bit truecolour"));
                }
            }
            b"IDAT" => idat.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
        at += 12 + len; // length + type + body + CRC
    }

    let mut raw = Vec::with_capacity(width * height * 3 + height);
    ZlibDecoder::new(idat.as_slice()).read_to_end(&mut raw)?;

    // Strip the per-row filter byte. Every row is filter 0, which `write`
    // guarantees and this checks rather than assumes.
    let stride = width * 3;
    let mut rgb = Vec::with_capacity(stride * height);
    for row in 0..height {
        let at = row * (stride + 1);
        let filter = *raw.get(at).ok_or_else(|| bad("truncated pixel data"))?;
        if filter != 0 {
            return Err(bad("only filter type 0 is supported"));
        }
        rgb.extend_from_slice(
            raw.get(at + 1..at + 1 + stride)
                .ok_or_else(|| bad("truncated row"))?,
        );
    }

    Ok(Image { width, height, rgb })
}
