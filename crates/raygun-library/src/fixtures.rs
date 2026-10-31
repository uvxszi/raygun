//! Builds small, valid, tagged MP3 files for tests and for generating large test libraries
//! (`cargo run -p raygun-library --example make_test_library`).

use std::path::Path;

use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::*;
use lofty::tag::{ItemKey, Tag, TagType};

/// One MPEG-1 Layer III frame (128 kbps, 44.1 kHz, no padding) of silence: 417 bytes.
fn silent_frame() -> Vec<u8> {
    let mut frame = vec![0u8; 417];
    frame[..4].copy_from_slice(&[0xFF, 0xFB, 0x90, 0x64]);
    frame
}

/// Tags written into a fixture file.
#[derive(Debug, Clone, Default)]
pub struct FixtureTags<'a> {
    pub title: Option<&'a str>,
    pub artist: Option<&'a str>,
    pub album_artist: Option<&'a str>,
    pub album: Option<&'a str>,
    pub track_no: Option<u32>,
    pub year: Option<u16>,
    pub art_png: Option<&'a [u8]>,
}

/// Writes an MP3 of about `seconds` of silence with the given tags (no tag at all when every field
/// is empty).
pub fn write_mp3(path: &Path, seconds: u32, tags: &FixtureTags) -> std::io::Result<()> {
    // 38.28 frames per second at 44.1 kHz with 1152 samples per frame.
    let frames = (seconds.max(1) as usize * 383).div_ceil(10);
    let frame = silent_frame();
    let mut bytes = Vec::with_capacity(frame.len() * frames);
    for _ in 0..frames {
        bytes.extend_from_slice(&frame);
    }
    std::fs::write(path, bytes)?;

    let mut tag = Tag::new(TagType::Id3v2);
    let mut has_any = false;
    let mut set = |key: ItemKey, value: Option<&str>| {
        if let Some(value) = value {
            tag.insert_text(key, value.to_owned());
            has_any = true;
        }
    };
    set(ItemKey::TrackTitle, tags.title);
    set(ItemKey::TrackArtist, tags.artist);
    set(ItemKey::AlbumArtist, tags.album_artist);
    set(ItemKey::AlbumTitle, tags.album);
    if let Some(track) = tags.track_no {
        tag.set_track(track);
        has_any = true;
    }
    if let Some(year) = tags.year {
        tag.insert_text(ItemKey::RecordingDate, year.to_string());
        has_any = true;
    }
    if let Some(png) = tags.art_png {
        tag.push_picture(
            Picture::unchecked(png.to_vec())
                .pic_type(PictureType::CoverFront)
                .mime_type(MimeType::Png)
                .build(),
        );
        has_any = true;
    }
    if has_any {
        tag.save_to_path(path, WriteOptions::default())
            .map_err(std::io::Error::other)?;
    }
    Ok(())
}

/// A 1×1 PNG in the given color, for distinct artwork per album.
pub fn tiny_png(red: u8, green: u8, blue: u8) -> Vec<u8> {
    // Uncompressed (stored) deflate block wrapped in zlib; CRCs computed below.
    let raw = [0u8, red, green, blue];
    let mut zlib = vec![0x78, 0x01, 0x01, 0x04, 0x00, 0xFB, 0xFF];
    zlib.extend_from_slice(&raw);
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    push_chunk(&mut png, b"IHDR", &ihdr);
    push_chunk(&mut png, b"IDAT", &zlib);
    push_chunk(&mut png, b"IEND", &[]);
    png
}

fn push_chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut crc_input = kind.to_vec();
    crc_input.extend_from_slice(data);
    png.extend_from_slice(&crc_input);
    png.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut low, mut high) = (1u32, 0u32);
    for &byte in bytes {
        low = (low + u32::from(byte)) % 65_521;
        high = (high + low) % 65_521;
    }
    (high << 16) | low
}
