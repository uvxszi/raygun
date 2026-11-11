//! Servers people own, connected by address and sign-in: Jellyfin and Emby, Subsonic and
//! OpenSubsonic servers (Navidrome...), Audiobookshelf, and OPDS catalogues (Komga, Kavita,
//! Calibre...). Each connection is an extension of runtime `server`
//! (`raygun_ext_host::servers`), so its films, music, books and comics show wherever sources
//! do. Its sign-in (an access token, a password or an API key) is kept in the encrypted secrets
//! store (`AiService`'s vault, device-local) and handed to the extension host at every load;
//! `server.json` in the connection's folder holds only the address and the user.

use std::path::Path;
use std::sync::Arc;

use raygun_ext_host::ServerSecrets;
use raygun_ext_host::servers::{self, ConnectInput, ServerConfig};
use raygun_model::{ExtensionInfo, ExtensionStatus, ServerInfo, ServerInput, ServerKind};
use tauri::{AppHandle, Manager};

use crate::ai::AiService;
use crate::extensions::ExtensionService;
use crate::local_network::LocalNetwork;

/// A server on the home network can't be reached without Android 17's permission.
pub const SERVERS_DENIED: &str = "Allow local network access to reach servers on your network";

pub struct ServerService {
    app: AppHandle,
    /// One addition or removal at a time.
    changing: tokio::sync::Mutex<()>,
}

fn to_host_kind(kind: ServerKind) -> servers::ServerKind {
    match kind {
        ServerKind::Jellyfin => servers::ServerKind::Jellyfin,
        ServerKind::Emby => servers::ServerKind::Emby,
        ServerKind::Subsonic => servers::ServerKind::Subsonic,
        ServerKind::Audiobookshelf => servers::ServerKind::Audiobookshelf,
        ServerKind::Opds => servers::ServerKind::Opds,
    }
}

fn from_host_kind(kind: servers::ServerKind) -> ServerKind {
    match kind {
        servers::ServerKind::Jellyfin => ServerKind::Jellyfin,
        servers::ServerKind::Emby => ServerKind::Emby,
        servers::ServerKind::Subsonic => ServerKind::Subsonic,
        servers::ServerKind::Audiobookshelf => ServerKind::Audiobookshelf,
        servers::ServerKind::Opds => ServerKind::Opds,
    }
}

/// The connections installed in `extensions_dir`: each `server.*` folder's `server.json`.
fn connections(extensions_dir: &Path) -> Vec<(String, ServerConfig)> {
    let Ok(entries) = std::fs::read_dir(extensions_dir) else {
        return Vec::new();
    };
    let mut found: Vec<(String, ServerConfig)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let id = entry.file_name().to_string_lossy().into_owned();
            if !id.starts_with(servers::ID_PREFIX) {
                return None;
            }
            let text = std::fs::read_to_string(entry.path().join(servers::CONFIG_FILE)).ok()?;
            let config: ServerConfig = serde_json::from_str(&text).ok()?;
            Some((id, config))
        })
        .collect();
    found.sort_by_key(|(_, config)| config.name.to_lowercase());
    found
}

/// The sign-ins of the installed connections, for loading them.
pub fn secrets(app: &AppHandle, extensions_dir: &Path) -> ServerSecrets {
    let Some(ai) = app.try_state::<AiService>() else {
        return ServerSecrets::new();
    };
    connections(extensions_dir)
        .into_iter()
        .filter_map(|(id, _)| Some((id.clone(), ai.server_secret(&id)?)))
        .collect()
}

/// What a connection shows: its address and user, and whether it loaded.
fn view(id: &str, config: &ServerConfig, loaded: Option<&ExtensionInfo>) -> ServerInfo {
    let error = match loaded {
        // Loaded without a sign-in that works: it's asked nothing until it's signed in again.
        Some(info) if info.status == ExtensionStatus::Loaded => {
            info.needs_setup.as_ref().map(|need| match need.reason {
                raygun_model::SetupReason::Refused => format!(
                    "The server refused the sign-in: sign in again\n{}",
                    need.detail.as_deref().unwrap_or_default()
                ),
                _ => "Raygun doesn't have this server's sign-in: sign in again".to_owned(),
            })
        }
        Some(info) => Some(
            info.error
                .clone()
                .unwrap_or_else(|| "This server couldn't be loaded".to_owned()),
        ),
        None => Some("This server isn't loaded yet".to_owned()),
    };
    ServerInfo {
        id: id.to_owned(),
        kind: from_host_kind(config.kind),
        name: config.name.clone(),
        address: config.url.clone(),
        username: config.username.clone(),
        provides: config
            .kind
            .provides()
            .iter()
            .map(|kind| (*kind).to_owned())
            .collect(),
        error,
    }
}

impl ServerService {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            changing: tokio::sync::Mutex::new(()),
        }
    }

    fn extensions(&self) -> Arc<ExtensionService> {
        self.app.state::<Arc<ExtensionService>>().inner().clone()
    }

    /// Every connection, each with why it isn't working when it isn't.
    pub async fn list(&self) -> Result<Vec<ServerInfo>, String> {
        let extensions = self.extensions();
        let dir = extensions.extensions_dir().to_owned();
        let found = tauri::async_runtime::spawn_blocking(move || connections(&dir))
            .await
            .map_err(|error| error.to_string())?;
        let infos = extensions.list().await;
        Ok(found
            .iter()
            .map(|(id, config)| view(id, config, infos.iter().find(|info| info.id == *id)))
            .collect())
    }

    /// Signs in to a server and connects it (the same server and user again replaces the
    /// connection, for a new password). The error says what to fix.
    pub async fn add(&self, input: ServerInput) -> Result<Vec<ServerInfo>, String> {
        let _changing = self.changing.lock().await;
        // Android 17: a server on the home network needs local network access; the UI asks for
        // it first, so this only stops a sign-in that would time out.
        if let Some(first) = servers::address_candidates(&input.address)?.first() {
            self.app
                .state::<LocalNetwork>()
                .check(first.as_str(), SERVERS_DENIED)
                .await?;
        }
        let connected = servers::connect(&ConnectInput {
            kind: Some(to_host_kind(input.kind)),
            address: input.address.clone(),
            username: input.username.clone(),
            secret: input.secret.clone(),
            api_key: input.api_key,
        })
        .await?;
        let id = connected.config.extension_id();
        self.app
            .state::<AiService>()
            .set_server_secret(&id, connected.secret.as_deref())
            .map_err(|error| format!("Couldn't keep the sign-in on this device: {error}"))?;
        let verified = servers::package(&connected.config).map_err(|error| error.to_string())?;
        self.extensions().install_verified(verified).await?;
        tracing::info!(server = %id, kind = connected.config.kind.as_str(), "server connected");
        self.list().await
    }

    /// The connections as `server.json` keeps them, by extension id (backups, M46.51).
    pub async fn configs(&self) -> Result<Vec<(String, ServerConfig)>, String> {
        let dir = self.extensions().extensions_dir().to_owned();
        tauri::async_runtime::spawn_blocking(move || connections(&dir))
            .await
            .map_err(|error| error.to_string())
    }

    /// Connects a server a backup holds without signing in again, with its sign-in when the
    /// backup kept it. `false` when it's connected already.
    pub async fn restore(
        &self,
        config: &ServerConfig,
        secret: Option<&str>,
    ) -> Result<bool, String> {
        let _changing = self.changing.lock().await;
        let id = config.extension_id();
        if self.list().await?.iter().any(|server| server.id == id) {
            return Ok(false);
        }
        if let Some(secret) = secret {
            self.app
                .state::<AiService>()
                .set_server_secret(&id, Some(secret))
                .map_err(|error| format!("Couldn't keep the sign-in on this device: {error}"))?;
        }
        let verified = servers::package(config).map_err(|error| error.to_string())?;
        self.extensions().install_verified(verified).await?;
        tracing::info!(server = %id, "server restored");
        Ok(true)
    }

    /// Disconnects a server: its folder, what it kept, and (with the uninstall) its sign-in go.
    pub async fn remove(&self, id: &str) -> Result<Vec<ServerInfo>, String> {
        let _changing = self.changing.lock().await;
        if !id.starts_with(servers::ID_PREFIX) {
            return Err("That isn't a connected server".into());
        }
        self.extensions().uninstall(id).await?;
        tracing::info!(server = %id, "server removed");
        self.list().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connections_are_read_from_their_folders() {
        let dir = tempfile::tempdir().unwrap();
        let config = ServerConfig {
            kind: servers::ServerKind::Jellyfin,
            url: "https://media.example.org".into(),
            name: "Home Media".into(),
            username: "viewer".into(),
            user_id: Some("user1".into()),
            secret: servers::SecretKind::Token,
        };
        let verified = servers::package(&config).unwrap();
        raygun_ext_host::package::install(dir.path(), &verified).unwrap();
        std::fs::create_dir_all(dir.path().join("dev.raygun.other")).unwrap();
        let found = connections(dir.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, config.extension_id());
        let shown = view(&found[0].0, &found[0].1, None);
        assert_eq!(shown.kind, ServerKind::Jellyfin);
        assert_eq!(shown.address, "https://media.example.org");
        assert_eq!(
            shown.provides,
            ["video.source", "music.metadata", "music.stream", "lyrics"]
        );
        assert!(shown.error.is_some(), "not loaded yet");
    }
}
