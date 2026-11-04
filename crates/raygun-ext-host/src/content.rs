//! The app-wide NSFW switch (M35). Extensions rated `nsfw` drop out of the provider lists while it
//! is off, and every extension can read it (`raygun.content.showNsfw()`) to filter its results.
//! Music has its own switch (M46.36): Explicit music covers explicit songs and 18+ music sources.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use raygun_model::ContentOverrides;

tokio::task_local! {
    /// Set while a search leaves out sources rated for adults (Search's "No 18+").
    static WITHOUT_ADULT: bool;
}

/// Runs `work` with providers rated for adults left out of every provider list it asks for when
/// `skip` is set, so a search doesn't ask them at all. Other calls meanwhile are untouched.
pub async fn without_adult_sources<F: Future>(skip: bool, work: F) -> F::Output {
    WITHOUT_ADULT.scope(skip, work).await
}

/// Whether the running task leaves adult providers out ([`without_adult_sources`]).
pub(crate) fn adult_left_out() -> bool {
    WITHOUT_ADULT.try_with(|skip| *skip).unwrap_or(false)
}

/// Shared by the host thread, the extensions and the app; cloning shares the same switch.
/// It also holds the sources the user disabled (M46.8): their providers drop out like NSFW ones,
/// and the user's own 18+ marks, which decide over a source's rating.
#[derive(Debug, Clone, Default)]
pub struct ContentFilter(
    Arc<AtomicBool>,
    Arc<RwLock<HashSet<String>>>,
    /// Explicit music hidden; shown by default.
    Arc<AtomicBool>,
    Arc<RwLock<Marks>>,
);

/// The user's 18+ marks and, for the repository marks, where each installed extension came from.
#[derive(Debug, Default)]
struct Marks {
    overrides: ContentOverrides,
    /// Extension id → the id of the repository it was installed from.
    repositories: HashMap<String, String>,
}

impl ContentFilter {
    pub fn show_explicit_music(&self) -> bool {
        !self.2.load(Ordering::Relaxed)
    }

    pub fn set_show_explicit_music(&self, show: bool) {
        self.2.store(!show, Ordering::Relaxed);
    }

    /// Whether an extension rated `rating` may give providers of `kind`: music follows Explicit
    /// music, everything else NSFW (M46.36).
    pub fn allows(&self, rating: &str, kind: &str) -> bool {
        rating != "nsfw" || self.allows_adult(kind)
    }

    /// Whether 18+ extensions may give providers of `kind` (see [`Self::allows`]).
    pub fn allows_adult(&self, kind: &str) -> bool {
        if kind.starts_with("music.") {
            self.show_explicit_music()
        } else {
            self.show_nsfw()
        }
    }

    pub fn show_nsfw(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub fn set_show_nsfw(&self, show: bool) {
        self.0.store(show, Ordering::Relaxed);
    }

    /// Puts the user's 18+ marks (`Settings::content_overrides`) into effect.
    pub fn set_overrides(&self, overrides: ContentOverrides) {
        if let Ok(mut marks) = self.3.write() {
            marks.overrides = overrides;
        }
    }

    /// Which repository (by id) each installed extension came from, for repository marks.
    pub fn set_extension_repositories(&self, repositories: HashMap<String, String>) {
        if let Ok(mut marks) = self.3.write() {
            marks.repositories = repositories;
        }
    }

    /// Whether an installed or built-in extension rated `rating` counts as 18+: the user's mark
    /// on it, then on its repository, then its own rating.
    pub fn extension_adult(&self, extension_id: &str, rating: &str) -> bool {
        let own = rating == "nsfw";
        self.3.read().map_or(own, |marks| {
            let repository = marks.repositories.get(extension_id).map(String::as_str);
            marks
                .overrides
                .extension_adult(extension_id, repository, own)
        })
    }

    /// Whether an extension a repository lists (`repository_id`) counts as 18+.
    pub fn listed_adult(&self, extension_id: &str, repository_id: &str, rating: &str) -> bool {
        let own = rating == "nsfw";
        self.3.read().map_or(own, |marks| {
            marks
                .overrides
                .extension_adult(extension_id, Some(repository_id), own)
        })
    }

    /// Whether a Stremio addon (by its stored id) that says `own` counts as 18+.
    pub fn addon_adult(&self, addon_id: &str, own: bool) -> bool {
        self.3
            .read()
            .map_or(own, |marks| marks.overrides.addon_adult(addon_id, own))
    }

    /// Whether the provider (`<extension>/<provider>`) is disabled: itself, or its whole
    /// extension (an extension id in the same list).
    pub fn is_disabled(&self, provider_id: &str) -> bool {
        self.1.read().is_ok_and(|disabled| {
            disabled.contains(provider_id)
                || provider_id
                    .split_once('/')
                    .is_some_and(|(extension, _)| disabled.contains(extension))
        })
    }

    pub fn set_disabled(&self, provider_ids: &[String]) {
        if let Ok(mut disabled) = self.1.write() {
            *disabled = provider_ids.iter().cloned().collect();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use raygun_model::ContentOverride;

    fn marks(
        extensions: &[(&str, ContentOverride)],
        repositories: &[(&str, ContentOverride)],
    ) -> ContentOverrides {
        let map = |pairs: &[(&str, ContentOverride)]| {
            pairs
                .iter()
                .map(|(id, mark)| ((*id).to_owned(), *mark))
                .collect()
        };
        ContentOverrides {
            extensions: map(extensions),
            repositories: map(repositories),
            addons: Default::default(),
        }
    }

    #[test]
    fn the_extension_mark_wins_then_the_repository_then_the_rating() {
        use ContentOverride::{Adult, Default as Own, NotAdult};
        let filter = ContentFilter::default();
        filter.set_extension_repositories(HashMap::from([
            ("a".to_owned(), "repo".to_owned()),
            ("b".to_owned(), "repo".to_owned()),
            ("c".to_owned(), "repo".to_owned()),
        ]));
        // Nothing marked: the source's own rating.
        assert!(filter.extension_adult("a", "nsfw"));
        assert!(!filter.extension_adult("a", "suggestive"));

        filter.set_overrides(marks(
            &[("a", NotAdult), ("b", Adult), ("c", Own)],
            &[("repo", Adult)],
        ));
        assert!(
            !filter.extension_adult("a", "nsfw"),
            "the extension's mark wins"
        );
        assert!(filter.extension_adult("b", "safe"));
        assert!(
            filter.extension_adult("c", "safe"),
            "the repository's mark comes next"
        );
        assert!(
            !filter.extension_adult("elsewhere", "safe"),
            "another repository's own rating"
        );
        assert!(filter.extension_adult("elsewhere", "nsfw"));
        // A repository's listing uses the repository it's listed in.
        assert!(filter.listed_adult("d", "repo", "safe"));
        assert!(!filter.listed_adult("d", "other", "safe"));
        assert!(!filter.listed_adult("a", "repo", "nsfw"));

        filter.set_overrides(marks(&[], &[("repo", NotAdult)]));
        assert!(!filter.extension_adult("c", "nsfw"));
        assert!(!filter.listed_adult("d", "repo", "nsfw"));
    }

    #[test]
    fn addon_marks_decide_over_the_manifest() {
        let filter = ContentFilter::default();
        assert!(filter.addon_adult("x", true));
        filter.set_overrides(ContentOverrides {
            addons: [
                ("x".to_owned(), ContentOverride::NotAdult),
                ("y".to_owned(), ContentOverride::Adult),
            ]
            .into(),
            ..Default::default()
        });
        assert!(!filter.addon_adult("x", true));
        assert!(filter.addon_adult("y", false));
        assert!(!filter.addon_adult("z", false));
    }

    #[test]
    fn normalized_marks_drop_the_ones_left_to_the_source() {
        let overrides = marks(
            &[
                ("a", ContentOverride::Default),
                ("b", ContentOverride::Adult),
            ],
            &[(" ", ContentOverride::Adult)],
        )
        .normalized();
        assert_eq!(overrides.extensions.len(), 1);
        assert!(overrides.repositories.is_empty());
    }

    #[tokio::test]
    async fn adult_sources_are_left_out_only_inside_the_scope() {
        assert!(!adult_left_out());
        assert!(without_adult_sources(true, async { adult_left_out() }).await);
        assert!(!without_adult_sources(false, async { adult_left_out() }).await);
        assert!(!adult_left_out());
    }

    #[test]
    fn an_extension_id_turns_off_every_provider_it_has() {
        let filter = ContentFilter::default();
        filter.set_disabled(&["dev.a".to_owned(), "dev.b/one".to_owned()]);
        assert!(filter.is_disabled("dev.a/books"));
        assert!(filter.is_disabled("dev.a/films"));
        assert!(filter.is_disabled("dev.b/one"));
        assert!(!filter.is_disabled("dev.b/two"));
        assert!(!filter.is_disabled("dev.ab/one"));
    }
}
