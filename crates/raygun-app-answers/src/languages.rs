//! Languages by the names people type (`French`, `français`), for "translate … to …".

const LANGUAGES: &[(&str, &[&str])] = &[
    ("en", &["english"]),
    ("fr", &["french", "français", "francais"]),
    ("de", &["german", "deutsch"]),
    ("es", &["spanish", "español", "espanol", "castilian"]),
    ("it", &["italian", "italiano"]),
    ("pt", &["portuguese", "português", "portugues"]),
    ("nl", &["dutch", "nederlands", "flemish"]),
    ("sv", &["swedish", "svenska"]),
    ("nb", &["norwegian", "norsk", "bokmål", "bokmal"]),
    ("da", &["danish", "dansk"]),
    ("fi", &["finnish", "suomi"]),
    ("is", &["icelandic", "íslenska"]),
    ("pl", &["polish", "polski"]),
    ("cs", &["czech", "čeština", "cestina"]),
    ("sk", &["slovak", "slovenčina"]),
    ("sl", &["slovenian", "slovene", "slovenščina"]),
    ("hr", &["croatian", "hrvatski"]),
    ("sr", &["serbian", "srpski", "српски"]),
    ("bs", &["bosnian"]),
    ("hu", &["hungarian", "magyar"]),
    ("ro", &["romanian", "română", "romana"]),
    ("bg", &["bulgarian", "български"]),
    ("ru", &["russian", "русский"]),
    ("uk", &["ukrainian", "українська"]),
    ("be", &["belarusian"]),
    ("el", &["greek", "ελληνικά"]),
    ("tr", &["turkish", "türkçe", "turkce"]),
    ("ar", &["arabic", "العربية"]),
    ("he", &["hebrew", "עברית"]),
    ("fa", &["persian", "farsi", "فارسی"]),
    ("ur", &["urdu"]),
    ("hi", &["hindi", "हिन्दी", "हिंदी"]),
    ("bn", &["bengali", "bangla"]),
    ("pa", &["punjabi"]),
    ("ta", &["tamil"]),
    ("te", &["telugu"]),
    ("mr", &["marathi"]),
    ("gu", &["gujarati"]),
    ("kn", &["kannada"]),
    ("ml", &["malayalam"]),
    (
        "zh",
        &["chinese", "mandarin", "中文", "汉语", "simplified chinese"],
    ),
    ("zh-Hant", &["traditional chinese"]),
    ("yue", &["cantonese"]),
    ("ja", &["japanese", "日本語"]),
    ("ko", &["korean", "한국어"]),
    ("vi", &["vietnamese", "tiếng việt"]),
    ("th", &["thai", "ไทย"]),
    ("id", &["indonesian", "bahasa indonesia"]),
    ("ms", &["malay", "bahasa melayu"]),
    ("fil", &["filipino", "tagalog"]),
    ("sw", &["swahili", "kiswahili"]),
    ("af", &["afrikaans"]),
    ("ga", &["irish", "gaeilge"]),
    ("cy", &["welsh", "cymraeg"]),
    ("gd", &["scottish gaelic", "gaelic"]),
    ("eu", &["basque", "euskara"]),
    ("ca", &["catalan", "català", "catala"]),
    ("gl", &["galician", "galego"]),
    ("et", &["estonian", "eesti"]),
    ("lv", &["latvian", "latviešu"]),
    ("lt", &["lithuanian", "lietuvių"]),
    ("sq", &["albanian", "shqip"]),
    ("mk", &["macedonian"]),
    ("hy", &["armenian"]),
    ("ka", &["georgian"]),
    ("az", &["azerbaijani", "azeri"]),
    ("kk", &["kazakh"]),
    ("uz", &["uzbek"]),
    ("mn", &["mongolian"]),
    ("ne", &["nepali"]),
    ("si", &["sinhala", "sinhalese"]),
    ("km", &["khmer"]),
    ("lo", &["lao"]),
    ("my", &["burmese"]),
    ("am", &["amharic"]),
    ("so", &["somali"]),
    ("yo", &["yoruba"]),
    ("ig", &["igbo"]),
    ("ha", &["hausa"]),
    ("zu", &["zulu"]),
    ("xh", &["xhosa"]),
    ("eo", &["esperanto"]),
    ("la", &["latin"]),
    ("mi", &["maori", "māori"]),
    ("haw", &["hawaiian"]),
];

/// The BCP 47 tag of a language named in English or in itself.
pub fn tag_of(name: &str) -> Option<&'static str> {
    let name = name.trim().trim_end_matches(['.', '?', '!']).to_lowercase();
    let name = name.strip_suffix(" language").unwrap_or(&name);
    LANGUAGES
        .iter()
        .find(|(_, names)| names.contains(&name))
        .map(|(tag, _)| *tag)
}

/// A language's English name, from its tag.
pub fn name_of(tag: &str) -> Option<String> {
    let primary = tag.split(['-', '_']).next().unwrap_or(tag).to_lowercase();
    LANGUAGES
        .iter()
        .find(|(known, _)| known.eq_ignore_ascii_case(tag) || known.eq_ignore_ascii_case(&primary))
        .map(|(_, names)| super::text::capitalized(names[0]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_are_named() {
        assert_eq!(tag_of("French"), Some("fr"));
        assert_eq!(tag_of("français"), Some("fr"));
        assert_eq!(tag_of("the japanese language"), None);
        assert_eq!(tag_of("Japanese language"), Some("ja"));
        assert_eq!(tag_of("Paris"), None);
        assert_eq!(name_of("pt-BR").as_deref(), Some("Portuguese"));
    }
}
