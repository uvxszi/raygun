//! The app-wide NSFW switch (M35). Extensions rated `nsfw` drop out of the provider lists while it
//! is off, and every extension can read it (`raygun.content.showNsfw()`) to filter its results.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

/// Shared by the host thread, the extensions and the app; cloning shares the same switch.
/// It also holds the sources the user disabled (M46.8): their providers drop out like NSFW ones.
#[derive(Debug, Clone, Default)]
pub struct ContentFilter(Arc<AtomicBool>, Arc<RwLock<HashSet<String>>>);

impl ContentFilter {
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
