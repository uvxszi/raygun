//! Raygun networking (ARCHITECTURE §12): the shared HTTP client, the loopback stream proxy, and
//! which hosts are on the local network (Android 17's local network protection).

pub mod cover_cache;
pub mod images;
pub mod local_network;
pub mod page_cache;
mod proxy;

use std::sync::Arc;

pub use cover_cache::CoverCache;
pub use page_cache::PageCache;
pub use proxy::{
    ImageLinks, ProxyStorage, StaticFiles, StreamProxy, artwork_url,
    image_token as proxy_image_token,
};

/// Sent unless a source supplies its own `User-Agent`; many hosts reject unknown clients.
pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";

/// HTTP client shared by the whole app.
///
/// TLS is rustls on the `ring` provider with Mozilla's bundled roots. The default reqwest stack
/// (aws-lc + platform verifier) complicates Android builds and needs extra JNI setup there; this
/// one behaves the same on every platform.
pub fn http_client() -> reqwest::Result<reqwest::Client> {
    client_builder().build()
}

/// A client builder with Raygun's TLS setup and default user agent, for clients that need their
/// own policies (extensions get one each, with a redirect policy enforcing their permissions).
pub fn client_builder() -> reqwest::ClientBuilder {
    // Also make ring the process-wide default, so any other client (or a library building its own)
    // finds a provider. Fails harmlessly if one is already installed.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    let mut tls = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("ring supports the default TLS versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
    tls.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    reqwest::Client::builder()
        .use_preconfigured_tls(tls)
        .user_agent(DEFAULT_USER_AGENT)
}
