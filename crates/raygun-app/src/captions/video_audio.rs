//! A video's sound for AI subtitles (M46.33). It's read from the address the player plays (the
//! stream proxy's link, which adds the source's headers, or its `/local/` link for downloads):
//! - a file is read with HTTP ranges and decoded by symphonia (MP4, Matroska/WebM, MP3, AAC…);
//! - HLS is read a segment at a time: MPEG-TS segments give up their AAC or MP3 stream through a
//!   small demuxer, fragmented MP4 ones are decoded after their init section, AES-128 ones are
//!   decrypted first.
//!
//! Blocking: call from `spawn_blocking`.

use std::collections::HashMap;
use std::io::{Cursor, Read, Seek, SeekFrom};

use aes::cipher::block_padding::Pkcs7;
use aes::cipher::{BlockDecryptMut, KeyIvInit};
use url::Url;

use super::audio::{AudioReader, decode_all};
use crate::export::video::{attribute, parse_iv};
use crate::video_downloads::hls;

type Decryptor = cbc::Decryptor<aes::Aes128>;

/// Bytes fetched at a time when reading a file by ranges.
const CHUNK: u64 = 1 << 20;
const SAMPLES_PER_MS: u64 = raygun_ai::speech::SAMPLE_RATE as u64 / 1000;
/// Decoded HLS segments kept, since windows overlap.
const KEPT_SEGMENTS: usize = 4;

pub enum VideoAudio {
    File(AudioReader),
    Hls(Box<Hls>),
}

impl VideoAudio {
    /// Opens what plays at `url`; `format` is the player's (`hls`, `dash`…) when it knows.
    pub fn open(client: reqwest::Client, url: &str, format: Option<&str>) -> Result<Self, String> {
        let hls_by_name = Url::parse(url)
            .map(|parsed| parsed.path().to_ascii_lowercase().ends_with(".m3u8"))
            .unwrap_or(false);
        if format == Some("dash") {
            return Err("AI subtitles can't read DASH streams yet".into());
        }
        if format == Some("hls") || hls_by_name {
            return Ok(Self::Hls(Box::new(Hls::open(client, url)?)));
        }
        let source = RangeSource::open(client.clone(), url)?;
        if source.starts_with(b"#EXTM3U") {
            return Ok(Self::Hls(Box::new(Hls::open(client, url)?)));
        }
        let extension = extension_of(url);
        let address = url.to_owned();
        let reader = AudioReader::from_opener(Box::new(move || {
            let source = RangeSource::open(client.clone(), &address)?;
            crate::music_downloads::transcode::open_source(Box::new(source), extension.as_deref())
        }))?;
        Ok(Self::File(reader))
    }

    pub fn duration_ms(&self) -> Option<u64> {
        match self {
            Self::File(reader) => reader.duration_ms(),
            Self::Hls(hls) => (hls.total_ms > 0 && hls.ended).then_some(hls.total_ms),
        }
    }

    /// Up to `len_ms` of 16 kHz mono from `from_ms`; empty past the end.
    pub fn window(&mut self, from_ms: u64, len_ms: u64) -> Result<Vec<f32>, String> {
        match self {
            Self::File(reader) => reader.window(from_ms, len_ms),
            Self::Hls(hls) => hls.window(from_ms, len_ms),
        }
    }
}

fn extension_of(url: &str) -> Option<String> {
    let path = Url::parse(url).ok()?.path().to_owned();
    let name = path.rsplit('/').next()?;
    let (_, extension) = name.rsplit_once('.')?;
    (extension.len() <= 5).then(|| extension.to_ascii_lowercase())
}

fn fetch(
    client: &reqwest::Client,
    url: &str,
    range: Option<(u64, u64)>,
) -> Result<Vec<u8>, String> {
    let mut request = client.get(url);
    if let Some((start, length)) = range {
        request = request.header(
            reqwest::header::RANGE,
            format!("bytes={start}-{}", start + length.max(1) - 1),
        );
    }
    tauri::async_runtime::block_on(async move {
        let response = request
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|error| format!("Couldn't get the video's sound: {error}"))?;
        response
            .bytes()
            .await
            .map(|bytes| bytes.to_vec())
            .map_err(|error| format!("Couldn't get the video's sound: {error}"))
    })
}

// ---- A file read by ranges ----

/// A web file as a seekable source: 1 MiB is fetched at a time around where symphonia reads.
pub struct RangeSource {
    client: reqwest::Client,
    url: String,
    len: Option<u64>,
    pos: u64,
    buf_start: u64,
    buf: Vec<u8>,
}

impl RangeSource {
    pub fn open(client: reqwest::Client, url: &str) -> Result<Self, String> {
        let mut source = Self {
            client,
            url: url.to_owned(),
            len: None,
            pos: 0,
            buf_start: 0,
            buf: Vec::new(),
        };
        source.fill(0).map_err(|error| error.to_string())?;
        Ok(source)
    }

    fn starts_with(&self, prefix: &[u8]) -> bool {
        self.buf_start == 0 && self.buf.starts_with(prefix)
    }

    fn fill(&mut self, at: u64) -> std::io::Result<()> {
        let request = self.client.get(&self.url).header(
            reqwest::header::RANGE,
            format!("bytes={at}-{}", at + CHUNK - 1),
        );
        let (status, total, body) = tauri::async_runtime::block_on(async move {
            let response = request.send().await.map_err(std::io::Error::other)?;
            let status = response.status().as_u16();
            let total = response
                .headers()
                .get(reqwest::header::CONTENT_RANGE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.rsplit_once('/'))
                .and_then(|(_, total)| total.parse::<u64>().ok());
            let body = if status == 416 {
                Vec::new()
            } else {
                response
                    .bytes()
                    .await
                    .map_err(std::io::Error::other)?
                    .to_vec()
            };
            Ok::<_, std::io::Error>((status, total, body))
        })?;
        match status {
            206 => {
                if total.is_some() {
                    self.len = total;
                }
                self.buf_start = at;
                self.buf = body;
            }
            // The server sent everything: keep it all.
            200 => {
                self.len = Some(body.len() as u64);
                self.buf_start = 0;
                self.buf = body;
            }
            416 => {
                self.len = Some(self.len.unwrap_or(at).min(at));
                self.buf_start = at;
                self.buf.clear();
            }
            other => {
                return Err(std::io::Error::other(format!("the video answered {other}")));
            }
        }
        Ok(())
    }
}

impl Read for RangeSource {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.len.is_some_and(|len| self.pos >= len) {
            return Ok(0);
        }
        let end = self.buf_start + self.buf.len() as u64;
        if self.pos < self.buf_start || self.pos >= end {
            self.fill(self.pos)?;
        }
        let end = self.buf_start + self.buf.len() as u64;
        if self.pos < self.buf_start || self.pos >= end {
            return Ok(0);
        }
        let offset = (self.pos - self.buf_start) as usize;
        let count = out.len().min(self.buf.len() - offset);
        out[..count].copy_from_slice(&self.buf[offset..offset + count]);
        self.pos += count as u64;
        Ok(count)
    }
}

impl Seek for RangeSource {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        let next = match to {
            SeekFrom::Start(at) => at as i128,
            SeekFrom::Current(by) => self.pos as i128 + by as i128,
            SeekFrom::End(by) => {
                let len = self
                    .len
                    .ok_or_else(|| std::io::Error::other("the video's length is unknown"))?;
                len as i128 + by as i128
            }
        };
        if next < 0 {
            return Err(std::io::Error::other("seek before the start"));
        }
        self.pos = next as u64;
        Ok(self.pos)
    }
}

impl symphonia::core::io::MediaSource for RangeSource {
    fn is_seekable(&self) -> bool {
        self.len.is_some()
    }

    fn byte_len(&self) -> Option<u64> {
        self.len
    }
}

// ---- HLS ----

#[derive(Debug, Clone, PartialEq, Eq)]
struct Segment {
    start_ms: u64,
    duration_ms: u64,
    url: String,
    range: Option<(u64, u64)>,
    /// The key's address and the IV, for AES-128.
    key: Option<(String, [u8; 16])>,
    /// The init section (fragmented MP4) and its range.
    map: Option<(String, Option<(u64, u64)>)>,
}

pub struct Hls {
    client: reqwest::Client,
    segments: Vec<Segment>,
    total_ms: u64,
    ended: bool,
    keys: HashMap<String, [u8; 16]>,
    maps: HashMap<String, Vec<u8>>,
    decoded: Vec<(usize, Vec<f32>)>,
}

impl Hls {
    fn open(client: reqwest::Client, url: &str) -> Result<Self, String> {
        let text = String::from_utf8_lossy(&fetch(&client, url, None)?).into_owned();
        let (text, base) = if hls::is_master(&text) {
            let master = hls::parse_master(&text, url)?;
            // The sound is the same in every variant: the lightest one, or its audio rendition.
            let variant = master
                .variants
                .iter()
                .min_by_key(|variant| variant.bandwidth.unwrap_or(u64::MAX))
                .ok_or("The stream lists no variants")?;
            let playlist = hls::audio_for(&master, variant)
                .map(|audio| audio.url.clone())
                .unwrap_or_else(|| variant.url.clone());
            let text = String::from_utf8_lossy(&fetch(&client, &playlist, None)?).into_owned();
            (text, playlist)
        } else {
            (text, url.to_owned())
        };
        let (segments, ended) = parse_media(&text, &base)?;
        let total_ms = segments
            .last()
            .map_or(0, |last| last.start_ms + last.duration_ms);
        Ok(Self {
            client,
            segments,
            total_ms,
            ended,
            keys: HashMap::new(),
            maps: HashMap::new(),
            decoded: Vec::new(),
        })
    }

    fn window(&mut self, from_ms: u64, len_ms: u64) -> Result<Vec<f32>, String> {
        let end_ms = from_ms + len_ms;
        let wanted: Vec<usize> = self
            .segments
            .iter()
            .enumerate()
            .filter(|(_, segment)| {
                segment.start_ms < end_ms && segment.start_ms + segment.duration_ms > from_ms
            })
            .map(|(index, _)| index)
            .collect();
        let Some(&first) = wanted.first() else {
            return Ok(Vec::new());
        };
        let mut samples = Vec::new();
        for index in wanted {
            samples.extend(self.segment_samples(index)?);
        }
        let skip =
            ((from_ms.saturating_sub(self.segments[first].start_ms)) * SAMPLES_PER_MS) as usize;
        let take = (len_ms * SAMPLES_PER_MS) as usize;
        Ok(samples.into_iter().skip(skip).take(take).collect())
    }

    fn segment_samples(&mut self, index: usize) -> Result<Vec<f32>, String> {
        if let Some((_, samples)) = self.decoded.iter().find(|(kept, _)| *kept == index) {
            return Ok(samples.clone());
        }
        let segment = self.segments[index].clone();
        let mut data = fetch(&self.client, &segment.url, segment.range)?;
        if let Some((key_url, iv)) = &segment.key {
            let key = match self.keys.get(key_url) {
                Some(key) => *key,
                None => {
                    let bytes = fetch(&self.client, key_url, None)?;
                    let key: [u8; 16] = bytes
                        .get(..16)
                        .and_then(|bytes| bytes.try_into().ok())
                        .ok_or("The stream's key is unreadable")?;
                    self.keys.insert(key_url.clone(), key);
                    key
                }
            };
            let length = Decryptor::new(&key.into(), &(*iv).into())
                .decrypt_padded_mut::<Pkcs7>(&mut data)
                .map_err(|_| "A segment couldn't be decrypted".to_owned())?
                .len();
            data.truncate(length);
        }
        let (bytes, extension) = if data.first() == Some(&0x47) {
            ts_audio(&data).ok_or("A segment has no sound the subtitles can read")?
        } else if let Some((map_url, range)) = &segment.map {
            let init = match self.maps.get(map_url) {
                Some(init) => init.clone(),
                None => {
                    let init = fetch(&self.client, map_url, *range)?;
                    self.maps.insert(map_url.clone(), init.clone());
                    init
                }
            };
            ([init, data].concat(), "mp4")
        } else if data.len() > 1 && data[0] == 0xFF && data[1] & 0xF6 == 0xF0 {
            (data, "aac")
        } else {
            (data, "mp4")
        };
        let samples = decode_all(Box::new(Cursor::new(bytes)), Some(extension))?;
        self.decoded.push((index, samples.clone()));
        if self.decoded.len() > KEPT_SEGMENTS {
            self.decoded.remove(0);
        }
        Ok(samples)
    }
}

/// A media playlist's segments with their times (`#EXTINF`), keys, init sections and ranges, and
/// whether it ends (`#EXT-X-ENDLIST`; a live one doesn't).
fn parse_media(text: &str, base: &str) -> Result<(Vec<Segment>, bool), String> {
    let base = Url::parse(base).map_err(|_| "The stream's address is invalid".to_owned())?;
    let resolve = |reference: &str| {
        base.join(reference)
            .map(|url| url.to_string())
            .map_err(|_| "The stream lists an invalid address".to_owned())
    };
    let mut segments = Vec::new();
    let mut sequence: u128 = 0;
    let mut start_ms = 0u64;
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
                Some(_) => return Err("This video is protected; AI subtitles can't hear it".into()),
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
                start_ms,
                duration_ms,
                url,
                range,
                key: key
                    .as_ref()
                    .map(|(uri, iv)| (uri.clone(), iv.unwrap_or_else(|| sequence.to_be_bytes()))),
                map: map.clone(),
            });
            start_ms += duration_ms;
            duration_ms = 0;
            sequence += 1;
        }
    }
    if segments.is_empty() {
        return Err("The stream has no segments".into());
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

// ---- MPEG-TS ----

/// The sound inside MPEG-TS packets: the first AAC (ADTS) or MP3 stream the program map lists,
/// its PES payloads joined, with the extension symphonia knows it by.
pub fn ts_audio(data: &[u8]) -> Option<(Vec<u8>, &'static str)> {
    let mut pmt_pid: Option<u16> = None;
    let mut audio: Option<(u16, &'static str)> = None;
    let mut out = Vec::new();
    for packet in data.as_chunks::<188>().0 {
        if packet[0] != 0x47 {
            continue;
        }
        let unit_start = packet[1] & 0x40 != 0;
        let pid = (u16::from(packet[1] & 0x1F) << 8) | u16::from(packet[2]);
        let adaptation = (packet[3] >> 4) & 0x3;
        if adaptation == 2 {
            continue;
        }
        let offset = if adaptation == 3 {
            5 + usize::from(packet[4])
        } else {
            4
        };
        if offset >= packet.len() {
            continue;
        }
        let payload = &packet[offset..];
        if pid == 0 && unit_start {
            pmt_pid = pmt_pid.or_else(|| program_map_pid(payload));
        } else if Some(pid) == pmt_pid && unit_start && audio.is_none() {
            audio = audio_stream(payload);
        } else if audio.is_some_and(|(audio_pid, _)| audio_pid == pid) {
            if unit_start {
                if payload.len() >= 9 && payload[..3] == [0, 0, 1] {
                    let start = 9 + usize::from(payload[8]);
                    if start <= payload.len() {
                        out.extend_from_slice(&payload[start..]);
                    }
                }
            } else {
                out.extend_from_slice(payload);
            }
        }
    }
    let (_, extension) = audio?;
    (!out.is_empty()).then_some((out, extension))
}

/// The program association table's first program's map PID.
fn program_map_pid(payload: &[u8]) -> Option<u16> {
    let table = payload.get(1 + usize::from(*payload.first()?)..)?;
    if *table.first()? != 0x00 {
        return None;
    }
    let section_length = (usize::from(table[1] & 0x0F) << 8) | usize::from(table[2]);
    let end = (3 + section_length).saturating_sub(4).min(table.len());
    let mut at = 8;
    while at + 4 <= end {
        let program = (u16::from(table[at]) << 8) | u16::from(table[at + 1]);
        let pid = (u16::from(table[at + 2] & 0x1F) << 8) | u16::from(table[at + 3]);
        if program != 0 {
            return Some(pid);
        }
        at += 4;
    }
    None
}

/// The program map's first audio stream: AAC in ADTS (0x0F) or MPEG audio (0x03, 0x04).
fn audio_stream(payload: &[u8]) -> Option<(u16, &'static str)> {
    let table = payload.get(1 + usize::from(*payload.first()?)..)?;
    if *table.first()? != 0x02 || table.len() < 12 {
        return None;
    }
    let section_length = (usize::from(table[1] & 0x0F) << 8) | usize::from(table[2]);
    let end = (3 + section_length).saturating_sub(4).min(table.len());
    let info_length = (usize::from(table[10] & 0x0F) << 8) | usize::from(table[11]);
    let mut at = 12 + info_length;
    while at + 5 <= end {
        let stream_type = table[at];
        let pid = (u16::from(table[at + 1] & 0x1F) << 8) | u16::from(table[at + 2]);
        let es_length = (usize::from(table[at + 3] & 0x0F) << 8) | usize::from(table[at + 4]);
        match stream_type {
            0x0F => return Some((pid, "aac")),
            0x03 | 0x04 => return Some((pid, "mp3")),
            _ => {}
        }
        at += 5 + es_length;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One 188-byte packet: header, then `payload` padded with 0xFF.
    fn packet(pid: u16, unit_start: bool, payload: &[u8]) -> Vec<u8> {
        let mut out = vec![
            0x47,
            (if unit_start { 0x40 } else { 0 }) | ((pid >> 8) as u8 & 0x1F),
            pid as u8,
            0x10,
        ];
        out.extend_from_slice(payload);
        out.resize(188, 0xFF);
        out
    }

    #[test]
    fn transport_streams_give_up_their_sound() {
        // PAT: program 1 → PMT on PID 0x100.
        let pat = [
            0, 0x00, 0xB0, 13, 0, 1, 0xC1, 0, 0, 0, 1, 0xE1, 0x00, 0, 0, 0, 0,
        ];
        // PMT: video (0x1B) on 0x101, AAC (0x0F) on 0x102.
        let pmt = [
            0, 0x02, 0xB0, 23, 0, 1, 0xC1, 0, 0, 0xE1, 0x01, 0xF0, 0, 0x1B, 0xE1, 0x01, 0xF0, 0,
            0x0F, 0xE1, 0x02, 0xF0, 0, 0, 0, 0, 0,
        ];
        let mut pes = vec![0, 0, 1, 0xC0, 0, 0, 0x80, 0x80, 5, 1, 2, 3, 4, 5];
        pes.extend_from_slice(b"ADTS-ONE");
        let mut stream = Vec::new();
        stream.extend(packet(0, true, &pat));
        stream.extend(packet(0x100, true, &pmt));
        stream.extend(packet(0x101, true, b"\x00\x00\x01\xE0video"));
        stream.extend(packet(0x102, true, &pes));
        let (audio, extension) = ts_audio(&stream).unwrap();
        assert_eq!(extension, "aac");
        assert!(audio.starts_with(b"ADTS-ONE"));
        assert!(!audio.windows(5).any(|window| window == b"video"));
    }

    #[test]
    fn media_playlists_give_timed_segments_keys_and_maps() {
        let text = "#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:3\n#EXT-X-KEY:METHOD=AES-128,URI=\"k.key\"\n\
            #EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:6.0,\na.m4s\n#EXTINF:4.5,\nb.m4s\n#EXT-X-ENDLIST\n";
        let (segments, ended) =
            parse_media(text, "http://127.0.0.1:1/media/t/https/cdn.x/v/index.m3u8").unwrap();
        assert!(ended);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[1].start_ms, 6_000);
        assert_eq!(segments[1].duration_ms, 4_500);
        assert_eq!(
            segments[0].url,
            "http://127.0.0.1:1/media/t/https/cdn.x/v/a.m4s"
        );
        let (key, iv) = segments[0].key.clone().unwrap();
        assert!(key.ends_with("/v/k.key"));
        assert_eq!(iv, 3u128.to_be_bytes());
        assert!(segments[0].map.as_ref().unwrap().0.ends_with("/v/init.mp4"));
    }

    /// Hears a real video: `VIDEO_URL=<stream or file> WHISPER_DIR=<model> cargo test -p
    /// raygun-app --release -- --ignored video_whisper_live --nocapture` (`VIDEO_FROM_MS` to
    /// start later).
    #[test]
    #[ignore = "needs the network, a video and a Whisper model"]
    fn video_whisper_live() {
        let url = std::env::var("VIDEO_URL").expect("VIDEO_URL");
        let dir = std::path::PathBuf::from(std::env::var("WHISPER_DIR").expect("WHISPER_DIR"));
        let from: u64 = std::env::var("VIDEO_FROM_MS")
            .ok()
            .and_then(|ms| ms.parse().ok())
            .unwrap_or(0);
        let client = raygun_net::http_client().unwrap();
        let mut sound = VideoAudio::open(client, &url, None).unwrap();
        println!("duration {:?}", sound.duration_ms());
        let mut whisper = raygun_ai::speech::Whisper::load(&dir).unwrap();
        let mut at = from;
        for _ in 0..2 {
            let started = std::time::Instant::now();
            let samples = sound.window(at, 30_000).unwrap();
            println!(
                "window at {at}: {} samples in {:?}",
                samples.len(),
                started.elapsed()
            );
            if samples.is_empty() {
                break;
            }
            let heard = whisper.transcribe(&samples, None).unwrap();
            for segment in &heard.segments {
                println!("  {:>7} {}", at + u64::from(segment.start_ms), segment.text);
            }
            at += u64::from(heard.consumed_ms.max(1000));
        }
    }

    #[test]
    fn protected_streams_are_refused() {
        let text = "#EXTM3U\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"k\"\n#EXTINF:6,\na.ts\n";
        assert!(parse_media(text, "https://cdn.x/v.m3u8").is_err());
    }
}
