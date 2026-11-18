//! Importing songs from elsewhere (M46.21): a Spotify playlist, album or song link (read from
//! Spotify's public embed page, at most its first 100 songs), a CSV export (Exportify), pasted
//! lines, or screenshots of a song list (read with `ocr`). Each song is then matched against the
//! music sources: the best match fills in its artwork, album and length (which helps playback find
//! the right recording), and a source's own stream when it has one. A playlist made from them
//! plays through the resolver like any other.

pub mod ocr;
pub mod paddle;
pub mod parse;
pub mod spotify;

use std::collections::HashSet;
use std::sync::Arc;

use raygun_model::{
    ImportProgress, ImportReader, ImportReaderKind, ImportStage, ImportedList, ImportedSong,
    JoinedSong, MatchedSong, MediaSource, ScreenshotSongs, TrackInput, TrackQuery,
};
use raygun_resolver::{Candidate, History, MIN_SCORE, artist_agrees, score};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::ai::AiService;
use crate::collections::{CollectionsService, identity};
use crate::extensions::ExtensionService;
use crate::uploads::Uploads;

/// Songs an embed page shows at most (Spotify's embed stops there); a list that long was cut.
const EMBED_LIMIT: usize = 100;
/// Reader ids (`ImportReader::id`); AI readers are `ai:<provider id>`.
const WINDOWS: &str = "windows";
const OCRS: &str = "ocrs";
const PADDLE: &str = "paddle";
const AI_PREFIX: &str = "ai:";
/// Pictures sent to a vision model are at most this long on their longer side (they read text
/// best near it, and a phone's screenshot stays legible).
const VISION_MAX_SIDE: u32 = 2400;

/// A screenshot readied for a vision model: as it is when it's a PNG or JPEG of a fitting size,
/// else a JPEG made smaller.
fn for_vision(bytes: &[u8]) -> Result<(Vec<u8>, &'static str), String> {
    let format = image::guess_format(bytes).ok();
    let picture =
        image::load_from_memory(bytes).map_err(|_| "That isn't a picture Raygun can open")?;
    let fits = picture.width().max(picture.height()) <= VISION_MAX_SIDE && bytes.len() < 4_000_000;
    match format {
        Some(image::ImageFormat::Png) if fits => return Ok((bytes.to_vec(), "image/png")),
        Some(image::ImageFormat::Jpeg) if fits => return Ok((bytes.to_vec(), "image/jpeg")),
        _ => {}
    }
    let picture = if fits {
        picture
    } else {
        picture.resize(
            VISION_MAX_SIDE,
            VISION_MAX_SIDE,
            image::imageops::FilterType::CatmullRom,
        )
    };
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 90)
        .encode_image(&picture.into_rgb8())
        .map_err(|error| error.to_string())?;
    Ok((jpeg, "image/jpeg"))
}
/// Songs matched at once.
const MATCH_TOGETHER: usize = 4;
/// Split rows the sources are asked about per import, at most (three searches each).
const SPLIT_CHECKS: usize = 24;

pub struct ImportService {
    app: AppHandle,
    client: reqwest::Client,
}

/// What `save` returns when the songs went to Favorites.
pub const FAVORITES: &str = "favorites";

impl ImportService {
    pub fn new(app: AppHandle, client: reqwest::Client) -> Self {
        Self { app, client }
    }

    fn extensions(&self) -> Arc<ExtensionService> {
        self.app.state::<Arc<ExtensionService>>().inner().clone()
    }

    pub fn app(&self) -> &AppHandle {
        &self.app
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<crate::startup::AppDb>().inner().0.clone()
    }

    async fn prefs(&self) -> raygun_model::ImportPrefs {
        crate::settings::load(&self.db())
            .await
            .map(|settings| settings.import)
            .unwrap_or_default()
    }

    /// The songs a link points at. A playlist or album goes through Spotify's Web API when the
    /// person's own app is set up (every song), else through the embed page (at most 100).
    pub async fn read_link(&self, link: &str) -> Result<ImportedList, String> {
        let spotify = parse::spotify_link(link).ok_or(
            "That isn't a link Raygun can read; paste a Spotify playlist, album or song link",
        )?;
        let client_id = self.prefs().await.spotify_client_id;
        let secret = self.app.state::<AiService>().spotify_secret();
        if let (false, Some(secret), "playlist" | "album") =
            (client_id.is_empty(), secret, spotify.kind.as_str())
        {
            let token = spotify::token(&self.client, &client_id, &secret).await?;
            let list =
                spotify::whole_list(&self.client, &token, &spotify.kind, &spotify.id).await?;
            if list.songs.is_empty() {
                return Err("That Spotify list has no songs".into());
            }
            return Ok(ImportedList {
                name: list.name,
                artwork_url: list.artwork_url,
                songs: list.songs,
                capped: false,
                joined: Vec::new(),
                screenshots: Vec::new(),
                reader: None,
            });
        }
        let url = format!(
            "https://open.spotify.com/embed/{}/{}",
            spotify.kind, spotify.id
        );
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|error| format!("Spotify couldn't be reached ({error})"))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err("Spotify has no such list; it may be private or removed".into());
        }
        if !response.status().is_success() {
            return Err(format!(
                "Spotify answered HTTP {}",
                response.status().as_u16()
            ));
        }
        let html = response.text().await.map_err(|error| error.to_string())?;
        let list = parse::spotify_embed(&html)?;
        // The embed stops at its limit: a list that long was most likely cut.
        let capped = list.songs.len() >= EMBED_LIMIT && spotify.kind != "track";
        Ok(ImportedList {
            name: list.name,
            artwork_url: list.artwork_url,
            songs: list.songs,
            capped,
            joined: Vec::new(),
            screenshots: Vec::new(),
            reader: None,
        })
    }

    /// Songs in a CSV export or pasted lines.
    pub fn read_text(&self, text: &str, name: Option<String>) -> Result<ImportedList, String> {
        let first = text
            .trim_start_matches('\u{feff}')
            .lines()
            .next()
            .unwrap_or_default();
        let songs = if first.contains(',') && first.to_lowercase().contains("name")
            || first.to_lowercase().starts_with("title,")
        {
            parse::csv(text)?
        } else {
            parse::lines(text)
        };
        if songs.is_empty() {
            return Err("No songs were found in that text".into());
        }
        Ok(ImportedList {
            name: name.unwrap_or_else(|| "Imported songs".into()),
            artwork_url: None,
            songs,
            capped: false,
            joined: Vec::new(),
            screenshots: Vec::new(),
            reader: None,
        })
    }

    /// The ways screenshots can be read here, best first: AI providers that read pictures,
    /// PaddleOCR, Windows' own reader (on Windows), ocrs. Each says whether it's ready.
    pub async fn readers(&self) -> Vec<ImportReader> {
        let ai = self.app.state::<AiService>();
        let mut readers: Vec<ImportReader> = ai
            .picture_readers()
            .await
            .into_iter()
            .map(|(id, name)| ImportReader {
                id: format!("{AI_PREFIX}{id}"),
                kind: ImportReaderKind::Ai,
                name: Some(name),
                ready: true,
                model: None,
                size_bytes: None,
            })
            .collect();
        let models = ai.models();
        let model = |id: &str, kind: ImportReaderKind, reader: &str| {
            let info = models.iter().find(|model| model.id == id);
            ImportReader {
                id: reader.to_owned(),
                kind,
                name: None,
                ready: info.is_some_and(|model| model.installed),
                model: Some(id.to_owned()),
                size_bytes: info.map(|model| model.size_bytes),
            }
        };
        readers.push(model(
            paddle::PADDLE_MODEL,
            ImportReaderKind::Paddle,
            PADDLE,
        ));
        if ocr::built_in() {
            readers.push(ImportReader {
                id: WINDOWS.to_owned(),
                kind: ImportReaderKind::Windows,
                name: None,
                ready: true,
                model: None,
                size_bytes: None,
            });
        }
        readers.push(model(ocr::OCRS_MODEL, ImportReaderKind::Ocrs, OCRS));
        readers
    }

    /// The reader `wanted` names (or Settings → Import's choice, `auto` = the best ready one,
    /// else the first that would be ready after a download).
    async fn reader_for(&self, wanted: Option<&str>) -> Option<ImportReader> {
        let choice = match wanted {
            Some(wanted) => wanted.to_owned(),
            None => self.prefs().await.reader,
        };
        let readers = self.readers().await;
        readers
            .iter()
            .find(|reader| reader.id == choice)
            .or_else(|| readers.iter().find(|reader| reader.ready))
            .or_else(|| {
                // On-device first when nothing is ready: Windows has one built in, so this is
                // PaddleOCR or ocrs to download.
                readers
                    .iter()
                    .find(|reader| reader.kind == ImportReaderKind::Paddle)
            })
            .cloned()
    }

    /// Whether screenshots can be read now by `reader` (or the chosen one).
    pub async fn reader_ready(&self, reader: Option<&str>) -> bool {
        self.reader_for(reader)
            .await
            .is_some_and(|reader| reader.ready)
    }

    /// Downloads `reader`'s models (or the chosen reader's; progress as `AiModelEvent`s). A
    /// cancelled download isn't a failure: the reader is left as it was.
    pub async fn prepare_reader(&self, reader: Option<&str>) -> Result<(), String> {
        let Some(reader) = self.reader_for(reader).await else {
            return Err("No text reader is offered here".into());
        };
        let Some(model) = reader.model.filter(|_| !reader.ready) else {
            return Ok(());
        };
        match self.app.state::<AiService>().download_model(&model).await {
            Ok(()) | Err(crate::ai::ModelDownloadError::Cancelled) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// The on-device engine for a reader, its models looked up now (after an install, the new
    /// folder; nothing is kept from before).
    fn engine_of(&self, reader: &ImportReader) -> Result<ocr::Engine, String> {
        let ai = self.app.state::<AiService>();
        let installed = |id: &str| {
            ai.installed_dir(id)
                .ok_or_else(|| "Download the text reader first".to_owned())
        };
        Ok(match reader.kind {
            ImportReaderKind::Windows => ocr::Engine::Windows,
            ImportReaderKind::Ocrs => ocr::Engine::Ocrs(installed(ocr::OCRS_MODEL)?),
            ImportReaderKind::Paddle => ocr::Engine::Paddle(installed(paddle::PADDLE_MODEL)?),
            ImportReaderKind::Ai => return Err("An AI reader isn't on the device".into()),
        })
    }

    fn progress(&self, picture: usize, pictures: usize, stage: ImportStage, rows: usize) {
        let progress = ImportProgress {
            picture: picture as u32,
            pictures: pictures as u32,
            stage,
            rows: rows as u32,
        };
        if let Err(error) = crate::events::ImportProgressEvent(progress).emit(&self.app) {
            tracing::debug!(%error, "couldn't announce reading progress");
        }
    }

    /// Songs in screenshots of song lists, uploaded first (`upload_*`), in the pictures' order,
    /// read by `reader` (an `ImportReader` id; `None` for Settings → Import's choice). With
    /// `join_split`, an artist line read as a row of its own is joined back to the song above
    /// it (`parse::split_rows`); when the layout leaves it open, the sources decide. Progress
    /// goes out as `ImportProgressEvent`s.
    pub async fn read_screenshots(
        &self,
        upload_ids: Vec<String>,
        join_split: bool,
        reader: Option<String>,
    ) -> Result<ImportedList, String> {
        let reader = self
            .reader_for(reader.as_deref())
            .await
            .ok_or("No text reader is offered here")?;
        if !reader.ready {
            return Err("Download the text reader first".into());
        }
        let total = upload_ids.len();
        let uploads = self.app.state::<Uploads>();
        let mut files = Vec::with_capacity(total);
        for id in &upload_ids {
            files.push(uploads.take(id)?);
        }
        let result = self.read_pictures(&files, join_split, &reader).await;
        for path in files {
            let _ = tokio::fs::remove_file(&path).await;
        }
        let mut list = result?;
        list.reader = Some(reader.id);
        Ok(list)
    }

    async fn read_pictures(
        &self,
        files: &[std::path::PathBuf],
        join_split: bool,
        reader: &ImportReader,
    ) -> Result<ImportedList, String> {
        let total = files.len();
        let engine = match reader.kind {
            ImportReaderKind::Ai => None,
            _ => {
                let engine = self.engine_of(reader)?;
                self.progress(1, total, ImportStage::Preparing, 0);
                Some(Arc::new(
                    tokio::task::spawn_blocking(move || engine.load())
                        .await
                        .map_err(|error| error.to_string())??,
                ))
            }
        };
        // Each picture's rows, or for an AI reader its songs as read.
        let mut pictures: Vec<Vec<parse::ScreenRow>> = Vec::new();
        let mut ai_songs: Vec<Vec<ImportedSong>> = Vec::new();
        for (at, path) in files.iter().enumerate() {
            let picture = at + 1;
            self.progress(picture, total, ImportStage::Preparing, 0);
            let bytes = tokio::fs::read(path)
                .await
                .map_err(|error| error.to_string())?;
            self.progress(picture, total, ImportStage::Reading, 0);
            match &engine {
                Some(engine) => {
                    let engine = engine.clone();
                    let lines = tokio::task::spawn_blocking(move || engine.read(&bytes))
                        .await
                        .map_err(|error| error.to_string())??;
                    tracing::info!(lines = lines.len(), reader = reader.id, "read a screenshot");
                    let rows = parse::screenshot_layout(lines);
                    self.progress(picture, total, ImportStage::Rows, rows.len());
                    pictures.push(rows);
                }
                None => {
                    let provider = reader.id.trim_start_matches(AI_PREFIX).to_owned();
                    let (sent, media_type) =
                        tokio::task::spawn_blocking(move || for_vision(&bytes))
                            .await
                            .map_err(|error| error.to_string())??;
                    let rows = self
                        .app
                        .state::<AiService>()
                        .read_song_rows(&provider, &sent, media_type)
                        .await?;
                    let songs: Vec<ImportedSong> = rows
                        .into_iter()
                        .map(|row| ImportedSong {
                            title: row.title,
                            artist: row.artist,
                            album: row.album,
                            duration_ms: None,
                            artwork_url: None,
                        })
                        .collect();
                    self.progress(picture, total, ImportStage::Rows, songs.len());
                    ai_songs.push(songs);
                }
            }
        }
        let mut songs: Vec<ImportedSong> = Vec::new();
        let mut joined: Vec<JoinedSong> = Vec::new();
        let mut screenshots: Vec<ScreenshotSongs> = Vec::new();
        let place = |songs: &mut Vec<ImportedSong>, song: ImportedSong| -> (u32, bool) {
            match songs
                .iter()
                .position(|known| known.title == song.title && known.artist == song.artist)
            {
                Some(at) => (at as u32, false),
                None => {
                    songs.push(song);
                    ((songs.len() - 1) as u32, true)
                }
            }
        };
        if engine.is_none() {
            for (at, read) in ai_songs.into_iter().enumerate() {
                let mut indexes = Vec::new();
                for song in read {
                    indexes.push(place(&mut songs, song).0);
                }
                screenshots.push(ScreenshotSongs {
                    picture: at as u32,
                    songs: indexes,
                });
            }
        } else {
            // Artists the pictures' whole rows name.
            let known: HashSet<String> = pictures
                .iter()
                .flatten()
                .filter_map(|row| row.song.artist.as_deref())
                .map(raygun_resolver::normalize)
                .collect();
            let mut checks_left = SPLIT_CHECKS;
            for (at, rows) in pictures.into_iter().enumerate() {
                let joins = if join_split {
                    self.joins(&rows, &known, &mut checks_left).await
                } else {
                    Vec::new()
                };
                let mut indexes = Vec::new();
                for (song, parts) in parse::join_rows(rows, &joins) {
                    let (index, added) = place(&mut songs, song);
                    indexes.push(index);
                    if let (true, Some(parts)) = (added, parts) {
                        joined.push(JoinedSong { index, parts });
                    }
                }
                screenshots.push(ScreenshotSongs {
                    picture: at as u32,
                    songs: indexes,
                });
            }
        }
        self.progress(total, total, ImportStage::Done, songs.len());
        if songs.is_empty() {
            return Err(
                "No songs were found in the pictures; a list with titles and artists reads best"
                    .into(),
            );
        }
        Ok(ImportedList {
            name: "Songs from screenshots".into(),
            artwork_url: None,
            songs,
            capped: false,
            joined,
            screenshots,
            reader: None,
        })
    }

    /// The rows of a screenshot to join to the row under them: the sure ones, and the unsure ones
    /// whose joined reading the sources know better (at most `checks_left` of those asked).
    async fn joins(
        &self,
        rows: &[parse::ScreenRow],
        known: &HashSet<String>,
        checks_left: &mut usize,
    ) -> Vec<usize> {
        let calls = parse::split_rows(rows, known);
        let mut joins: Vec<usize> = calls
            .iter()
            .filter(|(_, call)| *call == parse::SplitCall::Join)
            .map(|(at, _)| *at)
            .collect();
        let unsure: Vec<usize> = calls
            .iter()
            .filter(|(_, call)| *call == parse::SplitCall::Unsure)
            .map(|(at, _)| *at)
            .take(*checks_left)
            .collect();
        *checks_left -= unsure.len();
        for group in unsure.chunks(MATCH_TOGETHER) {
            let answers = futures_util::future::join_all(group.iter().map(|at| {
                let (title, artist) = (&rows[*at].song.title, &rows[*at + 1].as_artist);
                self.joined_reading_wins(title, artist)
            }))
            .await;
            joins.extend(
                group
                    .iter()
                    .zip(answers)
                    .filter(|(_, wins)| *wins)
                    .map(|(at, _)| *at),
            );
        }
        joins
    }

    /// Searches the sources for both readings of two rows, "title by artist" or two songs, and
    /// says whether the joined one matches better (scored by the resolver).
    async fn joined_reading_wins(&self, title: &str, artist: &str) -> bool {
        let extensions = self.extensions();
        let query = |title: &str, artist: Option<&str>| TrackQuery {
            title: title.to_owned(),
            artist: artist.map(str::to_owned),
            album: None,
            duration_ms: None,
        };
        let readings = [
            query(title, Some(artist)),
            query(title, None),
            query(artist, None),
        ];
        let (joined, title_alone, artist_alone) = tokio::join!(
            find_song(&extensions, &readings[0]),
            find_song(&extensions, &readings[1]),
            find_song(&extensions, &readings[2]),
        );
        let points = |found: Option<(i32, String, raygun_model::OnlineTrack)>| {
            found.map(|(points, _, _)| points)
        };
        parse::joined_reading_wins(points(joined), points(title_alone), points(artist_alone))
    }

    /// Matches songs against the music sources, a few at a time; each comes back with what the
    /// best match knows about it, or as it was when nothing matched well enough.
    pub async fn match_songs(&self, songs: Vec<ImportedSong>) -> Vec<MatchedSong> {
        let mut matched = Vec::with_capacity(songs.len());
        for group in songs.chunks(MATCH_TOGETHER) {
            let answers = futures_util::future::join_all(
                group.iter().cloned().map(|song| self.match_one(song)),
            )
            .await;
            matched.extend(answers);
        }
        matched
    }

    async fn match_one(&self, song: ImportedSong) -> MatchedSong {
        let query = TrackQuery {
            title: song.title.clone(),
            artist: song.artist.clone(),
            album: song.album.clone(),
            duration_ms: song.duration_ms,
        };
        let candidates = find_songs(&self.extensions(), &query, CANDIDATES).await;
        let best = candidates.first().map(|candidate| {
            (
                candidate.points,
                candidate.provider_name.clone(),
                candidate.track.clone(),
            )
        });
        match best {
            Some((points, provider, track)) if points >= MIN_SCORE => {
                // A stream only counts when it plays anywhere (not the YouTube embed, which is a
                // video). Otherwise the song plays by the names it matched under (M46.48): the
                // resolver then finds what that source has, as the review's preview did.
                let source = track
                    .source
                    .clone()
                    .filter(|source| matches!(source, MediaSource::Url { .. }))
                    .or_else(|| {
                        Some(MediaSource::Lookup {
                            title: track.title.clone(),
                            artist: track.artist.clone(),
                            album: track.album.clone(),
                            duration_ms: track.duration_ms,
                        })
                    });
                MatchedSong {
                    song: ImportedSong {
                        album: agreed_album(song.album.as_deref(), &candidates)
                            .or(song.album)
                            .or(track.album),
                        duration_ms: song.duration_ms.or(track.duration_ms),
                        artwork_url: song.artwork_url.or(track.artwork_url),
                        // A title alone ("Yesterday") is saved with its match's artist, so it's
                        // the same song as the one other screens show (M46.48).
                        artist: song
                            .artist
                            .filter(|artist| !artist.trim().is_empty())
                            .or(track.artist.filter(|artist| !artist.trim().is_empty())),
                        ..song
                    },
                    matched_by: Some(provider),
                    source,
                    candidates,
                    pending: false,
                }
            }
            _ => MatchedSong {
                song,
                matched_by: None,
                source: None,
                candidates: Vec::new(),
                pending: false,
            },
        }
    }

    /// Saves the songs where `target` says, in their order; returns the playlist's id, or
    /// `favorites` (M46.45). Songs the playlist or Favorites already holds, under any of their
    /// names, are left out unless `add_existing` (Favorites hold a song once whatever it says,
    /// M46.48). Songs still `pending` are matched in the background and filled in as they're
    /// found (M46.31); songs no source has are kept, marked unavailable.
    pub async fn save(
        &self,
        target: raygun_model::ImportTarget,
        songs: Vec<MatchedSong>,
        add_existing: bool,
    ) -> Result<String, String> {
        use raygun_model::ImportTarget;
        let collections = self.app.state::<CollectionsService>();
        let id = match &target {
            ImportTarget::New { name } => collections.create_playlist(name).await?.id,
            ImportTarget::Playlist { id } => id.clone(),
            ImportTarget::Favorites => FAVORITES.to_owned(),
        };
        let db = self.app.state::<crate::startup::AppDb>().inner().0.clone();
        let key_of = |song: &ImportedSong| {
            raygun_resolver::track_key(&TrackQuery {
                title: song.title.clone(),
                artist: song.artist.clone(),
                album: song.album.clone(),
                duration_ms: None,
            })
        };
        let tracks: Vec<TrackInput> = songs
            .iter()
            .map(|matched| TrackInput {
                explicit: false,
                title: matched.song.title.clone(),
                artist: matched.song.artist.clone(),
                album: matched.song.album.clone(),
                duration_ms: matched.song.duration_ms,
                artwork_url: matched.song.artwork_url.clone(),
                source: matched.source.clone(),
            })
            .collect();
        let added: Vec<bool> = if target == ImportTarget::Favorites {
            let index = collections.index().await?;
            let mut added = vec![false; tracks.len()];
            let mut favorited = identity::SongSet::default();
            // Newest first: the first song is added last, so it comes out on top.
            for (at, track) in tracks.into_iter().enumerate().rev() {
                let keys = identity::keys_of(&track.title, track.artist.as_deref());
                if index.holds(None, &keys) || !favorited.insert(keys) {
                    continue;
                }
                collections.set_favorite(track, true).await?;
                added[at] = true;
            }
            added
        } else {
            collections.add_tracks(&id, tracks, add_existing).await?
        };
        // Marks and background matching only for the songs this import added.
        let imported: Vec<String> = songs
            .iter()
            .zip(&added)
            .filter(|(_, added)| **added)
            .map(|(matched, _)| key_of(&matched.song))
            .collect();
        let _ = raygun_db::collections::mark_imported(&db, &imported, now_ms()).await;
        let matched: Vec<&MatchedSong> = songs
            .iter()
            .zip(&added)
            .filter(|(matched, added)| **added && !matched.pending)
            .map(|(matched, _)| matched)
            .collect();
        self.keep_matches(&matched).await;
        let missing: Vec<String> = songs
            .iter()
            .zip(&added)
            .filter(|(matched, added)| **added && !matched.pending && matched.matched_by.is_none())
            .map(|(matched, _)| key_of(&matched.song))
            .collect();
        let pending: Vec<ImportedSong> = songs
            .into_iter()
            .zip(&added)
            .filter(|(matched, added)| **added && matched.pending)
            .map(|(matched, _)| matched.song)
            .collect();
        // Nothing is marked while Offline Mode is on: no source was asked.
        if !raygun_net::network_mode::offline() {
            let _ = raygun_db::collections::mark_unavailable(&db, &missing, now_ms()).await;
        }
        if !pending.is_empty() {
            let app = self.app.clone();
            let id = id.clone();
            tauri::async_runtime::spawn(async move {
                let import = app.state::<ImportService>();
                for song in pending {
                    let key = key_of(&song);
                    // Nobody waits on these once saved: the sources take them as background
                    // work, paced per source.
                    let matched = raygun_ext_host::in_background(import.match_one(song)).await;
                    let db = &app.state::<crate::startup::AppDb>().inner().0;
                    if matched.matched_by.is_some() {
                        let found = raygun_db::collections::TrackFields {
                            track_key: key.clone(),
                            title: matched.song.title.clone(),
                            artist: matched.song.artist.clone(),
                            album: matched.song.album.clone(),
                            duration_ms: matched.song.duration_ms.map(i64::from),
                            artwork_url: matched.song.artwork_url.clone(),
                            source: matched
                                .source
                                .as_ref()
                                .and_then(|source| serde_json::to_string(source).ok()),
                        };
                        let _ =
                            raygun_db::collections::set_track_source(db, &key, &found, false).await;
                        import.keep_matches(&[&matched]).await;
                    } else if !raygun_net::network_mode::offline() {
                        let _ =
                            raygun_db::collections::mark_unavailable(db, &[key], now_ms()).await;
                    }
                    if let Err(error) = crate::events::ImportEvent(id.clone()).emit(&app) {
                        tracing::debug!(%error, "couldn't announce an import's progress");
                    }
                }
            });
        }
        Ok(id)
    }

    /// Keeps each imported song's match as its pick (M46.53), so playing it asks the match's source
    /// for its stream before any search. A match with a stream of its own already plays it as the
    /// song's source; one that's a YouTube video plays its audio file through the YouTube source
    /// installed (the embed alone isn't the song's, M46.48).
    async fn keep_matches(&self, songs: &[&MatchedSong]) {
        let resolver = self
            .app
            .state::<Arc<crate::resolver::ResolverService>>()
            .inner()
            .clone();
        let db = self.app.state::<crate::startup::AppDb>().inner().0.clone();
        for matched in songs {
            let Some((video_id, title, artist)) = matched_video(matched) else {
                continue;
            };
            let Some(pick) = resolver.youtube_audio_pick(&video_id).await else {
                // Without a YouTube source the song is searched for, as before.
                return;
            };
            for key in pick_keys(matched) {
                if let Err(error) = raygun_db::streams::keep_match(
                    &db,
                    &key,
                    &pick.provider_id,
                    &pick.candidate_id,
                    &title,
                    artist.as_deref(),
                    now_ms(),
                )
                .await
                {
                    tracing::warn!(%error, "couldn't keep an imported song's match");
                }
            }
        }
    }

    /// Gives every saved copy of a song the source the user picked (M46.31): copies saved under
    /// another artist credit or shown with names the user gave it too, the song in the queue and
    /// the resolver's remembered pick (M46.48, `CollectionsService::assign_source`).
    pub async fn assign_source(
        &self,
        song: TrackInput,
        chosen: raygun_model::OnlineTrack,
    ) -> Result<(), String> {
        let choice = raygun_model::SourceChoice {
            assign: true,
            source_name: None,
            title: false,
            artist: false,
            album: false,
            cover: false,
        };
        self.app
            .state::<CollectionsService>()
            .assign_source(song, chosen, choice)
            .await
    }

    /// Which saved songs no source had (M46.31).
    pub async fn unavailable(&self, keys: Vec<String>) -> Result<Vec<String>, String> {
        let db = &self.app.state::<crate::startup::AppDb>().inner().0;
        raygun_db::collections::unavailable(db, &keys)
            .await
            .map_err(|error| error.to_string())
    }
}

/// The ways screenshots can be read here, best first, each saying whether it's ready (v2.0.0).
#[tauri::command]
#[specta::specta]
pub async fn import_readers(
    import: tauri::State<'_, ImportService>,
) -> Result<Vec<ImportReader>, String> {
    Ok(import.readers().await)
}

/// Whether the person's Spotify app's secret is kept (Settings → Import), and its last
/// characters; `None` when there's none.
#[tauri::command]
#[specta::specta]
pub fn import_spotify_secret_hint(ai: tauri::State<'_, AiService>) -> Option<String> {
    ai.spotify_secret_hint()
}

/// Keeps the Spotify app's secret, or forgets it when empty.
#[tauri::command]
#[specta::specta]
pub fn import_set_spotify_secret(
    ai: tauri::State<'_, AiService>,
    secret: String,
) -> Result<(), String> {
    ai.set_spotify_secret(Some(&secret))
}

/// Sources' answers kept per imported song (M46.31).
const CANDIDATES: usize = 5;

/// The YouTube video an imported song matched, with the names it matched under: the answer its
/// source (a lookup under the match's names) came from, when that answer is a YouTube video.
fn matched_video(matched: &MatchedSong) -> Option<(String, String, Option<String>)> {
    matched.matched_by.as_ref()?;
    let Some(MediaSource::Lookup { title, artist, .. }) = &matched.source else {
        return None;
    };
    matched.candidates.iter().find_map(|candidate| {
        let track = &candidate.track;
        if track.title != *title || track.artist != *artist {
            return None;
        }
        match &track.source {
            Some(MediaSource::YouTube { video_id }) => {
                Some((video_id.clone(), title.clone(), artist.clone()))
            }
            _ => None,
        }
    })
}

/// The keys an imported song plays under: its own names, and the match's it plays by.
fn pick_keys(matched: &MatchedSong) -> Vec<String> {
    let mut keys = vec![raygun_resolver::track_key(&TrackQuery {
        title: matched.song.title.clone(),
        artist: matched.song.artist.clone(),
        album: None,
        duration_ms: None,
    })];
    if let Some(MediaSource::Lookup { title, artist, .. }) = &matched.source {
        keys.push(raygun_resolver::track_key(&TrackQuery {
            title: title.clone(),
            artist: artist.clone(),
            album: None,
            duration_ms: None,
        }));
    }
    keys.dedup();
    keys
}

/// The album an imported song gets (M46.46): the one its matches name, spelled as they spell it.
/// An imported album that none of them names (a misread screenshot) gives way to the album most
/// of them give; without matches that name albums, the imported one stays.
fn agreed_album(
    imported: Option<&str>,
    candidates: &[raygun_model::SongCandidate],
) -> Option<String> {
    let albums: Vec<&str> = candidates
        .iter()
        .filter(|candidate| candidate.points >= MIN_SCORE)
        .filter_map(|candidate| candidate.track.album.as_deref())
        .map(str::trim)
        .filter(|album| !album.is_empty())
        .collect();
    let imported = imported.map(str::trim).filter(|album| !album.is_empty());
    if let Some(imported) = imported {
        if let Some(near) = albums.iter().find(|album| near_album(imported, album)) {
            return Some((*near).to_owned());
        }
        if albums.is_empty() {
            return Some(imported.to_owned());
        }
    }
    // The album most matches give; a tie goes to the better match's.
    let mut counts: Vec<(String, usize, &str)> = Vec::new();
    for album in &albums {
        let key = raygun_resolver::normalize(album);
        match counts.iter_mut().find(|(other, _, _)| *other == key) {
            Some((_, count, _)) => *count += 1,
            None => counts.push((key, 1, album)),
        }
    }
    let best = counts.iter().map(|(_, count, _)| *count).max()?;
    counts
        .iter()
        .find(|(_, count, _)| *count == best)
        .map(|(_, _, album)| (*album).to_owned())
}

/// Two album names that are the same album, give or take a misread letter or an edition.
fn near_album(a: &str, b: &str) -> bool {
    let (a, b) = (raygun_resolver::normalize(a), raygun_resolver::normalize(b));
    if a.is_empty() || b.is_empty() {
        return false;
    }
    a == b || a.contains(&b) || b.contains(&a) || bigram_dice(&a, &b) >= 0.75
}

/// How alike two texts are by their letter pairs (0 to 1).
fn bigram_dice(a: &str, b: &str) -> f64 {
    let pairs = |text: &str| -> Vec<(char, char)> {
        let chars: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
        chars.windows(2).map(|pair| (pair[0], pair[1])).collect()
    };
    let (left, mut right) = (pairs(a), pairs(b));
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let total = left.len() + right.len();
    let mut shared = 0;
    for pair in left {
        if let Some(at) = right.iter().position(|other| *other == pair) {
            right.swap_remove(at);
            shared += 1;
        }
    }
    (2 * shared) as f64 / total as f64
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

/// The sources' answers for a song that score at least the import minimum, best first, at most
/// `limit` and one per source (M46.31: the sources list).
pub async fn find_songs(
    extensions: &ExtensionService,
    query: &TrackQuery,
    limit: usize,
) -> Vec<raygun_model::SongCandidate> {
    let terms = [query.artist.as_deref().unwrap_or_default(), &query.title].join(" ");
    best_answers(query, extensions.search(terms.trim()).await, limit)
}

/// [`find_songs`] over answers already in: each source's best answer for `query` that names its
/// artist and scores at least the import minimum, best first, at most `limit`.
pub fn best_answers(
    query: &TrackQuery,
    results: Vec<raygun_model::ProviderSearchResult>,
    limit: usize,
) -> Vec<raygun_model::SongCandidate> {
    let mut best: Vec<raygun_model::SongCandidate> = Vec::new();
    for result in results {
        let mut top: Option<raygun_model::SongCandidate> = None;
        for track in result.tracks {
            let candidate = Candidate {
                provider_id: result.provider.id.clone(),
                provider_name: result.provider.name.clone(),
                id: track.title.clone(),
                title: track.title.clone(),
                artist: track.artist.clone(),
                duration_ms: track.duration_ms,
                quality: None,
                bitrate_kbps: None,
                source: track.source.clone(),
            };
            if !artist_agrees(query, &candidate) {
                continue;
            }
            let points = score(query, &candidate, &History::default());
            if points >= MIN_SCORE && top.as_ref().is_none_or(|kept| points > kept.points) {
                top = Some(raygun_model::SongCandidate {
                    provider_id: result.provider.id.clone(),
                    provider_name: result.provider.name.clone(),
                    track,
                    points,
                });
            }
        }
        best.extend(top);
    }
    best.sort_by_key(|candidate| std::cmp::Reverse(candidate.points));
    best.truncate(limit);
    best
}

/// The sources' best answer for a song: `(points, source name, track)`, searching by artist and
/// title and scoring with the resolver. Imports (M46.21) and length lookups (M46.26) use it.
pub async fn find_song(
    extensions: &ExtensionService,
    query: &TrackQuery,
) -> Option<(i32, String, raygun_model::OnlineTrack)> {
    let terms = [query.artist.as_deref().unwrap_or_default(), &query.title].join(" ");
    let results = extensions.search(terms.trim()).await;
    let mut best: Option<(i32, String, raygun_model::OnlineTrack)> = None;
    for result in results {
        for track in result.tracks {
            let candidate = Candidate {
                provider_id: result.provider.id.clone(),
                provider_name: result.provider.name.clone(),
                id: track.title.clone(),
                title: track.title.clone(),
                artist: track.artist.clone(),
                duration_ms: track.duration_ms,
                quality: None,
                bitrate_kbps: None,
                source: track.source.clone(),
            };
            if !artist_agrees(query, &candidate) {
                continue;
            }
            let points = score(query, &candidate, &History::default());
            if best.as_ref().is_none_or(|(top, _, _)| points > *top) {
                best = Some((points, result.provider.name.clone(), track));
            }
        }
    }
    best
}

#[cfg(test)]
mod album_tests {
    use super::*;

    fn candidate(album: Option<&str>, points: i32) -> raygun_model::SongCandidate {
        raygun_model::SongCandidate {
            provider_id: "p".into(),
            provider_name: "P".into(),
            points,
            track: raygun_model::OnlineTrack {
                album: album.map(str::to_owned),
                ..serde_json::from_value(serde_json::json!({
                    "provider": { "id": "p", "extensionId": "p", "kind": "music.metadata", "name": "P" },
                    "id": "1", "title": "Song", "artist": "Someone", "album": null,
                    "durationMs": null, "artworkUrl": null, "source": null
                }))
                .unwrap()
            },
        }
    }

    #[test]
    fn the_matches_album_wins_over_a_misread_one() {
        let matches = [
            candidate(Some("Random Access Memories"), 90),
            candidate(Some("Random Access Memories"), 80),
            candidate(Some("Get Lucky (Remixes)"), 85),
        ];
        // A misread album no match names gives way.
        assert_eq!(
            agreed_album(Some("Randon Acces Memoirs 2"), &matches[2..]).as_deref(),
            Some("Get Lucky (Remixes)")
        );
        // Near enough: the sources' spelling.
        assert_eq!(
            agreed_album(Some("random acess memories"), &matches).as_deref(),
            Some("Random Access Memories")
        );
        // None imported: the album most matches give.
        assert_eq!(
            agreed_album(None, &matches).as_deref(),
            Some("Random Access Memories")
        );
        // Matches without albums keep the imported one.
        assert_eq!(
            agreed_album(Some("Mine"), &[candidate(None, 90)]).as_deref(),
            Some("Mine")
        );
    }

    #[test]
    fn a_youtube_match_is_kept_as_the_songs_pick_under_the_names_it_plays_by() {
        let mut video = candidate(None, 90);
        video.track.title = "Song (Official Video)".into();
        video.track.source = Some(MediaSource::YouTube {
            video_id: "dQw4w9WgXcQ".into(),
        });
        let lookup = MediaSource::Lookup {
            title: "Song (Official Video)".into(),
            artist: Some("Someone".into()),
            album: None,
            duration_ms: None,
        };
        let matched = MatchedSong {
            song: ImportedSong {
                title: "Song".into(),
                artist: Some("Someone Else".into()),
                album: None,
                duration_ms: None,
                artwork_url: None,
            },
            matched_by: Some("YouTube".into()),
            source: Some(lookup),
            candidates: vec![candidate(None, 95), video],
            pending: false,
        };
        assert_eq!(
            matched_video(&matched),
            Some((
                "dQw4w9WgXcQ".to_owned(),
                "Song (Official Video)".to_owned(),
                Some("Someone".to_owned())
            ))
        );
        assert_eq!(pick_keys(&matched), ["someone else|song", "someone|song"]);

        // A match with a stream of its own plays it already; one nothing matched has no pick.
        let streamed = MatchedSong {
            source: Some(MediaSource::Url {
                url: "https://example.org/song.mp3".into(),
                headers: Default::default(),
            }),
            ..matched.clone()
        };
        assert_eq!(matched_video(&streamed), None);
        let unmatched = MatchedSong {
            matched_by: None,
            ..matched
        };
        assert_eq!(matched_video(&unmatched), None);
    }
}

#[cfg(test)]
mod tests {
    /// Reads a screenshot of a song list: `OCR_PICTURE=<file> cargo test -p raygun-app --
    /// --ignored screenshot_live --nocapture` (Windows; elsewhere `OCRS_MODELS=<folder>` too).
    #[test]
    #[ignore = "needs a screenshot named by OCR_PICTURE"]
    fn screenshot_live_rows() {
        let picture = std::env::var("OCR_PICTURE").expect("OCR_PICTURE");
        let bytes = std::fs::read(picture).unwrap();
        let models = std::env::var("OCRS_MODELS")
            .ok()
            .map(std::path::PathBuf::from);
        let lines = super::ocr::read(&bytes, models.as_deref()).unwrap();
        for line in &lines {
            println!(
                "{:>6.0} {:>5.0} h{:>3.0}  {}",
                line.y, line.x, line.height, line.text
            );
        }
        let rows = super::parse::screenshot_layout(lines.clone());
        let calls = super::parse::split_rows(&rows, &Default::default());
        println!("split rows: {calls:?}");
        let songs = super::parse::screenshot_rows(lines);
        for song in &songs {
            println!("{:?} — {:?}", song.title, song.artist);
        }
        assert!(songs.len() >= 3);
    }

    /// How well each on-device reader reads the screenshot fixtures (`tests/fixtures/screenshots`):
    /// songs read with title and artist right, of those shown. `OCRS_MODELS=<folder>` and
    /// `PADDLE_MODELS=<folder>` name installed model folders; Windows' reader runs on Windows.
    /// `cargo test -p raygun-app-music --release -- --ignored screenshot_accuracy --nocapture`
    #[test]
    #[ignore = "needs the readers' models (OCRS_MODELS, PADDLE_MODELS)"]
    fn screenshot_accuracy() {
        use super::ocr::Engine;
        let folder =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/screenshots");
        let expected: std::collections::BTreeMap<String, Vec<serde_json::Value>> =
            serde_json::from_str(&std::fs::read_to_string(folder.join("expected.json")).unwrap())
                .unwrap();
        let mut engines: Vec<(&str, Engine)> = Vec::new();
        if super::ocr::built_in() {
            engines.push(("windows", Engine::Windows));
        }
        if let Ok(dir) = std::env::var("OCRS_MODELS") {
            engines.push(("ocrs", Engine::Ocrs(dir.into())));
        }
        if let Ok(dir) = std::env::var("PADDLE_MODELS") {
            engines.push(("paddle", Engine::Paddle(dir.into())));
        }
        let same =
            |a: &str, b: &str| raygun_resolver::normalize(a) == raygun_resolver::normalize(b);
        for (name, engine) in engines {
            let loaded = engine.load().unwrap();
            let (mut right, mut shown) = (0, 0);
            for (picture, songs) in &expected {
                let bytes = std::fs::read(folder.join(picture)).unwrap();
                let started = std::time::Instant::now();
                let lines = loaded.read(&bytes).unwrap();
                if std::env::var_os("OCR_LINES").is_some() {
                    for line in &lines {
                        println!(
                            "    {:>6.0} {:>5.0} h{:>3.0}  {}",
                            line.y, line.x, line.height, line.text
                        );
                    }
                }
                let read = super::parse::screenshot_rows(lines);
                let found = songs
                    .iter()
                    .filter(|song| {
                        read.iter().any(|row| {
                            same(&row.title, song["title"].as_str().unwrap())
                                && same(
                                    row.artist.as_deref().unwrap_or_default(),
                                    song["artist"].as_str().unwrap(),
                                )
                        })
                    })
                    .count();
                println!(
                    "{name:>8} {picture:<18} {found}/{} right, {} rows, {} ms",
                    songs.len(),
                    read.len(),
                    started.elapsed().as_millis()
                );
                for row in &read {
                    println!("           {:?} — {:?}", row.title, row.artist);
                }
                right += found;
                shown += songs.len();
            }
            println!("{name:>8} total {right}/{shown}");
        }
    }

    /// Spotify's real embed page: `cargo test -p raygun-app -- --ignored spotify_live --nocapture`.
    #[tokio::test]
    #[ignore = "needs network access to open.spotify.com"]
    async fn spotify_live_playlist_and_album() {
        let client = raygun_net::http_client().unwrap();
        for (kind, id) in [
            ("playlist", "37i9dQZF1DXcBWIGoYBM5M"),
            ("album", "2ANVost0y2y52ema1E9xAZ"),
        ] {
            let html = client
                .get(format!("https://open.spotify.com/embed/{kind}/{id}"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            let list = super::parse::spotify_embed(&html).unwrap();
            println!(
                "{kind}: {} — {} songs, first {:?}",
                list.name,
                list.songs.len(),
                list.songs.first().map(|song| (&song.title, &song.artist))
            );
            assert!(list.songs.len() >= 5, "{kind}");
            assert!(list.songs.iter().all(|song| song.artist.is_some()));
        }
    }
}
