//! Raygun networking (ARCHITECTURE §12): the shared HTTP client, the loopback stream proxy, and
//! which hosts are on the local network (Android 17's local network protection).

pub mod bitmap;
pub mod cover_cache;
pub mod images;
pub mod issue;
pub mod local_network;
pub mod manifest;
pub mod page_cache;
mod proxy;
pub mod server_auth;
pub mod subtitles;

use std::sync::{Arc, RwLock};

pub use cover_cache::CoverCache;
pub use issue::{Issue, IssueKind};
pub use page_cache::PageCache;
pub use proxy::lan::{LanLinks, LanRoute, SubtitleFormat};
pub use proxy::{
    FontLinks, ImageLinks, MediaLinks, ProxyStorage, StaticFiles, StreamProxy, artwork_url,
    image_token as proxy_image_token,
};

/// Sent unless a source supplies its own `User-Agent`; many hosts reject unknown clients.
pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";

/// The user's choice of User-Agent (M46.11, Mihon's "Default user agent"); empty for the default.
static USER_AGENT: RwLock<String> = RwLock::new(String::new());

/// The User-Agent requests made for sources carry when they don't name one: the user's choice,
/// or [`DEFAULT_USER_AGENT`].
pub fn user_agent() -> String {
    USER_AGENT
        .read()
        .ok()
        .map(|agent| agent.clone())
        .filter(|agent| !agent.is_empty())
        .unwrap_or_else(|| DEFAULT_USER_AGENT.to_owned())
}

/// Sets the User-Agent sources send from now on; empty restores the default.
pub fn set_user_agent(agent: &str) {
    if let Ok(mut current) = USER_AGENT.write() {
        agent.trim().clone_into(&mut current);
    }
}

/// Adds [`user_agent`] to a request unless `names` (its headers' names) has its own.
pub fn with_user_agent<'a>(
    request: reqwest::RequestBuilder,
    mut names: impl Iterator<Item = &'a str>,
) -> reqwest::RequestBuilder {
    if names.any(|name| name.eq_ignore_ascii_case("user-agent")) {
        request
    } else {
        request.header(reqwest::header::USER_AGENT, user_agent())
    }
}

/// Whether a stream must be asked for with a `Range` (M46.51): YouTube's media servers send a
/// request without one at playback speed and cut it off before the end, while one with a range,
/// `bytes=0-` included, comes at full speed.
pub fn wants_range(url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(url) else {
        return false;
    };
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    (host == "googlevideo.com" || host.ends_with(".googlevideo.com"))
        && url.path().starts_with("/videoplayback")
}

/// HTTP client shared by the whole app.
///
/// TLS is rustls on the `ring` provider. Desktop verifies certificates with the system, which
/// fetches intermediates a site forgot to send, as browsers do (M46.9.1: novel sites failed with
/// UnknownIssuer); Android uses Mozilla's bundled roots, since the platform verifier needs JNI
/// setup there.
pub fn http_client() -> reqwest::Result<reqwest::Client> {
    client_builder().build()
}

/// A client builder with Raygun's TLS setup and default user agent, for clients that need their
/// own policies (extensions get one each, with a redirect policy enforcing their permissions).
#[cfg(not(target_os = "android"))]
fn tls_config(provider: Arc<rustls::crypto::CryptoProvider>) -> rustls::ClientConfig {
    use rustls_platform_verifier::BuilderVerifierExt;
    rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .expect("ring supports the default TLS versions")
        .with_platform_verifier()
        .map(|builder| builder.with_no_client_auth())
        .unwrap_or_else(|_| bundled_roots(provider))
}

#[cfg(target_os = "android")]
fn tls_config(provider: Arc<rustls::crypto::CryptoProvider>) -> rustls::ClientConfig {
    bundled_roots(provider)
}

/// Mozilla's roots, built in.
fn bundled_roots(provider: Arc<rustls::crypto::CryptoProvider>) -> rustls::ClientConfig {
    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("ring supports the default TLS versions")
        .with_root_certificates(roots)
        .with_no_client_auth()
}

pub fn client_builder() -> reqwest::ClientBuilder {
    // Also make ring the process-wide default, so any other client (or a library building its own)
    // finds a provider. Fails harmlessly if one is already installed.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut tls = tls_config(provider);
    tls.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    reqwest::Client::builder()
        .use_preconfigured_tls(tls)
        .user_agent(DEFAULT_USER_AGENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_send_the_chosen_user_agent_unless_they_name_one() {
        let client = reqwest::Client::new();
        let sent = |request: reqwest::RequestBuilder| {
            let request = request.build().unwrap();
            let agents: Vec<String> = request
                .headers()
                .get_all(reqwest::header::USER_AGENT)
                .iter()
                .map(|value| value.to_str().unwrap().to_owned())
                .collect();
            agents
        };
        set_user_agent("  Custom/1.0 ");
        let plain = with_user_agent(client.get("https://example.com/"), std::iter::empty());
        assert_eq!(sent(plain), ["Custom/1.0"]);
        let own = client
            .get("https://example.com/")
            .header("user-agent", "Own/2");
        assert_eq!(
            sent(with_user_agent(own, ["User-Agent"].into_iter())),
            ["Own/2"]
        );
        set_user_agent("");
        assert_eq!(user_agent(), DEFAULT_USER_AGENT);
    }

    #[test]
    fn youtube_streams_are_asked_for_by_range() {
        assert!(wants_range(
            "https://rr3---sn-a5mekn6d.googlevideo.com/videoplayback?expire=1&itag=251"
        ));
        assert!(!wants_range("https://audius.co/v1/tracks/1/stream"));
        assert!(!wants_range("https://notgooglevideo.com/videoplayback"));
        assert!(!wants_range(
            "https://manifest.googlevideo.com/api/manifest/hls_variant/x"
        ));
        assert!(!wants_range("not an address"));
    }
}
