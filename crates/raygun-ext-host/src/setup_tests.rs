//! Sources that need a key or sign-in: asked nothing (no script, no request, nothing counted
//! against them) while a required setting is empty, or after their own answer said their key was
//! refused, until their settings change.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::Router;
use axum::routing::get;
use raygun_model::{ExtensionStatus, SetupReason};
use serde_json::json;

use crate::fanout::Standing;
use crate::manifest::{Manifest, ManifestError};
use crate::{CallError, ExtensionHost, ExtensionSource, HostConfig, UserSettings};

const ID: &str = "dev.raygun.keyed";
const PROVIDER: &str = "dev.raygun.keyed/p";

/// A local site that counts the requests it gets.
fn counting_site() -> (SocketAddr, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let counted = hits.clone();
    let app = Router::new().route(
        "/songs",
        get(move || {
            counted.fetch_add(1, Ordering::SeqCst);
            async { "[]" }
        }),
    );
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                axum::serve(listener, app).await.unwrap();
            });
    });
    (address, hits)
}

fn manifest(settings: serde_json::Value) -> String {
    json!({
        "id": ID,
        "name": "Keyed",
        "version": "1.0.0",
        "apiVersion": 1,
        "runtime": "js",
        "entry": "index.js",
        "provides": ["music.metadata"],
        "permissions": { "net": ["127.0.0.1"] },
        "settings": settings,
    })
    .to_string()
}

/// An extension with one required key whose provider counts its calls (in storage, which outlives
/// reloads) and asks `site`; a key of `bad` is refused the way an API refuses one.
fn keyed(site: SocketAddr) -> ExtensionSource {
    let code = format!(
        r#"
        const ask = async () => {{
          raygun.storage.set('calls', (raygun.storage.get('calls') ?? 0) + 1);
          await raygun.http.fetch('http://{site}/songs');
          if (raygun.settings.get('apiKey') === 'bad') raygun.needsSetup('the API refused the key');
          return {{ tracks: [] }};
        }};
        raygun.providers.register({{ kind: 'music.metadata', id: 'p', name: 'Keyed',
          search: ask, calls: async () => raygun.storage.get('calls') ?? 0 }});
        "#
    );
    ExtensionSource::built_in(&[
        (
            "raygun-extension.json",
            &manifest(json!([
                { "key": "apiKey", "type": "text", "label": "API key", "required": true },
                { "key": "region", "type": "text", "default": "" },
            ])),
        ),
        ("index.js", &code),
    ])
}

fn host_with_storage(dir: &tempfile::TempDir) -> ExtensionHost {
    ExtensionHost::start(HostConfig {
        storage_dir: Some(dir.path().to_owned()),
        call_timeout: Duration::from_secs(10),
        lanes: 1,
        ..HostConfig::default()
    })
    .unwrap()
}

fn with_key(key: &str) -> UserSettings {
    UserSettings::from([(ID.to_owned(), [("apiKey".to_owned(), json!(key))].into())])
}

/// How many searches reached the script, ever (its storage outlives reloads).
async fn calls(host: &ExtensionHost) -> serde_json::Value {
    host.call(PROVIDER, "calls", json!({})).await.unwrap()
}

#[test]
fn required_is_for_settings_that_can_be_empty() {
    let parsed = Manifest::parse(&manifest(json!([
        { "key": "apiKey", "type": "text", "required": true },
        { "key": "count", "type": "number", "required": true },
        { "key": "region", "type": "select", "options": ["eu"], "required": false },
    ])))
    .unwrap();
    assert!(parsed.settings[0].required && parsed.settings[1].required);
    assert!(!parsed.settings[2].required);
    // A switch always has a value.
    assert_eq!(
        Manifest::parse(&manifest(json!([
            { "key": "on", "type": "boolean", "required": true },
        ])))
        .unwrap_err(),
        ManifestError::Setting("on".into())
    );
    assert!(matches!(
        Manifest::parse(&manifest(json!([
            { "key": "apiKey", "type": "text", "required": "yes" },
        ]))),
        Err(ManifestError::Json(_))
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_source_without_its_key_is_asked_nothing_until_it_has_one() {
    let (site, hits) = counting_site();
    let dir = tempfile::tempdir().unwrap();
    let host = host_with_storage(&dir);
    let mut changes = host.setup_changes();

    for empty in [UserSettings::new(), with_key("   ")] {
        let infos = host.load_with_settings(vec![keyed(site)], empty).await;
        assert_eq!(
            infos[0].status,
            ExtensionStatus::Loaded,
            "{:?}",
            infos[0].error
        );
        let need = infos[0].needs_setup.clone().expect("needs its key");
        assert_eq!(need.reason, SetupReason::Settings);
        assert_eq!(need.missing, ["API key"]);
        assert!(infos[0].settings[0].required);
        assert!(changes.has_changed().unwrap());
        changes.mark_unchanged();

        // Left out of searches and lookups, listed where sources are listed.
        assert!(host.providers("music.metadata").await.is_empty());
        assert_eq!(host.listed_providers("music.metadata").await.len(), 1);
        assert!(host.search_music("song", 5).await.is_empty());
        // Asked directly, it answers at once without running.
        for _ in 0..4 {
            let error = host
                .call(PROVIDER, "search", json!({ "query": "song", "limit": 5 }))
                .await
                .unwrap_err();
            assert!(error.is_needs_setup(), "{error}");
            assert_eq!(error.kind(), None);
            assert_eq!(
                crate::needs_setup_in(&error.to_string()),
                Some(ID),
                "{error}"
            );
        }
        assert_eq!(host.health().standing(PROVIDER), Standing::Good);
        assert!(host.health().resting(PROVIDER).is_none());
        let waiting = host.sources_needing_setup();
        assert_eq!(waiting.len(), 1);
        assert_eq!(
            (
                waiting[0].extension_id.as_str(),
                waiting[0].kinds.as_slice()
            ),
            (ID, ["music.metadata".to_owned()].as_slice())
        );
    }
    assert_eq!(hits.load(Ordering::SeqCst), 0, "no request was sent");

    // Turned off, it isn't listed as waiting either.
    host.content().set_disabled(&[ID.to_owned()]);
    assert!(host.sources_needing_setup().is_empty());
    host.content().set_disabled(&[]);

    // With its key it's asked like any source.
    let infos = host
        .load_with_settings(vec![keyed(site)], with_key("good"))
        .await;
    assert_eq!(infos[0].needs_setup, None);
    assert!(host.sources_needing_setup().is_empty());
    let found = host.search_music("song", 5).await;
    assert_eq!((found.len(), found[0].error.as_deref()), (1, None));
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert_eq!(calls(&host).await, json!(1), "one search ran, ever");
}

fn test_host() -> ExtensionHost {
    ExtensionHost::start(HostConfig::default()).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_source_whose_key_was_refused_is_asked_again_once_its_settings_change() {
    let (site, hits) = counting_site();
    let dir = tempfile::tempdir().unwrap();
    let host = host_with_storage(&dir);
    host.load_with_settings(vec![keyed(site)], with_key("bad"))
        .await;
    let mut changes = host.setup_changes();
    changes.mark_unchanged();

    // Its answer says the key was refused: a needs-setup outcome, not a failure.
    let refused = host
        .call(PROVIDER, "search", json!({ "query": "song", "limit": 5 }))
        .await
        .unwrap_err();
    assert_eq!(
        refused,
        CallError::NeedsSetup {
            name: "Keyed".into(),
            extension: ID.into(),
            reason: "the API refused the key".into(),
        }
    );
    assert!(changes.has_changed().unwrap(), "the UI hears of it");
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    let need = host.needs_setup(ID).unwrap();
    assert_eq!(need.reason, SetupReason::Refused);
    assert_eq!(need.detail.as_deref(), Some("the API refused the key"));
    assert_eq!(
        host.list().await[0]
            .needs_setup
            .as_ref()
            .map(|need| need.reason),
        Some(SetupReason::Refused)
    );

    // From then on it's asked nothing, and nothing counts against it.
    assert!(host.search_music("song", 5).await.is_empty());
    let again = host
        .call(PROVIDER, "search", json!({ "query": "song", "limit": 5 }))
        .await
        .unwrap_err();
    assert!(again.is_needs_setup());
    assert_eq!(hits.load(Ordering::SeqCst), 1, "not asked again");
    assert_eq!(host.health().standing(PROVIDER), Standing::Good);

    // A reload with the same settings keeps it waiting.
    host.load_with_settings(vec![keyed(site)], with_key("bad"))
        .await;
    assert!(host.provider_needs_setup(PROVIDER));
    assert!(host.providers("music.metadata").await.is_empty());

    // New settings: asked again.
    let infos = host
        .load_with_settings(vec![keyed(site)], with_key("good"))
        .await;
    assert_eq!(infos[0].needs_setup, None);
    let found = host.search_music("song", 5).await;
    assert_eq!(found[0].error, None);
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    assert_eq!(calls(&host).await, json!(2));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_nuclear_plugin_s_settings_show_and_a_password_left_empty_waits() {
    let code = r#"
        __rgNuclear.registerSetting(JSON.stringify({ key: 'server', type: 'text', label: 'Server',
          options: [], default: null, required: false }));
        __rgNuclear.registerSetting(JSON.stringify({ key: 'password', type: 'text',
          label: 'Password', options: [], default: null, required: true }));
        raygun.providers.register({ kind: 'music.metadata', id: 'p', name: 'Subsonic',
          read: async () => raygun.settings.get('password') });
    "#;
    let nuclear = json!({
        "id": ID,
        "name": "Subsonic",
        "version": "1.0.0",
        "apiVersion": 1,
        "runtime": "js",
        "entry": "index.js",
        "provides": ["music.metadata"],
        "nuclear": { "package": "nuclear-plugin-subsonic", "converter": 7 },
    })
    .to_string();
    let source =
        || ExtensionSource::built_in(&[("raygun-extension.json", &nuclear), ("index.js", code)]);
    let host = test_host();
    let infos = host.load(vec![source()]).await;
    assert_eq!(
        infos[0].status,
        ExtensionStatus::Loaded,
        "{:?}",
        infos[0].error
    );
    let labels: Vec<_> = infos[0]
        .settings
        .iter()
        .map(|setting| setting.label.as_str())
        .collect();
    assert_eq!(labels, ["Server", "Password"]);
    assert_eq!(
        infos[0]
            .needs_setup
            .as_ref()
            .map(|need| need.missing.clone()),
        Some(vec!["Password".to_owned()])
    );
    // The value people set on its page reaches the plugin, though its manifest names no setting.
    let settings =
        UserSettings::from([(ID.to_owned(), [("password".to_owned(), json!("pw"))].into())]);
    let infos = host.load_with_settings(vec![source()], settings).await;
    assert_eq!(infos[0].needs_setup, None);
    assert_eq!(
        host.call(PROVIDER, "read", json!({})).await.unwrap(),
        json!("pw")
    );
}
