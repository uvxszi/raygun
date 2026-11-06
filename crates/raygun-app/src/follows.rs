//! Followed artists (M46.42). Following puts an artist in the library (Library → Audio → Artists)
//! and quick access (M46.41); the bell beside Follow turns on alerts for new releases, checked in
//! the background about once a day through the music sources' discographies (M46.36). A release
//! the artist didn't have at the last check is announced once (a system notification and
//! `ArtistReleasesEvent`) and listed in Updates → Releases. Tapping the notification opens the
//! release: an album over its artist's page, a single or an EP marked on the artist's page.
//!
//! Kept on this device (`local_meta`): the follows with what each artist's releases were at the
//! last check, and the latest news.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use raygun_model::{ArtistReleaseNews, FollowedArtist};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;
use tauri_specta::Event;
use tokio::sync::Mutex;

use crate::events::ArtistReleasesEvent;
use crate::extensions::ExtensionService;
use crate::startup::AppDb;

const FOLLOWS: &str = "music.followedArtists";
const NEWS: &str = "music.artistReleases";
/// News kept at most, newest first.
const NEWS_KEPT: usize = 100;
/// How often an artist with alerts is checked.
const CHECK_EVERY_MS: f64 = 24.0 * 60.0 * 60.0 * 1000.0;
/// How often the background loop looks for artists due a check.
const LOOP_EVERY: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    name: String,
    image_url: Option<String>,
    followed_at: f64,
    alerts: bool,
    /// When its releases were last checked (unix ms); 0 before the first check.
    #[serde(default)]
    checked_at: f64,
    /// The releases it had then (`provider/id`), to tell new ones apart.
    #[serde(default)]
    seen: Vec<String>,
}

pub struct ArtistFollows {
    app: AppHandle,
    lock: Mutex<()>,
}

fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as f64)
        .unwrap_or_default()
}

fn same(left: &str, right: &str) -> bool {
    raygun_resolver::normalize(left) == raygun_resolver::normalize(right)
}

fn release_key(album: &raygun_model::MusicAlbum) -> String {
    format!("{}/{}", album.provider.id, album.id)
}

/// A route's path with its search, encoded as a URL's query.
fn route(path: &str, search: &[(&str, &str)]) -> String {
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in search {
        query.append_pair(key, value);
    }
    format!("{path}?{}", query.finish())
}

/// What tapping a release's notification opens: an album's page over its artist's (Back leads
/// there); a single, an EP or another kind of release on the artist's page, marked in the
/// discography (`reveal`: the UI's `releaseReveal` names each release's tile that way).
fn release_route(artist: &str, album: &raygun_model::MusicAlbum) -> String {
    use raygun_model::ReleaseKind;
    let reveal = format!("release:{}", release_key(album));
    match album.kind {
        Some(ReleaseKind::Single | ReleaseKind::Ep | ReleaseKind::Other) => {
            route("/artist", &[("name", artist), ("reveal", &reveal)])
        }
        Some(ReleaseKind::Album | ReleaseKind::Compilation | ReleaseKind::Live) | None => format!(
            "{}\n{}",
            route("/artist", &[("name", artist)]),
            route(
                "/music/album",
                &[("provider", &album.provider.id), ("id", &album.id)]
            ),
        ),
    }
}

/// The notification for a release found: it opens the release when tapped.
fn release_notice(item: &ArtistReleaseNews) -> crate::notifications::Notice {
    crate::notifications::Notice {
        id: crate::notifications::notice_id(&release_key(&item.album)),
        channel: &crate::notifications::RELEASES,
        title: format!("New from {}", item.artist),
        text: item.album.title.clone(),
        route: release_route(&item.artist, &item.album),
    }
}

/// Releases in `found` that weren't `seen`, from this year or later (old ones that only now
/// reached a source aren't news).
fn new_releases(
    seen: &HashSet<String>,
    found: &[raygun_model::MusicAlbum],
    this_year: u32,
) -> Vec<raygun_model::MusicAlbum> {
    found
        .iter()
        .filter(|album| !seen.contains(&release_key(album)))
        .filter(|album| album.year.is_some_and(|year| year >= this_year))
        .cloned()
        .collect()
}

fn this_year() -> u32 {
    // Days since 1970 to a year, near enough for "this year" (leap days drift by hours).
    let days = now_ms() / 86_400_000.0;
    1970 + (days / 365.2425) as u32
}

impl ArtistFollows {
    pub fn new(app: AppHandle) -> Arc<Self> {
        Arc::new(Self {
            app,
            lock: Mutex::new(()),
        })
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().0.clone()
    }

    async fn stored(&self) -> Vec<Stored> {
        raygun_db::local_value(&self.db(), FOLLOWS)
            .await
            .ok()
            .flatten()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    async fn store(&self, follows: &[Stored]) -> Result<(), String> {
        let json = serde_json::to_string(follows).map_err(|error| error.to_string())?;
        raygun_db::set_local_value(&self.db(), FOLLOWS, &json)
            .await
            .map_err(|error| error.to_string())?;
        let _ = ArtistReleasesEvent.emit(&self.app);
        Ok(())
    }

    /// Followed artists, the latest followed first.
    pub async fn list(&self) -> Vec<FollowedArtist> {
        let mut list: Vec<FollowedArtist> = self
            .stored()
            .await
            .into_iter()
            .map(|stored| FollowedArtist {
                name: stored.name,
                image_url: stored.image_url,
                followed_at: stored.followed_at,
                alerts: stored.alerts,
            })
            .collect();
        list.sort_by(|a, b| b.followed_at.total_cmp(&a.followed_at));
        list
    }

    pub async fn follow(&self, name: &str, image_url: Option<String>) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("No artist named".into());
        }
        // The picture's own address: a proxy link from the page lasts only as long as this run.
        let image_url = image_url.map(|url| {
            self.app
                .try_state::<raygun_net::ImageLinks>()
                .map_or(url.clone(), |images| images.original(&url))
        });
        {
            let _turn = self.lock.lock().await;
            let mut follows = self.stored().await;
            if follows.iter().any(|stored| same(&stored.name, name)) {
                return Ok(());
            }
            follows.push(Stored {
                name: name.to_owned(),
                image_url: image_url.clone(),
                followed_at: now_ms(),
                ..Stored::default()
            });
            self.store(&follows).await?;
        }
        let search = [("name".to_owned(), name.to_owned())].into_iter().collect();
        crate::quick_access::add(
            &self.app,
            crate::quick_access::QuickPin {
                kind: "artist",
                to: "/artist".to_owned(),
                search,
                name: name.to_owned(),
                artwork_url: image_url,
            },
        )
        .await;
        Ok(())
    }

    pub async fn unfollow(&self, name: &str) -> Result<(), String> {
        let removed = {
            let _turn = self.lock.lock().await;
            let mut follows = self.stored().await;
            let before = follows.len();
            let gone: Vec<String> = follows
                .iter()
                .filter(|stored| same(&stored.name, name))
                .map(|stored| stored.name.clone())
                .collect();
            follows.retain(|stored| !same(&stored.name, name));
            if follows.len() != before {
                self.store(&follows).await?;
            }
            gone
        };
        for name in removed {
            let search = [("name".to_owned(), name)].into_iter().collect();
            crate::quick_access::remove(&self.app, "/artist", &search).await;
        }
        Ok(())
    }

    /// Turns release alerts on or off; on asks the system to allow notifications.
    pub async fn set_alerts(&self, name: &str, on: bool) -> Result<(), String> {
        {
            let _turn = self.lock.lock().await;
            let mut follows = self.stored().await;
            let Some(stored) = follows.iter_mut().find(|stored| same(&stored.name, name)) else {
                return Err("Follow the artist first".into());
            };
            stored.alerts = on;
            self.store(&follows).await?;
        }
        if on {
            let _ = self.app.notification().request_permission();
            // The first check notes what's out now, so only later releases are news.
            self.check_due(true).await;
        }
        Ok(())
    }

    /// Checks every followed artist with alerts now (Updates' refresh), not waiting for the day.
    pub async fn check_now(&self) {
        self.check_due(true).await;
    }

    /// The latest releases found for followed artists, newest first.
    pub async fn news(&self) -> Vec<ArtistReleaseNews> {
        raygun_db::local_value(&self.db(), NEWS)
            .await
            .ok()
            .flatten()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    /// Checks the artists with alerts that are due (all of them when `now`); new releases are
    /// kept as news and announced.
    async fn check_due(&self, now: bool) {
        let follows = self.stored().await;
        let due: Vec<Stored> = follows
            .into_iter()
            .filter(|stored| {
                stored.alerts && (now || now_ms() - stored.checked_at >= CHECK_EVERY_MS)
            })
            .collect();
        if due.is_empty() {
            return;
        }
        let extensions = self.app.state::<Arc<ExtensionService>>().inner().clone();
        let mut fresh: Vec<ArtistReleaseNews> = Vec::new();
        let mut checked: Vec<(String, Vec<String>)> = Vec::new();
        for artist in due {
            let found = extensions.artist_releases(&artist.name).await;
            if found.is_empty() {
                continue;
            }
            let seen: HashSet<String> = artist.seen.iter().cloned().collect();
            // The first check only notes what's out.
            if artist.checked_at > 0.0 {
                for album in new_releases(&seen, &found, this_year()) {
                    fresh.push(ArtistReleaseNews {
                        artist: artist.name.clone(),
                        album,
                        found_at: now_ms(),
                    });
                }
            }
            checked.push((artist.name, found.iter().map(release_key).collect()));
        }
        {
            let _turn = self.lock.lock().await;
            let mut follows = self.stored().await;
            for (name, seen) in checked {
                if let Some(stored) = follows.iter_mut().find(|stored| same(&stored.name, &name)) {
                    stored.checked_at = now_ms();
                    stored.seen = seen;
                }
            }
            let _ = self.store(&follows).await;
        }
        if fresh.is_empty() {
            return;
        }
        let mut news = self.news().await;
        for item in fresh.iter().rev() {
            news.insert(0, item.clone());
        }
        news.truncate(NEWS_KEPT);
        if let Ok(json) = serde_json::to_string(&news) {
            let _ = raygun_db::set_local_value(&self.db(), NEWS, &json).await;
        }
        for item in &fresh {
            crate::notifications::show_notice(&self.app, release_notice(item)).await;
        }
        let _ = ArtistReleasesEvent.emit(&self.app);
    }

    /// Checks followed artists with alerts in the background, about once a day each.
    pub fn start(self: &Arc<Self>) {
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            // Leave startup to the rest of the app.
            tokio::time::sleep(Duration::from_secs(90)).await;
            loop {
                service.check_due(false).await;
                crate::device_state::rest(LOOP_EVERY).await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn album(id: &str, year: Option<u32>) -> raygun_model::MusicAlbum {
        raygun_model::MusicAlbum {
            provider: raygun_model::ProviderInfo {
                adult: false,
                id: "dz".into(),
                extension_id: "dz".into(),
                kind: "music.metadata".into(),
                name: "Tunes".into(),
                search_pill: None,
            },
            id: id.into(),
            title: id.into(),
            artist: Some("Someone".into()),
            year,
            artwork_url: None,
            track_count: None,
            kind: None,
        }
    }

    #[test]
    fn a_release_notification_opens_the_album_over_its_artist() {
        let item = ArtistReleaseNews {
            artist: "Nina Simone & Friends".into(),
            album: raygun_model::MusicAlbum {
                kind: Some(raygun_model::ReleaseKind::Album),
                ..album("a/1", Some(2026))
            },
            found_at: 0.0,
        };
        let notice = release_notice(&item);
        assert_eq!(notice.title, "New from Nina Simone & Friends");
        assert_eq!(notice.text, "a/1");
        assert_eq!(notice.channel.id, crate::notifications::RELEASES.id);
        // The artist's page first, the album over it; names and ids are encoded.
        assert_eq!(
            notice.route,
            "/artist?name=Nina+Simone+%26+Friends\n/music/album?provider=dz&id=a%2F1"
        );
        // A release of no known kind is an album, as in the discography.
        let unknown = raygun_model::MusicAlbum {
            kind: None,
            ..item.album.clone()
        };
        assert!(release_route("Nina", &unknown).contains("\n/music/album?"));
    }

    #[test]
    fn a_single_opens_its_artist_with_the_single_marked() {
        for kind in [
            raygun_model::ReleaseKind::Single,
            raygun_model::ReleaseKind::Ep,
        ] {
            let single = raygun_model::MusicAlbum {
                kind: Some(kind),
                ..album("s1", Some(2026))
            };
            assert_eq!(
                release_route("Nina Simone", &single),
                "/artist?name=Nina+Simone&reveal=release%3Adz%2Fs1"
            );
        }
    }

    #[test]
    fn each_release_keeps_its_own_notification() {
        let first = crate::notifications::notice_id("dz/1");
        assert_eq!(first, crate::notifications::notice_id("dz/1"));
        assert_ne!(first, crate::notifications::notice_id("dz/2"));
        assert!(first >= 0 && crate::notifications::notice_id("") >= 0);
    }

    #[test]
    fn only_unseen_releases_of_this_year_are_news() {
        let seen: HashSet<String> = ["dz/old".to_owned()].into_iter().collect();
        let found = vec![
            album("old", Some(2026)),
            album("new", Some(2026)),
            album("reissue", Some(1999)),
            album("undated", None),
        ];
        let news = new_releases(&seen, &found, 2026);
        let ids: Vec<&str> = news.iter().map(|album| album.id.as_str()).collect();
        assert_eq!(ids, ["new"]);
    }
}
