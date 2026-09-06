//! A minimal PNG writer.
//!
//! Hand-rolled rather than pulled from crates.io, because the alternative is a
//! dependency tree on a tool whose whole job is to let somebody look at a
//! rectangle. PNG's container is four chunks and a CRC; the only real work is
//! that the pixel data must be zlib-framed, and zlib permits stored
//! (uncompressed) deflate blocks — so there is no compressor here, just a
//! header, a length, its complement, and the bytes.
//!
//! The files are correspondingly large. They are build artifacts in a preview
//! directory, looked at once and deleted, so that is the right trade.

use std::io::Write;
use std::path::Path;

use crate::gfx::Canvas;

pub fn write(path: &Path, canvas: &Canvas) -> std::io::Result<()> {
    let width = canvas.width() as u32;
    let height = canvas.height() as u32;

    // Each row is prefixed with its filter type, 0 = none.
    let mut raw = Vec::with_capacity((width as usize * 3 + 1) * height as usize);
    for (i, pixel) in canvas.pixels().iter().enumerate() {
        if i % width as usize == 0 {
            raw.push(0);
        }
        raw.push(pixel.red);
        raw.push(pixel.green);
        raw.push(pixel.blue);
    }

    let mut png: Vec<u8> = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit, truecolour RGB
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"IDAT", &zlib_stored(&raw));
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

/// `data` in a zlib stream of stored deflate blocks.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    // 0x78 0x01: deflate, 32K window, no preset dictionary, fastest level.
    let mut out = vec![0x78, 0x01];
    // A stored block's length is a u16, so anything larger is split.
    let mut chunks = data.chunks(0xFFFF).peekable();
    if data.is_empty() {
        out.extend_from_slice(&[0x01, 0x00, 0x00, 0xFF, 0xFF]);
    }
    while let Some(block) = chunks.next() {
        let final_block = u8::from(chunks.peek().is_none());
        let len = block.len() as u16;
        out.push(final_block);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(block);
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
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
