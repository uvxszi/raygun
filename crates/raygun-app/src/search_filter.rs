//! Search's "No 18+" and "No explicit" switches (`Settings::search_filter`), on top of what Show
//! NSFW and Explicit music already hide. Where a search fans out, sources rated for adults aren't
//! asked; what comes back loses titles marked adult and songs marked explicit. The UI filters
//! answers it kept from before the same way (`search/searchFilter.ts`).

use raygun_model::{
    MangaCard, MangaSource, OnlineTrack, ProviderSearchResult, SearchFilterPrefs,
    SearchListingsPrefs,
};
use tauri::Manager;

/// The switches as stored; both off when the settings can't be read.
pub async fn current(app: &tauri::AppHandle) -> SearchFilterPrefs {
    let db = app.state::<crate::startup::AppDb>().inner().0.clone();
    crate::settings::load(&db)
        .await
        .map(|settings| settings.search_filter)
        .unwrap_or_default()
}

/// How searches look through the listings of sources whose search fails or finds nothing
/// (`Settings::search_listings`), as stored; the defaults when the settings can't be read.
pub async fn listings(app: &tauri::AppHandle) -> SearchListingsPrefs {
    let db = app.state::<crate::startup::AppDb>().inner().0.clone();
    crate::settings::load(&db)
        .await
        .map(|settings| settings.search_listings)
        .unwrap_or_default()
}

/// Whether music searches leave out sources rated for adults: either switch does, since an 18+
/// music source is an explicit one (M46.36).
pub fn skips_adult_music(prefs: SearchFilterPrefs) -> bool {
    prefs.no_adult || prefs.no_explicit
}

/// Whether a manga, novel or video source is asked.
pub fn asks_source(prefs: SearchFilterPrefs, source: &MangaSource) -> bool {
    !(prefs.no_adult && source.adult)
}

/// Leaves out titles marked adult.
pub fn keep_cards(prefs: SearchFilterPrefs, cards: &mut Vec<MangaCard>) {
    if prefs.no_adult {
        cards.retain(|card| !card.adult);
    }
}

/// Leaves out songs marked explicit.
pub fn keep_tracks(prefs: SearchFilterPrefs, tracks: &mut Vec<OnlineTrack>) {
    if prefs.no_explicit {
        tracks.retain(|track| !track.explicit);
    }
}

/// A music source's answer without explicit songs, and nothing from a source rated for adults.
pub fn keep_songs(prefs: SearchFilterPrefs, result: &mut ProviderSearchResult) {
    if skips_adult_music(prefs) && result.provider.adult {
        result.tracks.clear();
    }
    keep_tracks(prefs, &mut result.tracks);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(id: &str, adult: bool) -> MangaCard {
        MangaCard {
            source_id: "src".into(),
            id: id.into(),
            title: id.into(),
            cover_url: None,
            in_library: false,
            adult,
            from_listing: false,
        }
    }

    fn track(title: &str, explicit: bool) -> OnlineTrack {
        OnlineTrack {
            external_id: None,
            title: title.into(),
            artist: None,
            album: None,
            duration_ms: None,
            artwork_url: None,
            source: None,
            explicit,
        }
    }

    const OFF: SearchFilterPrefs = SearchFilterPrefs {
        no_adult: false,
        no_explicit: false,
    };
    const NO_ADULT: SearchFilterPrefs = SearchFilterPrefs {
        no_adult: true,
        no_explicit: false,
    };
    const NO_EXPLICIT: SearchFilterPrefs = SearchFilterPrefs {
        no_adult: false,
        no_explicit: true,
    };

    #[test]
    fn no_adult_leaves_out_adult_titles_and_sources() {
        let mut cards = vec![card("a", false), card("b", true)];
        keep_cards(OFF, &mut cards);
        assert_eq!(cards.len(), 2);
        keep_cards(NO_EXPLICIT, &mut cards);
        assert_eq!(cards.len(), 2);
        keep_cards(NO_ADULT, &mut cards);
        assert_eq!(
            cards
                .iter()
                .map(|card| card.id.as_str())
                .collect::<Vec<_>>(),
            ["a"]
        );

        let mut source = MangaSource {
            id: "src".into(),
            name: "Source".into(),
            extension_id: "ext".into(),
            enabled: true,
            search_pill: None,
            adult: true,
        };
        assert!(asks_source(OFF, &source));
        assert!(!asks_source(NO_ADULT, &source));
        source.adult = false;
        assert!(asks_source(NO_ADULT, &source));
    }

    #[test]
    fn no_explicit_leaves_out_explicit_songs() {
        let mut tracks = vec![track("Clean", false), track("Explicit", true)];
        keep_tracks(NO_ADULT, &mut tracks);
        assert_eq!(tracks.len(), 2);
        keep_tracks(NO_EXPLICIT, &mut tracks);
        assert_eq!(
            tracks
                .iter()
                .map(|track| track.title.as_str())
                .collect::<Vec<_>>(),
            ["Clean"]
        );
    }

    #[test]
    fn either_switch_skips_adult_music_sources() {
        assert!(!skips_adult_music(OFF));
        assert!(skips_adult_music(NO_ADULT));
        assert!(skips_adult_music(NO_EXPLICIT));
    }
}
