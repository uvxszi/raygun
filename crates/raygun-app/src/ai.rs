//! AI settings (M34): translators with their keys in the encrypted secrets store, and the models
//! the model manager can download. Both are device-local; keys never leave this module except as
//! a four-character hint.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use base64::Engine;
use raygun_ai::models::{self, ModelInfo};
use raygun_ai::secrets::{self, Vault};
use raygun_ai::translate::{self, Provider, ProviderKind, TranslateError};
use raygun_model::{
    AiModel, AiModelProgress, AiModelState, AiProvider, AiProviderInput, AiProviderKind,
};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::events::AiModelEvent;
use crate::startup::AppDb;

/// `local_meta` key of the secrets store's key (base64).
const SECRETS_KEY: &str = "ai.secretsKey";
/// `local_meta` key of the translator list (JSON).
const PROVIDERS: &str = "ai.providers";
/// What "Test" translates.
const SAMPLE: &str = "Good morning! Three new chapters are waiting in your library.";
/// Progress events are sent at most this often (in bytes received).
const PROGRESS_STEP: u64 = 256 * 1024;

pub struct AiService {
    app: AppHandle,
    client: reqwest::Client,
    models_dir: PathBuf,
    vault: Vault,
    /// Bytes received per model being downloaded.
    downloads: Arc<Mutex<HashMap<String, u64>>>,
    /// Serializes changes to the provider list.
    providers_lock: tokio::sync::Mutex<()>,
}

impl AiService {
    /// Opens the secrets store, creating its key on first use.
    pub async fn open(
        app: AppHandle,
        secrets_path: PathBuf,
        models_dir: PathBuf,
        client: reqwest::Client,
    ) -> Result<Self, String> {
        let db = app.state::<AppDb>().0.clone();
        let stored = raygun_db::local_value(&db, SECRETS_KEY)
            .await
            .map_err(|error| error.to_string())?
            .and_then(|text| {
                base64::engine::general_purpose::STANDARD
                    .decode(text)
                    .ok()?
                    .try_into()
                    .ok()
            });
        let key: [u8; secrets::KEY_LEN] = match stored {
            Some(key) => key,
            None => {
                let key = secrets::new_key().map_err(|error| error.to_string())?;
                let text = base64::engine::general_purpose::STANDARD.encode(key);
                raygun_db::set_local_value(&db, SECRETS_KEY, &text)
                    .await
                    .map_err(|error| error.to_string())?;
                key
            }
        };
        let vault = tauri::async_runtime::spawn_blocking(move || Vault::open(&secrets_path, &key))
            .await
            .map_err(|error| error.to_string())?;
        Ok(Self {
            app,
            client,
            models_dir,
            vault,
            downloads: Arc::default(),
            providers_lock: tokio::sync::Mutex::new(()),
        })
    }

    // ---- Translators ----

    pub async fn providers(&self) -> Result<Vec<AiProvider>, String> {
        Ok(self
            .stored_providers()
            .await?
            .iter()
            .map(|provider| self.view(provider))
            .collect())
    }

    /// Adds or changes a provider. A `None` key keeps the stored one; an empty key removes it.
    pub async fn save_provider(&self, input: AiProviderInput) -> Result<AiProvider, String> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err("Give the translator a name".into());
        }
        let _guard = self.providers_lock.lock().await;
        let mut list = self.stored_providers().await?;
        let id = input
            .id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
        let provider = Provider {
            id: id.clone(),
            kind: to_kind(input.kind),
            name: name.to_owned(),
            base_url: trimmed(input.base_url),
            model: trimmed(input.model),
        };
        match list.iter_mut().find(|known| known.id == id) {
            Some(known) => *known = provider.clone(),
            None if input.id.is_some() => return Err("This translator was removed".into()),
            None => list.push(provider.clone()),
        }
        store_key(&self.vault, &id, input.key)?;
        self.store_providers(&list).await?;
        Ok(self.view(&provider))
    }

    pub async fn delete_provider(&self, id: &str) -> Result<(), String> {
        let _guard = self.providers_lock.lock().await;
        let mut list = self.stored_providers().await?;
        list.retain(|provider| provider.id != id);
        self.store_providers(&list).await?;
        self.vault
            .set(&secret_name(id), None)
            .map_err(|error| error.to_string())
    }

    /// Translates a sample sentence into Japanese with the provider, to check its settings.
    pub async fn test_provider(&self, id: &str) -> Result<String, String> {
        let provider = self
            .stored_providers()
            .await?
            .into_iter()
            .find(|provider| provider.id == id)
            .ok_or_else(|| "This translator was removed".to_owned())?;
        let key = self.vault.get(&secret_name(id));
        let texts = [SAMPLE.to_owned()];
        let result = translate::translate(
            &self.client,
            &provider,
            key.as_deref(),
            &texts,
            Some("en"),
            "ja",
        )
        .await;
        match result {
            Ok(mut translations) => Ok(translations.remove(0)),
            Err(error) => {
                tracing::info!(provider = %provider.name, %error, "translator test failed");
                Err(error.to_string())
            }
        }
    }

    /// Translates a song's lines into `to` with a provider, all in one request (DeepL takes them
    /// fifty at a time); a model that answers with the wrong number of lines gets one more try.
    /// Returns the provider's name with the lines (M46.48).
    pub async fn translate_lyrics(
        &self,
        id: &str,
        lines: &[String],
        to: &str,
    ) -> Result<(String, Vec<String>), String> {
        let provider = self
            .stored_providers()
            .await?
            .into_iter()
            .find(|provider| provider.id == id)
            .ok_or_else(|| "This translator was removed in Settings → AI".to_owned())?;
        let key = self.vault.get(&secret_name(id));
        let ask = || {
            translate::translate_as(
                &self.client,
                &provider,
                key.as_deref(),
                lines,
                None,
                to,
                translate::Content::Lyrics,
            )
        };
        let mut result = ask().await;
        if matches!(
            result,
            Err(TranslateError::CountMismatch { .. } | TranslateError::BadAnswer)
        ) && provider.kind != ProviderKind::Deepl
        {
            result = ask().await;
        }
        match result {
            Ok(translated) => Ok((provider.name.clone(), translated)),
            Err(error) => {
                tracing::info!(provider = %provider.name, %error, "lyrics translation failed");
                Err(error.to_string())
            }
        }
    }

    /// An OpenAI-compatible provider's address and key, for transcription (M46.32).
    pub async fn transcriber(&self, id: &str) -> Result<(String, Option<String>), String> {
        let provider = self
            .stored_providers()
            .await?
            .into_iter()
            .find(|provider| provider.id == id)
            .ok_or_else(|| "The transcription provider was removed".to_owned())?;
        if provider.kind != ProviderKind::OpenAiCompatible {
            return Err(format!(
                "{} can't transcribe; pick an OpenAI-compatible provider",
                provider.name
            ));
        }
        let base = provider
            .base_url
            .as_deref()
            .map(str::trim)
            .filter(|base| !base.is_empty())
            .unwrap_or(translate::OPENAI_BASE)
            .trim_end_matches('/')
            .to_owned();
        Ok((base, self.vault.get(&secret_name(id))))
    }

    fn view(&self, provider: &Provider) -> AiProvider {
        let secret = secret_name(&provider.id);
        AiProvider {
            id: provider.id.clone(),
            kind: from_kind(provider.kind),
            name: provider.name.clone(),
            base_url: provider.base_url.clone(),
            model: provider.model.clone(),
            has_key: self.vault.contains(&secret),
            key_hint: self.vault.hint(&secret),
        }
    }

    async fn stored_providers(&self) -> Result<Vec<Provider>, String> {
        let text = raygun_db::local_value(&self.db(), PROVIDERS)
            .await
            .map_err(|error| error.to_string())?;
        Ok(text
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default())
    }

    async fn store_providers(&self, list: &[Provider]) -> Result<(), String> {
        let text = serde_json::to_string(list).map_err(|error| error.to_string())?;
        raygun_db::set_local_value(&self.db(), PROVIDERS, &text)
            .await
            .map_err(|error| error.to_string())
    }

    // ---- Models ----

    pub fn models(&self) -> Vec<AiModel> {
        let downloads = self.downloads.lock().unwrap_or_else(|p| p.into_inner());
        models::catalog()
            .into_iter()
            .map(|model| AiModel {
                installed: models::is_installed(&self.models_dir, &model),
                downloading: downloads.get(&model.id).map(|received| *received as f64),
                size_bytes: model.size() as f64,
                id: model.id,
                name: model.name,
                kind: model.kind,
                description: model.description,
                license: model.license,
                license_url: model.license_url,
                source_url: model.source_url,
            })
            .collect()
    }

    /// The folder of an installed model (M46.21: the screenshot reader's), when it is installed.
    pub fn installed_dir(&self, id: &str) -> Option<std::path::PathBuf> {
        let model = models::find(id).ok()?;
        models::is_installed(&self.models_dir, &model)
            .then(|| models::model_dir(&self.models_dir, &model))
    }

    /// Downloads a model, reporting progress as `AiModelEvent`s; returns when it is installed.
    pub async fn download_model(&self, id: &str) -> Result<(), String> {
        let model = models::find(id).map_err(|error| error.to_string())?;
        {
            let mut downloads = self.downloads.lock().unwrap_or_else(|p| p.into_inner());
            if downloads.contains_key(id) {
                return Err("This model is already downloading".into());
            }
            downloads.insert(id.to_owned(), 0);
        }
        let total = model.size();
        let mut reported = 0u64;
        let result = models::download(&self.client, &self.models_dir, &model, |received, _| {
            if let Some(entry) = self
                .downloads
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .get_mut(id)
            {
                *entry = received;
            }
            if received == 0 || received >= reported + PROGRESS_STEP {
                reported = received;
                self.emit(&model, AiModelState::Downloading, received, None);
            }
        })
        .await;
        self.downloads
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(id);
        match result {
            Ok(()) => {
                tracing::info!(model = %id, bytes = total, "model installed");
                self.emit(&model, AiModelState::Installed, total, None);
                Ok(())
            }
            Err(error) => {
                tracing::warn!(model = %id, %error, "model download failed");
                self.emit(&model, AiModelState::Failed, 0, Some(error.to_string()));
                Err(error.to_string())
            }
        }
    }

    /// Whether a model is downloading now.
    pub fn is_downloading(&self, id: &str) -> bool {
        self.downloads
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains_key(id)
    }

    pub async fn delete_model(&self, id: &str) -> Result<(), String> {
        if self
            .downloads
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains_key(id)
        {
            return Err("This model is downloading".into());
        }
        let model = models::find(id).map_err(|error| error.to_string())?;
        models::delete(&self.models_dir, &model)
            .await
            .map_err(|error| error.to_string())
    }

    fn emit(&self, model: &ModelInfo, state: AiModelState, received: u64, error: Option<String>) {
        let _ = AiModelEvent(AiModelProgress {
            id: model.id.clone(),
            state,
            received_bytes: received as f64,
            total_bytes: model.size() as f64,
            error,
        })
        .emit(&self.app);
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().0.clone()
    }
}

/// Applies a key from the form: `None` keeps the stored one, an empty key removes it.
fn store_key(vault: &Vault, provider_id: &str, key: Option<String>) -> Result<(), String> {
    let Some(key) = key else {
        return Ok(());
    };
    let key = key.trim();
    vault
        .set(&secret_name(provider_id), (!key.is_empty()).then_some(key))
        .map_err(|error| error.to_string())
}

fn secret_name(provider_id: &str) -> String {
    format!("provider:{provider_id}")
}

fn trimmed(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn to_kind(kind: AiProviderKind) -> ProviderKind {
    match kind {
        AiProviderKind::Claude => ProviderKind::Claude,
        AiProviderKind::OpenAiCompatible => ProviderKind::OpenAiCompatible,
        AiProviderKind::Deepl => ProviderKind::Deepl,
    }
}

fn from_kind(kind: ProviderKind) -> AiProviderKind {
    match kind {
        ProviderKind::Claude => AiProviderKind::Claude,
        ProviderKind::OpenAiCompatible => AiProviderKind::OpenAiCompatible,
        ProviderKind::Deepl => AiProviderKind::Deepl,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_keys_keep_replace_or_remove() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(&dir.path().join("secrets.db"), &secrets::new_key().unwrap());
        store_key(&vault, "a", Some(" sk-first-1111 ".into())).unwrap();
        assert_eq!(vault.get("provider:a").as_deref(), Some("sk-first-1111"));
        store_key(&vault, "a", None).unwrap();
        assert_eq!(
            vault.hint("provider:a").as_deref(),
            Some("1111"),
            "no key keeps the old"
        );
        store_key(&vault, "a", Some("sk-second-2222".into())).unwrap();
        assert_eq!(vault.hint("provider:a").as_deref(), Some("2222"));
        store_key(&vault, "a", Some("  ".into())).unwrap();
        assert!(!vault.contains("provider:a"), "an empty key removes it");
        assert_eq!(trimmed(Some("  ".into())), None);
        assert_eq!(
            from_kind(to_kind(AiProviderKind::Deepl)),
            AiProviderKind::Deepl
        );
    }
}
