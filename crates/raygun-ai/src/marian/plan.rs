//! Which OPUS-MT models translate into a language on this device (M46.48): the catalogue says
//! what each model takes and gives (`ModelInfo::translate`); English is the bridge, so lyrics in
//! any language reach German as many → English, then English → German.

use crate::models::ModelInfo;

/// One model's turn: its catalogue id, and the `>>code<<` token naming its output language when
/// it writes several.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub model: String,
    pub target_token: Option<String>,
}

/// The models that bring lyrics into `target`: from any language (`steps`), and from English
/// (`from_english`, for lyrics already in English). `None` when no model in the catalogue writes
/// `target`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub steps: Vec<Step>,
    pub from_english: Vec<Step>,
}

impl Plan {
    /// Every model either route uses.
    pub fn models(&self) -> Vec<String> {
        let mut models: Vec<String> = Vec::new();
        for step in self.steps.iter().chain(&self.from_english) {
            if !models.contains(&step.model) {
                models.push(step.model.clone());
            }
        }
        models
    }
}

/// A language tag's base language and script, the way translations are matched: `zh-TW` is
/// Chinese in Traditional characters, `zh` and `zh-CN` in Simplified.
pub fn base_and_script(tag: &str) -> (String, Option<String>) {
    let mut parts = tag.split(['-', '_']);
    let base = parts.next().unwrap_or_default().to_ascii_lowercase();
    let mut script = None;
    let mut region = None;
    for part in parts {
        if part.len() == 4 && part.chars().all(|c| c.is_ascii_alphabetic()) {
            script = Some(format!(
                "{}{}",
                part[..1].to_ascii_uppercase(),
                part[1..].to_ascii_lowercase()
            ));
        } else if region.is_none() {
            region = Some(part.to_ascii_uppercase());
        }
    }
    if base == "zh" && script.is_none() {
        let traditional = matches!(region.as_deref(), Some("TW" | "HK" | "MO"));
        script = Some(if traditional { "Hant" } else { "Hans" }.to_owned());
    }
    (base, script)
}

/// Whether text in `written` reads as `wanted`: the same base language, and for Chinese the same
/// script.
pub fn same_language(written: &str, wanted: &str) -> bool {
    let (written_base, written_script) = base_and_script(written);
    let (wanted_base, wanted_script) = base_and_script(wanted);
    if written_base != wanted_base {
        return false;
    }
    written_base != "zh" || written_script == wanted_script
}

/// The token a many-language model needs to write `target`, from its catalogue entry.
fn target_token(model: &ModelInfo, target: &str) -> Option<Option<String>> {
    let pair = model.translate.as_ref()?;
    if let Some(code) = pair
        .targets
        .iter()
        .find(|(tag, _)| same_language(tag, target))
        .map(|(_, code)| code)
    {
        return Some(Some(format!(">>{code}<<")));
    }
    pair.to
        .iter()
        .any(|to| same_language(to, target))
        .then_some(None)
}

fn takes(model: &ModelInfo, from: &str) -> bool {
    model
        .translate
        .as_ref()
        .is_some_and(|pair| pair.from.iter().any(|tag| tag == from))
}

/// How `catalog` translates into `target`. `preferred` (a catalogue id) wins among models that
/// write `target`; dedicated pairs beat many-language ones otherwise.
pub fn plan(catalog: &[ModelInfo], target: &str, preferred: Option<&str>) -> Option<Plan> {
    let writes = |from: &str| {
        let mut fitting: Vec<(&ModelInfo, Option<String>)> = catalog
            .iter()
            .filter(|model| takes(model, from))
            .filter_map(|model| target_token(model, target).map(|token| (model, token)))
            .collect();
        // The preferred model first, then those that write the fewest languages.
        fitting.sort_by_key(|(model, _)| {
            let languages = model
                .translate
                .as_ref()
                .map_or(usize::MAX, |pair| pair.to.len() + pair.targets.len());
            (
                Some(model.id.as_str()) != preferred,
                languages,
                model.id.clone(),
            )
        });
        fitting
            .into_iter()
            .next()
            .map(|(model, target_token)| Step {
                model: model.id.clone(),
                target_token,
            })
    };
    let (base, _) = base_and_script(target);
    if base == "en" {
        let step = writes("mul")?;
        return Some(Plan {
            steps: vec![step],
            from_english: Vec::new(),
        });
    }
    let last = writes("en")?;
    let first = writes_english(catalog)?;
    Some(Plan {
        steps: vec![first, last.clone()],
        from_english: vec![last],
    })
}

fn writes_english(catalog: &[ModelInfo]) -> Option<Step> {
    catalog
        .iter()
        .filter(|model| takes(model, "mul"))
        .find(|model| target_token(model, "en").is_some())
        .map(|model| Step {
            model: model.id.clone(),
            target_token: None,
        })
}

/// Words that show a line is English and are rare in other languages' lyrics ("was", "will",
/// "come" and "for" aren't: German, Italian and Norwegian have them).
const ENGLISH_WORDS: &[&str] = &[
    "the", "and", "you", "your", "you're", "i'm", "i'll", "i've", "don't", "can't", "won't",
    "it's", "that", "this", "with", "what", "when", "where", "why", "how", "love", "baby", "just",
    "know", "like", "never", "be", "my", "of", "to", "it", "they", "she", "his", "but", "out",
    "up", "down", "now", "there", "here", "gonna", "wanna", "all", "are", "been", "have", "got",
    "get", "feel", "heart", "night", "tonight", "want", "need", "say", "said", "go", "take",
    "make", "every", "time", "way", "yeah", "think", "tell", "only", "can", "would", "could",
    "from", "our", "us", "them",
];

/// Whether lyrics are English: Latin letters only, and a quarter of their words common English
/// ones.
pub fn looks_english(lines: &[&str]) -> bool {
    let mut latin = 0usize;
    let mut other = 0usize;
    let mut words = 0usize;
    let mut english = 0usize;
    for line in lines {
        for character in line.chars().filter(|c| c.is_alphabetic()) {
            if character.is_ascii_alphabetic() {
                latin += 1;
            } else {
                other += 1;
            }
        }
        for word in line
            .split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '’'))
            .filter(|word| !word.is_empty())
        {
            words += 1;
            let word = word.to_lowercase().replace('’', "'");
            if ENGLISH_WORDS.contains(&word.as_str()) {
                english += 1;
            }
        }
    }
    // Accented letters are fine (café, naïve) in small numbers; other scripts aren't.
    words > 0 && other * 20 <= latin && english * 4 >= words
}

/// Whether one line has letters outside Latin's basic set, so it can't be English.
pub fn has_other_letters(line: &str) -> bool {
    line.chars()
        .filter(|c| c.is_alphabetic() && !c.is_ascii_alphabetic())
        .count()
        * 4
        > line.chars().filter(char::is_ascii_alphabetic).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::catalog;

    #[test]
    fn language_tags_match_by_base_and_chinese_script() {
        assert!(same_language("en", "en-GB"));
        assert!(same_language("pt", "pt-BR"));
        assert!(same_language("zh-Hans", "zh-CN"));
        assert!(same_language("zh-Hans", "zh"));
        assert!(!same_language("zh-Hans", "zh-TW"));
        assert!(same_language("zh-Hant", "zh-TW"));
        assert!(!same_language("ja", "ko"));
        assert_eq!(
            base_and_script("ja-Latn"),
            ("ja".into(), Some("Latn".into()))
        );
    }

    #[test]
    fn plans_go_through_english() {
        let models = catalog();
        let english = plan(&models, "en-US", None).unwrap();
        assert_eq!(english.steps.len(), 1);
        assert_eq!(english.steps[0].model, "opus-mt-mul-en");
        assert!(english.from_english.is_empty());

        // A dedicated pair beats the many-language model.
        let german = plan(&models, "de", None).unwrap();
        let ids: Vec<&str> = german
            .steps
            .iter()
            .map(|step| step.model.as_str())
            .collect();
        assert_eq!(ids, ["opus-mt-mul-en", "opus-mt-en-de"]);
        assert_eq!(german.from_english[0].model, "opus-mt-en-de");
        assert_eq!(german.from_english[0].target_token, None);

        // Chinese writes Traditional characters for Taiwan.
        let taiwan = plan(&models, "zh-TW", None).unwrap();
        assert_eq!(taiwan.from_english[0].model, "opus-mt-en-zh");
        assert_eq!(
            taiwan.from_english[0].target_token.as_deref(),
            Some(">>cmn_Hant<<")
        );

        // Japanese only through the many-language model, with its token.
        let japanese = plan(&models, "ja", None).unwrap();
        assert_eq!(japanese.from_english[0].model, "opus-mt-en-mul");
        assert_eq!(
            japanese.from_english[0].target_token.as_deref(),
            Some(">>jpn<<")
        );
        // A preferred model wins when it writes the language.
        let preferred = plan(&models, "de", Some("opus-mt-en-mul")).unwrap();
        assert_eq!(preferred.from_english[0].model, "opus-mt-en-mul");
        assert_eq!(
            preferred.from_english[0].target_token.as_deref(),
            Some(">>deu<<")
        );
        assert_eq!(
            preferred.models(),
            ["opus-mt-mul-en".to_owned(), "opus-mt-en-mul".to_owned()]
        );
        assert!(plan(&models, "tlh", None).is_none());
    }

    #[test]
    fn english_lyrics_are_told_apart() {
        assert!(looks_english(&[
            "I'm in love with the shape of you",
            "We push and pull like a magnet do",
        ]));
        assert!(!looks_english(&[
            "沈むように溶けてゆくように",
            "二人だけの空が広がる夜に"
        ]));
        assert!(!looks_english(&[
            "Despacito, quiero respirar tu cuello despacito",
            "Deja que te diga cosas al oído",
        ]));
        assert!(!looks_english(&[
            "Ich will, ich will",
            "Ihr seht mich, ihr hört mich"
        ]));
        assert!(!looks_english(&[]));
        assert!(has_other_letters("사랑해 baby"));
        assert!(!has_other_letters("Je t'aime, café"));
    }
}
