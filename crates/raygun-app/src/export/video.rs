//! Downloaded videos as files (M46.1). A plain download is copied out as it is. An HLS download
//! (segments, keys and init maps saved beside a playlist) is joined into one file: MPEG-TS
//! segments into a `.ts`, fragmented MP4 (an `#EXT-X-MAP` init section) into an `.mp4`, with
//! AES-128 segments decrypted by their saved keys. A stream whose audio is a separate rendition
//! would need remuxing, so it's refused. Saved subtitle tracks go beside the video.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use aes::cipher::BlockDecryptMut;
use aes::cipher::KeyIvInit;
use aes::cipher::block_padding::Pkcs7;
use raygun_db::titles::{self, TitleRow, UnitFields, UnitRow};
use raygun_db::video_downloads::{self as store, VideoDownloadRow};
use raygun_model::{ExportRequest, VideoSubtitle};
use tauri::{AppHandle, Manager};

use super::{Chosen, ExportService, Place, Planned, names, output};
use crate::catalog::to_message;
use crate::startup::{AppDb, AppPaths};

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
    /// Reads a saved playlist. A master playlist (separate audio) can't be joined.
    pub fn parse(playlist: &str) -> Result<Self, String> {
        if playlist.contains("#EXT-X-STREAM-INF") {
            return Err(
                "This video's sound is a separate stream, so it can't be saved as one file yet"
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
        let mut keys: Vec<(String, [u8; 16])> = Vec::new();
        for part in &self.parts {
            if cancel.load(Ordering::SeqCst) {
                return Err(std::io::Error::other("Cancelled"));
            }
            match part {
                Part::Map(name) => out.write_all(&std::fs::read(safe_join(dir, name)?)?)?,
                Part::Segment { name, key: None } => {
                    std::io::copy(&mut File::open(safe_join(dir, name)?)?, out)?;
                }
                Part::Segment {
                    name,
                    key: Some((file, iv)),
                } => {
                    let key = match keys.iter().find(|(known, _)| known == file) {
                        Some((_, key)) => *key,
                        None => {
                            let mut key = [0u8; 16];
                            File::open(safe_join(dir, file)?)?.read_exact(&mut key)?;
                            keys.push((file.clone(), key));
                            key
                        }
                    };
                    let mut data = std::fs::read(safe_join(dir, name)?)?;
                    let plain = Decryptor::new(&key.into(), &(*iv).into())
                        .decrypt_padded_mut::<Pkcs7>(&mut data)
                        .map_err(|_| std::io::Error::other("A segment couldn't be decrypted"))?;
                    out.write_all(plain)?;
                }
            }
        }
        Ok(())
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

/// The episode a download is, as an export unit: its id is the download's.
fn as_unit(row: &VideoDownloadRow) -> UnitRow {
    UnitRow {
        id: row.id.clone(),
        title_id: row.download.title_id.clone().unwrap_or_default(),
        fields: UnitFields {
            remote_id: row.id.clone(),
            name: row
                .download
                .subtitle
                .clone()
                .unwrap_or_else(|| row.download.title.clone()),
            number: None,
            volume: None,
            scanlator: None,
            date: None,
        },
        source_order: 0,
        position: None,
        total: None,
        completed_at: None,
    }
}

/// Finished downloads of `title_id`.
async fn finished(app: &AppHandle, title_id: &str) -> Result<Vec<VideoDownloadRow>, String> {
    let db = app.state::<AppDb>().inner().0.clone();
    Ok(store::all(&db)
        .await
        .map_err(to_message)?
        .into_iter()
        .filter(|row| row.state == "done" && row.download.title_id.as_deref() == Some(title_id))
        .collect())
}

/// Why a download can't be written as one file, if so.
fn problem(app: &AppHandle, row: &VideoDownloadRow) -> Option<String> {
    let file = row.file.as_deref()?;
    if !file.ends_with(".m3u8") {
        return None;
    }
    let playlist = std::fs::read_to_string(downloads_dir(app).join(&row.id).join(file)).ok()?;
    Join::parse(&playlist).err()
}

/// Videos export their finished downloads; nothing is fetched first (each needs a link picked).
pub(super) async fn plan(app: &AppHandle, request: &ExportRequest) -> Result<Planned, String> {
    let db = app.state::<AppDb>().inner().0.clone();
    let mut chosen = Vec::new();
    let mut problems = Vec::new();
    for title_id in &request.title_ids {
        let title = titles::title(&db, title_id)
            .await
            .map_err(to_message)?
            .ok_or_else(|| "A title is no longer stored".to_owned())?;
        let rows = finished(app, title_id).await?;
        problems.extend(rows.iter().filter_map(|row| problem(app, row)));
        chosen.push(Chosen {
            title,
            units: rows.iter().map(as_unit).collect(),
        });
    }
    let files: u32 = chosen.iter().map(|title| title.units.len() as u32).sum();
    let problem = if files == 0 {
        Some("Download episodes first: only downloaded ones can be saved".to_owned())
    } else if request.destination == raygun_model::ExportDestination::Share && files != 1 {
        Some("Sharing sends one file: export a single episode".to_owned())
    } else {
        problems.into_iter().next()
    };
    Ok(Planned {
        titles: chosen,
        missing: Vec::new(),
        files,
        problem,
    })
}

/// Writes each episode as one video file, its subtitles beside it.
pub(super) async fn write(
    service: &ExportService,
    job: &str,
    title: &TitleRow,
    units: &[UnitRow],
    place: &Place,
    cancel: &AtomicBool,
) -> Result<Vec<output::Saved>, String> {
    let app = service.app.clone();
    let db = app.state::<AppDb>().inner().0.clone();
    let mut saved = Vec::new();
    for unit in units {
        if cancel.load(Ordering::SeqCst) {
            return Err("Cancelled".into());
        }
        let row = store::get(&db, &unit.id)
            .await
            .map_err(to_message)?
            .ok_or("A download is gone")?;
        let files = write_row(service, &app, &row, title, place, cancel).await?;
        saved.extend(files);
        service.update(job, |job| job.written += 1);
    }
    Ok(saved)
}

async fn write_row(
    service: &ExportService,
    app: &AppHandle,
    row: &VideoDownloadRow,
    title: &TitleRow,
    place: &Place,
    cancel: &AtomicBool,
) -> Result<Vec<output::Saved>, String> {
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
    let cancel_flag = std::sync::Arc::new(AtomicBool::new(cancel.load(Ordering::SeqCst)));
    tauri::async_runtime::spawn_blocking(move || {
        let mut saved = Vec::new();
        if file.ends_with(".m3u8") {
            let playlist = std::fs::read_to_string(dir.join(&file)).map_err(to_message)?;
            let join = Join::parse(&playlist)?;
            let extension = join.extension();
            let name = names::file_name(&stem, extension);
            let mime = if extension == "mp4" {
                "video/mp4"
            } else {
                "video/mp2t"
            };
            let (video, _) = super::write_one(&outputs, &place, &folder, &name, mime, |out| {
                let mut out = std::io::BufWriter::new(out);
                join.write(&dir, &mut out, &cancel_flag)?;
                out.flush().map(|()| 0)
            })?;
            saved.push(video);
        } else {
            let extension = Path::new(&file)
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or("mp4")
                .to_owned();
            let name = names::file_name(&stem, &extension);
            let mime = match extension.as_str() {
                "mkv" => "video/x-matroska",
                "webm" => "video/webm",
                "ts" => "video/mp2t",
                _ => "video/mp4",
            };
            let source = safe_join(&dir, &file).map_err(to_message)?;
            let (video, _) = super::write_one(&outputs, &place, &folder, &name, mime, |out| {
                std::io::copy(&mut File::open(&source)?, out).map(|_| 0)
            })?;
            saved.push(video);
        }
        for track in subtitles {
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
        Ok::<_, String>(saved)
    })
    .await
    .map_err(to_message)?
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
            write_row(
                &service,
                &app,
                &row,
                &title,
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
    use super::*;

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
