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
fn subtitles_are_asked_for_cue_by_cue() {
    let subtitles = instructions(Some("ja"), "en", Content::Subtitles);
    assert!(subtitles.contains("subtitle cues") && subtitles.contains("`en`"));
    assert!(subtitles.contains("`ja`") && subtitles.contains("as many as there are cues"));
    assert!(subtitles.contains("never merge, split, skip"));
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

#[tokio::test]
async fn libretranslate_sends_every_text_and_its_key() {
    let (base, seen) = mock(200, json!({ "translatedText": ["Hallo", "Gute Nacht"] })).await;
    let client = raygun_net::http_client().unwrap();
    let libre = provider(ProviderKind::LibreTranslate, &base, None);
    let out = translate(&client, &libre, Some("secret"), &texts(), None, "zh-Hant")
        .await
        .unwrap();
    assert_eq!(out, ["Hallo", "Gute Nacht"]);
    {
        let seen = seen.lock().unwrap();
        assert_eq!(seen.path, "/translate");
        assert_eq!(seen.body["q"], json!(["Hello", "Good night"]));
        assert_eq!(seen.body["source"], "auto");
        assert_eq!(seen.body["target"], "zt");
        assert_eq!(seen.body["api_key"], "secret");
    }
    // Without a key none is sent; without an address it says so.
    let (base, seen) = mock(200, json!({ "translatedText": ["a", "b"] })).await;
    let libre = provider(ProviderKind::LibreTranslate, &base, None);
    translate(&client, &libre, None, &texts(), Some("en"), "de")
        .await
        .unwrap();
    assert!(seen.lock().unwrap().body.get("api_key").is_none());
    let nowhere = Provider {
        base_url: None,
        ..provider(ProviderKind::LibreTranslate, "", None)
    };
    assert!(matches!(
        translate(&client, &nowhere, None, &texts(), None, "de").await,
        Err(TranslateError::MissingAddress("LibreTranslate"))
    ));
}

/// A MyMemory-like server: `GET /get`, each text reversed; counts requests and remembers queries.
async fn mymemory_mock(
    first_status: Option<u16>,
) -> (
    String,
    Arc<Mutex<Vec<std::collections::HashMap<String, String>>>>,
) {
    use axum::response::IntoResponse;
    let seen: Arc<Mutex<Vec<std::collections::HashMap<String, String>>>> = Arc::default();
    let handler = {
        let seen = seen.clone();
        move |uri: axum::http::Uri| {
            let seen = seen.clone();
            let query: std::collections::HashMap<String, String> =
                reqwest::Url::parse(&format!("http://local{uri}"))
                    .unwrap()
                    .query_pairs()
                    .into_owned()
                    .collect();
            async move {
                let count = {
                    let mut seen = seen.lock().unwrap();
                    seen.push(query.clone());
                    seen.len()
                };
                if count == 1
                    && let Some(status) = first_status
                {
                    return (
                        StatusCode::from_u16(status).unwrap(),
                        [("retry-after", "1")],
                        String::new(),
                    )
                        .into_response();
                }
                let text: String = query["q"].chars().rev().collect();
                json!({
                    "responseData": { "translatedText": text },
                    "quotaFinished": false,
                    "responseStatus": 200
                })
                .to_string()
                .into_response()
            }
        }
    };
    let app = Router::new().route("/get", axum::routing::get(handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}"), seen)
}

#[tokio::test]
async fn mymemory_asks_text_by_text_and_sends_an_email_only_when_given() {
    let (base, seen) = mymemory_mock(None).await;
    let client = raygun_net::http_client().unwrap();
    let mymemory = provider(ProviderKind::MyMemory, &base, None);
    let out = translate(&client, &mymemory, None, &texts(), None, "it")
        .await
        .unwrap();
    assert_eq!(out, ["olleH", "thgin dooG"]);
    {
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        assert!(
            seen.iter()
                .all(|query| query["langpair"] == "autodetect|it")
        );
        assert!(seen.iter().all(|query| !query.contains_key("de")));
    }
    let (base, seen) = mymemory_mock(None).await;
    let mymemory = provider(ProviderKind::MyMemory, &base, None);
    translate(
        &client,
        &mymemory,
        Some("me@example.com"),
        &texts()[..1],
        Some("en"),
        "it",
    )
    .await
    .unwrap();
    assert_eq!(seen.lock().unwrap()[0]["de"], "me@example.com");
    assert_eq!(seen.lock().unwrap()[0]["langpair"], "en|it");
}

#[tokio::test]
async fn a_rate_limit_that_says_how_long_is_waited_out() {
    let (base, seen) = mymemory_mock(Some(429)).await;
    let client = raygun_net::http_client().unwrap();
    let mymemory = provider(ProviderKind::MyMemory, &base, None);
    let out = translate(&client, &mymemory, None, &texts()[..1], Some("en"), "it")
        .await
        .unwrap();
    assert_eq!(out, ["olleH"]);
    assert_eq!(seen.lock().unwrap().len(), 2);
}

#[test]
fn long_texts_go_in_pieces_after_sentences() {
    let text = "This is a sentence. ".repeat(8);
    let parts = pieces(text.trim_end(), 100);
    assert_eq!(parts.len(), 2);
    assert!(parts[0].ends_with('.') && parts.iter().all(|part| part.len() <= 100));
    assert_eq!(parts.join(" "), text.trim_end());
    // Without sentences it cuts at a space.
    let words = "word ".repeat(30);
    let cut = pieces(words.trim_end(), 100);
    assert!(
        cut.len() == 2
            && cut
                .iter()
                .all(|part| part.len() <= 100 && !part.ends_with(' '))
    );
    assert_eq!(pieces("short", 100), ["short"]);
    assert_eq!(unescape("it&#39;s &amp; &quot;so&quot;"), "it's & \"so\"");
    assert_eq!(libre_language("pt-BR"), "pb");
    assert_eq!(libre_language("de-AT"), "de");
}

#[tokio::test]
async fn a_whisper_server_does_not_translate() {
    let client = raygun_net::http_client().unwrap();
    let whisper = provider(ProviderKind::WhisperServer, "http://127.0.0.1:9", None);
    assert!(matches!(
        translate(&client, &whisper, None, &texts(), None, "de").await,
        Err(TranslateError::TranscribesOnly)
    ));
}

/// MyMemory's free endpoint, anonymously: `cargo test -p raygun-ai -- --ignored mymemory_live`.
#[tokio::test]
#[ignore = "network"]
async fn mymemory_live() {
    let client = raygun_net::http_client().unwrap();
    let mymemory = Provider {
        id: "m".into(),
        kind: ProviderKind::MyMemory,
        name: "MyMemory".into(),
        base_url: None,
        model: None,
    };
    let lines = vec!["Hello world".to_owned(), "Good night".to_owned()];
    let started = std::time::Instant::now();
    let out = translate_as(
        &client,
        &mymemory,
        None,
        &lines,
        None,
        "it",
        Content::Lyrics,
    )
    .await
    .unwrap();
    println!("MyMemory: {out:?} in {:?}", started.elapsed());
    assert_eq!(out.len(), 2);
    assert!(out[0].to_lowercase().contains("mondo"));
}

/// A LibreTranslate server: `LIBRETRANSLATE_URL=<address> [LIBRETRANSLATE_KEY=<key>] cargo test
/// -p raygun-ai -- --ignored libretranslate_live`.
#[tokio::test]
#[ignore = "network"]
async fn libretranslate_live() {
    let Ok(base) = std::env::var("LIBRETRANSLATE_URL") else {
        eprintln!("LIBRETRANSLATE_URL isn't set");
        return;
    };
    let key = std::env::var("LIBRETRANSLATE_KEY").ok();
    let client = raygun_net::http_client().unwrap();
    let libre = provider(ProviderKind::LibreTranslate, &base, None);
    let out = translate(&client, &libre, key.as_deref(), &texts(), Some("en"), "de")
        .await
        .unwrap();
    println!("LibreTranslate: {out:?}");
    assert_eq!(out.len(), 2);
}

#[tokio::test]
async fn google_request_answer_and_detection() {
    let reply = json!({
        "data": {
            "translations": [
                { "translatedText": "你好", "detectedSourceLanguage": "en" },
                { "translatedText": "晚安", "detectedSourceLanguage": "en" }
            ]
        }
    });
    let (base, seen) = mock(200, reply.clone()).await;
    let client = raygun_net::http_client().unwrap();
    let google = provider(ProviderKind::Google, &base, None);
    let done = translate_detecting(
        &client,
        &google,
        Some(" AIza-test "),
        &texts(),
        None,
        "zh-Hant",
        Content::Text,
    )
    .await
    .unwrap();
    assert_eq!(done.texts, ["你好", "晚安"]);
    assert_eq!(done.detected.as_deref(), Some("en"));
    {
        let seen = seen.lock().unwrap();
        assert_eq!(seen.path, "/language/translate/v2");
        // The key goes in its header, never in the address.
        assert_eq!(seen.headers["x-goog-api-key"], "AIza-test");
        assert_eq!(seen.body["q"], json!(["Hello", "Good night"]));
        assert_eq!(seen.body["target"], "zh-TW");
        assert_eq!(seen.body["format"], "text");
        assert!(seen.body.get("source").is_none());
    }

    // A language given goes as Google's code, and nothing is reported as detected.
    let (base, seen) = mock(200, reply).await;
    let google = provider(ProviderKind::Google, &base, None);
    let done = translate_detecting(
        &client,
        &google,
        Some("k"),
        &texts(),
        Some("pt-PT"),
        "zh",
        Content::Lyrics,
    )
    .await
    .unwrap();
    assert_eq!(done.detected, None);
    assert_eq!(seen.lock().unwrap().body["source"], "pt-PT");
    assert_eq!(seen.lock().unwrap().body["target"], "zh-CN");

    assert!(matches!(
        translate(&client, &google, None, &texts(), None, "de").await,
        Err(TranslateError::MissingKey("Google Translate"))
    ));
    let default = Provider {
        base_url: None,
        ..provider(ProviderKind::Google, "", None)
    };
    assert_eq!(default.base(Some("k")), GOOGLE_BASE);
}

#[tokio::test]
async fn google_quota_answers_read_as_rate_limits() {
    let client = raygun_net::http_client().unwrap();
    // Over a per-minute quota Google answers 403, naming the reason.
    let limited = json!({
        "error": {
            "code": 403,
            "message": "User Rate Limit Exceeded",
            "errors": [{
                "message": "User Rate Limit Exceeded",
                "domain": "usageLimits",
                "reason": "userRateLimitExceeded"
            }],
            "status": "PERMISSION_DENIED"
        }
    });
    let (base, _) = mock(403, limited).await;
    let google = provider(ProviderKind::Google, &base, None);
    match translate(&client, &google, Some("k"), &texts(), None, "de").await {
        Err(TranslateError::RateLimited(message)) => {
            assert_eq!(message, "User Rate Limit Exceeded");
        }
        other => panic!("expected a rate limit, got {other:?}"),
    }
    // A 429 that doesn't say how long to wait is a rate limit at once.
    let (base, _) = mock(
        429,
        json!({ "error": { "code": 429, "message": "Too many" } }),
    )
    .await;
    let google = provider(ProviderKind::Google, &base, None);
    assert!(matches!(
        translate(&client, &google, Some("k"), &texts(), None, "de").await,
        Err(TranslateError::RateLimited(_))
    ));
    // A wrong key is the provider's own refusal, with its words.
    let bad_key = json!({
        "error": {
            "code": 400,
            "message": "API key not valid. Please pass a valid API key.",
            "errors": [{ "message": "API key not valid.", "domain": "global", "reason": "badRequest" }],
            "status": "INVALID_ARGUMENT",
            "details": [{ "@type": "type.googleapis.com/google.rpc.ErrorInfo", "reason": "API_KEY_INVALID" }]
        }
    });
    let (base, _) = mock(400, bad_key).await;
    let google = provider(ProviderKind::Google, &base, None);
    match translate(&client, &google, Some("k"), &texts(), None, "de").await {
        Err(TranslateError::Provider { status, message }) => {
            assert_eq!(status, 400);
            assert!(message.starts_with("API key not valid"));
        }
        other => panic!("expected the provider's refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn google_waits_out_a_429_that_says_how_long() {
    use axum::response::IntoResponse;
    let calls = Arc::new(Mutex::new(0usize));
    let handler = {
        let calls = calls.clone();
        move || {
            let calls = calls.clone();
            async move {
                let count = {
                    let mut calls = calls.lock().unwrap();
                    *calls += 1;
                    *calls
                };
                if count == 1 {
                    return (
                        StatusCode::TOO_MANY_REQUESTS,
                        [("retry-after", "1")],
                        String::new(),
                    )
                        .into_response();
                }
                json!({ "data": { "translations": [{ "translatedText": "Hallo" }] } })
                    .to_string()
                    .into_response()
            }
        }
    };
    let app = Router::new().route("/{*path}", post(handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = raygun_net::http_client().unwrap();
    let google = provider(ProviderKind::Google, &format!("http://{address}"), None);
    let out = translate(&client, &google, Some("k"), &texts()[..1], Some("en"), "de")
        .await
        .unwrap();
    assert_eq!(out, ["Hallo"]);
    assert_eq!(*calls.lock().unwrap(), 2);
}

#[test]
fn google_takes_128_texts_and_about_5000_characters_a_request() {
    let short: Vec<String> = (0..300).map(|n| format!("line {n}")).collect();
    let sizes: Vec<usize> = google_batches(&short)
        .iter()
        .map(|batch| batch.len())
        .collect();
    assert_eq!(sizes, [128, 128, 44]);
    let long: Vec<String> = (0..3).map(|_| "word ".repeat(600)).collect();
    let sizes: Vec<usize> = google_batches(&long)
        .iter()
        .map(|batch| batch.len())
        .collect();
    assert_eq!(sizes, [1, 1, 1], "3,000 characters each: one a request");
    let huge = vec!["x".repeat(9_000), "y".into()];
    let sizes: Vec<usize> = google_batches(&huge)
        .iter()
        .map(|batch| batch.len())
        .collect();
    assert_eq!(sizes, [1, 1], "a text over the advice goes alone");
    assert!(google_batches(&[]).is_empty());
}

#[test]
fn language_codes_as_google_has_them() {
    assert_eq!(google_language("zh-Hant"), "zh-TW");
    assert_eq!(google_language("zh_HK"), "zh-TW");
    assert_eq!(google_language("zh"), "zh-CN");
    assert_eq!(google_language("en-US"), "en");
    assert_eq!(google_language("pt-PT"), "pt-PT");
    assert_eq!(google_language("pt-BR"), "pt");
    assert_eq!(google_language("fil"), "tl");
    assert_eq!(google_language("nb"), "no");
    assert_eq!(tag_from("iw"), "he");
    assert_eq!(tag_from("zh-CN"), "zh-Hans");
    assert_eq!(tag_from("zh-TW"), "zh-Hant");
    assert_eq!(tag_from("EN"), "en");
}

#[tokio::test]
async fn google_detects_each_text() {
    let reply = json!({
        "data": {
            "detections": [
                [{ "language": "de", "isReliable": false, "confidence": 0.98 }],
                [{ "language": "und", "isReliable": false, "confidence": 0 }]
            ]
        }
    });
    let (base, seen) = mock(200, reply).await;
    let client = raygun_net::http_client().unwrap();
    let google = provider(ProviderKind::Google, &base, None);
    let found = detect(&client, &google, Some("k"), &texts()).await.unwrap();
    assert_eq!(found, [Some("de".to_owned()), None]);
    {
        let seen = seen.lock().unwrap();
        assert_eq!(seen.path, "/language/translate/v2/detect");
        assert_eq!(seen.headers["x-goog-api-key"], "k");
        assert_eq!(seen.body["q"], json!(["Hello", "Good night"]));
    }
    // Translators without detection of their own say so.
    let claude = provider(ProviderKind::Claude, &base, None);
    assert!(matches!(
        detect(&client, &claude, Some("k"), &texts()).await,
        Err(TranslateError::NoDetection)
    ));
}

#[tokio::test]
async fn deepl_and_libretranslate_say_what_they_detected() {
    let client = raygun_net::http_client().unwrap();
    let (base, _) = mock(
        200,
        json!({ "translations": [{ "detected_source_language": "FR", "text": "Hello" }] }),
    )
    .await;
    let deepl = provider(ProviderKind::Deepl, &base, None);
    let done = translate_detecting(
        &client,
        &deepl,
        Some("k"),
        &texts()[..1],
        None,
        "en",
        Content::Text,
    )
    .await
    .unwrap();
    assert_eq!(done.detected.as_deref(), Some("fr"));

    let (base, _) = mock(
        200,
        json!({
            "translatedText": ["Hallo", "Gute Nacht"],
            "detectedLanguage": [
                { "confidence": 90, "language": "en" },
                { "confidence": 80, "language": "en" }
            ]
        }),
    )
    .await;
    let libre = provider(ProviderKind::LibreTranslate, &base, None);
    let done = translate_detecting(&client, &libre, None, &texts(), None, "de", Content::Text)
        .await
        .unwrap();
    assert_eq!(done.detected.as_deref(), Some("en"));

    let (base, seen) = mock(200, json!([{ "confidence": 92, "language": "it" }])).await;
    let libre = provider(ProviderKind::LibreTranslate, &base, None);
    let found = detect(&client, &libre, Some("secret"), &texts()[..1])
        .await
        .unwrap();
    assert_eq!(found, [Some("it".to_owned())]);
    assert_eq!(seen.lock().unwrap().path, "/detect");
    assert_eq!(seen.lock().unwrap().body["api_key"], "secret");
}

/// Google's Cloud Translation API with a key: `GOOGLE_TRANSLATE_KEY=<key> cargo test -p raygun-ai
/// -- --ignored google_live --nocapture`.
#[tokio::test]
#[ignore = "network"]
async fn google_live() {
    let Ok(key) = std::env::var("GOOGLE_TRANSLATE_KEY") else {
        eprintln!("GOOGLE_TRANSLATE_KEY isn't set");
        return;
    };
    let client = raygun_net::http_client().unwrap();
    let google = Provider {
        id: "g".into(),
        kind: ProviderKind::Google,
        name: "Google Translate".into(),
        base_url: None,
        model: None,
    };
    let done = translate_detecting(
        &client,
        &google,
        Some(&key),
        &texts(),
        None,
        "de",
        Content::Text,
    )
    .await
    .unwrap();
    println!("Google: {:?} from {:?}", done.texts, done.detected);
    assert_eq!(done.texts.len(), 2);
    assert_eq!(done.detected.as_deref(), Some("en"));
    let found = detect(&client, &google, Some(&key), &["Guten Morgen".to_owned()])
        .await
        .unwrap();
    assert_eq!(found, [Some("de".to_owned())]);
}
