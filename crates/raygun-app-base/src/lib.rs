//! What every part of the app leans on (M46.56): the events sent to the UI, the database and
//! storage handles, typed settings, the device's state, incognito, local network access and the
//! system's pickers. No domain logic; the app's crates build on it.

pub mod books;
pub mod device_state;
pub mod events;
pub mod extension_access;
pub mod local_network;
pub mod lookup_cache;
pub mod network;
pub mod network_mode;
pub mod news;
pub mod notices;
pub mod picker;
pub mod polite;
pub mod privacy;
pub mod quick_access_rows;
pub mod settings;
pub mod source_ids;

/// Resolved storage locations, available to commands as Tauri state.
pub struct AppPaths(pub raygun_paths::Paths);

/// The open database, available to commands as Tauri state.
pub struct AppDb(pub std::sync::Arc<raygun_db::Db>);

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}
