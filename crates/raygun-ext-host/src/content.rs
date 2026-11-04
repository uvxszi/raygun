//! The app-wide NSFW switch (M35). Extensions rated `nsfw` drop out of the provider lists while it
//! is off, and every extension can read it (`raygun.content.showNsfw()`) to filter its results.
//! Music has its own switch (M46.36): Explicit music covers explicit songs and 18+ music sources.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

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
/// It also holds the sources the user disabled (M46.8): their providers drop out like NSFW ones.
#[derive(Debug, Clone, Default)]
pub struct ContentFilter(
    Arc<AtomicBool>,
    Arc<RwLock<HashSet<String>>>,
    /// Explicit music hidden; shown by default.
    Arc<AtomicBool>,
);

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
        if rating != "nsfw" {
            return true;
        }
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

    /// Whether the provider (`<extension>/<provider>`) is disabled.
    pub fn is_disabled(&self, provider_id: &str) -> bool {
        self.1
            .read()
            .is_ok_and(|disabled| disabled.contains(provider_id))
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

    #[tokio::test]
    async fn adult_sources_are_left_out_only_inside_the_scope() {
        assert!(!adult_left_out());
        assert!(without_adult_sources(true, async { adult_left_out() }).await);
        assert!(!without_adult_sources(false, async { adult_left_out() }).await);
        assert!(!adult_left_out());
    }
}
