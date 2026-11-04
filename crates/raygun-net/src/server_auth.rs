//! Sign-ins of the servers people connect (Jellyfin, Subsonic, Audiobookshelf, OPDS catalogues):
//! what Raygun stores and syncs (covers, chapter and page addresses) stays free of credentials,
//! and the sign-in is added when Raygun itself fetches an address under a connected server, as a
//! browser sends a site's cookie. Requests anywhere else never carry it.

use std::collections::BTreeMap;
use std::sync::{PoisonError, RwLock};

use reqwest::Url;

/// How requests to one server sign in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerAuth {
    /// The server's address: requests to this scheme, host and port, under this path, sign in.
    pub base: Url,
    /// Query parameters added unless the address already has them (Subsonic's `u`, `t`, `s`).
    pub query: Vec<(String, String)>,
    /// Headers added unless the request already sets them (`Authorization`).
    pub headers: Vec<(String, String)>,
}

impl ServerAuth {
    fn covers(&self, url: &Url) -> bool {
        let base_path = self.base.path().trim_end_matches('/');
        url.scheme() == self.base.scheme()
            && url.host_str() == self.base.host_str()
            && url.port_or_known_default() == self.base.port_or_known_default()
            && url
                .path()
                .strip_prefix(base_path)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
    }
}

/// The connected servers, by who registered them (an extension id).
static SERVERS: RwLock<BTreeMap<String, ServerAuth>> = RwLock::new(BTreeMap::new());

/// Registers (or with `None` forgets) the sign-in of the server `owner` connects to.
pub fn set(owner: &str, auth: Option<ServerAuth>) {
    let mut servers = SERVERS.write().unwrap_or_else(PoisonError::into_inner);
    match auth {
        Some(auth) => {
            servers.insert(owner.to_owned(), auth);
        }
        None => {
            servers.remove(owner);
        }
    }
}

/// The sign-in for `url`: the most specific server it's under.
fn auth_for(url: &Url) -> Option<ServerAuth> {
    let servers = SERVERS.read().unwrap_or_else(PoisonError::into_inner);
    servers
        .values()
        .filter(|auth| auth.covers(url))
        .max_by_key(|auth| auth.base.path().len())
        .cloned()
}

/// Whether `url` is under a connected server.
pub fn is_signed(url: &str) -> bool {
    Url::parse(url).is_ok_and(|url| auth_for(&url).is_some())
}

/// `url` with its server's query parameters, and the headers to add to `present` (a request's
/// own headers, which win); unchanged for addresses under no connected server.
pub fn authorize(url: &str, present: &BTreeMap<String, String>) -> (String, Vec<(String, String)>) {
    let Ok(mut parsed) = Url::parse(url) else {
        return (url.to_owned(), Vec::new());
    };
    let Some(auth) = auth_for(&parsed) else {
        return (url.to_owned(), Vec::new());
    };
    let missing: Vec<&(String, String)> = auth
        .query
        .iter()
        .filter(|(name, _)| !parsed.query_pairs().any(|(key, _)| key == name.as_str()))
        .collect();
    if !missing.is_empty() {
        let mut pairs = parsed.query_pairs_mut();
        for (name, value) in missing {
            pairs.append_pair(name, value);
        }
    }
    let headers = auth
        .headers
        .into_iter()
        .filter(|(name, _)| !present.keys().any(|key| key.eq_ignore_ascii_case(name)))
        .collect();
    (parsed.to_string(), headers)
}

/// A GET request for `url` with `headers`, signed in when it's under a connected server.
pub fn get(
    client: &reqwest::Client,
    url: &str,
    headers: &BTreeMap<String, String>,
) -> reqwest::RequestBuilder {
    let (url, added) = authorize(url, headers);
    let mut request = client.get(url);
    for (name, value) in headers
        .iter()
        .chain(added.iter().map(|(name, value)| (name, value)))
    {
        request = request.header(name, value);
    }
    request
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auth(base: &str) -> ServerAuth {
        ServerAuth {
            base: base.parse().unwrap(),
            query: vec![("u".into(), "demo".into()), ("t".into(), "abc".into())],
            headers: vec![("Authorization".into(), "Bearer secret".into())],
        }
    }

    #[test]
    fn only_addresses_under_the_server_sign_in() {
        set("test.auth.a", Some(auth("https://media.example.org/music")));
        let none = BTreeMap::new();
        let (url, headers) = authorize(
            "https://media.example.org/music/rest/getCoverArt.view?id=7",
            &none,
        );
        assert_eq!(
            url,
            "https://media.example.org/music/rest/getCoverArt.view?id=7&u=demo&t=abc"
        );
        assert_eq!(headers, [("Authorization".into(), "Bearer secret".into())]);
        for elsewhere in [
            "https://media.example.org/musicals/x",
            "https://media.example.org/other",
            "http://media.example.org/music/x",
            "https://media.example.org:8443/music/x",
            "https://example.org/music/x",
        ] {
            assert_eq!(
                authorize(elsewhere, &none),
                (elsewhere.to_owned(), Vec::new())
            );
        }
        assert!(is_signed("https://media.example.org/music"));
        set("test.auth.a", None);
        assert!(!is_signed("https://media.example.org/music/x"));
    }

    #[test]
    fn a_request_keeps_its_own_parameters_and_headers() {
        set("test.auth.b", Some(auth("https://books.example.org")));
        let own = BTreeMap::from([("authorization".to_owned(), "Basic x".to_owned())]);
        let (url, headers) = authorize("https://books.example.org/a?u=other", &own);
        assert_eq!(url, "https://books.example.org/a?u=other&t=abc");
        assert!(headers.is_empty(), "the request's own Authorization wins");
        let client = crate::http_client().unwrap();
        let request = get(&client, "https://books.example.org/b", &own)
            .build()
            .unwrap();
        assert_eq!(request.headers().get_all("authorization").iter().count(), 1);
        set("test.auth.b", None);
    }

    #[test]
    fn the_most_specific_server_wins() {
        set("test.auth.c", Some(auth("https://nas.example.org")));
        let mut inner = auth("https://nas.example.org/abs");
        inner.headers = vec![("Authorization".into(), "Bearer inner".into())];
        set("test.auth.d", Some(inner));
        let (_, headers) = authorize("https://nas.example.org/abs/api/items", &BTreeMap::new());
        assert_eq!(headers[0].1, "Bearer inner");
        set("test.auth.c", None);
        set("test.auth.d", None);
    }
}
