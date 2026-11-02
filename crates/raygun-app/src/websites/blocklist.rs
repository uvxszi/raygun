//! The built-in browser's list of ad and tracker domains (M46.3): read from the usual list
//! formats and written as sorted 64-bit FNV-1a hashes for `Blocklist.kt`, which uses the same
//! hash and checks a host and its parent domains by binary search.

use std::collections::BTreeSet;

const MAGIC: &[u8; 4] = b"RGBL";
const VERSION: u32 = 1;

/// FNV-1a, 64 bits, as a signed number (Kotlin's `Long`, so both sides sort alike).
pub fn hash(domain: &str) -> i64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in domain.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash as i64
}

fn domain(text: &str) -> Option<String> {
    let domain = text.trim().trim_end_matches('.').to_ascii_lowercase();
    let valid = domain.contains('.')
        && domain.len() <= 253
        && !domain.starts_with(['.', '-'])
        && domain
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        // Hosts files list these for the machine itself.
        && !matches!(domain.as_str(), "localhost.localdomain" | "0.0.0.0" | "broadcasthost")
        && !domain.bytes().all(|byte| byte.is_ascii_digit() || byte == b'.');
    valid.then_some(domain)
}

/// The domains of a list: one per line (HaGeZi, OISD), hosts lines (`0.0.0.0 ads.example`), or
/// adblock rules that block a whole domain (`||ads.example^`). Comments and other rules are
/// skipped.
pub fn parse(text: &str) -> BTreeSet<String> {
    let mut domains = BTreeSet::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(['#', '!', '[']) {
            continue;
        }
        let line = line.split('#').next().unwrap_or_default().trim();
        if let Some(rule) = line.strip_prefix("||") {
            // Only whole-domain rules; anything with options or a path means something else.
            if let Some(name) = rule.strip_suffix('^').and_then(domain) {
                domains.insert(name);
            }
            continue;
        }
        let mut words = line.split_whitespace();
        let first = words.next().unwrap_or_default();
        let name = match words.next() {
            Some(second) if matches!(first, "0.0.0.0" | "127.0.0.1" | "::" | "::1") => second,
            Some(_) => continue,
            None => first,
        };
        if let Some(name) = domain(name) {
            domains.insert(name);
        }
    }
    domains
}

/// The file `Blocklist.kt` reads: `RGBL`, a version, a count, then sorted unique hashes, all
/// little-endian.
pub fn encode(domains: &BTreeSet<String>) -> Vec<u8> {
    let mut hashes: Vec<i64> = domains.iter().map(|domain| hash(domain)).collect();
    hashes.sort_unstable();
    hashes.dedup();
    let mut bytes = Vec::with_capacity(12 + hashes.len() * 8);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&(hashes.len() as u32).to_le_bytes());
    for value in hashes {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

/// The biggest ad, pop-under and tracker networks, as in `Blocklist.kt`: used until a list is
/// downloaded, then merged with it (some lists leave a few of them out).
#[cfg_attr(target_os = "android", allow(dead_code))]
pub const BUILT_IN: &[&str] = &[
    "doubleclick.net",
    "googlesyndication.com",
    "googleadservices.com",
    "adservice.google.com",
    "google-analytics.com",
    "googletagmanager.com",
    "amazon-adsystem.com",
    "adnxs.com",
    "taboola.com",
    "outbrain.com",
    "criteo.com",
    "criteo.net",
    "pubmatic.com",
    "rubiconproject.com",
    "openx.net",
    "scorecardresearch.com",
    "quantserve.com",
    "moatads.com",
    "adform.net",
    "mgid.com",
    "revcontent.com",
    "popads.net",
    "popcash.net",
    "propellerads.com",
    "propellerclick.com",
    "onclickads.net",
    "onclkds.com",
    "exoclick.com",
    "exosrv.com",
    "juicyads.com",
    "trafficjunky.net",
    "adsterra.com",
    "adsterratech.com",
    "hilltopads.net",
    "a-ads.com",
    "clickadu.com",
    "adcash.com",
    "monetag.com",
    "admaven.com",
    "ad-maven.com",
    "galaksion.com",
    "bidvertiser.com",
    "zedo.com",
    "yieldmo.com",
    "highperformanceformat.com",
    "profitablecpmrate.com",
    "pushame.com",
    "adskeeper.com",
];

/// The list a browser checks hosts against: what `Blocklist.kt` does on Android, for the desktop
/// browser (M46.6). A host is blocked when it or one of its parent domains is listed.
#[derive(Debug, Clone, Default)]
#[cfg_attr(target_os = "android", allow(dead_code))]
pub struct Blocklist {
    hashes: Vec<i64>,
}

#[cfg_attr(target_os = "android", allow(dead_code))]
impl Blocklist {
    /// The downloaded list with the built-in networks, or the built-in ones alone when there is
    /// no readable file.
    pub fn load(path: &std::path::Path) -> Self {
        let mut hashes = std::fs::read(path)
            .ok()
            .and_then(|bytes| decode(&bytes))
            .unwrap_or_default();
        hashes.extend(BUILT_IN.iter().map(|domain| hash(domain)));
        hashes.sort_unstable();
        hashes.dedup();
        Self { hashes }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.hashes.len()
    }

    pub fn blocks(&self, host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        let mut domain = host.trim_end_matches('.');
        while domain.contains('.') {
            if self.hashes.binary_search(&hash(domain)).is_ok() {
                return true;
            }
            domain = domain.split_once('.').map_or("", |(_, parent)| parent);
        }
        false
    }
}

/// The hashes in a file [`encode`] wrote, or `None` for anything else.
#[cfg_attr(target_os = "android", allow(dead_code))]
fn decode(bytes: &[u8]) -> Option<Vec<i64>> {
    if bytes.len() < 12 || &bytes[..4] != MAGIC {
        return None;
    }
    let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap_or_default());
    if word(4) != VERSION {
        return None;
    }
    let count = word(8) as usize;
    let body = &bytes[12..];
    if body.len() < count.checked_mul(8)? {
        return None;
    }
    Some(
        body.as_chunks::<8>()
            .0
            .iter()
            .take(count)
            .map(|chunk| i64::from_le_bytes(*chunk))
            .collect(),
    )
}

#[cfg_attr(target_os = "android", allow(dead_code))]
const SECOND_LEVEL: [&str; 10] = [
    "co", "com", "net", "org", "gov", "edu", "ac", "or", "ne", "go",
];

/// The site a host belongs to, as `Blocklist.site`: `www.example.co.uk` is `example.co.uk`.
#[cfg_attr(target_os = "android", allow(dead_code))]
pub fn site(host: &str) -> String {
    let host = host.to_ascii_lowercase();
    let labels: Vec<&str> = host.trim_end_matches('.').split('.').collect();
    if labels.len() <= 2 {
        return labels.join(".");
    }
    let last = labels[labels.len() - 1];
    let second = labels[labels.len() - 2];
    let keep = if last.len() == 2 && SECOND_LEVEL.contains(&second) {
        3
    } else {
        2
    };
    labels[labels.len() - keep..].join(".")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_blocks_hosts_and_their_subdomains_with_the_built_in_networks() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("blocklist.bin");
        let domains: BTreeSet<String> =
            ["ads.example.com".to_owned(), "tracker.net".to_owned()].into();
        std::fs::write(&path, encode(&domains)).unwrap();

        let list = Blocklist::load(&path);
        assert!(list.blocks("ads.example.com"));
        assert!(
            list.blocks("cdn.ads.example.com."),
            "a subdomain, with a trailing dot"
        );
        assert!(list.blocks("TRACKER.net"));
        assert!(
            !list.blocks("example.com"),
            "the parent of a listed domain isn't"
        );
        assert!(list.blocks("securepubads.g.doubleclick.net"), "built in");
        assert_eq!(list.len(), 2 + BUILT_IN.len());

        // A missing or foreign file leaves the built-in networks.
        let missing = Blocklist::load(&temp.path().join("none.bin"));
        assert_eq!(missing.len(), BUILT_IN.len());
        std::fs::write(&path, b"not a list").unwrap();
        assert_eq!(Blocklist::load(&path).len(), BUILT_IN.len());
    }

    #[test]
    fn hosts_belong_to_their_registrable_site() {
        assert_eq!(site("www.example.co.uk"), "example.co.uk");
        assert_eq!(site("a.b.example.com"), "example.com");
        assert_eq!(site("example.org."), "example.org");
        assert_eq!(site("localhost"), "localhost");
    }

    #[test]
    fn lists_in_every_format_give_their_domains() {
        let text = "# Title: a list\n\
                    ads.example.com\n\
                    Tracker.Example.org.\n\
                    0.0.0.0 popups.example.net\n\
                    127.0.0.1 localhost\n\
                    0.0.0.0 0.0.0.0\n\
                    ::1 ip6-localhost\n\
                    ||banner.example.io^\n\
                    ||script.example.io^$third-party\n\
                    ||example.io/ads/*\n\
                    ! adblock comment\n\
                    [Adblock Plus 2.0]\n\
                    some words here\n\
                    192.168.1.1\n\
                    nodot\n";
        let domains: Vec<String> = parse(text).into_iter().collect();
        assert_eq!(
            domains,
            [
                "ads.example.com",
                "banner.example.io",
                "popups.example.net",
                "tracker.example.org"
            ]
        );
    }

    #[test]
    fn hashes_match_the_kotlin_side() {
        // FNV-1a 64 of "example.com", as `Blocklist.fnv` computes it.
        assert_eq!(hash(""), 0xcbf2_9ce4_8422_2325_u64 as i64);
        assert_eq!(hash("a"), 0xaf63_dc4c_8601_ec8c_u64 as i64);
        let bytes = encode(&parse("b.example\na.example\na.example\n"));
        assert_eq!(&bytes[..4], b"RGBL");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 2);
        let first = i64::from_le_bytes(bytes[12..20].try_into().unwrap());
        let second = i64::from_le_bytes(bytes[20..28].try_into().unwrap());
        assert!(
            first < second,
            "sorted as signed numbers, as Kotlin searches them"
        );
    }
}
