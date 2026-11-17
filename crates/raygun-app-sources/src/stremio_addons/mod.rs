//! Stremio's addon protocol (`protocol`), addons that list addons (`addon_catalogs`), and the
//! addon list as Sources reaches it ([`Addons`]: raygun-app's `StremioService`, set at start).

pub mod addon_catalogs;
pub mod protocol;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use futures_util::future::join_all;
use raygun_model::StremioAddon;

/// An answer on its way.
pub type Answer<'a, T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send + 'a>>;

/// The Stremio addons added, as Sources and repositories change them.
pub trait AddonList: Send + Sync {
    /// The addons added.
    fn addons(&self) -> Answer<'_, Vec<StremioAddon>>;
    /// Adds the addon whose manifest is at `url`.
    fn add<'a>(&'a self, url: &'a str) -> Answer<'a, StremioAddon>;
    /// Removes an addon.
    fn remove<'a>(&'a self, id: &'a str) -> Answer<'a, ()>;
    /// What adding the manifest read at `url` would add.
    fn preview_manifest<'a>(
        &'a self,
        url: &'a str,
        manifest: protocol::Manifest,
    ) -> Answer<'a, StremioAddon>;
}

/// The addon list, as Tauri state.
pub type Addons = Arc<dyn AddonList>;

/// Runs `tasks` with at most `limit` at once, keeping their order. (A semaphore rather than a
/// buffered stream: the futures borrow `self`, and spawned sessions need them `Send`.)
pub async fn at_most<T, F: Future<Output = T>>(
    limit: usize,
    tasks: impl IntoIterator<Item = F>,
) -> Vec<T> {
    let permits = tokio::sync::Semaphore::new(limit);
    let permits = &permits;
    join_all(tasks.into_iter().map(|task| async move {
        let _permit = permits.acquire().await;
        task.await
    }))
    .await
}
