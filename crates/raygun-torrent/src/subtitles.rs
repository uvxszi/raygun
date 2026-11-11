//! Subtitle files that come with a torrent's video ("Sintel.en.srt" beside "Sintel.mp4",
//! "Subs/2_English.srt"): which of them go with the video playing, and the language and kind
//! their names give, so the player offers them as subtitle tracks.

use raygun_resolver::video_match;

use crate::pick::{TorrentFile, is_extra, is_video};

/// Subtitle formats the players read, by extension.
const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "vtt", "webvtt", "ass", "ssa", "ttml", "dfxp"];
/// The largest file taken for subtitles.
pub const MAX_SUBTITLE_BYTES: u64 = 8 * 1024 * 1024;
/// Most subtitle files offered with one video.
const MAX_SUBTITLES: usize = 40;

/// Two-letter codes (ISO 639-1) of languages subtitles are written in.
const TWO_LETTER: &[&str] = &[
    "af", "am", "ar", "az", "be", "bg", "bn", "bs", "ca", "cs", "cy", "da", "de", "el", "en", "es",
    "et", "eu", "fa", "fi", "fr", "ga", "gl", "gu", "he", "hi", "hr", "hu", "hy", "id", "is", "it",
    "ja", "ka", "kk", "km", "kn", "ko", "ku", "ky", "lo", "lt", "lv", "mk", "ml", "mn", "mr", "ms",
    "mt", "my", "nb", "ne", "nl", "nn", "no", "pa", "pl", "ps", "pt", "ro", "ru", "si", "sk", "sl",
    "so", "sq", "sr", "sv", "sw", "ta", "te", "th", "tl", "tr", "uk", "ur", "uz", "vi", "yi", "zh",
    "zu",
];

/// Three-letter codes (ISO 639-2, both forms) and those subtitle sites use ("pob", "chs").
const THREE_LETTER: &[&str] = &[
    "afr", "alb", "amh", "ara", "arm", "aze", "baq", "bel", "ben", "bos", "bul", "bur", "cat",
    "ces", "chi", "chs", "cht", "cym", "cze", "dan", "deu", "dut", "ell", "eng", "est", "eus",
    "fas", "fil", "fin", "fra", "fre", "geo", "ger", "gle", "glg", "gre", "guj", "heb", "hin",
    "hrv", "hun", "hye", "ice", "ind", "isl", "ita", "jpn", "kan", "kat", "kaz", "khm", "kor",
    "kur", "lao", "lav", "lit", "mac", "mal", "mar", "may", "mkd", "mlt", "mon", "msa", "mya",
    "nep", "nld", "nno", "nob", "nor", "pan", "per", "pob", "pol", "por", "ron", "rum", "rus",
    "sin", "slk", "slo", "slv", "som", "spa", "sqi", "srp", "swa", "swe", "tam", "tel", "tgl",
    "tha", "tur", "ukr", "urd", "uzb", "vie", "wel", "yid", "zho", "zul", "spl", "sea",
];

/// Language names as file names write them: in English, and a few in their own words.
const NAMES: &[&str] = &[
    "afrikaans",
    "albanian",
    "arabic",
    "armenian",
    "azerbaijani",
    "basque",
    "belarusian",
    "bengali",
    "bosnian",
    "brazilian",
    "bulgarian",
    "burmese",
    "cantonese",
    "castellano",
    "castilian",
    "catalan",
    "chinese",
    "croatian",
    "czech",
    "danish",
    "deutsch",
    "dutch",
    "english",
    "espanol",
    "español",
    "estonian",
    "farsi",
    "filipino",
    "finnish",
    "flemish",
    "francais",
    "français",
    "french",
    "galician",
    "georgian",
    "german",
    "greek",
    "gujarati",
    "hebrew",
    "hindi",
    "hrvatski",
    "hungarian",
    "icelandic",
    "indonesian",
    "irish",
    "italian",
    "italiano",
    "japanese",
    "kannada",
    "kazakh",
    "khmer",
    "korean",
    "latino",
    "latvian",
    "lithuanian",
    "macedonian",
    "magyar",
    "malay",
    "malayalam",
    "maltese",
    "mandarin",
    "marathi",
    "mongolian",
    "nederlands",
    "nepali",
    "norsk",
    "norwegian",
    "persian",
    "polish",
    "polski",
    "portugues",
    "portuguese",
    "português",
    "punjabi",
    "romana",
    "romanian",
    "russian",
    "serbian",
    "simplified",
    "sinhala",
    "slovak",
    "slovenian",
    "spanish",
    "srpski",
    "suomi",
    "svenska",
    "swahili",
    "swedish",
    "tagalog",
    "tamil",
    "telugu",
    "thai",
    "traditional",
    "turkce",
    "turkish",
    "türkçe",
    "ukrainian",
    "urdu",
    "uzbek",
    "vietnamese",
    "welsh",
    "русский",
    "українська",
    "日本語",
    "中文",
    "简体",
    "繁體",
    "한국어",
];

/// Words in subtitle file names that say what kind they are, with how they're shown.
const KINDS: &[(&str, &str)] = &[
    ("forced", "Forced"),
    ("sdh", "SDH"),
    ("hi", "SDH"),
    ("cc", "CC"),
    ("full", "Full"),
    ("signs", "Signs"),
    ("songs", "Songs"),
    ("commentary", "Commentary"),
    ("default", "Default"),
];

/// A subtitle file to offer with a video.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtitleFile {
    pub file: TorrentFile,
    /// `srt`, `vtt`, `ass`, `ssa`, `ttml`.
    pub format: String,
    /// The language its name gives, as written there ("en", "eng", "English", "pt-BR"); the
    /// players read it as a language.
    pub language: Option<String>,
    /// What else its name says of it ("Forced", "SDH"), or its name when it gives no language.
    pub label: Option<String>,
}

/// A file's subtitle format, by its extension.
pub fn subtitle_format(name: &str) -> Option<&'static str> {
    let (_, extension) = name.rsplit_once('.')?;
    let extension = extension.to_ascii_lowercase();
    SUBTITLE_EXTENSIONS
        .iter()
        .copied()
        .find(|known| *known == extension)
        .map(|known| if known == "webvtt" { "vtt" } else { known })
}

/// A name without its extension.
fn stem(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

/// `text` without `prefix` at its start, ignoring case, when the prefix ends a word there
/// ("Film" starts "Film.en", not "Filmmaker").
fn after_prefix<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    let rest = &text[prefix.len()..];
    (!prefix.is_empty()
        && head.eq_ignore_ascii_case(prefix)
        && !rest.starts_with(|character: char| character.is_alphanumeric()))
    .then_some(rest)
}

/// Whether a word of a file name names a language: a code ("en", "eng"), a tag ("pt-BR",
/// "zh-Hans") or a name ("English", "Português").
fn is_language(word: &str) -> bool {
    let lower = word.to_lowercase();
    if let Some((base, rest)) = lower.split_once('-') {
        return (TWO_LETTER.contains(&base) || THREE_LETTER.contains(&base))
            && matches!(rest.len(), 2 | 4)
            && rest
                .chars()
                .all(|character| character.is_ascii_alphanumeric());
    }
    match lower.chars().count() {
        2 => TWO_LETTER.contains(&lower.as_str()),
        3 => THREE_LETTER.contains(&lower.as_str()),
        _ => NAMES.contains(&lower.as_str()),
    }
}

/// The words of a name: split at dots, underscores, spaces, brackets and commas, and at dashes
/// that don't join a language and its region ("pt-BR" stays one).
fn words(text: &str) -> Vec<&str> {
    text.split(|character: char| {
        matches!(
            character,
            '.' | '_' | ' ' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | '+'
        )
    })
    .flat_map(|part| {
        if is_language(part) {
            vec![part]
        } else {
            part.split('-').collect()
        }
    })
    .filter(|word| !word.is_empty())
    .collect()
}

/// The language and the kind a subtitle file's name gives: read from what follows the video's
/// name ("Sintel.en.srt" beside "Sintel.mp4"), else from its whole name ("2_English.srt"). The
/// last language named counts (a title's own words come first: "It.2017.en.srt"), and "HI"
/// beside another language means hearing-impaired, not Hindi.
pub fn read_name(name: &str, video_stem: &str) -> (Option<String>, Vec<&'static str>) {
    let own = stem(name);
    let rest = after_prefix(own, video_stem).unwrap_or(own);
    let words = words(rest);
    let languages: Vec<&str> = words
        .iter()
        .copied()
        .filter(|word| is_language(word))
        .collect();
    let hindi_is_a_kind = languages.len() > 1;
    let language = languages
        .iter()
        .rev()
        .find(|word| !(hindi_is_a_kind && word.eq_ignore_ascii_case("hi")))
        .map(|word| (*word).to_owned());
    let mut kinds: Vec<&'static str> = Vec::new();
    for word in &words {
        let lower = word.to_lowercase();
        let Some((_, shown)) = KINDS.iter().find(|(kind, _)| *kind == lower) else {
            continue;
        };
        // "hi" is the language itself when it's the only one named.
        if lower == "hi"
            && language
                .as_deref()
                .is_some_and(|found| found.eq_ignore_ascii_case("hi"))
        {
            continue;
        }
        if !kinds.contains(shown) {
            kinds.push(shown);
        }
    }
    (language, kinds)
}

/// The episode a name numbers, when it says its season and episode.
fn episode_of(name: &str) -> Option<(u32, u64)> {
    let info = video_match::parse_episode(name);
    let episode = info.episode.filter(|_| !info.weak)?;
    Some((info.season?, (episode * 100.0).round() as u64))
}

/// The subtitle files that go with `video` among a torrent's `files`: all of them when it holds
/// one video; when it holds several (a season), those named after the video, in a folder named
/// after it, or numbered as its episode. At most a few dozen, smallest files only.
pub fn subtitles_for(files: &[TorrentFile], video: &TorrentFile) -> Vec<SubtitleFile> {
    let video_stem = stem(video.name());
    let largest = files
        .iter()
        .filter(|file| is_video(&file.path))
        .map(|file| file.len)
        .max()
        .unwrap_or(0);
    let several = files
        .iter()
        .filter(|file| is_video(&file.path) && !(is_extra(file) && file.len < largest / 3))
        .count()
        > 1;
    let episode = episode_of(video.name());
    let belongs = |file: &TorrentFile| {
        if !several {
            return true;
        }
        if after_prefix(file.name(), video_stem).is_some() {
            return true;
        }
        let folders = file
            .path
            .rsplit_once('/')
            .map_or("", |(folders, _)| folders);
        if folders
            .split('/')
            .any(|folder| folder.eq_ignore_ascii_case(video_stem))
        {
            return true;
        }
        episode.is_some() && episode_of(file.name()).or_else(|| episode_of(&file.path)) == episode
    };
    files
        .iter()
        .filter(|file| file.len > 0 && file.len <= MAX_SUBTITLE_BYTES)
        .filter_map(|file| subtitle_format(file.name()).map(|format| (file, format)))
        .filter(|(file, _)| belongs(file))
        .take(MAX_SUBTITLES)
        .map(|(file, format)| {
            let (language, kinds) = read_name(file.name(), video_stem);
            let label = if kinds.is_empty() {
                language.is_none().then(|| file.name().to_owned())
            } else {
                Some(kinds.join(" "))
            };
            SubtitleFile {
                file: file.clone(),
                format: format.to_owned(),
                language,
                label,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(index: usize, path: &str, kb: u64) -> TorrentFile {
        TorrentFile {
            index,
            path: path.into(),
            len: kb * 1024,
        }
    }

    fn read(name: &str, video: &str) -> (Option<String>, Vec<&'static str>) {
        read_name(name, video)
    }

    #[test]
    fn sintels_subtitle_files_come_with_its_video() {
        let files = [
            file(0, "Sintel/Sintel.de.srt", 2),
            file(1, "Sintel/Sintel.en.srt", 2),
            file(2, "Sintel/Sintel.es.srt", 2),
            file(3, "Sintel/Sintel.fr.srt", 2),
            file(4, "Sintel/Sintel.it.srt", 2),
            file(5, "Sintel/Sintel.mp4", 126_000),
            file(6, "Sintel/Sintel.nl.srt", 2),
            file(7, "Sintel/Sintel.pl.srt", 2),
            file(8, "Sintel/Sintel.pt.srt", 2),
            file(9, "Sintel/Sintel.ru.srt", 2),
            file(10, "Sintel/poster.jpg", 40),
        ];
        let found = subtitles_for(&files, &files[5]);
        let languages: Vec<&str> = found
            .iter()
            .filter_map(|subtitle| subtitle.language.as_deref())
            .collect();
        assert_eq!(
            languages,
            ["de", "en", "es", "fr", "it", "nl", "pl", "pt", "ru"]
        );
        assert!(found.iter().all(|subtitle| subtitle.format == "srt"));
        assert!(found.iter().all(|subtitle| subtitle.label.is_none()));
        assert_eq!(found[1].file.index, 1);
    }

    #[test]
    fn names_give_the_language_and_the_kind() {
        assert_eq!(read("Film.en.srt", "Film"), (Some("en".into()), vec![]));
        assert_eq!(
            read("Film.en.forced.srt", "Film"),
            (Some("en".into()), vec!["Forced"])
        );
        assert_eq!(
            read("Film.pt-BR.sdh.srt", "Film"),
            (Some("pt-BR".into()), vec!["SDH"])
        );
        // HI beside a language is hearing-impaired; alone, it's Hindi.
        assert_eq!(
            read("Film.en.hi.srt", "Film"),
            (Some("en".into()), vec!["SDH"])
        );
        assert_eq!(read("Film.hi.srt", "Film"), (Some("hi".into()), vec![]));
        // Other names: the language named last, past the title's own words.
        assert_eq!(
            read("2_English.srt", "Film"),
            (Some("English".into()), vec![])
        );
        assert_eq!(
            read("It.2017.eng.srt", "It.2017.1080p.BluRay"),
            (Some("eng".into()), vec![])
        );
        assert_eq!(
            read("Film.2019.Spanish (Latino).ass", "Film.2019.1080p"),
            (Some("Latino".into()), vec![])
        );
        assert_eq!(read("Film.forced.srt", "Film"), (None, vec!["Forced"]));
        assert_eq!(read("Film.srt", "Film"), (None, vec![]));
        assert_eq!(subtitle_format("A.WebVTT"), Some("vtt"));
        assert_eq!(subtitle_format("A.ASS"), Some("ass"));
        assert_eq!(subtitle_format("a.sub"), None);
        assert_eq!(subtitle_format("notes"), None);
    }

    #[test]
    fn a_seasons_torrent_offers_the_episodes_own_subtitles() {
        let files = [
            file(0, "Show S01/Show.S01E01.1080p.mkv", 700_000),
            file(1, "Show S01/Show.S01E01.1080p.en.srt", 40),
            file(2, "Show S01/Show.S01E02.1080p.mkv", 650_000),
            file(3, "Show S01/Show.S01E02.1080p.en.srt", 40),
            file(4, "Show S01/Subs/Show.S01E02.1080p/3_French.srt", 40),
            file(5, "Show S01/Subs/S01E02.Spanish.srt", 40),
            file(6, "Show S01/Subs/S01E01.German.srt", 40),
            // Not subtitles: too large, or empty.
            file(7, "Show S01/Show.S01E02.1080p.de.srt", 9 * 1024),
            file(8, "Show S01/Show.S01E02.1080p.fr.srt", 0),
        ];
        let found: Vec<(usize, Option<String>, Option<String>)> = subtitles_for(&files, &files[2])
            .into_iter()
            .map(|subtitle| (subtitle.file.index, subtitle.language, subtitle.label))
            .collect();
        assert_eq!(
            found,
            vec![
                (3, Some("en".into()), None),
                (4, Some("French".into()), None),
                (5, Some("Spanish".into()), None),
            ]
        );
    }

    #[test]
    fn a_film_with_a_sample_takes_every_subtitle_file() {
        let files = [
            file(0, "Film/Film.2010.mkv", 1_200_000),
            file(1, "Film/Sample/film-sample.mkv", 40_000),
            file(2, "Film/Subs/English.srt", 50),
            file(3, "Film/Subs/Commentary.srt", 50),
        ];
        let found = subtitles_for(&files, &files[0]);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].language.as_deref(), Some("English"));
        // No language in its name: listed by its name, its kind said.
        assert_eq!(found[1].language, None);
        assert_eq!(found[1].label.as_deref(), Some("Commentary"));
    }
}
