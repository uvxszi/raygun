//! Favorites, playlists and play history (M18). Saved tracks are metadata that works on any
//! device: playing one prefers the same song in the local library, then a stored portable stream,
//! then the resolver. A song is found among them by the names any screen shows for it
//! (`identity`, M46.48), so saving, unsaving and the saved marks agree everywhere.

pub mod identity;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use raygun_db::collections::{self, SavedRef, TrackFields};
use raygun_db::local_music::TrackRow;
use raygun_model::{
    Collection, CollectionLength, CollectionPlayRequest, DetailTarget, HistoryEntry, MediaKind,
    MediaSource, OnlineTrack, PlaylistAdded, PlaylistDetail, PlaylistItem, PlaylistSummary,
    QueueItem, QueueItemInput, SavedSong, SavedTrack, SourceChoice, TrackImportStatus, TrackInput,
    TrackQuery, TrackSaves,
};
use tauri::{AppHandle, Manager};

use crate::details::DetailService;
use crate::library::LibraryService;
use crate::playback::PlaybackService;
use crate::startup::AppDb;
use identity::{SavedIndex, SongSet, keys_of};

/// Distinct artworks on a playlist cover.
const COVER_ARTWORKS: usize = 4;
/// Longest playlist name kept.
const MAX_NAME_CHARS: usize = 100;

/// The `local_meta` key of what was played from lately (M46.37), and how many are kept.
const RECENT_CONTEXTS: &str = "music.recentContexts";
const RECENT_CONTEXTS_KEPT: usize = 24;
/// The `local_meta` key of the songs saved or played that their source marked explicit, by
/// song key (the saved tables don't store the mark), and how many are kept.
const EXPLICIT_SONGS: &str = "music.explicitSongs";
const EXPLICIT_SONGS_KEPT: usize = 5000;
/// The `local_meta` key set once saved songs were repaired (M46.48, see [`repair`]).
const REPAIRED: &str = "collections.identityRepaired";

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
        let added: Vec<i64> = rows.iter().map(|row| row.added_at).collect();
        let tracks = self
            .saved(rows.into_iter().map(|row| row.track).collect())
            .await?;
        Ok(with_dates(tracks, added))
    }

    /// Songs downloaded into the app, newest first (M46.18): Library → Audio → Downloaded. They
    /// play from their saved files (`music_downloads::offline_item`).
    pub async fn downloaded(&self) -> Result<Vec<SavedTrack>, String> {
        let rows: Vec<_> = raygun_db::music_downloads::all(self.db())
            .await
            .map_err(to_message)?
            .into_iter()
            .filter(|row| row.state == raygun_db::music_downloads::DONE)
            .collect();
        let added: Vec<i64> = rows
            .iter()
            .map(|row| row.finished_at.unwrap_or(row.created_at))
            .collect();
        let fields = rows
            .into_iter()
            .map(|row| collections::TrackFields {
                track_key: row.song.track_key,
                title: row.song.title,
                artist: row.song.artist,
                album: row.song.album,
                duration_ms: row.song.duration_ms,
                artwork_url: row.song.artwork_url,
                source: row.song.source,
            })
            .collect();
        Ok(with_dates(self.saved(fields).await?, added))
    }

    /// Every saved copy with the keys it answers to (M46.48).
    pub(crate) async fn index(&self) -> Result<SavedIndex, String> {
        let rows = collections::saved_refs(self.db())
            .await
            .map_err(to_message)?;
        let details = self.app.state::<Arc<DetailService>>();
        Ok(SavedIndex::new(rows, |row| {
            details.shown_names(&row.track_key, &row.title, row.artist.as_deref())
        }))
    }

    /// The saved copies of a song, by the names a screen shows for it (M46.48).
    pub(crate) async fn copies(&self, track: &TrackInput) -> Result<Vec<SavedRef>, String> {
        let index = self.index().await?;
        Ok(index
            .copies(&keys_of(&track.title, track.artist.as_deref()))
            .into_iter()
            .cloned()
            .collect())
    }

    /// Where a song is saved: Favorites and the playlists holding it (M46.8), whatever names it
    /// was saved with (M46.48).
    pub async fn saves(&self, track: TrackInput) -> Result<TrackSaves, String> {
        let index = self.index().await?;
        Ok(index.saves(&keys_of(&track.title, track.artist.as_deref())))
    }

    /// Every saved song with the keys it answers to and where it is (M46.48): one list that every
    /// saved mark and Save button reads.
    pub async fn saved_index(&self) -> Result<Vec<SavedSong>, String> {
        Ok(self.index().await?.songs())
    }

    /// Makes a song saved exactly where `saves` says: Favorites on or off, added to the playlists
    /// listed that don't hold it, every copy taken out of the ones not listed (M46.8, M46.48).
    pub async fn set_saves(&self, track: TrackInput, saves: TrackSaves) -> Result<(), String> {
        let index = self.index().await?;
        let keys = keys_of(&track.title, track.artist.as_deref());
        let plan = index.plan(&keys, &saves);
        self.note_explicit(std::slice::from_ref(&track)).await;
        let mut saved = fields(track);
        // A new copy plays what the others do (a source the user picked, M46.48).
        if saved.source.is_none() {
            saved.source = index.source_of(&keys);
        }
        let db = self.db();
        let now = now_ms();
        collections::remove_saved(db, &plan.remove_favorites, &plan.remove_items)
            .await
            .map_err(to_message)?;
        if plan.add_favorite {
            collections::set_favorite(db, &saved, true, now)
                .await
                .map_err(to_message)?;
        }
        for id in &plan.add_to {
            // A playlist deleted meanwhile (here or on another device) gets nothing.
            if collections::playlist(db, id)
                .await
                .map_err(to_message)?
                .is_some()
            {
                collections::add_to_playlist(db, id, std::slice::from_ref(&saved), now)
                    .await
                    .map_err(to_message)?;
            }
        }
        Ok(())
    }

    /// A source the user picked for a song (M46.31, M46.48). With `choice.assign`, every saved
    /// copy and the song in the queue take its stream (a lookup under the source's names when it
    /// has no stream of its own, so the song plays what the preview played), the resolver plays
    /// it first and tries again what failed before, and the unavailable mark goes. The details
    /// `choice` ticks become the song's own (Edit details reverts them); keys stay as they were.
    pub async fn assign_source(
        &self,
        song: TrackInput,
        chosen: OnlineTrack,
        choice: SourceChoice,
    ) -> Result<(), String> {
        let copies = self.copies(&song).await?;
        let picked = if choice.assign {
            Some(
                assign_stream(
                    self.db(),
                    &copies,
                    &song,
                    &chosen,
                    choice.source_name.as_deref(),
                )
                .await?,
            )
        } else {
            None
        };
        let details = SourceDetails::of(&chosen, &choice);
        if !details.is_empty() {
            self.use_details(&song, &copies, &details).await?;
        }
        if picked.is_none() && details.is_empty() {
            return Ok(());
        }
        // The song in the queue: its stream from its next load, its names at once.
        let song_keys = keys_of(&song.title, song.artist.as_deref());
        let artwork = details.artwork.as_deref().map(|url| {
            self.app
                .state::<raygun_net::ImageLinks>()
                .kept_cover(url, &Default::default())
        });
        self.app.state::<PlaybackService>().update_items(|item| {
            if !identity::any_same(&keys_of(&item.title, item.artist.as_deref()), &song_keys) {
                return false;
            }
            if let Some(source) = &picked {
                item.media_kind = media_kind(source);
                item.source = source.clone();
            }
            details.apply_to(item, artwork.clone());
            true
        });
        Ok(())
    }

    /// The keys a song's marks go under: its saved copies' and the names it shows.
    async fn mark_keys(&self, song: &TrackInput) -> Vec<String> {
        let mut keys: Vec<String> = self
            .copies(song)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|row| row.track_key)
            .collect();
        keys.push(shown_key_of(&song.title, song.artist.as_deref()));
        keys.sort_unstable();
        keys.dedup();
        keys
    }

    /// No source played a song, however long playback looked: it's marked as one no source has,
    /// as songs no source had when imported are (lists offer Find a source for it).
    pub async fn mark_unplayable(&self, song: &TrackInput) {
        let keys = self.mark_keys(song).await;
        if let Err(error) = collections::mark_unavailable(self.db(), &keys, now_ms()).await {
            tracing::warn!(%error, "couldn't mark a song no source played");
        }
        self.marks_changed(song);
    }

    /// Lists showing a song's marks and saved copies look again.
    fn marks_changed(&self, song: &TrackInput) {
        use tauri_specta::Event;
        if let Err(error) = crate::events::UnavailableEvent(song.title.clone()).emit(&self.app) {
            tracing::warn!(%error, "couldn't announce a song's marks");
        }
    }

    /// Playback found a song a source after what it had stopped working: the song's marks go,
    /// and `dead`, its own stream that stopped working, leaves every saved copy that carried it
    /// and the queue (they find their stream by their names, the found one first).
    pub async fn source_found(&self, song: &TrackInput, dead: Option<&MediaSource>) {
        let db = self.db();
        for key in self.mark_keys(song).await {
            if let Err(error) = collections::clear_unavailable(db, &key).await {
                tracing::warn!(%error, "couldn't clear a song's mark");
            }
        }
        let Some(dead) = dead.filter(|source| matches!(source, MediaSource::Url { .. })) else {
            self.marks_changed(song);
            return;
        };
        let carrying = |row: &&SavedRef| {
            row.source
                .as_deref()
                .and_then(|json| serde_json::from_str::<MediaSource>(json).ok())
                .is_some_and(|source| &source == dead)
        };
        let copies = self.copies(song).await.unwrap_or_default();
        let (favorites, items): (Vec<&SavedRef>, Vec<&SavedRef>) = copies
            .iter()
            .filter(carrying)
            .partition(|row| row.is_favorite());
        let ids = |rows: Vec<&SavedRef>| -> Vec<String> {
            rows.into_iter().map(|row| row.id.clone()).collect()
        };
        let none = TrackFields {
            track_key: String::new(),
            title: song.title.clone(),
            artist: song.artist.clone(),
            album: None,
            duration_ms: None,
            artwork_url: None,
            source: None,
        };
        if let Err(error) =
            collections::set_track_source_of(db, &ids(favorites), &ids(items), &none, false).await
        {
            tracing::warn!(%error, "couldn't take a stream that stopped working off a song");
        }
        let dead = dead.clone();
        self.app.state::<PlaybackService>().update_items(|item| {
            if item.source != dead {
                return false;
            }
            item.source = MediaSource::Lookup {
                title: item.title.clone(),
                artist: item.artist.clone(),
                album: item.album.clone(),
                duration_ms: item.duration_ms,
            };
            true
        });
        self.marks_changed(song);
    }

    /// Saves the source's details as the song's own, under every key its copies have, so each
    /// copy shows them (M46.30's edits, merged with what the user set before).
    async fn use_details(
        &self,
        song: &TrackInput,
        copies: &[SavedRef],
        details: &SourceDetails,
    ) -> Result<(), String> {
        // Each copy by its own names (an edit is kept under the key they make); a song saved
        // nowhere by the names it showed.
        let mut targets: Vec<(String, Option<String>)> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for row in copies {
            if !seen.contains(&row.track_key) {
                seen.push(row.track_key.clone());
                targets.push((row.title.clone(), row.artist.clone()));
            }
        }
        if targets.is_empty() {
            targets.push((song.title.clone(), song.artist.clone()));
        }
        let service = self.app.state::<Arc<DetailService>>();
        for (title, artist) in targets {
            let target = DetailTarget::Song {
                query: TrackQuery {
                    title,
                    artist,
                    album: None,
                    duration_ms: None,
                },
            };
            let mut edit = service.get(&target);
            details.merge_into(&mut edit);
            service.set(&target, edit).await?;
        }
        Ok(())
    }

    /// Where a saved song came from (M46.48): Import songs, and whether no source had it.
    pub async fn import_status(&self, song: &TrackInput) -> Result<TrackImportStatus, String> {
        let mut keys: Vec<String> = self
            .copies(song)
            .await?
            .into_iter()
            .map(|row| row.track_key)
            .collect();
        keys.push(shown_key_of(&song.title, song.artist.as_deref()));
        let db = self.db();
        Ok(TrackImportStatus {
            imported: !collections::imported(db, &keys)
                .await
                .map_err(to_message)?
                .is_empty(),
            unavailable: !collections::unavailable(db, &keys)
                .await
                .map_err(to_message)?
                .is_empty(),
        })
    }

    pub async fn is_favorite(&self, track: TrackInput) -> Result<bool, String> {
        let index = self.index().await?;
        Ok(index.holds(None, &keys_of(&track.title, track.artist.as_deref())))
    }

    /// Favorites a song once, or takes every copy of it out of Favorites (M46.48).
    pub async fn set_favorite(&self, track: TrackInput, favorite: bool) -> Result<(), String> {
        let copies: Vec<String> = self
            .copies(&track)
            .await?
            .into_iter()
            .filter(SavedRef::is_favorite)
            .map(|row| row.id)
            .collect();
        let db = self.db();
        if !favorite {
            return collections::remove_saved(db, &copies, &[])
                .await
                .map_err(to_message);
        }
        self.note_explicit(std::slice::from_ref(&track)).await;
        if !copies.is_empty() {
            return Ok(());
        }
        collections::set_favorite(db, &fields(track), true, now_ms())
            .await
            .map_err(to_message)
    }

    /// Favorites several songs at once, or takes every copy of them out of Favorites, reading the
    /// saved songs once (M46.49). The first song given is the newest favorite.
    pub async fn set_favorites(
        &self,
        tracks: Vec<TrackInput>,
        favorite: bool,
    ) -> Result<(), String> {
        let index = self.index().await?;
        let favorites_of = |track: &TrackInput| -> Vec<String> {
            index
                .copies(&keys_of(&track.title, track.artist.as_deref()))
                .into_iter()
                .filter(|row| row.is_favorite())
                .map(|row| row.id.clone())
                .collect()
        };
        let db = self.db();
        if !favorite {
            let mut ids: Vec<String> = tracks.iter().flat_map(favorites_of).collect();
            ids.sort();
            ids.dedup();
            return collections::remove_saved(db, &ids, &[])
                .await
                .map_err(to_message);
        }
        self.note_explicit(&tracks).await;
        let now = now_ms();
        for (place, track) in tracks.into_iter().enumerate() {
            if !favorites_of(&track).is_empty() {
                continue;
            }
            let added_at = now - i64::try_from(place).unwrap_or(i64::MAX / 2);
            collections::set_favorite(db, &fields(track), true, added_at)
                .await
                .map_err(to_message)?;
        }
        Ok(())
    }

    // ---- Playlists ----

    pub async fn playlists(&self) -> Result<Vec<PlaylistSummary>, String> {
        let rows = collections::playlists(self.db())
            .await
            .map_err(to_message)?;
        self.summaries(rows).await
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
        let added: Vec<i64> = rows.iter().map(|row| row.added_at).collect();
        let tracks = self
            .saved(rows.into_iter().map(|row| row.track).collect())
            .await?;
        let tracks = with_dates(tracks, added);
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
        let summary = self.summary(row).await?;
        crate::quick_access::add(
            &self.app,
            crate::quick_access::page_pin(
                "playlist",
                "/library/playlist",
                &summary.id,
                &summary.name,
                None,
            ),
        )
        .await;
        Ok(summary)
    }

    pub async fn rename_playlist(&self, id: &str, name: &str) -> Result<(), String> {
        let name = playlist_name(name)?;
        if collections::rename_playlist(self.db(), id, &name)
            .await
            .map_err(to_message)?
        {
            // Quick access sorts by name too.
            crate::quick_access::changed(&self.app, "/library/playlist");
            Ok(())
        } else {
            Err("This playlist no longer exists".into())
        }
    }

    pub async fn delete_playlist(&self, id: &str) -> Result<(), String> {
        collections::delete_playlist(self.db(), id)
            .await
            .map_err(to_message)?;
        let search = [("id".to_owned(), id.to_owned())].into_iter().collect();
        crate::quick_access::remove(&self.app, "/library/playlist", &search).await;
        Ok(())
    }

    /// A new playlist of `ids`' songs, playlist after playlist in the order given (M46.45);
    /// `skip_duplicates` keeps each song once, whatever names its copies have (M46.48). The
    /// merged playlists stay unless `remove_merged`.
    pub async fn merge_playlists(
        &self,
        ids: &[String],
        name: &str,
        skip_duplicates: bool,
        remove_merged: bool,
    ) -> Result<PlaylistSummary, String> {
        if ids.len() < 2 {
            return Err("Pick at least two playlists".into());
        }
        let db = self.db();
        let details = self.app.state::<Arc<DetailService>>();
        let mut fields: Vec<TrackFields> = Vec::new();
        let mut seen = SongSet::default();
        for id in ids {
            for item in collections::playlist_items(db, id)
                .await
                .map_err(to_message)?
            {
                let track = &item.track;
                let keys = identity::row_keys(
                    &track.track_key,
                    &track.title,
                    track.artist.as_deref(),
                    details.shown_names(&track.track_key, &track.title, track.artist.as_deref()),
                );
                if !skip_duplicates || seen.insert(keys) {
                    fields.push(item.track);
                }
            }
        }
        let summary = self.create_playlist(name).await?;
        collections::add_to_playlist(db, &summary.id, &fields, now_ms())
            .await
            .map_err(to_message)?;
        if remove_merged {
            for id in ids {
                self.delete_playlist(id).await?;
            }
        }
        let row = collections::playlist(db, &summary.id)
            .await
            .map_err(to_message)?
            .ok_or_else(|| "The new playlist went missing".to_owned())?;
        self.summary(row).await
    }

    /// Adds songs to the end of a playlist, in their order (M46.48): songs it already holds, under
    /// any of their names, stay out unless `duplicates` (Add anyway).
    pub async fn add_to_playlist(
        &self,
        id: &str,
        tracks: Vec<TrackInput>,
        duplicates: bool,
    ) -> Result<PlaylistAdded, String> {
        let added = self.add_tracks(id, tracks, duplicates).await?;
        let count = added.iter().filter(|added| **added).count() as u32;
        Ok(PlaylistAdded {
            added: count,
            already: added.len() as u32 - count,
        })
    }

    /// [`Self::add_to_playlist`], saying for each song whether it was added.
    pub(crate) async fn add_tracks(
        &self,
        id: &str,
        tracks: Vec<TrackInput>,
        duplicates: bool,
    ) -> Result<Vec<bool>, String> {
        let db = self.db();
        if collections::playlist(db, id)
            .await
            .map_err(to_message)?
            .is_none()
        {
            return Err("This playlist no longer exists".into());
        }
        let index = self.index().await?;
        self.note_explicit(&tracks).await;
        let mut added = Vec::with_capacity(tracks.len());
        let mut adding: Vec<TrackFields> = Vec::new();
        for track in tracks {
            let keys = keys_of(&track.title, track.artist.as_deref());
            let held = !duplicates && index.holds(Some(id), &keys);
            added.push(!held);
            if !held {
                let mut saved = fields(track);
                // A new copy plays what the others do (a source the user picked, M46.48).
                if saved.source.is_none() {
                    saved.source = index.source_of(&keys);
                }
                adding.push(saved);
            }
        }
        if !adding.is_empty() {
            collections::add_to_playlist(db, id, &adding, now_ms())
                .await
                .map_err(to_message)?;
        }
        Ok(added)
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

    /// Keys of every song in Favorites or a playlist (M46.45), with every other key their copies
    /// answer to (M46.48: the names they show with the user's details too).
    pub async fn saved_keys(&self) -> Result<Vec<String>, String> {
        Ok(self.index().await?.all_keys())
    }

    /// Saves a playlist in the order of `item_ids` (M46.44).
    pub async fn set_playlist_order(
        &self,
        playlist_id: &str,
        item_ids: &[String],
    ) -> Result<(), String> {
        collections::set_playlist_order(self.db(), playlist_id, item_ids)
            .await
            .map_err(to_message)
    }

    /// Moves a favorite before another, or last (M46.44).
    pub async fn move_favorite(&self, track_key: &str, before: Option<&str>) -> Result<(), String> {
        collections::move_favorite(self.db(), track_key, before)
            .await
            .map_err(to_message)
    }

    /// Saves Favorites in the order of `track_keys` (M46.44).
    pub async fn set_favorites_order(&self, track_keys: &[String]) -> Result<(), String> {
        collections::set_favorites_order(self.db(), track_keys)
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
        // A song played before and after the user renamed it is one song (M46.48): its latest
        // play stands for it.
        let mut seen = std::collections::HashSet::new();
        Ok(tracks
            .into_iter()
            .zip(played)
            .filter(|(track, _)| seen.insert(shown_key(track)))
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

    /// What was played from lately (M46.37), newest first: device-local, like history's order.
    pub async fn recent_contexts(&self) -> Result<Vec<raygun_model::PlayContext>, String> {
        let stored = raygun_db::local_value(self.db(), RECENT_CONTEXTS)
            .await
            .map_err(to_message)?;
        Ok(stored
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default())
    }

    /// Remembers that something was played from `context`: first in the list, once.
    pub async fn add_recent_context(
        &self,
        mut context: raygun_model::PlayContext,
    ) -> Result<(), String> {
        if context.name.trim().is_empty() || !context.to.starts_with('/') {
            return Err("A play context needs a name and a page".into());
        }
        let keep = crate::settings::load(self.db())
            .await
            .map(|settings| settings.privacy.recently_played)
            .unwrap_or(true);
        if !keep || crate::privacy::incognito(&self.app) {
            return Ok(());
        }
        context.played_at = now_ms() as f64;
        let mut list = self.recent_contexts().await?;
        list.retain(|other| other.to != context.to || other.search != context.search);
        list.insert(0, context);
        list.truncate(RECENT_CONTEXTS_KEPT);
        let json = serde_json::to_string(&list).map_err(to_message)?;
        raygun_db::set_local_value(self.db(), RECENT_CONTEXTS, &json)
            .await
            .map_err(to_message)
    }

    /// Writes a play to history in the background; failures only log. Nothing is written while
    /// listening history is off (M46.41) or incognito is on (M47).
    pub fn record_play(&self, item: QueueItem) {
        if crate::privacy::incognito(&self.app) {
            return;
        }
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let service = app.state::<CollectionsService>();
            let keep = crate::settings::load(service.db())
                .await
                .map(|settings| settings.privacy.history)
                .unwrap_or(true);
            if !keep {
                return;
            }
            let explicit = item.explicit;
            let track = fields(TrackInput {
                explicit: item.explicit,
                title: item.title,
                artist: item.artist,
                album: item.album,
                duration_ms: item.duration_ms,
                artwork_url: item.artwork_url,
                source: Some(item.source),
            });
            if explicit {
                service
                    .remember_explicit(vec![track.track_key.clone()])
                    .await;
            }
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
            Collection::Downloaded => self.downloaded().await?,
        };
        // A sorted or filtered table plays in the order it shows (M46.8).
        let tracks = match &request.order {
            Some(order) => in_order(tracks, order),
            None => tracks,
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
                Some(row) => QueueItemInput {
                    explicit: track.explicit,
                    ..library.queue_item(row)
                },
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
        // A song the user renamed is found in the library by the names its file has (M46.48).
        let original = self
            .app
            .state::<Arc<DetailService>>()
            .original_song_key(&track.title, track.artist.as_deref());
        let explicit = track.explicit;
        let saved = fields(track);
        let mut keys = vec![saved.track_key.clone()];
        keys.extend(original);
        let mut local = self.local_matches(&keys).await?;
        if let Some(row) = keys.iter().find_map(|key| local.remove(key)) {
            let mut item = self.app.state::<LibraryService>().queue_item(row);
            item.explicit = explicit;
            return Ok(item);
        }
        let mut tracks = self.saved(vec![saved]).await?;
        let mut item = queue_item(tracks.remove(0));
        item.explicit |= explicit;
        Ok(item)
    }

    // ---- Helpers ----

    /// The song keys remembered as explicit.
    async fn explicit_keys(&self) -> HashSet<String> {
        raygun_db::local_value(self.db(), EXPLICIT_SONGS)
            .await
            .ok()
            .flatten()
            .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok())
            .unwrap_or_default()
            .into_iter()
            .collect()
    }

    /// Remembers the explicit ones among songs being saved, so saved lists mark them too.
    pub(crate) async fn note_explicit(&self, tracks: &[TrackInput]) {
        let keys: Vec<String> = tracks
            .iter()
            .filter(|track| track.explicit)
            .map(|track| shown_key_of(&track.title, track.artist.as_deref()))
            .collect();
        self.remember_explicit(keys).await;
    }

    async fn remember_explicit(&self, keys: Vec<String>) {
        if keys.is_empty() {
            return;
        }
        let stored: Vec<String> = raygun_db::local_value(self.db(), EXPLICIT_SONGS)
            .await
            .ok()
            .flatten()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        let list = with_explicit(stored, keys);
        let Ok(json) = serde_json::to_string(&list) else {
            return;
        };
        if let Err(error) = raygun_db::set_local_value(self.db(), EXPLICIT_SONGS, &json).await {
            tracing::warn!(%error, "failed to remember explicit songs");
        }
    }

    async fn local_matches(&self, keys: &[String]) -> Result<HashMap<String, TrackRow>, String> {
        self.app.state::<LibraryService>().matches(keys).await
    }

    /// Stored rows as saved tracks, with local artwork where the song is in the library.
    async fn saved(&self, rows: Vec<TrackFields>) -> Result<Vec<SavedTrack>, String> {
        let keys: Vec<String> = rows.iter().map(|row| row.track_key.clone()).collect();
        let local = self.local_matches(&keys).await?;
        let library = self.app.state::<LibraryService>();
        let details = self.app.state::<Arc<DetailService>>();
        let mut tracks: Vec<SavedTrack> = rows
            .into_iter()
            .map(|row| {
                let found = local.get(&row.track_key);
                let local_art = found.and_then(|track| library.artwork_url(track.art.clone()));
                let mut track = SavedTrack {
                    explicit: false,
                    local: found.is_some(),
                    play_count: 0,
                    added_at: None,
                    artwork_url: local_art.or(row.artwork_url),
                    track_key: row.track_key,
                    title: row.title,
                    artist: row.artist,
                    album: row.album,
                    duration_ms: row.duration_ms.map(|duration| duration as u32),
                    source: row
                        .source
                        .and_then(|json| serde_json::from_str(&json).ok())
                        .and_then(saved_source),
                };
                // The user's own details (M46.30).
                details.apply_saved(&mut track);
                track
            })
            .collect();
        let explicit = self.explicit_keys().await;
        for track in &mut tracks {
            track.explicit = explicit.contains(&track.track_key);
        }
        // Plays count under the song's key and under the names it shows: the player records a
        // renamed song by its new names (M46.48).
        let shown: Vec<String> = tracks.iter().map(shown_key).collect();
        let mut asked = keys;
        asked.extend(shown.iter().cloned());
        asked.sort_unstable();
        asked.dedup();
        let plays = collections::play_counts(self.db(), &asked)
            .await
            .map_err(to_message)?;
        for (track, shown) in tracks.iter_mut().zip(&shown) {
            let own = plays.get(&track.track_key).copied().unwrap_or(0);
            let renamed = if *shown == track.track_key {
                0
            } else {
                plays.get(shown).copied().unwrap_or(0)
            };
            track.play_count = (own + renamed) as u32;
        }
        Ok(tracks)
    }

    async fn summary(&self, row: collections::PlaylistRow) -> Result<PlaylistSummary, String> {
        self.summaries(vec![row])
            .await?
            .pop()
            .ok_or_else(|| "The playlist went missing".to_owned())
    }

    /// Playlists as listed: covers from their first items' artwork (the library's where it has
    /// the song, the user's own covers), found for every playlist at once.
    async fn summaries(
        &self,
        rows: Vec<collections::PlaylistRow>,
    ) -> Result<Vec<PlaylistSummary>, String> {
        let keys: Vec<String> = rows
            .iter()
            .flat_map(|row| row.first_items.iter().map(|item| item.track_key.clone()))
            .collect();
        let local = self.local_matches(&keys).await?;
        let library = self.app.state::<LibraryService>();
        let details = self.app.state::<Arc<DetailService>>();
        Ok(rows
            .into_iter()
            .map(|row| {
                let mut artwork_urls: Vec<String> = Vec::new();
                for item in row.first_items {
                    let mut artwork = local
                        .get(&item.track_key)
                        .and_then(|track| library.artwork_url(track.art.clone()))
                        .or(item.artwork_url);
                    let (mut title, mut artist, mut album) = (item.title, item.artist, item.album);
                    details.apply(
                        &item.track_key,
                        &mut title,
                        &mut artist,
                        &mut album,
                        &mut artwork,
                    );
                    if let Some(url) = artwork
                        && !artwork_urls.contains(&url)
                    {
                        artwork_urls.push(url);
                    }
                }
                artwork_urls.truncate(COVER_ARTWORKS);
                // The user's own cover (M46.45).
                let cover_url = details
                    .get(&DetailTarget::Playlist { id: row.id.clone() })
                    .artwork_url;
                PlaylistSummary {
                    id: row.id,
                    name: row.name,
                    item_count: row.item_count as u32,
                    artwork_urls,
                    cover_url,
                    length: Some(CollectionLength {
                        known_secs: u32::try_from(row.known_ms / 1000).unwrap_or(u32::MAX),
                        known_count: row.known_count as u32,
                    }),
                }
            })
            .collect())
    }
}

/// The resolver's key of the names a saved track shows.
fn shown_key(track: &SavedTrack) -> String {
    raygun_resolver::track_key(&TrackQuery {
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: None,
        duration_ms: None,
    })
}

/// The stored side of a pick (M46.48): every saved copy of the song takes the picked stream (see
/// [`picked_source`]), its unavailable marks go, and the resolver forgets what failed for it and
/// plays the pick first under each of its keys: the stream itself, or what played under the
/// source's names (its preview). Returns what the song plays from now.
pub(crate) async fn assign_stream(
    db: &raygun_db::Db,
    copies: &[SavedRef],
    song: &TrackInput,
    chosen: &OnlineTrack,
    source_name: Option<&str>,
) -> Result<MediaSource, String> {
    let source = picked_source(chosen);
    let json = serde_json::to_string(&source).map_err(to_message)?;
    let mut keys: Vec<String> = copies.iter().map(|row| row.track_key.clone()).collect();
    keys.push(shown_key_of(&song.title, song.artist.as_deref()));
    keys.sort_unstable();
    keys.dedup();
    let found = TrackFields {
        track_key: String::new(),
        title: song.title.clone(),
        artist: song.artist.clone(),
        album: chosen.album.clone(),
        duration_ms: chosen.duration_ms.map(i64::from),
        artwork_url: chosen.artwork_url.clone().filter(|url| portable_url(url)),
        source: Some(json.clone()),
    };
    let (favorites, items): (Vec<&SavedRef>, Vec<&SavedRef>) =
        copies.iter().partition(|row| row.is_favorite());
    let ids = |rows: Vec<&SavedRef>| -> Vec<String> {
        rows.into_iter().map(|row| row.id.clone()).collect()
    };
    collections::set_track_source_of(db, &ids(favorites), &ids(items), &found, true)
        .await
        .map_err(to_message)?;
    for key in &keys {
        collections::clear_unavailable(db, key)
            .await
            .map_err(to_message)?;
    }
    let now = now_ms();
    let chosen_key = shown_key_of(&chosen.title, chosen.artist.as_deref());
    let mut forget = keys.clone();
    forget.push(chosen_key.clone());
    raygun_db::streams::clear_failures(db, &forget)
        .await
        .map_err(to_message)?;
    for key in &keys {
        match &source {
            MediaSource::Lookup { .. } => {
                raygun_db::streams::copy_picks(db, &chosen_key, key, now).await
            }
            _ => {
                raygun_db::streams::remember_pick(
                    db,
                    raygun_db::streams::Played {
                        track_key: key,
                        provider_id: source_name.unwrap_or(PICKED),
                        candidate_id: chosen.external_id.as_deref().unwrap_or(&json),
                        title: &chosen.title,
                        source_json: &json,
                        expires_at: None,
                        bitrate_kbps: None,
                    },
                    now,
                )
                .await
            }
        }
        .map_err(to_message)?;
    }
    Ok(source)
}

/// Merges what saving songs under several names left, once per device (M46.48): later copies of
/// a song in Favorites and in each playlist go (the first one added stays), and rows get the key
/// their names make. Migration 0031 already merged copies saved under the same key.
pub async fn repair(app: &AppHandle) {
    let db = &app.state::<AppDb>().inner().0;
    match raygun_db::local_value(db, REPAIRED).await {
        Ok(None) => {}
        Ok(Some(_)) => return,
        Err(error) => {
            tracing::warn!(%error, "couldn't check the saved songs' repair");
            return;
        }
    }
    let result = async {
        let plan = app.state::<CollectionsService>().index().await?.repair();
        if !plan.is_empty() {
            tracing::info!(
                favorites = plan.drop_favorites.len(),
                items = plan.drop_items.len(),
                rekeyed = plan.rekey_favorites.len() + plan.rekey_items.len(),
                "merging saved songs' copies"
            );
            collections::remove_saved(db, &plan.drop_favorites, &plan.drop_items)
                .await
                .map_err(to_message)?;
            collections::set_keys(db, &plan.rekey_favorites, &plan.rekey_items)
                .await
                .map_err(to_message)?;
        }
        raygun_db::set_local_value(db, REPAIRED, "1")
            .await
            .map_err(to_message)
    }
    .await;
    if let Err(error) = result {
        tracing::warn!(%error, "couldn't repair the saved songs");
    }
}

/// Plays from history take this many recent tracks.
const HISTORY_PLAY_LIMIT: u32 = 200;
/// The stream cache's provider for a picked stream whose source has no name.
const PICKED: &str = "picked";

/// What gets stored for a track: its key, and only sources and artwork that work elsewhere.
pub(crate) fn fields(track: TrackInput) -> TrackFields {
    let track_key = shown_key_of(&track.title, track.artist.as_deref());
    let source = track
        .source
        .and_then(|source| stored_source(source, &track.title, track.artist.as_deref()))
        .and_then(|source| serde_json::to_string(&source).ok());
    TrackFields {
        track_key,
        title: track.title,
        artist: track.artist,
        album: track.album,
        duration_ms: track.duration_ms.map(i64::from),
        artwork_url: track.artwork_url.filter(|url| portable_url(url)),
        source,
    }
}

/// A source that means the same on another device.
pub(crate) fn portable(source: MediaSource) -> Option<MediaSource> {
    match &source {
        MediaSource::Url { url, .. } if portable_url(url) => Some(source),
        MediaSource::YouTube { .. } => Some(source),
        _ => None,
    }
}

/// A source worth saving with a song: one that plays on any device, or a lookup under other
/// names than the song's own (the names of a source the user picked, M46.48).
pub(crate) fn stored_source(
    source: MediaSource,
    title: &str,
    artist: Option<&str>,
) -> Option<MediaSource> {
    match &source {
        MediaSource::Lookup {
            title: by_title,
            artist: by_artist,
            ..
        } => (by_title != title || by_artist.as_deref() != artist).then_some(source),
        _ => portable(source),
    }
}

/// A saved row's source as it plays: portable streams, and lookups (M46.48).
fn saved_source(source: MediaSource) -> Option<MediaSource> {
    match source {
        MediaSource::Lookup { .. } => Some(source),
        other => portable(other),
    }
}

/// What a picked source plays from (M46.48): its own stream when it has one that works on any
/// device, else a lookup under its names, which finds what its preview played.
pub(crate) fn picked_source(chosen: &OnlineTrack) -> MediaSource {
    chosen
        .source
        .clone()
        .and_then(portable)
        .unwrap_or_else(|| MediaSource::Lookup {
            title: chosen.title.clone(),
            artist: chosen.artist.clone(),
            album: chosen.album.clone(),
            duration_ms: chosen.duration_ms,
        })
}

fn media_kind(source: &MediaSource) -> MediaKind {
    match source {
        MediaSource::YouTube { .. } => MediaKind::Video,
        _ => MediaKind::Audio,
    }
}

/// The resolver's key of a title and artist.
fn shown_key_of(title: &str, artist: Option<&str>) -> String {
    raygun_resolver::track_key(&TrackQuery {
        title: title.to_owned(),
        artist: artist.map(str::to_owned),
        album: None,
        duration_ms: None,
    })
}

/// The details a pick takes from its source (M46.48), each only when ticked and known.
#[derive(Debug, Default, Clone, PartialEq)]
struct SourceDetails {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    artwork: Option<String>,
}

impl SourceDetails {
    fn of(chosen: &OnlineTrack, choice: &SourceChoice) -> Self {
        let known = |text: Option<&String>| {
            text.map(|text| text.trim().to_owned())
                .filter(|text| !text.is_empty())
        };
        Self {
            title: choice.title.then(|| known(Some(&chosen.title))).flatten(),
            artist: choice
                .artist
                .then(|| known(chosen.artist.as_ref()))
                .flatten(),
            album: choice.album.then(|| known(chosen.album.as_ref())).flatten(),
            artwork: choice
                .cover
                .then(|| chosen.artwork_url.clone())
                .flatten()
                .filter(|url| portable_url(url)),
        }
    }

    fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.artist.is_none()
            && self.album.is_none()
            && self.artwork.is_none()
    }

    /// Over an edit the user made before: what's ticked replaces, the rest stays.
    fn merge_into(&self, edit: &mut raygun_model::DetailEdit) {
        if let Some(title) = &self.title {
            edit.title = Some(title.clone());
        }
        if let Some(artist) = &self.artist {
            edit.artist = Some(artist.clone());
        }
        if let Some(album) = &self.album {
            edit.album = Some(album.clone());
        }
        if let Some(artwork) = &self.artwork {
            edit.artwork_url = Some(artwork.clone());
        }
    }

    /// A queued copy shows them at once; `artwork` is the cover as the webview loads it.
    fn apply_to(&self, item: &mut QueueItem, artwork: Option<String>) {
        if let Some(title) = &self.title {
            item.title = title.clone();
        }
        if let Some(artist) = &self.artist {
            item.artist = Some(artist.clone());
        }
        if let Some(album) = &self.album {
            item.album = Some(album.clone());
        }
        if artwork.is_some() {
            item.artwork_url = artwork;
        }
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
        explicit: track.explicit,
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

/// `stored` with `keys` last (newest), each once, keeping the newest [`EXPLICIT_SONGS_KEPT`].
fn with_explicit(mut stored: Vec<String>, keys: Vec<String>) -> Vec<String> {
    stored.retain(|key| !keys.contains(key));
    for key in keys {
        if !stored.contains(&key) {
            stored.push(key);
        }
    }
    let over = stored.len().saturating_sub(EXPLICIT_SONGS_KEPT);
    stored.drain(..over);
    stored
}

/// `tracks` with the dates they were saved, in the same order.
fn with_dates(tracks: Vec<SavedTrack>, added: Vec<i64>) -> Vec<SavedTrack> {
    tracks
        .into_iter()
        .zip(added)
        .map(|(track, added_at)| SavedTrack {
            added_at: Some(added_at as f64),
            ..track
        })
        .collect()
}

/// `tracks` in the order of `keys`; keys not among them are skipped, and a key listed twice
/// takes the next track with it (a playlist can hold a song twice).
fn in_order(tracks: Vec<SavedTrack>, keys: &[String]) -> Vec<SavedTrack> {
    let mut left: Vec<Option<SavedTrack>> = tracks.into_iter().map(Some).collect();
    keys.iter()
        .filter_map(|key| {
            left.iter_mut()
                .find(|slot| slot.as_ref().is_some_and(|track| &track.track_key == key))
                .and_then(Option::take)
        })
        .collect()
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
            explicit: false,
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

        // A lookup under other names is a source the user picked (M46.48); kept.
        let picked = MediaSource::Lookup {
            title: "Feeling Good".into(),
            artist: Some("Nina Simone".into()),
            album: None,
            duration_ms: None,
        };
        let kept = fields(input(Some(picked), None)).source.unwrap();
        assert!(kept.contains("lookup") && kept.contains("Feeling Good"));
        for source in [
            MediaSource::LocalFile {
                path: "/sdcard/Music/a.flac".into(),
            },
            // A lookup under the song's own names says nothing.
            MediaSource::Lookup {
                title: "Feeling Good (Remastered)".into(),
                artist: Some("Nina Simone".into()),
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
    fn tables_play_in_the_order_they_show() {
        let song = |key: &str| SavedTrack {
            explicit: false,
            track_key: key.into(),
            title: key.into(),
            artist: None,
            album: None,
            duration_ms: None,
            artwork_url: None,
            source: None,
            local: false,
            added_at: None,
            play_count: 0,
        };
        let tracks = vec![song("a"), song("b"), song("a"), song("c")];
        let keys = |tracks: Vec<SavedTrack>| -> Vec<String> {
            tracks.into_iter().map(|track| track.track_key).collect()
        };
        // A key twice takes both copies; unknown keys are skipped; unlisted tracks are left out.
        assert_eq!(
            keys(in_order(
                tracks.clone(),
                &["c".into(), "a".into(), "x".into(), "a".into()]
            )),
            ["c", "a", "a"]
        );
        let dated = with_dates(tracks, vec![10, 20, 30, 40]);
        assert_eq!(dated[1].added_at, Some(20.0));
    }

    #[test]
    fn saved_tracks_without_a_source_play_through_the_resolver() {
        let track = SavedTrack {
            explicit: false,
            track_key: "a|b".into(),
            title: "B".into(),
            artist: Some("A".into()),
            album: None,
            duration_ms: Some(1000),
            artwork_url: None,
            source: None,
            local: false,
            added_at: None,
            play_count: 0,
        };
        let item = queue_item(track.clone());
        assert!(matches!(item.source, MediaSource::Lookup { ref title, .. } if title == "B"));

        let video = queue_item(SavedTrack {
            explicit: false,
            source: Some(MediaSource::YouTube {
                video_id: "v".into(),
            }),
            ..track
        });
        assert_eq!(video.media_kind, MediaKind::Video);
    }

    fn pick(source: Option<MediaSource>) -> OnlineTrack {
        OnlineTrack {
            external_id: Some("2".into()),
            title: "Song".into(),
            artist: Some("Artist".into()),
            album: Some("Album".into()),
            duration_ms: Some(200_000),
            artwork_url: Some("https://img.example/a.jpg".into()),
            source,
            explicit: false,
        }
    }

    /// A saved row as `play` queues it, reading its source the same way.
    #[test]
    fn explicit_keys_stay_once_and_newest_last() {
        let stored = vec!["a".to_owned(), "b".to_owned()];
        let list = with_explicit(stored, vec!["a".to_owned(), "c".to_owned(), "c".to_owned()]);
        assert_eq!(list, ["b", "a", "c"]);
        let many: Vec<String> = (0..EXPLICIT_SONGS_KEPT).map(|n| n.to_string()).collect();
        let list = with_explicit(many, vec!["new".to_owned()]);
        assert_eq!(list.len(), EXPLICIT_SONGS_KEPT);
        assert_eq!(list.first().map(String::as_str), Some("1"));
        assert_eq!(list.last().map(String::as_str), Some("new"));
    }

    fn played(row: &TrackFields) -> QueueItem {
        let input = queue_item(SavedTrack {
            explicit: false,
            track_key: row.track_key.clone(),
            title: row.title.clone(),
            artist: row.artist.clone(),
            album: row.album.clone(),
            duration_ms: row.duration_ms.map(|duration| duration as u32),
            artwork_url: row.artwork_url.clone(),
            source: row
                .source
                .as_deref()
                .and_then(|json| serde_json::from_str(json).ok())
                .and_then(saved_source),
            local: false,
            added_at: None,
            play_count: 0,
        });
        QueueItem {
            explicit: false,
            id: "q1".into(),
            title: input.title,
            artist: input.artist,
            album: input.album,
            duration_ms: input.duration_ms,
            artwork_url: input.artwork_url,
            media_kind: input.media_kind,
            source: input.source,
        }
    }

    #[tokio::test]
    async fn a_picked_source_is_what_the_song_plays() {
        use raygun_db::streams;

        let temp = tempfile::TempDir::new().unwrap();
        let db = raygun_db::Db::open(&temp.path().join("raygun.db"))
            .await
            .unwrap();
        // Imported from a screenshot with its names misread, and no source had it.
        let song = TrackInput {
            explicit: false,
            title: "Sogn".into(),
            artist: Some("Artst".into()),
            album: None,
            duration_ms: None,
            artwork_url: None,
            source: None,
        };
        let id = collections::create_playlist(&db, "Imported", 1)
            .await
            .unwrap();
        collections::add_to_playlist(&db, &id, &[fields(song.clone())], 2)
            .await
            .unwrap();
        collections::mark_unavailable(&db, &["artst|sogn".to_owned()], 3)
            .await
            .unwrap();
        // Played once: every match failed, and failures are kept out for a day.
        streams::mark_failed(&db, "artst|sogn", "dev.a/stream", "1", "Sogn", "404", 4)
            .await
            .unwrap();
        // Find a source: the preview played the pick under its own names.
        let previewed = r#"{"kind":"url","url":"https://b.example/2.mp3","headers":{}}"#;
        streams::mark_verified(
            &db,
            streams::Played {
                track_key: "artist|song",
                provider_id: "dev.b/stream",
                candidate_id: "2",
                title: "Song",
                source_json: previewed,
                expires_at: None,
                bitrate_kbps: None,
            },
            5,
        )
        .await
        .unwrap();

        // Use this, for a source with no stream of its own (a catalogue's answer).
        let copies = collections::saved_refs(&db).await.unwrap();
        let source = assign_stream(&db, &copies, &song, &pick(None), Some("Tunes"))
            .await
            .unwrap();
        assert!(matches!(source, MediaSource::Lookup { ref title, .. } if title == "Song"));

        // Played again: it looks the pick's names up, and the resolver's first remembered
        // stream for them is what the preview played.
        let items = collections::playlist_items(&db, &id).await.unwrap();
        assert_eq!(items[0].track.album.as_deref(), Some("Album"));
        let item = played(&items[0].track);
        assert_eq!(
            item.title, "Sogn",
            "the names stay unless the details are taken"
        );
        let query = TrackQuery::for_item(&item);
        assert_eq!(query.title, "Song");
        let remembered = streams::candidates(&db, &raygun_resolver::track_key(&query))
            .await
            .unwrap();
        assert_eq!(remembered[0].source.as_deref(), Some(previewed));
        // Not unavailable any more; the old names' failures are forgotten, and their first
        // remembered stream is the preview's too.
        assert!(
            collections::unavailable(&db, &["artst|sogn".to_owned()])
                .await
                .unwrap()
                .is_empty()
        );
        let old = streams::candidates(&db, "artst|sogn").await.unwrap();
        assert_eq!(old[0].source.as_deref(), Some(previewed));
        assert!(old.iter().all(|row| row.failed_at.is_none()));

        // A source with a stream of its own: the song plays it directly, and the resolver
        // remembers it for the song's names.
        let stream = url("https://c.example/3.mp3");
        let source = assign_stream(
            &db,
            &copies,
            &song,
            &pick(Some(stream.clone())),
            Some("Indie Tracks"),
        )
        .await
        .unwrap();
        assert_eq!(source, stream);
        let item = played(&collections::playlist_items(&db, &id).await.unwrap()[0].track);
        assert_eq!(item.source, stream);
        assert_eq!(
            crate::resolution::Tracker::default().begin(
                1,
                &item,
                0.0,
                crate::resolution::Cause::Chosen
            ),
            crate::resolution::LoadPlan::Direct
        );
        let remembered = streams::candidates(&db, "artst|sogn").await.unwrap();
        assert!(remembered.iter().any(|row| {
            row.provider_id == "Indie Tracks" && row.verified_at == remembered[0].verified_at
        }));
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
