//! Many shows, books, episodes and chapters at once: what a selection in Library → Audio, a
//! show's episodes or a book's chapters does to everything picked (mark played, download, remove
//! downloads, queue). A picked show or book stands for its episodes or chapters.

use std::collections::{HashMap, HashSet};

use raygun_db::podcasts::{self as store, EpisodeDownload, EpisodeRow, PodcastRow};
use raygun_db::{Db, DbError};
use raygun_model::PodcastPicks;

use super::{AUDIOBOOK, resume_point};

/// The most new episodes one show's Download takes.
pub(super) const SHOW_DOWNLOADS: usize = 10;

/// A picked show or book, with its episodes (newest first) or chapters (in order).
pub(super) struct PickedShow {
    pub podcast: PodcastRow,
    pub episodes: Vec<EpisodeRow>,
}

/// What a selection holds: the picked shows and books with their episodes, then the episodes
/// picked on their own (not those of a picked show), each once, in the order they were given.
pub(super) struct Gathered {
    pub shows: Vec<PickedShow>,
    pub episodes: Vec<EpisodeRow>,
}

impl Gathered {
    /// Every episode and chapter the selection holds.
    pub fn all(&self) -> impl Iterator<Item = &EpisodeRow> {
        self.shows
            .iter()
            .flat_map(|show| show.episodes.iter())
            .chain(self.episodes.iter())
    }

    /// The shows and books it touches, each once.
    pub fn podcast_ids(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        self.shows
            .iter()
            .map(|show| show.podcast.id.as_str())
            .chain(self.episodes.iter().map(|row| row.podcast_id.as_str()))
            .filter(|id| seen.insert(*id))
            .map(str::to_owned)
            .collect()
    }
}

/// Reads what `picks` names; shows and episodes that are gone are left out.
pub(super) async fn gather(db: &Db, picks: &PodcastPicks) -> Result<Gathered, DbError> {
    let mut show_ids = HashSet::new();
    let mut shows = Vec::new();
    for id in &picks.shows {
        if !show_ids.insert(id.as_str()) {
            continue;
        }
        if let Some(podcast) = store::podcast(db, id).await? {
            let episodes = store::episodes(db, id).await?;
            shows.push(PickedShow { podcast, episodes });
        }
    }
    let mut episode_ids = HashSet::new();
    let mut episodes = Vec::new();
    for id in &picks.episodes {
        if !episode_ids.insert(id.as_str()) {
            continue;
        }
        if let Some(row) = store::episode(db, id).await?
            && !show_ids.contains(row.podcast_id.as_str())
        {
            episodes.push(row);
        }
    }
    Ok(Gathered { shows, episodes })
}

/// The episodes whose played mark changes to `played`.
pub(super) fn to_mark<'a>(
    episodes: impl IntoIterator<Item = &'a EpisodeRow>,
    played: bool,
) -> Vec<String> {
    episodes
        .into_iter()
        .filter(|row| row.played_at.is_some() != played)
        .map(|row| row.id.clone())
        .collect()
}

/// What Download takes, leaving out what is downloaded or on its way (`busy`): every chapter of a
/// book, a show's new episodes (up to [`SHOW_DOWNLOADS`]), and the episodes picked on their own.
pub(super) fn to_download(
    gathered: &Gathered,
    downloads: &HashMap<String, EpisodeDownload>,
    busy: impl Fn(&str) -> bool,
) -> Vec<String> {
    let wanted = |row: &&EpisodeRow| {
        !busy(&row.id)
            && downloads
                .get(&row.id)
                .is_none_or(|download| download.state != "done")
    };
    let mut ids = Vec::new();
    for show in &gathered.shows {
        if show.podcast.fields.kind == AUDIOBOOK {
            ids.extend(
                show.episodes
                    .iter()
                    .filter(wanted)
                    .map(|row| row.id.clone()),
            );
        } else {
            ids.extend(
                new_episodes(show)
                    .into_iter()
                    .filter(wanted)
                    .take(SHOW_DOWNLOADS)
                    .map(|row| row.id.clone()),
            );
        }
    }
    ids.extend(
        gathered
            .episodes
            .iter()
            .filter(wanted)
            .map(|row| row.id.clone()),
    );
    ids
}

/// A show's new episodes, newest first: the unplayed ones that came after it was followed (what
/// its card counts), else its newest unplayed one.
fn new_episodes(show: &PickedShow) -> Vec<&EpisodeRow> {
    let unplayed = || show.episodes.iter().filter(|row| row.played_at.is_none());
    let new: Vec<&EpisodeRow> = match show.podcast.subscribed_at {
        Some(since) => unplayed().filter(|row| row.found_at > since).collect(),
        None => Vec::new(),
    };
    if new.is_empty() {
        unplayed().take(1).collect()
    } else {
        new
    }
}

/// What Remove downloads takes: every episode of the selection with a download, finished, failed,
/// waiting or under way.
pub(super) fn with_downloads(
    gathered: &Gathered,
    downloads: &HashMap<String, EpisodeDownload>,
    busy: impl Fn(&str) -> bool,
) -> Vec<String> {
    gathered
        .all()
        .filter(|row| downloads.contains_key(&row.id) || busy(&row.id))
        .map(|row| row.id.clone())
        .collect()
}

/// What Play next and Add to queue take, in order: a book from where it goes on to its end (as
/// its Play does), a show's newest unplayed episode, then the episodes picked on their own.
pub(super) fn to_queue(gathered: &Gathered) -> Vec<String> {
    let mut ids = Vec::new();
    for show in &gathered.shows {
        if show.podcast.fields.kind == AUDIOBOOK {
            if let Some(start) = resume_point(&show.episodes) {
                ids.extend(show.episodes[start..].iter().map(|row| row.id.clone()));
            }
        } else if let Some(newest) = show.episodes.iter().find(|row| row.played_at.is_none()) {
            ids.push(newest.id.clone());
        }
    }
    ids.extend(gathered.episodes.iter().map(|row| row.id.clone()));
    ids
}

#[cfg(test)]
mod tests {
    use raygun_db::podcasts::{EpisodeFields, PodcastFields};

    use super::*;

    fn podcast(id: &str, kind: &str, subscribed_at: Option<i64>) -> PodcastRow {
        PodcastRow {
            id: id.into(),
            fields: PodcastFields {
                feed_url: format!("https://feeds.example/{id}.xml"),
                title: id.into(),
                author: None,
                description: None,
                artwork: None,
                link: None,
                kind: kind.into(),
            },
            subscribed_at,
            refreshed_at: None,
            episode_order: None,
            speed: None,
        }
    }

    fn episode(podcast_id: &str, id: &str, played_at: Option<i64>, found_at: i64) -> EpisodeRow {
        EpisodeRow {
            id: id.into(),
            podcast_id: podcast_id.into(),
            fields: EpisodeFields {
                guid: id.into(),
                title: id.into(),
                description: None,
                audio_url: format!("https://cdn.example/{id}.mp3"),
                audio_type: None,
                duration_ms: Some(600_000),
                published_at: None,
                artwork: None,
                number: None,
                transcript: None,
            },
            position_ms: 0,
            played_at,
            found_at,
            listened_at: played_at,
            podcast_title: podcast_id.into(),
            podcast_author: None,
            podcast_artwork: None,
            podcast_kind: "podcast".into(),
            podcast_speed: None,
        }
    }

    fn download(id: &str, state: &str) -> (String, EpisodeDownload) {
        (
            id.to_owned(),
            EpisodeDownload {
                episode_id: id.into(),
                state: state.into(),
                path: None,
                received_bytes: 0,
                total_bytes: None,
                error: None,
            },
        )
    }

    /// A show followed at 100: n3 and n2 came after, n1 is played, old was there before.
    fn show() -> PickedShow {
        PickedShow {
            podcast: podcast("show", "podcast", Some(100)),
            episodes: vec![
                episode("show", "n3", None, 300),
                episode("show", "n2", None, 200),
                episode("show", "n1", Some(250), 150),
                episode("show", "old", None, 50),
            ],
        }
    }

    /// A book whose first chapter is finished.
    fn book() -> PickedShow {
        PickedShow {
            podcast: podcast("book", AUDIOBOOK, Some(10)),
            episodes: vec![
                episode("book", "c1", Some(20), 1),
                episode("book", "c2", None, 1),
                episode("book", "c3", None, 1),
            ],
        }
    }

    fn ids(list: &[String]) -> Vec<&str> {
        list.iter().map(String::as_str).collect()
    }

    #[test]
    fn marking_changes_only_what_differs() {
        let gathered = Gathered {
            shows: vec![book()],
            episodes: vec![episode("show", "n2", None, 200)],
        };
        assert_eq!(ids(&to_mark(gathered.all(), true)), ["c2", "c3", "n2"]);
        assert_eq!(ids(&to_mark(gathered.all(), false)), ["c1"]);
        assert_eq!(gathered.podcast_ids(), ["book", "show"]);
    }

    #[test]
    fn downloads_take_books_whole_shows_new_episodes_and_picked_ones() {
        let gathered = Gathered {
            shows: vec![show(), book()],
            episodes: vec![
                episode("other", "x1", None, 1),
                episode("other", "x2", Some(5), 1),
            ],
        };
        let downloads: HashMap<String, EpisodeDownload> =
            [download("n2", "done"), download("c3", "failed")].into();
        let busy = |id: &str| id == "c2";
        // n2 is downloaded, c2 on its way; a failed one is tried again; a played pick is still
        // downloaded when asked for.
        assert_eq!(
            ids(&to_download(&gathered, &downloads, busy)),
            ["n3", "c1", "c3", "x1", "x2"]
        );
        assert_eq!(
            ids(&with_downloads(&gathered, &downloads, busy)),
            ["n2", "c2", "c3"]
        );
    }

    #[test]
    fn a_show_without_new_episodes_downloads_its_newest_unplayed_one() {
        let mut caught_up = show();
        for row in &mut caught_up.episodes {
            if row.id != "old" {
                row.played_at = Some(400);
            }
        }
        let gathered = Gathered {
            shows: vec![caught_up],
            episodes: Vec::new(),
        };
        assert_eq!(
            ids(&to_download(&gathered, &HashMap::new(), |_| false)),
            ["old"]
        );

        let mut many = show();
        many.episodes = (0..15)
            .map(|index| episode("show", &format!("e{index}"), None, 1000 - index))
            .collect();
        let gathered = Gathered {
            shows: vec![many],
            episodes: Vec::new(),
        };
        assert_eq!(
            to_download(&gathered, &HashMap::new(), |_| false).len(),
            SHOW_DOWNLOADS
        );
    }

    #[test]
    fn queueing_takes_books_from_where_they_go_on_and_shows_newest_unplayed() {
        let gathered = Gathered {
            shows: vec![book(), show()],
            episodes: vec![
                episode("other", "x2", None, 1),
                episode("other", "x1", None, 1),
            ],
        };
        assert_eq!(ids(&to_queue(&gathered)), ["c2", "c3", "n3", "x2", "x1"]);

        let mut finished = book();
        for row in &mut finished.episodes {
            row.played_at = Some(30);
        }
        let gathered = Gathered {
            shows: vec![finished],
            episodes: Vec::new(),
        };
        assert!(
            to_queue(&gathered).is_empty(),
            "a finished book has nothing left"
        );
    }

    #[tokio::test]
    async fn gathering_reads_shows_and_leaves_out_their_picked_episodes() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = Db::open(&temp.path().join("raygun.db")).await.unwrap();
        let fields = |id: &str| PodcastFields {
            feed_url: format!("https://feeds.example/{id}.xml"),
            title: id.into(),
            author: None,
            description: None,
            artwork: None,
            link: None,
            kind: "podcast".into(),
        };
        let item = |guid: &str| EpisodeFields {
            guid: guid.into(),
            title: guid.into(),
            description: None,
            audio_url: format!("https://cdn.example/{guid}.mp3"),
            audio_type: None,
            duration_ms: None,
            published_at: Some(guid.len() as i64),
            artwork: None,
            number: None,
            transcript: None,
        };
        let one = store::upsert_podcast(&db, &fields("one"), 1).await.unwrap();
        let two = store::upsert_podcast(&db, &fields("two"), 1).await.unwrap();
        store::sync_episodes(&db, &one, &[item("a"), item("bb")], 1)
            .await
            .unwrap();
        store::sync_episodes(&db, &two, &[item("c")], 1)
            .await
            .unwrap();
        let in_one = store::episodes(&db, &one).await.unwrap();
        let c = store::episodes(&db, &two).await.unwrap()[0].id.clone();

        let picks = PodcastPicks {
            shows: vec![one.clone(), "gone".into(), one.clone()],
            episodes: vec![in_one[0].id.clone(), c.clone(), c.clone()],
        };
        let gathered = gather(&db, &picks).await.unwrap();
        assert_eq!(
            gathered.shows.len(),
            1,
            "a missing show is left out, a repeat too"
        );
        assert_eq!(gathered.shows[0].episodes.len(), 2);
        assert_eq!(
            gathered
                .episodes
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            [c.as_str()],
            "an episode of a picked show counts once, with its show"
        );
        assert_eq!(gathered.all().count(), 3);
        assert_eq!(gathered.podcast_ids(), [one, two]);
    }
}
