//! Audio from other sources: the session's other streams of the episode playing, offered for
//! their sound. The player plays one beside the picture it has (Media3's `MergingMediaSource` on
//! Android, a second media element kept in step on desktop), so a dub or the original language
//! from another addon, server or release plays over the picture of the source that plays best.
//!
//! What's offered: links of the same episode (not those whose names say another one) that can
//! play (not failed, a web address, not a torrent, a page or a live stream), the languages their
//! names give first when the stream playing doesn't have them, one per source and language, at
//! most [`MOST_OFFERS`], in the order the session tries links.

use std::collections::{BTreeMap, HashSet};

use serde::Serialize;
use serde_json::{Value, json};

use super::{DOWNLOAD_GROUP, GroupState, Link, Session, own_group};

/// Most other streams offered for their sound.
pub const MOST_OFFERS: usize = 12;

/// Words for spoken languages in streams' names, with the language each names. Two-letter codes
/// are left out ("it", "de" and "no" are words), and so are three-letter ones that are common
/// names or words ("dan", "ron", "por", "tel").
const WORDS: &[(&str, &str)] = &[
    ("eng", "en"),
    ("english", "en"),
    ("ita", "it"),
    ("italian", "it"),
    ("italiano", "it"),
    ("spa", "es"),
    ("esp", "es"),
    ("spanish", "es"),
    ("español", "es"),
    ("espanol", "es"),
    ("castellano", "es"),
    ("castilian", "es"),
    ("latino", "es"),
    ("latam", "es"),
    ("fre", "fr"),
    ("fra", "fr"),
    ("french", "fr"),
    ("français", "fr"),
    ("francais", "fr"),
    ("truefrench", "fr"),
    ("vf", "fr"),
    ("vff", "fr"),
    ("vfq", "fr"),
    ("vfi", "fr"),
    ("ger", "de"),
    ("deu", "de"),
    ("german", "de"),
    ("deutsch", "de"),
    ("jap", "ja"),
    ("jpn", "ja"),
    ("japanese", "ja"),
    ("kor", "ko"),
    ("korean", "ko"),
    ("chn", "zh"),
    ("zho", "zh"),
    ("chinese", "zh"),
    ("mandarin", "zh"),
    ("cantonese", "zh"),
    ("rus", "ru"),
    ("russian", "ru"),
    ("portuguese", "pt"),
    ("português", "pt"),
    ("portugues", "pt"),
    ("dublado", "pt"),
    ("ptbr", "pt"),
    ("hin", "hi"),
    ("hindi", "hi"),
    ("tamil", "ta"),
    ("telugu", "te"),
    ("malayalam", "ml"),
    ("kannada", "kn"),
    ("bengali", "bn"),
    ("marathi", "mr"),
    ("punjabi", "pa"),
    ("tur", "tr"),
    ("turkish", "tr"),
    ("pol", "pl"),
    ("polish", "pl"),
    ("polski", "pl"),
    ("dut", "nl"),
    ("nld", "nl"),
    ("dutch", "nl"),
    ("ara", "ar"),
    ("arabic", "ar"),
    ("heb", "he"),
    ("hebrew", "he"),
    ("tha", "th"),
    ("thai", "th"),
    ("vie", "vi"),
    ("vietnamese", "vi"),
    ("ukr", "uk"),
    ("ukrainian", "uk"),
    ("cze", "cs"),
    ("ces", "cs"),
    ("czech", "cs"),
    ("hun", "hu"),
    ("hungarian", "hu"),
    ("gre", "el"),
    ("ell", "el"),
    ("greek", "el"),
    ("swe", "sv"),
    ("swedish", "sv"),
    ("nor", "no"),
    ("norwegian", "no"),
    ("danish", "da"),
    ("finnish", "fi"),
    ("rum", "ro"),
    ("romanian", "ro"),
    ("indonesian", "id"),
    ("malay", "ms"),
    ("persian", "fa"),
    ("farsi", "fa"),
    ("tagalog", "tl"),
    ("filipino", "tl"),
];

/// Words saying a stream has several languages ("Multi", "Dual Audio").
const SEVERAL: &[&str] = &["multi", "multiaudio", "dual"];

/// The language a country's flag stands for in streams' names ("🇮🇹 ITA", "🇬🇧/🇪🇸"); countries of
/// several languages (Canada, Switzerland, Belgium) say nothing.
fn flag_language(country: &str) -> Option<&'static str> {
    Some(match country {
        "GB" | "US" | "AU" | "NZ" | "IE" => "en",
        "IT" => "it",
        "ES" | "MX" | "AR" | "CO" | "CL" | "PE" | "VE" => "es",
        "FR" => "fr",
        "DE" | "AT" => "de",
        "JP" => "ja",
        "KR" => "ko",
        "CN" | "TW" | "HK" => "zh",
        "RU" => "ru",
        "PT" | "BR" => "pt",
        "IN" => "hi",
        "TR" => "tr",
        "PL" => "pl",
        "NL" => "nl",
        "SE" => "sv",
        "NO" => "no",
        "DK" => "da",
        "FI" => "fi",
        "CZ" => "cs",
        "HU" => "hu",
        "GR" => "el",
        "UA" => "uk",
        "SA" | "AE" | "EG" => "ar",
        "IL" => "he",
        "TH" => "th",
        "VN" => "vi",
        "ID" => "id",
        "MY" => "ms",
        "RO" => "ro",
        "BG" => "bg",
        "HR" => "hr",
        "RS" => "sr",
        "SK" => "sk",
        "IR" => "fa",
        "PH" => "tl",
        _ => return None,
    })
}

/// The spoken languages a stream's names give.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Spoken {
    /// Languages as codes ("en", "it"), each once, in the order named.
    pub codes: Vec<&'static str>,
    /// Its names say it has several ("Multi", "Dual Audio").
    pub several: bool,
}

/// What a stream's names (its label, its description) say it's spoken in: flags, language names
/// and their codes ("ITA", "Latino", "VFF", "PT-BR"), and words for several languages.
pub fn spoken(texts: &[&str]) -> Spoken {
    let mut codes: Vec<&'static str> = Vec::new();
    let mut several = false;
    let mut add = |code: &'static str| {
        if !codes.contains(&code) {
            codes.push(code);
        }
    };
    let letter = |c: char| {
        let code = c as u32;
        (0x1F1E6..=0x1F1FF)
            .contains(&code)
            .then(|| char::from(b'A' + (code - 0x1F1E6) as u8))
    };
    for text in texts {
        // Flags: two regional indicator letters name a country.
        let chars: Vec<char> = text.chars().collect();
        let mut at = 0;
        while at + 1 < chars.len() {
            match (letter(chars[at]), letter(chars[at + 1])) {
                (Some(first), Some(second)) => {
                    if let Some(code) = flag_language(&format!("{first}{second}")) {
                        add(code);
                    }
                    at += 2;
                }
                _ => at += 1,
            }
        }
        let lower = text.to_lowercase();
        // "PT-BR" and "pt_br": two words that name Brazilian Portuguese together.
        if lower.replace(['-', '_', ' ', '.'], "").contains("ptbr") {
            add("pt");
        }
        for word in lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|word| !word.is_empty())
        {
            if let Some((_, code)) = WORDS.iter().find(|(name, _)| *name == word) {
                add(code);
            }
            several |= SEVERAL.contains(&word);
        }
    }
    Spoken { codes, several }
}

/// Another stream of the episode, offered for its sound.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioOffer {
    /// `<group>:<link>`: the same while the episode's links stay.
    pub id: String,
    pub group: usize,
    pub link: usize,
    /// The source it comes from (the addon, the server).
    pub source: String,
    /// The languages its names give, as codes.
    pub languages: Vec<&'static str>,
    /// Its names say it has several languages.
    pub several: bool,
    /// What its names say of it ("1080p · WEB-DL"), else its own name, short.
    pub title: String,
    /// Its sound, when its names say ("DD+ 5.1").
    pub sound: Option<String>,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub format: Option<String>,
}

/// Whether a link's sound can be played beside another stream's picture: a web address the
/// player opens itself (not a torrent, which plays through the engine one at a time; not a page
/// or YouTube's embed; not a live stream, whose clock is its own).
fn playable_beside(link: &Link) -> bool {
    let request = &link.request;
    let web = request.url.starts_with("https://") || request.url.starts_with("http://");
    web && !request.live
        && !raygun_torrent::is_torrent(&request.url, request.format.as_deref())
        && !crate::video::is_page(request)
}

/// "1080p · WEB-DL", or the link's own name (its first line, short) when its names say neither.
fn offer_title(link: &Link) -> String {
    let facts = link.facts();
    let parts: Vec<String> = facts
        .height
        .map(|height| format!("{height}p"))
        .into_iter()
        .chain(facts.release.map(str::to_owned))
        .collect();
    if !parts.is_empty() {
        return parts.join(" · ");
    }
    let first = link.label.lines().next().unwrap_or_default().trim();
    first.chars().take(60).collect()
}

/// The session's other streams of the episode playing that can give their sound, best first: a
/// language the stream playing doesn't have, then several languages, then the same languages,
/// then those that say none; within each, as the session would try them. One per source and
/// languages, at most [`MOST_OFFERS`]. Nothing while nothing plays, while Offline Mode is on, or
/// for a live stream.
pub fn audio_offers(session: &Session, now: i64) -> Vec<AudioOffer> {
    let Some((playing_group, playing_link)) = session.current else {
        return Vec::new();
    };
    let Some(playing) = session
        .groups
        .get(playing_group)
        .and_then(|group| group.links.get(playing_link))
    else {
        return Vec::new();
    };
    if playing.request.live || raygun_net::network_mode::offline() {
        return Vec::new();
    }
    let heard = spoken(&[&playing.label]);
    let ranking = session.ranking(now);
    let link_session = session.link.is_some();
    let mut found = Vec::new();
    for (group_index, group) in session.groups.iter().enumerate() {
        // A download is the same file as a stream of it; a link's own torrent files aren't sound.
        if group.state != GroupState::Ready
            || group.id == DOWNLOAD_GROUP
            || (link_session && own_group(true, group))
        {
            continue;
        }
        for (link_index, link) in group.links.iter().enumerate() {
            let usable = link.failed.is_none()
                && !link.mismatch
                && !session.dead_urls.contains(&link.request.url)
                && link.request.url != playing.request.url
                && (group_index, link_index) != (playing_group, playing_link)
                && playable_beside(link);
            if !usable {
                continue;
            }
            let says = spoken(&[&link.label]);
            let tier = if says.codes.iter().any(|code| !heard.codes.contains(code)) {
                0
            } else if says.several && !heard.several {
                1
            } else if !says.codes.is_empty() || says.several {
                2
            } else {
                3
            };
            found.push((
                (
                    tier,
                    ranking.link_key(&group.name, link),
                    group_index,
                    link_index,
                ),
                group_index,
                link_index,
                says,
            ));
        }
    }
    found.sort_by_key(|entry| entry.0);
    let mut seen = HashSet::new();
    found
        .into_iter()
        .filter(|(_, group, _, says)| {
            let mut codes = says.codes.clone();
            codes.sort_unstable();
            seen.insert((*group, codes, says.several))
        })
        .take(MOST_OFFERS)
        .map(|(_, group_index, link_index, says)| {
            let group = &session.groups[group_index];
            let link = &group.links[link_index];
            AudioOffer {
                id: format!("{group_index}:{link_index}"),
                group: group_index,
                link: link_index,
                source: group.name.clone(),
                languages: says.codes,
                several: says.several,
                title: offer_title(link),
                sound: link.facts().audio,
                url: link.request.url.clone(),
                headers: link.request.headers.clone(),
                format: link.request.format.clone(),
            }
        })
        .collect()
}

/// The player's `audioSources` update: the offers for the video playing (`id`, its key), so
/// offers for one that's on its way out aren't taken for the next.
pub fn audio_update(session: &Session, now: i64) -> Value {
    let id = session
        .current
        .and_then(|(group, link)| session.groups.get(group)?.links.get(link))
        .map(|link| link.request.key.clone());
    json!({
        "type": "audioSources",
        "id": id,
        "items": audio_offers(session, now),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_and_names_say_what_a_stream_is_spoken_in() {
        assert_eq!(
            spoken(&["Torrentio\n4k | 🇬🇧 / 🇮🇹 | 12 GB"]).codes,
            vec!["en", "it"]
        );
        assert_eq!(
            spoken(&["Movie.2019.1080p.ITA.ENG.WEB-DL"]).codes,
            vec!["it", "en"]
        );
        assert_eq!(spoken(&["[Latino] 1080p"]).codes, vec!["es"]);
        assert_eq!(spoken(&["Film.TRUEFRENCH.720p"]).codes, vec!["fr"]);
        assert_eq!(spoken(&["Filme Dublado PT-BR"]).codes, vec!["pt"]);
        assert_eq!(spoken(&["Anime S01E01 (Japanese Audio)"]).codes, vec!["ja"]);
        let several = spoken(&["Movie 2160p MULTi DV"]);
        assert!(several.several);
        assert!(several.codes.is_empty());
        assert!(spoken(&["Dual Audio 1080p"]).several);
        assert!(spoken(&["Multi Audio"]).several);
    }

    #[test]
    fn words_that_only_look_like_languages_say_nothing() {
        // WEB-DL's "DL", "it", "de", names and plain words.
        let none = spoken(&["It Follows 2014 1080p WEB-DL x264 by Dan, subido por Ron"]);
        assert!(none.codes.is_empty(), "{none:?}");
        assert!(!none.several);
        // Canada's flag could be English or French.
        assert!(spoken(&["🇨🇦 1080p"]).codes.is_empty());
    }
}
