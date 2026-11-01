//! The app-wide NSFW switch (M35). Extensions rated `nsfw` drop out of the provider lists while it
//! is off, and every extension can read it (`raygun.content.showNsfw()`) to filter its results.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Shared by the host thread, the extensions and the app; cloning shares the same switch.
#[derive(Debug, Clone, Default)]
pub struct ContentFilter(Arc<AtomicBool>);

impl ContentFilter {
    pub fn show_nsfw(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub fn set_show_nsfw(&self, show: bool) {
        self.0.store(show, Ordering::Relaxed);
    }
}
