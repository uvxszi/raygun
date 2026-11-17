//! Search's filters (`Settings::search_filter`): the "No 18+" pill beside the kinds, and what each
//! kind's Filters sheet switches on. "No 18+" and "18+ only" (videos, comics, books) count only
//! while 18+ content is on (`Settings::show_nsfw`; off, the content filter already hides it), and
//! turn each other off. Audio has "Explicit only" (while Explicit music is on) and "Playable only"
//! (songs that come with a stream), Videos "Playable only" (Stremio titles an addon gives streams
//! for), Comics and Books "Not in library". Where a search fans out, sources the filters leave
//! out aren't asked; what comes back loses what they leave out. Explicit songs otherwise follow
//! `Settings::explicit_music` alone (the host drops them). The UI filters answers it kept from
//! before the same way (`search/searchFilter.ts`). Library titles aren't filtered here.

use raygun_model::{
    MangaCard, MangaSource, MediaSource, OnlineTrack, ProviderSearchResult, SearchFilterPrefs,
    SearchListingsPrefs, StremioItem,
};
use tauri::Manager;

/// The filters as they count now; all off when the settings can't be read.
pub async fn current(app: &tauri::AppHandle) -> SearchFilterPrefs {
    let db = app.state::<crate::startup::AppDb>().inner().0.clone();
    crate::settings::load(&db)
        .await
        .map(|settings| effective(&settings))
        .unwrap_or_default()
}

/// The filters as stored, each counted only while what it needs is on: the pill and "18+ only"
/// while 18+ content is on (never both: the pill wins), "Explicit only" while Explicit music is.
pub fn effective(settings: &raygun_model::Settings) -> SearchFilterPrefs {
    let stored = settings.search_filter;
    let adult = settings.show_nsfw;
    SearchFilterPrefs {
        no_adult: adult && stored.no_adult,
        adult_only: adult && stored.adult_only && !stored.no_adult,
        explicit_only: settings.explicit_music && stored.explicit_only,
        ..stored
    }
}

/// How searches look through the listings of sources whose search fails or finds nothing
/// (`Settings::search_listings`), as stored; the defaults when the settings can't be read.
pub async fn listings(app: &tauri::AppHandle) -> SearchListingsPrefs {
    let db = app.state::<crate::startup::AppDb>().inner().0.clone();
    // Data Saver reads fewer pages while it applies.
    crate::settings::load_in_effect(&db)
        .await
        .map(|settings| settings.search_listings)
        .unwrap_or_default()
}

/// Whether music searches leave out sources rated for adults (with Explicit music off the host
/// leaves them out already, M46.36).
pub fn skips_adult_music(prefs: SearchFilterPrefs) -> bool {
    prefs.no_adult
}

/// Whether a manga, novel or video source is asked: one rated for adults isn't under "No 18+".
/// Under "18+ only" every source is asked, since one not rated for adults may mark titles adult.
pub fn asks_source(prefs: SearchFilterPrefs, source: &MangaSource) -> bool {
    !(prefs.no_adult && source.adult)
}

/// A source's titles as the filters keep them (`medium` is `title.kind`: manga, novel or video):
/// adult ones left out under "No 18+"; under "18+ only" only adult ones, or every one of a source
/// rated for adults; comics or books already in the library left out under "Not in library".
/// Video sources play their own titles, so Videos' "Playable only" keeps them, but not a
/// source's folders (a Kodi add-on's), which are lists rather than something to play.
pub fn keep_cards(
    prefs: SearchFilterPrefs,
    medium: &str,
    source: &MangaSource,
    cards: &mut Vec<MangaCard>,
) {
    let hide_saved = hides_saved(prefs, medium);
    let no_folders = prefs.playable_videos && medium == "video";
    cards.retain(|card| {
        !(prefs.no_adult && card.adult)
            && !(prefs.adult_only && !source.adult && !card.adult)
            && !(hide_saved && card.in_library)
            && !(no_folders && card.folder)
    });
}

/// Whether "Not in library" is on for `medium`'s searches.
fn hides_saved(prefs: SearchFilterPrefs, medium: &str) -> bool {
    match medium {
        "manga" => prefs.hide_saved_comics,
        "novel" => prefs.hide_saved_books,
        _ => false,
    }
}

/// Whether a song comes with a stream of its own (an address, a file, a YouTube video), as
/// Audio's "Playable only" asks; a song its source only describes is looked for when it plays,
/// and may not be found.
pub fn streams_itself(track: &OnlineTrack) -> bool {
    track
        .source
        .as_ref()
        .is_some_and(|source| !matches!(source, MediaSource::Lookup { .. }))
}

/// Whether a song a source found stays: marked explicit under "Explicit only", with a stream of
/// its own under "Playable only".
pub fn keeps_song(prefs: SearchFilterPrefs, track: &OnlineTrack) -> bool {
    (!prefs.explicit_only || track.explicit) && (!prefs.playable_songs || streams_itself(track))
}

/// The filters without Audio's song filters, for a song search that isn't Search's (Find a
/// source lists every source's answer for a song).
pub fn without_song_filters(prefs: SearchFilterPrefs) -> SearchFilterPrefs {
    SearchFilterPrefs {
        explicit_only: false,
        playable_songs: false,
        ..prefs
    }
}

/// A music source's answer as the filters keep it: nothing from a source rated for adults while
/// the pill is on, and only the songs [`keeps_song`] keeps.
pub fn keep_songs(prefs: SearchFilterPrefs, result: &mut ProviderSearchResult) {
    if skips_adult_music(prefs) && result.provider.adult {
        result.tracks.clear();
    }
    result.tracks.retain(|track| keeps_song(prefs, track));
}

/// Whether a Stremio addon's catalogs are searched: one rated for adults isn't under "No 18+",
/// one that isn't isn't under "18+ only" (its titles carry no adult mark of their own).
pub fn asks_addon(prefs: SearchFilterPrefs, adult: bool) -> bool {
    if adult {
        !prefs.no_adult
    } else {
        !prefs.adult_only
    }
}

/// An addon's search results under Videos' "Playable only": the titles an addon gives streams
/// for (`StremioItem::playable`).
pub fn keep_items(prefs: SearchFilterPrefs, items: &mut Vec<StremioItem>) {
    if prefs.playable_videos {
        items.retain(|item| item.playable);
    }
}

#[cfg(test)]
mod tests {
    use raygun_model::ProviderInfo;

    use super::*;

    fn card(id: &str, adult: bool, in_library: bool) -> MangaCard {
        MangaCard {
            source_id: "src".into(),
            id: id.into(),
            title: id.into(),
            cover_url: None,
            in_library,
            adult,
            from_listing: false,
            authors: Vec::new(),
            live: None,
            subtitle: None,
            folder: false,
        }
    }

    fn source(adult: bool) -> MangaSource {
        MangaSource {
            id: "src".into(),
            name: "Source".into(),
            extension_id: "ext".into(),
            enabled: true,
            search_pill: None,
            adult,
        }
    }

    fn ids(cards: &[MangaCard]) -> Vec<&str> {
        cards.iter().map(|card| card.id.as_str()).collect()
    }

    fn track(title: &str, explicit: bool, source: Option<MediaSource>) -> OnlineTrack {
        OnlineTrack {
            external_id: None,
            title: title.into(),
            artist: Some("Band".into()),
            album: None,
            duration_ms: None,
            artwork_url: None,
            source,
            explicit,
        }
    }

    fn stremio(id: &str, playable: bool) -> StremioItem {
        StremioItem {
            kind: "movie".into(),
            id: id.into(),
            name: id.into(),
            poster: None,
            poster_shape: "poster".into(),
            release_info: None,
            rating: None,
            playable,
            background: None,
            trailer: None,
        }
    }

    const OFF: SearchFilterPrefs = SearchFilterPrefs {
        no_adult: false,
        adult_only: false,
        explicit_only: false,
        playable_songs: false,
        playable_videos: false,
        hide_saved_comics: false,
        hide_saved_books: false,
    };
    const NO_ADULT: SearchFilterPrefs = SearchFilterPrefs {
        no_adult: true,
        ..OFF
    };
    const ADULT_ONLY: SearchFilterPrefs = SearchFilterPrefs {
        adult_only: true,
        ..OFF
    };

    #[test]
    fn no_adult_leaves_out_adult_titles_and_sources() {
        let mut cards = vec![card("a", false, false), card("b", true, false)];
        keep_cards(OFF, "manga", &source(false), &mut cards);
        assert_eq!(cards.len(), 2);
        keep_cards(NO_ADULT, "manga", &source(false), &mut cards);
        assert_eq!(ids(&cards), ["a"]);

        assert!(asks_source(OFF, &source(true)));
        assert!(!asks_source(NO_ADULT, &source(true)));
        assert!(asks_source(NO_ADULT, &source(false)));
    }

    #[test]
    fn adult_only_keeps_adult_titles_and_every_title_of_an_adult_source() {
        // Every source is asked: one not rated for adults may still mark titles adult.
        assert!(asks_source(ADULT_ONLY, &source(false)));
        let mut cards = vec![card("a", false, false), card("b", true, false)];
        keep_cards(ADULT_ONLY, "video", &source(false), &mut cards);
        assert_eq!(ids(&cards), ["b"]);

        let mut cards = vec![card("a", false, false), card("b", true, false)];
        keep_cards(ADULT_ONLY, "novel", &source(true), &mut cards);
        assert_eq!(ids(&cards), ["a", "b"]);
    }

    #[test]
    fn not_in_library_counts_for_its_own_kind_only() {
        let comics = SearchFilterPrefs {
            hide_saved_comics: true,
            ..OFF
        };
        let saved = || vec![card("new", false, false), card("saved", false, true)];
        let mut cards = saved();
        keep_cards(comics, "manga", &source(false), &mut cards);
        assert_eq!(ids(&cards), ["new"]);
        let mut cards = saved();
        keep_cards(comics, "novel", &source(false), &mut cards);
        assert_eq!(cards.len(), 2);
        let mut cards = saved();
        keep_cards(
            SearchFilterPrefs {
                hide_saved_books: true,
                ..OFF
            },
            "novel",
            &source(false),
            &mut cards,
        );
        assert_eq!(ids(&cards), ["new"]);
    }

    #[test]
    fn songs_follow_explicit_only_and_playable_only() {
        let url = MediaSource::Url {
            url: "https://example.com/a.mp3".into(),
            headers: Default::default(),
        };
        let lookup = MediaSource::Lookup {
            title: "Described".into(),
            artist: None,
            album: None,
            duration_ms: None,
        };
        let youtube = MediaSource::YouTube {
            video_id: "abc".into(),
        };
        let mut result = ProviderSearchResult {
            provider: ProviderInfo {
                id: "ext/search".into(),
                extension_id: "ext".into(),
                kind: "music.metadata".into(),
                name: "Tunes".into(),
                search_pill: None,
                adult: false,
            },
            tracks: vec![
                track("Clean stream", false, Some(url)),
                track("Rude stream", true, Some(youtube)),
                track("Rude described", true, None),
                track("Clean lookup", false, Some(lookup)),
            ],
            error: None,
            from_listings: false,
        };
        let titles = |result: &ProviderSearchResult| {
            result
                .tracks
                .iter()
                .map(|track| track.title.clone())
                .collect::<Vec<_>>()
        };

        let mut explicit = result.clone();
        keep_songs(
            SearchFilterPrefs {
                explicit_only: true,
                ..OFF
            },
            &mut explicit,
        );
        assert_eq!(titles(&explicit), ["Rude stream", "Rude described"]);

        let mut playable = result.clone();
        keep_songs(
            SearchFilterPrefs {
                playable_songs: true,
                ..OFF
            },
            &mut playable,
        );
        assert_eq!(titles(&playable), ["Clean stream", "Rude stream"]);

        // Find a source asks without the song filters; the pill still counts.
        let both = SearchFilterPrefs {
            no_adult: true,
            explicit_only: true,
            playable_songs: true,
            ..OFF
        };
        let mut unfiltered = result.clone();
        keep_songs(without_song_filters(both), &mut unfiltered);
        assert_eq!(unfiltered.tracks.len(), 4);
        assert!(without_song_filters(both).no_adult);

        keep_songs(both, &mut result);
        assert_eq!(titles(&result), ["Rude stream"]);
    }

    #[test]
    fn addons_and_their_titles_follow_the_video_filters() {
        assert!(asks_addon(OFF, true) && asks_addon(OFF, false));
        assert!(!asks_addon(NO_ADULT, true) && asks_addon(NO_ADULT, false));
        assert!(asks_addon(ADULT_ONLY, true) && !asks_addon(ADULT_ONLY, false));

        let mut items = vec![stremio("tt1", true), stremio("yt1", false)];
        keep_items(OFF, &mut items);
        assert_eq!(items.len(), 2);
        keep_items(
            SearchFilterPrefs {
                playable_videos: true,
                ..OFF
            },
            &mut items,
        );
        assert_eq!(
            items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["tt1"]
        );
    }

    #[test]
    fn playable_only_leaves_out_video_sources_folders() {
        let playable = SearchFilterPrefs {
            playable_videos: true,
            ..OFF
        };
        let cards = || {
            let mut folder = card("folder", false, false);
            folder.folder = true;
            vec![card("title", false, false), folder]
        };
        let mut kept = cards();
        keep_cards(playable, "video", &source(false), &mut kept);
        assert_eq!(ids(&kept), ["title"]);
        let mut kept = cards();
        keep_cards(OFF, "video", &source(false), &mut kept);
        assert_eq!(kept.len(), 2);
    }

    #[test]
    fn filters_count_only_while_what_they_need_is_on() {
        let mut settings = raygun_model::Settings {
            search_filter: SearchFilterPrefs {
                no_adult: true,
                explicit_only: true,
                playable_videos: true,
                ..OFF
            },
            show_nsfw: false,
            explicit_music: false,
            ..raygun_model::Settings::default()
        };
        let now = effective(&settings);
        assert_eq!(
            now,
            SearchFilterPrefs {
                playable_videos: true,
                ..OFF
            }
        );
        assert!(!skips_adult_music(now));

        settings.show_nsfw = true;
        settings.explicit_music = true;
        let now = effective(&settings);
        assert!(now.no_adult && now.explicit_only && skips_adult_music(now));

        // "18+ only" counts while the pill is off, and only while 18+ content is on.
        settings.search_filter.no_adult = false;
        settings.search_filter.adult_only = true;
        assert!(effective(&settings).adult_only);
        settings.search_filter.no_adult = true;
        assert!(!effective(&settings).adult_only);
        settings.search_filter.no_adult = false;
        settings.show_nsfw = false;
        assert!(!effective(&settings).adult_only);
    }
}
