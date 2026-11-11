//! Translation providers: Claude (the Messages API over HTTP; Rust has no official SDK), any
//! OpenAI-compatible endpoint (including local Ollama and LM Studio), DeepL, Google's Cloud
//! Translation API (Basic, v2, with the user's API key), and the free ones: a LibreTranslate
//! server (self-hosted or a public one, key optional) and MyMemory (anonymous with a daily limit;
//! an email raises it, sent only when one was entered). A Whisper server (OpenAI-compatible
//! transcription, self-hosted) is listed with them but only transcribes.
//!
//! One call translates a list of texts and must return one translation per text; translators
//! that work out the source language themselves say which they read ([`Translated::detected`]),
//! and Google's and LibreTranslate's language detection can be asked on its own ([`detect`]).
//! Offline Mode stops every request before it starts, except to a server on this device.
//!
//! Google is reached only through its documented API (`translation.googleapis.com`, Google APIs
//! Terms of Service §2(c): access by the documented means, without masking the client); the
//! undocumented endpoint Google's own pages use (`translate_a/single`) is never called.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const CLAUDE_BASE: &str = "https://api.anthropic.com";
pub const CLAUDE_MODEL: &str = "claude-sonnet-5-5";
const ANTHROPIC_VERSION: &str = "2026-10-21";
pub const OPENAI_BASE: &str = "http://localhost:11434/v1";
const DEEPL_FREE: &str = "https://api-free.deepl.com";
const DEEPL_PRO: &str = "https://api.deepl.com";
/// Google's Cloud Translation API.
pub const GOOGLE_BASE: &str = "https://translation.googleapis.com";
/// Google takes at most 128 texts a request…
const GOOGLE_BATCH: usize = 128;
/// …and advises at most 5,000 characters a request (a longer text goes alone).
const GOOGLE_MAX_CHARS: usize = 5_000;
/// Google's answers to a request over a quota (a 403, or a 429).
const GOOGLE_LIMITS: [&str; 6] = [
    "rateLimitExceeded",
    "userRateLimitExceeded",
    "dailyLimitExceeded",
    "quotaExceeded",
    "RATE_LIMIT_EXCEEDED",
    "RESOURCE_EXHAUSTED",
];
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
    /// Google's Cloud Translation API (Basic, v2) with the user's API key.
    Google,
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
                ProviderKind::Google => GOOGLE_BASE.into(),
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
    /// Over the translator's quota or rate (Google's 403s and 429s): its own words.
    #[error("the translator is limiting requests: {0}")]
    RateLimited(String),
    /// Offline Mode kept the request from leaving the device.
    #[error("{}", raygun_net::network_mode::OFFLINE_MESSAGE)]
    Offline,
    #[error("this translator doesn't tell languages apart on its own")]
    NoDetection,
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
    translate_detecting(client, provider, key, texts, from, to, content)
        .await
        .map(|done| done.texts)
}

/// Texts translated, and the language the translator read them as.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Translated {
    /// One translation per text, in order.
    pub texts: Vec<String>,
    /// The language the translator found the texts in (a language tag: `en`, `zh-Hans`), when it
    /// was left to work it out (no `from`) and said: Google, DeepL and LibreTranslate do.
    pub detected: Option<String>,
}

/// [`translate_as`], with the language the translator detected when `from` is `None`.
pub async fn translate_detecting(
    client: &reqwest::Client,
    provider: &Provider,
    key: Option<&str>,
    texts: &[String],
    from: Option<&str>,
    to: &str,
    content: Content,
) -> Result<Translated, TranslateError> {
    if texts.is_empty() {
        return Ok(Translated::default());
    }
    let key = key.map(str::trim).filter(|key| !key.is_empty());
    reachable(provider, key)?;
    let system = instructions(from, to, content);
    let mut detected = None;
    let translations = match provider.kind {
        ProviderKind::Claude => claude(client, provider, key, texts, &system).await?,
        ProviderKind::OpenAiCompatible => openai(client, provider, key, texts, &system).await?,
        ProviderKind::Deepl => {
            let mut all = Vec::with_capacity(texts.len());
            for batch in texts.chunks(DEEPL_BATCH) {
                let (done, found) = deepl(client, provider, key, batch, from, to).await?;
                detected = detected.or(found);
                all.extend(done);
            }
            all
        }
        ProviderKind::Google => {
            let mut all = Vec::with_capacity(texts.len());
            for batch in google_batches(texts) {
                let (done, found) = google(client, provider, key, batch, from, to).await?;
                detected = detected.or(found);
                all.extend(done);
            }
            all
        }
        ProviderKind::LibreTranslate => {
            let mut all = Vec::with_capacity(texts.len());
            for batch in texts.chunks(LIBRE_BATCH) {
                let (done, found) = libretranslate(client, provider, key, batch, from, to).await?;
                detected = detected.or(found);
                all.extend(done);
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
    Ok(Translated {
        texts: translations,
        detected: if from.is_none() { detected } else { None },
    })
}

/// The language each text is written in, by the translator's own detection: Google's
/// `detect` endpoint, or a LibreTranslate server's. `None` for a text it couldn't place;
/// [`TranslateError::NoDetection`] from translators without one.
pub async fn detect(
    client: &reqwest::Client,
    provider: &Provider,
    key: Option<&str>,
    texts: &[String],
) -> Result<Vec<Option<String>>, TranslateError> {
    if !matches!(
        provider.kind,
        ProviderKind::Google | ProviderKind::LibreTranslate
    ) {
        return Err(TranslateError::NoDetection);
    }
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let key = key.map(str::trim).filter(|key| !key.is_empty());
    reachable(provider, key)?;
    if provider.kind == ProviderKind::Google {
        let mut out = Vec::with_capacity(texts.len());
        for batch in google_batches(texts) {
            out.extend(google_detect(client, provider, key, batch).await?);
        }
        return Ok(out);
    }
    let mut out = Vec::with_capacity(texts.len());
    for text in texts {
        out.push(libre_detect(client, provider, key, text).await?);
    }
    Ok(out)
}

/// Fails at once while Offline Mode keeps requests from leaving the device and the translator
/// isn't on this device (a local Ollama or LibreTranslate server still answers).
fn reachable(provider: &Provider, key: Option<&str>) -> Result<(), TranslateError> {
    let base = provider.base(key);
    if base.is_empty() {
        // Without an address the translator says so itself.
        return Ok(());
    }
    raygun_net::network_mode::check(&base).map_err(|_| TranslateError::Offline)
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

/// Translations, and the language the translator detected (when it said).
type Answered = (Vec<String>, Option<String>);

async fn deepl(
    client: &reqwest::Client,
    provider: &Provider,
    key: Option<&str>,
    texts: &[String],
    from: Option<&str>,
    to: &str,
) -> Result<Answered, TranslateError> {
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
    let items = answer
        .get("translations")
        .and_then(Value::as_array)
        .ok_or(TranslateError::BadAnswer)?;
    let detected = items
        .iter()
        .find_map(|item| item.get("detected_source_language").and_then(Value::as_str))
        .map(|code| tag_from(&code.to_lowercase()));
    Ok((
        items
            .iter()
            .filter_map(|item| item.get("text").and_then(Value::as_str).map(str::to_owned))
            .collect(),
        detected,
    ))
}

/// Texts in the groups Google takes at once: at most `GOOGLE_BATCH` texts and about
/// `GOOGLE_MAX_CHARS` characters (a longer text goes alone).
fn google_batches(texts: &[String]) -> Vec<&[String]> {
    let mut batches = Vec::new();
    let mut start = 0;
    let mut chars = 0;
    for (index, text) in texts.iter().enumerate() {
        let size = text.chars().count();
        if index > start && (index - start >= GOOGLE_BATCH || chars + size > GOOGLE_MAX_CHARS) {
            batches.push(&texts[start..index]);
            start = index;
            chars = 0;
        }
        chars += size;
    }
    if start < texts.len() {
        batches.push(&texts[start..]);
    }
    batches
}

/// Google's `translate`: plain text in, plain text out (no HTML entities), the key in its header
/// rather than the address. A 429 that says how long is waited out.
async fn google(
    client: &reqwest::Client,
    provider: &Provider,
    key: Option<&str>,
    texts: &[String],
    from: Option<&str>,
    to: &str,
) -> Result<Answered, TranslateError> {
    let key = key.ok_or(TranslateError::MissingKey("Google Translate"))?;
    let mut body = json!({ "q": texts, "target": google_language(to), "format": "text" });
    if let Some(from) = from {
        body["source"] = json!(google_language(from));
    }
    let request = client
        .post(format!(
            "{}/language/translate/v2",
            provider.base(Some(key))
        ))
        .header("x-goog-api-key", key)
        .json(&body);
    let answer = google_json(send_waiting(request).await?).await?;
    let items = answer
        .pointer("/data/translations")
        .and_then(Value::as_array)
        .ok_or(TranslateError::BadAnswer)?;
    let detected = items
        .iter()
        .find_map(|item| item.get("detectedSourceLanguage").and_then(Value::as_str))
        .filter(|code| !code.is_empty() && *code != "und")
        .map(tag_from);
    let texts = items
        .iter()
        .map(|item| {
            item.get("translatedText")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or(TranslateError::BadAnswer)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((texts, detected))
}

/// Google's `detect`: each text's likeliest language.
async fn google_detect(
    client: &reqwest::Client,
    provider: &Provider,
    key: Option<&str>,
    texts: &[String],
) -> Result<Vec<Option<String>>, TranslateError> {
    let key = key.ok_or(TranslateError::MissingKey("Google Translate"))?;
    let request = client
        .post(format!(
            "{}/language/translate/v2/detect",
            provider.base(Some(key))
        ))
        .header("x-goog-api-key", key)
        .json(&json!({ "q": texts }));
    let answer = google_json(send_waiting(request).await?).await?;
    let detections = answer
        .pointer("/data/detections")
        .and_then(Value::as_array)
        .ok_or(TranslateError::BadAnswer)?;
    if detections.len() != texts.len() {
        return Err(TranslateError::CountMismatch {
            expected: texts.len(),
            got: detections.len(),
        });
    }
    Ok(detections
        .iter()
        .map(|guesses| {
            // The likeliest first; "und" when Google couldn't tell.
            guesses
                .as_array()
                .and_then(|guesses| guesses.first())
                .and_then(|guess| guess.get("language"))
                .and_then(Value::as_str)
                .filter(|code| !code.is_empty() && *code != "und")
                .map(tag_from)
        })
        .collect())
}

/// Google's answer as JSON. Its errors carry a reason: over a quota or a rate (a 403 as often as
/// a 429) is [`TranslateError::RateLimited`], the rest keep their status and Google's message.
async fn google_json(response: reqwest::Response) -> Result<Value, TranslateError> {
    let status = response.status();
    if status.is_success() {
        return read_json(response).await;
    }
    let text = response.text().await?;
    let parsed: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let error = &parsed["error"];
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or(text);
    let message: String = message.chars().take(MAX_ERROR_CHARS).collect();
    let mut reasons: Vec<&str> = Vec::new();
    for list in ["errors", "details"] {
        for item in error
            .get(list)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            reasons.extend(item.get("reason").and_then(Value::as_str));
        }
    }
    reasons.extend(error.get("status").and_then(Value::as_str));
    let limited = status == reqwest::StatusCode::TOO_MANY_REQUESTS
        || reasons.iter().any(|reason| GOOGLE_LIMITS.contains(reason));
    if limited {
        return Err(TranslateError::RateLimited(message));
    }
    Err(TranslateError::Provider {
        status: status.as_u16(),
        message,
    })
}

/// Google's language codes: the base language, Chinese by its characters (`zh-CN` Simplified,
/// `zh-TW` Traditional), European Portuguese apart, Filipino as `tl`, Norwegian as `no`.
fn google_language(code: &str) -> String {
    let lower = code.replace('_', "-").to_lowercase();
    match lower.as_str() {
        "zh-hant" | "zh-tw" | "zh-hk" | "zh-mo" => "zh-TW".into(),
        "zh" | "zh-hans" | "zh-cn" | "zh-sg" => "zh-CN".into(),
        "pt-pt" => "pt-PT".into(),
        "fil" => "tl".into(),
        "nb" | "nn" => "no".into(),
        _ => base_language(&lower).to_owned(),
    }
}

/// A language a translator named, as a tag the app uses: Chinese by its characters, and the old
/// codes Google still answers with for Hebrew and Javanese.
fn tag_from(code: &str) -> String {
    match code.replace('_', "-").to_lowercase().as_str() {
        "zh-cn" | "zh-sg" | "zh-hans" | "zh" => "zh-Hans".into(),
        "zh-tw" | "zh-hk" | "zh-hant" => "zh-Hant".into(),
        "iw" => "he".into(),
        "jw" => "jv".into(),
        "pt-pt" => "pt-PT".into(),
        "pt-br" => "pt-BR".into(),
        other => base_language(other).to_owned(),
    }
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
) -> Result<Answered, TranslateError> {
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
    // With `auto`, what it read the texts as: one for one text, one each for several.
    let detected = match answer.get("detectedLanguage") {
        Some(Value::Array(items)) => items.first(),
        other => other,
    }
    .and_then(|found| found.get("language"))
    .and_then(Value::as_str)
    .filter(|code| !code.is_empty())
    .map(tag_from);
    match answer.get("translatedText") {
        Some(Value::Array(items)) => Ok((
            items
                .iter()
                .map(|item| item.as_str().unwrap_or_default().to_owned())
                .collect(),
            detected,
        )),
        // An older server answers one text with one string.
        Some(Value::String(text)) if texts.len() == 1 => Ok((vec![text.clone()], detected)),
        _ => Err(TranslateError::BadAnswer),
    }
}

/// LibreTranslate's `/detect` for one text: its likeliest language.
async fn libre_detect(
    client: &reqwest::Client,
    provider: &Provider,
    key: Option<&str>,
    text: &str,
) -> Result<Option<String>, TranslateError> {
    let base = provider.base(key);
    if base.is_empty() {
        return Err(TranslateError::MissingAddress("LibreTranslate"));
    }
    let mut body = json!({ "q": text });
    if let Some(key) = key {
        body["api_key"] = json!(key);
    }
    let answer =
        read_json(send_waiting(client.post(format!("{base}/detect")).json(&body)).await?).await?;
    Ok(answer
        .as_array()
        .and_then(|guesses| guesses.first())
        .and_then(|guess| guess.get("language"))
        .and_then(Value::as_str)
        .filter(|code| !code.is_empty())
        .map(tag_from))
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
