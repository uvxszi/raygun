//! Downloaded videos as files (M46.1). They're saved as MP4 or Matroska (packets copied into the
//! new container, `remux`), as their sound alone (M4A or MP3), or as they downloaded: a plain
//! download copied out, an HLS download (segments, keys and init maps beside a playlist) joined
//! into one file, MPEG-TS segments into a `.ts` and fragmented MP4 (an `#EXT-X-MAP` init section)
//! into an `.mp4`, AES-128 segments decrypted by their saved keys. A stream whose sound is a
//! separate rendition joins it only when converted. A video a format can't hold is kept as it
//! downloaded, and the job says so. Saved subtitle tracks go beside the video. Episodes that
//! aren't downloaded yet are queued first (finding their links on their own) and the export waits
//! for them.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use aes::cipher::BlockDecryptMut;
use aes::cipher::KeyIvInit;
use aes::cipher::block_padding::Pkcs7;
use std::collections::{HashMap, HashSet};

use raygun_db::titles::{self, TitleRow, UnitRow};
use raygun_db::video_downloads::{self as store, VideoDownloadRow};
use raygun_model::{ExportFormat, ExportRequest, ExportScope, VideoSubtitle};
use tauri::{AppHandle, Manager};

use super::{Chosen, ExportService, Place, Planned, names, output, remux};
use crate::catalog::{now_ms, to_message};
use crate::startup::{AppDb, AppPaths};
use crate::video_downloads::VideoDownloadService;

type Decryptor = cbc::Decryptor<aes::Aes128>;

/// The video downloads' folder name (as in `video_downloads`).
const FOLDER: &str = "video";

/// One file of a joined stream.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    /// An init section, written once before the segments that use it.
    Map(String),
    Segment {
        name: String,
        /// The key file and IV, for AES-128.
        key: Option<(String, [u8; 16])>,
    },
}

/// A saved media playlist, ready to join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Join {
    parts: Vec<Part>,
}

impl Join {
    /// Reads a saved playlist. A master playlist (separate audio) can't be joined: converting
    /// (`remux`) reads its two playlists instead.
    pub fn parse(playlist: &str) -> Result<Self, String> {
        if playlist.contains("#EXT-X-STREAM-INF") {
            return Err(
                "This video's sound is a separate stream: save it as MP4 or MKV to join them"
                    .into(),
            );
        }
        let mut parts = Vec::new();
        let mut sequence: u128 = 0;
        let mut key: Option<(String, Option<[u8; 16]>)> = None;
        let mut map: Option<String> = None;
        for line in playlist
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            if let Some(value) = line.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
                sequence = value.trim().parse().unwrap_or(0);
            } else if let Some(attributes) = line.strip_prefix("#EXT-X-KEY:") {
                key = match attribute(attributes, "METHOD").as_deref() {
                    Some("AES-128") => {
                        let uri =
                            attribute(attributes, "URI").ok_or("An encryption key is missing")?;
                        let iv = attribute(attributes, "IV")
                            .map(|iv| parse_iv(&iv))
                            .transpose()?;
                        Some((uri, iv))
                    }
                    Some("NONE") | None => None,
                    Some(_) => return Err("This video is protected".into()),
                };
            } else if let Some(attributes) = line.strip_prefix("#EXT-X-MAP:") {
                let uri = attribute(attributes, "URI").ok_or("An init section is missing")?;
                if map.as_deref() != Some(uri.as_str()) {
                    parts.push(Part::Map(uri.clone()));
                    map = Some(uri);
                }
            } else if !line.starts_with('#') {
                let key = key.as_ref().map(|(file, iv)| {
                    // Without an IV, the segment's sequence number is the IV (RFC 8216 §5.2).
                    (file.clone(), iv.unwrap_or_else(|| sequence.to_be_bytes()))
                });
                parts.push(Part::Segment {
                    name: line.to_owned(),
                    key,
                });
                sequence += 1;
            }
        }
        if !parts
            .iter()
            .any(|part| matches!(part, Part::Segment { .. }))
        {
            return Err("The saved stream has no segments".into());
        }
        Ok(Self { parts })
    }

    /// `mp4` for fragmented MP4, else `ts`.
    pub fn extension(&self) -> &'static str {
        if self.parts.iter().any(|part| matches!(part, Part::Map(_))) {
            "mp4"
        } else {
            "ts"
        }
    }

    /// Writes the joined stream, reading the parts from `dir`.
    pub fn write(
        &self,
        dir: &Path,
        out: &mut impl Write,
        cancel: &AtomicBool,
    ) -> std::io::Result<()> {
        for segment in self.segments(dir) {
            if cancel.load(Ordering::SeqCst) {
                return Err(std::io::Error::other("Cancelled"));
            }
            let segment = segment?;
            if let Some(init) = &segment.init {
                out.write_all(init)?;
            }
            out.write_all(&segment.data)?;
        }
        Ok(())
    }

    /// The segments in order, decrypted, each with the init section it starts (fragmented MP4).
    pub fn segments<'a>(&'a self, dir: &'a Path) -> remux::Segments<'a> {
        let mut keys: Vec<(String, [u8; 16])> = Vec::new();
        let mut init: Option<Vec<u8>> = None;
        let mut parts = self.parts.iter();
        Box::new(std::iter::from_fn(move || {
            loop {
                let part = parts.next()?;
                let read = (|| -> std::io::Result<Option<remux::Segment>> {
                    match part {
                        Part::Map(name) => {
                            init = Some(std::fs::read(safe_join(dir, name)?)?);
                            Ok(None)
                        }
                        Part::Segment { name, key } => {
                            let mut data = std::fs::read(safe_join(dir, name)?)?;
                            if let Some((file, iv)) = key {
                                let key = match keys.iter().find(|(known, _)| known == file) {
                                    Some((_, key)) => *key,
                                    None => {
                                        let mut key = [0u8; 16];
                                        File::open(safe_join(dir, file)?)?.read_exact(&mut key)?;
                                        keys.push((file.clone(), key));
                                        key
                                    }
                                };
                                let length = Decryptor::new(&key.into(), &(*iv).into())
                                    .decrypt_padded_mut::<Pkcs7>(&mut data)
                                    .map_err(|_| {
                                        std::io::Error::other("A segment couldn't be decrypted")
                                    })?
                                    .len();
                                data.truncate(length);
                            }
                            Ok(Some(remux::Segment {
                                init: init.take(),
                                data,
                            }))
                        }
                    }
                })();
                match read {
                    Ok(Some(segment)) => return Some(Ok(segment)),
                    Ok(None) => {}
                    Err(error) => return Some(Err(error)),
                }
            }
        }))
    }
}

/// A part's file inside the download folder: names are the saved ones, never paths.
fn safe_join(dir: &Path, name: &str) -> std::io::Result<PathBuf> {
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(std::io::Error::other(
            "The saved playlist points outside its folder",
        ));
    }
    Ok(dir.join(name))
}

pub(crate) fn attribute(list: &str, name: &str) -> Option<String> {
    let mut rest = list;
    while !rest.is_empty() {
        let (key, after) = rest.split_once('=')?;
        let (value, next) = if let Some(quoted) = after.strip_prefix('"') {
            let end = quoted.find('"')?;
            (&quoted[..end], quoted[end + 1..].trim_start_matches(','))
        } else {
            match after.split_once(',') {
                Some((value, next)) => (value, next),
                None => (after, ""),
            }
        };
        if key.trim() == name {
            return Some(value.to_owned());
        }
        rest = next;
    }
    None
}

pub(crate) fn parse_iv(value: &str) -> Result<[u8; 16], String> {
    let hex = value
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    let number = u128::from_str_radix(hex, 16).map_err(|_| "An encryption IV is unreadable")?;
    Ok(number.to_be_bytes())
}

fn downloads_dir(app: &AppHandle) -> PathBuf {
    app.state::<AppPaths>().0.downloads().join(FOLDER)
}

/// An episode an export downloads first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Missing {
    pub unit_id: String,
    /// Its progress key, which its download goes by.
    pub key: String,
    /// The title's player group (`Origin::group_key`).
    pub group: String,
    /// Its id at the source.
    pub remote_id: String,
}

/// A title's player group: `stremio:<type>:<id>` for Stremio titles, else `title:<id>`.
fn group_of(title: &TitleRow) -> String {
    match crate::video_library::stremio_id(title) {
        Some((kind, id)) => format!("stremio:{kind}:{id}"),
        None => format!("title:{}", title.id),
    }
}

/// Scopes may name episodes by their stored id or their id at the source (Stremio's pages know
/// only those): both become stored ids.
fn stored_ids(scope: &ExportScope, units: &[&UnitRow]) -> ExportScope {
    let stored = |id: &String| {
        units
            .iter()
            .find(|unit| unit.id == *id)
            .or_else(|| units.iter().find(|unit| unit.fields.remote_id == *id))
            .map_or_else(|| id.clone(), |unit| unit.id.clone())
    };
    match scope {
        ExportScope::Range { from, to } => ExportScope::Range {
            from: stored(from),
            to: stored(to),
        },
        ExportScope::Chosen { ids } => ExportScope::Chosen {
            ids: ids.iter().map(stored).collect(),
        },
        other => other.clone(),
    }
}

/// The episodes a scope takes, in watching order: episodes not out yet are left out unless
/// they're downloaded (`downloaded` holds stored ids).
pub(super) fn pick_episodes<'a>(
    ordered: Vec<&'a UnitRow>,
    scope: &ExportScope,
    downloaded: &HashSet<String>,
    now: i64,
) -> Vec<&'a UnitRow> {
    let scope = stored_ids(scope, &ordered);
    super::pick(ordered, &scope, downloaded, false)
        .into_iter()
        .filter(|unit| downloaded.contains(&unit.id) || unit.fields.date.is_none_or(|at| at <= now))
        .collect()
}

/// The file a format converts to, or `None` for the video as it downloaded.
pub(super) fn target_of(format: ExportFormat, mp3_kbps: u32) -> Option<remux::Target> {
    match format {
        ExportFormat::Mp4 => Some(remux::Target::Mp4),
        ExportFormat::Mkv => Some(remux::Target::Mkv),
        ExportFormat::M4a => Some(remux::Target::M4a),
        ExportFormat::Mp3 => Some(remux::Target::Mp3 { kbps: mp3_kbps }),
        _ => None,
    }
}

/// A file's type from its name.
pub(super) fn mime_of(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or_default() {
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "ts" => "video/mp2t",
        "m4a" => "audio/mp4",
        "mp3" => "audio/mpeg",
        "mp4" | "m4v" => "video/mp4",
        _ => "video/*",
    }
}

/// What each download holds, read once (by download id and file): its streams, or why it can't
/// be read.
#[derive(Default)]
pub(crate) struct Probes(Mutex<HashMap<String, Result<remux::Probe, String>>>);

impl Probes {
    fn get(&self, key: &str) -> Option<Result<remux::Probe, String>> {
        self.0.lock().ok()?.get(key).cloned()
    }

    fn put(&self, key: String, probe: Result<remux::Probe, String>) {
        if let Ok(mut probes) = self.0.lock() {
            probes.insert(key, probe);
        }
    }
}

/// The MP3 bitrate: the Downloads audio quality, as songs saved as MP3 (Best is 320 kbps).
async fn mp3_kbps(app: &AppHandle) -> u32 {
    let db = app.state::<AppDb>().inner().0.clone();
    crate::settings::load(&db)
        .await
        .unwrap_or_default()
        .audio_quality
        .download
        .target_kbps()
        .unwrap_or(320)
}

/// The episodes a request exports, and those it downloads first.
pub(super) async fn plan(
    service: &ExportService,
    request: &ExportRequest,
) -> Result<Planned, String> {
    let app = &service.app;
    let db = app.state::<AppDb>().inner().0.clone();
    let now = now_ms();
    let mut chosen = Vec::new();
    let mut missing = Vec::new();
    let mut problems = Vec::new();
    let mut rows = Vec::new();
    for title_id in &request.title_ids {
        let title = titles::title(&db, title_id)
            .await
            .map_err(to_message)?
            .ok_or_else(|| "A title is no longer stored".to_owned())?;
        if title.fields.source_id == crate::local_files::LOCAL_VIDEO {
            problems.push("These videos are already files on this device".to_owned());
            continue;
        }
        let units = titles::units(&db, title_id).await.map_err(to_message)?;
        let keys: HashMap<&str, String> = units
            .iter()
            .map(|unit| {
                (
                    unit.id.as_str(),
                    crate::video_library::unit_key(&title, &unit.fields.remote_id),
                )
            })
            .collect();
        let states = store::states_for_keys(&db, &keys.values().cloned().collect::<Vec<_>>())
            .await
            .map_err(to_message)?;
        let downloaded: HashSet<String> = keys
            .iter()
            .filter(|(_, key)| states.get(*key).map(String::as_str) == Some(store::DONE))
            .map(|(id, _)| (*id).to_owned())
            .collect();
        let picked: Vec<UnitRow> = pick_episodes(
            crate::video_sources::watching_order(&units),
            &request.scope,
            &downloaded,
            now,
        )
        .into_iter()
        .cloned()
        .collect();
        let group = group_of(&title);
        for unit in &picked {
            let key = keys[unit.id.as_str()].clone();
            if downloaded.contains(&unit.id) {
                if let Ok(Some(row)) = store::done_for_key(&db, &key).await {
                    rows.push(row);
                }
            } else {
                missing.push(Missing {
                    unit_id: unit.id.clone(),
                    key,
                    group: group.clone(),
                    remote_id: unit.fields.remote_id.clone(),
                });
            }
        }
        chosen.push(Chosen {
            title,
            units: picked,
        });
    }
    problems.extend(problems_of(service, &rows, request.format).await);
    let files: u32 = chosen.iter().map(|title| title.units.len() as u32).sum();
    let problem = if files == 0 {
        Some(problems.first().cloned().unwrap_or_else(|| {
            if request.scope == ExportScope::Downloaded {
                "No episodes are downloaded yet".to_owned()
            } else {
                "There are no episodes to export".to_owned()
            }
        }))
    } else if request.destination == raygun_model::ExportDestination::Share && files != 1 {
        Some("Sharing sends one file: export a single episode".to_owned())
    } else {
        problems.into_iter().next()
    };
    Ok(Planned {
        titles: chosen,
        missing: Vec::new(),
        videos: missing,
        files,
        problem,
    })
}

/// Queues `missing` (each finds its link on its own) and waits until each is downloaded or
/// failed. Returns the stored ids of the failed ones.
pub(super) async fn wait_for_downloads(
    service: &ExportService,
    job: &str,
    missing: &[Missing],
    cancel: &AtomicBool,
) -> Result<HashSet<String>, String> {
    if missing.is_empty() {
        return Ok(HashSet::new());
    }
    let app = service.app.clone();
    let downloads = app
        .state::<std::sync::Arc<VideoDownloadService>>()
        .inner()
        .clone();
    let mut groups: Vec<(&str, Vec<String>)> = Vec::new();
    for episode in missing {
        match groups.iter_mut().find(|(group, _)| *group == episode.group) {
            Some((_, ids)) => ids.push(episode.remote_id.clone()),
            None => groups.push((&episode.group, vec![episode.remote_id.clone()])),
        }
    }
    for (group, ids) in &groups {
        downloads.enqueue_episodes(group, ids).await?;
    }
    let db = app.state::<AppDb>().inner().0.clone();
    let keys: Vec<String> = missing.iter().map(|episode| episode.key.clone()).collect();
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err("Cancelled".into());
        }
        let states = store::states_for_keys(&db, &keys)
            .await
            .map_err(to_message)?;
        let (done, failed) = settled(missing, &states);
        let paused = downloads.is_paused();
        service.update(job, |job| {
            job.downloaded = done as u32;
            job.message = paused.then(|| "Downloads are paused".to_owned());
        });
        if done + failed.len() >= missing.len() {
            return Ok(failed);
        }
        tokio::time::sleep(super::WAIT_STEP).await;
    }
}

/// How many of `missing` are downloaded, and which failed (or left the queue), by their download
/// states.
pub(super) fn settled(
    missing: &[Missing],
    states: &HashMap<String, String>,
) -> (usize, HashSet<String>) {
    let done = missing
        .iter()
        .filter(|episode| states.get(&episode.key).map(String::as_str) == Some(store::DONE))
        .count();
    let failed = missing
        .iter()
        .filter(|episode| {
            states
                .get(&episode.key)
                .is_none_or(|state| state == store::FAILED)
        })
        .map(|episode| episode.unit_id.clone())
        .collect();
    (done, failed)
}

/// Writes each episode as one file in `format` (a video's subtitles beside it). An episode the
/// format can't hold is saved as it downloaded, and the job's message says so.
#[allow(clippy::too_many_arguments)]
pub(super) async fn write(
    service: &ExportService,
    job: &str,
    title: &TitleRow,
    units: &[UnitRow],
    format: ExportFormat,
    place: &Place,
    cancel: &AtomicBool,
) -> Result<Vec<output::Saved>, String> {
    let app = service.app.clone();
    let db = app.state::<AppDb>().inner().0.clone();
    let target = target_of(format, mp3_kbps(&app).await);
    let mut saved = Vec::new();
    let mut kept = Vec::new();
    for unit in units {
        if cancel.load(Ordering::SeqCst) {
            return Err("Cancelled".into());
        }
        let key = crate::video_library::unit_key(title, &unit.fields.remote_id);
        let row = store::done_for_key(&db, &key)
            .await
            .map_err(to_message)?
            .ok_or_else(|| format!("{} isn't downloaded", unit.fields.name))?;
        let (files, why) = write_row(service, &app, &row, title, target, place, cancel).await?;
        saved.extend(files);
        kept.extend(why);
        service.update(job, |job| job.written += 1);
    }
    if let Some(why) = kept.first() {
        let message = match kept.len() {
            1 => format!("One episode was saved as it downloaded. {why}"),
            count => format!("{count} episodes were saved as they downloaded. {why}"),
        };
        service.update(job, |job| job.message = Some(message));
    }
    Ok(saved)
}

/// What a download holds, read from its start.
fn probe_row(dir: &Path, file: &str) -> Result<remux::Probe, String> {
    let source = remux::Source::of_download(dir, file).map_err(|error| error.to_string())?;
    remux::probe(&source).map_err(|error| error.to_string())
}

/// Why the downloaded episodes can't be written in `format`, each reason once.
async fn problems_of(
    service: &ExportService,
    rows: &[VideoDownloadRow],
    format: ExportFormat,
) -> Vec<String> {
    let dir = downloads_dir(&service.app);
    let target = target_of(format, 128);
    let mut problems: Vec<String> = Vec::new();
    for row in rows {
        let Some(file) = row.file.clone() else {
            continue;
        };
        let folder = dir.join(&row.id);
        let found = match target {
            // As it downloaded: a stream with separate sound can't be one file.
            None if file.ends_with(".m3u8") => std::fs::read_to_string(folder.join(&file))
                .ok()
                .and_then(|playlist| Join::parse(&playlist).err()),
            None => None,
            Some(target) => {
                let key = format!("{}/{file}", row.id);
                let probe = match service.probes.get(&key) {
                    Some(probe) => probe,
                    None => {
                        let probe =
                            tauri::async_runtime::spawn_blocking(move || probe_row(&folder, &file))
                                .await
                                .map_err(to_message)
                                .and_then(|probe| probe);
                        service.probes.put(key, probe.clone());
                        probe
                    }
                };
                match probe {
                    Ok(probe) => remux::refusal(&probe, target),
                    Err(error) => Some(error),
                }
            }
        };
        if let Some(problem) = found
            && !problems.contains(&problem)
        {
            problems.push(problem);
        }
    }
    problems
}

/// Converts a download into `target`: `Ok(Err(why))` when the target can't hold it (nothing is
/// left behind, so it can be saved as it downloaded instead).
#[allow(clippy::too_many_arguments)]
fn convert_row(
    outputs: &output::Outputs,
    place: &Place,
    folder: &[String],
    stem: &str,
    dir: &Path,
    file: &str,
    target: remux::Target,
    cancel: &AtomicBool,
) -> Result<Result<output::Saved, String>, String> {
    let source = remux::Source::of_download(dir, file).map_err(|error| error.to_string())?;
    match remux::probe(&source) {
        Ok(probe) => {
            if let Some(why) = remux::refusal(&probe, target) {
                return Ok(Err(why));
            }
        }
        Err(remux::Error::Unsupported(why)) => return Ok(Err(why)),
        Err(error) => return Err(error.to_string()),
    }
    let name = names::file_name(stem, target.extension());
    let mut refused = None;
    let written = super::write_one(outputs, place, folder, &name, target.mime(), |out| {
        let mut out = std::io::BufWriter::new(out);
        match remux::convert(&source, target, &mut out, cancel) {
            Ok(()) => out.flush().map(|()| 0),
            Err(remux::Error::Io(error)) => Err(error),
            Err(remux::Error::Unsupported(why)) => {
                refused = Some(why);
                Err(std::io::Error::other("refused"))
            }
            Err(error) => Err(std::io::Error::other(error.to_string())),
        }
    });
    match (written, refused) {
        (_, Some(why)) => Ok(Err(why)),
        (Ok((saved, _)), None) => Ok(Ok(saved)),
        (Err(error), None) => Err(error),
    }
}

/// Writes one download: converted into `target`, or as it downloaded (`None`, or when the target
/// can't hold it: the reason comes back).
async fn write_row(
    service: &ExportService,
    app: &AppHandle,
    row: &VideoDownloadRow,
    title: &TitleRow,
    target: Option<remux::Target>,
    place: &Place,
    cancel: &AtomicBool,
) -> Result<(Vec<output::Saved>, Option<String>), String> {
    let file = row.file.clone().ok_or("The download has no file")?;
    let dir = downloads_dir(app).join(&row.id);
    let stem = match row.download.subtitle.as_deref() {
        Some(episode) if !episode.trim().is_empty() => {
            format!("{} - {episode}", title.fields.title)
        }
        _ => title.fields.title.clone(),
    };
    let folder = vec![
        super::medium_folder("video").to_owned(),
        names::safe_name(&title.fields.title),
    ];
    let subtitles: Vec<VideoSubtitle> =
        serde_json::from_str(&row.local_subtitles).unwrap_or_default();
    let outputs = service.outputs.clone();
    let place = place.clone();
    // Cancelling reaches the blocking writer through its own flag.
    let cancel_flag = std::sync::Arc::new(AtomicBool::new(cancel.load(Ordering::SeqCst)));
    let finished = std::sync::Arc::new(AtomicBool::new(false));
    let (writer_cancel, writer_finished) = (cancel_flag.clone(), finished.clone());
    let writer = tauri::async_runtime::spawn_blocking(move || {
        let cancel_flag = writer_cancel;
        let _finished = Finished(writer_finished);
        let mut saved = Vec::new();
        let mut kept = None;
        let converted = match target {
            Some(target) => match convert_row(
                &outputs,
                &place,
                &folder,
                &stem,
                &dir,
                &file,
                target,
                &cancel_flag,
            )? {
                Ok(video) => {
                    saved.push(video);
                    true
                }
                Err(why) => {
                    kept = Some(why);
                    false
                }
            },
            None => false,
        };
        // The sound alone has no use for subtitles.
        let audio_only =
            matches!(target, Some(remux::Target::M4a | remux::Target::Mp3 { .. })) && converted;
        if !converted && file.ends_with(".m3u8") {
            let playlist = std::fs::read_to_string(dir.join(&file)).map_err(to_message)?;
            let join = Join::parse(&playlist)?;
            let name = names::file_name(&stem, join.extension());
            let (video, _) =
                super::write_one(&outputs, &place, &folder, &name, mime_of(&name), |out| {
                    let mut out = std::io::BufWriter::new(out);
                    join.write(&dir, &mut out, &cancel_flag)?;
                    out.flush().map(|()| 0)
                })?;
            saved.push(video);
        } else if !converted {
            let extension = Path::new(&file)
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or("mp4")
                .to_owned();
            let name = names::file_name(&stem, &extension);
            let source = safe_join(&dir, &file).map_err(to_message)?;
            let (video, _) =
                super::write_one(&outputs, &place, &folder, &name, mime_of(&name), |out| {
                    std::io::copy(&mut File::open(&source)?, out).map(|_| 0)
                })?;
            saved.push(video);
        }
        for track in subtitles.into_iter().filter(|_| !audio_only) {
            let Ok(source) = safe_join(&dir, &track.url) else {
                continue;
            };
            let extension = track.format.clone().unwrap_or_else(|| "srt".into());
            let language = track.lang.clone().unwrap_or_else(|| "und".into());
            let name = names::file_name(&format!("{stem}.{language}"), &extension);
            // Subtitles are extras: one that can't be written doesn't fail the video.
            // A generic type: Android adds ".txt" to a name whose extension doesn't match a
            // text type, and has no type for most subtitle formats.
            let _ = super::write_one(
                &outputs,
                &place,
                &folder,
                &name,
                "application/octet-stream",
                |out| std::io::copy(&mut File::open(&source)?, out).map(|_| 0),
            );
        }
        Ok::<_, String>((saved, kept))
    });
    let watcher = async {
        while !finished.load(Ordering::SeqCst) {
            if cancel.load(Ordering::SeqCst) {
                cancel_flag.store(true, Ordering::SeqCst);
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    };
    let (result, ()) = tokio::join!(writer, watcher);
    result.map_err(to_message)?
}

/// Sets its flag when dropped: the blocking writer finished, however it ended.
struct Finished(std::sync::Arc<AtomicBool>);

impl Drop for Finished {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// Saves a video that just finished downloading outside the app, when Settings → Downloads asks
/// for it.
pub fn save_finished(app: &AppHandle, download_id: &str) {
    let app = app.clone();
    let id = download_id.to_owned();
    tauri::async_runtime::spawn(async move {
        let Some(service) = app.try_state::<std::sync::Arc<ExportService>>() else {
            return;
        };
        let service = service.inner().clone();
        let result = async {
            let prefs = service.save_prefs().await;
            let destination = match prefs.location {
                raygun_model::SaveLocation::App => return Ok(None),
                raygun_model::SaveLocation::Downloads => raygun_model::ExportDestination::Downloads,
                raygun_model::SaveLocation::Folder => raygun_model::ExportDestination::Folder,
            };
            let place = super::place_for(&prefs, destination)?;
            let db = app.state::<AppDb>().inner().0.clone();
            let row = store::get(&db, &id)
                .await
                .map_err(to_message)?
                .ok_or("The download is gone")?;
            let Some(title_id) = row.download.title_id.clone() else {
                return Ok(None);
            };
            let title = titles::title(&db, &title_id)
                .await
                .map_err(to_message)?
                .ok_or("The title is no longer stored")?;
            // Saved as it downloaded, unless its sound is a separate stream: then joined in an MP4.
            let separate = row.file.as_deref().is_some_and(|file| {
                file.ends_with(".m3u8")
                    && std::fs::read_to_string(downloads_dir(&app).join(&row.id).join(file))
                        .is_ok_and(|playlist| playlist.contains("#EXT-X-STREAM-INF"))
            });
            write_row(
                &service,
                &app,
                &row,
                &title,
                separate.then_some(remux::Target::Mp4),
                &place,
                &AtomicBool::new(false),
            )
            .await
            .map(|_| Some(title_id))
        }
        .await;
        if let Err(message) = result {
            tracing::warn!(%message, download = %id, "could not save a video outside the app");
            service.record_failure("", message).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use raygun_db::titles::UnitFields;

    use super::*;

    fn episode(id: &str, season: &str, number: f64, date: Option<i64>) -> UnitRow {
        UnitRow {
            id: id.into(),
            title_id: "t".into(),
            fields: UnitFields {
                remote_id: format!("remote-{id}"),
                name: format!("Episode {number}"),
                number: Some(number),
                volume: Some(season.into()),
                scanlator: None,
                date,
            },
            source_order: 0,
            position: None,
            total: None,
            completed_at: None,
            progress_at: None,
        }
    }

    fn ids(units: Vec<&UnitRow>) -> Vec<&str> {
        units.into_iter().map(|unit| unit.id.as_str()).collect()
    }

    #[test]
    fn export_scopes_pick_episodes_in_watching_order() {
        let units = [
            episode("a", "1", 1.0, None),
            episode("b", "1", 2.0, None),
            episode("c", "2", 1.0, None),
            episode("d", "2", 2.0, Some(10_000)),
        ];
        let ordered = || units.iter().collect::<Vec<_>>();
        let downloaded: HashSet<String> = ["b".to_owned()].into();
        assert_eq!(
            ids(pick_episodes(ordered(), &ExportScope::All, &downloaded, 5)),
            vec!["a", "b", "c"],
            "an episode not out yet is left out"
        );
        assert_eq!(
            ids(pick_episodes(
                ordered(),
                &ExportScope::All,
                &downloaded,
                20_000
            )),
            vec!["a", "b", "c", "d"]
        );
        assert_eq!(
            ids(pick_episodes(
                ordered(),
                &ExportScope::Downloaded,
                &downloaded,
                5
            )),
            vec!["b"]
        );
        // From episode X to Y, named by stored or source ids, either way round.
        let range = ExportScope::Range {
            from: "remote-c".into(),
            to: "b".into(),
        };
        assert_eq!(
            ids(pick_episodes(ordered(), &range, &downloaded, 5)),
            vec!["b", "c"]
        );
        // A season or ticked episodes come as a list.
        let chosen = ExportScope::Chosen {
            ids: vec!["remote-a".into(), "c".into(), "gone".into()],
        };
        assert_eq!(
            ids(pick_episodes(ordered(), &chosen, &downloaded, 5)),
            vec!["a", "c"]
        );
    }

    #[test]
    fn formats_convert_or_keep_the_download() {
        assert_eq!(target_of(ExportFormat::Mp4, 192), Some(remux::Target::Mp4));
        assert_eq!(target_of(ExportFormat::Mkv, 192), Some(remux::Target::Mkv));
        assert_eq!(target_of(ExportFormat::M4a, 192), Some(remux::Target::M4a));
        assert_eq!(
            target_of(ExportFormat::Mp3, 192),
            Some(remux::Target::Mp3 { kbps: 192 })
        );
        assert_eq!(target_of(ExportFormat::Video, 192), None);
        assert_eq!(mime_of("Show - E1.mkv"), "video/x-matroska");
        assert_eq!(mime_of("Show - E1.ts"), "video/mp2t");
        assert_eq!(mime_of("Show - E1.m4a"), "audio/mp4");
        assert_eq!(mime_of("Show"), "video/*");
    }

    #[test]
    fn downloads_are_probed_once_for_what_formats_take() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/export/remux/fixtures");
        let probe = probe_row(&fixtures.join("ts"), "index.m3u8").unwrap();
        assert!(matches!(probe.video, Some(remux::Codec::H264(_))));
        assert!(remux::refusal(&probe, remux::Target::Mp4).is_none());
        let probes = Probes::default();
        probes.put("d1/index.m3u8".into(), Ok(probe.clone()));
        assert_eq!(probes.get("d1/index.m3u8"), Some(Ok(probe)));
        // MP3 sound can't go in an M4A: the plan says so before anything is written.
        let mp3 = probe_row(&fixtures, "mp3.m2t").unwrap();
        assert!(remux::refusal(&mp3, remux::Target::M4a).is_some_and(|why| why.contains("MP3")));
        // A separate audio stream is one file only when converted.
        let pair = std::fs::read_to_string(fixtures.join("pair/index.m3u8")).unwrap();
        assert!(Join::parse(&pair).is_err());
        assert!(probe_row(&fixtures.join("pair"), "index.m3u8").is_ok());
    }

    #[test]
    fn an_export_waits_until_each_episode_is_downloaded_or_failed() {
        let missing: Vec<Missing> = ["a", "b", "c"]
            .iter()
            .map(|id| Missing {
                unit_id: (*id).into(),
                key: format!("source:s:{id}"),
                group: "title:t".into(),
                remote_id: (*id).into(),
            })
            .collect();
        let mut states = HashMap::from([
            ("source:s:a".to_owned(), store::DONE.to_owned()),
            ("source:s:b".to_owned(), store::DOWNLOADING.to_owned()),
            ("source:s:c".to_owned(), store::QUEUED.to_owned()),
        ]);
        let (done, failed) = settled(&missing, &states);
        assert_eq!((done, failed.len()), (1, 0));
        states.insert("source:s:b".into(), store::FAILED.into());
        // A download taken off the queue counts as failed: the export goes on without it.
        states.remove("source:s:c");
        let (done, failed) = settled(&missing, &states);
        assert_eq!(done, 1);
        assert_eq!(failed, HashSet::from(["b".to_owned(), "c".to_owned()]));
        assert!(done + failed.len() >= missing.len());
    }

    #[test]
    fn saved_playlists_join_with_their_keys() {
        let playlist = "#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:7\n#EXT-X-KEY:METHOD=AES-128,URI=\"key1.key\"\n\
                        00001.ts\n#EXT-X-KEY:METHOD=AES-128,URI=\"key1.key\",IV=0x0000000000000000000000000000000A\n\
                        00002.ts\n#EXT-X-KEY:METHOD=NONE\n00003.ts\n#EXT-X-ENDLIST\n";
        let join = Join::parse(playlist).unwrap();
        assert_eq!(join.extension(), "ts");
        let mut seven = [0u8; 16];
        seven[15] = 7;
        let mut ten = [0u8; 16];
        ten[15] = 10;
        assert_eq!(
            join.parts,
            vec![
                Part::Segment {
                    name: "00001.ts".into(),
                    key: Some(("key1.key".into(), seven))
                },
                Part::Segment {
                    name: "00002.ts".into(),
                    key: Some(("key1.key".into(), ten))
                },
                Part::Segment {
                    name: "00003.ts".into(),
                    key: None
                },
            ]
        );
        let fmp4 =
            Join::parse("#EXTM3U\n#EXT-X-MAP:URI=\"init1.mp4\"\n00001.m4s\n00002.m4s\n").unwrap();
        assert_eq!(fmp4.extension(), "mp4");
        assert!(Join::parse("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\nvideo.m3u8\n").is_err());
        assert!(Join::parse("#EXTM3U\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"k\"\n1.ts\n").is_err());
    }

    #[test]
    fn segments_are_decrypted_in_order() {
        use aes::cipher::BlockEncryptMut;
        type Encryptor = cbc::Encryptor<aes::Aes128>;
        let dir = tempfile::tempdir().unwrap();
        let key = [3u8; 16];
        std::fs::write(dir.path().join("key1.key"), key).unwrap();
        let mut iv = [0u8; 16];
        iv[15] = 1;
        let text = b"second segment";
        let mut buffer = [0u8; 32];
        buffer[..text.len()].copy_from_slice(text);
        let encrypted = Encryptor::new(&key.into(), &iv.into())
            .encrypt_padded_mut::<Pkcs7>(&mut buffer, text.len())
            .unwrap()
            .to_vec();
        std::fs::write(dir.path().join("00001.ts"), b"first segment ").unwrap();
        std::fs::write(dir.path().join("00002.ts"), encrypted).unwrap();
        let join = Join::parse(
            "#EXTM3U\n00001.ts\n#EXT-X-KEY:METHOD=AES-128,URI=\"key1.key\"\n00002.ts\n#EXT-X-ENDLIST\n",
        )
        .unwrap();
        let mut out = Vec::new();
        join.write(dir.path(), &mut out, &AtomicBool::new(false))
            .unwrap();
        assert_eq!(out, b"first segment second segment");
        assert!(
            Join::parse("#EXTM3U\n../x.ts\n")
                .unwrap()
                .write(dir.path(), &mut Vec::new(), &AtomicBool::new(false))
                .is_err()
        );
    }
}
