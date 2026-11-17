//! HLS playlists for downloads (M43): a master playlist's variant chosen by quality, and a media
//! playlist turned into the files to fetch plus a copy that points at them. Pure; the service
//! does the fetching.

use raygun_model::VideoResolution;
use url::Url;

use crate::video_quality;

/// A file to fetch: a segment, key or init map, saved under `name`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetch {
    pub url: String,
    /// Byte range `(start, length)` of a resource shared by several segments.
    pub range: Option<(u64, u64)>,
    pub name: String,
}

/// A media playlist ready to download: what to fetch and the playlist that plays the saved files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaPlan {
    pub files: Vec<Fetch>,
    pub playlist: String,
}

/// A variant stream of a master playlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    pub url: String,
    pub bandwidth: Option<u64>,
    /// Height in pixels, from `RESOLUTION`.
    pub height: Option<u32>,
    /// The `AUDIO` group it plays with.
    pub audio_group: Option<String>,
}

/// An alternative audio rendition (`#EXT-X-MEDIA:TYPE=AUDIO` with a URI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioRendition {
    pub group: String,
    pub url: String,
    pub default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Master {
    pub variants: Vec<Variant>,
    pub audio: Vec<AudioRendition>,
}

/// Whether a playlist lists variants rather than segments.
pub fn is_master(text: &str) -> bool {
    text.lines()
        .any(|line| line.trim_start().starts_with("#EXT-X-STREAM-INF"))
}

pub fn parse_master(text: &str, base: &str) -> Result<Master, String> {
    let base = Url::parse(base).map_err(|_| "The stream's address is invalid".to_owned())?;
    let mut variants = Vec::new();
    let mut audio = Vec::new();
    let mut pending: Option<Variant> = None;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(attributes) = line.strip_prefix("#EXT-X-STREAM-INF:") {
            pending = Some(Variant {
                url: String::new(),
                bandwidth: attribute(attributes, "BANDWIDTH").and_then(|value| value.parse().ok()),
                height: attribute(attributes, "RESOLUTION")
                    .and_then(|value| video_quality::resolution_height(&value)),
                audio_group: attribute(attributes, "AUDIO"),
            });
        } else if let Some(attributes) = line.strip_prefix("#EXT-X-MEDIA:") {
            let is_audio = attribute(attributes, "TYPE").as_deref() == Some("AUDIO");
            if let (true, Some(group), Some(uri)) = (
                is_audio,
                attribute(attributes, "GROUP-ID"),
                attribute(attributes, "URI"),
            ) {
                audio.push(AudioRendition {
                    group,
                    url: resolve(&base, &uri)?,
                    default: attribute(attributes, "DEFAULT").as_deref() == Some("YES"),
                });
            }
        } else if !line.starts_with('#')
            && let Some(mut variant) = pending.take()
        {
            variant.url = resolve(&base, line)?;
            variants.push(variant);
        }
    }
    if variants.is_empty() {
        return Err("The stream lists no variants".into());
    }
    Ok(Master { variants, audio })
}

/// The variant to download, by the rule links are ordered by (`video_quality::rank`): heights
/// from `RESOLUTION`, else estimated from `BANDWIDTH`; among equals the highest bandwidth.
pub fn choose_variant(variants: &[Variant], preferred: VideoResolution) -> &Variant {
    let key = |variant: &Variant| {
        let height = variant
            .height
            .or_else(|| variant.bandwidth.map(video_quality::bandwidth_height));
        (
            video_quality::rank(height, preferred),
            u64::MAX - variant.bandwidth.unwrap_or(0),
        )
    };
    variants
        .iter()
        .min_by_key(|variant| key(variant))
        .expect("parse_master returns at least one variant")
}

/// The audio rendition a variant plays with, the default one of its group first.
pub fn audio_for<'a>(master: &'a Master, variant: &Variant) -> Option<&'a AudioRendition> {
    let group = variant.audio_group.as_deref()?;
    let renditions = master.audio.iter().filter(|audio| audio.group == group);
    renditions
        .clone()
        .find(|audio| audio.default)
        .or_else(|| renditions.clone().next())
}

/// Plans a media playlist's download: every segment, key and init map gets a local name starting
/// with `prefix`, and the copy refers to those names. Live playlists (no `#EXT-X-ENDLIST`) and
/// SAMPLE-AES encryption are refused.
pub fn plan_media(text: &str, base: &str, prefix: &str) -> Result<MediaPlan, String> {
    let base = Url::parse(base).map_err(|_| "The stream's address is invalid".to_owned())?;
    if !text.trim_start().starts_with("#EXTM3U") {
        return Err("The stream isn't an HLS playlist".into());
    }
    if !text.lines().any(|line| line.trim() == "#EXT-X-ENDLIST") {
        return Err("Live streams can't be downloaded".into());
    }
    let mut files = Vec::new();
    let mut playlist = Vec::new();
    let mut segments = 0usize;
    let mut keys = 0usize;
    let mut maps = 0usize;
    // `#EXT-X-BYTERANGE` before the next segment, and where the last range of a URL ended.
    let mut pending_range: Option<(u64, Option<u64>)> = None;
    let mut range_end: Option<(String, u64)> = None;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(attributes) = line.strip_prefix("#EXT-X-KEY:") {
            let method = attribute(attributes, "METHOD").unwrap_or_default();
            match method.as_str() {
                "NONE" => playlist.push(line.to_owned()),
                "AES-128" => {
                    let uri = attribute(attributes, "URI")
                        .ok_or_else(|| "An encryption key has no address".to_owned())?;
                    keys += 1;
                    let name = format!("{prefix}key{keys}.key");
                    files.push(Fetch {
                        url: resolve(&base, &uri)?,
                        range: None,
                        name: name.clone(),
                    });
                    playlist.push(format!(
                        "#EXT-X-KEY:{}",
                        replace_attribute(attributes, "URI", &name)
                    ));
                }
                _ => return Err("This stream is protected and can't be downloaded".into()),
            }
        } else if let Some(attributes) = line.strip_prefix("#EXT-X-MAP:") {
            let uri = attribute(attributes, "URI")
                .ok_or_else(|| "An init section has no address".to_owned())?;
            let url = resolve(&base, &uri)?;
            let range = attribute(attributes, "BYTERANGE")
                .map(|value| parse_range(&value))
                .transpose()?
                .map(|(length, offset)| (offset.unwrap_or(0), length));
            maps += 1;
            let name = format!("{prefix}init{maps}{}", extension(&url, "mp4"));
            files.push(Fetch {
                url,
                range,
                name: name.clone(),
            });
            playlist.push(format!("#EXT-X-MAP:URI=\"{name}\""));
        } else if let Some(value) = line.strip_prefix("#EXT-X-BYTERANGE:") {
            pending_range = Some(parse_range(value)?);
        } else if line.starts_with('#') {
            playlist.push(line.to_owned());
        } else {
            let url = resolve(&base, line)?;
            let range = pending_range.take().map(|(length, offset)| {
                let start = offset.unwrap_or_else(|| match &range_end {
                    Some((last, end)) if *last == url => *end,
                    _ => 0,
                });
                range_end = Some((url.clone(), start + length));
                (start, length)
            });
            segments += 1;
            let name = format!("{prefix}{segments:05}{}", extension(&url, "ts"));
            files.push(Fetch {
                url,
                range,
                name: name.clone(),
            });
            playlist.push(name);
        }
    }
    if segments == 0 {
        return Err("The stream has no segments".into());
    }
    let mut playlist = playlist.join("\n");
    playlist.push('\n');
    Ok(MediaPlan { files, playlist })
}

/// A local master playlist joining a saved video playlist with its saved audio rendition.
pub fn local_master(variant: &Variant, video: &str, audio: &str) -> String {
    let bandwidth = variant.bandwidth.unwrap_or(1);
    format!(
        "#EXTM3U\n\
         #EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"audio\",NAME=\"Audio\",DEFAULT=YES,AUTOSELECT=YES,URI=\"{audio}\"\n\
         #EXT-X-STREAM-INF:BANDWIDTH={bandwidth},AUDIO=\"audio\"\n\
         {video}\n"
    )
}

/// `<length>[@<offset>]`.
fn parse_range(value: &str) -> Result<(u64, Option<u64>), String> {
    let invalid = || "The stream has an unreadable byte range".to_owned();
    let (length, offset) = match value.trim().trim_matches('"').split_once('@') {
        Some((length, offset)) => (length, Some(offset.parse().map_err(|_| invalid())?)),
        None => (value.trim().trim_matches('"'), None),
    };
    Ok((length.parse().map_err(|_| invalid())?, offset))
}

fn resolve(base: &Url, reference: &str) -> Result<String, String> {
    base.join(reference)
        .map(String::from)
        .map_err(|_| format!("The stream lists an invalid address: {reference}"))
}

/// `.ts`, `.m4s`, `.aac`... from a URL's path, when it's a short plain one; else `.<fallback>`.
fn extension(url: &str, fallback: &str) -> String {
    let path = Url::parse(url)
        .map(|url| url.path().to_owned())
        .unwrap_or_default();
    let name = path.rsplit('/').next().unwrap_or_default();
    match name.rsplit_once('.') {
        Some((_, extension))
            if (1..=5).contains(&extension.len())
                && extension.bytes().all(|byte| byte.is_ascii_alphanumeric()) =>
        {
            format!(".{}", extension.to_ascii_lowercase())
        }
        _ => format!(".{fallback}"),
    }
}

/// An attribute's value from an attribute list (`KEY=value,KEY2="quoted, value"`).
fn attribute(list: &str, name: &str) -> Option<String> {
    attributes(list)
        .into_iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value)
}

fn attributes(list: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut rest = list.trim();
    while !rest.is_empty() {
        let Some((key, after)) = rest.split_once('=') else {
            break;
        };
        let (value, after) = if let Some(quoted) = after.strip_prefix('"') {
            match quoted.split_once('"') {
                Some((value, after)) => (value.to_owned(), after),
                None => (quoted.to_owned(), ""),
            }
        } else {
            match after.split_once(',') {
                Some((value, after)) => (value.to_owned(), after),
                None => (after.to_owned(), ""),
            }
        };
        found.push((key.trim().to_owned(), value));
        rest = after.trim_start_matches(',').trim_start();
    }
    found
}

/// The attribute list with `name`'s value replaced by a quoted `value`.
fn replace_attribute(list: &str, name: &str, value: &str) -> String {
    attributes(list)
        .into_iter()
        .map(|(key, old)| {
            if key == name {
                format!("{key}=\"{value}\"")
            } else if matches!(key.as_str(), "URI" | "KEYFORMAT" | "KEYFORMATVERSIONS") {
                format!("{key}=\"{old}\"")
            } else {
                format!("{key}={old}")
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    const MASTER: &str = "#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aac\",NAME=\"English\",DEFAULT=YES,URI=\"audio/en.m3u8\"
#EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=640x360,AUDIO=\"aac\"
low/index.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2500000,RESOLUTION=1280x720,AUDIO=\"aac\"
mid/index.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=5000000,RESOLUTION=1920x1080,CODECS=\"avc1.640028,mp4a.40.2\"
https://other.example/high.m3u8
";

    #[test]
    fn variants_follow_the_preferred_quality() {
        let master = parse_master(MASTER, "https://cdn.example/show/master.m3u8").unwrap();
        assert_eq!(master.variants.len(), 3);
        assert_eq!(
            master.variants[0].url,
            "https://cdn.example/show/low/index.m3u8"
        );
        let pick = |preferred| choose_variant(&master.variants, preferred).height;
        assert_eq!(pick(VideoResolution::Highest), Some(1080), "the highest");
        assert_eq!(pick(VideoResolution::P2160), Some(1080));
        assert_eq!(pick(VideoResolution::P720), Some(720));
        assert_eq!(pick(VideoResolution::P480), Some(360), "closest below");
        let large = &master.variants[1..];
        assert_eq!(
            choose_variant(large, VideoResolution::P360).height,
            Some(720),
            "nothing below: the lowest above"
        );

        let mid = choose_variant(&master.variants, VideoResolution::P720);
        let audio = audio_for(&master, mid).unwrap();
        assert_eq!(audio.url, "https://cdn.example/show/audio/en.m3u8");
        let high = choose_variant(&master.variants, VideoResolution::Highest);
        assert_eq!(audio_for(&master, high), None, "muxed audio");

        // Without `RESOLUTION`, the bit rates say roughly how large each variant is.
        let by_rate = "#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=6000000
hi.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2400000
mid.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=14000000
top.m3u8
";
        let by_rate = parse_master(by_rate, "https://cdn.example/master.m3u8").unwrap();
        let url = |preferred| choose_variant(&by_rate.variants, preferred).url.clone();
        assert_eq!(url(VideoResolution::P1080), "https://cdn.example/hi.m3u8");
        assert_eq!(url(VideoResolution::P720), "https://cdn.example/mid.m3u8");
        assert_eq!(
            url(VideoResolution::Highest),
            "https://cdn.example/top.m3u8"
        );
        assert!(is_master(MASTER));
    }

    #[test]
    fn media_playlists_point_at_saved_files() {
        let text = "#EXTM3U
#EXT-X-VERSION:7
#EXT-X-TARGETDURATION:6
#EXT-X-MAP:URI=\"init.mp4\",BYTERANGE=\"720@0\"
#EXT-X-KEY:METHOD=AES-128,URI=\"https://keys.example/k?id=1\",IV=0x00000000000000000000000000000001
#EXTINF:6.0,
seg/1.m4s?token=abc
#EXT-X-BYTERANGE:1000@720
#EXTINF:6.0,
whole.mp4
#EXT-X-BYTERANGE:500
#EXTINF:4.5,
whole.mp4
#EXT-X-ENDLIST
";
        let plan = plan_media(text, "https://cdn.example/show/v/index.m3u8", "v").unwrap();
        let names: Vec<_> = plan.files.iter().map(|file| file.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "vinit1.mp4",
                "vkey1.key",
                "v00001.m4s",
                "v00002.mp4",
                "v00003.mp4"
            ]
        );
        assert_eq!(plan.files[0].range, Some((0, 720)));
        assert_eq!(plan.files[1].url, "https://keys.example/k?id=1");
        assert_eq!(
            plan.files[2].url,
            "https://cdn.example/show/v/seg/1.m4s?token=abc"
        );
        assert_eq!(plan.files[3].range, Some((720, 1000)));
        assert_eq!(
            plan.files[4].range,
            Some((1720, 500)),
            "continues the last range"
        );
        assert_eq!(
            plan.playlist,
            "#EXTM3U
#EXT-X-VERSION:7
#EXT-X-TARGETDURATION:6
#EXT-X-MAP:URI=\"vinit1.mp4\"
#EXT-X-KEY:METHOD=AES-128,URI=\"vkey1.key\",IV=0x00000000000000000000000000000001
#EXTINF:6.0,
v00001.m4s
#EXTINF:6.0,
v00002.mp4
#EXTINF:4.5,
v00003.mp4
#EXT-X-ENDLIST
"
        );
    }

    #[test]
    fn live_and_protected_streams_are_refused() {
        let live = "#EXTM3U\n#EXTINF:6,\na.ts\n";
        assert_eq!(
            plan_media(live, "https://cdn.example/a.m3u8", "").unwrap_err(),
            "Live streams can't be downloaded"
        );
        let protected = "#EXTM3U\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"skd://x\"\n#EXTINF:6,\na.ts\n#EXT-X-ENDLIST\n";
        assert!(plan_media(protected, "https://cdn.example/a.m3u8", "").is_err());
        assert!(plan_media("<html>", "https://cdn.example/a.m3u8", "").is_err());
    }

    #[test]
    fn a_local_master_joins_video_and_audio() {
        let variant = Variant {
            url: String::new(),
            bandwidth: Some(2_500_000),
            height: Some(720),
            audio_group: Some("aac".into()),
        };
        let master = local_master(&variant, "video.m3u8", "audio.m3u8");
        assert!(master.contains("URI=\"audio.m3u8\""));
        assert!(master.ends_with("AUDIO=\"audio\"\nvideo.m3u8\n"));
    }
}
