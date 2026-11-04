//! Network permissions: which hosts an extension may reach (ARCHITECTURE §7.3).

use std::net::IpAddr;

use reqwest::Url;

/// `api.example.org` matches that host only; `*.example.org` matches its subdomains (not
/// `example.org` itself, which needs its own entry). `*` matches any public host (Shosetsu
/// scripts, M32, which name no hosts): never this machine or the local network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostPattern {
    Exact(String),
    Subdomains(String),
    AnyPublic,
}

impl HostPattern {
    pub fn parse(pattern: &str) -> Option<Self> {
        let pattern = pattern.trim().to_ascii_lowercase();
        if pattern == "*" {
            return Some(Self::AnyPublic);
        }
        let (wildcard, host) = match pattern.strip_prefix("*.") {
            Some(rest) => (true, rest.to_owned()),
            None => (false, pattern),
        };
        let valid = !host.is_empty()
            && host.len() <= 253
            && host.split('.').all(|label| {
                !label.is_empty()
                    && !label.starts_with('-')
                    && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            })
            // A wildcard needs a base with a dot: `*.example.org`, never `*.org`.
            && (!wildcard || host.contains('.'));
        valid.then_some(if wildcard {
            Self::Subdomains(host)
        } else {
            Self::Exact(host)
        })
    }

    fn matches(&self, host: &str) -> bool {
        match self {
            Self::AnyPublic => is_public_host(host),
            Self::Exact(exact) => host == exact,
            Self::Subdomains(base) => host
                .strip_suffix(base.as_str())
                .is_some_and(|prefix| prefix.len() > 1 && prefix.ends_with('.')),
        }
    }
}

impl std::fmt::Display for HostPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exact(host) => f.write_str(host),
            Self::Subdomains(base) => write!(f, "*.{base}"),
            Self::AnyPublic => f.write_str("*"),
        }
    }
}

/// A host name or address outside this machine and the local network. Names are checked here;
/// what they resolve to is checked by the resolver of the client that reaches them.
pub fn is_public_host(host: &str) -> bool {
    let host = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = host.parse::<IpAddr>() {
        return is_public_ip(ip);
    }
    !(host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
        || host.ends_with(".lan")
        || host.ends_with(".home.arpa")
        || !host.contains('.'))
}

/// A globally routable address: not loopback, private, link-local, shared, unspecified,
/// documentation or multicast.
pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, ..] = ip.octets();
            !(ip.is_loopback()
                || ip.is_private()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_unspecified()
                || ip.is_multicast()
                || a == 0
                || (a == 100 && (64..128).contains(&b)))
        }
        IpAddr::V6(ip) => {
            if let Some(mapped) = ip.to_ipv4_mapped() {
                return is_public_ip(IpAddr::V4(mapped));
            }
            let first = ip.segments()[0];
            !(ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80)
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetPolicy {
    patterns: Vec<HostPattern>,
}

impl NetPolicy {
    pub fn new(patterns: Vec<HostPattern>) -> Self {
        Self { patterns }
    }

    pub fn patterns(&self) -> &[HostPattern] {
        &self.patterns
    }

    /// Whether a pattern other than `*` names `host` (it may then be a local address).
    pub fn names(&self, host: &str) -> bool {
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        self.patterns
            .iter()
            .any(|pattern| *pattern != HostPattern::AnyPublic && pattern.matches(&host))
    }

    /// Whether a request to `url` is allowed: http(s) only, to a listed host.
    pub fn allows(&self, url: &Url) -> bool {
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some_and(|host| {
                let host = host.trim_end_matches('.').to_ascii_lowercase();
                self.patterns.iter().any(|pattern| pattern.matches(&host))
            })
    }
}
