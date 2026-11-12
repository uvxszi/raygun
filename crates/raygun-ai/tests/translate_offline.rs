//! Offline Mode and the translators. Offline Mode is one switch for the whole process, so this
//! runs as a test binary of its own: turning it on can't stop other tests' downloads.

use axum::Router;
use axum::routing::post;
use raygun_ai::translate::{Provider, ProviderKind, TranslateError, detect, translate};

fn google(base_url: Option<String>) -> Provider {
    Provider {
        id: "g".into(),
        kind: ProviderKind::Google,
        name: "Google Translate".into(),
        base_url,
        model: None,
    }
}

#[tokio::test]
async fn offline_mode_keeps_requests_on_this_device() {
    let app = Router::new().route(
        "/{*path}",
        post(|| async { r#"{"data":{"translations":[{"translatedText":"Hallo"}]}}"# }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = raygun_net::http_client().unwrap();
    let texts = vec!["Hello".to_owned()];

    raygun_net::network_mode::set_offline(true);
    // Google's own address is somewhere else: nothing is sent.
    let away = translate(&client, &google(None), Some("k"), &texts, None, "de").await;
    let away_detect = detect(&client, &google(None), Some("k"), &texts).await;
    // A translator on this device still answers.
    let here = translate(
        &client,
        &google(Some(format!("http://{address}"))),
        Some("k"),
        &texts,
        None,
        "de",
    )
    .await;
    raygun_net::network_mode::set_offline(false);

    assert!(matches!(away, Err(TranslateError::Offline)), "{away:?}");
    assert!(
        matches!(away_detect, Err(TranslateError::Offline)),
        "{away_detect:?}"
    );
    assert_eq!(TranslateError::Offline.to_string(), "Offline mode is on");
    assert_eq!(here.unwrap(), ["Hallo"]);
}
