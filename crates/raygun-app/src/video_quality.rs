//! Picture sizes of video links and stream variants: read from labels, file names and playlist
//! attributes, and the one rule that picks among a video's sizes for playing and downloading
//! (`VideoPlayerPrefs::preferred_resolution`). The players cap adaptive streams the same way
//! (`PlayerTracks.kt`, `video/mediaEngine.ts`).
//!
//! Also what the players' Sources and Quality sheets show: a link's facts read from its names
//! (dynamic range, codec, sound, size, release; [`link_facts`]), and the Quality menu: the
//! resolution ladder, which of the stream's variants or the session's links plays each step
//! ([`rows`]), and the link a tapped step plays ([`pick`]).

use std::cmp::Reverse;
use std::collections::{BTreeSet, HashSet};

use raygun_model::VideoResolution;
use serde::{Deserialize, Serialize};

/// The height a picture counts as: what a 16:9 picture with the same long side has, so 1920×800
/// (a wide film) and 1080×1920 (an upright video) are both 1080p. Rounded, so 854×480 is 480p.
pub fn picture_height(width: u32, height: u32) -> u32 {
    let long = u64::from(width.max(height));
    ((long * 9 + 8) / 16) as u32
}

/// A height from an HLS `RESOLUTION` value ("1920x1080").
pub fn resolution_height(value: &str) -> Option<u32> {
    let (width, height) = value.trim().split_once(['x', 'X'])?;
    let (width, height) = (width.parse().ok()?, height.parse().ok()?);
    (width > 0 && height > 0).then(|| picture_height(width, height))
}

/// A rough height for a stream known only by its bit rate (an HLS variant without
/// `RESOLUTION`), from common H.264 ladders.
pub fn bandwidth_height(bits_per_second: u64) -> u32 {
    match bits_per_second {
        12_000_000.. => 2160,
        7_000_000.. => 1440,
        3_500_000.. => 1080,
        1_800_000.. => 720,
        900_000.. => 480,
        _ => 360,
    }
}

/// Heights a bare number in a label can mean.
const KNOWN_HEIGHTS: [u32; 9] = [4320, 2160, 1440, 1080, 720, 576, 480, 360, 240];

/// The height a label, name or file name gives: "1080p", "720i", "1920x1080", "4K", "UHD",
/// "Full HD", "FHD", "QHD", then a bare "1080" or "720", then "HD" (720) or "SD" (480).
pub fn label_height(text: &str) -> Option<u32> {
    let tokens: Vec<String> = text
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    let explicit = |index: usize, token: &str| -> Option<u32> {
        if let Some(number) = token
            .strip_suffix(['p', 'i'])
            .and_then(|number| number.parse::<u32>().ok())
        {
            return (100..=4320).contains(&number).then_some(number);
        }
        if let Some(height) = resolution_height(token) {
            return Some(height);
        }
        match token {
            "8k" | "4320" => Some(4320),
            "4k" | "uhd" | "2160" => Some(2160),
            "2k" | "qhd" | "1440" => Some(1440),
            "fhd" | "fullhd" => Some(1080),
            "hd" if index > 0 && tokens[index - 1] == "full" => Some(1080),
            _ => None,
        }
    };
    let bare = |token: &str| {
        token
            .parse::<u32>()
            .ok()
            .filter(|number| KNOWN_HEIGHTS.contains(number))
    };
    let loose = |token: &str| match token {
        "hd" => Some(720),
        "sd" => Some(480),
        _ => None,
    };
    let first = |find: &dyn Fn(usize, &str) -> Option<u32>| {
        tokens
            .iter()
            .enumerate()
            .find_map(|(index, token)| find(index, token))
    };
    first(&explicit)
        .or_else(|| first(&|_, token| bare(token)))
        .or_else(|| first(&|_, token| loose(token)))
}

/// The first height any of `texts` gives.
pub fn texts_height<'a>(texts: impl IntoIterator<Item = Option<&'a str>>) -> Option<u32> {
    texts.into_iter().flatten().find_map(label_height)
}

/// How well a height suits `preferred`, lower first: with Highest the highest; otherwise the
/// highest at or below the preferred height, then the lowest above it; unknown sizes last.
pub fn rank(height: Option<u32>, preferred: VideoResolution) -> (u8, u32) {
    match (height, preferred.max_height()) {
        (None, _) => (2, 0),
        (Some(height), None) => (0, u32::MAX - height),
        (Some(height), Some(limit)) if height <= limit => (0, limit - height),
        (Some(height), Some(limit)) => (1, height - limit),
    }
}

// ---- What a link's names say about it ----

/// A picture's dynamic range.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum Range {
    #[default]
    Sdr,
    /// HDR10, HDR10+ or HLG.
    Hdr,
    /// Dolby Vision (with or without an HDR10 layer).
    Dv,
}

impl Range {
    pub fn key(self) -> &'static str {
        match self {
            Self::Sdr => "sdr",
            Self::Hdr => "hdr",
            Self::Dv => "dv",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "sdr" => Some(Self::Sdr),
            "hdr" => Some(Self::Hdr),
            "dv" => Some(Self::Dv),
            _ => None,
        }
    }
}

/// What a link's name, description and file name tell about it, for the Sources sheet.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkFacts {
    pub height: Option<u32>,
    pub range: Range,
    /// "HEVC", "H.264", "AV1", "VP9".
    pub codec: Option<&'static str>,
    /// 10 for "10bit".
    pub bit_depth: Option<u8>,
    /// "DD+ Atmos 5.1", "AAC 2.0".
    pub audio: Option<String>,
    pub size_bytes: Option<u64>,
    /// "REMUX", "BluRay", "WEB-DL", "WEBRip", "WEB", "HDTV", "DVD", "CAM".
    pub release: Option<&'static str>,
}

/// Words of a name, upper-cased: split at anything but letters, digits and `+`.
fn words(text: &str) -> Vec<String> {
    text.split(|character: char| !(character.is_ascii_alphanumeric() || character == '+'))
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_uppercase)
        .collect()
}

/// Each word with the one after it.
fn pairs(words: &[String]) -> impl Iterator<Item = (&str, Option<&str>)> {
    words
        .iter()
        .enumerate()
        .map(|(index, word)| (word.as_str(), words.get(index + 1).map(String::as_str)))
}

fn codec_of(words: &[String]) -> Option<&'static str> {
    pairs(words).find_map(|(word, next)| match word {
        "HEVC" | "X265" | "H265" => Some("HEVC"),
        "H" if next == Some("265") => Some("HEVC"),
        "AV1" => Some("AV1"),
        "VP9" => Some("VP9"),
        "X264" | "H264" | "AVC" => Some("H.264"),
        "H" if next == Some("264") => Some("H.264"),
        _ => None,
    })
}

fn range_of(words: &[String]) -> Range {
    let mut range = Range::Sdr;
    for (word, next) in pairs(words) {
        match word {
            "DV" | "DOVI" | "DOLBYVISION" => return Range::Dv,
            "DOLBY" if next == Some("VISION") => return Range::Dv,
            "HDR" | "HDR10" | "HDR10+" | "HDR10PLUS" | "HLG" => range = Range::Hdr,
            _ => {}
        }
    }
    range
}

fn bit_depth_of(words: &[String]) -> Option<u8> {
    pairs(words).find_map(|(word, next)| match word {
        "10BIT" | "10BITS" | "HI10" | "HI10P" => Some(10),
        "12BIT" | "12BITS" => Some(12),
        "10" | "12" if matches!(next, Some("BIT" | "BITS")) => word.parse().ok(),
        _ => None,
    })
}

/// Releases from the best kind of copy down: the first found of these wins.
const RELEASES: [&str; 8] = [
    "REMUX", "BluRay", "WEB-DL", "WEBRip", "WEB", "HDTV", "DVD", "CAM",
];

fn release_of(words: &[String]) -> Option<&'static str> {
    let found: Vec<&'static str> = pairs(words)
        .filter_map(|(word, next)| match word {
            "REMUX" | "BDREMUX" => Some("REMUX"),
            "BLURAY" | "BDRIP" | "BRRIP" => Some("BluRay"),
            "BLU" if next == Some("RAY") => Some("BluRay"),
            "WEBDL" => Some("WEB-DL"),
            "WEB" if next == Some("DL") => Some("WEB-DL"),
            "WEBRIP" => Some("WEBRip"),
            "WEB" if next == Some("RIP") => Some("WEBRip"),
            "WEB" => Some("WEB"),
            "HDTV" | "PDTV" => Some("HDTV"),
            "DVDRIP" | "DVD" | "DVD5" | "DVD9" => Some("DVD"),
            "CAM" | "HDCAM" | "CAMRIP" | "TELESYNC" | "HDTS" | "TELECINE" => Some("CAM"),
            _ => None,
        })
        .collect();
    RELEASES
        .into_iter()
        .find(|release| found.contains(release))
}

/// A word without the channel count glued to it ("DDP5" from "DDP5.1").
fn without_digits(word: &str) -> &str {
    word.trim_end_matches(|character: char| character.is_ascii_digit())
}

/// The sound: its codec, Atmos, and its channels.
fn audio_of(words: &[String], texts: &[&str]) -> Option<String> {
    const CODECS: [&str; 10] = [
        "TrueHD", "DTS-HD", "DTS:X", "DTS", "DD+", "DD", "AAC", "Opus", "FLAC", "MP3",
    ];
    let found: Vec<&'static str> = pairs(words)
        .filter_map(|(word, next)| match word {
            "MP3" => Some("MP3"),
            "AC3" => Some("DD"),
            "EAC3" => Some("DD+"),
            _ => audio_word(without_digits(word), next),
        })
        .collect();
    let codec = CODECS.into_iter().find(|codec| found.contains(codec));
    let atmos = words.iter().any(|word| word == "ATMOS");
    let channels = texts.iter().find_map(|text| channels_of(text)).or_else(|| {
        words.iter().find_map(|word| match word.as_str() {
            "8CH" => Some("7.1"),
            "6CH" => Some("5.1"),
            "2CH" => Some("2.0"),
            _ => None,
        })
    });
    let parts: Vec<&str> = [codec, atmos.then_some("Atmos"), channels]
        .into_iter()
        .flatten()
        .collect();
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// A sound codec's word, without the channels glued to it, and the word after it.
fn audio_word(word: &str, next: Option<&str>) -> Option<&'static str> {
    match word {
        "TRUEHD" => Some("TrueHD"),
        "DTSHD" => Some("DTS-HD"),
        "DTS" if next == Some("HD") => Some("DTS-HD"),
        "DTSX" => Some("DTS:X"),
        "DTS" if next == Some("X") => Some("DTS:X"),
        "DTS" => Some("DTS"),
        "DDP" | "DD+" | "EAC" => Some("DD+"),
        "E" if next.is_some_and(|next| next.starts_with("AC3")) => Some("DD+"),
        "DD" => Some("DD"),
        "AAC" => Some("AAC"),
        "OPUS" => Some("Opus"),
        "FLAC" => Some("FLAC"),
        _ => None,
    }
}

/// Channel layouts a name gives as "5.1"; never a size ("2.1 GB") or part of a longer number.
fn channels_of(text: &str) -> Option<&'static str> {
    const LAYOUTS: [&str; 9] = ["7.1", "5.1", "2.0", "6.1", "1.0", "2.1", "5.0", "7.0", "4.0"];
    let bytes = text.as_bytes();
    for (at, byte) in bytes.iter().enumerate() {
        if *byte != b'.' || at == 0 || at + 1 >= bytes.len() {
            continue;
        }
        let (before, after) = (bytes[at - 1], bytes[at + 1]);
        let lone_before = at < 2 || !bytes[at - 2].is_ascii_digit();
        let lone_after = bytes.get(at + 2).is_none_or(|next| !next.is_ascii_digit());
        if !(before.is_ascii_digit() && after.is_ascii_digit() && lone_before && lone_after) {
            continue;
        }
        let rest = text[at + 2..].trim_start().to_ascii_uppercase();
        if ["GB", "GIB", "MB", "MIB", "TB", "KB"]
            .iter()
            .any(|unit| rest.starts_with(unit))
        {
            continue;
        }
        let layout = &text[at - 1..at + 2];
        if let Some(known) = LAYOUTS.into_iter().find(|known| *known == layout) {
            return Some(known);
        }
    }
    None
}

/// A size a name gives ("💾 12.3 GB", "1.4GiB", "700 MB"), in bytes.
pub fn size_of(text: &str) -> Option<u64> {
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if !bytes[at].is_ascii_digit() || (at > 0 && bytes[at - 1].is_ascii_alphanumeric()) {
            at += 1;
            continue;
        }
        let start = at;
        while at < bytes.len() && (bytes[at].is_ascii_digit() || matches!(bytes[at], b'.' | b',')) {
            at += 1;
        }
        let number: f64 = match text[start..at].replace(',', ".").trim_end_matches('.').parse() {
            Ok(number) => number,
            Err(_) => continue,
        };
        let rest = text[at..].trim_start();
        let unit: String = rest
            .chars()
            .take_while(char::is_ascii_alphabetic)
            .collect::<String>()
            .to_ascii_uppercase();
        let scale = match unit.as_str() {
            "TB" => 1e12,
            "TIB" => 1_099_511_627_776.0,
            "GB" => 1e9,
            "GIB" => 1_073_741_824.0,
            "MB" => 1e6,
            "MIB" => 1_048_576.0,
            _ => continue,
        };
        let size = (number * scale).round();
        if size >= 1e6 {
            return Some(size as u64);
        }
    }
    None
}

/// What `texts` (a link's label, name, description, file name) say about it; `size` and
/// `height` are what the source says outright.
pub fn link_facts(texts: &[Option<&str>], size: Option<u64>, height: Option<u32>) -> LinkFacts {
    let texts: Vec<&str> = texts.iter().copied().flatten().collect();
    let words: Vec<String> = texts.iter().flat_map(|text| words(text)).collect();
    LinkFacts {
        height: height.or_else(|| texts_height(texts.iter().map(|text| Some(*text)))),
        range: range_of(&words),
        codec: codec_of(&words),
        bit_depth: bit_depth_of(&words),
        audio: audio_of(&words, &texts),
        size_bytes: size
            .filter(|size| *size > 0)
            .or_else(|| texts.iter().find_map(|text| size_of(text))),
        release: release_of(&words),
    }
}

/// The bit rate a file of `size_bytes` lasting `duration_ms` streams at, in kb/s.
pub fn bitrate_kbps(size_bytes: Option<u64>, duration_ms: Option<i64>) -> Option<u64> {
    let duration = u64::try_from(duration_ms?).ok().filter(|ms| *ms > 0)?;
    size_bytes.map(|size| size.saturating_mul(8) / duration)
}

// ---- The Quality menu ----

/// The steps the Quality menu lists, largest first; 8K only when something has it.
pub const LADDER: [u32; 9] = [4320, 2160, 1440, 1080, 720, 480, 360, 240, 144];

/// The ladder step a height counts as: the nearest one, ties going up.
pub fn step(height: u32) -> u32 {
    LADDER
        .into_iter()
        .min_by_key(|step| (step.abs_diff(height), Reverse(*step)))
        .unwrap_or(height)
}

/// A step of the Quality menu: a height on the ladder and a dynamic range ("1080:sdr").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QualityKey {
    pub height: u32,
    pub range: Range,
}

impl QualityKey {
    /// The step `height` counts as, in `range`.
    pub fn of(height: u32, range: Range) -> Self {
        Self {
            height: step(height),
            range,
        }
    }

    pub fn text(&self) -> String {
        format!("{}:{}", self.height, self.range.key())
    }

    pub fn parse(text: &str) -> Option<Self> {
        let (height, range) = text.split_once(':')?;
        Some(Self {
            height: height.parse().ok().filter(|height| LADDER.contains(height))?,
            range: Range::from_key(range)?,
        })
    }
}

/// A link the menu can play: its place in the session and its step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offer {
    pub group: usize,
    pub link: usize,
    pub key: QualityKey,
    pub codec: Option<&'static str>,
    /// Its place in the order the session tries links (lower first).
    pub rank: usize,
}

/// A variant of the stream playing (HLS or DASH), as the player reports it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Variant {
    pub height: u32,
    #[serde(default)]
    pub range: Range,
    #[serde(default)]
    pub codec: Option<String>,
}

/// How a step of the menu plays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum RowState {
    /// What plays now.
    Playing,
    /// Another variant of the stream playing: the player switches to it itself.
    Variant,
    /// Another link of the source playing.
    #[serde(rename_all = "camelCase")]
    ThisSource { group: usize, link: usize },
    /// Only another source has it: `source` is the one that would play.
    #[serde(rename_all = "camelCase")]
    OtherSource {
        group: usize,
        link: usize,
        source: String,
    },
    /// No source has it.
    Missing,
}

/// A step of the Quality menu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityRow {
    pub key: String,
    pub height: u32,
    pub range: Range,
    /// The codec of what would play, when known.
    pub codec: Option<String>,
    #[serde(flatten)]
    pub state: RowState,
}

/// What the Quality menu is made from.
pub struct Menu<'a> {
    /// The session's links that can play, with their steps.
    pub offers: &'a [Offer],
    /// The sources' names, by group.
    pub names: &'a [String],
    /// The link playing.
    pub current: Option<(usize, usize)>,
    /// The playing stream's own variants (HLS, DASH).
    pub variants: &'a [Variant],
    /// The variant playing, when the stream has several.
    pub playing: Option<QualityKey>,
}

/// The menu's rows, largest first: every step of the ladder from 4K down (8K when something has
/// it), each range a step comes in on its own row (Dolby Vision, HDR, then SDR). A step plays
/// from the stream playing when it has that variant, else from another link of the source
/// playing, else from another source (the first a session would try); steps no source has are
/// listed as missing.
pub fn rows(menu: &Menu) -> Vec<QualityRow> {
    let current_group = menu.current.map(|(group, _)| group);
    let current_offer = menu
        .current
        .and_then(|(group, link)| {
            menu.offers
                .iter()
                .find(|offer| offer.group == group && offer.link == link)
        })
        .copied();
    let variants: Vec<(QualityKey, Option<&str>)> = menu
        .variants
        .iter()
        .filter(|variant| variant.height > 0)
        .map(|variant| {
            (
                QualityKey::of(variant.height, variant.range),
                variant.codec.as_deref(),
            )
        })
        .collect();
    let playing = menu
        .playing
        .or_else(|| current_offer.map(|offer| offer.key))
        .filter(|_| menu.current.is_some() || !variants.is_empty());
    let known: BTreeSet<QualityKey> = menu
        .offers
        .iter()
        .map(|offer| offer.key)
        .chain(variants.iter().map(|(key, _)| *key))
        .chain(playing)
        .collect();
    let best = |own_source: bool, key: QualityKey| {
        menu.offers
            .iter()
            .filter(|offer| offer.key == key && Some((offer.group, offer.link)) != menu.current)
            .filter(|offer| (Some(offer.group) == current_group) == own_source)
            .min_by_key(|offer| offer.rank)
    };
    let mut out = Vec::new();
    for height in LADDER {
        let mut ranges: Vec<Range> = known
            .iter()
            .filter(|key| key.height == height)
            .map(|key| key.range)
            .collect();
        if ranges.is_empty() {
            if height == 4320 {
                continue;
            }
            ranges.push(Range::Sdr);
        }
        ranges.sort_by_key(|range| Reverse(*range));
        for range in ranges {
            let key = QualityKey { height, range };
            let variant = variants.iter().find(|(found, _)| *found == key);
            let (state, codec) = if playing == Some(key) {
                let codec = variant
                    .and_then(|(_, codec)| *codec)
                    .or(current_offer.and_then(|offer| offer.codec));
                (RowState::Playing, codec)
            } else if let Some((_, codec)) = variant {
                (RowState::Variant, *codec)
            } else if let Some(offer) = best(true, key) {
                (
                    RowState::ThisSource {
                        group: offer.group,
                        link: offer.link,
                    },
                    offer.codec,
                )
            } else if let Some(offer) = best(false, key) {
                (
                    RowState::OtherSource {
                        group: offer.group,
                        link: offer.link,
                        source: menu.names.get(offer.group).cloned().unwrap_or_default(),
                    },
                    offer.codec,
                )
            } else {
                (RowState::Missing, None)
            };
            out.push(QualityRow {
                key: key.text(),
                height,
                range,
                codec: codec.map(str::to_owned),
                state,
            });
        }
    }
    out
}

/// The link a tapped step plays: the source playing first, then the others in the order a session
/// tries them. Links already tried for this step are passed over (once all were, the first comes
/// round again); never the link playing.
pub fn pick(
    offers: &[Offer],
    key: QualityKey,
    current: Option<(usize, usize)>,
    tried: &HashSet<(usize, usize)>,
) -> Option<(usize, usize)> {
    let current_group = current.map(|(group, _)| group);
    let mut candidates: Vec<&Offer> = offers
        .iter()
        .filter(|offer| offer.key == key && Some((offer.group, offer.link)) != current)
        .collect();
    candidates.sort_by_key(|offer| (Some(offer.group) != current_group, offer.rank));
    candidates
        .iter()
        .find(|offer| !tried.contains(&(offer.group, offer.link)))
        .or(candidates.first())
        .map(|offer| (offer.group, offer.link))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heights_come_from_labels_names_and_files() {
        let cases = [
            ("1080p", Some(1080)),
            ("720", Some(720)),
            ("Server 2 · 480p", Some(480)),
            ("Torrentio\n4k", Some(2160)),
            ("4K HDR", Some(2160)),
            ("UHD BluRay", Some(2160)),
            ("FHD", Some(1080)),
            ("Full HD", Some(1080)),
            ("QHD", Some(1440)),
            ("HD", Some(720)),
            ("SD", Some(480)),
            ("Movie.2019.1080i.HDTV.x264", Some(1080)),
            ("Show.S01E02.2160p.WEB-DL.DDP5.1.HDR.H.265", Some(2160)),
            ("1920x800", Some(1080)),
            ("HD 1080p", Some(1080)),
            ("torrentio|720p|BluRay", Some(720)),
            ("HDRip x265 10bit", None),
            ("Auto", None),
            ("Episode 12", None),
        ];
        for (text, height) in cases {
            assert_eq!(label_height(text), height, "{text}");
        }
        assert_eq!(
            texts_height([None, Some("Addon"), Some("Film.720p.mkv")]),
            Some(720)
        );
    }

    #[test]
    fn pictures_count_by_their_long_side() {
        assert_eq!(picture_height(1920, 1080), 1080);
        assert_eq!(picture_height(1920, 800), 1080);
        assert_eq!(picture_height(1080, 1920), 1080);
        assert_eq!(picture_height(854, 480), 480);
        assert_eq!(picture_height(3840, 1600), 2160);
        assert_eq!(resolution_height("1280x720"), Some(720));
        assert_eq!(resolution_height("0x0"), None);
        assert_eq!(bandwidth_height(5_000_000), 1080);
        assert_eq!(bandwidth_height(800_000), 360);
    }

    fn pick(heights: &[Option<u32>], preferred: VideoResolution) -> Option<u32> {
        heights
            .iter()
            .copied()
            .min_by_key(|height| rank(*height, preferred))
            .flatten()
    }

    #[test]
    fn the_highest_at_or_below_the_preference_else_the_lowest_above() {
        let all = [
            Some(360),
            None,
            Some(2160),
            Some(720),
            Some(1080),
            Some(480),
        ];
        assert_eq!(pick(&all, VideoResolution::Highest), Some(2160));
        assert_eq!(pick(&all, VideoResolution::P2160), Some(2160));
        assert_eq!(pick(&all, VideoResolution::P1440), Some(1080));
        assert_eq!(pick(&all, VideoResolution::P1080), Some(1080));
        assert_eq!(pick(&all, VideoResolution::P720), Some(720));
        assert_eq!(
            pick(&[Some(1080), Some(2160)], VideoResolution::P720),
            Some(1080)
        );
        assert_eq!(
            pick(&[None, Some(480)], VideoResolution::Highest),
            Some(480)
        );
        assert_eq!(VideoResolution::default(), VideoResolution::P1080);
    }

    #[test]
    fn a_links_names_give_its_range_codec_sound_size_and_release() {
        let torrent = link_facts(
            &[
                Some("Torrentio\n4k DV | HDR"),
                Some(
                    "The.Movie.2019.2160p.WEB-DL.DDP5.1.Atmos.DV.HDR.H.265-GROUP\n👤 23 💾 15.2 GB ⚙️ RARBG",
                ),
            ],
            None,
            None,
        );
        assert_eq!(
            torrent,
            LinkFacts {
                height: Some(2160),
                range: Range::Dv,
                codec: Some("HEVC"),
                bit_depth: None,
                audio: Some("DD+ Atmos 5.1".into()),
                size_bytes: Some(15_200_000_000),
                release: Some("WEB-DL"),
            }
        );
        let plain = link_facts(
            &[Some("Show.S01E02.1080p.BluRay.x264.AAC2.0-X"), Some("1.4GiB")],
            Some(0),
            None,
        );
        assert_eq!(plain.height, Some(1080));
        assert_eq!(plain.range, Range::Sdr);
        assert_eq!(plain.codec, Some("H.264"));
        assert_eq!(plain.audio.as_deref(), Some("AAC 2.0"));
        assert_eq!(plain.size_bytes, Some(1_503_238_554));
        assert_eq!(plain.release, Some("BluRay"));
        // The source's own size and height win; "2.1 GB" is a size, not a sound.
        let given = link_facts(
            &[Some("Remux 2.1 GB HDR10+ 10bit TrueHD 7.1 E-AC3")],
            Some(3_000_000_000),
            Some(720),
        );
        assert_eq!(given.height, Some(720));
        assert_eq!(given.size_bytes, Some(3_000_000_000));
        assert_eq!(given.range, Range::Hdr);
        assert_eq!(given.bit_depth, Some(10));
        assert_eq!(given.audio.as_deref(), Some("TrueHD 7.1"));
        assert_eq!(given.release, Some("REMUX"));
        let nothing = link_facts(&[Some("Server 1"), None], None, None);
        assert_eq!(nothing, LinkFacts::default());
        assert_eq!(size_of("💾 700 MB"), Some(700_000_000));
        assert_eq!(size_of("x264 12 seeds"), None);
        assert_eq!(bitrate_kbps(Some(1_350_000_000), Some(5_400_000)), Some(2_000));
        assert_eq!(bitrate_kbps(Some(1), None), None);
    }

    #[test]
    fn heights_fall_on_the_ladder_and_keys_read_back() {
        assert_eq!(step(1080), 1080);
        assert_eq!(step(1076), 1080);
        assert_eq!(step(576), 480);
        assert_eq!(step(900), 1080, "a tie goes up");
        assert_eq!(step(100), 144);
        assert_eq!(step(5000), 4320);
        let key = QualityKey::of(2160, Range::Hdr);
        assert_eq!(key.text(), "2160:hdr");
        assert_eq!(QualityKey::parse("2160:hdr"), Some(key));
        assert_eq!(QualityKey::parse("1000:sdr"), None, "only ladder steps");
        assert_eq!(QualityKey::parse("1080:hdr10"), None);
    }

    fn offer(group: usize, link: usize, height: u32, range: Range, rank: usize) -> Offer {
        Offer {
            group,
            link,
            key: QualityKey::of(height, range),
            codec: None,
            rank,
        }
    }

    fn states(rows: &[QualityRow]) -> Vec<(String, String)> {
        rows.iter()
            .map(|row| {
                let state = match &row.state {
                    RowState::Playing => "playing".to_owned(),
                    RowState::Variant => "variant".to_owned(),
                    RowState::ThisSource { group, link } => format!("here {group}/{link}"),
                    RowState::OtherSource { source, .. } => format!("from {source}"),
                    RowState::Missing => "missing".to_owned(),
                };
                (row.key.clone(), state)
            })
            .collect()
    }

    #[test]
    fn the_menu_lists_the_ladder_from_the_source_playing_then_the_others() {
        let names = vec!["Alpha".to_owned(), "Beta".to_owned()];
        let offers = [
            offer(0, 0, 1080, Range::Sdr, 0),
            offer(0, 1, 720, Range::Sdr, 2),
            offer(1, 0, 2160, Range::Dv, 1),
            offer(1, 1, 2160, Range::Sdr, 3),
            offer(1, 2, 720, Range::Sdr, 4),
        ];
        let menu = Menu {
            offers: &offers,
            names: &names,
            current: Some((0, 0)),
            variants: &[],
            playing: None,
        };
        let shown = states(&rows(&menu));
        let expected = [
            ("2160:dv", "from Beta"),
            ("2160:sdr", "from Beta"),
            ("1440:sdr", "missing"),
            ("1080:sdr", "playing"),
            ("720:sdr", "here 0/1"),
            ("480:sdr", "missing"),
            ("360:sdr", "missing"),
            ("240:sdr", "missing"),
            ("144:sdr", "missing"),
        ];
        assert_eq!(
            shown,
            expected
                .iter()
                .map(|(key, state)| ((*key).to_owned(), (*state).to_owned()))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn the_streams_own_variants_come_before_other_links() {
        let names = vec!["Alpha".to_owned()];
        let offers = [offer(0, 0, 1080, Range::Sdr, 0), offer(0, 1, 480, Range::Sdr, 1)];
        let variants = [
            Variant {
                height: 1080,
                range: Range::Sdr,
                codec: Some("H.264".into()),
            },
            Variant {
                height: 720,
                range: Range::Sdr,
                codec: None,
            },
            Variant {
                height: 480,
                range: Range::Sdr,
                codec: None,
            },
        ];
        let menu = Menu {
            offers: &offers,
            names: &names,
            current: Some((0, 0)),
            variants: &variants,
            playing: Some(QualityKey::of(720, Range::Sdr)),
        };
        let rows = rows(&menu);
        let shown = states(&rows);
        assert!(shown.contains(&("720:sdr".into(), "playing".into())));
        assert!(shown.contains(&("1080:sdr".into(), "variant".into())));
        assert!(shown.contains(&("480:sdr".into(), "variant".into())));
        assert_eq!(rows[2].codec.as_deref(), Some("H.264"));
        // Without a session, the variants alone.
        let alone = Menu {
            offers: &[],
            names: &[],
            current: None,
            variants: &variants,
            playing: Some(QualityKey::of(1080, Range::Sdr)),
        };
        let shown = states(&super::rows(&alone));
        assert_eq!(shown[2], ("1080:sdr".into(), "playing".into()));
        assert_eq!(shown[3], ("720:sdr".into(), "variant".into()));
    }

    #[test]
    fn a_step_tapped_again_tries_the_next_source_with_it() {
        let offers = [
            offer(0, 0, 1080, Range::Sdr, 2),
            offer(1, 0, 720, Range::Sdr, 0),
            offer(2, 0, 720, Range::Sdr, 1),
            offer(0, 1, 720, Range::Sdr, 5),
        ];
        let key = QualityKey::of(720, Range::Sdr);
        let mut tried = HashSet::new();
        // The source playing has it: that link first.
        let first = super::pick(&offers, key, Some((0, 0)), &tried).unwrap();
        assert_eq!(first, (0, 1));
        tried.insert(first);
        // Tapped again while it plays: the other sources, in play order.
        let second = super::pick(&offers, key, Some(first), &tried).unwrap();
        assert_eq!(second, (1, 0));
        tried.insert(second);
        let third = super::pick(&offers, key, Some(second), &tried).unwrap();
        assert_eq!(third, (2, 0));
        tried.insert(third);
        // Every one was tried: round again, never the one playing.
        assert_eq!(super::pick(&offers, key, Some(third), &tried), Some((1, 0)));
        assert_eq!(
            super::pick(&offers, QualityKey::of(2160, Range::Sdr), None, &tried),
            None
        );
    }
}
