//! Translation providers: Claude (the Messages API over HTTP; Rust has no official SDK), any
//! OpenAI-compatible endpoint (including local Ollama and LM Studio) and DeepL.
//!
//! One call translates a list of texts and must return one translation per text. Batching across
//! a chapter and glossaries build on this in M37.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const CLAUDE_BASE: &str = "https://api.anthropic.com";
pub const CLAUDE_MODEL: &str = "claude-sonnet-5-5";
const ANTHROPIC_VERSION: &str = "2026-10-21";
pub const OPENAI_BASE: &str = "http://localhost:11434/v1";
const DEEPL_FREE: &str = "https://api-free.deepl.com";
const DEEPL_PRO: &str = "https://api.deepl.com";
/// Longest provider error text kept in a message.
const MAX_ERROR_CHARS: usize = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderKind {
    Claude,
    OpenAiCompatible,
    Deepl,
}

/// A configured translator. Its key is kept apart, in the secrets store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub kind: ProviderKind,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl Provider {
    fn base(&self, key: Option<&str>) -> String {
        let base = self
            .base_url
            .as_deref()
            .map(str::trim)
            .filter(|base| !base.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| match self.kind {
                ProviderKind::Claude => CLAUDE_BASE.into(),
                ProviderKind::OpenAiCompatible => OPENAI_BASE.into(),
                ProviderKind::Deepl if key.is_some_and(|key| key.ends_with(":fx")) => {
                    DEEPL_FREE.into()
                }
                ProviderKind::Deepl => DEEPL_PRO.into(),
            });
        base.trim_end_matches('/').to_owned()
    }

    fn model_or(&self, default: &str) -> String {
        self.model
            .as_deref()
            .map(str::trim)
            .filter(|model| !model.is_empty())
            .unwrap_or(default)
            .to_owned()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TranslateError {
    #[error("{0} needs an API key")]
    MissingKey(&'static str),
    #[error("{0} needs a model name")]
    MissingModel(&'static str),
    #[error("couldn't reach the translator: {0}")]
    Http(#[from] reqwest::Error),
    #[error("the translator answered {status}: {message}")]
    Provider { status: u16, message: String },
    #[error("the translator declined this text")]
    Refused,
    #[error("the translator's answer was cut off")]
    Truncated,
    #[error("the translator's answer couldn't be read")]
    BadAnswer,
    #[error("the translator returned {got} translations for {expected} texts")]
    CountMismatch { expected: usize, got: usize },
}

/// Translates `texts` from `from` (a language code, or `None` to detect) to `to`.
pub async fn translate(
    client: &reqwest::Client,
    provider: &Provider,
    key: Option<&str>,
    texts: &[String],
    from: Option<&str>,
    to: &str,
) -> Result<Vec<String>, TranslateError> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let key = key.map(str::trim).filter(|key| !key.is_empty());
    let translations = match provider.kind {
        ProviderKind::Claude => claude(client, provider, key, texts, from, to).await?,
        ProviderKind::OpenAiCompatible => openai(client, provider, key, texts, from, to).await?,
        ProviderKind::Deepl => deepl(client, provider, key, texts, from, to).await?,
    };
    if translations.len() != texts.len() {
        return Err(TranslateError::CountMismatch {
            expected: texts.len(),
            got: translations.len(),
        });
    }
    Ok(translations)
}

fn instructions(from: Option<&str>, to: &str) -> String {
    let source = from.map_or("the source language".to_owned(), |from| format!("`{from}`"));
    format!(
        "You translate text from {source} into the language with code `{to}`. You get a JSON \
         array of strings and answer with a JSON object {{\"translations\": [...]}} holding one \
         translation per string, in the same order. Keep names, line breaks and formatting; \
         translate meaning naturally rather than word for word; never add notes."
    )
}

/// The `{translations: string[]}` answer schema.
fn answer_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "translations": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["translations"],
        "additionalProperties": false
    })
}

async fn claude(
    client: &reqwest::Client,
    provider: &Provider,
    key: Option<&str>,
    texts: &[String],
    from: Option<&str>,
    to: &str,
) -> Result<Vec<String>, TranslateError> {
    let key = key.ok_or(TranslateError::MissingKey("Claude"))?;
    let body = json!({
        "model": provider.model_or(CLAUDE_MODEL),
        "max_tokens": 16000,
        "system": instructions(from, to),
        "output_config": { "format": { "type": "json_schema", "schema": answer_schema() } },
        "messages": [{ "role": "user", "content": serde_json::to_string(texts).unwrap_or_default() }],
    });
    let response = client
        .post(format!("{}/v1/messages", provider.base(Some(key))))
        .header("x-api-key", key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .json(&body)
        .send()
        .await?;
    let answer = read_json(response).await?;
    match answer.get("stop_reason").and_then(Value::as_str) {
        Some("refusal") => return Err(TranslateError::Refused),
        Some("max_tokens") => return Err(TranslateError::Truncated),
        _ => {}
    }
    let text: String = answer
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect();
    parse_translations(&text)
}

async fn openai(
    client: &reqwest::Client,
    provider: &Provider,
    key: Option<&str>,
    texts: &[String],
    from: Option<&str>,
    to: &str,
) -> Result<Vec<String>, TranslateError> {
    let model = provider
        .model
        .as_deref()
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .ok_or(TranslateError::MissingModel("This translator"))?;
    let body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": instructions(from, to) },
            { "role": "user", "content": serde_json::to_string(texts).unwrap_or_default() },
        ],
        "response_format": { "type": "json_object" },
    });
    let mut request = client
        .post(format!("{}/chat/completions", provider.base(key)))
        .json(&body);
    if let Some(key) = key {
        request = request.bearer_auth(key);
    }
    let answer = read_json(request.send().await?).await?;
    let choice = answer
        .get("choices")
        .and_then(|choices| choices.get(0))
        .ok_or(TranslateError::BadAnswer)?;
    if choice.get("finish_reason").and_then(Value::as_str) == Some("length") {
        return Err(TranslateError::Truncated);
    }
    let text = choice
        .pointer("/message/content")
        .and_then(Value::as_str)
        .ok_or(TranslateError::BadAnswer)?;
    parse_translations(text)
}

async fn deepl(
    client: &reqwest::Client,
    provider: &Provider,
    key: Option<&str>,
    texts: &[String],
    from: Option<&str>,
    to: &str,
) -> Result<Vec<String>, TranslateError> {
    let key = key.ok_or(TranslateError::MissingKey("DeepL"))?;
    let mut body = json!({ "text": texts, "target_lang": deepl_target(to) });
    if let Some(from) = from {
        body["source_lang"] = json!(base_language(from).to_uppercase());
    }
    let response = client
        .post(format!("{}/v2/translate", provider.base(Some(key))))
        .header("Authorization", format!("DeepL-Auth-Key {key}"))
        .json(&body)
        .send()
        .await?;
    let answer = read_json(response).await?;
    Ok(answer
        .get("translations")
        .and_then(Value::as_array)
        .ok_or(TranslateError::BadAnswer)?
        .iter()
        .filter_map(|item| item.get("text").and_then(Value::as_str).map(str::to_owned))
        .collect())
}

fn base_language(code: &str) -> &str {
    code.split(['-', '_']).next().unwrap_or(code)
}

/// DeepL's target code: upper case, with a regional variant where DeepL requires one.
fn deepl_target(code: &str) -> String {
    let upper = code.replace('_', "-").to_uppercase();
    match upper.as_str() {
        "EN" => "EN-US".into(),
        "PT" => "PT-BR".into(),
        "ZH-HANS" | "ZH-CN" => "ZH-HANS".into(),
        "ZH-HANT" | "ZH-TW" => "ZH-HANT".into(),
        _ => upper,
    }
}

/// The body as JSON; an error status becomes a `Provider` error carrying the provider's message.
async fn read_json(response: reqwest::Response) -> Result<Value, TranslateError> {
    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        let parsed: Option<Value> = serde_json::from_str(&text).ok();
        let message = parsed
            .as_ref()
            .and_then(|value| {
                value
                    .pointer("/error/message")
                    .or_else(|| value.get("message"))
                    .or_else(|| value.get("error"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .unwrap_or(text);
        return Err(TranslateError::Provider {
            status: status.as_u16(),
            message: message.chars().take(MAX_ERROR_CHARS).collect(),
        });
    }
    serde_json::from_str(&text).map_err(|_| TranslateError::BadAnswer)
}

/// Reads `{"translations": [...]}` from a model's answer, tolerating text around the object (local
/// models often add some) and a bare array.
fn parse_translations(text: &str) -> Result<Vec<String>, TranslateError> {
    let candidates = [
        text.find('{').zip(text.rfind('}')),
        text.find('[').zip(text.rfind(']')),
    ];
    for (start, end) in candidates.into_iter().flatten() {
        if end < start {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(&text[start..=end]) else {
            continue;
        };
        let list = value.get("translations").unwrap_or(&value);
        if let Some(items) = list.as_array() {
            return Ok(items
                .iter()
                .map(|item| match item {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                })
                .collect());
        }
    }
    Err(TranslateError::BadAnswer)
}

#[cfg(test)]
mod tests;
