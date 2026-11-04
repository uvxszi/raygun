//! A stream fetched as bytes for a capture (M46.49), through the stream proxy's `/media/` route
//! as the player gets it (the source's headers added, playlists rewritten to stay in the route):
//! HLS a segment at a time (AES-128 decrypted, MPEG-TS demuxed to its sound, fragmented MP4 kept
//! after its init section), a file by ranges. Each part is tried again when it fails, the parts
//! still missing after a pass are fetched in another, and the parts are joined in order.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;

use aes::cipher::block_padding::Pkcs7;
use aes::cipher::{BlockDecryptMut, KeyIvInit};
use futures_util::StreamExt;
use reqwest::header::{CONTENT_RANGE, CONTENT_TYPE, RANGE};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Notify;
use url::Url;

use crate::export::video::{attribute, parse_iv};
use crate::video_downloads::hls;

type Decryptor = cbc::Decryptor<aes::Aes128>;

/// A file is fetched this many bytes at a time.
const CHUNK: u64 = 2 * 1024 * 1024;
/// Waits before each new try of a part.
const RETRIES: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(3),
    Duration::from_secs(8),
];
/// Passes over the parts still missing.
const PASSES: usize = 2;
/// A request that sends nothing for this long has dropped.
const STALL: Duration = Duration::from_secs(30);
/// Biggest stream fetched (a podcast episode can be long).
const MAX_BYTES: u64 = 1024 * 1024 * 1024;

/// What a stream is, from its first bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Hls,
    Dash,
    File {
        length: Option<u64>,
        /// It answers Range requests.
        ranges: bool,
        content_type: String,
    },
}

#[derive(Debug)]
pub enum BytesError {
    Stopped,
    /// It couldn't be written here.
    Device(String),
    Failed(String),
}

impl BytesError {
    fn device(error: std::io::Error) -> Self {
        Self::Device(error.to_string())
    }
}

/// The joined file, and how long its playlist says it is.
pub struct Fetched {
    pub path: PathBuf,
    pub listed_ms: Option<u64>,
}

/// Bytes so far, and the share of the parts done when known.
pub type Progress<'a> = &'a (dyn Fn(u64, Option<f64>) + Send + Sync);

/// What the stream at `link` is.
pub async fn sniff(
    client: &reqwest::Client,
    link: &str,
    stop: &Notify,
) -> Result<Kind, BytesError> {
    let mut last = String::new();
    for wait in std::iter::once(Duration::ZERO).chain(RETRIES) {
        if !wait.is_zero() {
            tokio::select! {
                () = tokio::time::sleep(wait) => {}
                () = stop.notified() => return Err(BytesError::Stopped),
            }
        }
        let request = client.get(link).header(RANGE, "bytes=0-4095");
        let response = tokio::select! {
            response = tokio::time::timeout(STALL, request.send()) => response,
            () = stop.notified() => return Err(BytesError::Stopped),
        };
        let response = match response {
            Ok(Ok(response)) => response,
            Ok(Err(error)) => {
                last = error.to_string();
                continue;
            }
            Err(_) => {
                last = "The stream didn't answer".into();
                continue;
            }
        };
        let status = response.status();
        if !status.is_success() {
            last = format!("The stream answered {status}");
            if busy(status) {
                continue;
            }
            return Err(BytesError::Failed(last));
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let ranges = status == reqwest::StatusCode::PARTIAL_CONTENT;
        let length = if ranges {
            response
                .headers()
                .get(CONTENT_RANGE)
                .and_then(|value| value.to_str().ok())
                .and_then(|range| range.rsplit('/').next())
                .and_then(|total| total.parse::<u64>().ok())
        } else {
            response.content_length()
        };
        let mut head = Vec::new();
        let mut body = response.bytes_stream();
        while head.len() < 4096 {
            match tokio::time::timeout(STALL, body.next()).await {
                Ok(Some(Ok(chunk))) => head.extend_from_slice(&chunk),
                _ => break,
            }
        }
        let text = String::from_utf8_lossy(&head[..head.len().min(4096)]);
        let start = text.trim_start_matches('\u{feff}').trim_start();
        return Ok(
            if start.starts_with("#EXTM3U") || content_type.contains("mpegurl") {
                Kind::Hls
            } else if text.contains("<MPD") || content_type.contains("dash+xml") {
                Kind::Dash
            } else {
                Kind::File {
                    length,
                    ranges,
                    content_type,
                }
            },
        );
    }
    Err(BytesError::Failed(last))
}

fn busy(status: reqwest::StatusCode) -> bool {
    status.is_server_error()
        || status == reqwest::StatusCode::TOO_MANY_REQUESTS
        || status == reqwest::StatusCode::REQUEST_TIMEOUT
}

/// One part (a whole address, or `(start, length)` of it), tried again as it fails.
async fn fetch_part(
    client: &reqwest::Client,
    url: &str,
    range: Option<(u64, u64)>,
    stop: &Notify,
) -> Result<Vec<u8>, BytesError> {
    let mut last = String::new();
    for wait in std::iter::once(Duration::ZERO).chain(RETRIES) {
        if !wait.is_zero() {
            tokio::select! {
                () = tokio::time::sleep(wait) => {}
                () = stop.notified() => return Err(BytesError::Stopped),
            }
        }
        let mut request = client.get(url);
        if let Some((start, length)) = range {
            request = request.header(
                RANGE,
                format!("bytes={start}-{}", start + length.max(1) - 1),
            );
        }
        let fetched = async {
            let response = request
                .send()
                .await
                .map_err(|error| (true, error.to_string()))?;
            let status = response.status();
            if !status.is_success() {
                return Err((busy(status), format!("It answered {status}")));
            }
            if range.is_some() && status != reqwest::StatusCode::PARTIAL_CONTENT {
                return Err((false, "It doesn't send parts of the file".to_owned()));
            }
            response
                .bytes()
                .await
                .map(|bytes| bytes.to_vec())
                .map_err(|error| (true, error.to_string()))
        };
        let result = tokio::select! {
            result = tokio::time::timeout(STALL * 2, fetched) => result,
            () = stop.notified() => return Err(BytesError::Stopped),
        };
        match result {
            Ok(Ok(bytes)) => return Ok(bytes),
            Ok(Err((again, error))) => {
                last = error;
                if !again {
                    break;
                }
            }
            Err(_) => last = "It stalled".into(),
        }
    }
    Err(BytesError::Failed(last))
}

// ---- A file by ranges ----

/// A file through the proxy: by ranges when it answers them, each part tried again, else in one
/// go (started over when it drops). Saved as `capture.<extension>` in `dir`.
pub async fn file(
    client: &reqwest::Client,
    link: &str,
    kind: &Kind,
    dir: &Path,
    progress: Progress<'_>,
    stop: &Notify,
) -> Result<Fetched, BytesError> {
    let Kind::File {
        length,
        ranges,
        content_type,
    } = kind
    else {
        return Err(BytesError::Failed("Not a file".into()));
    };
    if length.is_some_and(|length| length > MAX_BYTES) {
        return Err(BytesError::Failed("It's too big to save".into()));
    }
    let extension = super::super::audio_extension(content_type, link);
    let path = dir.join(format!("capture.{extension}"));
    match (length, ranges) {
        (Some(length), true) if *length > 0 => {
            ranged(client, link, *length, &path, progress, stop).await?
        }
        _ => whole(client, link, &path, progress, stop).await?,
    }
    Ok(Fetched {
        path,
        listed_ms: None,
    })
}

async fn ranged(
    client: &reqwest::Client,
    link: &str,
    length: u64,
    path: &Path,
    progress: Progress<'_>,
    stop: &Notify,
) -> Result<(), BytesError> {
    let mut file = tokio::fs::File::create(path)
        .await
        .map_err(BytesError::device)?;
    file.set_len(length).await.map_err(BytesError::device)?;
    let parts = length.div_ceil(CHUNK);
    let mut done = vec![false; parts as usize];
    let mut saved = 0u64;
    let mut last = String::new();
    for _ in 0..PASSES {
        for index in 0..parts {
            if done[index as usize] {
                continue;
            }
            let start = index * CHUNK;
            let size = CHUNK.min(length - start);
            match fetch_part(client, link, Some((start, size)), stop).await {
                Ok(bytes) if bytes.len() as u64 == size => {
                    file.seek(std::io::SeekFrom::Start(start))
                        .await
                        .map_err(BytesError::device)?;
                    file.write_all(&bytes).await.map_err(BytesError::device)?;
                    done[index as usize] = true;
                    saved += size;
                    progress(saved, Some(saved as f64 / length as f64));
                }
                Ok(bytes) => last = format!("A part came with {} of {size} bytes", bytes.len()),
                Err(BytesError::Failed(error)) => last = error,
                Err(other) => return Err(other),
            }
        }
        if done.iter().all(|part| *part) {
            file.flush().await.map_err(BytesError::device)?;
            return Ok(());
        }
    }
    let missing = done.iter().filter(|part| !**part).count();
    Err(BytesError::Failed(format!(
        "{missing} of {parts} parts of the file wouldn't download ({last})"
    )))
}

async fn whole(
    client: &reqwest::Client,
    link: &str,
    path: &Path,
    progress: Progress<'_>,
    stop: &Notify,
) -> Result<(), BytesError> {
    let mut last = String::new();
    for wait in std::iter::once(Duration::ZERO).chain(RETRIES) {
        if !wait.is_zero() {
            tokio::select! {
                () = tokio::time::sleep(wait) => {}
                () = stop.notified() => return Err(BytesError::Stopped),
            }
        }
        let response = tokio::select! {
            response = tokio::time::timeout(STALL, client.get(link).send()) => response,
            () = stop.notified() => return Err(BytesError::Stopped),
        };
        let response = match response {
            Ok(Ok(response)) if response.status().is_success() => response,
            Ok(Ok(response)) => {
                last = format!("It answered {}", response.status());
                if busy(response.status()) {
                    continue;
                }
                break;
            }
            Ok(Err(error)) => {
                last = error.to_string();
                continue;
            }
            Err(_) => {
                last = "It didn't answer".into();
                continue;
            }
        };
        let total = response.content_length();
        let mut file = tokio::fs::File::create(path)
            .await
            .map_err(BytesError::device)?;
        let mut body = response.bytes_stream();
        let mut saved = 0u64;
        let finished = loop {
            let next = tokio::select! {
                next = tokio::time::timeout(STALL, body.next()) => next,
                () = stop.notified() => return Err(BytesError::Stopped),
            };
            match next {
                Ok(Some(Ok(chunk))) => {
                    file.write_all(&chunk).await.map_err(BytesError::device)?;
                    saved += chunk.len() as u64;
                    if saved > MAX_BYTES {
                        return Err(BytesError::Failed("It's too big to save".into()));
                    }
                    progress(
                        saved,
                        total
                            .filter(|total| *total > 0)
                            .map(|total| saved as f64 / total as f64),
                    );
                }
                Ok(None) => break true,
                Ok(Some(Err(error))) => {
                    last = error.to_string();
                    break false;
                }
                Err(_) => {
                    last = "It stalled".into();
                    break false;
                }
            }
        };
        if finished && total.is_none_or(|total| total == saved) {
            file.flush().await.map_err(BytesError::device)?;
            return Ok(());
        }
        if finished {
            last = format!("It ended after {saved} bytes");
        }
    }
    Err(BytesError::Failed(last))
}

// ---- HLS ----

#[derive(Debug, Clone, PartialEq, Eq)]
struct Segment {
    url: String,
    range: Option<(u64, u64)>,
    duration_ms: u64,
    /// The key's address and the IV, for AES-128.
    key: Option<(String, [u8; 16])>,
    /// The init section (fragmented MP4) and its range.
    map: Option<(String, Option<(u64, u64)>)>,
}

/// An HLS stream's sound, segment by segment, joined into `capture.<aac|mp3|m4a>` in `dir`.
pub async fn hls(
    client: &reqwest::Client,
    link: &str,
    dir: &Path,
    progress: Progress<'_>,
    stop: &Notify,
) -> Result<Fetched, BytesError> {
    let text = playlist(client, link, stop).await?;
    let (text, base) = if hls::is_master(&text) {
        let master = hls::parse_master(&text, link).map_err(BytesError::Failed)?;
        let playlist_url = best_sound(&master)
            .ok_or_else(|| BytesError::Failed("The stream lists nothing to play".into()))?;
        (playlist(client, &playlist_url, stop).await?, playlist_url)
    } else {
        (text, link.to_owned())
    };
    let (segments, ended) = parse_media(&text, &base).map_err(BytesError::Failed)?;
    if !ended {
        return Err(BytesError::Failed(
            "It's a live stream, which has no end to save".into(),
        ));
    }
    let listed_ms: u64 = segments.iter().map(|segment| segment.duration_ms).sum();
    let parts = dir.join("parts");
    tokio::fs::create_dir_all(&parts)
        .await
        .map_err(BytesError::device)?;
    let mut keys: HashMap<String, [u8; 16]> = HashMap::new();
    let mut inits: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut done = vec![None::<&'static str>; segments.len()];
    let mut saved = 0u64;
    let mut last = String::new();
    for _ in 0..PASSES {
        for (index, segment) in segments.iter().enumerate() {
            if done[index].is_some() {
                continue;
            }
            match segment_sound(client, segment, &mut keys, &mut inits, stop).await {
                Ok((bytes, extension)) => {
                    tokio::fs::write(parts.join(index.to_string()), &bytes)
                        .await
                        .map_err(BytesError::device)?;
                    saved += bytes.len() as u64;
                    done[index] = Some(extension);
                    let finished = done.iter().filter(|part| part.is_some()).count();
                    progress(saved, Some(finished as f64 / segments.len() as f64));
                }
                Err(BytesError::Failed(error)) => last = error,
                Err(other) => return Err(other),
            }
        }
        if done.iter().all(Option::is_some) {
            break;
        }
    }
    let missing = done.iter().filter(|part| part.is_none()).count();
    if missing > 0 {
        let _ = tokio::fs::remove_dir_all(&parts).await;
        return Err(BytesError::Failed(format!(
            "{missing} of {} pieces wouldn't download ({last})",
            segments.len()
        )));
    }
    // Every piece must hold the same kind of sound to be joined.
    let extension = done[0].unwrap_or("aac");
    if done.iter().any(|part| *part != Some(extension)) {
        let _ = tokio::fs::remove_dir_all(&parts).await;
        return Err(BytesError::Failed(
            "Its pieces hold different kinds of sound".into(),
        ));
    }
    let path = dir.join(format!("capture.{extension}"));
    let mut out = tokio::fs::File::create(&path)
        .await
        .map_err(BytesError::device)?;
    if extension == "m4a"
        && let Some(init) = inits.values().next()
    {
        out.write_all(init).await.map_err(BytesError::device)?;
    }
    for index in 0..segments.len() {
        let bytes = tokio::fs::read(parts.join(index.to_string()))
            .await
            .map_err(BytesError::device)?;
        out.write_all(&bytes).await.map_err(BytesError::device)?;
    }
    out.flush().await.map_err(BytesError::device)?;
    let _ = tokio::fs::remove_dir_all(&parts).await;
    Ok(Fetched {
        path,
        listed_ms: (listed_ms > 0).then_some(listed_ms),
    })
}

async fn playlist(
    client: &reqwest::Client,
    url: &str,
    stop: &Notify,
) -> Result<String, BytesError> {
    let bytes = fetch_part(client, url, None, stop).await?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// The playlist with the best sound: the best audio-only variant, else the audio rendition of
/// the lightest variant, else the lightest variant (its sound is the same as the others').
fn best_sound(master: &hls::Master) -> Option<String> {
    let audio_only = master
        .variants
        .iter()
        .filter(|variant| variant.height.is_none())
        .max_by_key(|variant| variant.bandwidth.unwrap_or(0));
    if let Some(variant) = audio_only {
        return Some(
            hls::audio_for(master, variant)
                .map(|audio| audio.url.clone())
                .unwrap_or_else(|| variant.url.clone()),
        );
    }
    let lightest = master
        .variants
        .iter()
        .min_by_key(|variant| variant.bandwidth.unwrap_or(u64::MAX))?;
    Some(
        hls::audio_for(master, lightest)
            .map(|audio| audio.url.clone())
            .unwrap_or_else(|| lightest.url.clone()),
    )
}

/// One segment's sound: fetched, decrypted, and as joinable bytes with their kind.
async fn segment_sound(
    client: &reqwest::Client,
    segment: &Segment,
    keys: &mut HashMap<String, [u8; 16]>,
    inits: &mut BTreeMap<String, Vec<u8>>,
    stop: &Notify,
) -> Result<(Vec<u8>, &'static str), BytesError> {
    let mut data = fetch_part(client, &segment.url, segment.range, stop).await?;
    if let Some((key_url, iv)) = &segment.key {
        let key = match keys.get(key_url) {
            Some(key) => *key,
            None => {
                let bytes = fetch_part(client, key_url, None, stop).await?;
                let key: [u8; 16] = bytes
                    .get(..16)
                    .and_then(|bytes| bytes.try_into().ok())
                    .ok_or_else(|| BytesError::Failed("The stream's key is unreadable".into()))?;
                keys.insert(key_url.clone(), key);
                key
            }
        };
        let length = Decryptor::new(&key.into(), &(*iv).into())
            .decrypt_padded_mut::<Pkcs7>(&mut data)
            .map_err(|_| BytesError::Failed("A piece couldn't be decrypted".into()))?
            .len();
        data.truncate(length);
    }
    if let Some((map_url, range)) = &segment.map {
        if !inits.contains_key(map_url) {
            let init = fetch_part(client, map_url, *range, stop).await?;
            if inits.is_empty() {
                inits.insert(map_url.clone(), init);
            } else {
                return Err(BytesError::Failed("Its pieces change format midway".into()));
            }
        }
        return Ok((data, "m4a"));
    }
    sound_of(data).map_err(BytesError::Failed)
}

/// A packed segment's sound: MPEG-TS demuxed, an ID3 tag (packed audio's timestamp) dropped.
fn sound_of(data: Vec<u8>) -> Result<(Vec<u8>, &'static str), String> {
    let data = without_id3(data);
    if data.first() == Some(&0x47) {
        return crate::captions::video_audio::ts_audio(&data)
            .ok_or_else(|| "A piece has no sound Raygun can read".to_owned());
    }
    match data.as_slice() {
        [0xFF, second, ..] if second & 0xF6 == 0xF0 => Ok((data, "aac")),
        [0xFF, second, ..] if second & 0xE0 == 0xE0 => Ok((data, "mp3")),
        _ => Err("A piece has no sound Raygun can read".into()),
    }
}

/// `data` without a leading ID3v2 tag.
fn without_id3(data: Vec<u8>) -> Vec<u8> {
    if data.len() < 10 || &data[..3] != b"ID3" {
        return data;
    }
    let size = data[6..10]
        .iter()
        .fold(0usize, |size, byte| (size << 7) | usize::from(byte & 0x7F));
    let footer = if data[5] & 0x10 != 0 { 10 } else { 0 };
    let end = (10 + size + footer).min(data.len());
    data[end..].to_vec()
}

/// A media playlist's segments with their lengths, keys, init sections and ranges, and whether
/// it ends (`#EXT-X-ENDLIST`).
fn parse_media(text: &str, base: &str) -> Result<(Vec<Segment>, bool), String> {
    let base = Url::parse(base).map_err(|_| "The stream's address is invalid".to_owned())?;
    let resolve = |reference: &str| {
        base.join(reference)
            .map(|url| url.to_string())
            .map_err(|_| "The stream lists an invalid address".to_owned())
    };
    let mut segments = Vec::new();
    let mut sequence: u128 = 0;
    let mut duration_ms = 0u64;
    let mut key: Option<(String, Option<[u8; 16]>)> = None;
    let mut map: Option<(String, Option<(u64, u64)>)> = None;
    let mut pending_range: Option<(u64, Option<u64>)> = None;
    let mut range_end: Option<(String, u64)> = None;
    let mut ended = false;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(value) = line.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
            sequence = value.trim().parse().unwrap_or(0);
        } else if let Some(value) = line.strip_prefix("#EXTINF:") {
            let seconds: f64 = value
                .split(',')
                .next()
                .and_then(|seconds| seconds.trim().parse().ok())
                .unwrap_or(0.0);
            duration_ms = (seconds * 1000.0).round() as u64;
        } else if let Some(attributes) = line.strip_prefix("#EXT-X-KEY:") {
            key = match attribute(attributes, "METHOD").as_deref() {
                Some("AES-128") => {
                    let uri = attribute(attributes, "URI").ok_or("An encryption key is missing")?;
                    let iv = attribute(attributes, "IV")
                        .map(|iv| parse_iv(&iv))
                        .transpose()?;
                    Some((resolve(&uri)?, iv))
                }
                Some("NONE") | None => None,
                Some(_) => return Err("The stream is protected (DRM), so it can't be saved".into()),
            };
        } else if let Some(attributes) = line.strip_prefix("#EXT-X-MAP:") {
            let uri = attribute(attributes, "URI").ok_or("An init section is missing")?;
            let range = attribute(attributes, "BYTERANGE")
                .and_then(|value| parse_byterange(&value))
                .map(|(length, offset)| (offset.unwrap_or(0), length));
            map = Some((resolve(&uri)?, range));
        } else if let Some(value) = line.strip_prefix("#EXT-X-BYTERANGE:") {
            pending_range = parse_byterange(value);
        } else if line == "#EXT-X-ENDLIST" {
            ended = true;
        } else if !line.starts_with('#') {
            let url = resolve(line)?;
            let range = pending_range.take().map(|(length, offset)| {
                let start = offset.unwrap_or_else(|| match &range_end {
                    Some((last, end)) if *last == url => *end,
                    _ => 0,
                });
                range_end = Some((url.clone(), start + length));
                (start, length)
            });
            segments.push(Segment {
                url,
                range,
                duration_ms,
                key: key
                    .as_ref()
                    .map(|(uri, iv)| (uri.clone(), iv.unwrap_or_else(|| sequence.to_be_bytes()))),
                map: map.clone(),
            });
            duration_ms = 0;
            sequence += 1;
        }
    }
    if segments.is_empty() {
        return Err("The stream has no pieces".into());
    }
    Ok((segments, ended))
}

/// `<length>[@<offset>]`.
fn parse_byterange(value: &str) -> Option<(u64, Option<u64>)> {
    let (length, offset) = match value.trim().split_once('@') {
        Some((length, offset)) => (length, Some(offset.parse().ok()?)),
        None => (value.trim(), None),
    };
    Some((length.parse().ok()?, offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playlists_give_their_pieces_keys_and_ranges() {
        let text = "#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:5\n#EXT-X-KEY:METHOD=AES-128,URI=\"key\"\n\
                    #EXTINF:4.0,\n#EXT-X-BYTERANGE:100@0\na.ts\n#EXTINF:2.5,\n\
                    #EXT-X-BYTERANGE:50\na.ts\n#EXT-X-ENDLIST\n";
        let (segments, ended) =
            parse_media(text, "http://127.0.0.1:1/media/t/x/list.m3u8").unwrap();
        assert!(ended);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].url, "http://127.0.0.1:1/media/t/x/a.ts");
        assert_eq!(segments[0].range, Some((0, 100)));
        assert_eq!(
            segments[1].range,
            Some((100, 50)),
            "a range goes on from the last"
        );
        assert_eq!(segments[1].duration_ms, 2_500);
        let (key, iv) = segments[1].key.clone().unwrap();
        assert_eq!(key, "http://127.0.0.1:1/media/t/x/key");
        assert_eq!(iv, 6u128.to_be_bytes(), "the IV is the sequence number");
        assert!(
            parse_media(
                "#EXTM3U\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"k\"\n",
                "http://a/b"
            )
            .unwrap_err()
            .contains("protected")
        );
        let (_, live) = parse_media("#EXTM3U\n#EXTINF:4,\na.ts\n", "http://a/b").unwrap();
        assert!(!live);
    }

    #[test]
    fn packed_pieces_give_their_sound() {
        let adts = vec![0xFF, 0xF1, 0x50, 0x80, 0x01, 0xFF, 0xFC];
        let mut tagged = b"ID3\x04\x00\x00\x00\x00\x00\x03abc".to_vec();
        tagged.extend_from_slice(&adts);
        assert_eq!(sound_of(tagged).unwrap(), (adts.clone(), "aac"));
        assert_eq!(sound_of(vec![0xFF, 0xFB, 0x90, 0x00]).unwrap().1, "mp3");
        assert!(sound_of(b"<html>".to_vec()).is_err());
    }
}
