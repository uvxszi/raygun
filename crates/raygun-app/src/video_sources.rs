//! Video sources (M42): `video.source` providers through the shared catalogue (sources, browse,
//! search with filters, stored titles and episodes), then an episode's servers resolved one by
//! one, links ordered by the preferred quality, and playback in the M38 player.

use raygun_db::titles::{self, TitleRow, UnitRow};
use raygun_db::video::{self as progress_store, VideoEntry, VideoProgress};
use raygun_ext_host::video::{VIDEO_SOURCE, VideoLink};
use raygun_model::{
    AnimeEpisode, VideoEpisodeView, VideoLinkView, VideoRequest, VideoServerView, VideoSourcePlay,
    VideoSubtitle, VideoTitleView,
};
use raygun_net::ImageLinks;
use tauri::{AppHandle, Manager};

use crate::catalog::{Catalog, is_adult, to_message};

/// `title.kind` of video titles.
const KIND: &str = "video";

pub struct VideoSourceService {
    catalog: Catalog,
}

impl VideoSourceService {
    pub fn new(app: AppHandle, images: ImageLinks) -> Self {
        Self {
            catalog: Catalog::new(app, images, KIND, VIDEO_SOURCE),
        }
    }

    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// A title's page: details and episodes (fetched first when `refresh` is set or they never
    /// were), with watching progress.
    pub async fn title(
        &self,
        source_id: &str,
        remote_id: &str,
        refresh: bool,
    ) -> Result<VideoTitleView, String> {
        // Local Files' videos (M46.27) are stored by the scan; there's no source to ask.
        if source_id == crate::local_files::LOCAL_VIDEO {
            let row = titles::find_title(&self.catalog.db(), source_id, remote_id)
                .await
                .map_err(to_message)?
                .ok_or("This video is no longer in Local Files")?;
            return self.title_view(row).await;
        }
        tracing::info!(
            source = source_id,
            id = remote_id,
            refresh,
            "opening a video title"
        );
        let row = self
            .catalog
            .title_row(source_id, remote_id, refresh)
            .await?;
        let view = self.title_view(row).await;
        tracing::info!(
            source = source_id,
            id = remote_id,
            ok = view.is_ok(),
            "video title ready"
        );
        view
    }

    async fn title_view(&self, row: TitleRow) -> Result<VideoTitleView, String> {
        let db = self.catalog.db();
        let units = titles::units(&db, &row.id).await.map_err(to_message)?;
        let progress =
            progress_store::progress_with_prefix(&db, &content_key(&row.fields.source_id, ""))
                .await
                .map_err(to_message)?;
        let source_name = self
            .catalog
            .sources()
            .await
            .into_iter()
            .find(|source| source.id == row.fields.source_id)
            .map_or_else(
                || {
                    if row.fields.source_id == crate::local_files::LOCAL_VIDEO {
                        "Local files".to_owned()
                    } else {
                        row.fields.source_id.clone()
                    }
                },
                |source| source.name,
            );
        let hidden_nsfw = !self.catalog.extensions().show_nsfw() && is_adult(&row.fields.genres);
        let keys: Vec<String> = units
            .iter()
            .map(|unit| content_key(&row.fields.source_id, &unit.fields.remote_id))
            .collect();
        let downloaded = crate::video_library::downloaded_keys(&db, &keys).await?;
        let fields = row.fields;
        Ok(VideoTitleView {
            episodes: units
                .iter()
                .map(|unit| episode_view(&fields.source_id, unit, &progress, &downloaded))
                .collect(),
            hidden_nsfw,
            id: row.id,
            source_name,
            cover_url: fields
                .cover
                .as_deref()
                .map(|url| self.catalog.cover(url, &fields.cover_headers)),
            source_id: fields.source_id,
            remote_id: fields.remote_id,
            title: fields.title,
            media_type: fields.media_type,
            year: fields.year,
            description: fields.description,
            genres: fields.genres,
            cast: fields.authors,
            directors: fields.artists,
            status: fields.status,
            url: fields.url,
            in_library: row.in_library_at.is_some(),
        })
    }

    /// A title's comments from its source, a page at a time (M46.45).
    pub async fn comments(
        &self,
        source_id: &str,
        remote_id: &str,
        page: Option<&str>,
    ) -> Result<raygun_model::VideoComments, String> {
        let extensions = self.catalog.extensions();
        let (_ready, host) = extensions.ready().await;
        host.video_comments(source_id, remote_id, page).await
    }

    /// An episode's servers; the ones the source resolved already come with their links.
    pub async fn servers(
        &self,
        source_id: &str,
        episode_remote_id: &str,
    ) -> Result<Vec<VideoServerView>, String> {
        // A Local Files video (M46.27) plays its file through the stream proxy.
        if source_id == crate::local_files::LOCAL_VIDEO {
            return self.local_servers(episode_remote_id).await;
        }
        let servers = {
            let extensions = self.catalog.extensions();
            let (_ready, host) = extensions.ready().await;
            host.video_servers(source_id, episode_remote_id).await?
        };
        let preferred = self.preferred_quality().await;
        Ok(servers
            .into_iter()
            .map(|server| VideoServerView {
                links: server.videos.map(|videos| link_views(videos, preferred)),
                id: server.id,
                name: server.name,
            })
            .collect())
    }

    /// One server's links, the preferred one first.
    pub async fn links(
        &self,
        source_id: &str,
        server_id: &str,
    ) -> Result<Vec<VideoLinkView>, String> {
        let videos = {
            let extensions = self.catalog.extensions();
            let (_ready, host) = extensions.ready().await;
            host.video_links(source_id, server_id).await?
        };
        Ok(link_views(videos, self.preferred_quality().await))
    }

    /// Plays a link of a stored episode, resuming where it was left.
    pub async fn play(&self, request: VideoSourcePlay) -> Result<(), String> {
        if request.link.format.as_deref() == Some(raygun_ext_host::video::TORRENT) {
            return Err(
                "This video is a torrent, which Raygun doesn't play: copy its link into a torrent app"
                    .into(),
            );
        }
        let external = request.external;
        let video = self.request(request).await?;
        if external {
            crate::video::play_elsewhere(self.catalog.app(), video).await
        } else {
            crate::video::play(self.catalog.app(), video).await
        }
    }

    /// What the player gets for a link of a stored episode (also what a download saves).
    pub async fn request(&self, request: VideoSourcePlay) -> Result<VideoRequest, String> {
        let (row, unit, single) = self.episode(&request.title_id, &request.episode_id).await?;
        Ok(link_request(&row, &unit, single, request.link))
    }

    /// Saves a title to the video library (M43) or takes it out; episodes watched before it was
    /// saved count as watched.
    pub async fn set_in_library(&self, title_id: &str, in_library: bool) -> Result<(), String> {
        self.catalog.set_in_library(title_id, in_library).await?;
        let db = self.catalog.db();
        if let (true, Some(row)) = (
            in_library,
            titles::title(&db, title_id).await.map_err(to_message)?,
        ) {
            crate::video_library::backfill(&db, &row).await?;
        }
        Ok(())
    }

    /// Marks a stored episode watched or not.
    pub async fn set_watched(
        &self,
        title_id: &str,
        episode_id: &str,
        watched: bool,
    ) -> Result<(), String> {
        let (row, unit, single) = self.episode(title_id, episode_id).await?;
        let entry = VideoEntry {
            key: content_key(&row.fields.source_id, &unit.fields.remote_id),
            subtitle: episode_subtitle(&row, &unit, single),
            title: row.fields.title,
            poster: row.fields.cover,
            // Nothing was played, so there's no link to replay.
            request: "{}".into(),
        };
        crate::video::mark(&self.catalog.db(), &entry, watched).await
    }

    /// A stored title and one of its episodes, and whether it's the only one (a movie).
    async fn episode(
        &self,
        title_id: &str,
        episode_id: &str,
    ) -> Result<(TitleRow, UnitRow, bool), String> {
        let db = self.catalog.db();
        let row = titles::title(&db, title_id)
            .await
            .map_err(to_message)?
            .ok_or_else(|| "This title is no longer stored".to_owned())?;
        let units = titles::units(&db, title_id).await.map_err(to_message)?;
        let single = units.len() == 1;
        let unit = units
            .into_iter()
            .find(|unit| unit.id == episode_id)
            .ok_or_else(|| "This episode is no longer listed".to_owned())?;
        Ok((row, unit, single))
    }

    async fn local_servers(&self, episode_remote_id: &str) -> Result<Vec<VideoServerView>, String> {
        let app = self.catalog.app();
        let path = app
            .state::<std::sync::Arc<crate::local_files::LocalFilesService>>()
            .video_path(episode_remote_id)
            .await?;
        if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Err("This video file is gone".into());
        }
        let url = app
            .state::<crate::playback::PlaybackService>()
            .register_source(&raygun_model::MediaSource::LocalFile {
                path: path.to_string_lossy().into_owned(),
            });
        let video = VideoLink {
            url,
            label: Some("File".into()),
            quality: None,
            headers: Default::default(),
            format: None,
            subtitles: Vec::new(),
            chapters: Vec::new(),
        };
        Ok(vec![VideoServerView {
            id: "local".into(),
            name: "This device".into(),
            links: Some(link_views(vec![video], None)),
        }])
    }

    async fn preferred_quality(&self) -> Option<u32> {
        crate::settings::load(&self.catalog.db())
            .await
            .ok()
            .and_then(|settings| settings.preferred_video_quality)
    }
}

/// The key watching progress is kept under for a source's episode.
pub fn content_key(source_id: &str, episode_remote_id: &str) -> String {
    format!("source:{source_id}:{episode_remote_id}")
}

/// What a session's player and episode list call an episode: its subtitle, else its name.
pub(crate) fn episode_label(row: &TitleRow, unit: &UnitRow, single: bool) -> String {
    episode_subtitle(row, unit, single)
        .filter(|_| !single)
        .unwrap_or_else(|| {
            if single {
                row.fields.title.clone()
            } else {
                unit.fields.name.clone()
            }
        })
}

/// The player's request for a link of a stored episode.
pub(crate) fn link_request(
    row: &TitleRow,
    unit: &UnitRow,
    single: bool,
    link: VideoLinkView,
) -> VideoRequest {
    // Whole episode numbers of anime can be looked up on AniSkip.
    let anime = (row.fields.media_type.as_deref() == Some("anime"))
        .then_some(unit.fields.number)
        .flatten()
        .filter(|number| *number >= 1.0 && number.fract() == 0.0 && *number < 100_000.0)
        .map(|number| AnimeEpisode {
            title: row.fields.title.clone(),
            episode: number as u32,
            mal_id: None,
        });
    VideoRequest {
        group: Some(format!("title:{}", row.id)),
        chapters: link.chapters,
        anime,
        key: content_key(&row.fields.source_id, &unit.fields.remote_id),
        subtitle: episode_subtitle(row, unit, single),
        title: row.fields.title.clone(),
        // The source's own address: a proxy link wouldn't outlive the app.
        poster: row.fields.cover.clone(),
        url: link.url,
        headers: link.headers,
        format: link.format,
        subtitles: link.subtitles,
    }
}

/// "S1 E2 · Name", "E2 · Name", the name alone, or a movie's year.
fn episode_subtitle(row: &TitleRow, unit: &UnitRow, single: bool) -> Option<String> {
    let season = season(unit);
    let number = unit
        .fields
        .number
        .map(raygun_ext_host::manga::format_number);
    let name = &unit.fields.name;
    match (single && season.is_none(), season, number) {
        (true, ..) => row.fields.year.map(|year| year.to_string()),
        (false, Some(season), Some(number)) => Some(format!(
            "S{} E{number} · {name}",
            raygun_ext_host::manga::format_number(season)
        )),
        (false, None, Some(number)) if !name.ends_with(&format!(" {number}")) => {
            Some(format!("E{number} · {name}"))
        }
        _ => Some(name.clone()),
    }
}

/// A video episode's season, kept as its volume.
pub(crate) fn season(unit: &UnitRow) -> Option<f64> {
    unit.fields
        .volume
        .as_deref()
        .and_then(|volume| volume.parse::<f64>().ok())
        .filter(|season| season.is_finite())
}

fn episode_view(
    source_id: &str,
    unit: &UnitRow,
    progress: &[VideoProgress],
    downloaded: &std::collections::HashSet<String>,
) -> VideoEpisodeView {
    let key = content_key(source_id, &unit.fields.remote_id);
    let saved = progress.iter().find(|row| row.entry.key == key);
    VideoEpisodeView {
        id: unit.id.clone(),
        remote_id: unit.fields.remote_id.clone(),
        name: unit.fields.name.clone(),
        number: unit.fields.number,
        season: season(unit),
        date: unit.fields.date.map(|ms| ms as f64),
        position_ms: saved.map_or(0.0, |row| row.position_ms as f64),
        duration_ms: saved.and_then(|row| row.duration_ms).map(|ms| ms as f64),
        watched: saved.is_some_and(|row| row.watched_at.is_some()),
        downloaded: downloaded.contains(&key),
    }
}

fn link_views(mut videos: Vec<VideoLink>, preferred: Option<u32>) -> Vec<VideoLinkView> {
    order_links(&mut videos, preferred);
    videos
        .into_iter()
        .enumerate()
        .map(|(index, video)| VideoLinkView {
            preferred: index == 0,
            url: video.url,
            label: video.label,
            quality: video.quality,
            headers: video.headers,
            format: video.format,
            chapters: video.chapters,
            subtitles: video
                .subtitles
                .into_iter()
                .map(|track| VideoSubtitle {
                    url: track.url,
                    label: track.label.or_else(|| track.lang.clone()),
                    lang: track.lang,
                    format: None,
                })
                .collect(),
        })
        .collect()
}

/// Orders links by the preferred height: at or below it, closest first; then above it, lowest
/// first; unknown heights last. With no preference, the highest first. Ties keep the source's
/// order.
pub fn order_links(videos: &mut [VideoLink], preferred: Option<u32>) {
    videos.sort_by_key(|video| match (video.quality, preferred) {
        (None, _) => (2, 0),
        (Some(height), None) => (0, u32::MAX - height),
        (Some(height), Some(limit)) if height <= limit => (0, limit - height),
        (Some(height), Some(limit)) => (1, height - limit),
    });
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn link(quality: Option<u32>, label: &str) -> VideoLink {
        VideoLink {
            url: format!("https://cdn.example/{label}.mp4"),
            label: Some(label.into()),
            quality,
            headers: BTreeMap::new(),
            format: None,
            subtitles: Vec::new(),
            chapters: Vec::new(),
        }
    }

    fn labels(videos: &[VideoLink]) -> Vec<&str> {
        videos
            .iter()
            .map(|video| video.label.as_deref().unwrap())
            .collect()
    }

    #[test]
    fn links_follow_the_preferred_quality() {
        let all = || {
            vec![
                link(Some(360), "360"),
                link(None, "auto"),
                link(Some(1080), "1080"),
                link(Some(720), "720"),
                link(Some(480), "480"),
            ]
        };
        let mut highest = all();
        order_links(&mut highest, None);
        assert_eq!(labels(&highest), ["1080", "720", "480", "360", "auto"]);

        let mut capped = all();
        order_links(&mut capped, Some(720));
        assert_eq!(labels(&capped), ["720", "480", "360", "1080", "auto"]);

        let mut low = all();
        order_links(&mut low, Some(240));
        assert_eq!(
            labels(&low),
            ["360", "480", "720", "1080", "auto"],
            "nothing small enough: the smallest above it"
        );
    }
}
