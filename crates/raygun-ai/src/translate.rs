//! Translation providers: Claude (the Messages API over HTTP; Rust has no official SDK), any
//! OpenAI-compatible endpoint (including local Ollama and LM Studio), DeepL, and the free ones:
//! a LibreTranslate server (self-hosted or a public one, key optional) and MyMemory (anonymous
//! with a daily limit; an email raises it, sent only when one was entered). A Whisper server
//! (OpenAI-compatible transcription, self-hosted) is listed with them but only transcribes.
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
/// MyMemory's own free endpoint.
pub const MYMEMORY_BASE: &str = "https://api.mymemory.translated.net";
/// MyMemory takes at most 500 bytes a text; longer ones go in pieces.
const MYMEMORY_MAX_BYTES: usize = 480;
/// MyMemory texts asked for at once.
const MYMEMORY_AT_ONCE: usize = 4;
/// LibreTranslate texts per request.
const LIBRE_BATCH: usize = 40;
/// A 429 is waited out this many times, when it says for how long (at most `MAX_RETRY_WAIT`).
const RATE_RETRIES: usize = 2;
const MAX_RETRY_WAIT: std::time::Duration = std::time::Duration::from_secs(20);
/// Longest provider error text kept in a message.
const MAX_ERROR_CHARS: usize = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderKind {
    Claude,
    OpenAiCompatible,
    Deepl,
    LibreTranslate,
    MyMemory,
    /// An OpenAI-compatible transcription server (faster-whisper-server, whisper.cpp's server):
    /// it hears words and doesn't translate.
    WhisperServer,
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
                ProviderKind::MyMemory => MYMEMORY_BASE.into(),
                // No public server is free and dependable: the address is the user's.
                ProviderKind::LibreTranslate | ProviderKind::WhisperServer => String::new(),
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
    #[error("{0} needs its server's address")]
    MissingAddress(&'static str),
    #[error("a Whisper server transcribes; it doesn't translate")]
    TranscribesOnly,
    #[error("MyMemory's free translations for today are used up")]
    QuotaUsed,
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

/// What the texts are, so the translator keeps to their form.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Content {
    #[default]
    Text,
    /// A song's lines, in order (M46.48).
    Lyrics,
    /// A video's subtitle cues, in the order they show.
    Subtitles,
}

/// DeepL takes at most this many texts per request.
const DEEPL_BATCH: usize = 50;

/// Translates `texts` from `from` (a language code, or `None` to detect) to `to`.
pub async fn translate(
    client: &reqwest::Client,
    provider: &Provider,
    key: Option<&str>,
    texts: &[String],
    from: Option<&str>,
    to: &str,
) -> Result<Vec<String>, TranslateError> {
    translate_as(client, provider, key, texts, from, to, Content::Text).await
}

/// [`translate`] for a kind of content: the translator is told what the texts are.
pub async fn translate_as(
    client: &reqwest::Client,
    provider: &Provider,
    key: Option<&str>,
    texts: &[String],
    from: Option<&str>,
    to: &str,
    content: Content,
) -> Result<Vec<String>, TranslateError> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let key = key.map(str::trim).filter(|key| !key.is_empty());
    let system = instructions(from, to, content);
    let translations = match provider.kind {
        ProviderKind::Claude => claude(client, provider, key, texts, &system).await?,
        ProviderKind::OpenAiCompatible => openai(client, provider, key, texts, &system).await?,
        ProviderKind::Deepl => {
            let mut all = Vec::with_capacity(texts.len());
            for batch in texts.chunks(DEEPL_BATCH) {
                all.extend(deepl(client, provider, key, batch, from, to).await?);
            }
            all
        }
        ProviderKind::LibreTranslate => {
            let mut all = Vec::with_capacity(texts.len());
            for batch in texts.chunks(LIBRE_BATCH) {
                all.extend(libretranslate(client, provider, key, batch, from, to).await?);
            }
            all
        }
        ProviderKind::MyMemory => mymemory(client, provider, key, texts, from, to).await?,
        ProviderKind::WhisperServer => return Err(TranslateError::TranscribesOnly),
    };
    if translations.len() != texts.len() {
        return Err(TranslateError::CountMismatch {
            expected: texts.len(),
            got: translations.len(),
        });
    }
    Ok(translations)
}

fn instructions(from: Option<&str>, to: &str, content: Content) -> String {
    let source = from.map_or("the source language".to_owned(), |from| format!("`{from}`"));
    match content {
        Content::Text => format!(
            "You translate text from {source} into the language with code `{to}`. You get a JSON \
             array of strings and answer with a JSON object {{\"translations\": [...]}} holding \
             one translation per string, in the same order. Keep names, line breaks and \
             formatting; translate meaning naturally rather than word for word; never add notes."
        ),
        Content::Lyrics => format!(
            "You translate song lyrics from {source} into the language with code `{to}`. You get \
             a JSON array of the song's lines in order and answer with a JSON object \
             {{\"translations\": [...]}} holding one translation per line, in the same order and \
             as many as there are lines. Translate each line so it reads naturally and the song's \
             meaning carries across lines; keep names; leave a line that is only sounds (\"oh\", \
             \"la la\") as it is; never merge, split, skip or explain lines."
        ),
        Content::Subtitles => format!(
            "You translate a video's subtitles from {source} into the language with code `{to}`. \
             You get a JSON array of the subtitle cues in the order they show and answer with a \
             JSON object {{\"translations\": [...]}} holding one translation per cue, in the \
             same order and as many as there are cues. Translate each cue as natural spoken \
             dialogue that is short enough to read on screen, carrying the meaning across cues; \
             keep names, the line breaks inside a cue and a leading dash that marks another \
             speaker; leave sound descriptions like \"[music]\" translated but in brackets; \
             never merge, split, skip or explain cues."
        ),
    }
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
    system: &str,
) -> Result<Vec<String>, TranslateError> {
    let key = key.ok_or(TranslateError::MissingKey("Claude"))?;
    let body = json!({
        "model": provider.model_or(CLAUDE_MODEL),
        "max_tokens": 16000,
        "system": system,
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
    system: &str,
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
            { "role": "system", "content": system },
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

/// Sends a request again after a 429 that says how long to wait (`Retry-After` in seconds, up
/// to `MAX_RETRY_WAIT`), a couple of times; other answers come back as they are.
async fn send_waiting(
    request: reqwest::RequestBuilder,
) -> Result<reqwest::Response, TranslateError> {
    let mut tries = 0;
    loop {
        let Some(again) = request.try_clone() else {
            return Ok(request.send().await?);
        };
        let response = again.send().await?;
        let wait = (response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS)
            .then(|| retry_after(&response))
            .flatten()
            .filter(|wait| *wait <= MAX_RETRY_WAIT);
        match wait {
            Some(wait) if tries < RATE_RETRIES => {
                tries += 1;
                tracing::info!(?wait, "translator limiting requests: waiting");
                tokio::time::sleep(wait).await;
            }
            _ => return Ok(response),
        }
    }
}

fn retry_after(response: &reqwest::Response) -> Option<std::time::Duration> {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
        .map(std::time::Duration::from_secs)
}

/// LibreTranslate's `/translate`, several texts in one request.
async fn libretranslate(
    client: &reqwest::Client,
    provider: &Provider,
    key: Option<&str>,
    texts: &[String],
    from: Option<&str>,
    to: &str,
) -> Result<Vec<String>, TranslateError> {
    let base = provider.base(key);
    if base.is_empty() {
        return Err(TranslateError::MissingAddress("LibreTranslate"));
    }
    let mut body = json!({
        "q": texts,
        "source": from.map_or("auto".to_owned(), libre_language),
        "target": libre_language(to),
        "format": "text",
    });
    if let Some(key) = key {
        body["api_key"] = json!(key);
    }
    let answer =
        read_json(send_waiting(client.post(format!("{base}/translate")).json(&body)).await?)
            .await?;
    match answer.get("translatedText") {
        Some(Value::Array(items)) => Ok(items
            .iter()
            .map(|item| item.as_str().unwrap_or_default().to_owned())
            .collect()),
        // An older server answers one text with one string.
        Some(Value::String(text)) if texts.len() == 1 => Ok(vec![text.clone()]),
        _ => Err(TranslateError::BadAnswer),
    }
}

/// LibreTranslate's language codes: the base language, with Chinese by its characters.
fn libre_language(code: &str) -> String {
    let lower = code.replace('_', "-").to_lowercase();
    match lower.as_str() {
        "zh-hant" | "zh-tw" | "zh-hk" | "zh-mo" => "zt".into(),
        "pt-br" => "pb".into(),
        _ => base_language(&lower).to_owned(),
    }
}

/// MyMemory's `/get`: a text a request, a few at once; texts over its limit go in pieces.
async fn mymemory(
    client: &reqwest::Client,
    provider: &Provider,
    email: Option<&str>,
    texts: &[String],
    from: Option<&str>,
    to: &str,
) -> Result<Vec<String>, TranslateError> {
    let base = provider.base(email);
    let pair = format!("{}|{}", from.unwrap_or("autodetect"), to);
    let mut out = Vec::with_capacity(texts.len());
    for group in texts.chunks(MYMEMORY_AT_ONCE) {
        let asks: Vec<_> = group
            .iter()
            .map(|text| mymemory_text(client, &base, email, text, &pair))
            .collect();
        out.extend(futures_util::future::try_join_all(asks).await?);
    }
    Ok(out)
}

/// One text, in pieces when it's over MyMemory's limit.
async fn mymemory_text(
    client: &reqwest::Client,
    base: &str,
    email: Option<&str>,
    text: &str,
    pair: &str,
) -> Result<String, TranslateError> {
    let mut parts = Vec::new();
    for piece in pieces(text, MYMEMORY_MAX_BYTES) {
        parts.push(mymemory_one(client, base, email, piece, pair).await?);
    }
    Ok(parts.join(" "))
}
async fn mymemory_one(
    client: &reqwest::Client,
    base: &str,
    email: Option<&str>,
    text: &str,
    pair: &str,
) -> Result<String, TranslateError> {
    if text.trim().is_empty() {
        return Ok(text.to_owned());
    }
    let mut query = vec![("q", text), ("langpair", pair)];
    if let Some(email) = email {
        query.push(("de", email));
    }
    let url = reqwest::Url::parse_with_params(&format!("{base}/get"), &query)
        .map_err(|_| TranslateError::MissingAddress("MyMemory"))?;
    let request = client.get(url);
    let answer = read_json(send_waiting(request).await?).await?;
    if answer.get("quotaFinished").and_then(Value::as_bool) == Some(true) {
        return Err(TranslateError::QuotaUsed);
    }
    let status = match answer.get("responseStatus") {
        Some(Value::Number(number)) => number.as_u64().unwrap_or(200),
        Some(Value::String(text)) => text.parse().unwrap_or(200),
        _ => 200,
    };
    if status != 200 {
        if status == 429 {
            return Err(TranslateError::QuotaUsed);
        }
        let message = answer
            .get("responseDetails")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return Err(TranslateError::Provider {
            status: status as u16,
            message: message.chars().take(MAX_ERROR_CHARS).collect(),
        });
    }
    answer
        .pointer("/responseData/translatedText")
        .and_then(Value::as_str)
        .map(unescape)
        .ok_or(TranslateError::BadAnswer)
}

/// A text in pieces of at most `max` bytes, cut after sentences where it can, else at spaces.
fn pieces(text: &str, max: usize) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while rest.len() > max {
        let mut cut = max;
        while !rest.is_char_boundary(cut) {
            cut -= 1;
        }
        let head = &rest[..cut];
        let at = head
            .rfind(['.', '!', '?', '。', '！', '？'])
            .map(|at| at + head[at..].chars().next().map_or(1, char::len_utf8))
            .filter(|at| *at > max / 3)
            .or_else(|| head.rfind(' ').filter(|at| *at > 0))
            .unwrap_or(cut);
        out.push(rest[..at].trim());
        rest = rest[at..].trim_start();
    }
    if !rest.is_empty() || out.is_empty() {
        out.push(rest);
    }
    out
}

/// MyMemory sometimes answers with HTML entities for quotes and ampersands.
fn unescape(text: &str) -> String {
    text.replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
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
