//! Which hosts are on the local network, as Android 17's local network protection defines it
//! (developer.android.com/privacy-and-security/local-network-definition). Apps targeting SDK 37
//! need `ACCESS_LOCAL_NETWORK` to reach them; without it, connections time out. Knowing the host
//! is local lets the app ask first, or fail with a clear message.

use std::net::IpAddr;
use std::time::Duration;

/// Longest a name lookup may take before the host is taken as not local.
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(3);

/// Whether `ip` is on the local network: private (RFC 1918), link-local, shared (CGNAT),
/// multicast or broadcast, or an IPv6 unique-local, link-local or multicast address. Loopback
/// isn't: connections to this device need no permission.
pub fn is_local_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, ..] = ip.octets();
            ip.is_private()
                || ip.is_link_local()
                || ip.is_multicast()
                || ip.is_broadcast()
                || (a == 100 && (64..128).contains(&b))
        }
        IpAddr::V6(ip) => {
            if let Some(mapped) = ip.to_ipv4_mapped() {
                return is_local_ip(IpAddr::V4(mapped));
            }
            let first = ip.segments()[0];
            ip.is_multicast() || (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
        }
    }
}

/// Where a host is, as far as its name or address tells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostScope {
    /// A local address, or an mDNS `.local` name (looking one up already needs the permission).
    Local,
    /// This device or the internet: a loopback or public address, or `localhost`.
    Elsewhere,
    /// A name only DNS can place: `nas.lan` or a single-label name may well be local.
    Name,
}

/// Where `host` (as in a URL, IPv6 in brackets or not) is, without a lookup.
pub fn host_scope(host: &str) -> HostScope {
    let host = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if let Ok(ip) = host.parse::<IpAddr>() {
        return if is_local_ip(ip) {
            HostScope::Local
        } else {
            HostScope::Elsewhere
        };
    }
    if host == "localhost" || host.ends_with(".localhost") {
        HostScope::Elsewhere
    } else if host == "local" || host.ends_with(".local") {
        HostScope::Local
    } else {
        HostScope::Name
    }
}

/// Whether a request to `url` goes to the local network. Names are looked up in DNS, which Android
/// answers without the permission; a lookup that fails counts as not local, so the request then
/// fails on its own terms.
pub async fn reaches_local_network(url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(url) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    match host_scope(host) {
        HostScope::Local => true,
        HostScope::Elsewhere => false,
        HostScope::Name => {
            let port = url.port_or_known_default().unwrap_or(80);
            let lookup = tokio::net::lookup_host((host.to_owned(), port));
            match tokio::time::timeout(LOOKUP_TIMEOUT, lookup).await {
                Ok(Ok(mut addresses)) => addresses.any(|address| is_local_ip(address.ip())),
                _ => false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    #[test]
    fn private_link_local_shared_and_multicast_addresses_are_local() {
        for address in [
            "10.0.2.2",
            "10.255.255.255",
            "172.16.0.1",
            "172.31.255.254",
            "192.168.1.20",
            "169.254.10.1",
            "100.64.0.1",
            "100.127.255.255",
            "224.0.0.251",
            "255.255.255.255",
            "fd12:3456::1",
            "fc00::1",
            "fe80::1",
            "ff02::fb",
            "::ffff:192.168.0.5",
        ] {
            assert!(is_local_ip(ip(address)), "{address}");
        }
    }

    #[test]
    fn loopback_public_and_neighbouring_addresses_are_not_local() {
        for address in [
            "127.0.0.1",
            "::1",
            "0.0.0.0",
            "::",
            "8.8.8.8",
            "172.15.255.255",
            "172.32.0.0",
            "192.169.0.1",
            "100.63.255.255",
            "100.128.0.0",
            "2606:4700::1111",
            "::ffff:8.8.8.8",
        ] {
            assert!(!is_local_ip(ip(address)), "{address}");
        }
    }

    #[test]
    fn hosts_are_placed_by_address_or_name() {
        assert_eq!(host_scope("192.168.1.20"), HostScope::Local);
        assert_eq!(host_scope("[fe80::1]"), HostScope::Local);
        assert_eq!(host_scope("nas.local"), HostScope::Local);
        assert_eq!(host_scope("NAS.Local."), HostScope::Local);
        assert_eq!(host_scope("127.0.0.1"), HostScope::Elsewhere);
        assert_eq!(host_scope("[::1]"), HostScope::Elsewhere);
        assert_eq!(host_scope("localhost"), HostScope::Elsewhere);
        assert_eq!(host_scope("addon.localhost"), HostScope::Elsewhere);
        assert_eq!(host_scope("1.1.1.1"), HostScope::Elsewhere);
        assert_eq!(host_scope("v3-cinemeta.strem.io"), HostScope::Name);
        assert_eq!(host_scope("nas"), HostScope::Name);
        assert_eq!(host_scope("localnews.example"), HostScope::Name);
    }

    #[tokio::test]
    async fn urls_reach_the_local_network_by_their_host() {
        assert!(reaches_local_network("http://10.0.2.2:7011/manifest.json").await);
        assert!(reaches_local_network("http://[fd00::5]:7000/manifest.json").await);
        assert!(reaches_local_network("https://media.local/manifest.json").await);
        assert!(!reaches_local_network("http://127.0.0.1:7010/manifest.json").await);
        assert!(!reaches_local_network("http://localhost:7010/manifest.json").await);
        assert!(!reaches_local_network("https://1.1.1.1/").await);
        assert!(!reaches_local_network("not a url").await);
        // A name that doesn't resolve isn't treated as local.
        assert!(!reaches_local_network("http://raygun-test.invalid/manifest.json").await);
    }
}
