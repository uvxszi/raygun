//! Search's "No 18+" pill (`Settings::search_filter`), which counts only while 18+ content is on
//! (`Settings::show_nsfw`; off, the content filter already hides it). Where a search fans out,
//! sources rated for adults aren't asked; what comes back loses titles marked adult. Explicit songs
//! follow `Settings::explicit_music` alone (the host drops them). The UI filters answers it kept
//! from before the same way (`search/searchFilter.ts`). Library titles aren't filtered here.

use raygun_model::{
    MangaCard, MangaSource, ProviderSearchResult, SearchFilterPrefs, SearchListingsPrefs,
};
use tauri::Manager;

/// The pill as it counts now: on only while 18+ content is shown; off when the settings can't be
/// read.
pub async fn current(app: &tauri::AppHandle) -> SearchFilterPrefs {
    let db = app.state::<crate::startup::AppDb>().inner().0.clone();
    crate::settings::load(&db)
        .await
        .map(|settings| effective(&settings))
        .unwrap_or_default()
}

/// The pill as stored, counted only while 18+ content is on.
pub fn effective(settings: &raygun_model::Settings) -> SearchFilterPrefs {
    SearchFilterPrefs {
        no_adult: settings.show_nsfw && settings.search_filter.no_adult,
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

/// A music source's answer, nothing from a source rated for adults while the pill is on.
pub fn keep_songs(prefs: SearchFilterPrefs, result: &mut ProviderSearchResult) {
    if skips_adult_music(prefs) && result.provider.adult {
        result.tracks.clear();
    }
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

    const OFF: SearchFilterPrefs = SearchFilterPrefs { no_adult: false };
    const NO_ADULT: SearchFilterPrefs = SearchFilterPrefs { no_adult: true };

    #[test]
    fn no_adult_leaves_out_adult_titles_and_sources() {
        let mut cards = vec![card("a", false), card("b", true)];
        keep_cards(OFF, &mut cards);
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
    fn the_pill_counts_only_while_adult_content_is_on() {
        let mut settings = raygun_model::Settings::default();
        settings.search_filter.no_adult = true;
        settings.show_nsfw = false;
        assert_eq!(effective(&settings), OFF);
        settings.show_nsfw = true;
        assert_eq!(effective(&settings), NO_ADULT);
        assert!(skips_adult_music(NO_ADULT));
        assert!(!skips_adult_music(OFF));
    }
}
