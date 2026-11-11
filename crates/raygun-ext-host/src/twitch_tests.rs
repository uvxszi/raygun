//! The Twitch extension (`extensions/twitch`) against a local fixture server: the real bundle, its
//! Helix and sign-in addresses pointed at this machine. Answers follow Helix's reference shapes.

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::Response;
use axum::routing::{get, post};
use raygun_model::{ExtensionStatus, FilterValue};
use serde_json::{Value, json};

use crate::fixture_tests::{json_response, query_params, serve};
use crate::manga::MangaList;
use crate::{ExtensionHost, ExtensionSource, HostConfig};

const PROVIDER: &str = "dev.raygun.twitch/twitch";
const CLIENT_ID: &str = "abcdefghijklmnopqrstuvwxyz0123";
const SECRET: &str = "secret0123456789";
const TOKEN: &str = "apptoken42";

/// What the fixture server saw: each request's path and query, and the token it carried.
#[derive(Default)]
struct Seen {
    requests: Vec<(String, HashMap<String, String>, Option<String>)>,
    tokens_made: usize,
    /// Helix answers 401 to this many requests first (an expired token).
    expired: usize,
}

fn stream(login: &str, name: &str, viewers: u64, game: (&str, &str)) -> Value {
    json!({
        "id": format!("4{viewers}"), "user_id": format!("1{viewers}"), "user_login": login,
        "user_name": name, "game_id": game.0, "game_name": game.1, "type": "live",
        "title": format!("{name} live"), "tags": ["English"], "viewer_count": viewers,
        "started_at": "2026-11-11T18:00:00Z", "language": "en",
        "thumbnail_url": format!("https://static-cdn.jtvnw.net/previews-ttv/live_user_{login}-{{width}}x{{height}}.jpg"),
        "is_mature": false,
    })
}

fn helix_answer(path: &str, params: &HashMap<String, String>) -> Value {
    match path {
        "/helix/streams" if params.contains_key("user_id") => match params["user_id"].as_str() {
            "101051819" => {
                json!({ "data": [stream("afrochess", "AfroChess", 12_345, ("743", "Chess"))] })
            }
            _ => json!({ "data": [] }),
        },
        "/helix/streams" => {
            if params.get("game_id").map(String::as_str) == Some("743") {
                return json!({ "data": [stream("afrochess", "AfroChess", 12_345, ("743", "Chess"))], "pagination": {} });
            }
            match params.get("after").map(String::as_str) {
                None => json!({
                    "data": [
                        stream("kaicenat", "KaiCenat", 250_000, ("509658", "Just Chatting")),
                        stream("afrochess", "AfroChess", 12_345, ("743", "Chess")),
                        { "user_login": "not a login", "user_name": "Broken", "id": "1", "user_id": "2" },
                    ],
                    "pagination": { "cursor": "page2" },
                }),
                Some("page2") => json!({
                    "data": [stream("quietcorner", "QuietCorner", 3, ("509658", "Just Chatting"))],
                    "pagination": {},
                }),
                Some(_) => json!({ "data": [] }),
            }
        }
        "/helix/search/channels" => json!({
            "data": [
                {
                    "id": "2", "broadcaster_login": "chessoffline", "display_name": "ChessOffline",
                    "broadcaster_language": "en", "game_id": "743", "game_name": "Chess",
                    "is_live": false, "tags": [], "thumbnail_url": "https://static-cdn.jtvnw.net/jtv_user_pictures/off-300x300.png",
                    "title": "", "started_at": "",
                },
                {
                    "id": "101051819", "broadcaster_login": "afrochess", "display_name": "AfroChess",
                    "broadcaster_language": "en", "game_id": "743", "game_name": "Chess",
                    "is_live": true, "tags": [], "thumbnail_url": "https://static-cdn.jtvnw.net/jtv_user_pictures/afro-300x300.png",
                    "title": "Blitz", "started_at": "2026-11-11T18:00:00Z",
                },
                {
                    "id": "3", "broadcaster_login": "schachde", "display_name": "SchachDE",
                    "broadcaster_language": "de", "game_id": "743", "game_name": "Chess",
                    "is_live": true, "tags": [], "thumbnail_url": "https://static-cdn.jtvnw.net/jtv_user_pictures/de-300x300.png",
                    "title": "Blitz auf Deutsch", "started_at": "2026-11-11T18:30:00Z",
                },
            ],
            "pagination": {},
        }),
        "/helix/users" => match params.get("login").map(String::as_str) {
            Some("afrochess") => json!({ "data": [{
                "id": "101051819", "login": "afrochess", "display_name": "AfroChess",
                "description": "Chess every evening.",
                "profile_image_url": "https://static-cdn.jtvnw.net/jtv_user_pictures/afro-300x300.png",
                "offline_image_url": "",
            }] }),
            _ => json!({ "data": [] }),
        },
        "/helix/channels" => json!({ "data": [{
            "broadcaster_id": "101051819", "broadcaster_login": "afrochess", "game_name": "Chess",
            "title": "Blitz", "tags": ["English", "Educational"],
        }] }),
        "/helix/videos" => json!({ "data": [
            { "id": "335921245", "title": "Sunday blitz", "created_at": "2026-11-08T17:00:00Z", "published_at": "2026-11-08T17:00:05Z", "type": "archive", "duration": "3h8m33s" },
            { "id": "335000000", "title": "Best mates", "created_at": "2026-11-01T17:00:00Z", "type": "highlight", "duration": "10m" },
        ], "pagination": {} }),
        "/helix/clips" => json!({ "data": [
            { "id": "AwkwardHelplessSalamanderSwiftRage", "title": "Queen sacrifice", "created_at": "2026-11-01T20:00:00Z", "view_count": 9000, "duration": 28.5 },
        ] }),
        "/helix/games/top" => json!({ "data": [
            { "id": "509658", "name": "Just Chatting", "box_art_url": "" },
            { "id": "743", "name": "Chess", "box_art_url": "" },
        ] }),
        _ => json!({ "data": [] }),
    }
}

fn fixture_server(seen: Arc<Mutex<Seen>>) -> SocketAddr {
    let token_seen = seen.clone();
    let app = Router::new()
        .route(
            "/oauth2/token",
            post(move |body: String| {
                let seen = token_seen.clone();
                async move {
                    let form: HashMap<String, String> = body
                        .split('&')
                        .filter_map(|pair| pair.split_once('='))
                        .map(|(key, value)| (key.to_owned(), value.to_owned()))
                        .collect();
                    let good = form.get("client_id").map(String::as_str) == Some(CLIENT_ID)
                        && form.get("client_secret").map(String::as_str) == Some(SECRET)
                        && form.get("grant_type").map(String::as_str) == Some("client_credentials");
                    if !good {
                        return json_response(
                            StatusCode::FORBIDDEN,
                            json!({ "status": 403, "message": "invalid client secret" }),
                        );
                    }
                    seen.lock().unwrap().tokens_made += 1;
                    json_response(
                        StatusCode::OK,
                        json!({ "access_token": TOKEN, "expires_in": 5_000_000, "token_type": "bearer" }),
                    )
                }
            }),
        )
        .route(
            "/helix/{*path}",
            get(move |uri: Uri, headers: HeaderMap| {
                let seen = seen.clone();
                async move { helix(&seen, &uri, &headers) }
            }),
        );
    serve(app)
}

fn helix(seen: &Mutex<Seen>, uri: &Uri, headers: &HeaderMap) -> Response {
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    };
    let params = query_params(uri);
    let path = uri.path().to_owned();
    let mut seen = seen.lock().unwrap();
    seen.requests
        .push((path.clone(), params.clone(), header("authorization")));
    if seen.expired > 0 {
        seen.expired -= 1;
        return json_response(
            StatusCode::UNAUTHORIZED,
            json!({ "error": "Unauthorized", "status": 401, "message": "Invalid OAuth token" }),
        );
    }
    if header("client-id").as_deref() != Some(CLIENT_ID)
        || header("authorization").as_deref() != Some(&format!("Bearer {TOKEN}"))
    {
        return json_response(
            StatusCode::UNAUTHORIZED,
            json!({ "error": "Unauthorized", "status": 401, "message": "Client ID and OAuth token do not match" }),
        );
    }
    json_response(StatusCode::OK, helix_answer(&path, &params))
}

/// The Twitch bundle with Helix and the sign-in pointed at `address`, given `settings`.
async fn twitch_host(address: SocketAddr, settings: &[(&str, &str)]) -> ExtensionHost {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions/twitch");
    let mut manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("raygun-extension.json")).unwrap())
            .unwrap();
    manifest["permissions"]["net"] = json!(["127.0.0.1"]);
    let mut code = std::fs::read_to_string(dir.join("dist/index.js")).unwrap();
    for (base, local) in [
        ("https://api.twitch.tv/helix", "/helix"),
        ("https://id.twitch.tv/oauth2/token", "/oauth2/token"),
    ] {
        assert!(code.contains(base), "the bundle doesn't use {base}");
        code = code.replace(base, &format!("http://{address}{local}"));
    }
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host
        .load_with_settings(
            vec![ExtensionSource::built_in(&[
                ("raygun-extension.json", &manifest.to_string()),
                ("dist/index.js", &code),
            ])],
            crate::UserSettings::from([(
                "dev.raygun.twitch".to_owned(),
                settings
                    .iter()
                    .map(|(key, value)| ((*key).to_owned(), json!(value)))
                    .collect::<BTreeMap<_, _>>(),
            )]),
        )
        .await;
    assert_eq!(
        infos[0].status,
        ExtensionStatus::Loaded,
        "{:?}",
        infos[0].error
    );
    host
}

#[tokio::test]
async fn live_streams_list_by_viewers_with_their_channel_and_category() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let address = fixture_server(seen.clone());
    let host = twitch_host(
        address,
        &[("clientId", CLIENT_ID), ("clientSecret", SECRET)],
    )
    .await;

    let first = host
        .manga_list(PROVIDER, MangaList::Popular, 1)
        .await
        .unwrap();
    let listed: Vec<_> = first
        .items
        .iter()
        .map(|item| {
            (
                item.id.as_str(),
                item.subtitle.as_deref(),
                item.live.and_then(|live| live.viewers),
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            ("kaicenat", Some("KaiCenat · Just Chatting"), Some(250_000)),
            ("afrochess", Some("AfroChess · Chess"), Some(12_345)),
        ],
        "a stream without a usable login is left out"
    );
    assert!(first.has_next_page);
    assert!(
        first.items[0]
            .live
            .is_some_and(|live| live.started_at == Some(1_794_420_000_000.0))
    );
    assert!(
        first.items[0]
            .cover_url
            .as_deref()
            .is_some_and(|cover| cover.contains("live_user_kaicenat-640x360.jpg?t="))
    );
    let second = host
        .manga_list(PROVIDER, MangaList::Popular, 2)
        .await
        .unwrap();
    assert_eq!(second.items[0].id, "quietcorner");
    assert!(!second.has_next_page);

    let seen = seen.lock().unwrap();
    assert_eq!(seen.tokens_made, 1, "one token for both pages");
    assert_eq!(
        seen.requests[1].1.get("after").map(String::as_str),
        Some("page2"),
        "the next page by the cursor Twitch gave"
    );
}

#[tokio::test]
async fn searches_find_channels_live_first_and_filters_list_streams() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let address = fixture_server(seen.clone());
    let host = twitch_host(
        address,
        &[("clientId", CLIENT_ID), ("clientSecret", SECRET)],
    )
    .await;

    let found = host.manga_search(PROVIDER, "chess", 1).await.unwrap();
    let ids: Vec<_> = found
        .items
        .iter()
        .map(|item| (item.id.as_str(), item.live.is_some()))
        .collect();
    assert_eq!(
        ids,
        [
            ("afrochess", true),
            ("schachde", true),
            ("chessoffline", false)
        ]
    );

    let english: BTreeMap<String, FilterValue> = BTreeMap::from([
        ("language".to_owned(), FilterValue::Select("en".into())),
        ("liveOnly".to_owned(), FilterValue::Toggle(true)),
    ]);
    let filtered = host
        .manga_search_filtered(PROVIDER, "chess", 1, &english)
        .await
        .unwrap();
    assert_eq!(
        filtered
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["afrochess", "chessoffline"]
    );
    let chess: BTreeMap<String, FilterValue> =
        BTreeMap::from([("category".to_owned(), FilterValue::Select("743".into()))]);
    let streams = host
        .manga_search_filtered(PROVIDER, "", 1, &chess)
        .await
        .unwrap();
    assert_eq!(streams.items.len(), 1);
    assert_eq!(streams.items[0].id, "afrochess");

    let filters = host.manga_filters(PROVIDER).await.unwrap();
    let keys: Vec<_> = filters
        .iter()
        .map(|filter| match filter {
            raygun_model::SourceFilter::Select { key, options, .. } => (key.clone(), options.len()),
            raygun_model::SourceFilter::Toggle { key, .. } => (key.clone(), 0),
            raygun_model::SourceFilter::Tags { key, .. } => (key.clone(), 0),
        })
        .collect();
    assert_eq!(
        keys,
        [
            ("category".to_owned(), 3),
            ("language".to_owned(), 27),
            ("liveOnly".to_owned(), 0),
        ]
    );

    let seen = seen.lock().unwrap();
    let asked = |path: &str| {
        seen.requests
            .iter()
            .filter(|(seen_path, _, _)| seen_path == path)
            .map(|(_, params, _)| params.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        asked("/helix/search/channels")[1]
            .get("live_only")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        asked("/helix/streams")[0]
            .get("game_id")
            .map(String::as_str),
        Some("743")
    );
}

#[tokio::test]
async fn a_channel_page_has_its_stream_videos_and_clips_played_by_twitch() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let address = fixture_server(seen.clone());
    let host = twitch_host(
        address,
        &[("clientId", CLIENT_ID), ("clientSecret", SECRET)],
    )
    .await;

    let details = host.manga_details(PROVIDER, "AfroChess").await.unwrap();
    assert_eq!(details.title, "AfroChess");
    assert_eq!(details.status, "ongoing");
    assert_eq!(details.genres, ["Chess", "English", "Educational"]);
    assert!(
        details.description.as_deref().is_some_and(
            |text| text.starts_with("Live now: AfroChess live · Chess · 12.3K watching")
        )
    );

    let episodes = host.video_episodes(PROVIDER, "afrochess").await.unwrap();
    let names: Vec<_> = episodes
        .iter()
        .map(|episode| (episode.id.as_str(), episode.name.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            ("live:afrochess", "Live now: AfroChess live"),
            ("video:335921245", "Sunday blitz"),
            ("video:335000000", "Highlight: Best mates"),
            (
                "clip:AwkwardHelplessSalamanderSwiftRage",
                "Clip: Queen sacrifice"
            ),
        ]
    );

    let servers = host
        .video_servers(PROVIDER, "live:afrochess")
        .await
        .unwrap();
    let videos = servers[0].videos.as_ref().unwrap();
    let pages: Vec<_> = videos
        .iter()
        .map(|video| (video.url.as_str(), video.format.as_deref(), video.live))
        .collect();
    assert_eq!(
        pages,
        [
            ("https://www.twitch.tv/afrochess", Some("page"), true),
            (
                "https://player.twitch.tv/?channel=afrochess&parent=twitch.tv&player=popout",
                Some("page"),
                true
            ),
        ],
        "Twitch's own player, as its developer agreement requires"
    );
    let missing = host.manga_details(PROVIDER, "nobody").await.unwrap_err();
    assert!(missing.contains("no channel called nobody"), "{missing}");
}

#[tokio::test]
async fn keys_are_asked_for_and_an_expired_token_is_replaced() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let address = fixture_server(seen.clone());

    // Without keys nothing is asked.
    let keyless = twitch_host(address, &[]).await;
    let error = keyless
        .manga_list(PROVIDER, MangaList::Popular, 1)
        .await
        .unwrap_err();
    assert_eq!(
        crate::needs_setup_in(&error),
        Some("dev.raygun.twitch"),
        "{error}"
    );
    assert!(seen.lock().unwrap().requests.is_empty());

    // Keys Twitch refuses: the source needs setup.
    let refused = twitch_host(
        address,
        &[("clientId", CLIENT_ID), ("clientSecret", "wrong")],
    )
    .await;
    let error = refused
        .manga_list(PROVIDER, MangaList::Popular, 1)
        .await
        .unwrap_err();
    assert_eq!(
        crate::needs_setup_in(&error),
        Some("dev.raygun.twitch"),
        "{error}"
    );

    // An expired token: a new one, then the request again.
    seen.lock().unwrap().expired = 1;
    let host = twitch_host(
        address,
        &[("clientId", CLIENT_ID), ("clientSecret", SECRET)],
    )
    .await;
    let listed = host
        .manga_list(PROVIDER, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_eq!(listed.items.len(), 2);
    assert_eq!(seen.lock().unwrap().tokens_made, 2);
}
