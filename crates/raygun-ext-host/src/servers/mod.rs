//! Servers people own, connected by address and sign-in rather than installed from a repository:
//! Jellyfin and Emby (`jellyfin`), Subsonic and OpenSubsonic servers such as Navidrome
//! (`subsonic`), Audiobookshelf (`audiobookshelf`), and OPDS catalogues such as Komga, Kavita
//! and Calibre (`opds`).
//!
//! A connection is an extension folder of runtime `server`: `raygun-extension.json` and
//! `server.json` ([`ServerConfig`]: the kind, the address and the user, never the secret). The
//! app keeps the secret (an access token, a password or an API key) in its encrypted secrets
//! store and hands it over when the folder loads. [`ServerExtension`] answers the provider calls
//! of the server's kinds in Rust, and registers its sign-in with `raygun_net::server_auth`, so
//! covers, pages and streams fetched later sign in too while stored addresses carry no secret.

pub mod audiobookshelf;
mod epub;
pub(crate) mod http;
pub mod jellyfin;
pub mod opds;
pub mod subsonic;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use raygun_model::ProviderInfo;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use self::http::Http;
use crate::ExtensionSource;
use crate::audiobook::AUDIOBOOK_DIRECTORY;
use crate::failure::Failure;
use crate::manga::MANGA_SOURCE;
use crate::manifest::{API_VERSION, Manifest};
use crate::novel::NOVEL_SOURCE;
use crate::package::{PackageError, VerifiedPackage, from_files};
use crate::video::VIDEO_SOURCE;
use crate::{LYRICS, MUSIC_METADATA, MUSIC_STREAM};

/// The connection's own file in its extension folder.
pub const CONFIG_FILE: &str = "server.json";
/// Extension ids of connections start with this.
pub const ID_PREFIX: &str = "server.";
/// How long a resolved stream's address is used before it's asked for again.
const STREAM_LIFETIME: Duration = Duration::from_secs(6 * 60 * 60);
/// How often a password sign-in that expires is renewed (Audiobookshelf's last an hour).
const RENEW_EVERY: Duration = Duration::from_secs(40 * 60);
/// How soon a renewal that failed is tried again, the first time; then twice as long each time.
const RETRY_FIRST: Duration = Duration::from_secs(30);

/// What kind of server a connection is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ServerKind {
    Jellyfin,
    Emby,
    Subsonic,
    Audiobookshelf,
    Opds,
}

impl ServerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Jellyfin => "jellyfin",
            Self::Emby => "emby",
            Self::Subsonic => "subsonic",
            Self::Audiobookshelf => "audiobookshelf",
            Self::Opds => "opds",
        }
    }

    /// The kind's name for people.
    pub fn label(self) -> &'static str {
        match self {
            Self::Jellyfin => "Jellyfin",
            Self::Emby => "Emby",
            Self::Subsonic => "Subsonic",
            Self::Audiobookshelf => "Audiobookshelf",
            Self::Opds => "OPDS",
        }
    }

    /// The provider kinds a connection of this kind provides.
    pub fn provides(self) -> &'static [&'static str] {
        match self {
            Self::Jellyfin | Self::Emby => &[VIDEO_SOURCE, MUSIC_METADATA, MUSIC_STREAM, LYRICS],
            Self::Subsonic => &[MUSIC_METADATA, MUSIC_STREAM, LYRICS],
            Self::Audiobookshelf => &[AUDIOBOOK_DIRECTORY],
            Self::Opds => &[MANGA_SOURCE, NOVEL_SOURCE],
        }
    }

    /// Its providers: local id and kind.
    fn providers(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Jellyfin | Self::Emby => &[
                ("videos", VIDEO_SOURCE),
                ("music", MUSIC_METADATA),
                ("streams", MUSIC_STREAM),
                ("lyrics", LYRICS),
            ],
            Self::Subsonic => &[
                ("music", MUSIC_METADATA),
                ("streams", MUSIC_STREAM),
                ("lyrics", LYRICS),
            ],
            Self::Audiobookshelf => &[("books", AUDIOBOOK_DIRECTORY)],
            Self::Opds => &[("comics", MANGA_SOURCE), ("books", NOVEL_SOURCE)],
        }
    }
}

/// What the kept secret is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SecretKind {
    /// No sign-in (an open OPDS catalogue).
    #[default]
    None,
    /// The password itself (Subsonic's tokens, Audiobookshelf's sign-ins, HTTP Basic).
    Password,
    /// The password, sent as it is: Subsonic servers that can't check tokens (LDAP users).
    PlainPassword,
    /// An API key.
    ApiKey,
    /// An access token the server gave at sign-in (Jellyfin, Emby).
    Token,
}

/// A connection, as `server.json` keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    pub kind: ServerKind,
    /// The server's address, without a trailing slash; an OPDS catalogue's root feed.
    pub url: String,
    /// What the server calls itself, else its host.
    pub name: String,
    /// Who signed in; empty for API keys and open catalogues.
    #[serde(default)]
    pub username: String,
    /// The server's id for that user (Jellyfin, Emby).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default)]
    pub secret: SecretKind,
}

impl ServerConfig {
    /// The connection's extension id: `server.<kind>.<12 hex of the address and user>`, the same
    /// whenever the same server is added for the same user, so adding it again replaces it.
    pub fn extension_id(&self) -> String {
        let digest = Sha256::digest(
            format!("{}|{}|{}", self.kind.as_str(), self.url, self.username).as_bytes(),
        );
        let hex: String = digest
            .iter()
            .take(6)
            .map(|byte| format!("{byte:02x}"))
            .collect();
        format!("{ID_PREFIX}{}.{hex}", self.kind.as_str())
    }

    /// The id servers list this device by (Jellyfin keeps one sign-in per device).
    fn device_id(&self) -> String {
        format!(
            "raygun-{}",
            self.extension_id().rsplit('.').next().unwrap_or_default()
        )
    }

    fn base(&self) -> Result<Url, String> {
        Url::parse(&self.url).map_err(|error| format!("the server's address is invalid: {error}"))
    }
}

/// The extension folder for a connection: its manifest and `server.json`.
pub fn package(config: &ServerConfig) -> Result<VerifiedPackage, PackageError> {
    let base = config.base().map_err(PackageError::Manifest)?;
    let host = base
        .host_str()
        .ok_or_else(|| PackageError::Manifest("the address names no host".into()))?
        .to_ascii_lowercase();
    let website = match config.kind {
        ServerKind::Jellyfin => http::under(&base, "web/").to_string(),
        ServerKind::Emby => http::under(&base, "web/index.html").to_string(),
        ServerKind::Opds => format!("{}/", base.origin().ascii_serialization()),
        _ => format!("{}/", config.url.trim_end_matches('/')),
    };
    let name: String = config.name.trim().chars().take(80).collect();
    let manifest = json!({
        "id": config.extension_id(),
        "name": if name.is_empty() { host.clone() } else { name },
        "version": "1.0.0",
        "apiVersion": API_VERSION,
        "runtime": "server",
        "entry": CONFIG_FILE,
        "provides": config.kind.provides(),
        "permissions": { "net": [host.clone()] },
        "contentRating": "safe",
        "description": format!("{} server at {host}", config.kind.label()),
        "website": website,
    });
    let config_json = serde_json::to_vec_pretty(config)
        .map_err(|error| PackageError::Manifest(error.to_string()))?;
    from_files(
        &manifest.to_string(),
        vec![(CONFIG_FILE.to_owned(), config_json)],
    )
}

/// What adding a server asks for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConnectInput {
    pub kind: Option<ServerKind>,
    pub address: String,
    pub username: String,
    /// The password, or the API key when `api_key` is set; empty for none.
    pub secret: String,
    pub api_key: bool,
}

/// A server signed in to: the connection and the secret to keep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connected {
    pub config: ServerConfig,
    pub secret: Option<String>,
}

/// The addresses to try for what was typed: as it is, and without a scheme `https://` first,
/// then `http://` (servers on a home network rarely have certificates). The path stays as typed:
/// an OPDS catalogue's address may need its trailing slash.
pub fn address_candidates(typed: &str) -> Result<Vec<Url>, String> {
    let typed = typed.trim();
    if typed.is_empty() {
        return Err("Type the server's address".into());
    }
    let with_scheme = |scheme: &str| Url::parse(&format!("{scheme}://{typed}"));
    let candidates: Vec<Url> = if typed.contains("://") {
        vec![Url::parse(typed).map_err(|_| format!("{typed} isn't an address"))?]
    } else {
        ["https", "http"]
            .iter()
            .filter_map(|scheme| with_scheme(scheme).ok())
            .collect()
    };
    let mut usable = Vec::new();
    for mut url in candidates {
        if !matches!(url.scheme(), "http" | "https") {
            return Err("Use an http or https address".into());
        }
        match url.host() {
            None => return Err(format!("{typed} isn't an address")),
            Some(url::Host::Ipv6(_)) => {
                return Err("Use the server's name or IPv4 address".into());
            }
            Some(_) => {}
        }
        url.set_fragment(None);
        usable.push(url);
    }
    Ok(usable)
}

/// Signs in to a server (trying the address's forms in turn) and returns the connection, or why
/// it couldn't, in a sentence for people.
pub async fn connect(input: &ConnectInput) -> Result<Connected, String> {
    let kind = input.kind.ok_or("Choose what kind of server it is")?;
    let mut last = None;
    for url in address_candidates(&input.address)? {
        let http = client_for(kind, &url)?;
        match connect_at(kind, &http, &url, input).await {
            Ok(connected) => return Ok(connected),
            // A refused sign-in is the answer; another form of the address won't change it.
            Err(Reach::Answered(message)) => return Err(message),
            Err(Reach::Unreachable(message)) => last = Some(message),
        }
    }
    Err(last.unwrap_or_else(|| "Couldn't reach the server".into()))
}

/// Why a sign-in failed: the server answered (and said no), or it couldn't be reached.
enum Reach {
    Answered(String),
    Unreachable(String),
}

fn reach(host: &str, failure: Failure, refused: &str) -> Reach {
    use raygun_net::IssueKind;
    let kind = failure.trouble.as_ref().map(|trouble| trouble.kind);
    match kind {
        Some(IssueKind::Denied) => Reach::Answered(refused.to_owned()),
        Some(
            IssueKind::Dns
            | IssueKind::Timeout
            | IssueKind::Tls
            | IssueKind::Refused
            | IssueKind::Unreachable
            | IssueKind::Offline,
        ) => Reach::Unreachable(format!("Couldn't reach {host}\n{}", failure.message)),
        Some(IssueKind::NotFound | IssueKind::SiteChanged) => Reach::Unreachable(format!(
            "{host} didn't answer as this kind of server does\n{}",
            failure.message
        )),
        _ => Reach::Answered(raygun_net::issue::message(
            &format!("{host} refused the sign-in"),
            &failure.message,
        )),
    }
}

async fn connect_at(
    kind: ServerKind,
    http: &Http,
    url: &Url,
    input: &ConnectInput,
) -> Result<Connected, Reach> {
    let host = url.host_str().unwrap_or_default().to_owned();
    let username = input.username.trim().to_owned();
    let secret = input.secret.clone();
    let wrong = if input.api_key {
        "The server refused this API key"
    } else {
        "The user name or password is wrong"
    };
    match kind {
        ServerKind::Jellyfin | ServerKind::Emby => {
            let mut base = url.clone();
            let mut info = None;
            for candidate in [url.clone(), http::under(url, "emby")] {
                let answer = http
                    .json::<jellyfin::PublicInfo>(
                        http.client()
                            .get(http::under(&candidate, "System/Info/Public")),
                    )
                    .await;
                if let Ok(found) = answer {
                    base = candidate;
                    info = Some(found);
                    break;
                }
            }
            let info = info.ok_or_else(|| {
                Reach::Unreachable(format!("Couldn't find a Jellyfin or Emby server at {host}"))
            })?;
            let emby = info.is_emby();
            let mut config = ServerConfig {
                kind: if emby {
                    ServerKind::Emby
                } else {
                    ServerKind::Jellyfin
                },
                url: base.to_string().trim_end_matches('/').to_owned(),
                name: if info.server_name.trim().is_empty() {
                    host.clone()
                } else {
                    info.server_name.clone()
                },
                username: username.clone(),
                user_id: None,
                secret: SecretKind::Token,
            };
            let device = config.device_id();
            let (user_id, token) = if input.api_key {
                let user = jellyfin::token_user(http, &base, emby, &device, &secret)
                    .await
                    .map_err(|failure| reach(&host, failure, wrong))?;
                config.secret = SecretKind::ApiKey;
                (user, secret)
            } else {
                if username.is_empty() {
                    return Err(Reach::Answered("Type your user name on the server".into()));
                }
                jellyfin::sign_in(http, &base, emby, &device, &username, &secret)
                    .await
                    .map_err(|failure| reach(&host, failure, wrong))?
            };
            config.user_id = Some(user_id);
            Ok(Connected {
                config,
                secret: Some(token),
            })
        }
        ServerKind::Subsonic => {
            let sign = if input.api_key {
                subsonic::Sign::Key(secret.clone())
            } else {
                if username.is_empty() {
                    return Err(Reach::Answered("Type your user name on the server".into()));
                }
                subsonic::Sign::Token {
                    username: username.clone(),
                    password: secret.clone(),
                }
            };
            let mut secret_kind = if input.api_key {
                SecretKind::ApiKey
            } else {
                SecretKind::Password
            };
            if let Err(failure) = subsonic::ping(http, url, &sign).await {
                // Error 41: this user's password can't be checked by token (LDAP); send it as is.
                if subsonic::error_code(&failure) == Some(41) {
                    let plain = subsonic::Sign::Plain {
                        username: username.clone(),
                        password: secret.clone(),
                    };
                    subsonic::ping(http, url, &plain)
                        .await
                        .map_err(|failure| reach(&host, failure, wrong))?;
                    secret_kind = SecretKind::PlainPassword;
                } else if failure.answered && failure.trouble.is_none() {
                    return Err(Reach::Answered(failure.message));
                } else {
                    return Err(reach(&host, failure, wrong));
                }
            }
            Ok(Connected {
                config: ServerConfig {
                    kind,
                    url: url.to_string().trim_end_matches('/').to_owned(),
                    name: host.clone(),
                    username: if input.api_key {
                        String::new()
                    } else {
                        username
                    },
                    user_id: None,
                    secret: secret_kind,
                },
                secret: Some(secret),
            })
        }
        ServerKind::Audiobookshelf => {
            let sign = if input.api_key {
                audiobookshelf::Sign::Key(secret.clone())
            } else {
                if username.is_empty() {
                    return Err(Reach::Answered("Type your user name on the server".into()));
                }
                audiobookshelf::Sign::Password {
                    username: username.clone(),
                    password: secret.clone(),
                }
            };
            let me = audiobookshelf::check(http, url, &sign)
                .await
                .map_err(|failure| reach(&host, failure, wrong))?;
            Ok(Connected {
                config: ServerConfig {
                    kind,
                    url: url.to_string().trim_end_matches('/').to_owned(),
                    name: host.clone(),
                    username: if input.api_key { me } else { username },
                    user_id: None,
                    secret: if input.api_key {
                        SecretKind::ApiKey
                    } else {
                        SecretKind::Password
                    },
                },
                secret: Some(secret),
            })
        }
        ServerKind::Opds => {
            let basic = (!username.is_empty()).then(|| opds::basic_auth(&username, &secret));
            // A catalogue's address, or a server's: Komga's and Calibre's catalogues sit at
            // well-known paths under it.
            let mut tried = Vec::new();
            for candidate in [
                url.clone(),
                http::under(url, "opds/v1.2/catalog"),
                http::under(url, "opds"),
            ] {
                if tried.contains(&candidate) {
                    continue;
                }
                tried.push(candidate.clone());
                match opds::fetch_feed(http, &candidate, basic.as_deref()).await {
                    Ok((feed, answer)) => {
                        let root = answer.url;
                        return Ok(Connected {
                            config: ServerConfig {
                                kind,
                                url: root.to_string(),
                                name: if feed.title.trim().is_empty() {
                                    host.clone()
                                } else {
                                    feed.title.trim().to_owned()
                                },
                                username: username.clone(),
                                user_id: None,
                                secret: if basic.is_some() {
                                    SecretKind::Password
                                } else {
                                    SecretKind::None
                                },
                            },
                            secret: basic.is_some().then(|| secret.clone()),
                        });
                    }
                    // 401 asks for a sign-in; a 403 may only mean there's nothing at this path.
                    Err(failure) if http::status_of(&failure) == Some(401) => {
                        return Err(Reach::Answered(if basic.is_some() {
                            wrong.to_owned()
                        } else {
                            "This catalogue needs a user name and password".to_owned()
                        }));
                    }
                    Err(failure) if http::is_denied(&failure) => {}
                    Err(failure) => {
                        // Not reaching the host at all ends the search for its catalogue.
                        if let Reach::Unreachable(message) = reach(&host, failure, wrong)
                            && message.starts_with("Couldn't reach")
                        {
                            return Err(Reach::Unreachable(message));
                        }
                    }
                }
            }
            Err(Reach::Answered(format!(
                "Couldn't find an OPDS catalogue at {host}"
            )))
        }
    }
}

/// The client for a kind of server: catalogues may lead to their mirrors on other public sites;
/// servers stay on their own host.
fn client_for(kind: ServerKind, base: &Url) -> Result<Http, String> {
    match kind {
        ServerKind::Opds => Http::open(base),
        _ => Http::new(base),
    }
}

/// When a stream address handed out now should be asked for again (Unix ms).
fn stream_expiry() -> i64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from((now + STREAM_LIFETIME).as_millis()).unwrap_or(i64::MAX)
}

enum Backend {
    Jellyfin(jellyfin::Jellyfin),
    Subsonic(subsonic::Subsonic),
    Audiobookshelf(Rc<audiobookshelf::Audiobookshelf>),
    Opds(opds::Opds),
}

/// Distinguishes one load of a connection from the next in `raygun_net::server_auth`, so an old
/// one going away never takes its successor's sign-in with it.
static LOADS: AtomicU64 = AtomicU64::new(0);

/// A loaded connection.
pub struct ServerExtension {
    manifest: Manifest,
    config: ServerConfig,
    backend: Backend,
    settings: BTreeMap<String, Value>,
    /// Its name in `raygun_net::server_auth`.
    owner: String,
    /// Keeps an expiring sign-in fresh.
    renewer: Option<tokio::task::AbortHandle>,
}

impl ServerExtension {
    /// Reads `server.json` and prepares the client; `secret` is what the app kept, `data_dir` the
    /// connection's own folder (books fetched from a catalogue are kept there). Requests only
    /// start with the first call (or the background renewal of a sign-in that expires).
    pub async fn load(
        manifest: Manifest,
        source: &ExtensionSource,
        secret: Option<String>,
        settings: BTreeMap<String, Value>,
        data_dir: Option<PathBuf>,
    ) -> Result<Self, String> {
        let config: ServerConfig = serde_json::from_str(&source.read(&manifest.entry)?)
            .map_err(|error| format!("{CONFIG_FILE} is invalid: {error}"))?;
        let base = config.base()?;
        if base.host_str().map(str::to_ascii_lowercase)
            != manifest.net.patterns().first().map(ToString::to_string)
        {
            return Err("the server's address differs from the host it may reach".into());
        }
        let http = client_for(config.kind, &base)?;
        let owner = format!("{}#{}", manifest.id, LOADS.fetch_add(1, Ordering::Relaxed));
        let needs_secret = config.secret != SecretKind::None;
        let secret = match (secret, needs_secret) {
            (Some(secret), true) => secret,
            (_, false) => String::new(),
            (None, true) => {
                return Err("Raygun no longer has this server's sign-in: sign in again".into());
            }
        };
        let mut renewer = None;
        let backend = match config.kind {
            ServerKind::Jellyfin | ServerKind::Emby => {
                let user = config
                    .user_id
                    .clone()
                    .ok_or("the connection names no user: sign in again")?;
                let client = jellyfin::Jellyfin::new(
                    http,
                    base.clone(),
                    user,
                    secret,
                    config.device_id(),
                    config.kind == ServerKind::Emby,
                );
                register(&owner, &base, Vec::new(), client.headers());
                Backend::Jellyfin(client)
            }
            ServerKind::Subsonic => {
                let sign = match config.secret {
                    SecretKind::ApiKey => subsonic::Sign::Key(secret),
                    SecretKind::PlainPassword => subsonic::Sign::Plain {
                        username: config.username.clone(),
                        password: secret,
                    },
                    _ => subsonic::Sign::Token {
                        username: config.username.clone(),
                        password: secret,
                    },
                };
                let client = subsonic::Subsonic::new(http, base.clone(), &sign);
                register(&owner, &base, client.auth_query(), Vec::new());
                Backend::Subsonic(client)
            }
            ServerKind::Audiobookshelf => {
                let sign = match config.secret {
                    SecretKind::ApiKey => audiobookshelf::Sign::Key(secret),
                    _ => audiobookshelf::Sign::Password {
                        username: config.username.clone(),
                        password: secret,
                    },
                };
                let (registered, at) = (owner.clone(), base.clone());
                let on_token: Rc<dyn Fn(&str)> = Rc::new(move |token: &str| {
                    register(
                        &registered,
                        &at,
                        Vec::new(),
                        vec![("Authorization".to_owned(), format!("Bearer {token}"))],
                    );
                });
                let client = Rc::new(audiobookshelf::Audiobookshelf::new(
                    http,
                    base.clone(),
                    sign,
                    on_token,
                ));
                if client.expires() {
                    // Signs in now, so chapters stream before any call, and keeps it fresh.
                    let renewing = client.clone();
                    let task = tokio::task::spawn_local(async move {
                        let mut retry = RETRY_FIRST;
                        loop {
                            let wait = match renewing.renew().await {
                                Ok(_) => {
                                    retry = RETRY_FIRST;
                                    RENEW_EVERY
                                }
                                // Offline or the server down: asked again soon, less often each
                                // time.
                                Err(failure) => {
                                    tracing::debug!(error = %failure.message, "an Audiobookshelf sign-in couldn't be renewed");
                                    let wait = retry;
                                    retry = (retry * 2).min(RENEW_EVERY);
                                    wait
                                }
                            };
                            tokio::time::sleep(wait).await;
                        }
                    });
                    renewer = Some(task.abort_handle());
                }
                Backend::Audiobookshelf(client)
            }
            ServerKind::Opds => {
                let basic = (config.secret == SecretKind::Password)
                    .then(|| opds::basic_auth(&config.username, &secret));
                if let Some(basic) = &basic {
                    // Covers and pages may sit anywhere on the catalogue's host.
                    let mut origin = base.clone();
                    origin.set_path("/");
                    origin.set_query(None);
                    register(
                        &owner,
                        &origin,
                        Vec::new(),
                        vec![("Authorization".to_owned(), basic.clone())],
                    );
                }
                Backend::Opds(opds::Opds::new(http, base.clone(), basic, data_dir))
            }
        };
        Ok(Self {
            manifest,
            config,
            backend,
            settings,
            owner,
            renewer,
        })
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    pub fn user_settings(&self) -> &BTreeMap<String, Value> {
        &self.settings
    }

    pub fn providers(&self) -> Vec<ProviderInfo> {
        self.config
            .kind
            .providers()
            .iter()
            .map(|(local, kind)| ProviderInfo {
                id: format!("{}/{local}", self.manifest.id),
                extension_id: self.manifest.id.clone(),
                kind: (*kind).to_owned(),
                name: self.manifest.name.clone(),
                search_pill: None,
                adult: false,
            })
            .collect()
    }

    pub async fn call(&self, local_id: &str, method: &str, args: &Value) -> Result<Value, Failure> {
        match (&self.backend, local_id) {
            (Backend::Jellyfin(server), "videos") => server.video_call(method, args).await,
            (Backend::Jellyfin(server), "music") => server.music_call(method, args).await,
            (Backend::Jellyfin(server), "streams") => server.stream_call(method, args).await,
            (Backend::Jellyfin(server), "lyrics") => server.lyrics_call(method, args).await,
            (Backend::Subsonic(server), "music") => server.music_call(method, args).await,
            (Backend::Subsonic(server), "streams") => server.stream_call(method, args).await,
            (Backend::Subsonic(server), "lyrics") => server.lyrics_call(method, args).await,
            (Backend::Audiobookshelf(server), "books") => server.books_call(method, args).await,
            (Backend::Opds(server), "comics") => server.comics_call(method, args).await,
            (Backend::Opds(server), "books") => server.books_call(method, args).await,
            _ => Err(Failure::said(format!("{local_id} has no method {method}"))),
        }
    }
}

impl Drop for ServerExtension {
    fn drop(&mut self) {
        if let Some(renewer) = self.renewer.take() {
            renewer.abort();
        }
        raygun_net::server_auth::set(&self.owner, None);
    }
}

/// Registers how requests under `base` sign in.
fn register(owner: &str, base: &Url, query: Vec<(String, String)>, headers: Vec<(String, String)>) {
    raygun_net::server_auth::set(
        owner,
        Some(raygun_net::server_auth::ServerAuth {
            base: base.clone(),
            query,
            headers,
        }),
    );
}
