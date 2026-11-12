//! Choosing the voice books are read with, from every voice there is: a downloaded one reads at
//! once; one that isn't downloaded is downloaded first (with the voice engine when that isn't
//! here yet) and then reads, unless another voice was chosen meanwhile.

use std::sync::Arc;

use raygun_ai::tts::select;
use raygun_model::{AiModelState, ReadAloudEngine, ReadAloudPrefs, SettingChange, TtsVoice};
use tauri::Manager;
use tauri_specta::Event;

use super::ReadAloud;
use crate::ai::{AiService, ModelDownloadError};
use crate::startup::AppDb;

/// A voice chosen before it was downloaded: it reads once its model is here, while the voices
/// chosen are still as they were when it was chosen.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Pending {
    voice: TtsVoice,
    before: (ReadAloudEngine, std::collections::BTreeMap<String, String>),
}

/// `prefs` with `voice` reading its language (and its engine reading books).
fn with_voice(prefs: &ReadAloudPrefs, voice: &TtsVoice) -> ReadAloudPrefs {
    let mut prefs = prefs.clone();
    prefs.engine = voice.engine;
    prefs
        .voices
        .insert(select::primary(&voice.language), voice.key.clone());
    prefs
}

impl Pending {
    fn of(voice: TtsVoice, prefs: &ReadAloudPrefs) -> Self {
        Self {
            voice,
            before: (prefs.engine, prefs.voices.clone()),
        }
    }

    /// Whether nothing chose another voice since (settings, the reader, another choice).
    fn still_wanted(&self, prefs: &ReadAloudPrefs) -> bool {
        self.before == (prefs.engine, prefs.voices.clone())
    }
}

impl ReadAloud {
    /// Reads books in `key`'s language with that voice from now on (none: the device's own
    /// voices), downloading it first when it isn't here. How it ended: `installed` (chosen),
    /// `downloading` (its model is already on its way: it's chosen when that ends), or
    /// `cancelled`.
    pub(super) async fn use_voice(
        self: &Arc<Self>,
        key: Option<String>,
    ) -> Result<AiModelState, String> {
        let prefs = self.settings().await.read_aloud;
        let Some(key) = key else {
            *self.pending.lock().unwrap_or_else(|p| p.into_inner()) = None;
            let device = ReadAloudPrefs {
                engine: ReadAloudEngine::System,
                ..prefs
            };
            self.save_prefs(device).await?;
            return Ok(AiModelState::Installed);
        };
        let voice = super::voice_list()
            .voices
            .into_iter()
            .find(|voice| voice.key == key)
            .ok_or_else(|| format!("There is no voice {key}"))?;
        let ai = self.app.state::<AiService>();
        let runtime = raygun_ai::models::runtime_for(raygun_ai::tts::RUNTIME_ENGINE)
            .ok_or("Downloaded voices don't run on this device")?;
        let model_here = ai.installed_dir(&voice.model_id).is_some();
        let runtime_here = ai.installed_dir(&runtime.id).is_some();
        if model_here && runtime_here {
            *self.pending.lock().unwrap_or_else(|p| p.into_inner()) = None;
            self.save_prefs(with_voice(&prefs, &voice)).await?;
            return Ok(AiModelState::Installed);
        }
        // The voice's model (which brings the engine with it), or only the engine.
        let fetched = if model_here {
            runtime.id.clone()
        } else {
            voice.model_id.clone()
        };
        *self.pending.lock().unwrap_or_else(|p| p.into_inner()) = Some(Pending::of(voice, &prefs));
        if ai.is_downloading(&fetched) {
            return Ok(AiModelState::Downloading);
        }
        let downloaded = ai.download_model(&fetched).await;
        let pending = {
            let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
            // The choice waiting for this download (a later one may wait for another).
            let waits_here = pending
                .as_ref()
                .is_some_and(|pending| pending.voice.model_id == fetched || fetched == runtime.id);
            if waits_here { pending.take() } else { None }
        };
        match downloaded {
            Ok(()) => {
                if let Some(pending) = pending {
                    let now = self.settings().await.read_aloud;
                    let here = ai.installed_dir(&pending.voice.model_id).is_some();
                    if here && pending.still_wanted(&now) {
                        self.save_prefs(with_voice(&now, &pending.voice)).await?;
                    }
                }
                Ok(AiModelState::Installed)
            }
            Err(ModelDownloadError::Cancelled) => Ok(AiModelState::Cancelled),
            Err(error) => Err(error.into()),
        }
    }

    /// Saves reading aloud's settings and puts them into effect (the reading goes on with them).
    async fn save_prefs(&self, prefs: ReadAloudPrefs) -> Result<(), String> {
        let db = self.app.state::<AppDb>().0.clone();
        let settings = crate::settings::update(&db, SettingChange::ReadAloud(prefs))
            .await
            .map_err(|error| error.to_string())?;
        crate::settings::apply_live(&self.app, &settings, Some(&["readAloud"])).await;
        let _ = crate::events::SettingsChangedEvent("readAloud".into()).emit(&self.app);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voice(key: &str, language: &str, engine: ReadAloudEngine) -> TtsVoice {
        TtsVoice {
            key: key.into(),
            model_id: key.split('/').next().unwrap_or_default().into(),
            engine,
            name: key.into(),
            language: language.into(),
            gender: None,
        }
    }

    #[test]
    fn a_chosen_voice_reads_its_language_with_its_engine() {
        let prefs = ReadAloudPrefs {
            voices: [("de".to_owned(), "piper-de/thorsten".to_owned())].into(),
            ..ReadAloudPrefs::default()
        };
        let heart = voice("kokoro/af_heart", "en-US", ReadAloudEngine::Kokoro);
        let chosen = with_voice(&prefs, &heart);
        assert_eq!(chosen.engine, ReadAloudEngine::Kokoro);
        assert_eq!(
            chosen.voices.get("en").map(String::as_str),
            Some("kokoro/af_heart")
        );
        assert_eq!(
            chosen.voices.get("de").map(String::as_str),
            Some("piper-de/thorsten"),
            "other languages keep theirs"
        );
    }

    #[test]
    fn a_voice_waiting_for_its_download_gives_way_to_later_choices() {
        let prefs = ReadAloudPrefs::default();
        let heart = voice("kokoro/af_heart", "en-US", ReadAloudEngine::Kokoro);
        let pending = Pending::of(heart.clone(), &prefs);
        assert!(pending.still_wanted(&prefs));
        // Speed or volume changed meanwhile: still chosen.
        let louder = ReadAloudPrefs {
            volume: 50,
            ..prefs.clone()
        };
        assert!(pending.still_wanted(&louder));
        // Another voice or engine chosen meanwhile: that one stays.
        let alba = voice("piper-gb/alba", "en-GB", ReadAloudEngine::Piper);
        assert!(!pending.still_wanted(&with_voice(&prefs, &alba)));
        let piper = ReadAloudPrefs {
            engine: ReadAloudEngine::Piper,
            ..prefs
        };
        assert!(!pending.still_wanted(&piper));
    }
}
