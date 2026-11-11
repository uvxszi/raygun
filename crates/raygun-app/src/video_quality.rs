//! Picture sizes of video links and stream variants: read from labels, file names, playlist
//! attributes and the sizes sources and streams give, and grouped into the standard tiers
//! (2160p, 1080p, 720p…) that the Quality menu lists and the preferred resolution names. A
//! picture keeps its own size beside its tier: a film cropped to 1920×816 is 1080p, shown as
//! "1080p (1920×816)" ([`Picture`]). The one rule that picks among a video's sizes for playing
//! and downloading (`VideoPlayerPrefs::preferred_resolution`, [`rank`]); the players cap adaptive
//! streams the same way (`PlayerTracks.kt`, `video/mediaEngine.ts`).
//!
//! Also what the players' Sources and Quality sheets show: a link's facts read from its names
//! (dynamic range, codec, sound, size, release; [`link_facts`]), and the Quality menu: the tiers
//! the stream's variants and the session's links have, which of them plays each ([`rows`]), and
//! the link a tapped tier plays, the nearest tier any source has when none has it ([`pick`]).

use std::cmp::Reverse;
use std::collections::{BTreeSet, HashSet};

use raygun_model::VideoResolution;
use serde::{Deserialize, Serialize};

/// The standard tiers by the height of a 16:9 picture, largest first: what the Quality menu
/// lists and the preferred resolution names.
pub const TIERS: [u32; 9] = [4320, 2160, 1440, 1080, 720, 480, 360, 240, 144];

/// 16:9 heights between tiers that pictures are made at (PAL's 576, qHD's 540): they count as the
/// nearest tier, where other heights between tiers are taken for cropped films ([`step`]).
const PLAIN_HEIGHTS: [u32; 4] = [576, 540, 270, 180];

/// The share of its tier's height a film cropped in a 16:9 frame keeps at least, in percent
/// (2.40:1 keeps 74 %: 1920×800).
const CROP_PERCENT: u64 = 74;
/// How far past its tier's height a picture may go and still be it, in percent: encoders store
/// 1080 lines as 1088.
const PADDED_PERCENT: u64 = 103;

/// The tier nearest `height`, ties going up.
fn nearest_tier(height: u32) -> u32 {
    TIERS
        .into_iter()
        .min_by_key(|tier| (tier.abs_diff(height), Reverse(*tier)))
        .unwrap_or(height)
}

/// The height a picture with both sides counts as: a 16:9 picture's with its long side, so
/// 1920×800 (a wide film) and 1080×1920 (an upright video) are both 1080; or its short side when
/// that is more, so 640×480 and 1440×1080 (narrower than 16:9) are 480 and 1080. Rounded, so
/// 854×480 is 480.
pub fn picture_height(width: u32, height: u32) -> u32 {
    let (short, long) = (width.min(height), width.max(height));
    let wide = ((u64::from(long) * 9 + 8) / 16) as u32;
    wide.max(short)
}

/// The tier a height alone counts as. A tier's own height, and the plain 16:9 heights between
/// tiers, count as the nearest tier; any other height is taken for a film cropped from a wider
/// frame, the smallest tier it fits (1036, 816 and 800 lines are 1080p films, 1600 a 2160p one,
/// 1088 a padded 1080); a height that fits none counts as the nearest.
pub fn step(height: u32) -> u32 {
    if TIERS.contains(&height) || PLAIN_HEIGHTS.contains(&height) {
        return nearest_tier(height);
    }
    let lines = u64::from(height) * 100;
    TIERS
        .into_iter()
        .rev()
        .find(|tier| {
            let tier = u64::from(*tier);
            lines >= tier * CROP_PERCENT && lines <= tier * PADDED_PERCENT
        })
        .unwrap_or_else(|| nearest_tier(height))
}

/// What's known of a picture's size: both sides (a stream's, a name's "1920x816"), its height (a
/// source's, a name's "816p"), its width (a stream that only says that), or only the tier a name
/// gives ("4K", "Full HD").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Picture {
    Size { width: u32, height: u32 },
    Height(u32),
    Width(u32),
    Tier(u32),
}

impl Picture {
    /// A stream's picture, either side of which may be unknown (0); none when both are.
    pub fn of(width: u32, height: u32) -> Option<Self> {
        match (width > 0, height > 0) {
            (true, true) => Some(Self::Size { width, height }),
            (false, true) => Some(Self::Height(height)),
            (true, false) => Some(Self::Width(width)),
            (false, false) => None,
        }
    }

    /// The tier it counts as.
    pub fn tier(self) -> u32 {
        match self {
            Self::Size { width, height } => nearest_tier(picture_height(width, height)),
            Self::Height(height) => step(height),
            Self::Width(width) => nearest_tier(picture_height(width, 0)),
            Self::Tier(tier) => nearest_tier(tier),
        }
    }

    /// Its own size as shown beside its tier ("1920×816", "816p"), when that isn't the tier's
    /// plain 16:9 size; a width or a name alone says nothing the tier doesn't.
    pub fn exact(self) -> Option<String> {
        let tier = self.tier();
        match self {
            Self::Size { width, height } => {
                let plain = height == tier && width.abs_diff(tier * 16 / 9) <= 8;
                (!plain).then(|| format!("{width}×{height}"))
            }
            Self::Height(height) => (height != tier).then(|| format!("{height}p")),
            Self::Width(_) | Self::Tier(_) => None,
        }
    }
}

/// The tier of an HLS `RESOLUTION` value ("1920x1080", "1920x816").
pub fn resolution_height(value: &str) -> Option<u32> {
    resolution_size(value).map(Picture::tier)
}

/// Both sides of a `RESOLUTION`-like value ("1920x1080").
fn resolution_size(value: &str) -> Option<Picture> {
    let (width, height) = value.trim().split_once(['x', 'X'])?;
    let (width, height) = (width.parse().ok()?, height.parse().ok()?);
    (width > 0 && height > 0).then_some(Picture::Size { width, height })
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

/// The picture a label, name or file name gives: "1080p", "720i", "816p", "1920x1080", "4K",
/// "UHD", "Full HD", "FHD", "QHD", then a bare "1080" or "720", then "HD" (720) or "SD" (480).
pub fn label_picture(text: &str) -> Option<Picture> {
    let tokens: Vec<String> = text
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    let explicit = |index: usize, token: &str| -> Option<Picture> {
        if let Some(number) = token
            .strip_suffix(['p', 'i'])
            .and_then(|number| number.parse::<u32>().ok())
        {
            return (100..=4320)
                .contains(&number)
                .then_some(Picture::Height(number));
        }
        if let Some(size) = resolution_size(token) {
            return Some(size);
        }
        match token {
            "8k" | "4320" => Some(Picture::Tier(4320)),
            "4k" | "uhd" | "2160" => Some(Picture::Tier(2160)),
            "2k" | "qhd" | "1440" => Some(Picture::Tier(1440)),
            "fhd" | "fullhd" => Some(Picture::Tier(1080)),
            "hd" if index > 0 && tokens[index - 1] == "full" => Some(Picture::Tier(1080)),
            _ => None,
        }
    };
    let bare = |token: &str| {
        token
            .parse::<u32>()
            .ok()
            .filter(|number| KNOWN_HEIGHTS.contains(number))
            .map(Picture::Height)
    };
    let loose = |token: &str| match token {
        "hd" => Some(Picture::Tier(720)),
        "sd" => Some(Picture::Tier(480)),
        _ => None,
    };
    let first = |find: &dyn Fn(usize, &str) -> Option<Picture>| {
        tokens
            .iter()
            .enumerate()
            .find_map(|(index, token)| find(index, token))
    };
    first(&explicit)
        .or_else(|| first(&|_, token| bare(token)))
        .or_else(|| first(&|_, token| loose(token)))
}

/// The tier a label, name or file name gives ([`label_picture`]): "816p" and "1920x816" are 1080.
pub fn label_height(text: &str) -> Option<u32> {
    label_picture(text).map(Picture::tier)
}

/// The first tier any of `texts` gives.
pub fn texts_height<'a>(texts: impl IntoIterator<Item = Option<&'a str>>) -> Option<u32> {
    texts.into_iter().flatten().find_map(label_height)
}

/// How well a height suits `preferred`, lower first: with Highest the highest; otherwise the
/// highest at or below the preferred height, then the lowest above it; unknown sizes last.
/// Heights count by their tier ([`step`]), so a 1088-line or a cropped 816-line 1080p picture
/// is 1080p.
pub fn rank(height: Option<u32>, preferred: VideoResolution) -> (u8, u32) {
    match (height.map(step), preferred.max_height()) {
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
    /// The tier its picture counts as.
    pub height: Option<u32>,
    /// Its picture's own size beside the tier, when that isn't the tier's plain one ("1920×816").
    pub exact: Option<String>,
    #[serde(skip)]
    pub picture: Option<Picture>,
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
    RELEASES.into_iter().find(|release| found.contains(release))
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
    const LAYOUTS: [&str; 9] = [
        "7.1", "5.1", "2.0", "6.1", "1.0", "2.1", "5.0", "7.0", "4.0",
    ];
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
        let number: f64 = match text[start..at]
            .replace(',', ".")
            .trim_end_matches('.')
            .parse()
        {
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
/// `height` are what the source says outright (a height in pixels, or the tier its names gave).
/// A name's "1920x816" stays the picture of a link whose height is that tier.
pub fn link_facts(texts: &[Option<&str>], size: Option<u64>, height: Option<u32>) -> LinkFacts {
    let texts: Vec<&str> = texts.iter().copied().flatten().collect();
    let words: Vec<String> = texts.iter().flat_map(|text| words(text)).collect();
    let named = texts.iter().find_map(|text| label_picture(text));
    let picture = match height {
        Some(height) => Some(match named {
            Some(found @ Picture::Size { .. }) if found.tier() == step(height) => found,
            _ => Picture::Height(height),
        }),
        None => named,
    };
    LinkFacts {
        height: picture.map(Picture::tier),
        exact: picture.and_then(Picture::exact),
        picture,
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

/// A row of the Quality menu: a tier and a dynamic range ("1080:sdr").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QualityKey {
    pub height: u32,
    pub range: Range,
}

/// How far a tier is from another: tiers between them, whether it's the larger, whether its
/// range differs. Lower is nearer: the lower of two tiers as far, the same range first.
pub type Distance = (usize, bool, bool);

/// A tier's place among [`TIERS`].
fn tier_index(height: u32) -> usize {
    let tier = nearest_tier(height);
    TIERS
        .iter()
        .position(|known| *known == tier)
        .unwrap_or_default()
}

impl QualityKey {
    /// The tier `height` counts as ([`step`]), in `range`.
    pub fn of(height: u32, range: Range) -> Self {
        Self {
            height: step(height),
            range,
        }
    }

    /// The tier `picture` counts as, in `range`.
    pub fn of_picture(picture: Picture, range: Range) -> Self {
        Self {
            height: picture.tier(),
            range,
        }
    }

    pub fn text(&self) -> String {
        format!("{}:{}", self.height, self.range.key())
    }

    pub fn parse(text: &str) -> Option<Self> {
        let (height, range) = text.split_once(':')?;
        Some(Self {
            height: height
                .parse()
                .ok()
                .filter(|height| TIERS.contains(height))?,
            range: Range::from_key(range)?,
        })
    }

    /// How far `other` is from this one.
    pub fn distance(&self, other: QualityKey) -> Distance {
        (
            tier_index(self.height).abs_diff(tier_index(other.height)),
            other.height > self.height,
            other.range != self.range,
        )
    }
}

/// A link the menu can play: its place in the session, its tier and its own picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offer {
    pub group: usize,
    pub link: usize,
    pub key: QualityKey,
    pub codec: Option<&'static str>,
    /// What its source or names say of its picture.
    pub picture: Option<Picture>,
    /// Its place in the order the session tries links (lower first).
    pub rank: usize,
}

/// A variant of the stream playing (HLS or DASH), as the player reports it: its picture's own
/// sides (0 when unknown).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Variant {
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub range: Range,
    #[serde(default)]
    pub codec: Option<String>,
}

impl Variant {
    pub fn picture(&self) -> Option<Picture> {
        Picture::of(self.width, self.height)
    }

    /// Its row of the menu; none when its size isn't known.
    pub fn key(&self) -> Option<QualityKey> {
        self.picture()
            .map(|picture| QualityKey::of_picture(picture, self.range))
    }
}

/// A variant's sides as the player reported them, so it can tell which of its tracks a row
/// means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct VariantSize {
    pub width: u32,
    pub height: u32,
}

/// How a row of the menu plays.
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
}

/// A row of the Quality menu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityRow {
    pub key: String,
    /// The tier.
    pub height: u32,
    pub range: Range,
    /// The codec of what would play, when known.
    pub codec: Option<String>,
    /// The picture of what would play, beside its tier, when that isn't the tier's plain size
    /// ("1920×816", "816p").
    pub exact: Option<String>,
    /// The stream's own variants in this row, by their sides as reported: the one to switch to
    /// is among them.
    pub sizes: Vec<VariantSize>,
    #[serde(flatten)]
    pub state: RowState,
}

/// What the Quality menu is made from.
pub struct Menu<'a> {
    /// The session's links that can play, with their tiers.
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

/// The menu's rows, largest first: the tiers the stream's variants and the session's links have,
/// each range a tier comes in on its own row (Dolby Vision, HDR, then SDR). A row plays from the
/// stream playing when it has that variant, else from another link of the source playing, else
/// from another source (the first a session would try); a tier only the link playing claims
/// while its stream plays another isn't listed. Each row names the picture that would play when
/// it isn't its tier's plain size.
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
    let variants: Vec<(QualityKey, &Variant)> = menu
        .variants
        .iter()
        .filter_map(|variant| variant.key().map(|key| (key, variant)))
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
    for key in known.into_iter().rev() {
        let held: Vec<&Variant> = variants
            .iter()
            .filter(|(found, _)| *found == key)
            .map(|(_, variant)| *variant)
            .collect();
        // The largest picture of the stream's variants in this row.
        let variant = held
            .iter()
            .max_by_key(|variant| (variant.height, variant.width))
            .copied();
        let (state, codec, picture) = if playing == Some(key) {
            let offer = current_offer.filter(|offer| offer.key == key);
            (
                RowState::Playing,
                variant
                    .and_then(|variant| variant.codec.clone())
                    .or_else(|| offer.and_then(|offer| offer.codec).map(str::to_owned)),
                variant
                    .and_then(Variant::picture)
                    .or_else(|| offer.and_then(|offer| offer.picture)),
            )
        } else if let Some(variant) = variant {
            (RowState::Variant, variant.codec.clone(), variant.picture())
        } else if let Some(offer) = best(true, key) {
            (
                RowState::ThisSource {
                    group: offer.group,
                    link: offer.link,
                },
                offer.codec.map(str::to_owned),
                offer.picture,
            )
        } else if let Some(offer) = best(false, key) {
            (
                RowState::OtherSource {
                    group: offer.group,
                    link: offer.link,
                    source: menu.names.get(offer.group).cloned().unwrap_or_default(),
                },
                offer.codec.map(str::to_owned),
                offer.picture,
            )
        } else {
            continue;
        };
        out.push(QualityRow {
            key: key.text(),
            height: key.height,
            range: key.range,
            codec,
            exact: picture.and_then(Picture::exact),
            sizes: held
                .iter()
                .map(|variant| VariantSize {
                    width: variant.width,
                    height: variant.height,
                })
                .collect(),
            state,
        });
    }
    out
}

/// The link a tapped row plays: one with its tier and range, from the source playing first, then
/// the others in the order a session tries them. When no link has it at all (a tier asked for
/// before the links changed), the nearest any has ([`QualityKey::distance`]), again the source
/// playing's first; the row that plays, tapped again with no other link to it, plays nothing
/// else. Links already tried for this row are passed over (once all were, the first comes round
/// again); never the link playing.
pub fn pick(
    offers: &[Offer],
    key: QualityKey,
    current: Option<(usize, usize)>,
    tried: &HashSet<(usize, usize)>,
) -> Option<(usize, usize)> {
    let current_group = current.map(|(group, _)| group);
    let others: Vec<&Offer> = offers
        .iter()
        .filter(|offer| Some((offer.group, offer.link)) != current)
        .collect();
    let nearest = if offers.iter().any(|offer| offer.key == key) {
        key.distance(key)
    } else {
        others.iter().map(|offer| key.distance(offer.key)).min()?
    };
    let mut candidates: Vec<&Offer> = others
        .into_iter()
        .filter(|offer| key.distance(offer.key) == nearest)
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
        // Narrower than 16:9: by their height.
        assert_eq!(picture_height(640, 480), 480);
        assert_eq!(picture_height(1440, 1080), 1080);
        assert_eq!(resolution_height("1280x720"), Some(720));
        assert_eq!(resolution_height("1920x816"), Some(1080));
        assert_eq!(resolution_height("1280x800"), Some(720));
        assert_eq!(resolution_height("0x0"), None);
        assert_eq!(bandwidth_height(5_000_000), 1080);
        assert_eq!(bandwidth_height(800_000), 360);
    }

    #[test]
    fn cropped_and_padded_heights_count_as_their_tier() {
        // Films cropped from a 1080p frame (2.40:1, 2.35:1, 1.85:1), and a padded 1080.
        for height in [800, 816, 1036, 1076, 1080, 1088] {
            assert_eq!(step(height), 1080, "{height}");
        }
        assert_eq!(step(1600), 2160);
        assert_eq!(step(2076), 2160);
        for height in [536, 544, 696, 720] {
            assert_eq!(step(height), 720, "{height}");
        }
        // Plain 16:9 heights between tiers go to the nearest.
        assert_eq!(step(576), 480);
        assert_eq!(step(540), 480);
        assert_eq!(step(270), 240);
        // A height fitting no tier as a crop: the nearest.
        assert_eq!(step(530), 480);
        assert_eq!(step(3000), 2160);
    }

    #[test]
    fn pictures_keep_their_own_size_beside_their_tier() {
        let cropped = Picture::Size {
            width: 1920,
            height: 816,
        };
        assert_eq!(cropped.tier(), 1080);
        assert_eq!(cropped.exact().as_deref(), Some("1920×816"));
        let plain = Picture::Size {
            width: 1920,
            height: 1080,
        };
        assert_eq!((plain.tier(), plain.exact()), (1080, None));
        assert_eq!(
            Picture::Size {
                width: 854,
                height: 480
            }
            .exact(),
            None
        );
        let old = Picture::Size {
            width: 640,
            height: 480,
        };
        assert_eq!((old.tier(), old.exact().as_deref()), (480, Some("640×480")));
        let wide = Picture::Size {
            width: 1280,
            height: 800,
        };
        assert_eq!(wide.tier(), 720);
        assert_eq!(Picture::Height(816).tier(), 1080);
        assert_eq!(Picture::Height(816).exact().as_deref(), Some("816p"));
        assert_eq!(Picture::Height(1080).exact(), None);
        // A width alone, or a name, gives the tier and nothing more.
        assert_eq!(Picture::Width(1920).tier(), 1080);
        assert_eq!(Picture::Width(1920).exact(), None);
        assert_eq!(Picture::Tier(2160).exact(), None);
        assert_eq!(Picture::of(0, 0), None);
        assert_eq!(Picture::of(1280, 0), Some(Picture::Width(1280)));
        // Names: a size, a cropped height, a tier's name.
        assert_eq!(label_picture("Film.2010.1920x816.mkv"), Some(cropped));
        assert_eq!(label_picture("816p"), Some(Picture::Height(816)));
        assert_eq!(label_height("816p"), Some(1080));
        assert_eq!(label_picture("Full HD"), Some(Picture::Tier(1080)));
        assert_eq!(label_picture("Episode 3"), None);
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
        // Heights count by their tier: a padded or cropped 1080p is 1080p, not past it or below.
        assert_eq!(
            pick(&[Some(720), Some(1088)], VideoResolution::P1080),
            Some(1088)
        );
        assert_eq!(rank(Some(816), VideoResolution::P1080), (0, 0));
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
                exact: None,
                picture: Some(Picture::Tier(2160)),
                range: Range::Dv,
                codec: Some("HEVC"),
                bit_depth: None,
                audio: Some("DD+ Atmos 5.1".into()),
                size_bytes: Some(15_200_000_000),
                release: Some("WEB-DL"),
            }
        );
        // A cropped film's own size stays beside its tier, from its name or its source.
        let cropped = link_facts(&[Some("Film.2010.1080p.1920x800.mkv")], None, None);
        assert_eq!(
            (cropped.height, cropped.exact.as_deref()),
            (Some(1080), None),
            "the first size named, 1080p, is plain"
        );
        let cropped = link_facts(&[Some("Film 1920x800 x264")], None, Some(1080));
        assert_eq!(
            (cropped.height, cropped.exact.as_deref()),
            (Some(1080), Some("1920×800"))
        );
        let source = link_facts(&[Some("Server 1")], None, Some(816));
        assert_eq!(
            (source.height, source.exact.as_deref()),
            (Some(1080), Some("816p"))
        );
        let plain = link_facts(
            &[
                Some("Show.S01E02.1080p.BluRay.x264.AAC2.0-X"),
                Some("1.4GiB"),
            ],
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
        assert_eq!(
            bitrate_kbps(Some(1_350_000_000), Some(5_400_000)),
            Some(2_000)
        );
        assert_eq!(bitrate_kbps(Some(1), None), None);
    }

    #[test]
    fn heights_fall_on_tiers_and_keys_read_back() {
        assert_eq!(step(1080), 1080);
        assert_eq!(step(1076), 1080);
        assert_eq!(step(576), 480);
        assert_eq!(step(900), 1080);
        assert_eq!(step(100), 144);
        assert_eq!(step(5000), 4320);
        let key = QualityKey::of(2160, Range::Hdr);
        assert_eq!(key.text(), "2160:hdr");
        assert_eq!(QualityKey::parse("2160:hdr"), Some(key));
        assert_eq!(QualityKey::parse("1000:sdr"), None, "only tiers");
        assert_eq!(QualityKey::parse("1080:hdr10"), None);
        // Nearer is fewer tiers apart, then the lower, then the same range.
        let wanted = QualityKey::of(1080, Range::Sdr);
        assert!(
            wanted.distance(QualityKey::of(720, Range::Sdr))
                < wanted.distance(QualityKey::of(1440, Range::Sdr))
        );
        assert!(
            wanted.distance(QualityKey::of(1440, Range::Sdr))
                < wanted.distance(QualityKey::of(480, Range::Sdr))
        );
        assert!(
            wanted.distance(QualityKey::of(1080, Range::Hdr))
                < wanted.distance(QualityKey::of(720, Range::Sdr))
        );
        assert_eq!(wanted.distance(wanted), (0, false, false));
    }

    fn offer(group: usize, link: usize, height: u32, range: Range, rank: usize) -> Offer {
        Offer {
            group,
            link,
            key: QualityKey::of(height, range),
            codec: None,
            picture: Some(Picture::Height(height)),
            rank,
        }
    }

    fn variant(width: u32, height: u32, codec: Option<&str>) -> Variant {
        Variant {
            width,
            height,
            range: Range::Sdr,
            codec: codec.map(str::to_owned),
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
                };
                (row.key.clone(), state)
            })
            .collect()
    }

    fn owned(rows: &[(&str, &str)]) -> Vec<(String, String)> {
        rows.iter()
            .map(|(key, state)| ((*key).to_owned(), (*state).to_owned()))
            .collect()
    }

    #[test]
    fn the_menu_lists_what_the_sources_have_from_the_source_playing_first() {
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
        // No row for a tier nobody has (1440p, 480p and below).
        assert_eq!(
            states(&rows(&menu)),
            owned(&[
                ("2160:dv", "from Beta"),
                ("2160:sdr", "from Beta"),
                ("1080:sdr", "playing"),
                ("720:sdr", "here 0/1"),
            ])
        );
    }

    #[test]
    fn cinema_crops_group_under_their_tier_with_their_own_size() {
        let names = vec!["Alpha".to_owned(), "Beta".to_owned()];
        let mut offers = vec![
            // Alpha plays a 1920×816 film; Beta has 1036 and 800 lines, and a 1600-line 4K.
            offer(0, 0, 816, Range::Sdr, 0),
            offer(1, 0, 1036, Range::Sdr, 1),
            offer(1, 1, 800, Range::Sdr, 2),
            offer(1, 2, 1600, Range::Sdr, 3),
        ];
        offers[0].picture = Some(Picture::Size {
            width: 1920,
            height: 816,
        });
        offers[3].picture = Some(Picture::Size {
            width: 3840,
            height: 1600,
        });
        let menu = Menu {
            offers: &offers,
            names: &names,
            current: Some((0, 0)),
            variants: &[],
            playing: None,
        };
        let rows = rows(&menu);
        assert_eq!(
            states(&rows),
            owned(&[("2160:sdr", "from Beta"), ("1080:sdr", "playing")])
        );
        assert_eq!(rows[0].exact.as_deref(), Some("3840×1600"));
        assert_eq!(rows[1].exact.as_deref(), Some("1920×816"));
        // Tapped again, the 1080p row plays Beta's first 1080p link: 1036 lines.
        let again = super::pick(
            &offers,
            QualityKey::of(1080, Range::Sdr),
            Some((0, 0)),
            &HashSet::new(),
        );
        assert_eq!(again, Some((1, 0)));
    }

    #[test]
    fn the_streams_own_variants_come_before_other_links() {
        let names = vec!["Alpha".to_owned()];
        let offers = [
            offer(0, 0, 1080, Range::Sdr, 0),
            offer(0, 1, 480, Range::Sdr, 1),
        ];
        let variants = [
            variant(1920, 1080, Some("H.264")),
            variant(1280, 720, None),
            variant(854, 480, None),
        ];
        let menu = Menu {
            offers: &offers,
            names: &names,
            current: Some((0, 0)),
            variants: &variants,
            playing: Some(QualityKey::of(720, Range::Sdr)),
        };
        let rows = rows(&menu);
        assert_eq!(
            states(&rows),
            owned(&[
                ("1080:sdr", "variant"),
                ("720:sdr", "playing"),
                ("480:sdr", "variant"),
            ])
        );
        assert_eq!(rows[0].codec.as_deref(), Some("H.264"));
        assert_eq!(
            rows[0].sizes,
            vec![VariantSize {
                width: 1920,
                height: 1080
            }]
        );
        assert_eq!(rows[0].exact, None);
        // Without a session, the variants alone; a cropped variant keeps its size, a variant
        // known only by its width counts by it.
        let variants = [
            variant(1920, 800, None),
            variant(1280, 0, None),
            variant(0, 0, None),
        ];
        let alone = Menu {
            offers: &[],
            names: &[],
            current: None,
            variants: &variants,
            playing: Some(QualityKey::of(1080, Range::Sdr)),
        };
        let rows = super::rows(&alone);
        assert_eq!(
            states(&rows),
            owned(&[("1080:sdr", "playing"), ("720:sdr", "variant")])
        );
        assert_eq!(rows[0].exact.as_deref(), Some("1920×800"));
        assert_eq!(
            rows[1].sizes,
            vec![VariantSize {
                width: 1280,
                height: 0
            }]
        );
    }

    #[test]
    fn a_tier_tapped_again_tries_the_next_source_with_it() {
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
        assert_eq!(super::pick(&[], key, None, &tried), None);
    }

    #[test]
    fn a_tier_no_source_has_plays_the_nearest_from_the_source_playing_first() {
        let offers = [
            offer(0, 0, 2160, Range::Sdr, 0),
            offer(1, 0, 1440, Range::Sdr, 1),
            offer(1, 1, 720, Range::Sdr, 2),
            offer(0, 1, 720, Range::Sdr, 3),
            offer(2, 0, 360, Range::Sdr, 4),
        ];
        let tried = HashSet::new();
        // 1080p: 1440p and 720p are as near; the lower, from the source playing.
        assert_eq!(
            super::pick(
                &offers,
                QualityKey::of(1080, Range::Sdr),
                Some((0, 0)),
                &tried
            ),
            Some((0, 1))
        );
        // Nothing playing: the order a session tries them.
        assert_eq!(
            super::pick(&offers, QualityKey::of(1080, Range::Sdr), None, &tried),
            Some((1, 1))
        );
        // 480p: 360p and 720p are as near; the lower.
        assert_eq!(
            super::pick(
                &offers,
                QualityKey::of(480, Range::Sdr),
                Some((0, 0)),
                &tried
            ),
            Some((2, 0))
        );
        // The row that plays, tapped again with no other link to it: nothing else plays (the
        // player says no other source has it), not another range.
        let alone = [
            offer(0, 0, 1080, Range::Sdr, 0),
            offer(1, 0, 1080, Range::Hdr, 1),
        ];
        assert_eq!(
            super::pick(
                &alone,
                QualityKey::of(1080, Range::Sdr),
                Some((0, 0)),
                &tried
            ),
            None
        );
        // HDR asked for, none has it: the same tier in SDR before another tier.
        assert_eq!(
            super::pick(
                &offers,
                QualityKey::of(1440, Range::Hdr),
                Some((0, 0)),
                &tried
            ),
            Some((1, 0))
        );
    }
}
