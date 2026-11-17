//! The extension host as the app's crates reach it: the app's `ExtensionService` is registered at
//! start as an [`Extensions`] state, so crates below the one that owns it can call sources.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use raygun_ext_host::ExtensionHost;

/// Extensions once loaded: `ready` waits while they load and holds them loaded meanwhile.
pub trait ExtensionAccess: Send + Sync {
    /// The host, once extensions are loaded; the guard keeps them so while held.
    #[allow(clippy::type_complexity)]
    fn ready(
        &self,
    ) -> Pin<
        Box<
            dyn Future<Output = (tokio::sync::RwLockReadGuard<'_, ()>, &ExtensionHost)> + Send + '_,
        >,
    >;
}

/// The extension service, as Tauri state.
pub type Extensions = Arc<dyn ExtensionAccess>;
