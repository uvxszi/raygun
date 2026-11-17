//! Words for Search's answers (`raygun_app_answers::words`): the book reader's dictionaries and
//! the translators, as the answers crate reaches them.

use std::sync::Arc;

use raygun_app_answers::words::{Looking, Translating, Translation, WordSources};
use raygun_model::DictionaryRequest;
use tauri::{AppHandle, Manager};

use crate::lyrics_translation::{AI_PREFIX, DEVICE, LyricsTranslations};

pub struct AppWords(pub AppHandle);

impl WordSources for AppWords {
    fn lookup(&self, request: DictionaryRequest) -> Looking<'_> {
        Box::pin(async move {
            let dictionary = self
                .0
                .try_state::<Arc<crate::dictionary::DictionaryService>>()?;
            Some(dictionary.lookup(request).await)
        })
    }

    fn translate<'a>(&'a self, text: &'a str, to: &'a str) -> Translating<'a> {
        Box::pin(translate(&self.0, text, to))
    }
}

async fn translate(app: &AppHandle, text: &str, to: &str) -> Result<Option<Translation>, String> {
    let Some(translations) = app.try_state::<Arc<LyricsTranslations>>() else {
        return Ok(None);
    };
    let options = translations.translators(&[text], to).await?;
    let ready = options.iter().find(|option| {
        option.supported
            && ((option.id == DEVICE && option.download_bytes == 0.0 && !option.models.is_empty())
                || option.id.starts_with(AI_PREFIX))
    });
    let Some(chosen) = ready else {
        return Ok(None);
    };
    let lines = [text.to_owned()];
    let (name, translated) = if chosen.id == DEVICE {
        let done = translations
            .translate_on_device(to, &lines, |_, _| {})
            .await?;
        (String::new(), done)
    } else {
        let provider = chosen.id.trim_start_matches(AI_PREFIX);
        app.state::<crate::ai::AiService>()
            .translate_texts_from(
                provider,
                &lines,
                None,
                to,
                raygun_ai::translate::Content::Text,
            )
            .await?
    };
    Ok(Some(Translation {
        chosen_name: chosen.name.clone(),
        on_device: chosen.id == DEVICE,
        name,
        lines: translated,
    }))
}
