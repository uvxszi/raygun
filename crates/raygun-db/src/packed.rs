//! Large device-local values kept compressed: `local_meta` values (Made for You, the saved
//! queue, artists' releases…) and the lyrics cache. Their columns hold text, so a packed value is
//! deflate as base64 after a marker. Short values, ones that don't get smaller, and everything
//! written before stay plain text and read as they are, so nothing needs migrating.

use std::borrow::Cow;
use std::io::{Read, Write};

use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;

/// Starts a packed value. No JSON, number, id or name kept in these columns starts with a
/// control character.
const MARK: &str = "\u{1}z1:";

/// Values shorter than this stay plain: packing would save little and cost a decode, bytes.
pub const PACK_FROM: usize = 2048;

/// `value` as it's kept: packed when it's long and packing makes it smaller.
pub fn pack(value: &str) -> Cow<'_, str> {
    if value.len() < PACK_FROM || value.starts_with(MARK) {
        return Cow::Borrowed(value);
    }
    let mut encoder = ZlibEncoder::new(Vec::with_capacity(value.len() / 4), Compression::default());
    let Ok(bytes) = encoder
        .write_all(value.as_bytes())
        .and_then(|()| encoder.finish())
    else {
        return Cow::Borrowed(value);
    };
    let packed = format!("{MARK}{}", STANDARD_NO_PAD.encode(bytes));
    if packed.len() < value.len() {
        Cow::Owned(packed)
    } else {
        Cow::Borrowed(value)
    }
}

/// What `stored` holds: unpacked when [pack] packed it, as it is otherwise. `None` for a packed
/// value that's damaged.
pub fn unpack(stored: &str) -> Option<Cow<'_, str>> {
    let Some(body) = stored.strip_prefix(MARK) else {
        return Some(Cow::Borrowed(stored));
    };
    let bytes = STANDARD_NO_PAD.decode(body).ok()?;
    let mut text = String::new();
    ZlibDecoder::new(bytes.as_slice())
        .read_to_string(&mut text)
        .ok()?;
    Some(Cow::Owned(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A day's mixes as Made for You keeps them: long, repetitive JSON.
    fn mixes() -> String {
        let songs: Vec<String> = (0..200)
            .map(|index| {
                format!(
                    r#"{{"title":"Song {index}","artist":"Artist {}","album":null,"durationMs":{},"artworkUrl":"https://coverartarchive.org/release/{index}/front-250"}}"#,
                    index % 17,
                    180_000 + index * 7
                )
            })
            .collect();
        format!(
            r#"{{"day":"2026-11-09","mixes":[{{"songs":[{}]}}]}}"#,
            songs.join(",")
        )
    }

    #[test]
    fn long_values_are_packed_and_read_back_exactly() {
        let value = mixes();
        let packed = pack(&value);
        assert!(packed.starts_with(MARK));
        assert!(
            packed.len() * 4 < value.len(),
            "{} of {}",
            packed.len(),
            value.len()
        );
        assert_eq!(unpack(&packed).as_deref(), Some(value.as_str()));
    }

    #[test]
    fn short_values_and_ones_that_dont_shrink_stay_plain() {
        assert!(matches!(pack("[1,2,3]"), Cow::Borrowed("[1,2,3]")));
        // Random letters hardly compress; base64 would make them longer.
        let mut seed = 7_u32;
        let noise: String = (0..4096)
            .map(|_| {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                char::from(b'!' + ((seed >> 16) % 90) as u8)
            })
            .collect();
        assert!(matches!(pack(&noise), Cow::Borrowed(_)));
    }

    #[test]
    fn plain_values_written_before_read_as_they_are() {
        for value in ["", "1700000000000", "pending", r#"{"a":[1,2]}"#, &mixes()] {
            assert_eq!(unpack(value).as_deref(), Some(value));
        }
    }

    #[test]
    fn a_damaged_packed_value_reads_as_none() {
        let packed = pack(&mixes()).into_owned();
        assert_eq!(unpack(&packed[..packed.len() / 2]), None);
        assert_eq!(unpack(&format!("{MARK}not base64!")), None);
    }

    #[test]
    fn unicode_survives() {
        let value = "Sigur Rós – Hoppípolla, 宇多田ヒカル, Beyoncé. ".repeat(100);
        assert_eq!(unpack(&pack(&value)).as_deref(), Some(value.as_str()));
    }
}
