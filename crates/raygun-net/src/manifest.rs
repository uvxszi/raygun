//! Streaming playlists through the proxy's `/media/` route (M46.4): HLS playlists and DASH
//! manifests name their segments by address, and those addresses must lead back into the proxy so
//! the segments get the stream's headers and CORS.
//!
//! The route's paths mirror the upstream address (`/media/<token>/https/host/a/b.m3u8`), so a
//! relative address already resolves into the proxy. HLS addresses are all resolved and rewritten
//! anyway (cheap, and right after redirects); in DASH only absolute and root-relative ones are,
//! because relative ones are templates resolved against nested `BaseURL`s.

use reqwest::Url;

/// What a response turned out to be, from its first bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Manifest {
    Hls,
    Dash,
}

/// How many bytes are looked at to tell a playlist from media.
pub const SNIFF_BYTES: usize = 1024;

/// An HLS playlist starts with `#EXTM3U`; a DASH manifest has an `<MPD` element near the start.
/// Hosts often serve both as `text/plain` or `application/octet-stream`, or under a disguised
/// extension, so the type and the name aren't trusted.
pub fn sniff(head: &[u8]) -> Option<Manifest> {
    let head = &head[..head.len().min(SNIFF_BYTES)];
    let head = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head);
    let start = head.iter().position(|byte| !byte.is_ascii_whitespace())?;
    let head = &head[start..];
    if head.starts_with(b"#EXTM3U") {
        return Some(Manifest::Hls);
    }
    let looks_like_xml = head.starts_with(b"<");
    let has_mpd = head.windows(4).any(|window| window == b"<MPD");
    (looks_like_xml && has_mpd).then_some(Manifest::Dash)
}

/// Rewrites every address in an HLS playlist: the lines naming segments or playlists and the
/// `URI="…"` attributes of tags (keys, maps, renditions, parts…). Each is resolved against `base`
/// (the playlist's own address) and handed to `link`. Addresses that aren't web addresses
/// (`data:`, `skd:` keys) stay as they are.
pub fn rewrite_hls(text: &str, base: &Url, link: &dyn Fn(&Url) -> String) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / 2);
    for line in text.split_inclusive('\n') {
        let content = line.trim_end_matches(['\r', '\n']);
        let ending = &line[content.len()..];
        let trimmed = content.trim();
        if trimmed.is_empty() {
            out.push_str(line);
        } else if trimmed.starts_with("#EXT") {
            out.push_str(&rewrite_uri_attributes(content, base, link));
            out.push_str(ending);
        } else if trimmed.starts_with('#') {
            out.push_str(line);
        } else {
            out.push_str(&resolve_and_link(trimmed, base, link).unwrap_or_else(|| content.into()));
            out.push_str(ending);
        }
    }
    out
}

fn rewrite_uri_attributes(tag: &str, base: &Url, link: &dyn Fn(&Url) -> String) -> String {
    const ATTRIBUTE: &str = "URI=\"";
    let mut out = String::with_capacity(tag.len());
    let mut rest = tag;
    while let Some(found) = rest.find(ATTRIBUTE) {
        // `URI=` must be a whole attribute name, not the end of another one.
        let whole = found == 0 || matches!(rest.as_bytes()[found - 1], b':' | b',' | b' ');
        let value_start = found + ATTRIBUTE.len();
        let Some(length) = rest[value_start..].find('"') else {
            break;
        };
        out.push_str(&rest[..value_start]);
        let value = &rest[value_start..value_start + length];
        match whole.then(|| resolve_and_link(value, base, link)).flatten() {
            Some(linked) => out.push_str(&linked),
            None => out.push_str(value),
        }
        rest = &rest[value_start + length..];
    }
    out.push_str(rest);
    out
}

fn resolve_and_link(address: &str, base: &Url, link: &dyn Fn(&Url) -> String) -> Option<String> {
    let resolved = base.join(address).ok()?;
    matches!(resolved.scheme(), "http" | "https").then(|| link(&resolved))
}

/// Rewrites the absolute and root-relative addresses of a DASH manifest: `BaseURL` elements and
/// the `media`, `initialization`, `sourceURL` and `xlink:href` attributes. Relative ones are left
/// to resolve against the proxied manifest's own path, unless the manifest was reached through a
/// redirect (`base` differs from where it was asked for): then relative `BaseURL`s are rewritten
/// too, and a manifest without one gets a `BaseURL` naming where it really is.
pub fn rewrite_dash(
    text: &str,
    base: &Url,
    redirected: bool,
    link: &dyn Fn(&Url) -> String,
) -> String {
    let rewrite = |value: &str, relative_too: bool| -> Option<String> {
        let decoded = value.trim().replace("&amp;", "&");
        let absolute = decoded.starts_with("http://")
            || decoded.starts_with("https://")
            || decoded.starts_with("//")
            || decoded.starts_with('/');
        if !absolute && !relative_too {
            return None;
        }
        resolve_and_link(&decoded, base, link).map(|linked| linked.replace('&', "&amp;"))
    };

    let mut out = String::with_capacity(text.len() + 256);
    let mut rest = text;
    let mut saw_base_url = false;
    loop {
        let next_element = rest.find("<BaseURL");
        let next_attribute = find_attribute(rest);
        let element_first = match (next_element, next_attribute) {
            (Some(element), Some((attribute, _))) => element < attribute,
            (Some(_), None) => true,
            (None, Some(_)) => false,
            (None, None) => break,
        };
        if element_first {
            let start = next_element.unwrap_or_default();
            let Some(open_end) = rest[start..].find('>').map(|end| start + end + 1) else {
                break;
            };
            let Some(close) = rest[open_end..]
                .find("</BaseURL>")
                .map(|close| open_end + close)
            else {
                break;
            };
            saw_base_url = true;
            out.push_str(&rest[..open_end]);
            let value = &rest[open_end..close];
            match rewrite(value, redirected) {
                Some(linked) => out.push_str(&linked),
                None => out.push_str(value),
            }
            rest = &rest[close..];
        } else {
            let (start, name_length) = next_attribute.unwrap_or_default();
            let value_start = start + name_length;
            let Some(length) = rest[value_start..].find('"') else {
                break;
            };
            out.push_str(&rest[..value_start]);
            let value = &rest[value_start..value_start + length];
            match rewrite(value, false) {
                Some(linked) => out.push_str(&linked),
                None => out.push_str(value),
            }
            rest = &rest[value_start + length..];
        }
    }
    out.push_str(rest);

    if redirected && !saw_base_url {
        insert_base_url(&out, base, link).unwrap_or(out)
    } else {
        out
    }
}

/// The next address attribute (`media="`, …) with its length, preceded by whitespace.
fn find_attribute(text: &str) -> Option<(usize, usize)> {
    const NAMES: [&str; 4] = [
        "media=\"",
        "initialization=\"",
        "sourceURL=\"",
        "xlink:href=\"",
    ];
    NAMES
        .iter()
        .filter_map(|name| {
            let mut from = 0;
            while let Some(found) = text[from..].find(name).map(|found| from + found) {
                if found > 0 && text.as_bytes()[found - 1].is_ascii_whitespace() {
                    return Some((found, name.len()));
                }
                from = found + name.len();
            }
            None
        })
        .min()
}

/// Puts `<BaseURL>` (the manifest's real folder, proxied) first inside the `MPD` element.
fn insert_base_url(text: &str, base: &Url, link: &dyn Fn(&Url) -> String) -> Option<String> {
    let mpd = text.find("<MPD")?;
    let open_end = mpd + text[mpd..].find('>')? + 1;
    let folder = base.join("./").ok()?;
    let element = format!("<BaseURL>{}</BaseURL>", link(&folder).replace('&', "&amp;"));
    Some(format!(
        "{}{element}{}",
        &text[..open_end],
        &text[open_end..]
    ))
}

/// The upstream address a `/media/<token>/` path stands for: `https/host:port/a/b` plus the
/// query. Only web addresses with a host are accepted.
pub fn upstream_url(rest: &str, query: Option<&str>) -> Option<Url> {
    let (scheme, rest) = rest.split_once('/')?;
    if !matches!(scheme, "http" | "https") {
        return None;
    }
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    if host.is_empty() || host.contains(['@', '\\']) {
        return None;
    }
    let mut address = format!("{scheme}://{host}/{path}");
    if let Some(query) = query.filter(|query| !query.is_empty()) {
        address.push('?');
        address.push_str(query);
    }
    let url = Url::parse(&address).ok()?;
    url.host_str().is_some().then_some(url)
}

/// Whether an address names an HLS playlist: its last path segment ends in `.m3u8` (`playlist.m3u8`,
/// `master.m3u8`, whatever the query). Players pick their HLS reader by it, so a song streamed in
/// HLS pieces plays through the `/media/` route, which keeps the name.
pub fn names_hls(address: &str) -> bool {
    Url::parse(address).is_ok_and(|url| {
        url.path_segments()
            .and_then(|mut segments| segments.next_back())
            .is_some_and(|last| last.to_ascii_lowercase().ends_with(".m3u8"))
    })
}

/// The `/media/` path (after the token) that stands for `url`.
pub fn media_path(url: &Url) -> String {
    let mut path = format!("{}/{}", url.scheme(), url.host_str().unwrap_or_default());
    if let Some(port) = url.port() {
        path.push_str(&format!(":{port}"));
    }
    path.push_str(url.path());
    if let Some(query) = url.query() {
        path.push('?');
        path.push_str(query);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(url: &Url) -> String {
        format!("http://proxy/media/t/{}", media_path(url))
    }

    #[test]
    fn hls_playlists_are_told_by_their_name() {
        // SoundCloud's, old and new, and a server's master playlist.
        for hls in [
            "https://cf-hls-media.sndcdn.com/playlist/AbC.128.mp3/playlist.m3u8?Policy=x&Signature=y",
            "https://playback.media-streaming.soundcloud.cloud/x/aac_160k/u/playlist.m3u8?expires=1",
            "http://127.0.0.1:8096/Audio/1/MASTER.M3U8",
        ] {
            assert!(names_hls(hls), "{hls}");
        }
        for other in [
            "https://cdn.example/song.mp3",
            "https://cdn.example/stream?format=m3u8",
            "https://cdn.example/playlist.m3u8/segment.ts",
            "not an address",
        ] {
            assert!(!names_hls(other), "{other}");
        }
    }

    #[test]
    fn sniffs_playlists_and_manifests_whatever_they_are_called() {
        assert_eq!(sniff(b"#EXTM3U\n#EXT-X-VERSION:3\n"), Some(Manifest::Hls));
        assert_eq!(sniff(b"\xEF\xBB\xBF  \r\n#EXTM3U\n"), Some(Manifest::Hls));
        assert_eq!(
            sniff(b"<?xml version=\"1.0\"?>\n<MPD xmlns=\"urn:mpeg:dash:schema:mpd:2011\">"),
            Some(Manifest::Dash)
        );
        assert_eq!(sniff(b"\x00\x00\x00\x18ftypmp42"), None);
        assert_eq!(sniff(b"G@\x11\x10"), None);
        assert_eq!(sniff(b"<html><body>no</body></html>"), None);
        assert_eq!(sniff(b""), None);
    }

    #[test]
    fn rewrites_every_address_of_an_hls_playlist() {
        let base = Url::parse("https://cdn.example/show/ep1/master.m3u8?sig=1").unwrap();
        let playlist = "#EXTM3U\r\n\
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"English\",URI=\"audio/en.m3u8\"\r\n\
#EXT-X-STREAM-INF:BANDWIDTH=800000,AUDIO=\"a\"\r\n\
720p/index.m3u8\r\n\
#EXT-X-STREAM-INF:BANDWIDTH=200000\r\n\
https://other.example/low.m3u8?token=a&b=c\r\n\
\r\n\
# a comment stays\r\n";
        let rewritten = rewrite_hls(playlist, &base, &link);
        assert_eq!(
            rewritten,
            "#EXTM3U\r\n\
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"English\",URI=\"http://proxy/media/t/https/cdn.example/show/ep1/audio/en.m3u8\"\r\n\
#EXT-X-STREAM-INF:BANDWIDTH=800000,AUDIO=\"a\"\r\n\
http://proxy/media/t/https/cdn.example/show/ep1/720p/index.m3u8\r\n\
#EXT-X-STREAM-INF:BANDWIDTH=200000\r\n\
http://proxy/media/t/https/other.example/low.m3u8?token=a&b=c\r\n\
\r\n\
# a comment stays\r\n"
        );
    }

    #[test]
    fn hls_keys_maps_and_root_relative_segments() {
        let base = Url::parse("http://cdn.example:8080/a/b/index.m3u8").unwrap();
        let playlist = "#EXTM3U\n\
#EXT-X-KEY:METHOD=AES-128,URI=\"/keys/k1\",IV=0x01\n\
#EXT-X-SESSION-KEY:METHOD=SAMPLE-AES,URI=\"skd://key-id\"\n\
#EXT-X-MAP:URI=\"data:application/octet-stream;base64,AAAA\"\n\
#EXTINF:4.0,\n\
/seg/1.ts\n\
#EXTINF:4.0,\n\
//mirror.example/seg/2.ts\n";
        let rewritten = rewrite_hls(playlist, &base, &link);
        assert!(rewritten.contains("URI=\"http://proxy/media/t/http/cdn.example:8080/keys/k1\""));
        assert!(rewritten.contains("URI=\"skd://key-id\""));
        assert!(rewritten.contains("URI=\"data:application/octet-stream;base64,AAAA\""));
        assert!(rewritten.contains("\nhttp://proxy/media/t/http/cdn.example:8080/seg/1.ts\n"));
        assert!(rewritten.contains("\nhttp://proxy/media/t/http/mirror.example/seg/2.ts\n"));
        // The last line had no ending and gets none.
        let unterminated = rewrite_hls("#EXTM3U\nseg.ts", &base, &link);
        assert!(unterminated.ends_with("/a/b/seg.ts"));
    }

    #[test]
    fn rewrites_absolute_dash_addresses_and_leaves_templates() {
        let base = Url::parse("https://cdn.example/v/manifest.mpd").unwrap();
        let manifest = r#"<?xml version="1.0"?>
<MPD xmlns="urn:mpeg:dash:schema:mpd:2011" mediaPresentationDuration="PT10S">
  <BaseURL>https://media.example/v/?a=1&amp;b=2</BaseURL>
  <Period>
    <AdaptationSet mimeType="video/mp4">
      <SegmentTemplate media="$RepresentationID$/$Number$.m4s" initialization="/init/$RepresentationID$.mp4" startNumber="1"/>
      <Representation id="720" bandwidth="800000"><BaseURL>720/</BaseURL></Representation>
    </AdaptationSet>
  </Period>
</MPD>"#;
        let rewritten = rewrite_dash(manifest, &base, false, &link);
        assert!(rewritten.contains(
            "<BaseURL>http://proxy/media/t/https/media.example/v/?a=1&amp;b=2</BaseURL>"
        ));
        assert!(rewritten.contains("media=\"$RepresentationID$/$Number$.m4s\""));
        assert!(rewritten.contains(
            "initialization=\"http://proxy/media/t/https/cdn.example/init/$RepresentationID$.mp4\""
        ));
        assert!(rewritten.contains("<BaseURL>720/</BaseURL>"));
        // `mimeType=` and other attributes are untouched.
        assert!(rewritten.contains("mimeType=\"video/mp4\""));
    }

    #[test]
    fn a_redirected_dash_manifest_names_its_real_folder() {
        let base = Url::parse("https://edge.example/real/path/manifest.mpd").unwrap();
        let manifest = "<MPD type=\"static\"><Period><AdaptationSet><SegmentTemplate media=\"seg-$Number$.m4s\"/></AdaptationSet></Period></MPD>";
        let rewritten = rewrite_dash(manifest, &base, true, &link);
        assert!(rewritten.starts_with(
            "<MPD type=\"static\"><BaseURL>http://proxy/media/t/https/edge.example/real/path/</BaseURL><Period>"
        ));
        let with_base = "<MPD><BaseURL>video/</BaseURL></MPD>";
        assert_eq!(
            rewrite_dash(with_base, &base, true, &link),
            "<MPD><BaseURL>http://proxy/media/t/https/edge.example/real/path/video/</BaseURL></MPD>"
        );
    }

    #[test]
    fn media_paths_lead_back_to_their_addresses() {
        let url = upstream_url("https/cdn.example:8443/a%20b/c.m3u8", Some("x=1&y=2")).unwrap();
        assert_eq!(
            url.as_str(),
            "https://cdn.example:8443/a%20b/c.m3u8?x=1&y=2"
        );
        assert_eq!(
            media_path(&url),
            "https/cdn.example:8443/a%20b/c.m3u8?x=1&y=2"
        );
        assert_eq!(
            upstream_url("http/host.example", None).unwrap().as_str(),
            "http://host.example/"
        );
        assert!(upstream_url("file/etc/passwd", None).is_none());
        assert!(upstream_url("https//path", None).is_none());
        assert!(upstream_url("https/user@host.example/x", None).is_none());
        assert!(upstream_url("ftp/host.example/x", None).is_none());
    }
}
