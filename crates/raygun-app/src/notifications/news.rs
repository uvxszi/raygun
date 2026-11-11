//! News worth a sound: new chapters and episodes that library updates and feed refreshes found
//! (one notification per title), and a newer Raygun found by a background check. Each kind has
//! its own switch in `Settings::notifications`; a tap opens what the notification announces.

use raygun_model::NotificationPrefs;

use super::{APP_UPDATES, LIBRARY_UPDATES, Notice, notice_id};

/// What a library notification is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewsKind {
    Comics,
    Books,
    Videos,
    Podcasts,
    Audiobooks,
}

impl NewsKind {
    /// The kind of a library title (`title.kind`).
    pub fn of_title(kind: &str) -> Option<Self> {
        match kind {
            "manga" => Some(Self::Comics),
            "novel" => Some(Self::Books),
            "video" => Some(Self::Videos),
            _ => None,
        }
    }

    /// Whether `prefs` lets this kind post a notification.
    pub fn wanted(self, prefs: &NotificationPrefs) -> bool {
        match self {
            Self::Comics => prefs.comics,
            Self::Books => prefs.books,
            Self::Videos => prefs.videos,
            Self::Podcasts => prefs.podcasts,
            Self::Audiobooks => prefs.audiobooks,
        }
    }

    /// "3 new chapters", "1 new episode".
    fn count(self, count: usize) -> String {
        let unit = match self {
            Self::Comics | Self::Books | Self::Audiobooks => "chapter",
            Self::Videos | Self::Podcasts => "episode",
        };
        let plural = if count == 1 { "" } else { "s" };
        format!("{count} new {unit}{plural}")
    }
}

fn route(path: &str, search: &[(&str, &str)]) -> String {
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in search {
        query.append_pair(key, value);
    }
    format!("{path}?{}", query.finish())
}

/// The page of a library title: comics and books by source, Stremio's series by type and id
/// (stored as `<type>:<id>`), other videos by source.
pub fn title_route(kind: NewsKind, source_id: &str, remote_id: &str) -> String {
    match kind {
        NewsKind::Comics => route("/manga/title", &[("source", source_id), ("id", remote_id)]),
        NewsKind::Books => route("/novel/title", &[("source", source_id), ("id", remote_id)]),
        NewsKind::Videos if source_id == crate::video_library::STREMIO_SOURCE => {
            match remote_id.split_once(':') {
                Some((kind, id)) => route("/watch/title", &[("type", kind), ("id", id)]),
                None => route("/watch/title", &[("id", remote_id)]),
            }
        }
        NewsKind::Videos => route("/watch/video", &[("source", source_id), ("id", remote_id)]),
        NewsKind::Podcasts => route("/podcast", &[("id", remote_id)]),
        NewsKind::Audiobooks => route("/audiobook", &[("id", remote_id)]),
    }
}

/// "One Piece: 3 new chapters", opening the title. The same title's next news replaces it.
pub fn library_notice(
    kind: NewsKind,
    key: &str,
    title: &str,
    count: usize,
    route: String,
) -> Notice {
    Notice {
        id: notice_id(&format!("library:{key}")),
        channel: &LIBRARY_UPDATES,
        title: title.to_owned(),
        text: kind.count(count),
        route,
    }
}

/// A newer Raygun: opens About, where it installs.
pub fn app_update_notice(version: &str) -> Notice {
    Notice {
        id: notice_id("app-update"),
        channel: &APP_UPDATES,
        title: format!("Raygun {version} is out"),
        text: "Open About to update".to_owned(),
        route: "/about".to_owned(),
    }
}

/// Posts each of `news` whose kind is switched on.
pub async fn announce(
    app: &tauri::AppHandle,
    prefs: &NotificationPrefs,
    news: Vec<(NewsKind, Notice)>,
) {
    for (kind, notice) in news {
        if kind.wanted(prefs) {
            super::show_notice(app, notice).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_open_their_own_pages() {
        assert_eq!(
            title_route(NewsKind::Comics, "mangadex", "a b"),
            "/manga/title?source=mangadex&id=a+b"
        );
        assert_eq!(
            title_route(NewsKind::Books, "gutenberg", "7"),
            "/novel/title?source=gutenberg&id=7"
        );
        assert_eq!(
            title_route(NewsKind::Videos, "stremio", "series:tt0944947"),
            "/watch/title?type=series&id=tt0944947"
        );
        assert_eq!(
            title_route(NewsKind::Videos, "archive", "film"),
            "/watch/video?source=archive&id=film"
        );
        assert_eq!(title_route(NewsKind::Podcasts, "", "p1"), "/podcast?id=p1");
        assert_eq!(
            title_route(NewsKind::Audiobooks, "", "b1"),
            "/audiobook?id=b1"
        );
    }

    #[test]
    fn one_notification_per_title_says_how_many_are_new() {
        let notice = library_notice(NewsKind::Comics, "t1", "One Piece", 3, "/x".into());
        assert_eq!(
            (notice.title.as_str(), notice.text.as_str()),
            ("One Piece", "3 new chapters")
        );
        assert_eq!(notice.channel.id, LIBRARY_UPDATES.id);
        assert!(notice.channel.alerts);
        let again = library_notice(NewsKind::Comics, "t1", "One Piece", 1, "/x".into());
        assert_eq!(again.id, notice.id);
        assert_eq!(again.text, "1 new chapter");
        assert_eq!(
            library_notice(NewsKind::Podcasts, "p", "Show", 2, String::new()).text,
            "2 new episodes"
        );
        assert_ne!(
            library_notice(NewsKind::Comics, "t2", "Other", 1, String::new()).id,
            notice.id
        );
    }

    #[test]
    fn each_kind_follows_its_own_switch() {
        let prefs = NotificationPrefs {
            books: false,
            ..NotificationPrefs::default()
        };
        assert!(NewsKind::Comics.wanted(&prefs));
        assert!(!NewsKind::Books.wanted(&prefs));
        assert_eq!(NewsKind::of_title("video"), Some(NewsKind::Videos));
        assert_eq!(NewsKind::of_title("music"), None);
    }

    #[test]
    fn a_new_version_opens_about() {
        let notice = app_update_notice("1.6.0");
        assert_eq!(notice.title, "Raygun 1.6.0 is out");
        assert_eq!(notice.route, "/about");
        assert!(notice.channel.alerts);
    }
}
