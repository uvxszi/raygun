use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;

use super::*;

#[derive(Default)]
struct Seen {
    path: String,
    headers: HeaderMap,
    body: Value,
}

type Shared = Arc<Mutex<Seen>>;

/// A server answering every POST with `status` and `reply`, remembering the last request.
async fn mock(status: u16, reply: Value) -> (String, Shared) {
    let seen: Shared = Arc::default();
    let reply = Arc::new((StatusCode::from_u16(status).unwrap(), reply.to_string()));
    let handler = {
        let reply = reply.clone();
        move |State(seen): State<Shared>, uri: axum::http::Uri, headers: HeaderMap, body: String| {
            let reply = reply.clone();
            async move {
                *seen.lock().unwrap() = Seen {
                    path: uri.path().to_owned(),
                    headers,
                    body: serde_json::from_str(&body).unwrap_or(Value::Null),
                };
                (reply.0, reply.1.clone())
            }
        }
    };
    let app = Router::new()
        .route("/{*path}", post(handler))
        .with_state(seen.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}"), seen)
}

fn provider(kind: ProviderKind, base: &str, model: Option<&str>) -> Provider {
    Provider {
        id: "p".into(),
        kind,
        name: "Test".into(),
        base_url: Some(format!("{base}/")),
        model: model.map(str::to_owned),
    }
}

fn texts() -> Vec<String> {
    vec!["Hello".into(), "Good night".into()]
}

#[tokio::test]
async fn claude_request_and_answer() {
    let reply = json!({
        "type": "message",
        "stop_reason": "end_turn",
        "content": [
            { "type": "thinking", "thinking": "" },
            { "type": "text", "text": "{\"translations\":[\"こんにちは\",\"おやすみ\"]}" }
        ]
    });
    let (base, seen) = mock(200, reply).await;
    let client = raygun_net::http_client().unwrap();
    let claude = provider(ProviderKind::Claude, &base, None);
    let out = translate(
        &client,
        &claude,
        Some("sk-test"),
        &texts(),
        Some("en"),
        "ja",
    )
    .await
    .unwrap();
    assert_eq!(out, ["こんにちは", "おやすみ"]);

    {
        let seen = seen.lock().unwrap();
        assert_eq!(seen.path, "/v1/messages");
        assert_eq!(seen.headers["x-api-key"], "sk-test");
        assert_eq!(seen.headers["anthropic-version"], ANTHROPIC_VERSION);
        assert_eq!(seen.body["model"], CLAUDE_MODEL);
        assert_eq!(seen.body["output_config"]["format"]["type"], "json_schema");
        assert!(seen.body.get("temperature").is_none());
        assert_eq!(
            seen.body["messages"][0]["content"],
            "[\"Hello\",\"Good night\"]"
        );
        assert!(seen.body["system"].as_str().unwrap().contains("`ja`"));
    }

    assert!(matches!(
        translate(&client, &claude, None, &texts(), None, "ja").await,
        Err(TranslateError::MissingKey(_))
    ));
}

#[tokio::test]
async fn claude_refusal_and_auth_error() {
    let client = raygun_net::http_client().unwrap();
    let (base, _) = mock(200, json!({ "stop_reason": "refusal", "content": [] })).await;
    let claude = provider(ProviderKind::Claude, &base, Some("claude-opus-5-5"));
    assert!(matches!(
        translate(&client, &claude, Some("k"), &texts(), None, "ja").await,
        Err(TranslateError::Refused)
    ));

    let error = json!({
        "type": "error",
        "error": { "type": "authentication_error", "message": "invalid x-api-key" }
    });
    let (base, _) = mock(401, error).await;
    let claude = provider(ProviderKind::Claude, &base, None);
    let result = translate(&client, &claude, Some("k"), &texts(), None, "ja").await;
    match result {
        Err(TranslateError::Provider { status, message }) => {
            assert_eq!(status, 401);
            assert_eq!(message, "invalid x-api-key");
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn openai_compatible_with_prose_and_count_mismatch() {
    let reply = json!({
        "choices": [{
            "finish_reason": "stop",
            "message": { "content": "Sure! {\"translations\": [\"Hola\", \"Buenas noches\"]} Done." }
        }]
    });
    let (base, seen) = mock(200, reply).await;
    let client = raygun_net::http_client().unwrap();
    let local = provider(ProviderKind::OpenAiCompatible, &base, Some("qwen3"));
    let out = translate(&client, &local, None, &texts(), None, "es")
        .await
        .unwrap();
    assert_eq!(out, ["Hola", "Buenas noches"]);
    {
        let seen = seen.lock().unwrap();
        assert_eq!(seen.path, "/chat/completions");
        assert!(
            seen.headers.get("authorization").is_none(),
            "no key, no header"
        );
        assert_eq!(seen.body["model"], "qwen3");
    }
    translate(&client, &local, Some("sk-x"), &texts(), None, "es")
        .await
        .unwrap();
    assert_eq!(seen.lock().unwrap().headers["authorization"], "Bearer sk-x");

    let no_model = provider(ProviderKind::OpenAiCompatible, &base, None);
    assert!(matches!(
        translate(&client, &no_model, None, &texts(), None, "es").await,
        Err(TranslateError::MissingModel(_))
    ));

    let (base, _) = mock(
        200,
        json!({ "choices": [{ "message": { "content": "[\"only one\"]" } }] }),
    )
    .await;
    let short = provider(ProviderKind::OpenAiCompatible, &base, Some("m"));
    assert!(matches!(
        translate(&client, &short, None, &texts(), None, "es").await,
        Err(TranslateError::CountMismatch {
            expected: 2,
            got: 1
        })
    ));
}

#[tokio::test]
async fn deepl_request_and_error() {
    let reply = json!({
        "translations": [
            { "detected_source_language": "EN", "text": "Hallo" },
            { "detected_source_language": "EN", "text": "Gute Nacht" }
        ]
    });
    let (base, seen) = mock(200, reply).await;
    let client = raygun_net::http_client().unwrap();
    let deepl = provider(ProviderKind::Deepl, &base, None);
    let out = translate(
        &client,
        &deepl,
        Some("abc:fx"),
        &texts(),
        Some("en-GB"),
        "de",
    )
    .await
    .unwrap();
    assert_eq!(out, ["Hallo", "Gute Nacht"]);
    {
        let seen = seen.lock().unwrap();
        assert_eq!(seen.path, "/v2/translate");
        assert_eq!(seen.headers["authorization"], "DeepL-Auth-Key abc:fx");
        assert_eq!(seen.body["target_lang"], "DE");
        assert_eq!(seen.body["source_lang"], "EN");
        assert_eq!(seen.body["text"], json!(["Hello", "Good night"]));
    }

    let (base, _) = mock(403, json!({ "message": "Wrong endpoint" })).await;
    let deepl = provider(ProviderKind::Deepl, &base, None);
    assert!(matches!(
        translate(&client, &deepl, Some("k"), &texts(), None, "en").await,
        Err(TranslateError::Provider { status: 403, .. })
    ));
}

#[tokio::test]
async fn deepl_takes_fifty_texts_a_request() {
    // Answers each request with its own texts, marked, and counts the requests.
    let requests = Arc::new(Mutex::new(Vec::<usize>::new()));
    let handler = {
        let requests = requests.clone();
        move |body: String| {
            let requests = requests.clone();
            async move {
                let body: Value = serde_json::from_str(&body).unwrap();
                let texts = body["text"].as_array().unwrap().clone();
                requests.lock().unwrap().push(texts.len());
                let translations: Vec<Value> = texts
                    .iter()
                    .map(|text| json!({ "text": format!("{}!", text.as_str().unwrap()) }))
                    .collect();
                json!({ "translations": translations }).to_string()
            }
        }
    };
    let app = Router::new().route("/{*path}", post(handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = raygun_net::http_client().unwrap();
    let deepl = provider(ProviderKind::Deepl, &format!("http://{address}"), None);
    let lines: Vec<String> = (0..120).map(|line| format!("line {line}")).collect();
    let out = translate_as(
        &client,
        &deepl,
        Some("k"),
        &lines,
        None,
        "de",
        Content::Lyrics,
    )
    .await
    .unwrap();
    assert_eq!(out.len(), 120);
    assert_eq!(out[0], "line 0!");
    assert_eq!(out[119], "line 119!");
    assert_eq!(*requests.lock().unwrap(), [50, 50, 20]);
}

#[test]
fn lyrics_are_asked_for_line_by_line() {
    let lyrics = instructions(None, "de", Content::Lyrics);
    assert!(lyrics.contains("song lyrics") && lyrics.contains("`de`"));
    assert!(lyrics.contains("never merge, split, skip"));
    assert!(!instructions(Some("en"), "ja", Content::Text).contains("song"));
}

#[test]
fn defaults_and_parsing() {
    let deepl = Provider {
        id: "d".into(),
        kind: ProviderKind::Deepl,
        name: "DeepL".into(),
        base_url: None,
        model: None,
    };
    assert_eq!(deepl.base(Some("x:fx")), DEEPL_FREE);
    assert_eq!(deepl.base(Some("x")), DEEPL_PRO);
    assert_eq!(deepl_target("en"), "EN-US");
    assert_eq!(deepl_target("pt"), "PT-BR");
    assert_eq!(deepl_target("zh_TW"), "ZH-HANT");
    assert_eq!(deepl_target("ja"), "JA");
    assert_eq!(
        parse_translations("```json\n{\"translations\":[\"a\"]}\n```").unwrap(),
        ["a"]
    );
    assert!(matches!(
        parse_translations("no json here"),
        Err(TranslateError::BadAnswer)
    ));
}
