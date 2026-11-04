//! Season and episode identity for videos: the season a title names ("Title II", "2nd Season",
//! "S2", "Part 2", "第2期"), the title without it, the season and episode a file or stream name
//! gives ("S01E02", "[Group] Title S2 - 03", "第3話"), whether two titles are one show's same
//! season, the order a title's episodes are watched in, and what an episode is called. Anime
//! sites and Kitsu list each season as a title of its own; Cinemeta lists every season under one.
//! Pure logic; the app does the I/O.

use std::cmp::Ordering;
use std::sync::LazyLock;

use regex::{Captures, Regex};
use unicode_normalization::UnicodeNormalization;

#[cfg(test)]
mod tests;

/// What a title says about its season.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TitleInfo {
    /// The title without its season, part, year and decorations: lower case, accents folded,
    /// words separated by single spaces.
    pub base: String,
    /// The same words as written, for showing.
    pub name: String,
    /// The season it names: "Season 2", "II", "2nd Season", "S2", "第2期".
    pub season: Option<u32>,
    /// The part (split cour) it names: "Part 2", "Cour 2", "第2クール".
    pub part: Option<u32>,
    /// "The Final Season": a season with no number.
    pub final_season: bool,
    /// The year it carries, as remakes do: "Hunter x Hunter (2011)".
    pub year: Option<i32>,
}

impl TitleInfo {
    /// Whether the title names a season, a part or the final season.
    pub fn names_season(&self) -> bool {
        self.season.is_some() || self.part.is_some() || self.final_season
    }

    /// The base as compared: no spaces, long vowels folded ("Kyoukai" and "Kyokai" agree).
    pub fn key(&self) -> String {
        compare_key(&self.base)
    }

    /// Whether `other` names the same season: the same number (none counts as the first), part
    /// (none counts as the first) and final season, and years at most one apart when both have
    /// one.
    pub fn same_season(&self, other: &Self) -> bool {
        let years = match (self.year, other.year) {
            (Some(a), Some(b)) => (a - b).abs() <= 1,
            _ => true,
        };
        years
            && self.final_season == other.final_season
            && (self.final_season || self.season.unwrap_or(1) == other.season.unwrap_or(1))
            && self.part.unwrap_or(1) == other.part.unwrap_or(1)
    }

    /// Whether `other` is the same show's same season.
    pub fn same_title(&self, other: &Self) -> bool {
        !self.base.is_empty() && self.key() == other.key() && self.same_season(other)
    }
}

/// Whether two titles are the same show's same season: "Mushoku Tensei II: Isekai Ittara Honki
/// Dasu" and "Mushoku Tensei: Isekai Ittara Honki Dasu 2nd Season" are; "Overlord" and
/// "Overlord II" aren't.
pub fn same_title(a: &str, b: &str) -> bool {
    parse_title(a).same_title(&parse_title(b))
}

/// The season a title names, if any.
pub fn season_of_title(title: &str) -> Option<u32> {
    parse_title(title).season
}

/// A title without its season marker, normalized ("Overlord II" → "overlord").
pub fn base_title(title: &str) -> String {
    parse_title(title).base
}

/// Words that name a season when a count follows ("Season 2") or an ordinal comes before
/// ("2nd Season").
const SEASON_WORDS: [&str; 7] = [
    "season",
    "saison",
    "temporada",
    "staffel",
    "stagione",
    "sezon",
    "сезон",
];
/// Words that name a part (a split cour) the same ways.
const PART_WORDS: [&str; 5] = ["part", "partie", "parte", "teil", "cour"];
/// Bracketed words that only say which version it is: "(TV)", "[Dub]", "(English Dub)".
const DECORATIONS: [&str; 30] = [
    "tv",
    "sub",
    "subs",
    "subbed",
    "dub",
    "dubs",
    "dubbed",
    "raw",
    "uncensored",
    "uncut",
    "english",
    "eng",
    "japanese",
    "multi",
    "audio",
    "dual",
    "vostfr",
    "legendado",
    "dublado",
    "hd",
    "sd",
    "fhd",
    "uhd",
    "4k",
    "480p",
    "720p",
    "1080p",
    "2160p",
    "bd",
    "web",
];
/// A number after these is the title's own ("Movie 2", "Vol 3"), not a season.
const NUMBER_PREFIXES: [&str; 14] = [
    "no", "vol", "volume", "episode", "ep", "chapter", "ch", "case", "number", "movie", "film",
    "ova", "oad", "special",
];
const NUMBER_WORDS: [&str; 20] = [
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
];
const ORDINAL_WORDS: [&str; 12] = [
    "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth",
    "eleventh", "twelfth",
];
/// Roman numerals read as seasons. A lone "X" ends many names ("Gundam X"), so it never counts.
const ROMAN: [&str; 20] = [
    "i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x", "xi", "xii", "xiii", "xiv", "xv",
    "xvi", "xvii", "xviii", "xix", "xx",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ordinal {
    Nth(u32),
    Final,
}

fn ordinal(word: &str) -> Option<Ordinal> {
    if matches!(word, "final" | "last") {
        return Some(Ordinal::Final);
    }
    if let Some(index) = ORDINAL_WORDS.iter().position(|known| *known == word) {
        return Some(Ordinal::Nth(index as u32 + 1));
    }
    let digits = word.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let suffix = &word[digits.len()..];
    (!digits.is_empty()
        && digits.chars().all(|c| c.is_ascii_digit())
        && matches!(suffix, "st" | "nd" | "rd" | "th"))
    .then(|| digits.parse().ok())
    .flatten()
    .filter(|number| (1..=99).contains(number))
    .map(Ordinal::Nth)
}

fn roman_value(word: &str) -> Option<u32> {
    ROMAN
        .iter()
        .position(|known| *known == word)
        .map(|index| index as u32 + 1)
}

/// A count after "Season" or "Part": digits, a roman numeral or a word.
fn count(word: &str) -> Option<u32> {
    if !word.is_empty() && word.chars().all(|c| c.is_ascii_digit()) {
        return word.parse().ok().filter(|number| *number <= 99);
    }
    roman_value(word).or_else(|| {
        NUMBER_WORDS
            .iter()
            .position(|known| *known == word)
            .map(|index| index as u32 + 1)
    })
}

/// "s2", "s02": a season.
fn s_number(word: &str) -> Option<u32> {
    let digits = word.strip_prefix('s')?;
    (matches!(digits.len(), 1 | 2) && digits.chars().all(|c| c.is_ascii_digit()))
        .then(|| digits.parse().ok())
        .flatten()
}

/// A four-digit year a title may carry.
fn year_of(word: &str) -> Option<i32> {
    (word.len() == 4 && word.chars().all(|c| c.is_ascii_digit()))
        .then(|| word.parse().ok())
        .flatten()
        .filter(|year| (1900..=2035).contains(year))
}

#[derive(Debug, Clone)]
struct Token {
    /// As written.
    text: String,
    /// Lower case, accents folded.
    lower: String,
    /// It starts the text, or a subtitle separator (": ", " - ", "~", a bracket) comes before it.
    break_before: bool,
    /// Its bracketed group, from 1; 0 outside brackets.
    group: usize,
}

/// Accents dropped, the rest recomposed (Hangul stays syllables), lower case.
fn fold(word: &str) -> String {
    word.nfd()
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
        .nfc()
        .collect::<String>()
        .to_lowercase()
}

/// Whether the characters between two words separate a title from its subtitle.
fn is_break(separator: &str) -> bool {
    let spaced = separator.chars().any(char::is_whitespace);
    separator.chars().any(|c| match c {
        '~' | '～' | '|' | '(' | ')' | '[' | ']' | '{' | '}' | '【' | '】' | '「' | '」' | '〔'
        | '〕' | '–' | '—' => true,
        ':' | '-' => spaced,
        _ => false,
    })
}

fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut word = String::new();
    let mut separator = String::new();
    let mut depth = 0usize;
    let mut groups = 0usize;
    let mut group = 0usize;
    let mut break_before = true;
    let push = |tokens: &mut Vec<Token>, word: &mut String, break_before: bool, group: usize| {
        if !word.is_empty() {
            tokens.push(Token {
                lower: fold(word),
                text: std::mem::take(word),
                break_before,
                group,
            });
        }
    };
    for c in text.chars() {
        if c.is_alphanumeric() {
            if word.is_empty() {
                break_before = tokens.is_empty() || is_break(&separator);
                group = if depth > 0 { groups } else { 0 };
                separator.clear();
            }
            word.push(c);
            continue;
        }
        push(&mut tokens, &mut word, break_before, group);
        separator.push(c);
        match c {
            '(' | '[' | '{' | '【' | '「' | '〔' => {
                depth += 1;
                if depth == 1 {
                    groups += 1;
                }
            }
            ')' | ']' | '}' | '】' | '」' | '〕' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    push(&mut tokens, &mut word, break_before, group);
    tokens
}

/// "第2期", "第二季", "2期", "2nd シーズン", "第2クール", "시즌 2", "2기": the markers of titles
/// in Japanese, Chinese, Korean and Thai.
static CJK_ORDINAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"第\s*([0-9]+|[〇零一二三四五六七八九十]+)\s*(期|季|シーズン|シリーズ|部|章|クール|幕)",
    )
    .expect("valid pattern")
});
static CJK_COUNTED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"([0-9]+)\s*(?:st|nd|rd|th)?\s*(期|季|シーズン|クール|기)").expect("valid pattern")
});
static CJK_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(シーズン|시즌|ซีซั่น|ภาค)\s*([0-9]+)").expect("valid pattern"));

fn chinese_number(text: &str) -> Option<u32> {
    if let Ok(number) = text.parse() {
        return Some(number);
    }
    let digit = |c: char| match c {
        '〇' | '零' => Some(0),
        _ => "一二三四五六七八九"
            .chars()
            .position(|known| known == c)
            .map(|index| index as u32 + 1),
    };
    match text.chars().collect::<Vec<_>>()[..] {
        ['十'] => Some(10),
        [one] => digit(one),
        ['十', one] => Some(10 + digit(one)?),
        [tens, '十'] => Some(digit(tens)? * 10),
        [tens, '十', one] => Some(digit(tens)? * 10 + digit(one)?),
        _ => None,
    }
}

fn take_cjk_markers(text: &str, info: &mut TitleInfo) -> String {
    let mut mark = |number: Option<u32>, kind: &str| {
        let Some(number) = number else { return };
        if matches!(kind, "部" | "章" | "クール" | "幕") {
            info.part.get_or_insert(number);
        } else {
            info.season.get_or_insert(number);
        }
    };
    let text = CJK_ORDINAL.replace_all(text, |caps: &Captures| {
        mark(chinese_number(&caps[1]), &caps[2]);
        " ～ "
    });
    let text = CJK_COUNTED.replace_all(&text, |caps: &Captures| {
        mark(caps[1].parse().ok(), &caps[2]);
        " ～ "
    });
    let text = CJK_WORD.replace_all(&text, |caps: &Captures| {
        mark(caps[2].parse().ok(), "期");
        " ～ "
    });
    text.into_owned()
}

/// What a title says about its season; see [`TitleInfo`].
pub fn parse_title(title: &str) -> TitleInfo {
    let mut info = TitleInfo::default();
    let text: String = title.nfkc().collect::<String>();
    let text = text.replace('×', " x ").replace('&', " and ");
    let text = take_cjk_markers(&text, &mut info);
    let tokens = tokenize(&text);
    let mut used = vec![false; tokens.len()];

    // Markers that say so anywhere: "Season 2", "2nd Season", "The Final Season", "Part 2", "S2".
    for index in 0..tokens.len() {
        if used[index] {
            continue;
        }
        let word = tokens[index].lower.as_str();
        let next = tokens
            .get(index + 1)
            .filter(|_| !used[index + 1])
            .map(|token| token.lower.as_str());
        let previous = index
            .checked_sub(1)
            .filter(|before| !used[*before])
            .map(|before| tokens[before].lower.as_str());
        let season_word = SEASON_WORDS.contains(&word);
        let part_word = PART_WORDS.contains(&word);
        // "Series 2" (British seasons) only with a count; "Second Stage" only with an ordinal.
        if (season_word || part_word || word == "series")
            && let Some(number) = next.and_then(count)
        {
            if part_word {
                info.part.get_or_insert(number);
            } else {
                info.season.get_or_insert(number);
            }
            used[index] = true;
            used[index + 1] = true;
            continue;
        }
        if (season_word || part_word || word == "stage")
            && let Some(ordinal) = previous.and_then(ordinal)
        {
            match (ordinal, part_word) {
                (Ordinal::Nth(number), false) => {
                    info.season.get_or_insert(number);
                }
                (Ordinal::Nth(number), true) => {
                    info.part.get_or_insert(number);
                }
                (Ordinal::Final, false) => info.final_season = true,
                // "Final Part": the last cour, its number unknown.
                (Ordinal::Final, true) => continue,
            }
            used[index] = true;
            used[index - 1] = true;
            if index >= 2 && !used[index - 2] && tokens[index - 2].lower == "the" {
                used[index - 2] = true;
            }
            continue;
        }
        if let Some(number) = s_number(word) {
            info.season.get_or_insert(number);
            used[index] = true;
        }
    }

    // Bracketed years and versions: "(2019)", "(TV)", "[Dub]".
    let groups = tokens.iter().map(|token| token.group).max().unwrap_or(0);
    for group in 1..=groups {
        let members: Vec<usize> = (0..tokens.len())
            .filter(|&index| tokens[index].group == group && !used[index])
            .collect();
        if let [only] = members[..]
            && let Some(year) = year_of(&tokens[only].lower)
        {
            info.year.get_or_insert(year);
            used[only] = true;
        } else if !members.is_empty()
            && members
                .iter()
                .all(|&index| DECORATIONS.contains(&tokens[index].lower.as_str()))
        {
            for index in members {
                used[index] = true;
            }
        }
    }

    // A trailing "Dub" or year: "Naruto Shippuden Dub", "Hunter x Hunter 2011".
    let remaining =
        |used: &[bool]| -> Vec<usize> { (0..tokens.len()).filter(|&index| !used[index]).collect() };
    let left = remaining(&used);
    if let [.., last] = left[..]
        && left.len() >= 3
        && matches!(
            tokens[last].lower.as_str(),
            "dub" | "dubbed" | "sub" | "subbed"
        )
    {
        used[last] = true;
    }
    let left = remaining(&used);
    if let [.., last] = left[..]
        && left.len() >= 2
        && let Some(year) = year_of(&tokens[last].lower)
    {
        info.year.get_or_insert(year);
        used[last] = true;
    }

    // A roman numeral or small number ending the title or its main part, after other words:
    // "Overlord II", "Mushoku Tensei II: Isekai Ittara Honki Dasu", "Psycho-Pass 3".
    let left = remaining(&used);
    if info.season.is_none() {
        let boundary =
            |before: usize, after: usize| after > before + 1 || tokens[after].break_before;
        for (position, &index) in left.iter().enumerate() {
            let starts = position == 0 || boundary(left[position - 1], index);
            let ends = position + 1 == left.len() || boundary(index, left[position + 1]);
            if starts || !ends {
                continue;
            }
            let token = &tokens[index];
            if NUMBER_PREFIXES.contains(&tokens[left[position - 1]].lower.as_str()) {
                continue;
            }
            let roman = roman_value(&token.lower).filter(|_| {
                token.lower != "x"
                    && (token.lower.len() >= 2 || token.text.chars().all(char::is_uppercase))
            });
            let bare = (token.lower.len() <= 2 && token.lower.chars().all(|c| c.is_ascii_digit()))
                .then(|| token.lower.parse::<u32>().ok())
                .flatten()
                .filter(|number| (1..=20).contains(number));
            if let Some(number) = roman.or(bare) {
                info.season = Some(number);
                used[index] = true;
                break;
            }
        }
    }

    let mut kept: Vec<&Token> = (0..tokens.len())
        .filter(|&index| !used[index])
        .map(|index| &tokens[index])
        .collect();
    if kept.len() > 1 && kept[0].lower == "the" {
        kept.remove(0);
    }
    info.base = kept
        .iter()
        .map(|token| token.lower.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    info.name = kept
        .iter()
        .map(|token| token.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    info
}

fn compare_key(base: &str) -> String {
    base.replace(' ', "").replace("ou", "o").replace("uu", "u")
}

// ---- Episodes in file and stream names ----

/// What a file or stream name says about its episode.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EpisodeInfo {
    pub season: Option<u32>,
    pub episode: Option<f64>,
    /// The last episode of a file holding several ("S01E01-E02") or of a batch ("01-12").
    pub last_episode: Option<f64>,
    /// The name before the episode marker, as a title: its base names the show, and its season
    /// marker ("Title II - 03") counts when the episode marker has none.
    pub show: TitleInfo,
    /// What follows the episode marker, release tags left out ("The Return" in "E03 - The
    /// Return").
    pub rest: String,
    /// The number came from a weak hint (a number ending the name, "Title 03"), not a marker.
    pub weak: bool,
}

const VIDEO_EXTENSIONS: [&str; 10] = [
    "mkv", "mp4", "avi", "webm", "mov", "m4v", "wmv", "ts", "flv", "mpg",
];

/// Release tags that carry no episode: resolutions, codecs, audio, sources, versions.
static RELEASE_TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(?:\d{3,4}p|\d{3,4}x\d{3,4}|[48]k|uhd|fhd|hd|sd|x26[45]|h26[45]|hevc|avc|av1|vp9|10bits?|8bits?|hi10p?|aac\d*|ac3|eac3|ddp?\d*|dts|truehd|atmos|flac|opus|mp3|web|webdl|webrip|dl|rip|bd|bdrip|bluray|brrip|dvd|dvdrip|hdtv|remux|multi|dual|subs?|subbed|dubbed|dub|proper|repack|batch|complete|v\d)$",
    )
    .expect("valid pattern")
});
/// A bracketed group: release group, tags, a checksum, a batch range.
static BRACKETED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\[(【][^\])】]*[\])】]").expect("valid pattern"));
static BATCH_RANGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?:ep?\s*)?(\d{1,4})\s*[-~～]\s*(\d{1,4})\s*$").expect("valid pattern")
});
/// "S01-S04", "Seasons 1 - 4", "Season 1-4", "Season 1 to 3": several seasons. ("S2 - 03" and
/// "Season 2 - 03" are an episode.)
static SEASON_RANGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\bS\d{1,2}\s*[-~～]\s*S\d{1,2}\b|\bseasons\s*\d{1,2}\s*(?:[-~～&]|to|and)\s*\d{1,2}\b|\bseason\s*\d{1,2}(?:[-~～]| to | and | & )\d{1,2}\b",
    )
    .expect("valid pattern")
});
static SXXEYY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bS(\d{1,2})[ ._-]?E(\d{1,4})(?:[ ._-]?E?(\d{1,4}))?\b")
        .expect("valid pattern")
});
static NXNN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(\d{1,2})x(\d{2,3})\b").expect("valid pattern"));
static SEASON_EPISODE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:season|saison|temporada|staffel)\s*(\d{1,2})\b[\s,._:-]*(?:episode|épisode|episodio|folge|eps?|e)\.?\s*(\d{1,4}(?:\.\d)?)\b",
    )
    .expect("valid pattern")
});
static S_DASH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\bS(\d{1,2})\b[\s._]*(?:[-–][\s._]*)?(?:(?:episode|eps?)\.?\s*)?(\d{1,4}(?:\.\d)?)(?:v\d)?\b",
    )
    .expect("valid pattern")
});
static CJK_EPISODE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"第?\s*(\d{1,4})\s*(?:話|话|集|回|夜)").expect("valid pattern"));
static EPISODE_WORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:(?:episode|épisode|episodio|folge|eps?)\.?\s*|e)(\d{1,4}(?:\.\d)?)(?:v\d)?\b",
    )
    .expect("valid pattern")
});
static HASH_NUMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"#\s*(\d{1,4})\b").expect("valid pattern"));
static DASH_RANGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\s[-–]\s+(\d{1,4})\s*[-~～]\s*(\d{1,4})(?:\s|$)").expect("valid pattern")
});
static DASH_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\s[-–]\s+(\d{1,4}(?:\.\d)?)(?:v\d)?(?:\s|$)").expect("valid pattern")
});

/// A name with its extension, brackets and scene dots dealt with, and the batch range a bracket
/// gave ("[01-12]").
fn clean_release(name: &str) -> (String, Option<(f64, f64)>) {
    let mut text: String = name.nfkc().collect::<String>().trim().to_owned();
    if let Some((stem, extension)) = text.rsplit_once('.')
        && VIDEO_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
    {
        text = stem.to_owned();
    }
    text = text.replace('_', " ");
    // Scene names: "Show.Name.S01E02.1080p"; dots between digits stay ("12.5", "5.1").
    if !text.contains(' ') && text.contains('.') {
        let characters: Vec<char> = text.chars().collect();
        text = characters
            .iter()
            .enumerate()
            .map(|(index, &c)| {
                let digits = index > 0
                    && characters[index - 1].is_ascii_digit()
                    && characters.get(index + 1).is_some_and(char::is_ascii_digit);
                if c == '.' && !digits { ' ' } else { c }
            })
            .collect();
    }
    let mut batch = None;
    let text = BRACKETED.replace_all(&text, |caps: &Captures| {
        let inside = &caps[0][caps[0].char_indices().nth(1).map_or(0, |(at, _)| at)..];
        let inside = inside
            .char_indices()
            .last()
            .map_or(inside, |(at, _)| &inside[..at]);
        if let Some(range) = BATCH_RANGE.captures(inside) {
            if let (Ok(first), Ok(last)) = (range[1].parse(), range[2].parse()) {
                batch.get_or_insert((first, last));
            }
            return " ".to_owned();
        }
        // A season inside brackets belongs to the name: "Title (Season 2) - 03".
        let lower = inside.to_lowercase();
        if SEASON_WORDS.iter().any(|word| lower.contains(word))
            || lower
                .split_whitespace()
                .any(|word| s_number(word).is_some())
        {
            return format!(" {inside} ");
        }
        " ".to_owned()
    });
    (text.split_whitespace().collect::<Vec<_>>().join(" "), batch)
}

/// The marker found: where it is, and what it says.
struct Marker {
    start: usize,
    end: usize,
    season: Option<u32>,
    episode: f64,
    last: Option<f64>,
}

fn group<T: std::str::FromStr>(caps: &Captures, at: usize) -> Option<T> {
    caps.get(at).and_then(|found| found.as_str().parse().ok())
}

/// A marker from a match: `season`, `episode` and `last` are capture groups (0 for none).
fn found(caps: &Captures, season: usize, episode: usize, last: usize) -> Option<Marker> {
    let whole = caps.get(0)?;
    let first: f64 = group(caps, episode)?;
    Some(Marker {
        start: whole.start(),
        end: whole.end(),
        season: if season > 0 {
            group(caps, season)
        } else {
            None
        },
        last: if last > 0 { group(caps, last) } else { None }.filter(|last| *last > first),
        episode: first,
    })
}

fn marker(text: &str) -> Option<Marker> {
    // A four-digit number after a dash is more often a year than an episode.
    let not_year = |caps: &Captures, at: usize| {
        group::<f64>(caps, at).is_some_and(|value| !(1900.0..=2035.0).contains(&value))
    };
    if let Some(caps) = SXXEYY.captures(text) {
        return found(&caps, 1, 2, 3);
    }
    for pattern in [&*SEASON_EPISODE, &*NXNN] {
        if let Some(caps) = pattern.captures(text) {
            return found(&caps, 1, 2, 0);
        }
    }
    if let Some(caps) = S_DASH.captures_iter(text).find(|caps| not_year(caps, 2)) {
        return found(&caps, 1, 2, 0);
    }
    for pattern in [&*CJK_EPISODE, &*EPISODE_WORD, &*HASH_NUMBER] {
        if let Some(caps) = pattern.captures(text) {
            return found(&caps, 0, 1, 0);
        }
    }
    if let Some(caps) = DASH_RANGE.captures(text) {
        return found(&caps, 0, 1, 2);
    }
    let caps = DASH_NUMBER
        .captures_iter(text)
        .find(|caps| not_year(caps, 1))?;
    found(&caps, 0, 1, 0)
}

/// Words after an episode marker that say something: release tags dropped.
fn meaningful(text: &str) -> String {
    text.split(|c: char| c.is_whitespace() || matches!(c, '-' | '–' | '.' | ',' | ':'))
        .filter(|word| !word.is_empty() && !RELEASE_TAG.is_match(word))
        .collect::<Vec<_>>()
        .join(" ")
}

/// What a file or stream name says about its episode: "Show.S01E02.1080p.mkv",
/// "[SubsPlease] Frieren - 12 (1080p) [ABCD1234].mkv", "Show II - 03", "Show 2nd Season - 03",
/// "Show S2 - 03", "Season 2 Episode 5", "1x02", "Episode 12", "第3話", "Show 03".
pub fn parse_episode(name: &str) -> EpisodeInfo {
    let (text, batch) = clean_release(name);
    let pack = SEASON_RANGE.is_match(&text);
    let mut info = EpisodeInfo::default();
    if let Some(found) = marker(&text) {
        info.show = parse_title(&text[..found.start]);
        info.rest = meaningful(&text[found.end..]);
        info.season = found.season.or(info.show.season);
        info.episode = Some(found.episode);
        info.last_episode = found.last;
    } else {
        // No marker: a number ending the name, release tags aside ("Show 03 1080p"), or a name
        // that is only a number ("03").
        let words: Vec<&str> = text.split(' ').collect();
        let end = words
            .iter()
            .rposition(|word| !RELEASE_TAG.is_match(word))
            .map_or(0, |index| index + 1);
        // "Season 1", "Vol 2": not an episode.
        let counted = end >= 2 && {
            let before = words[end - 2].to_lowercase();
            SEASON_WORDS.contains(&before.as_str())
                || PART_WORDS.contains(&before.as_str())
                || NUMBER_PREFIXES.contains(&before.as_str())
                || before == "series"
        };
        let trailing = words[..end].last().filter(|word| {
            !counted
                && word.len() <= 4
                && word.chars().all(|c| c.is_ascii_digit())
                && year_of(word).is_none()
        });
        match trailing {
            Some(number) => {
                info.show = parse_title(&words[..end - 1].join(" "));
                info.episode = number.parse().ok();
                info.weak = true;
            }
            None => info.show = parse_title(&text),
        }
        info.season = info.show.season;
        if let (None, Some((first, last))) = (info.episode, batch) {
            info.episode = Some(first);
            info.last_episode = Some(last).filter(|last| *last > first);
            info.weak = false;
        }
    }
    if pack {
        info.season = None;
    }
    info
}

/// A file name's season and episode, when a marker names one: `(None, 12.0)` for
/// "Episode 12", `(Some(2), 3.0)` for "Show II - 03". A number merely ending the name ("Sleep 2",
/// "Movie 2") is no episode here; [`parse_episode`] reports it as a weak hint.
pub fn episode_of(name: &str) -> Option<(Option<u32>, f64)> {
    let info = parse_episode(name);
    info.episode
        .filter(|_| !info.weak)
        .map(|episode| (info.season, episode))
}

// ---- Streams for an episode ----

/// One way of numbering the episode a stream is wanted for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EpisodeNumber {
    /// `None`: counted across seasons (an absolute number), or a season nobody knows.
    pub season: Option<u32>,
    pub episode: u32,
}

/// Whether a stream's name or description names another episode than the one `wanted` numbers:
/// another season and episode, another season's pack, or another episode in every numbering.
/// Unclear names never count against a stream, and a line that fits clears it.
pub fn names_other_episode(text: &str, wanted: &[EpisodeNumber]) -> bool {
    if wanted.is_empty() {
        return false;
    }
    let mut other = false;
    for part in text.split(['\n', '·', '|']) {
        let info = parse_episode(part);
        if info.weak {
            continue;
        }
        match fits(&info, wanted) {
            Some(true) => return false,
            Some(false) => other = true,
            None => {}
        }
    }
    other
}

fn fits(info: &EpisodeInfo, wanted: &[EpisodeNumber]) -> Option<bool> {
    match (info.season, info.episode) {
        (season, Some(first)) => {
            let last = info.last_episode.unwrap_or(first);
            Some(wanted.iter().any(|number| {
                let episode = f64::from(number.episode);
                episode >= first - 0.01
                    && episode <= last + 0.01
                    && match (season, number.season) {
                        (Some(found), Some(asked)) => found == asked,
                        _ => true,
                    }
            }))
        }
        (Some(season), None) => {
            let seasons: Vec<u32> = wanted.iter().filter_map(|number| number.season).collect();
            (!seasons.is_empty()).then(|| seasons.contains(&season))
        }
        (None, None) => None,
    }
}

// ---- Episode order and names ----

/// What orders a title's episodes.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EpisodeOrder {
    pub season: Option<f64>,
    pub number: Option<f64>,
    /// Release date, Unix ms.
    pub date: Option<i64>,
}

/// Whether some episode of the list has a season (1 and up).
pub fn has_seasons(episodes: &[EpisodeOrder]) -> bool {
    episodes
        .iter()
        .any(|episode| episode.season.is_some_and(|season| season >= 1.0))
}

/// Whether an episode is a special: season 0, or no season where others have one.
pub fn is_special(season: Option<f64>, seasons: bool) -> bool {
    match season {
        Some(season) => season < 1.0,
        None => seasons,
    }
}

/// The indices of `episodes` (in the source's order) in watching order: by season, specials
/// last, then by number when every episode has one, else by date when every one has one, else as
/// the source lists them, oldest first. A source whose numbered episodes rise lists oldest first;
/// otherwise it lists newest first, as Aniyomi's do.
pub fn watching_order(episodes: &[EpisodeOrder]) -> Vec<usize> {
    let seasons = has_seasons(episodes);
    let rank = |episode: &EpisodeOrder| -> (u8, f64) {
        match episode.season {
            _ if is_special(episode.season, seasons) => {
                (1 + u8::from(episode.season.is_none()), 0.0)
            }
            Some(season) => (0, season),
            None => (0, 0.0),
        }
    };
    let numbers: Vec<f64> = episodes
        .iter()
        .filter_map(|episode| episode.number)
        .collect();
    let rising = numbers.len() >= 2 && numbers[0] < numbers[numbers.len() - 1];
    let mut order: Vec<usize> = (0..episodes.len()).collect();
    if !rising {
        order.reverse();
    }
    let by = |a: f64, b: f64| a.partial_cmp(&b).unwrap_or(Ordering::Equal);
    let all_numbered = episodes.iter().all(|episode| episode.number.is_some());
    let all_dated = episodes.iter().all(|episode| episode.date.is_some());
    order.sort_by(|&a, &b| {
        let (first, second) = (&episodes[a], &episodes[b]);
        let (rank_a, season_a) = rank(first);
        let (rank_b, season_b) = rank(second);
        let within = if all_numbered {
            by(first.number.unwrap_or(0.0), second.number.unwrap_or(0.0))
        } else if all_dated {
            first.date.cmp(&second.date)
        } else {
            Ordering::Equal
        };
        rank_a
            .cmp(&rank_b)
            .then(by(season_a, season_b))
            .then(within)
    });
    order
}

/// "12" for 12.0, "12.5" otherwise.
pub fn format_number(number: f64) -> String {
    if number.fract() == 0.0 {
        format!("{number:.0}")
    } else {
        number.to_string()
    }
}

/// Whether an episode's name only repeats its number: "Episode 5", "Ep. 05", "S01E05", "5",
/// "第5話".
pub fn is_generic_name(name: &str, season: Option<f64>, number: f64) -> bool {
    let name = name.trim();
    if name.is_empty()
        || name
            .parse::<f64>()
            .is_ok_and(|value| (value - number).abs() < 0.01)
    {
        return true;
    }
    let info = parse_episode(name);
    !info.weak
        && info
            .episode
            .is_some_and(|episode| (episode - number).abs() < 0.01)
        && info.show.base.is_empty()
        && info.rest.is_empty()
        && match (info.season, season) {
            (Some(found), Some(asked)) => (f64::from(found) - asked).abs() < 0.01,
            _ => true,
        }
}

/// What an episode is called in lists and the player: "S1 E2 · Name", "E2 · Name", or the name
/// alone; a name that only repeats the number ("Episode 2") isn't repeated after it.
pub fn episode_label(season: Option<f64>, number: Option<f64>, name: &str) -> String {
    let name = name.trim();
    let Some(number) = number else {
        return name.to_owned();
    };
    let episode = format_number(number);
    let generic = is_generic_name(name, season, number);
    match season {
        Some(season) if generic => format!("S{} E{episode}", format_number(season)),
        Some(season) => format!("S{} E{episode} · {name}", format_number(season)),
        // "Episode 5" reads well as it is.
        None if name.ends_with(&format!(" {episode}")) => name.to_owned(),
        None if generic => format!("E{episode}"),
        None => format!("E{episode} · {name}"),
    }
}
