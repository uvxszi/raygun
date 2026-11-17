//! How a show's or book's episodes list (M46.48): by date, number, title or length, either way,
//! or unplayed first; and only unplayed, started or downloaded ones. Each show and book keeps
//! its own; podcasts start newest first and books in chapter order.

use std::cmp::Ordering;

use raygun_model::{
    EpisodeDownloadState, EpisodeFilter, EpisodeOrder, EpisodeSort, PodcastEpisode,
};

/// The order a kind starts with.
pub fn default_for(book: bool) -> EpisodeOrder {
    if book {
        EpisodeOrder {
            sort: EpisodeSort::Number,
            descending: false,
            filters: Vec::new(),
        }
    } else {
        EpisodeOrder {
            sort: EpisodeSort::Date,
            descending: true,
            filters: Vec::new(),
        }
    }
}

/// An order as stored: only the sort is needed.
#[derive(serde::Deserialize)]
struct Stored {
    sort: EpisodeSort,
    #[serde(default)]
    descending: bool,
    #[serde(default)]
    filters: Vec<EpisodeFilter>,
}

/// A stored order (JSON), or the kind's own when there's none or it can't be read.
pub fn stored(json: Option<&str>, book: bool) -> EpisodeOrder {
    json.and_then(|json| serde_json::from_str::<Stored>(json).ok())
        .map(|stored| {
            let mut filters = stored.filters;
            filters.sort_by_key(|filter| *filter as u8);
            filters.dedup();
            EpisodeOrder {
                sort: stored.sort,
                descending: stored.descending,
                filters,
            }
        })
        .unwrap_or_else(|| default_for(book))
}

/// `episodes` (as stored: newest first, or a book's chapters in order) the way `order` lists them.
pub fn arrange(episodes: Vec<PodcastEpisode>, order: &EpisodeOrder) -> Vec<PodcastEpisode> {
    let mut kept: Vec<(usize, PodcastEpisode)> = episodes
        .into_iter()
        .enumerate()
        .filter(|(_, episode)| order.filters.iter().all(|filter| passes(episode, *filter)))
        .collect();
    let flip = |ordering: Ordering| {
        if order.descending {
            ordering.reverse()
        } else {
            ordering
        }
    };
    kept.sort_by(|(a_index, a), (b_index, b)| {
        let by_key = match order.sort {
            EpisodeSort::Date => missing_last(a.published_at, b.published_at, flip),
            EpisodeSort::Number => missing_last(a.number, b.number, flip),
            EpisodeSort::Duration => missing_last(a.duration_ms, b.duration_ms, flip),
            EpisodeSort::Title => flip(natural(&a.title, &b.title)),
            EpisodeSort::Unplayed => progress_rank(a).cmp(&progress_rank(b)),
        };
        by_key.then(a_index.cmp(b_index))
    });
    kept.into_iter().map(|(_, episode)| episode).collect()
}

fn passes(episode: &PodcastEpisode, filter: EpisodeFilter) -> bool {
    match filter {
        EpisodeFilter::Unplayed => !episode.played,
        EpisodeFilter::Started => !episode.played && episode.position_ms > 0.0,
        EpisodeFilter::Downloaded => episode
            .download
            .as_ref()
            .is_some_and(|download| download.state == EpisodeDownloadState::Done),
    }
}

/// Started first, then the rest not played, then the played.
fn progress_rank(episode: &PodcastEpisode) -> u8 {
    match (episode.played, episode.position_ms > 0.0) {
        (false, true) => 0,
        (false, false) => 1,
        (true, _) => 2,
    }
}

/// Values compared `flip`'s way, with missing ones last either way.
fn missing_last<T: PartialOrd>(
    a: Option<T>,
    b: Option<T>,
    flip: impl Fn(Ordering) -> Ordering,
) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => flip(a.partial_cmp(&b).unwrap_or(Ordering::Equal)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// Titles compared as people read them: case aside, and "Chapter 2" before "Chapter 10".
pub fn natural(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let number = |chars: &mut std::iter::Peekable<std::str::Chars<'_>>| {
                    let mut digits = String::new();
                    while let Some(digit) = chars.peek().copied().filter(char::is_ascii_digit) {
                        digits.push(digit);
                        chars.next();
                    }
                    let trimmed = digits.trim_start_matches('0').to_owned();
                    (trimmed.len(), trimmed)
                };
                let (left, right) = (number(&mut a), number(&mut b));
                match left.cmp(&right) {
                    Ordering::Equal => {}
                    other => return other,
                }
            }
            (Some(x), Some(y)) => {
                let (x, y) = (x.to_lowercase().next(), y.to_lowercase().next());
                match x.cmp(&y) {
                    Ordering::Equal => {
                        a.next();
                        b.next();
                    }
                    other => return other,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use raygun_model::EpisodeDownload;

    fn episode(id: &str, number: u32, published: Option<f64>) -> PodcastEpisode {
        PodcastEpisode {
            id: id.into(),
            podcast_id: "p".into(),
            podcast_title: "Show".into(),
            title: format!("Chapter {number}"),
            description: None,
            published_at: published,
            duration_ms: Some(f64::from(number) * 1000.0),
            position_ms: 0.0,
            played: false,
            artwork_url: None,
            video: false,
            download: None,
            number: Some(number),
        }
    }

    fn ids(episodes: &[PodcastEpisode]) -> Vec<&str> {
        episodes.iter().map(|episode| episode.id.as_str()).collect()
    }

    #[test]
    fn sorts_either_way_with_missing_values_last() {
        let list = vec![
            episode("c", 3, Some(300.0)),
            episode("b", 2, None),
            episode("a", 1, Some(100.0)),
            episode("j", 10, Some(1000.0)),
        ];
        let order = |sort, descending| EpisodeOrder {
            sort,
            descending,
            filters: Vec::new(),
        };
        assert_eq!(
            ids(&arrange(list.clone(), &order(EpisodeSort::Date, true))),
            ["j", "c", "a", "b"]
        );
        assert_eq!(
            ids(&arrange(list.clone(), &order(EpisodeSort::Date, false))),
            ["a", "c", "j", "b"]
        );
        assert_eq!(
            ids(&arrange(list.clone(), &order(EpisodeSort::Number, false))),
            ["a", "b", "c", "j"]
        );
        assert_eq!(
            ids(&arrange(list.clone(), &order(EpisodeSort::Title, false))),
            ["a", "b", "c", "j"],
            "Chapter 10 comes after Chapter 3"
        );
        assert_eq!(
            ids(&arrange(list.clone(), &order(EpisodeSort::Duration, true))),
            ["j", "c", "b", "a"]
        );
    }

    #[test]
    fn unplayed_first_and_filters() {
        let mut list = vec![
            episode("a", 1, Some(100.0)),
            episode("b", 2, Some(200.0)),
            episode("c", 3, Some(300.0)),
            episode("d", 4, Some(400.0)),
        ];
        list[0].played = true;
        list[2].position_ms = 5_000.0;
        list[3].download = Some(EpisodeDownload {
            state: EpisodeDownloadState::Done,
            received_bytes: 1.0,
            total_bytes: None,
            error: None,
        });
        let unplayed_first = EpisodeOrder {
            sort: EpisodeSort::Unplayed,
            descending: false,
            filters: Vec::new(),
        };
        assert_eq!(
            ids(&arrange(list.clone(), &unplayed_first)),
            ["c", "b", "d", "a"]
        );
        let only = |filters: Vec<EpisodeFilter>| EpisodeOrder {
            filters,
            ..default_for(true)
        };
        assert_eq!(
            ids(&arrange(list.clone(), &only(vec![EpisodeFilter::Unplayed]))),
            ["b", "c", "d"]
        );
        assert_eq!(
            ids(&arrange(list.clone(), &only(vec![EpisodeFilter::Started]))),
            ["c"]
        );
        assert_eq!(
            ids(&arrange(list, &only(vec![EpisodeFilter::Downloaded]))),
            ["d"]
        );
    }

    #[test]
    fn stored_orders_and_defaults() {
        assert_eq!(stored(None, true), default_for(true));
        assert_eq!(stored(Some("not json"), false), default_for(false));
        let order = stored(
            Some(
                r#"{"sort":"title","descending":true,"filters":["downloaded","unplayed","unplayed"]}"#,
            ),
            false,
        );
        assert_eq!(order.sort, EpisodeSort::Title);
        assert_eq!(
            order.filters,
            [EpisodeFilter::Unplayed, EpisodeFilter::Downloaded]
        );
        assert_eq!(
            stored(Some(r#"{"sort":"duration"}"#), true),
            EpisodeOrder {
                sort: EpisodeSort::Duration,
                descending: false,
                filters: Vec::new(),
            },
            "only the sort is needed"
        );
        assert_eq!(natural("Chapter 2", "chapter 10"), Ordering::Less);
        assert_eq!(natural("Part 002", "Part 2"), Ordering::Equal);
        assert_eq!(natural("b", "A"), Ordering::Greater);
    }
}
