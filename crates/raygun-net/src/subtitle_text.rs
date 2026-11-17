//! Subtitle files as players read them: sources serve them gzipped, in old encodings
//! (Windows-1252, UTF-16) and under addresses that don't say their format, so the proxy's
//! `/subtitle/` route unpacks them, gives UTF-8, and writes them in the format the player was
//! told (`as=srt`/`as=vtt`, through [`crate::subtitles`]): a WebVTT or ASS file behind an
//! address without an extension would otherwise be read as SubRip and show nothing.

use std::io::Read;

use crate::subtitles;

/// What a subtitle file holds, by its content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Srt,
    Vtt,
    Ass,
    Other,
}

/// The largest file unpacked from gzip.
const MAX_UNPACKED: u64 = 16 * 1024 * 1024;

/// `bytes` unpacked, as UTF-8, in the `wanted` format (`srt` or `vtt`) when it's in another
/// timed text one; anything else as it is (in UTF-8).
pub fn normalize(bytes: &[u8], wanted: Option<&str>) -> Vec<u8> {
    let unpacked = gunzip(bytes);
    let text = decode(unpacked.as_deref().unwrap_or(bytes));
    let kind = kind_of(&text);
    let wanted = wanted.map(str::to_ascii_lowercase);
    let out = match (wanted.as_deref(), kind) {
        (Some("srt" | "subrip"), Kind::Vtt | Kind::Ass) => {
            subtitles::to_srt(&subtitles::parse(&text))
        }
        (Some("vtt" | "webvtt"), Kind::Srt | Kind::Ass) => {
            subtitles::to_vtt(&subtitles::parse(&text))
        }
        _ => text,
    };
    out.into_bytes()
}

fn gunzip(bytes: &[u8]) -> Option<Vec<u8>> {
    if !bytes.starts_with(&[0x1f, 0x8b]) {
        return None;
    }
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .take(MAX_UNPACKED)
        .read_to_end(&mut out)
        .ok()?;
    Some(out)
}

/// Text from a file's bytes: UTF-8 (a byte order mark dropped), UTF-16 by its mark, else
/// Windows-1252 (what most older subtitle files are in).
pub fn decode(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return encoding_rs::UTF_16LE
            .decode_with_bom_removal(bytes)
            .0
            .into_owned();
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return encoding_rs::UTF_16BE
            .decode_with_bom_removal(bytes)
            .0
            .into_owned();
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => encoding_rs::WINDOWS_1252.decode(bytes).0.into_owned(),
    }
}

/// What `text` is, by what it starts with and holds.
pub fn kind_of(text: &str) -> Kind {
    let start = text.trim_start_matches('\u{feff}').trim_start();
    if start.starts_with("WEBVTT") {
        return Kind::Vtt;
    }
    if start.to_ascii_lowercase().starts_with("[script info]") || text.contains("\nDialogue:") {
        return Kind::Ass;
    }
    match text.lines().find(|line| line.contains("-->")) {
        // SubRip's times have a comma; WebVTT's a dot (some files leave out the header).
        Some(timing) if timing.contains(',') => Kind::Srt,
        Some(_) => Kind::Vtt,
        None => Kind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VTT: &str = "WEBVTT\n\n1\n00:01.000 --> 00:02.500 align:start\n<i>Hello</i>\n\n00:00:03.000 --> 00:00:04.000\nAgain\n";

    #[test]
    fn webvtt_reads_as_subrip() {
        let srt = String::from_utf8(normalize(VTT.as_bytes(), Some("srt"))).unwrap();
        assert_eq!(kind_of(&srt), Kind::Srt);
        assert!(srt.contains("00:00:01,000 --> 00:00:02,500"));
        assert!(srt.contains("00:00:03,000 --> 00:00:04,000\r\nAgain"));
    }

    #[test]
    fn subrip_reads_as_webvtt_and_stays_subrip_when_asked() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\nHi\n";
        let vtt = String::from_utf8(normalize(srt.as_bytes(), Some("vtt"))).unwrap();
        assert!(vtt.starts_with("WEBVTT"));
        assert!(vtt.contains("00:00:01.000 --> 00:00:02.000\nHi"));
        assert_eq!(normalize(srt.as_bytes(), Some("srt")), srt.as_bytes());
        assert_eq!(kind_of(srt), Kind::Srt);
        assert_eq!(kind_of("00:01.000 --> 00:02.000\nHi"), Kind::Vtt);
    }

    #[test]
    fn ass_reads_as_subrip() {
        let ass = "[Script Info]\nTitle: x\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:05.00,0:00:06.50,Default,,0,0,0,,Second\nDialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,First\n";
        let srt = String::from_utf8(normalize(ass.as_bytes(), Some("srt"))).unwrap();
        let first = srt.find("First").unwrap();
        let second = srt.find("Second").unwrap();
        assert!(first < second, "{srt}");
        assert!(srt.contains("00:00:05,000 --> 00:00:06,500"));
    }

    #[test]
    fn old_encodings_and_gzip_become_utf8() {
        let latin = b"1\n00:00:01,000 --> 00:00:02,000\nCaf\xe9\n";
        let text = String::from_utf8(normalize(latin, None)).unwrap();
        assert!(text.contains("Café"));
        let mut packed = Vec::new();
        {
            use std::io::Write;
            let mut encoder =
                flate2::write::GzEncoder::new(&mut packed, flate2::Compression::default());
            encoder.write_all(VTT.as_bytes()).unwrap();
        }
        let srt = String::from_utf8(normalize(&packed, Some("srt"))).unwrap();
        assert!(srt.starts_with("1\r\n00:00:01,000"), "{srt}");
        let utf16: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain("WEBVTT".encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        assert_eq!(normalize(&utf16, None), b"WEBVTT");
    }
}
