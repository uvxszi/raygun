//! Sound (M46.12): each song's own trims, volume and lyrics edits, and the loudness songs measured
//! while they played. Both are cached in memory for the playback service, which reads them for
//! every load; edits to the playing song reach its engine at once.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use raygun_db::song_edits::{self as store, SongEditRow};
use raygun_model::{EditedLyrics, LyricLine, Lyrics, SongEdit, SoundPrefs, TrackQuery};
use tauri::{AppHandle, Manager};

use crate::catalog::{now_ms, to_message};
use crate::playback::PlaybackService;
use crate::startup::AppDb;

/// The name shown for lyrics a person wrote or corrected.
pub const OWN_LYRICS: &str = "Your lyrics";
/// Normalization never boosts a quiet song more than this, dB; the limiter catches the rest.
const MAX_BOOST_DB: f64 = 10.0;
/// ... nor quietens a loud one more than this.
const MAX_CUT_DB: f64 = 20.0;
/// Measurements outside this range are a silent or broken stream, not a song.
const PLAUSIBLE_LUFS: std::ops::RangeInclusive<f64> = -50.0..=5.0;

#[derive(Default)]
struct Cache {
    edits: HashMap<String, SongEdit>,
    loudness: HashMap<String, f64>,
}

/// How a song plays: where it starts and ends, and its gain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tuning {
    pub start_ms: f64,
    pub end_ms: Option<f64>,
    pub gain_db: f64,
    /// Its loudness isn't known yet: the engine measures it.
    pub measure: bool,
}

pub struct SoundService {
    app: AppHandle,
    cache: Mutex<Cache>,
}

/// The key edits and loudness are kept under.
pub fn key(query: &TrackQuery) -> String {
    raygun_resolver::track_key(query)
}

impl SoundService {
    pub fn new(app: AppHandle) -> Arc<Self> {
        Arc::new(Self {
            app,
            cache: Mutex::new(Cache::default()),
        })
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().inner().0.clone()
    }

    /// Loads the edits and measurements, and gives the playback service the sound settings.
    pub async fn start(self: &Arc<Self>) {
        let db = self.db();
        let edits = store::all(&db).await.unwrap_or_else(|error| {
            tracing::warn!(%error, "song edits unreadable");
            Vec::new()
        });
        let loudness = store::loudness_all(&db).await.unwrap_or_default();
        {
            let mut cache = self.cache.lock().expect("sound cache poisoned");
            for row in edits {
                let key = row.track_key.clone();
                cache.edits.insert(key, from_row(row));
            }
            cache.loudness.extend(loudness);
        }
        match crate::settings::load(&db).await {
            Ok(settings) => self
                .app
                .state::<PlaybackService>()
                .set_sound(settings.sound),
            Err(error) => tracing::warn!(%error, "could not read the sound settings"),
        }
    }

    pub fn edit(&self, key: &str) -> SongEdit {
        self.cache
            .lock()
            .expect("sound cache poisoned")
            .edits
            .get(key)
            .cloned()
            .unwrap_or_default()
    }

    /// How the song with `key` plays under `sound`.
    pub fn tuning(&self, key: &str, sound: &SoundPrefs) -> Tuning {
        let (edit, lufs) = {
            let cache = self.cache.lock().expect("sound cache poisoned");
            (
                cache.edits.get(key).cloned().unwrap_or_default(),
                cache.loudness.get(key).copied(),
            )
        };
        let normalization = match lufs {
            Some(lufs) if sound.normalize => {
                (f64::from(sound.normalize_target) - lufs).clamp(-MAX_CUT_DB, MAX_BOOST_DB)
            }
            _ => 0.0,
        };
        Tuning {
            start_ms: f64::from(edit.start_ms),
            end_ms: edit.end_ms.map(f64::from),
            gain_db: f64::from(edit.gain) / 10.0 + normalization,
            measure: lufs.is_none(),
        }
    }

    /// Saves a song's edit (an empty one forgets it) and applies it if the song is playing.
    pub async fn set(&self, query: &TrackQuery, edit: SongEdit) -> Result<SongEdit, String> {
        edit.validate()?;
        let key = key(query);
        let db = self.db();
        if edit.is_empty() {
            store::remove(&db, &key).await.map_err(to_message)?;
        } else {
            store::put(&db, &to_row(&key, &edit)?)
                .await
                .map_err(to_message)?;
        }
        {
            let mut cache = self.cache.lock().expect("sound cache poisoned");
            if edit.is_empty() {
                cache.edits.remove(&key);
            } else {
                cache.edits.insert(key.clone(), edit.clone());
            }
        }
        self.app.state::<PlaybackService>().song_edited(&key);
        Ok(edit)
    }

    /// Remembers a song's measured loudness; nonsense from a silent stream is dropped.
    pub async fn record_loudness(&self, key: String, lufs: f64) {
        if !lufs.is_finite() || !PLAUSIBLE_LUFS.contains(&lufs) {
            tracing::debug!(song = %key, lufs, "loudness out of range, not kept");
            return;
        }
        let rounded = (lufs * 100.0).round() / 100.0;
        if let Err(error) = store::put_loudness(&self.db(), &key, rounded, now_ms()).await {
            tracing::warn!(%error, "could not keep a song's loudness");
            return;
        }
        tracing::debug!(song = %key, lufs = rounded, "loudness measured");
        self.cache
            .lock()
            .expect("sound cache poisoned")
            .loudness
            .insert(key, rounded);
    }

    /// The song's own lyrics when it has them, else `fetched` (a source's) with its offset. Synced
    /// lines its trims cut off are left out (M46.36): only what plays shows.
    pub fn lyrics(&self, query: &TrackQuery, fetched: Option<Lyrics>) -> Option<Lyrics> {
        let edit = self.edit(&key(query));
        let lyrics = if let Some(own) = edit.lyrics.clone() {
            Lyrics {
                provider_name: OWN_LYRICS.to_owned(),
                synced: own.synced,
                plain: own.plain,
                offset_ms: edit.lyrics_offset_ms,
                edited: true,
            }
        } else {
            Lyrics {
                offset_ms: edit.lyrics_offset_ms,
                ..fetched?
            }
        };
        Some(Lyrics {
            synced: within_trims(lyrics.synced, lyrics.offset_ms, &edit),
            ..lyrics
        })
    }

    /// Whether the song has its own lyrics (then no source is asked).
    pub fn has_own_lyrics(&self, query: &TrackQuery) -> bool {
        self.edit(&key(query)).lyrics.is_some()
    }
}

/// Synced lines that show while the trimmed song plays (M46.36): the one already showing at the
/// start trim and every one that begins before the end trim. Times are as shown (`offset_ms` on).
fn within_trims(lines: Vec<LyricLine>, offset_ms: i32, edit: &SongEdit) -> Vec<LyricLine> {
    if edit.start_ms == 0 && edit.end_ms.is_none() {
        return lines;
    }
    let shown = |line: &LyricLine| i64::from(line.time_ms) + i64::from(offset_ms);
    let start = i64::from(edit.start_ms);
    let end = edit.end_ms.map(i64::from).unwrap_or(i64::MAX);
    // The last line beginning at or before the start is showing when the song starts.
    let first = lines
        .iter()
        .rposition(|line| shown(line) <= start)
        .unwrap_or(0);
    lines
        .into_iter()
        .enumerate()
        .filter(|(index, line)| *index >= first && shown(line) < end)
        .map(|(_, line)| line)
        .collect()
}

fn from_row(row: SongEditRow) -> SongEdit {
    SongEdit {
        start_ms: u32::try_from(row.start_ms).unwrap_or(0),
        end_ms: row.end_ms.and_then(|end| u32::try_from(end).ok()),
        gain: i32::try_from(row.gain).unwrap_or(0),
        lyrics_offset_ms: i32::try_from(row.lyrics_offset_ms).unwrap_or(0),
        lyrics: row
            .lyrics
            .as_deref()
            .and_then(|json| serde_json::from_str::<EditedLyrics>(json).ok()),
    }
}

fn to_row(key: &str, edit: &SongEdit) -> Result<SongEditRow, String> {
    Ok(SongEditRow {
        track_key: key.to_owned(),
        start_ms: i64::from(edit.start_ms),
        end_ms: edit.end_ms.map(i64::from),
        gain: i64::from(edit.gain),
        lyrics_offset_ms: i64::from(edit.lyrics_offset_ms),
        lyrics: edit
            .lyrics
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(to_message)?,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn trims_leave_out_the_lines_they_cut() {
        let line = |time_ms: u32| LyricLine {
            time_ms,
            text: time_ms.to_string(),
        };
        let lines = vec![
            line(0),
            line(5_000),
            line(12_000),
            line(20_000),
            line(30_000),
        ];
        let edit = SongEdit {
            start_ms: 10_000,
            end_ms: Some(25_000),
            ..SongEdit::default()
        };
        let kept: Vec<u32> = within_trims(lines.clone(), 0, &edit)
            .iter()
            .map(|line| line.time_ms)
            .collect();
        // 5 s is still showing at the 10 s start; 30 s comes after the end.
        assert_eq!(kept, [5_000, 12_000, 20_000]);
        // An offset moves the lines before they're cut.
        let moved: Vec<u32> = within_trims(lines.clone(), 6_000, &edit)
            .iter()
            .map(|line| line.time_ms)
            .collect();
        assert_eq!(moved, [0, 5_000, 12_000]);
        assert_eq!(
            within_trims(lines.clone(), 0, &SongEdit::default()).len(),
            5
        );
    }

    use super::*;

    #[test]
    fn edits_round_trip_through_rows() {
        let edit = SongEdit {
            start_ms: 2_000,
            end_ms: Some(150_000),
            gain: -35,
            lyrics_offset_ms: -300,
            lyrics: Some(EditedLyrics {
                synced: vec![raygun_model::LyricLine {
                    time_ms: 12_000,
                    text: "Swim".into(),
                }],
                plain: None,
            }),
        };
        let row = to_row("bts|swim", &edit).unwrap();
        assert_eq!(row.track_key, "bts|swim");
        assert_eq!(from_row(row), edit);
    }
}
