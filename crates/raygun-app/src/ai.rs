//! AI settings (M34): translators with their keys in the encrypted secrets store, and the models
//! the model manager can download. Both are device-local; keys never leave this module except as
//! a four-character hint.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use base64::Engine;
use raygun_ai::models::{self, CancellationToken, ModelError, ModelInfo};
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

/// A model downloading now: the bytes in so far, and what cancels it.
struct Running {
    received: u64,
    cancel: CancellationToken,
}

/// Why a model didn't download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelDownloadError {
    /// Stopped by [`AiService::cancel_model_download`]: nothing of it was kept.
    Cancelled,
    Failed(String),
}

impl std::fmt::Display for ModelDownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelDownloadError::Cancelled => f.write_str("The download was cancelled"),
            ModelDownloadError::Failed(why) => f.write_str(why),
        }
    }
}

impl From<ModelDownloadError> for String {
    fn from(error: ModelDownloadError) -> String {
        error.to_string()
    }
}

pub struct AiService {
    app: AppHandle,
    client: reqwest::Client,
    models_dir: PathBuf,
    vault: Vault,
    /// The models being downloaded, by id.
    downloads: Arc<Mutex<HashMap<String, Running>>>,
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

    // ---- Connected servers' sign-ins (`crate::servers`) ----

    /// A connected server's kept sign-in (a token, a password or an API key), by its
    /// connection's extension id.
    pub fn server_secret(&self, id: &str) -> Option<String> {
        self.vault.get(&format!("server:{id}"))
    }

    /// Keeps, or with `None` forgets, a connected server's sign-in.
    pub fn set_server_secret(&self, id: &str, secret: Option<&str>) -> Result<(), String> {
        self.vault
            .set(&format!("server:{id}"), secret)
            .map_err(|error| error.to_string())
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
        if provider.kind == ProviderKind::WhisperServer {
            // It only hears speech: answering its model list says the address works.
            let (base, key) = self.transcriber(id).await?;
            let mut request = self.client.get(format!("{base}/models"));
            if let Some(key) = key {
                request = request.bearer_auth(key);
            }
            let response = request
                .send()
                .await
                .map_err(|error| issue_text(&provider.name, &TranslateError::Http(error)))?;
            let status = response.status().as_u16();
            return if response.status().is_success() {
                Ok(format!("{} answered", provider.name))
            } else {
                Err(issue_text(
                    &provider.name,
                    &TranslateError::Provider {
                        status,
                        message: String::new(),
                    },
                ))
            };
        }
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
                Err(issue_text(&provider.name, &error))
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
        self.translate_texts(id, lines, to, translate::Content::Lyrics)
            .await
    }

    /// [`Self::translate_lyrics`] for any kind of text (a video's subtitle cues, a batch at a
    /// time): the provider is told what the texts are.
    pub async fn translate_texts(
        &self,
        id: &str,
        lines: &[String],
        to: &str,
        content: translate::Content,
    ) -> Result<(String, Vec<String>), String> {
        self.translate_texts_from(id, lines, None, to, content)
            .await
    }

    /// [`Self::translate_texts`] from a known language (a reader's translation layer says which).
    pub async fn translate_texts_from(
        &self,
        id: &str,
        lines: &[String],
        from: Option<&str>,
        to: &str,
        content: translate::Content,
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
                from,
                to,
                content,
            )
        };
        let mut result = ask().await;
        if matches!(
            result,
            Err(TranslateError::CountMismatch { .. } | TranslateError::BadAnswer)
        ) && matches!(
            provider.kind,
            ProviderKind::Claude | ProviderKind::OpenAiCompatible
        ) {
            result = ask().await;
        }
        match result {
            Ok(translated) => Ok((provider.name.clone(), translated)),
            Err(error) => {
                tracing::info!(provider = %provider.name, %error, ?content, "translation failed");
                Err(issue_text(&provider.name, &error))
            }
        }
    }

    /// How much of a model downloading now is in, in bytes.
    pub fn received(&self, id: &str) -> Option<u64> {
        self.downloads
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(id)
            .map(|running| running.received)
    }

    /// An OpenAI-compatible provider's address and key, for transcription (M46.32).
    pub async fn transcriber(&self, id: &str) -> Result<(String, Option<String>), String> {
        let provider = self
            .stored_providers()
            .await?
            .into_iter()
            .find(|provider| provider.id == id)
            .ok_or_else(|| "The transcription provider was removed".to_owned())?;
        if !from_kind(provider.kind).transcribes() {
            return Err(format!(
                "{} can't transcribe; pick an OpenAI-compatible provider or a Whisper server",
                provider.name
            ));
        }
        if provider.kind == ProviderKind::WhisperServer
            && provider
                .base_url
                .as_deref()
                .is_none_or(|base| base.trim().is_empty())
        {
            return Err(format!("{} needs its server's address", provider.name));
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

    /// The translators as stored, and with `keys` their keys by translator id (backups, M46.51).
    pub async fn backup_translators(
        &self,
        keys: bool,
    ) -> Result<
        (
            serde_json::Value,
            std::collections::BTreeMap<String, String>,
        ),
        String,
    > {
        let list = self.stored_providers().await?;
        let kept = if keys {
            list.iter()
                .filter_map(|provider| {
                    Some((
                        provider.id.clone(),
                        self.vault.get(&secret_name(&provider.id))?,
                    ))
                })
                .collect()
        } else {
            Default::default()
        };
        Ok((
            serde_json::to_value(&list).map_err(|error| error.to_string())?,
            kept,
        ))
    }

    /// Adds the translators of a backup that aren't here, with their keys when it kept them.
    /// Returns how many were added.
    pub async fn restore_translators(
        &self,
        list: &serde_json::Value,
        keys: &std::collections::BTreeMap<String, String>,
    ) -> Result<u32, String> {
        let incoming: Vec<Provider> =
            serde_json::from_value(list.clone()).map_err(|error| error.to_string())?;
        let _guard = self.providers_lock.lock().await;
        let mut stored = self.stored_providers().await?;
        let mut added = 0;
        for provider in incoming {
            if stored.iter().any(|known| known.id == provider.id) {
                continue;
            }
            store_key(&self.vault, &provider.id, keys.get(&provider.id).cloned())?;
            stored.push(provider);
            added += 1;
        }
        if added > 0 {
            self.store_providers(&stored).await?;
        }
        Ok(added)
    }

    /// How many translators there are.
    pub async fn translator_count(&self) -> usize {
        self.stored_providers().await.map_or(0, |list| list.len())
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
                downloading: downloads
                    .get(&model.id)
                    .map(|running| running.received as f64),
                size_bytes: model.size() as f64,
                id: model.id,
                name: model.name,
                kind: model.kind,
                engine: model.engine,
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

    /// Downloads a model, reporting progress as `AiModelEvent`s; returns when it is installed, or
    /// when [`Self::cancel_model_download`] stopped it (nothing of it kept).
    pub async fn download_model(&self, id: &str) -> Result<(), ModelDownloadError> {
        let model =
            models::find(id).map_err(|error| ModelDownloadError::Failed(error.to_string()))?;
        let cancel = CancellationToken::new();
        {
            let mut downloads = self.downloads.lock().unwrap_or_else(|p| p.into_inner());
            if downloads.contains_key(id) {
                return Err(ModelDownloadError::Failed(
                    "This model is already downloading".into(),
                ));
            }
            downloads.insert(
                id.to_owned(),
                Running {
                    received: 0,
                    cancel: cancel.clone(),
                },
            );
        }
        let total = model.size();
        let mut reported = 0u64;
        let result = models::download(
            &self.client,
            &self.models_dir,
            &model,
            &cancel,
            |received, _| {
                if let Some(running) = self
                    .downloads
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .get_mut(id)
                {
                    running.received = received;
                }
                if received == 0 || received >= reported + PROGRESS_STEP {
                    reported = received;
                    self.emit(&model, AiModelState::Downloading, received, None);
                }
            },
        )
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
            Err(ModelError::Cancelled) => {
                tracing::info!(model = %id, "model download cancelled");
                self.emit(&model, AiModelState::Cancelled, 0, None);
                Err(ModelDownloadError::Cancelled)
            }
            Err(error) => {
                tracing::warn!(model = %id, %error, "model download failed");
                self.emit(&model, AiModelState::Failed, 0, Some(error.to_string()));
                Err(ModelDownloadError::Failed(error.to_string()))
            }
        }
    }

    /// Stops a model's download wherever it is; what it fetched is deleted and the model stays
    /// as it was. Whether one was running.
    pub fn cancel_model_download(&self, id: &str) -> bool {
        let downloads = self.downloads.lock().unwrap_or_else(|p| p.into_inner());
        let Some(running) = downloads.get(id) else {
            return false;
        };
        running.cancel.cancel();
        true
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

/// A translator's failure as an issue: one sentence naming it (rate limited, refused, didn't
/// answer...), the provider's own words after it.
fn issue_text(name: &str, error: &TranslateError) -> String {
    use raygun_net::issue::{Issue, IssueKind, status_kind};
    let kind = match error {
        TranslateError::Provider { status, .. } => status_kind(*status),
        TranslateError::QuotaUsed => Some(IssueKind::RateLimited),
        TranslateError::Http(error) if error.is_timeout() => Some(IssueKind::Timeout),
        TranslateError::Http(error) if error.is_connect() => Some(IssueKind::Unreachable),
        TranslateError::Http(_) => Some(IssueKind::Unreachable),
        _ => None,
    };
    match kind {
        Some(kind) => Issue::new(kind, name, error.to_string()).to_string(),
        None => raygun_net::issue::named(name, &error.to_string()),
    }
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
        AiProviderKind::LibreTranslate => ProviderKind::LibreTranslate,
        AiProviderKind::MyMemory => ProviderKind::MyMemory,
        AiProviderKind::WhisperServer => ProviderKind::WhisperServer,
    }
}

fn from_kind(kind: ProviderKind) -> AiProviderKind {
    match kind {
        ProviderKind::Claude => AiProviderKind::Claude,
        ProviderKind::OpenAiCompatible => AiProviderKind::OpenAiCompatible,
        ProviderKind::Deepl => AiProviderKind::Deepl,
        ProviderKind::LibreTranslate => AiProviderKind::LibreTranslate,
        ProviderKind::MyMemory => AiProviderKind::MyMemory,
        ProviderKind::WhisperServer => AiProviderKind::WhisperServer,
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
