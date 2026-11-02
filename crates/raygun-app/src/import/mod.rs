//! Importing songs from elsewhere (M46.21): a Spotify playlist, album or song link (read from
//! Spotify's public embed page, at most its first 100 songs), a CSV export (Exportify), pasted
//! lines, or screenshots of a song list (read with `ocr`). Each song is then matched against the
//! music sources: the best match fills in its artwork, album and length (which helps playback find
//! the right recording), and a source's own stream when it has one. A playlist made from them
//! plays through the resolver like any other.

pub mod ocr;
pub mod parse;

use std::sync::Arc;

use raygun_model::{ImportedList, ImportedSong, MatchedSong, MediaSource, TrackInput, TrackQuery};
use raygun_resolver::{Candidate, History, MIN_SCORE, score};
use tauri::{AppHandle, Manager};

use crate::ai::AiService;
use crate::collections::CollectionsService;
use crate::extensions::ExtensionService;
use crate::uploads::Uploads;

/// Songs read from an embed page, at most (Spotify's embed stops there).
const EMBED_LIMIT: usize = 100;
/// Songs matched at once.
const MATCH_TOGETHER: usize = 4;

pub struct ImportService {
    app: AppHandle,
    client: reqwest::Client,
}

impl ImportService {
    pub fn new(app: AppHandle, client: reqwest::Client) -> Self {
        Self { app, client }
    }

    fn extensions(&self) -> Arc<ExtensionService> {
        self.app.state::<Arc<ExtensionService>>().inner().clone()
    }

    /// The songs a link points at.
    pub async fn read_link(&self, link: &str) -> Result<ImportedList, String> {
        let spotify = parse::spotify_link(link).ok_or(
            "That isn't a link Raygun can read; paste a Spotify playlist, album or song link",
        )?;
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
        let mut list = parse::spotify_embed(&html)?;
        let capped = list.songs.len() >= EMBED_LIMIT && spotify.kind == "playlist";
        list.songs.truncate(EMBED_LIMIT);
        Ok(ImportedList {
            name: list.name,
            artwork_url: list.artwork_url,
            songs: list.songs,
            capped,
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
        })
    }

    /// Whether reading pictures needs its text reader downloaded first (not on Windows).
    pub fn reader_ready(&self) -> bool {
        ocr::built_in()
            || self
                .app
                .state::<AiService>()
                .installed_dir(ocr::OCRS_MODEL)
                .is_some()
    }

    /// Downloads the text reader for screenshots (12 MB; progress as `AiModelEvent`s).
    pub async fn prepare_reader(&self) -> Result<(), String> {
        if self.reader_ready() {
            return Ok(());
        }
        self.app
            .state::<AiService>()
            .download_model(ocr::OCRS_MODEL)
            .await
    }

    /// Songs in screenshots of song lists, uploaded first (`upload_*`), in the pictures' order.
    pub async fn read_screenshots(&self, upload_ids: Vec<String>) -> Result<ImportedList, String> {
        let models = if ocr::built_in() {
            None
        } else {
            Some(
                self.app
                    .state::<AiService>()
                    .installed_dir(ocr::OCRS_MODEL)
                    .ok_or("Download the text reader first")?,
            )
        };
        let uploads = self.app.state::<Uploads>();
        let mut songs: Vec<ImportedSong> = Vec::new();
        for id in upload_ids {
            let path = uploads.take(&id)?;
            let bytes = tokio::fs::read(&path)
                .await
                .map_err(|error| error.to_string())?;
            let _ = tokio::fs::remove_file(&path).await;
            let models = models.clone();
            let lines = tokio::task::spawn_blocking(move || ocr::read(&bytes, models.as_deref()))
                .await
                .map_err(|error| error.to_string())??;
            tracing::info!(lines = lines.len(), "read a screenshot");
            for song in parse::screenshot_rows(lines) {
                if !songs
                    .iter()
                    .any(|known| known.title == song.title && known.artist == song.artist)
                {
                    songs.push(song);
                }
            }
        }
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
        })
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
        let terms = [song.artist.as_deref().unwrap_or_default(), &song.title].join(" ");
        let results = self.extensions().search(terms.trim()).await;
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
                let points = score(&query, &candidate, &History::default());
                if best.as_ref().is_none_or(|(top, _, _)| points > *top) {
                    best = Some((points, result.provider.name.clone(), track));
                }
            }
        }
        match best {
            Some((points, provider, track)) if points >= MIN_SCORE => {
                // A stream only counts when it plays anywhere (not the YouTube embed, which is a
                // video; the resolver finds a song's stream when it plays).
                let source = track
                    .source
                    .filter(|source| matches!(source, MediaSource::Url { .. }));
                MatchedSong {
                    song: ImportedSong {
                        album: song.album.or(track.album),
                        duration_ms: song.duration_ms.or(track.duration_ms),
                        artwork_url: song.artwork_url.or(track.artwork_url),
                        ..song
                    },
                    matched_by: Some(provider),
                    source,
                }
            }
            _ => MatchedSong {
                song,
                matched_by: None,
                source: None,
            },
        }
    }

    /// Saves songs as a new playlist named `name`; returns its id.
    pub async fn save(&self, name: &str, songs: Vec<MatchedSong>) -> Result<String, String> {
        let collections = self.app.state::<CollectionsService>();
        let playlist = collections.create_playlist(name).await?;
        let tracks = songs
            .into_iter()
            .map(|matched| TrackInput {
                title: matched.song.title,
                artist: matched.song.artist,
                album: matched.song.album,
                duration_ms: matched.song.duration_ms,
                artwork_url: matched.song.artwork_url,
                source: matched.source,
            })
            .collect();
        collections.add_to_playlist(&playlist.id, tracks).await?;
        Ok(playlist.id)
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
        let songs = super::parse::screenshot_rows(lines);
        for song in &songs {
            println!("{:?} — {:?}", song.title, song.artist);
        }
        assert!(songs.len() >= 3);
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
