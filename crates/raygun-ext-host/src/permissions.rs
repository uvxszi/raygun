//! Network permissions: which hosts an extension may reach (ARCHITECTURE §7.3).

use reqwest::Url;

/// `api.example.org` matches that host only; `*.example.org` matches its subdomains (not
/// `example.org` itself, which needs its own entry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostPattern {
    Exact(String),
    Subdomains(String),
}

impl HostPattern {
    pub fn parse(pattern: &str) -> Option<Self> {
        let pattern = pattern.trim().to_ascii_lowercase();
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

    /// Whether a request to `url` is allowed: http(s) only, to a listed host.
    pub fn allows(&self, url: &Url) -> bool {
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some_and(|host| {
                let host = host.trim_end_matches('.').to_ascii_lowercase();
                self.patterns.iter().any(|pattern| pattern.matches(&host))
            })
    }
}
