//! Episodes queued without a link: several at once from a title page or the library, and the
//! episodes an export downloads first. Such a row's `url` names the episode
//! ([`pending_url`]); when its turn comes, the worker looks up the episode's links as a playback
//! session would (every server or addon, ordered by what played and worked before and the
//! preferred picture size) and downloads the best one that can be saved, trying the next ones
//! when it fails.

use futures_util::future::join_all;
use raygun_db::titles;
use raygun_model::{VideoLinkView, VideoRequest};
use tauri::{AppHandle, Manager};

use crate::catalog::{now_ms, to_message};
use crate::stremio::{StremioPlayTarget, StremioService};
use crate::video_session::{
    EpisodeRef, Group, GroupState, Link, Origin, Ranking, VideoSessions, link_label,
    mark_mismatches, order_groups,
};
use crate::video_sources::{self, VideoSourceService};

/// The `url` of a row whose link is looked up when it downloads.
const PENDING: &str = "raygun-episode:";
/// Links tried for one episode before it fails.
pub const CANDIDATES: usize = 3;
/// How long one server's links may take to arrive.
const LINKS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
/// The subtitle memory of Stremio titles (subtitles come from the subtitle addons, shared by
/// every stream).
pub const STREMIO_SUBTITLES: &str = "stremio";

pub fn pending_url(episode: &EpisodeRef) -> String {
    format!(
        "{PENDING}{}",
        serde_json::to_string(episode).unwrap_or_default()
    )
}

/// The episode a pending row names.
pub fn parse_pending(url: &str) -> Option<EpisodeRef> {
    serde_json::from_str(url.strip_prefix(PENDING)?).ok()
}

/// A link a download can save: a web address that isn't DASH, a YouTube video, a live stream or a
/// page that plays in its site's own player, or a torrent (listed as failed while torrents can't
/// play, so taken only while they can).
pub fn downloadable(request: &VideoRequest) -> bool {
    if raygun_torrent::is_torrent(&request.url, request.format.as_deref()) {
        return true;
    }
    let url = request.url.to_lowercase();
    (url.starts_with("https://") || url.starts_with("http://"))
        && !url.contains(".mpd")
        && !request.live
        && !matches!(
            request.format.as_deref(),
            Some("dash")
                | Some(raygun_ext_host::video::TORRENT)
                | Some(raygun_ext_host::video::PAGE)
        )
}

/// The links to try, best first: as a session would order them, leaving out ones that can't be
/// saved or have failed, and those naming another episode when others are there.
pub fn best_links(mut groups: Vec<Group>, ranking: &Ranking) -> Vec<VideoRequest> {
    order_groups(&mut groups, ranking);
    let mut links: Vec<(&Group, &Link)> = groups
        .iter()
        .filter(|group| group.state == GroupState::Ready)
        .flat_map(|group| group.links.iter().map(move |link| (group, link)))
        .filter(|(_, link)| link.failed.is_none() && downloadable(&link.request))
        .collect();
    links.sort_by_cached_key(|(group, link)| ranking.link_key(&group.name, link));
    if links.iter().any(|(_, link)| !link.mismatch) {
        links.retain(|(_, link)| !link.mismatch);
    }
    links
        .into_iter()
        .take(CANDIDATES)
        .map(|(_, link)| link.request.clone())
        .collect()
}

/// An episode's links to download from, best first, and the source whose subtitle choice
/// applies.
pub async fn candidates(
    app: &AppHandle,
    episode: &EpisodeRef,
) -> Result<(Vec<VideoRequest>, String), String> {
    let sessions = app.state::<VideoSessions>();
    let (groups, subtitles_source) = match &episode.origin {
        Origin::Source { title_id } => (
            source_groups(app, title_id, &episode.episode).await?,
            source_of(app, title_id).await?,
        ),
        Origin::Stremio { kind, id } => {
            let (title, poster, episodes) = sessions.title_episodes(&episode.origin).await?;
            let found = episodes
                .iter()
                .find(|item| item.remote_id == episode.episode)
                .ok_or("This episode is no longer listed")?;
            let target = StremioPlayTarget {
                kind: kind.clone(),
                meta_id: Some(id.clone()),
                video_id: found.remote_id.clone(),
                subtitle: raygun_resolver::video_match::adds_to_title(&title, &found.label)
                    .then(|| found.label.clone()),
                title,
                poster,
            };
            let db = app.state::<crate::startup::AppDb>().inner().0.clone();
            let language = crate::video::subtitle_language(&db).await;
            let stremio = app.state::<std::sync::Arc<StremioService>>();
            let mut groups = stremio.playable(&target, &language).await?;
            mark_mismatches(&mut groups, &found.numbers);
            (groups, STREMIO_SUBTITLES.to_owned())
        }
    };
    let (memory, health, resolution) = sessions.ranking_inputs(&episode.origin).await;
    let ranking = Ranking::new(&memory, &health, resolution, now_ms());
    let group_key = episode.origin.group_key();
    let mut links = best_links(groups, &ranking);
    for link in &mut links {
        link.group.get_or_insert_with(|| group_key.clone());
    }
    if links.is_empty() {
        return Err("No source had a link that can be downloaded".into());
    }
    Ok((links, subtitles_source))
}

async fn source_of(app: &AppHandle, title_id: &str) -> Result<String, String> {
    let db = app.state::<crate::startup::AppDb>().inner().0.clone();
    Ok(titles::title(&db, title_id)
        .await
        .map_err(to_message)?
        .ok_or("This title is no longer stored")?
        .fields
        .source_id)
}

/// A stored episode's servers with their links (each server's looked up at once, bounded).
async fn source_groups(
    app: &AppHandle,
    title_id: &str,
    remote_id: &str,
) -> Result<Vec<Group>, String> {
    let db = app.state::<crate::startup::AppDb>().inner().0.clone();
    let row = titles::title(&db, title_id)
        .await
        .map_err(to_message)?
        .ok_or("This title is no longer stored")?;
    let units = titles::units(&db, title_id).await.map_err(to_message)?;
    let single = units.len() == 1;
    let unit = units
        .iter()
        .find(|unit| unit.fields.remote_id == remote_id)
        .ok_or("This episode is no longer listed")?;
    let sources = app.state::<VideoSourceService>();
    let source_id = &row.fields.source_id;
    let servers = sources.servers(source_id, remote_id).await?;
    let to_links = |links: Vec<VideoLinkView>| -> Vec<Link> {
        links
            .into_iter()
            .map(|link| Link {
                label: link_label(
                    &row.fields.title,
                    link.label.clone().unwrap_or_else(|| {
                        link.quality
                            .map_or_else(|| "Video".to_owned(), |height| format!("{height}p"))
                    }),
                ),
                tag: None,
                failed: None,
                mismatch: false,
                height: link.quality,
                size: None,
                request: video_sources::link_request(&row, unit, single, link),
            })
            .collect()
    };
    let lookups = servers.into_iter().map(|server| {
        let sources = &sources;
        async move {
            let links = match server.links {
                Some(links) => Ok(links),
                None => tokio::time::timeout(LINKS_TIMEOUT, sources.links(source_id, &server.id))
                    .await
                    .unwrap_or_else(|_| Err("The server didn't answer".into())),
            };
            (server.id, server.name, links)
        }
    });
    Ok(join_all(lookups)
        .await
        .into_iter()
        .map(|(id, name, links)| match links {
            Ok(links) => Group::finished(id, name, to_links(links), None),
            Err(error) => Group::finished(id, name, Vec::new(), Some(error)),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use raygun_model::VideoResolution;

    use super::*;
    use crate::video_session::{Remembered, SourceHealth};

    fn request(url: &str, format: Option<&str>) -> VideoRequest {
        VideoRequest {
            key: "source:s:1".into(),
            title: "Show".into(),
            subtitle: None,
            poster: None,
            url: url.into(),
            headers: Default::default(),
            format: format.map(str::to_owned),
            subtitles: Vec::new(),
            group: None,
            chapters: Vec::new(),
            anime: None,
            live: false,
        }
    }

    fn link(label: &str, url: &str, height: u32, mismatch: bool) -> Link {
        Link {
            label: label.into(),
            tag: None,
            failed: None,
            mismatch,
            height: Some(height),
            size: None,
            request: request(url, None),
        }
    }

    #[test]
    fn pending_rows_name_their_episode() {
        let episode = EpisodeRef {
            origin: Origin::Stremio {
                kind: "series".into(),
                id: "tt1".into(),
            },
            episode: "tt1:1:2".into(),
        };
        let url = pending_url(&episode);
        assert!(url.starts_with(PENDING));
        assert_eq!(parse_pending(&url), Some(episode));
        assert_eq!(parse_pending("https://cdn/film.mp4"), None);
    }

    #[test]
    fn only_savable_links_are_tried_best_first() {
        assert!(!downloadable(&request("youtube:abc", None)));
        assert!(!downloadable(&request("https://cdn/a.mpd", None)));
        assert!(downloadable(&request("magnet:?xt", Some("torrent"))));
        assert!(downloadable(&request("https://cdn/a.m3u8", Some("hls"))));
        assert!(!downloadable(&request(
            "https://player.twitch.tv/?channel=a",
            Some("page")
        )));
        let live = VideoRequest {
            live: true,
            ..request("https://cdn/live.m3u8", Some("hls"))
        };
        assert!(!downloadable(&live), "a live stream never ends");

        let groups = vec![
            Group::finished(
                "a".into(),
                "Alpha".into(),
                vec![
                    link("480p", "https://a/480.mp4", 480, false),
                    link("dash", "https://a/x.mpd", 1080, false),
                ],
                None,
            ),
            Group::finished(
                "b".into(),
                "Beta".into(),
                vec![
                    link("1080p", "https://b/1080.mp4", 1080, false),
                    link("other episode", "https://b/wrong.mp4", 1080, true),
                ],
                None,
            ),
            Group::finished("c".into(), "Gamma".into(), Vec::new(), Some("down".into())),
        ];
        let memory = Remembered::default();
        let health = SourceHealth::default();
        let ranking = Ranking::new(&memory, &health, VideoResolution::default(), 1);
        let urls: Vec<String> = best_links(groups, &ranking)
            .into_iter()
            .map(|request| request.url)
            .collect();
        assert_eq!(urls, vec!["https://b/1080.mp4", "https://a/480.mp4"]);
    }
}
