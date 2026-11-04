//! Lyrics kept on this device (M46.46). What a source gives for a song is kept by its track key, so
//! it shows at once next time and offline; none found is remembered for a day. A song with no
//! lyrics anywhere is asked again with simpler words (its core title, its first artist). Kept
//! lyrics and transcriptions of songs not played for `LyricsCachePrefs::forget_after_days` are
//! forgotten by a daily sweep, except Favorites, downloaded songs and local files (each a switch).

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use raygun_db::lyrics_cache as store;
use raygun_model::{Lyrics, LyricsCacheUsage, LyricsClearScope, TrackQuery};
use tauri::{AppHandle, Manager};

use crate::extensions::ExtensionService;
use crate::startup::AppDb;

/// "None found" is believed this long before asking the sources again.
const NONE_FOR_MS: i64 = 24 * 60 * 60 * 1000;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// How often the sweep runs.
const SWEEP_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

pub struct LyricsStore {
    app: AppHandle,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

fn to_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// The words to ask again with when a song has no lyrics: its core title (without "(Remastered)",
/// "feat." and the like) and its first artist, without album or length.
pub fn simpler_query(query: &TrackQuery) -> TrackQuery {
    let first_artist = query.artist.as_deref().map(|artist| {
        let lower = artist.to_lowercase();
        let cut = [", ", " & ", " x ", " feat", " ft.", " and ", " with "]
            .iter()
            .filter_map(|separator| lower.find(separator))
            .min()
            .unwrap_or(artist.len());
        artist[..cut].trim().to_owned()
    });
    TrackQuery {
        title: raygun_resolver::core_title(&query.title),
        artist: first_artist,
        album: None,
        duration_ms: None,
    }
}

impl LyricsStore {
    pub fn new(app: AppHandle) -> Arc<Self> {
        Arc::new(Self { app })
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().0.clone()
    }

    /// A source's lyrics for a song: kept ones at once, else asked for (with simpler words when
    /// none come) and kept. Every source answers, so the translations any of them have are kept
    /// too (M46.48); `LyricsTranslations::attach` lines them up. While incognito is on (M47) kept
    /// ones are read, but nothing is kept (translations only in memory) and no song is marked as
    /// shown.
    pub async fn lyrics(&self, query: &TrackQuery) -> Result<Option<Lyrics>, String> {
        let key = raygun_resolver::track_key(query);
        let now = now_ms();
        let incognito = crate::privacy::incognito(&self.app);
        let kept = if incognito {
            store::peek(&self.db(), &key).await
        } else {
            store::get(&self.db(), &key, now).await
        };
        if let Some(kept) = kept.map_err(to_message)? {
            match kept.lyrics.as_deref() {
                Some(json) => {
                    if let Ok(lyrics) = serde_json::from_str::<Lyrics>(json) {
                        return Ok(Some(Lyrics {
                            translations: Vec::new(),
                            ..lyrics
                        }));
                    }
                }
                None if kept.fetched_at > 0 && now - kept.fetched_at < NONE_FOR_MS => {
                    return Ok(None);
                }
                None => {}
            }
        }
        let extensions = self.app.state::<Arc<ExtensionService>>().inner().clone();
        let mut answers = extensions.all_lyrics(query).await?;
        if answers.is_empty() {
            let simpler = simpler_query(query);
            if simpler.title != query.title || simpler.artist != query.artist {
                tracing::info!(song = %key, "no lyrics; asking again with simpler words");
                answers = extensions.all_lyrics(&simpler).await?;
            }
        }
        self.keep_sources(query, &answers).await;
        let found = answers.into_iter().next().map(|lyrics| Lyrics {
            translations: Vec::new(),
            ..lyrics
        });
        let json = found
            .as_ref()
            .and_then(|lyrics| serde_json::to_string(lyrics).ok());
        if !incognito && let Err(error) = store::put(&self.db(), &key, json.as_deref(), now).await {
            tracing::warn!(%error, "couldn't keep a song's lyrics");
        }
        Ok(found)
    }

    /// Keeps the translations lyrics sources gave for a song, in place of what the same sources
    /// gave before (M46.48); only in memory while incognito is on.
    pub async fn keep_sources(&self, query: &TrackQuery, answers: &[Lyrics]) {
        let key = raygun_resolver::track_key(query);
        let answered: Vec<String> = answers
            .iter()
            .map(|lyrics| format!("{}{}", store::SOURCE_PREFIX, lyrics.provider_name))
            .collect();
        let rows = crate::lyrics_translation::source_rows(answers, now_ms());
        if crate::privacy::incognito(&self.app) {
            self.app
                .state::<Arc<crate::lyrics_translation::LyricsTranslations>>()
                .hold(&key, &answered, rows);
            return;
        }
        if let Err(error) =
            store::replace_source_translations(&self.db(), &key, &answered, &rows).await
        {
            tracing::warn!(%error, "couldn't keep a song's lyrics translations");
        }
    }

    /// Marks a song's lyrics as shown (its own lyrics showed, so nothing was asked for); not while
    /// incognito is on (M47).
    pub async fn touch(&self, query: &TrackQuery) {
        if crate::privacy::incognito(&self.app) {
            return;
        }
        let key = raygun_resolver::track_key(query);
        let _ = store::touch(&self.db(), &key, now_ms()).await;
    }

    /// Forgets everything kept of a song's lyrics: the source's, the transcription and the
    /// song's own lyrics (the lyrics menu's Delete saved lyrics).
    pub async fn forget(&self, query: &TrackQuery) -> Result<(), String> {
        let key = raygun_resolver::track_key(query);
        let db = self.db();
        store::remove(&db, std::slice::from_ref(&key))
            .await
            .map_err(to_message)?;
        raygun_db::captions::clear(&db, &crate::captions::song_key(query))
            .await
            .map_err(to_message)?;
        let sound = self.app.state::<Arc<crate::sound::SoundService>>();
        let mut edit = sound.edit(&crate::sound::key(query));
        if edit.lyrics.is_some() {
            edit.lyrics = None;
            sound.set(query, edit).await?;
        }
        Ok(())
    }

    /// How much is kept.
    pub async fn usage(&self) -> Result<LyricsCacheUsage, String> {
        let db = self.db();
        Ok(LyricsCacheUsage {
            songs: store::keys(&db).await.map_err(to_message)?.len() as u32,
            transcriptions: raygun_db::captions::song_keys(&db)
                .await
                .map_err(to_message)?
                .len() as u32,
            translations: store::translated_keys(&db).await.map_err(to_message)?.len() as u32,
        })
    }

    /// Clears kept lyrics (and transcriptions when asked) of the songs `scope` names; returns how
    /// many songs lost something.
    pub async fn clear(
        &self,
        scope: LyricsClearScope,
        transcriptions: bool,
    ) -> Result<u32, String> {
        let db = self.db();
        // Translations go with the lyrics they translate (M46.48).
        let mut lyrics_keys = store::keys(&db).await.map_err(to_message)?;
        let with_lyrics: HashSet<String> = lyrics_keys.iter().cloned().collect();
        lyrics_keys.extend(
            store::translated_keys(&db)
                .await
                .map_err(to_message)?
                .into_iter()
                .filter(|key| !with_lyrics.contains(key)),
        );
        let heard: Vec<String> = if transcriptions {
            raygun_db::captions::song_keys(&db)
                .await
                .map_err(to_message)?
        } else {
            Vec::new()
        };
        let chosen = |key: &str, sets: &Places| match scope {
            LyricsClearScope::All => true,
            LyricsClearScope::NotSaved => !sets.any(key),
            LyricsClearScope::Favorites => sets.favorites.contains(key),
            LyricsClearScope::Saved => sets.saved.contains(key),
            LyricsClearScope::Downloaded => sets.downloaded.contains(key),
        };
        let places = self.places().await?;
        let gone: Vec<String> = lyrics_keys
            .into_iter()
            .filter(|key| chosen(key, &places))
            .collect();
        store::remove(&db, &gone).await.map_err(to_message)?;
        let mut cleared: HashSet<String> = gone.into_iter().collect();
        for episode in heard {
            let key = episode.strip_prefix("song:").unwrap_or(&episode).to_owned();
            if chosen(&key, &places) {
                raygun_db::captions::clear(&db, &episode)
                    .await
                    .map_err(to_message)?;
                cleared.insert(key);
            }
        }
        Ok(cleared.len() as u32)
    }

    /// Where the user keeps songs, by track key.
    async fn places(&self) -> Result<Places, String> {
        let db = self.db();
        let favorites = raygun_db::collections::favorites(&db)
            .await
            .map_err(to_message)?
            .into_iter()
            .map(|row| row.track.track_key)
            .collect();
        let saved: HashSet<String> = raygun_db::collections::saved_keys(&db)
            .await
            .map_err(to_message)?
            .into_iter()
            .collect();
        let songs = self
            .app
            .state::<Arc<crate::music_downloads::SongDownloadService>>();
        let downloaded = songs.downloaded_keys().into_iter().collect();
        let local = raygun_db::local_music::track_keys(&db)
            .await
            .map_err(to_message)?
            .into_iter()
            .collect();
        Ok(Places {
            saved,
            favorites,
            downloaded,
            local,
        })
    }

    /// Forgets kept lyrics and transcriptions of songs not shown for the set number of days,
    /// except the places the settings keep.
    async fn sweep(&self) -> Result<usize, String> {
        let prefs = crate::settings::load(&self.db())
            .await
            .map_err(to_message)?
            .lyrics_cache;
        if prefs.forget_after_days == 0 {
            return Ok(0);
        }
        let db = self.db();
        let before = now_ms() - i64::from(prefs.forget_after_days) * DAY_MS;
        let places = self.places().await?;
        let kept = |key: &str| {
            (prefs.keep_favorites && places.favorites.contains(key))
                || (prefs.keep_downloaded && places.downloaded.contains(key))
                || (prefs.keep_local && places.local.contains(key))
        };
        let stale: Vec<String> = store::stale(&db, before)
            .await
            .map_err(to_message)?
            .into_iter()
            .filter(|key| !kept(key))
            .collect();
        for key in &stale {
            raygun_db::captions::clear(&db, &format!("song:{key}"))
                .await
                .map_err(to_message)?;
        }
        store::remove(&db, &stale).await.map_err(to_message)?;
        Ok(stale.len())
    }

    /// Sweeps a while after start, then once a day.
    pub fn start(self: &Arc<Self>) {
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(120)).await;
            loop {
                match service.sweep().await {
                    Ok(0) => {}
                    Ok(count) => tracing::info!(count, "forgot lyrics of songs not played lately"),
                    Err(error) => tracing::warn!(%error, "couldn't sweep kept lyrics"),
                }
                tokio::time::sleep(SWEEP_EVERY).await;
            }
        });
    }
}

struct Places {
    favorites: HashSet<String>,
    /// Favorites and playlists.
    saved: HashSet<String>,
    downloaded: HashSet<String>,
    local: HashSet<String>,
}

impl Places {
    fn any(&self, key: &str) -> bool {
        self.saved.contains(key) || self.downloaded.contains(key) || self.local.contains(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simpler_words_drop_extras_and_other_artists() {
        let query = TrackQuery {
            title: "Get Lucky (Radio Edit) [feat. Pharrell Williams]".into(),
            artist: Some("Daft Punk, Pharrell Williams".into()),
            album: Some("Random Access Memories".into()),
            duration_ms: Some(248_000),
        };
        let simpler = simpler_query(&query);
        assert_eq!(simpler.title, "get lucky");
        assert_eq!(simpler.artist.as_deref(), Some("Daft Punk"));
        assert!(simpler.album.is_none() && simpler.duration_ms.is_none());
    }
}
