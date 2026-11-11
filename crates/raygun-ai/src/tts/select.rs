//! Which voice reads a book: the one chosen for its language, else a downloaded voice of the
//! chosen engine that speaks it.

use crate::models::{ModelInfo, VoiceInfo};

/// A voice of a downloaded model.
#[derive(Debug, Clone, Copy)]
pub struct Candidate<'a> {
    pub model: &'a ModelInfo,
    pub voice: &'a VoiceInfo,
}

impl Candidate<'_> {
    /// `<model id>/<voice id>`, as settings store a chosen voice.
    pub fn key(&self) -> String {
        voice_key(&self.model.id, &self.voice.id)
    }

    /// `piper` or `kokoro`.
    pub fn engine(&self) -> &str {
        self.model.engine.as_deref().unwrap_or_default()
    }
}

pub fn voice_key(model: &str, voice: &str) -> String {
    format!("{model}/{voice}")
}

/// Every voice of the given (downloaded) models, in catalogue order.
pub fn candidates(models: &[ModelInfo]) -> Vec<Candidate<'_>> {
    models
        .iter()
        .filter(|model| model.kind == "tts")
        .flat_map(|model| {
            model
                .tts
                .iter()
                .flat_map(|setup| &setup.voices)
                .map(move |voice| Candidate { model, voice })
        })
        .collect()
}

/// A language tag's primary language, lower case, with Norwegian's tags as one (`nb`).
pub fn primary(tag: &str) -> String {
    let primary = tag
        .split(['-', '_'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    match primary.as_str() {
        "no" | "nn" => "nb".to_owned(),
        _ => primary,
    }
}

/// The voice `engine` reads `language` with: `chosen` (the key settings keep for the language)
/// when it's one of `voices` and speaks it, else the first voice of the engine that does, else
/// nothing (the device's own voices read it then). Without a language, `fallback` (the voice
/// chosen for the app's language, say) or the engine's first voice.
pub fn pick<'a>(
    engine: &str,
    voices: &[Candidate<'a>],
    language: Option<&str>,
    chosen: Option<&str>,
    fallback: Option<&str>,
) -> Option<Candidate<'a>> {
    let of_engine = || voices.iter().filter(|voice| voice.engine() == engine);
    let by_key = |key: Option<&str>| {
        key.and_then(|key| of_engine().find(|voice| voice.key() == key))
            .copied()
    };
    let Some(language) = language else {
        return by_key(fallback).or_else(|| of_engine().next().copied());
    };
    let wanted = primary(language);
    let speaks = |voice: &&Candidate<'_>| primary(&voice.voice.language) == wanted;
    by_key(chosen)
        .filter(|voice| primary(&voice.voice.language) == wanted)
        .or_else(|| {
            // A voice of the same region first (`en-GB` for a British book), then any.
            of_engine()
                .filter(speaks)
                .find(|voice| voice.voice.language.eq_ignore_ascii_case(language))
                .or_else(|| of_engine().find(speaks))
                .copied()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{TtsSetup, catalog};

    fn voice(id: &str, language: &str) -> VoiceInfo {
        VoiceInfo {
            id: id.into(),
            name: id.into(),
            language: language.into(),
            gender: None,
            speaker: 0,
            lang: None,
        }
    }

    fn model(id: &str, engine: &str, voices: Vec<VoiceInfo>) -> ModelInfo {
        let mut model = catalog()
            .into_iter()
            .find(|model| model.kind == "tts")
            .unwrap();
        model.id = id.into();
        model.engine = Some(engine.into());
        model.tts = Some(TtsSetup {
            voices,
            ..TtsSetup::default()
        });
        model
    }

    #[test]
    fn picks_the_chosen_voice_then_one_that_speaks_the_language() {
        let models = vec![
            model("piper-us", "piper", vec![voice("amy", "en-US")]),
            model("piper-gb", "piper", vec![voice("alba", "en-GB")]),
            model("piper-de", "piper", vec![voice("thorsten", "de-DE")]),
            model(
                "kokoro",
                "kokoro",
                vec![voice("af_heart", "en-US"), voice("ef_dora", "es")],
            ),
        ];
        let voices = candidates(&models);
        let pick_key = |engine, language, chosen| {
            pick(engine, &voices, language, chosen, None).map(|voice| voice.key())
        };
        assert_eq!(
            pick_key("piper", Some("en"), Some("piper-gb/alba")).as_deref(),
            Some("piper-gb/alba")
        );
        // A chosen voice for another language (or engine) doesn't read this one.
        assert_eq!(
            pick_key("piper", Some("de"), Some("piper-gb/alba")).as_deref(),
            Some("piper-de/thorsten")
        );
        assert_eq!(
            pick_key("kokoro", Some("en"), Some("piper-gb/alba")).as_deref(),
            Some("kokoro/af_heart")
        );
        // The book's region first.
        assert_eq!(
            pick_key("piper", Some("en-GB"), None).as_deref(),
            Some("piper-gb/alba")
        );
        assert_eq!(
            pick_key("piper", Some("en"), None).as_deref(),
            Some("piper-us/amy")
        );
        // Nobody speaks it: the device's voices read it.
        assert_eq!(pick_key("piper", Some("es"), None), None);
        assert_eq!(pick_key("system", Some("en"), None), None);
        // Without a language: the fallback, else the engine's first voice.
        assert_eq!(
            pick("kokoro", &voices, None, None, Some("kokoro/ef_dora")).map(|voice| voice.key()),
            Some("kokoro/ef_dora".into())
        );
        assert_eq!(
            pick("piper", &voices, None, None, None).map(|voice| voice.key()),
            Some("piper-us/amy".into())
        );
    }

    #[test]
    fn treats_norwegian_tags_as_one() {
        assert_eq!(primary("no"), "nb");
        assert_eq!(primary("nb-NO"), "nb");
        assert_eq!(primary("pt_BR"), "pt");
    }
}
