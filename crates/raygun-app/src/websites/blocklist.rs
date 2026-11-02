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

#[cfg(test)]
mod tests {
    use super::*;

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
