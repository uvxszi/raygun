//! Links that name torrents: magnet links (and bare info hashes), and `.torrent` files' web
//! addresses. Besides what BitTorrent's links say (`xt`, `dn`, `tr`, `so`), Raygun adds which
//! episode a session wants (`x.ep=<season>:<episode>` on magnets, `#ep=` on file addresses), so a
//! season's torrent opens the right file when no file index was given.

use std::str::FromStr;

use raygun_resolver::video_match::EpisodeNumber;

/// The `format` video sources and sessions mark torrent links with.
pub const FORMAT: &str = "torrent";

/// Raygun's own parameter for the episode wanted (BEP 9 leaves `x.` names to clients).
const EPISODE_PARAM: &str = "x.ep";
/// The same on a `.torrent` file's address, after `#`.
const FILE_EPISODE_PARAM: &str = "ep";
/// A file index on a `.torrent` file's address, after `#`.
const FILE_INDEX_PARAM: &str = "so";

/// A torrent as its link names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TorrentLink {
    Magnet(Magnet),
    /// A `.torrent` file's web address (without Raygun's `#` hints).
    File {
        url: String,
        hint: FileHint,
    },
}

impl TorrentLink {
    /// Which file to open, as far as the link says.
    pub fn hint(&self) -> &FileHint {
        match self {
            Self::Magnet(magnet) => &magnet.hint,
            Self::File { hint, .. } => hint,
        }
    }

    /// The torrent's name, when the link gives one (a magnet's `dn`).
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Magnet(magnet) => magnet.name.as_deref(),
            Self::File { .. } => None,
        }
    }
}

/// What a magnet link says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Magnet {
    /// The v1 info hash, 40 lowercase hex digits.
    pub info_hash: String,
    pub name: Option<String>,
    pub trackers: Vec<String>,
    pub hint: FileHint,
}

/// Which of a torrent's files is wanted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileHint {
    /// The file's index in the torrent (`so`, Stremio's `fileIdx`).
    pub index: Option<usize>,
    /// The episode a session wants, in each numbering it knows.
    pub episodes: Vec<EpisodeNumber>,
}

/// Whether a link opens as a torrent: a magnet link, a link marked `torrent`, or a `.torrent`
/// file's address.
pub fn is_torrent(url: &str, format: Option<&str>) -> bool {
    let url = url.trim();
    if format == Some(FORMAT)
        || url
            .get(..7)
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("magnet:"))
    {
        return true;
    }
    (url.starts_with("https://") || url.starts_with("http://"))
        && url::Url::parse(url).is_ok_and(|parsed| {
            parsed
                .path()
                .rsplit('/')
                .next()
                .is_some_and(|name| name.to_ascii_lowercase().ends_with(".torrent"))
        })
}

/// Why a link isn't a torrent Raygun opens.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct LinkError(pub String);

/// Reads a magnet link, a bare info hash or a `.torrent` file's address.
pub fn parse(link: &str) -> Result<TorrentLink, LinkError> {
    let link = link.trim();
    if is_bare_hash(link) {
        return Ok(TorrentLink::Magnet(Magnet {
            info_hash: hex_hash(link)?,
            name: None,
            trackers: Vec::new(),
            hint: FileHint::default(),
        }));
    }
    let url = url::Url::parse(link).map_err(|_| LinkError("This isn't a link".into()))?;
    match url.scheme() {
        "magnet" => parse_magnet(&url).map(TorrentLink::Magnet),
        "http" | "https" => {
            let mut hint = FileHint::default();
            if let Some(fragment) = url.fragment() {
                for (key, value) in url::form_urlencoded::parse(fragment.as_bytes()) {
                    match key.as_ref() {
                        FILE_INDEX_PARAM => hint.index = value.parse().ok(),
                        FILE_EPISODE_PARAM => hint.episodes.extend(parse_episode(&value)),
                        _ => {}
                    }
                }
            }
            let mut url = url;
            url.set_fragment(None);
            Ok(TorrentLink::File {
                url: url.into(),
                hint,
            })
        }
        _ => Err(LinkError(
            "Only magnet links and .torrent files' addresses open as torrents".into(),
        )),
    }
}

fn parse_magnet(url: &url::Url) -> Result<Magnet, LinkError> {
    let mut info_hash = None;
    let mut name = None;
    let mut trackers: Vec<String> = Vec::new();
    let mut hint = FileHint::default();
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "xt" => {
                if let Some(hash) = value.strip_prefix("urn:btih:") {
                    info_hash = Some(hex_hash(hash)?);
                }
            }
            "dn" if !value.trim().is_empty() => name = Some(value.trim().to_owned()),
            "tr" if !value.trim().is_empty() => {
                let tracker = value.trim().to_owned();
                if !trackers.contains(&tracker) {
                    trackers.push(tracker);
                }
            }
            // The first file of a selection is the one played.
            "so" => {
                hint.index = value
                    .split(',')
                    .filter_map(|part| part.split('-').next())
                    .find_map(|part| part.trim().parse().ok());
            }
            EPISODE_PARAM => hint.episodes.extend(parse_episode(&value)),
            _ => {}
        }
    }
    let info_hash = info_hash.ok_or_else(|| {
        LinkError("This magnet link has no BitTorrent v1 info hash (btih)".into())
    })?;
    Ok(Magnet {
        info_hash,
        name,
        trackers,
        hint,
    })
}

fn is_bare_hash(text: &str) -> bool {
    (text.len() == 40 && text.bytes().all(|byte| byte.is_ascii_hexdigit()))
        || (text.len() == 32
            && text
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || (b'2'..=b'7').contains(&byte)))
}

/// An info hash as 40 lowercase hex digits, from hex or base32.
fn hex_hash(hash: &str) -> Result<String, LinkError> {
    librqbit_core::hash_id::Id20::from_str(hash.trim())
        .map(|id| id.as_string())
        .map_err(|_| LinkError("The torrent's info hash isn't valid".into()))
}

/// `1:2` (season 1, episode 2) or `:14` (episode 14 counted across seasons).
fn parse_episode(text: &str) -> Option<EpisodeNumber> {
    let (season, episode) = text.split_once(':')?;
    Some(EpisodeNumber {
        season: if season.trim().is_empty() {
            None
        } else {
            Some(season.trim().parse().ok()?)
        },
        episode: episode.trim().parse().ok()?,
    })
}

fn episode_text(number: &EpisodeNumber) -> String {
    match number.season {
        Some(season) => format!("{season}:{}", number.episode),
        None => format!(":{}", number.episode),
    }
}

/// `link` with the episodes a session wants added, so a season's torrent opens the episode's
/// file. A link that isn't a torrent (by [`is_torrent`]), or already says, stays as it is.
pub fn with_episodes(link: &str, format: Option<&str>, episodes: &[EpisodeNumber]) -> String {
    if episodes.is_empty() || !is_torrent(link, format) {
        return link.to_owned();
    }
    let Ok(parsed) = parse(link) else {
        return link.to_owned();
    };
    if !parsed.hint().episodes.is_empty() {
        return link.to_owned();
    }
    let pairs: Vec<String> = episodes.iter().map(episode_text).collect();
    match parsed {
        TorrentLink::Magnet(_) => {
            let mut link = link.trim().to_owned();
            for pair in pairs {
                link.push_str(&format!("&{EPISODE_PARAM}={pair}"));
            }
            link
        }
        TorrentLink::File { url, mut hint } => {
            hint.episodes = episodes.to_vec();
            file_link(&url, &hint)
        }
    }
}

/// `link` opening the torrent's file `index` (`so=` on a magnet link, `#so=` on a `.torrent`
/// file's address), in place of any file it named. A link that isn't a torrent stays as it is.
pub fn with_file(link: &str, format: Option<&str>, index: usize) -> String {
    let trimmed = link.trim();
    if !(is_torrent(link, format) || is_bare_hash(trimmed)) {
        return link.to_owned();
    }
    match parse(trimmed) {
        Ok(TorrentLink::Magnet(magnet)) if is_bare_hash(trimmed) => {
            format!("magnet:?xt=urn:btih:{}&so={index}", magnet.info_hash)
        }
        Ok(TorrentLink::Magnet(_)) => {
            let (head, query) = trimmed.split_once('?').unwrap_or((trimmed, ""));
            let mut params: Vec<String> = query
                .split('&')
                .filter(|param| !param.is_empty() && !param.starts_with("so="))
                .map(str::to_owned)
                .collect();
            params.push(format!("so={index}"));
            format!("{head}?{}", params.join("&"))
        }
        Ok(TorrentLink::File { url, mut hint }) => {
            hint.index = Some(index);
            file_link(&url, &hint)
        }
        Err(_) => link.to_owned(),
    }
}

/// A `.torrent` file's address with Raygun's hints after `#`.
fn file_link(url: &str, hint: &FileHint) -> String {
    let mut fragment = url::form_urlencoded::Serializer::new(String::new());
    if let Some(index) = hint.index {
        fragment.append_pair(FILE_INDEX_PARAM, &index.to_string());
    }
    for number in &hint.episodes {
        fragment.append_pair(FILE_EPISODE_PARAM, &episode_text(number));
    }
    let fragment = fragment.finish();
    if fragment.is_empty() {
        url.to_owned()
    } else {
        format!("{url}#{fragment}")
    }
}

impl Magnet {
    /// The magnet link the engine is given: the hash, the name, and the trackers it can use.
    /// Trackers over HTTPS are left out where they can't be checked (`https_trackers` false).
    pub fn for_engine(&self, https_trackers: bool) -> String {
        let mut link = format!("magnet:?xt=urn:btih:{}", self.info_hash);
        if let Some(name) = &self.name {
            link.push_str("&dn=");
            link.push_str(&encode(name));
        }
        for tracker in self.usable_trackers(https_trackers) {
            link.push_str("&tr=");
            link.push_str(&encode(&tracker));
        }
        link
    }

    /// The trackers the engine can announce to.
    pub fn usable_trackers(&self, https_trackers: bool) -> Vec<String> {
        self.trackers
            .iter()
            .filter(|tracker| usable_tracker(tracker, https_trackers))
            .cloned()
            .collect()
    }
}

/// Whether a tracker's address is one the engine announces to: UDP and HTTP, and HTTPS where
/// certificates can be checked.
pub fn usable_tracker(tracker: &str, https_trackers: bool) -> bool {
    let lower = tracker.trim().to_ascii_lowercase();
    lower.starts_with("udp://")
        || lower.starts_with("http://")
        || (https_trackers && lower.starts_with("https://"))
}

fn encode(text: &str) -> String {
    url::form_urlencoded::byte_serialize(text.as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "08ada5a7a6183aae1e09d831df6748d566095a10";

    #[test]
    fn magnets_say_their_hash_name_trackers_and_file() {
        let link = format!(
            "magnet:?xt=urn:btih:{}&dn=Sintel&tr=udp%3A%2F%2Ftracker.example%3A1337&tr=https%3A%2F%2Ft.example%2Fannounce&so=3,5-6",
            HASH.to_uppercase()
        );
        let TorrentLink::Magnet(magnet) = parse(&link).unwrap() else {
            panic!("a magnet");
        };
        assert_eq!(magnet.info_hash, HASH);
        assert_eq!(magnet.name.as_deref(), Some("Sintel"));
        assert_eq!(magnet.trackers.len(), 2);
        assert_eq!(magnet.hint.index, Some(3));
        // HTTPS trackers stay out where certificates can't be checked.
        assert_eq!(
            magnet.for_engine(false),
            format!("magnet:?xt=urn:btih:{HASH}&dn=Sintel&tr=udp%3A%2F%2Ftracker.example%3A1337")
        );
        assert_eq!(magnet.usable_trackers(true).len(), 2);
    }

    #[test]
    fn bare_and_base32_hashes_are_magnets() {
        let TorrentLink::Magnet(bare) = parse(HASH).unwrap() else {
            panic!("a magnet");
        };
        assert_eq!(bare.info_hash, HASH);
        // The same hash in base32.
        let TorrentLink::Magnet(base32) =
            parse("magnet:?xt=urn:btih:BCW2LJ5GDA5K4HQJ3AY56Z2I2VTASWQQ").unwrap()
        else {
            panic!("a magnet");
        };
        assert_eq!(base32.info_hash, HASH);
        assert!(parse("magnet:?dn=nothing").is_err());
        assert!(parse("ftp://example.com/a.torrent").is_err());
    }

    #[test]
    fn torrent_links_are_told_apart() {
        assert!(is_torrent("magnet:?xt=urn:btih:abc", None));
        assert!(is_torrent("MAGNET:?xt=urn:btih:abc", None));
        assert!(is_torrent(
            "https://archive.org/download/Sintel/Sintel_archive.torrent",
            None
        ));
        assert!(is_torrent("https://example.com/get?id=1", Some(FORMAT)));
        assert!(!is_torrent("https://example.com/film.mp4", None));
        assert!(!is_torrent("https://example.com/a.torrent.html", None));
    }

    #[test]
    fn the_episode_wanted_travels_in_the_link() {
        let wanted = [
            EpisodeNumber {
                season: Some(1),
                episode: 2,
            },
            EpisodeNumber {
                season: None,
                episode: 14,
            },
        ];
        let magnet = with_episodes(&format!("magnet:?xt=urn:btih:{HASH}"), None, &wanted);
        assert_eq!(
            magnet,
            format!("magnet:?xt=urn:btih:{HASH}&x.ep=1:2&x.ep=:14")
        );
        assert_eq!(parse(&magnet).unwrap().hint().episodes, wanted);
        // Said once only.
        assert_eq!(with_episodes(&magnet, None, &wanted), magnet);

        let file = with_episodes("https://example.com/show.torrent", None, &wanted[..1]);
        assert_eq!(file, "https://example.com/show.torrent#ep=1%3A2");
        let TorrentLink::File { url, hint } = parse(&file).unwrap() else {
            panic!("a file");
        };
        assert_eq!(url, "https://example.com/show.torrent");
        assert_eq!(hint.episodes, wanted[..1]);
        // Not a torrent: as it was.
        assert_eq!(
            with_episodes("https://example.com/a.mp4", None, &wanted),
            "https://example.com/a.mp4"
        );
        // A source's torrent at an address without `.torrent`.
        assert_eq!(
            with_episodes("https://example.com/get?id=1", Some(FORMAT), &wanted[1..]),
            "https://example.com/get?id=1#ep=%3A14"
        );
    }

    #[test]
    fn a_link_can_name_the_file_to_open() {
        let magnet = format!("magnet:?xt=urn:btih:{HASH}&dn=Sintel&so=1&tr=udp%3A%2F%2Ft.example");
        let picked = with_file(&magnet, None, 4);
        assert_eq!(
            picked,
            format!("magnet:?xt=urn:btih:{HASH}&dn=Sintel&tr=udp%3A%2F%2Ft.example&so=4")
        );
        assert_eq!(parse(&picked).unwrap().hint().index, Some(4));
        assert_eq!(parse(&picked).unwrap().name(), Some("Sintel"));
        assert_eq!(
            with_file(HASH, None, 2),
            format!("magnet:?xt=urn:btih:{HASH}&so=2")
        );
        let file = with_file("https://example.com/a.torrent#so=1", None, 3);
        assert_eq!(file, "https://example.com/a.torrent#so=3");
        assert_eq!(
            with_file("https://example.com/a.mp4", None, 3),
            "https://example.com/a.mp4"
        );
    }
}
