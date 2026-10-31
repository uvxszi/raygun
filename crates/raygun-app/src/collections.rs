//! Favorites, playlists and play history (M18). Saved tracks are metadata that works on any
//! device: playing one prefers the same song in the local library, then a stored portable stream,
//! then the resolver.

use std::collections::HashMap;

use raygun_db::collections::{self, TrackFields};
use raygun_db::local_music::TrackRow;
use raygun_model::{
    Collection, CollectionPlayRequest, HistoryEntry, MediaKind, MediaSource, PlaylistDetail,
    PlaylistItem, PlaylistSummary, QueueItem, QueueItemInput, SavedTrack, TrackInput, TrackQuery,
};
use tauri::{AppHandle, Manager};

use crate::library::LibraryService;
use crate::playback::PlaybackService;
use crate::startup::AppDb;

/// Distinct artworks on a playlist cover.
const COVER_ARTWORKS: usize = 4;
/// Longest playlist name kept.
const MAX_NAME_CHARS: usize = 100;

pub struct CollectionsService {
    app: AppHandle,
}

impl CollectionsService {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    fn db(&self) -> &raygun_db::Db {
        &self.app.state::<AppDb>().inner().0
    }

    // ---- Favorites ----

    pub async fn favorites(&self) -> Result<Vec<SavedTrack>, String> {
        let rows = collections::favorites(self.db())
            .await
            .map_err(to_message)?;
        self.saved(rows.into_iter().map(|row| row.track).collect())
            .await
    }

    pub async fn is_favorite(&self, track: TrackInput) -> Result<bool, String> {
        let key = fields(track).track_key;
        collections::is_favorite(self.db(), &key)
            .await
            .map_err(to_message)
    }

    pub async fn set_favorite(&self, track: TrackInput, favorite: bool) -> Result<(), String> {
        collections::set_favorite(self.db(), &fields(track), favorite, now_ms())
            .await
            .map_err(to_message)
    }

    // ---- Playlists ----

    pub async fn playlists(&self) -> Result<Vec<PlaylistSummary>, String> {
        let rows = collections::playlists(self.db())
            .await
            .map_err(to_message)?;
        let mut summaries = Vec::with_capacity(rows.len());
        for row in rows {
            summaries.push(self.summary(row).await?);
        }
        Ok(summaries)
    }

    pub async fn playlist(&self, id: &str) -> Result<PlaylistDetail, String> {
        let db = self.db();
        let row = collections::playlist(db, id)
            .await
            .map_err(to_message)?
            .ok_or_else(|| "This playlist no longer exists".to_owned())?;
        let rows = collections::playlist_items(db, id)
            .await
            .map_err(to_message)?;
        let ids: Vec<String> = rows.iter().map(|row| row.id.clone()).collect();
        let tracks = self
            .saved(rows.into_iter().map(|row| row.track).collect())
            .await?;
        Ok(PlaylistDetail {
            playlist: self.summary(row).await?,
            items: ids
                .into_iter()
                .zip(tracks)
                .map(|(id, track)| PlaylistItem { id, track })
                .collect(),
        })
    }

    pub async fn create_playlist(&self, name: &str) -> Result<PlaylistSummary, String> {
        let name = playlist_name(name)?;
        let db = self.db();
        let id = collections::create_playlist(db, &name, now_ms())
            .await
            .map_err(to_message)?;
        let row = collections::playlist(db, &id)
            .await
            .map_err(to_message)?
            .ok_or_else(|| "The new playlist went missing".to_owned())?;
        self.summary(row).await
    }

    pub async fn rename_playlist(&self, id: &str, name: &str) -> Result<(), String> {
        let name = playlist_name(name)?;
        if collections::rename_playlist(self.db(), id, &name)
            .await
            .map_err(to_message)?
        {
            Ok(())
        } else {
            Err("This playlist no longer exists".into())
        }
    }

    pub async fn delete_playlist(&self, id: &str) -> Result<(), String> {
        collections::delete_playlist(self.db(), id)
            .await
            .map_err(to_message)
    }

    pub async fn add_to_playlist(&self, id: &str, tracks: Vec<TrackInput>) -> Result<(), String> {
        let db = self.db();
        if collections::playlist(db, id)
            .await
            .map_err(to_message)?
            .is_none()
        {
            return Err("This playlist no longer exists".into());
        }
        let fields: Vec<TrackFields> = tracks.into_iter().map(fields).collect();
        collections::add_to_playlist(db, id, &fields, now_ms())
            .await
            .map_err(to_message)
    }

    pub async fn remove_playlist_item(&self, item_id: &str) -> Result<(), String> {
        collections::remove_playlist_item(self.db(), item_id)
            .await
            .map_err(to_message)
    }

    pub async fn move_playlist_item(
        &self,
        item_id: &str,
        before_item_id: Option<&str>,
    ) -> Result<(), String> {
        collections::move_playlist_item(self.db(), item_id, before_item_id)
            .await
            .map_err(to_message)
    }

    // ---- History ----

    pub async fn history(&self, limit: u32) -> Result<Vec<HistoryEntry>, String> {
        let rows = collections::recent_history(self.db(), limit.into())
            .await
            .map_err(to_message)?;
        let played: Vec<i64> = rows.iter().map(|row| row.played_at).collect();
        let tracks = self
            .saved(rows.into_iter().map(|row| row.track).collect())
            .await?;
        Ok(tracks
            .into_iter()
            .zip(played)
            .map(|(track, played_at)| HistoryEntry {
                track,
                played_at: played_at as f64,
            })
            .collect())
    }

    pub async fn clear_history(&self) -> Result<(), String> {
        collections::clear_history(self.db())
            .await
            .map_err(to_message)
    }

    /// Writes a play to history in the background; failures only log.
    pub fn record_play(&self, item: QueueItem) {
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let service = app.state::<CollectionsService>();
            let track = fields(TrackInput {
                title: item.title,
                artist: item.artist,
                album: item.album,
                duration_ms: item.duration_ms,
                artwork_url: item.artwork_url,
                source: Some(item.source),
            });
            tracing::info!(track = %track.track_key, "play counted");
            if let Err(error) = collections::record_play(service.db(), &track, now_ms()).await {
                tracing::warn!(%error, "failed to record a play");
            }
        });
    }

    // ---- Playing ----

    pub async fn play(&self, request: CollectionPlayRequest) -> Result<(), String> {
        let tracks = match &request.collection {
            Collection::Favorites => self.favorites().await?,
            Collection::Playlist { id } => self
                .playlist(id)
                .await?
                .items
                .into_iter()
                .map(|item| item.track)
                .collect(),
            Collection::History => self
                .history(HISTORY_PLAY_LIMIT)
                .await?
                .into_iter()
                .map(|entry| entry.track)
                .collect(),
        };
        if tracks.is_empty() {
            return Err("Nothing to play here yet".into());
        }
        let keys: Vec<String> = tracks.iter().map(|track| track.track_key.clone()).collect();
        let mut local = self.local_matches(&keys).await?;
        let library = self.app.state::<LibraryService>();
        let items = tracks
            .into_iter()
            .map(|track| match local.remove(&track.track_key) {
                Some(row) => library.queue_item(row),
                None => queue_item(track),
            })
            .collect();
        self.app.state::<PlaybackService>().replace(
            items,
            request.start_index.map(|index| index as usize),
            request.shuffle,
        );
        Ok(())
    }

    /// A song to play or queue: the library's file when it has the song, else a stream the
    /// track came with, else a lookup for the resolver.
    pub async fn queue_item_for(&self, track: TrackInput) -> Result<QueueItemInput, String> {
        let saved = fields(track);
        let key = saved.track_key.clone();
        if let Some(row) = self
            .local_matches(std::slice::from_ref(&key))
            .await?
            .remove(&key)
        {
            return Ok(self.app.state::<LibraryService>().queue_item(row));
        }
        let mut tracks = self.saved(vec![saved]).await?;
        Ok(queue_item(tracks.remove(0)))
    }

    // ---- Helpers ----

    async fn local_matches(&self, keys: &[String]) -> Result<HashMap<String, TrackRow>, String> {
        self.app.state::<LibraryService>().matches(keys).await
    }

    /// Stored rows as saved tracks, with local artwork where the song is in the library.
    async fn saved(&self, rows: Vec<TrackFields>) -> Result<Vec<SavedTrack>, String> {
        let keys: Vec<String> = rows.iter().map(|row| row.track_key.clone()).collect();
        let local = self.local_matches(&keys).await?;
        let library = self.app.state::<LibraryService>();
        Ok(rows
            .into_iter()
            .map(|row| {
                let found = local.get(&row.track_key);
                let local_art = found.and_then(|track| library.artwork_url(track.art.clone()));
                SavedTrack {
                    local: found.is_some(),
                    artwork_url: local_art.or(row.artwork_url),
                    track_key: row.track_key,
                    title: row.title,
                    artist: row.artist,
                    album: row.album,
                    duration_ms: row.duration_ms.map(|duration| duration as u32),
                    source: row
                        .source
                        .and_then(|json| serde_json::from_str(&json).ok())
                        .and_then(portable),
                }
            })
            .collect())
    }

    async fn summary(&self, row: collections::PlaylistRow) -> Result<PlaylistSummary, String> {
        let mut artwork_urls: Vec<String> = Vec::new();
        for track in self.saved(row.first_items).await? {
            if let Some(url) = track.artwork_url
                && !artwork_urls.contains(&url)
            {
                artwork_urls.push(url);
            }
        }
        artwork_urls.truncate(COVER_ARTWORKS);
        Ok(PlaylistSummary {
            id: row.id,
            name: row.name,
            item_count: row.item_count as u32,
            artwork_urls,
        })
    }
}

/// Plays from history take this many recent tracks.
const HISTORY_PLAY_LIMIT: u32 = 200;

/// What gets stored for a track: its key, and only sources and artwork that work elsewhere.
pub(crate) fn fields(track: TrackInput) -> TrackFields {
    let track_key = raygun_resolver::track_key(&TrackQuery {
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: None,
        duration_ms: None,
    });
    TrackFields {
        track_key,
        title: track.title,
        artist: track.artist,
        album: track.album,
        duration_ms: track.duration_ms.map(i64::from),
        artwork_url: track.artwork_url.filter(|url| portable_url(url)),
        source: track
            .source
            .and_then(portable)
            .and_then(|source| serde_json::to_string(&source).ok()),
    }
}

/// A source that means the same on another device.
fn portable(source: MediaSource) -> Option<MediaSource> {
    match &source {
        MediaSource::Url { url, .. } if portable_url(url) => Some(source),
        MediaSource::YouTube { .. } => Some(source),
        _ => None,
    }
}

/// An http(s) URL that isn't served by this device (the stream proxy, artwork server).
pub(crate) fn portable_url(url: &str) -> bool {
    let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    else {
        return false;
    };
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .rsplit('@')
        .next()
        .unwrap_or_default();
    let host = host
        .strip_prefix('[')
        .and_then(|bracketed| bracketed.split(']').next())
        .unwrap_or_else(|| host.split(':').next().unwrap_or_default())
        .to_ascii_lowercase();
    !host.is_empty() && host != "localhost" && !host.starts_with("127.") && host != "::1"
}

/// A saved track not in the local library, as a queue item.
fn queue_item(track: SavedTrack) -> QueueItemInput {
    let source = track.source.unwrap_or_else(|| MediaSource::Lookup {
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: track.album.clone(),
        duration_ms: track.duration_ms,
    });
    QueueItemInput {
        media_kind: match source {
            MediaSource::YouTube { .. } => MediaKind::Video,
            _ => MediaKind::Audio,
        },
        title: track.title,
        artist: track.artist,
        album: track.album,
        duration_ms: track.duration_ms,
        artwork_url: track.artwork_url,
        source,
    }
}

fn playlist_name(name: &str) -> Result<String, String> {
    let name: String = name.trim().chars().take(MAX_NAME_CHARS).collect();
    if name.is_empty() {
        Err("A playlist needs a name".into())
    } else {
        Ok(name)
    }
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn input(source: Option<MediaSource>, artwork: Option<&str>) -> TrackInput {
        TrackInput {
            title: "Feeling Good (Remastered)".into(),
            artist: Some("Nina Simone".into()),
            album: Some("I Put a Spell on You".into()),
            duration_ms: Some(170_000),
            artwork_url: artwork.map(String::from),
            source,
        }
    }

    fn url(url: &str) -> MediaSource {
        MediaSource::Url {
            url: url.into(),
            headers: BTreeMap::from([("Referer".into(), "https://archive.org".into())]),
        }
    }

    #[test]
    fn only_portable_sources_and_artwork_are_kept() {
        let kept = fields(input(
            Some(url("https://archive.org/x.mp3")),
            Some("https://coverartarchive.org/a.jpg"),
        ));
        assert_eq!(kept.track_key, "nina simone|feeling good");
        assert!(kept.source.unwrap().contains("archive.org/x.mp3"));
        assert_eq!(
            kept.artwork_url.as_deref(),
            Some("https://coverartarchive.org/a.jpg")
        );

        let youtube = fields(input(
            Some(MediaSource::YouTube {
                video_id: "abc".into(),
            }),
            None,
        ));
        assert!(youtube.source.unwrap().contains("abc"));

        for source in [
            MediaSource::LocalFile {
                path: "/sdcard/Music/a.flac".into(),
            },
            MediaSource::Lookup {
                title: "x".into(),
                artist: None,
                album: None,
                duration_ms: None,
            },
            url("http://127.0.0.1:41000/stream/1"),
            url("file:///sdcard/a.mp3"),
        ] {
            assert_eq!(
                fields(input(Some(source.clone()), None)).source,
                None,
                "{source:?}"
            );
        }
        for artwork in [
            "http://127.0.0.1:40371/art/cover.png",
            "http://localhost:1/a.png",
            "http://[::1]:8/a.png",
            "data:image/png;base64,xx",
        ] {
            assert_eq!(
                fields(input(None, Some(artwork))).artwork_url,
                None,
                "{artwork}"
            );
        }
    }

    #[test]
    fn saved_tracks_without_a_source_play_through_the_resolver() {
        let track = SavedTrack {
            track_key: "a|b".into(),
            title: "B".into(),
            artist: Some("A".into()),
            album: None,
            duration_ms: Some(1000),
            artwork_url: None,
            source: None,
            local: false,
        };
        let item = queue_item(track.clone());
        assert!(matches!(item.source, MediaSource::Lookup { ref title, .. } if title == "B"));

        let video = queue_item(SavedTrack {
            source: Some(MediaSource::YouTube {
                video_id: "v".into(),
            }),
            ..track
        });
        assert_eq!(video.media_kind, MediaKind::Video);
    }

    #[test]
    fn playlist_names_are_trimmed_and_required() {
        assert_eq!(playlist_name("  Mix  ").unwrap(), "Mix");
        assert!(playlist_name("   ").is_err());
        assert_eq!(
            playlist_name(&"x".repeat(300)).unwrap().len(),
            MAX_NAME_CHARS
        );
    }
}
