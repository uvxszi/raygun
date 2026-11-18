//! Reading song lists from screenshots with a provider's vision model (v2.0.0): Claude, or an
//! OpenAI-compatible model that takes pictures. The model is asked for the songs as rows (title,
//! artist, album) instead of text lines, which keeps a title and its artist together however the
//! app laid them out. One picture a request, so a long import reports picture by picture.

use base64::Engine as _;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::translate::{
    ANTHROPIC_VERSION, CLAUDE_MODEL, Provider, ProviderKind, TranslateError, reachable, read_json,
};

/// A song a picture shows.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SongRow {
    pub title: String,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
}

const INSTRUCTIONS: &str = "You read screenshots of music apps. List every song the picture \
     shows, top to bottom, as a JSON object {\"songs\": [{\"title\": ..., \"artist\": ..., \
     \"album\": ...}]}. Copy titles and artists exactly as written (keep their language, \
     capitalisation and featured artists). Use null for an artist or album the row doesn't show. \
     Leave out headings, playlist names, buttons, times, counts and anything that isn't a song; a \
     song cut off at the picture's edge counts only when its title can be read whole. Answer \
     with the JSON object only.";

/// Whether a provider's kind can be asked to read pictures (its model must take them, which only
/// asking tells).
pub fn reads_pictures(provider: &Provider) -> bool {
    match provider.kind {
        ProviderKind::Claude => true,
        ProviderKind::OpenAiCompatible => provider
            .model
            .as_deref()
            .is_some_and(|model| !model.trim().is_empty()),
        _ => false,
    }
}

/// The songs a picture (`media_type` `image/jpeg` or `image/png`) shows, in order.
pub async fn song_rows(
    client: &reqwest::Client,
    provider: &Provider,
    key: Option<&str>,
    picture: &[u8],
    media_type: &str,
) -> Result<Vec<SongRow>, TranslateError> {
    let key = key.map(str::trim).filter(|key| !key.is_empty());
    reachable(provider, key)?;
    let data = base64::engine::general_purpose::STANDARD.encode(picture);
    let text = match provider.kind {
        ProviderKind::Claude => claude(client, provider, key, &data, media_type).await?,
        ProviderKind::OpenAiCompatible => openai(client, provider, key, &data, media_type).await?,
        _ => return Err(TranslateError::BadAnswer),
    };
    parse_rows(&text)
}

fn schema() -> Value {
    let maybe = json!({ "type": ["string", "null"] });
    json!({
        "type": "object",
        "properties": {
            "songs": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": { "title": { "type": "string" }, "artist": maybe, "album": maybe },
                    "required": ["title", "artist", "album"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["songs"],
        "additionalProperties": false
    })
}

async fn claude(
    client: &reqwest::Client,
    provider: &Provider,
    key: Option<&str>,
    data: &str,
    media_type: &str,
) -> Result<String, TranslateError> {
    let key = key.ok_or(TranslateError::MissingKey("Claude"))?;
    let body = json!({
        "model": provider.model_or(CLAUDE_MODEL),
        "max_tokens": 16000,
        "system": INSTRUCTIONS,
        "output_config": { "format": { "type": "json_schema", "schema": schema() } },
        "messages": [{ "role": "user", "content": [
            { "type": "image", "source": { "type": "base64", "media_type": media_type, "data": data } },
            { "type": "text", "text": "The songs in this screenshot." },
        ] }],
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
    Ok(answer
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect())
}

async fn openai(
    client: &reqwest::Client,
    provider: &Provider,
    key: Option<&str>,
    data: &str,
    media_type: &str,
) -> Result<String, TranslateError> {
    let model = provider
        .model
        .as_deref()
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .ok_or(TranslateError::MissingModel("This provider"))?;
    let body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": INSTRUCTIONS },
            { "role": "user", "content": [
                { "type": "text", "text": "The songs in this screenshot." },
                { "type": "image_url", "image_url": { "url": format!("data:{media_type};base64,{data}") } },
            ] },
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
    choice
        .pointer("/message/content")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(TranslateError::BadAnswer)
}

/// Reads `{"songs": [...]}` from an answer, tolerating text around it and a bare array; rows
/// without a title are dropped and blank artists and albums are none.
pub fn parse_rows(text: &str) -> Result<Vec<SongRow>, TranslateError> {
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
        let list = value.get("songs").unwrap_or(&value);
        let Some(items) = list.as_array() else {
            continue;
        };
        let clean = |value: Option<String>| {
            value
                .map(|text| text.trim().to_owned())
                .filter(|text| !text.is_empty())
        };
        return Ok(items
            .iter()
            .filter_map(|item| serde_json::from_value::<SongRow>(item.clone()).ok())
            .filter_map(|row| {
                let title = row.title.trim().to_owned();
                (!title.is_empty()).then(|| SongRow {
                    title,
                    artist: clean(row.artist),
                    album: clean(row.album),
                })
            })
            .collect());
    }
    Err(TranslateError::BadAnswer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_read_from_answers_with_text_around_them() {
        let rows = parse_rows(
            "Here you go:\n{\"songs\": [{\"title\": \" Africa \", \"artist\": \"Toto\", \
             \"album\": null}, {\"title\": \"\", \"artist\": \"x\"}, {\"title\": \"Creep\", \
             \"artist\": \" \"}]}",
        )
        .unwrap();
        assert_eq!(
            rows,
            [
                SongRow {
                    title: "Africa".into(),
                    artist: Some("Toto".into()),
                    album: None
                },
                SongRow {
                    title: "Creep".into(),
                    artist: None,
                    album: None
                }
            ]
        );
        assert_eq!(parse_rows("[{\"title\": \"Juice\"}]").unwrap().len(), 1);
        assert!(parse_rows("no songs").is_err());
    }

    #[test]
    fn only_providers_with_vision_models_read_pictures() {
        let provider = |kind, model: Option<&str>| Provider {
            id: "p".into(),
            kind,
            name: "P".into(),
            base_url: None,
            model: model.map(str::to_owned),
        };
        assert!(reads_pictures(&provider(ProviderKind::Claude, None)));
        assert!(reads_pictures(&provider(
            ProviderKind::OpenAiCompatible,
            Some("gpt-5")
        )));
        assert!(!reads_pictures(&provider(
            ProviderKind::OpenAiCompatible,
            None
        )));
        assert!(!reads_pictures(&provider(ProviderKind::Deepl, None)));
    }
}
