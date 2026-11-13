//! Image type and pixel size, read from the file header.

use std::fs::File;
use std::io::{self, BufReader, ErrorKind, Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Clone, Copy)]
pub enum Format {
    Jpeg,
    Png,
    Gif,
    Bmp,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Format::Jpeg => "jpg",
            Format::Png => "png",
            Format::Gif => "gif",
            Format::Bmp => "bmp",
        }
    }

    pub fn media_type(self) -> &'static str {
        match self {
            Format::Jpeg => "image/jpeg",
            Format::Png => "image/png",
            Format::Gif => "image/gif",
            Format::Bmp => "image/bmp",
        }
    }
}

pub struct Image {
    pub format: Format,
    pub width: u32,
    pub height: u32,
}

/// Identifies an image by its magic bytes, not its file name.
/// Returns `None` when the file is not an image kindlegen accepts.
pub fn probe(path: &Path) -> io::Result<Option<Image>> {
    let mut file = BufReader::new(File::open(path)?);
    let mut head = Vec::with_capacity(HEAD_LENGTH);
    file.by_ref()
        .take(HEAD_LENGTH as u64)
        .read_to_end(&mut head)?;

    let found = if head.starts_with(b"\x89PNG\r\n\x1a\n") && head.len() >= 24 {
        Some((Format::Png, be32(&head[16..]), be32(&head[20..])))
    } else if (head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a")) && head.len() >= 10 {
        Some((Format::Gif, le16(&head[6..]), le16(&head[8..])))
    } else if head.starts_with(b"BM") && head.len() >= HEAD_LENGTH {
        // The height is negative for top-down bitmaps.
        let side = |at: usize| (le32(&head[at..]) as i32).unsigned_abs();
        Some((Format::Bmp, side(18), side(22)))
    } else if head.starts_with(&[0xFF, 0xD8]) {
        file.seek(SeekFrom::Start(2))?;
        match jpeg_size(&mut file) {
            Ok(size) => size.map(|(width, height)| (Format::Jpeg, width, height)),
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => None,
            Err(e) => return Err(e),
        }
    } else {
        None
    };

    Ok(found
        .filter(|&(_, width, height)| width > 0 && height > 0)
        .map(|(format, width, height)| Image {
            format,
            width,
            height,
        }))
}

// JPEG segment markers (the byte after 0xFF).
const JPEG_TEM: u8 = 0x01;
const JPEG_RST0: u8 = 0xD0;
const JPEG_SOI: u8 = 0xD8;
const JPEG_EOI: u8 = 0xD9;
const JPEG_SOS: u8 = 0xDA;
const JPEG_SOF0: u8 = 0xC0;
const JPEG_SOF15: u8 = 0xCF;
/// In the start-of-frame range, but not frame headers.
const JPEG_DHT: u8 = 0xC4;
const JPEG_JPG: u8 = 0xC8;
const JPEG_DAC: u8 = 0xCC;

/// Bytes read up front: enough for the PNG, GIF and BMP size fields.
const HEAD_LENGTH: usize = 26;

/// Walks the JPEG segments up to the first start-of-frame, which holds the size.
fn jpeg_size(file: &mut BufReader<File>) -> io::Result<Option<(u32, u32)>> {
    loop {
        let mut byte = [0u8; 1];
        file.read_exact(&mut byte)?;
        if byte[0] != 0xFF {
            return Ok(None);
        }
        // A marker may be preceded by any number of 0xFF fill bytes.
        while byte[0] == 0xFF {
            file.read_exact(&mut byte)?;
        }
        let marker = byte[0];
        match marker {
            JPEG_TEM | JPEG_RST0..=JPEG_SOI => continue, // markers without a payload
            JPEG_EOI | JPEG_SOS => return Ok(None),      // image data began without a frame header
            _ => {}
        }

        let mut length = [0u8; 2];
        file.read_exact(&mut length)?;
        let length = u16::from_be_bytes(length);
        let is_frame_header = matches!(marker, JPEG_SOF0..=JPEG_SOF15)
            && !matches!(marker, JPEG_DHT | JPEG_JPG | JPEG_DAC);
        if is_frame_header {
            let mut frame = [0u8; 5]; // precision, height, width
            file.read_exact(&mut frame)?;
            return Ok(Some((be16(&frame[3..]), be16(&frame[1..]))));
        }
        if length < 2 {
            return Ok(None);
        }
        file.seek_relative(i64::from(length) - 2)?;
    }
}

fn be16(bytes: &[u8]) -> u32 {
    u32::from(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn le16(bytes: &[u8]) -> u32 {
    u32::from(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn le32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}
