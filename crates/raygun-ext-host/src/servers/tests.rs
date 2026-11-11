//! Connected servers against stand-ins of their APIs (answers shaped as the servers' own, with
//! public-domain titles), through the host as the app uses them; and `#[ignore]` live checks
//! against public demo servers.

use std::collections::HashMap;
use std::net::SocketAddr;

use axum::Router;
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde_json::{Value, json};

use super::*;
use crate::manga::MangaList;
use crate::{ExtensionHost, HostConfig, ServerSecrets};

fn params(uri: &Uri) -> HashMap<String, String> {
    url::form_urlencoded::parse(uri.query().unwrap_or_default().as_bytes())
        .into_owned()
        .collect()
}

fn json_answer(body: Value) -> Response {
    (
        [(header::CONTENT_TYPE, "application/json")],
        body.to_string(),
    )
        .into_response()
}

fn xml_answer(body: &str) -> Response {
    (
        [(
            header::CONTENT_TYPE,
            "application/atom+xml;profile=opds-catalog",
        )],
        body.to_owned(),
    )
        .into_response()
}

fn header_text(headers: &HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}

/// Serves `app` on a free local port from its own thread.
fn serve(app: Router) -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            axum::serve(listener, app).await.unwrap();
        });
    });
    address
}

/// Installs a connection's folder and loads it, with its secret, into a new host.
async fn connected_host(connected: &Connected) -> (ExtensionHost, tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let verified = package(&connected.config).unwrap();
    let id = verified.manifest.id.clone();
    let folder = crate::package::install(dir.path(), &verified).unwrap();
    let host = ExtensionHost::start(HostConfig {
        lanes: 1,
        ..HostConfig::default()
    })
    .unwrap();
    let mut secrets = ServerSecrets::new();
    if let Some(secret) = &connected.secret {
        secrets.insert(id.clone(), secret.clone());
    }
    let infos = host
        .load_with(
            vec![crate::ExtensionSource::installed(folder)],
            Default::default(),
            secrets,
        )
        .await;
    assert_eq!(infos.len(), 1);
    assert_eq!(
        infos[0].status,
        raygun_model::ExtensionStatus::Loaded,
        "{:?}",
        infos[0].error
    );
    (host, dir, id)
}

fn input(kind: ServerKind, address: String, username: &str, secret: &str) -> ConnectInput {
    ConnectInput {
        kind: Some(kind),
        address,
        username: username.into(),
        secret: secret.into(),
        api_key: false,
    }
}

fn query(title: &str, artist: &str) -> raygun_model::TrackQuery {
    raygun_model::TrackQuery {
        title: title.into(),
        artist: Some(artist.into()),
        album: None,
        duration_ms: None,
    }
}

// ---- Jellyfin ----

fn jellyfin_signed(headers: &HeaderMap) -> bool {
    header_text(headers, "authorization").contains("Token=\"tok-1\"")
}

fn jellyfin_items(uri: &Uri, address: SocketAddr) -> Value {
    let query = params(uri);
    let _ = address;
    match query.get("IncludeItemTypes").map(String::as_str) {
        Some("Audio") => json!({ "Items": [{
            "Id": "a1", "Name": "Thunderdays", "Type": "Audio", "Artists": ["Binaerpilot"],
            "Album": "Promo", "AlbumId": "al1", "AlbumPrimaryImageTag": "t1",
            "RunTimeTicks": 1_416_881_630_i64, "HasLyrics": true,
            "MediaSources": [{ "Id": "a1", "Container": "mp3", "Bitrate": 320_000 }],
        }], "TotalRecordCount": 1 }),
        Some("MusicAlbum") => json!({ "Items": [{
            "Id": "al1", "Name": "Promo", "Type": "MusicAlbum", "AlbumArtist": "Binaerpilot",
            "ProductionYear": 2006, "ChildCount": 3, "ImageTags": { "Primary": "t1" },
        }], "TotalRecordCount": 1 }),
        _ if query.get("ParentId").map(String::as_str) == Some("al1") => json!({ "Items": [{
            "Id": "a1", "Name": "Thunderdays", "Type": "Audio", "Artists": ["Binaerpilot"],
            "Album": "Promo", "AlbumId": "al1", "RunTimeTicks": 1_416_881_630_i64,
        }], "TotalRecordCount": 1 }),
        _ => {
            assert_eq!(
                query.get("IncludeItemTypes").map(String::as_str),
                Some("Movie,Series")
            );
            json!({ "Items": [
                { "Id": "m1", "Name": "Night of the Living Dead", "Type": "Movie",
                  "ProductionYear": 1968, "ImageTags": { "Primary": "p1" } },
                { "Id": "s1", "Name": "Pioneer One", "Type": "Series", "ImageTags": {} },
            ], "TotalRecordCount": 3 })
        }
    }
}

fn jellyfin_item(id: &str) -> Value {
    match id {
        "m1" => json!({
            "Id": "m1", "Name": "Night of the Living Dead", "Type": "Movie", "ProductionYear": 1968,
            "Overview": "Strangers trapped in a farmhouse.", "Genres": ["Horror"],
            "People": [{ "Name": "Duane Jones", "Type": "Actor" }, { "Name": "George A. Romero", "Type": "Director" }],
            "ImageTags": { "Primary": "p1" },
            "MediaSources": [{ "Id": "m1", "Name": "NOTLD", "Container": "mp4", "MediaStreams": [
                { "Type": "Video", "Index": 0, "Height": 480, "Codec": "h264" },
                { "Type": "Audio", "Index": 1, "Codec": "aac" },
                { "Type": "Subtitle", "Index": 2, "Codec": "srt", "Language": "eng", "DisplayTitle": "English",
                  "IsExternal": true, "IsTextSubtitleStream": true },
                { "Type": "Subtitle", "Index": 3, "Codec": "subrip", "Language": "fre", "DisplayTitle": "French",
                  "IsExternal": false, "IsTextSubtitleStream": true },
            ] }],
            "Chapters": [{ "StartPositionTicks": 0, "Name": "Start" }],
        }),
        "s1" => {
            json!({ "Id": "s1", "Name": "Pioneer One", "Type": "Series", "Status": "Ended", "ImageTags": {} })
        }
        "al1" => {
            json!({ "Id": "al1", "Name": "Promo", "Type": "MusicAlbum", "AlbumArtist": "Binaerpilot", "ImageTags": {} })
        }
        _ => json!({ "Id": id, "Name": "?", "Type": "Episode" }),
    }
}

fn jellyfin_app(address: std::sync::Arc<std::sync::OnceLock<SocketAddr>>) -> Router {
    let signed = |headers: &HeaderMap| -> Option<Response> {
        (!jellyfin_signed(headers)).then(|| StatusCode::UNAUTHORIZED.into_response())
    };
    let items_address = address.clone();
    Router::new()
        .route(
            "/jf/System/Info/Public",
            get(|| async { json_answer(json!({ "ServerName": "Home Media", "ProductName": "Jellyfin Server", "Version": "10.11.0" })) }),
        )
        .route(
            "/jf/Users/AuthenticateByName",
            post(|headers: HeaderMap, body: String| async move {
                let sent: Value = serde_json::from_str(&body).unwrap_or_default();
                let client = header_text(&headers, "authorization");
                if client.contains("Client=\"Raygun\"") && sent["Username"] == "viewer" && sent["Pw"] == "secret" {
                    json_answer(json!({ "AccessToken": "tok-1", "User": { "Id": "user1" } }))
                } else {
                    StatusCode::UNAUTHORIZED.into_response()
                }
            }),
        )
        .route(
            "/jf/Items",
            get(move |headers: HeaderMap, uri: Uri| async move {
                if let Some(refused) = signed(&headers) {
                    return refused;
                }
                assert_eq!(params(&uri).get("userId").map(String::as_str), Some("user1"));
                json_answer(jellyfin_items(&uri, *items_address.get().unwrap()))
            }),
        )
        .route(
            "/jf/Items/{id}",
            get(move |headers: HeaderMap, axum::extract::Path(id): axum::extract::Path<String>| async move {
                if let Some(refused) = signed(&headers) {
                    return refused;
                }
                json_answer(jellyfin_item(&id))
            }),
        )
        .route(
            "/jf/Shows/{id}/Episodes",
            get(|| async {
                json_answer(json!({ "Items": [
                    { "Id": "e1", "Name": "Earthfall", "Type": "Episode", "IndexNumber": 1, "ParentIndexNumber": 1,
                      "PremiereDate": "2010-06-16T00:00:00.0000000Z" },
                    { "Id": "e2", "Name": "The Man From Orbit", "Type": "Episode", "IndexNumber": 2, "ParentIndexNumber": 1 },
                ] }))
            }),
        )
        .route(
            "/jf/MediaSegments/{id}",
            get(|| async { json_answer(json!({ "Items": [{ "Type": "Intro", "StartTicks": 0, "EndTicks": 900_000_000 }] })) }),
        )
        .route(
            "/jf/Audio/{id}/Lyrics",
            get(|| async {
                json_answer(json!({ "Lyrics": [{ "Text": "Line one", "Start": 10_000_000 }, { "Text": "Line two", "Start": 50_000_000 }] }))
            }),
        )
}

#[tokio::test(flavor = "multi_thread")]
async fn jellyfin_serves_films_series_and_music() {
    let slot = std::sync::Arc::new(std::sync::OnceLock::new());
    let address = serve(jellyfin_app(slot.clone()));
    slot.set(address).unwrap();
    let base = format!("http://{address}/jf");

    let refused = connect(&input(
        ServerKind::Jellyfin,
        base.clone(),
        "viewer",
        "wrong",
    ))
    .await;
    assert_eq!(refused.unwrap_err(), "The user name or password is wrong");
    let connected = connect(&input(
        ServerKind::Jellyfin,
        format!("{base}/"),
        "viewer",
        "secret",
    ))
    .await
    .unwrap();
    assert_eq!(connected.config.name, "Home Media");
    assert_eq!(connected.config.url, base);
    assert_eq!(connected.config.user_id.as_deref(), Some("user1"));
    assert_eq!(connected.config.secret, SecretKind::Token);
    assert_eq!(
        connected.secret.as_deref(),
        Some("tok-1"),
        "the token is kept, not the password"
    );

    let (host, _dir, id) = connected_host(&connected).await;
    let videos = format!("{id}/videos");
    let popular = host
        .manga_list(&videos, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_eq!(popular.items.len(), 2);
    assert!(popular.has_next_page, "3 titles, 2 shown");
    let cover = popular.items[0].cover_url.clone().unwrap();
    assert!(cover.starts_with(&format!("{base}/Items/m1/Images/Primary?tag=p1")));
    assert!(
        raygun_net::server_auth::is_signed(&cover),
        "covers fetched later sign in"
    );

    let details = host.manga_details(&videos, "m1").await.unwrap();
    assert_eq!(details.media_type.as_deref(), Some("movie"));
    assert_eq!(details.year, Some(1968));
    assert_eq!(details.genres, ["Horror"]);
    let episodes = host.video_episodes(&videos, "s1").await.unwrap();
    assert_eq!(episodes.len(), 2);
    assert_eq!(episodes[0].name, "Earthfall");
    let film = host.video_episodes(&videos, "m1").await.unwrap();
    assert_eq!(film.len(), 1, "a film is its one episode");

    let servers = host.video_servers(&videos, "m1").await.unwrap();
    assert_eq!(servers.len(), 2);
    let original = &servers[0].videos.as_ref().unwrap()[0];
    assert!(
        original
            .url
            .starts_with(&format!("{base}/Videos/m1/stream?static=true"))
    );
    assert!(original.headers["Authorization"].contains("Token=\"tok-1\""));
    assert_eq!(
        original.subtitles.len(),
        1,
        "the file carries its own; external ones are added"
    );
    let converted = &servers[1].videos.as_ref().unwrap()[0];
    assert!(
        converted
            .url
            .contains("/Videos/m1/master.m3u8?MediaSourceId=m1")
    );
    assert_eq!(converted.subtitles.len(), 2);
    assert!(
        converted.subtitles[0]
            .url
            .ends_with("/Videos/m1/m1/Subtitles/2/0/Stream.vtt")
    );

    let found = host.search_music("Thunder", 10).await;
    assert_eq!(found.len(), 1);
    let tracks = &found[0].tracks;
    assert_eq!(tracks[0].title, "Thunderdays");
    assert_eq!(tracks[0].artist.as_deref(), Some("Binaerpilot"));
    assert_eq!(tracks[0].duration_ms, Some(141_688));
    assert!(
        tracks[0].source.is_none(),
        "songs play through the stream provider"
    );

    let candidates = host
        .stream_candidates(
            &query("Thunderdays", "Binaerpilot"),
            10,
            raygun_resolver::QualityTarget::Any,
        )
        .await;
    let candidate = &candidates[0].result.as_ref().unwrap()[0];
    assert_eq!(
        (candidate.id.as_str(), candidate.bitrate_kbps),
        ("a1", Some(320))
    );
    let stream = host
        .resolve_stream(&format!("{id}/streams"), "a1", crate::StreamUse::default())
        .await
        .unwrap();
    let raygun_model::MediaSource::Url { url, headers } = stream.source else {
        panic!("a web stream");
    };
    assert!(url.starts_with(&format!("{base}/Audio/a1/universal?UserId=user1")));
    assert!(headers["Authorization"].contains("Token=\"tok-1\""));
    assert!(stream.expires_at.is_some());

    let lyrics = host
        .lyrics(&query("Thunderdays", "Binaerpilot"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(lyrics.synced.len(), 2);
    assert_eq!(lyrics.synced[1].time_ms, 5000);

    // Gone with the connection: its sign-in no longer reaches the server.
    host.load_with(Vec::new(), Default::default(), ServerSecrets::new())
        .await;
    assert!(!raygun_net::server_auth::is_signed(&cover));
}

// ---- Subsonic ----

fn subsonic_app() -> Router {
    let ok = |body: Value| {
        let mut response = json!({ "status": "ok", "version": "1.16.1", "type": "navidrome", "openSubsonic": true });
        if let (Some(response), Some(body)) = (response.as_object_mut(), body.as_object()) {
            response.extend(body.clone());
        }
        json_answer(json!({ "subsonic-response": response }))
    };
    Router::new().route(
        "/music/rest/{method}",
        get(move |axum::extract::Path(method): axum::extract::Path<String>, uri: Uri| async move {
            let query = params(&uri);
            let salt = query.get("s").cloned().unwrap_or_default();
            let token: String = md5::Md5::digest(format!("sesame{salt}").as_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            if query.get("u").map(String::as_str) != Some("listener") || query.get("t") != Some(&token) {
                return json_answer(json!({ "subsonic-response": { "status": "failed", "version": "1.16.1",
                    "error": { "code": 40, "message": "Wrong username or password" } } }));
            }
            let song = json!({ "id": "tr-7", "title": "Moonlight Sonata", "artist": "Ludwig van Beethoven",
                "album": "Piano Sonatas", "albumId": "al-3", "coverArt": "al-3", "duration": 360,
                "bitRate": 1411, "suffix": "flac" });
            match method.as_str() {
                "ping.view" => ok(json!({})),
                "search3.view" => ok(json!({ "searchResult3": {
                    "song": [song],
                    "album": [{ "id": "al-3", "name": "Piano Sonatas", "artist": "Ludwig van Beethoven",
                        "year": 1802, "songCount": 1, "coverArt": "al-3" }],
                    "artist": [{ "id": "ar-1", "name": "Ludwig van Beethoven" }],
                } })),
                "getAlbum.view" => ok(json!({ "album": { "id": "al-3", "name": "Piano Sonatas", "song": [song] } })),
                "getSong.view" => ok(json!({ "song": song })),
                "getLyricsBySongId.view" => ok(json!({ "lyricsList": { "structuredLyrics": [{
                    "synced": true, "offset": 0, "line": [{ "start": 0, "value": "(instrumental)" }] }] } })),
                _ => json_answer(json!({ "subsonic-response": { "status": "failed", "error": { "code": 70, "message": "not found" } } })),
            }
        }),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn subsonic_searches_streams_and_finds_lyrics() {
    let address = serve(subsonic_app());
    let base = format!("http://{address}/music");
    let refused = connect(&input(
        ServerKind::Subsonic,
        base.clone(),
        "listener",
        "wrong",
    ))
    .await;
    assert_eq!(refused.unwrap_err(), "The user name or password is wrong");
    let connected = connect(&input(
        ServerKind::Subsonic,
        base.clone(),
        "listener",
        "sesame",
    ))
    .await
    .unwrap();
    assert_eq!(connected.config.secret, SecretKind::Password);
    let (host, _dir, id) = connected_host(&connected).await;

    let found = host.search_music("moonlight", 10).await;
    let track = &found[0].tracks[0];
    assert_eq!(track.title, "Moonlight Sonata");
    assert_eq!(track.duration_ms, Some(360_000));
    let art = track.artwork_url.clone().unwrap();
    assert_eq!(
        art,
        format!("{base}/rest/getCoverArt.view?id=al-3&size=600"),
        "stored without the sign-in"
    );
    let (signed, _) = raygun_net::server_auth::authorize(&art, &Default::default());
    assert!(
        signed.contains("u=listener") && signed.contains("&t="),
        "fetched with it"
    );

    let albums = host.search_albums("sonatas", 5).await;
    assert_eq!(albums[0].title, "Piano Sonatas");
    let album = host
        .music_album(&albums[0].provider.id, &albums[0].id)
        .await
        .unwrap();
    assert_eq!(
        (album.album.title.as_str(), album.tracks.len()),
        ("Piano Sonatas", 1)
    );

    let stream = host
        .resolve_stream(
            &format!("{id}/streams"),
            "tr-7",
            crate::StreamUse {
                quality: raygun_resolver::QualityTarget::Kbps(160),
                decodable: false,
            },
        )
        .await
        .unwrap();
    let raygun_model::MediaSource::Url { url, .. } = stream.source else {
        panic!("a web stream");
    };
    assert!(url.starts_with(&format!(
        "{base}/rest/stream.view?id=tr-7&maxBitRate=160&format=raw&u=listener"
    )));

    let lyrics = host
        .lyrics(&query("Moonlight Sonata", "Beethoven"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(lyrics.synced[0].text, "(instrumental)");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_without_a_sign_in_that_works_is_asked_nothing() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let hits = std::sync::Arc::new(AtomicUsize::new(0));
    let counted = hits.clone();
    let app = subsonic_app().layer(axum::middleware::from_fn(
        move |request: axum::extract::Request, next: axum::middleware::Next| {
            let counted = counted.clone();
            async move {
                counted.fetch_add(1, Ordering::SeqCst);
                next.run(request).await
            }
        },
    ));
    let base = format!("http://{}/music", serve(app));
    let connected = connect(&input(
        ServerKind::Subsonic,
        base.clone(),
        "listener",
        "sesame",
    ))
    .await
    .unwrap();

    // Raygun lost the sign-in: the connection loads, waits to be signed in again, and its
    // providers are left out of searches without a request.
    let signed_out = Connected {
        secret: None,
        ..connected.clone()
    };
    let (host, _dir, id) = connected_host(&signed_out).await;
    let before = hits.load(Ordering::SeqCst);
    let need = host.needs_setup(&id).expect("needs signing in");
    assert_eq!(need.reason, raygun_model::SetupReason::SignIn);
    assert!(host.search_music("moonlight", 10).await.is_empty());
    let unasked = host
        .call(
            &format!("{id}/music"),
            "search",
            json!({ "query": "x", "limit": 5 }),
        )
        .await
        .unwrap_err();
    assert!(unasked.is_needs_setup(), "{unasked}");
    assert_eq!(hits.load(Ordering::SeqCst), before, "nothing was sent");
    assert_eq!(host.sources_needing_setup()[0].extension_id, id);

    // A sign-in the server refuses (wrong user or password): asked once, then not again.
    let refused = Connected {
        secret: Some("wrong".into()),
        ..connected
    };
    let (host, _dir, id) = connected_host(&refused).await;
    assert!(
        host.needs_setup(&id).is_none(),
        "nothing known before asking"
    );
    let before = hits.load(Ordering::SeqCst);
    let found = host.search_music("moonlight", 10).await;
    assert!(
        found[0]
            .error
            .as_deref()
            .is_some_and(|error| crate::needs_setup_in(error) == Some(id.as_str())),
        "{found:?}"
    );
    assert_eq!(hits.load(Ordering::SeqCst), before + 1);
    assert_eq!(
        host.needs_setup(&id).map(|need| need.reason),
        Some(raygun_model::SetupReason::Refused)
    );
    assert!(host.search_music("moonlight", 10).await.is_empty());
    assert_eq!(hits.load(Ordering::SeqCst), before + 1, "not asked again");
    assert_eq!(
        host.health().standing(&format!("{id}/music")),
        crate::fanout::Standing::Good,
        "not counted as failing"
    );
}

// ---- Audiobookshelf ----

fn abs_app() -> Router {
    let signed = |headers: &HeaderMap| header_text(headers, "authorization") == "Bearer access-1";
    let book = json!({ "id": "li_1", "libraryId": "lib1", "mediaType": "book", "media": {
        "metadata": { "title": "The Time Machine", "authorName": "H. G. Wells", "language": "English",
            "description": "A scientist travels to the year 802,701." },
        "coverPath": "/metadata/items/li_1/cover.jpg", "duration": 12_745.0, "numAudioFiles": 2 } });
    let ebook = json!({ "id": "li_2", "mediaType": "book", "media": {
        "metadata": { "title": "Only an ebook" }, "duration": 0, "numAudioFiles": 0 } });
    let (list, found) = (
        json!({ "results": [book.clone(), ebook], "total": 2 }),
        json!({ "book": [{ "libraryItem": book.clone() }] }),
    );
    let item = {
        let mut item = book.clone();
        item["media"]["audioFiles"] = json!([
            { "index": 2, "ino": "222", "duration": 6000.5, "mimeType": "audio/mpeg",
              "metadata": { "filename": "02 - The Time Traveller Returns.mp3" } },
            { "index": 1, "ino": "111", "duration": 6744.5, "mimeType": "audio/mpeg",
              "metadata": { "filename": "01 - Introduction.mp3" }, "metaTags": { "tagTitle": "Introduction" } },
            { "index": 3, "ino": "333", "exclude": true, "metadata": { "filename": "notes.mp3" } },
        ]);
        item
    };
    Router::new()
        .route(
            "/abs/login",
            post(|headers: HeaderMap, body: String| async move {
                let sent: Value = serde_json::from_str(&body).unwrap_or_default();
                if sent["username"] == "reader" && sent["password"] == "pw" && header_text(&headers, "x-return-tokens") == "true" {
                    json_answer(json!({ "user": { "accessToken": "access-1", "refreshToken": "refresh-1" } }))
                } else {
                    StatusCode::UNAUTHORIZED.into_response()
                }
            }),
        )
        .route(
            "/abs/api/me",
            get(move |headers: HeaderMap| async move {
                if signed(&headers) { json_answer(json!({ "username": "reader" })) } else { StatusCode::UNAUTHORIZED.into_response() }
            }),
        )
        .route(
            "/abs/api/libraries",
            get(move |headers: HeaderMap| async move {
                if !signed(&headers) {
                    return StatusCode::UNAUTHORIZED.into_response();
                }
                json_answer(json!({ "libraries": [
                    { "id": "lib1", "name": "Books", "mediaType": "book" },
                    { "id": "lib2", "name": "Podcasts", "mediaType": "podcast" },
                ] }))
            }),
        )
        .route(
            "/abs/api/libraries/{id}/items",
            get(move |headers: HeaderMap, uri: Uri| {
                let list = list.clone();
                async move {
                    if !signed(&headers) {
                        return StatusCode::UNAUTHORIZED.into_response();
                    }
                    assert_eq!(params(&uri).get("sort").map(String::as_str), Some("addedAt"));
                    json_answer(list)
                }
            }),
        )
        .route(
            "/abs/api/libraries/{id}/search",
            get(move || {
                let found = found.clone();
                async move { json_answer(found) }
            }),
        )
        .route(
            "/abs/api/items/{id}",
            get(move |headers: HeaderMap| {
                let item = item.clone();
                async move {
                    if signed(&headers) { json_answer(item) } else { StatusCode::UNAUTHORIZED.into_response() }
                }
            }),
        )
        .route(
            "/abs/auth/refresh",
            post(|headers: HeaderMap| async move {
                if header_text(&headers, "x-refresh-token") == "refresh-1" {
                    json_answer(json!({ "user": { "accessToken": "access-1", "refreshToken": "refresh-2" } }))
                } else {
                    StatusCode::UNAUTHORIZED.into_response()
                }
            }),
        )
}

#[tokio::test(flavor = "multi_thread")]
async fn audiobookshelf_lists_books_and_their_files() {
    let address = serve(abs_app());
    let base = format!("http://{address}/abs");
    let connected = connect(&input(
        ServerKind::Audiobookshelf,
        base.clone(),
        "reader",
        "pw",
    ))
    .await
    .unwrap();
    assert_eq!(
        connected.secret.as_deref(),
        Some("pw"),
        "the password, to sign in again"
    );
    let (host, _dir, id) = connected_host(&connected).await;
    let books = format!("{id}/books");

    let page = host.audiobooks(&books, "popular", "", 1).await.unwrap();
    assert_eq!(page.books.len(), 1, "books with no audio stay out");
    assert_eq!(page.books[0].title, "The Time Machine");
    assert_eq!(page.books[0].runtime.as_deref(), Some("3:32:25"));
    let cover = page.books[0].artwork_url.clone().unwrap();
    assert_eq!(cover, format!("{base}/api/items/li_1/cover"));
    let searched = host.audiobooks(&books, "search", "time", 1).await.unwrap();
    assert_eq!(searched.books.len(), 1);

    let details = host.audiobook_details(&books, "li_1").await.unwrap();
    let titles: Vec<&str> = details
        .chapters
        .iter()
        .map(|chapter| chapter.title.as_str())
        .collect();
    assert_eq!(
        titles,
        ["Introduction", "02 - The Time Traveller Returns"],
        "in order, excluded files out"
    );
    let audio = &details.chapters[0].audio_url;
    assert_eq!(audio, &format!("{base}/api/items/li_1/file/111"));
    let (_, headers) = raygun_net::server_auth::authorize(audio, &Default::default());
    assert_eq!(
        headers,
        [("Authorization".to_owned(), "Bearer access-1".to_owned())],
        "streams sign in"
    );
}

// ---- OPDS ----

const OPDS_ROOT: &str = r#"<feed xmlns="http://www.w3.org/2005/Atom"><id>root</id><title>Comic Shelf</title>
<link type="application/opensearchdescription+xml" rel="search" href="/opds/v1.2/search"/>
<entry><title>Keep Reading</title><id>keep</id><link type="application/atom+xml;profile=opds-catalog;kind=navigation" rel="subsection" href="/opds/v1.2/keep-reading"/></entry>
<entry><title>All series</title><id>all</id><link type="application/atom+xml;profile=opds-catalog;kind=navigation" rel="subsection" href="/opds/v1.2/series"/></entry>
<entry><title>Latest series</title><id>latest</id><link type="application/atom+xml;profile=opds-catalog;kind=navigation" rel="subsection" href="/opds/v1.2/series/latest"/></entry>
</feed>"#;

const OPDS_SERIES_LIST: &str = r#"<feed xmlns="http://www.w3.org/2005/Atom"><id>allSeries</id><title>All series</title>
<link rel="next" type="application/atom+xml;profile=opds-catalog;kind=navigation" href="/opds/v1.2/series?page=1"/>
<entry><title>Space Adventures</title><id>63</id><link type="application/atom+xml;profile=opds-catalog;kind=navigation" rel="subsection" href="/opds/v1.2/series/63"/></entry>
<entry><title>The Time Machine</title><id>70</id><link type="application/atom+xml;profile=opds-catalog;kind=navigation" rel="subsection" href="/opds/v1.2/series/70"/></entry>
</feed>"#;

const OPDS_BOOK_SERIES: &str = r#"<feed xmlns="http://www.w3.org/2005/Atom"><id>70</id><title>The Time Machine</title>
<entry><title>The Time Machine</title><id>71</id><author><name>H. G. Wells</name></author>
<content type="xhtml"><div xmlns="http://www.w3.org/1999/xhtml"><p>A scientist travels</p><p>to the year 802,701.</p></div></content>
<link type="application/epub+zip" rel="http://opds-spec.org/acquisition" length="900000" href="/opds/v1.2/books/71/file/images.epub"/>
<link type="application/epub+zip" rel="http://opds-spec.org/acquisition" length="1200" href="/opds/v1.2/books/71/file/book.epub"/></entry>
</feed>"#;

const OPDS_SERIES: &str = r#"<feed xmlns="http://www.w3.org/2005/Atom"><id>63</id><title>Space Adventures</title>
<entry><title>Space Adventures 01</title><updated>2025-12-15T11:36:12Z</updated><id>64</id>
<link type="image/jpeg" rel="http://opds-spec.org/image/thumbnail" href="/opds/v1.2/books/64/thumbnail/small"/>
<link href="/opds/v1.2/books/64/pages/{pageNumber}" xmlns:pse="http://vaemendis.net/opds-pse/ns" pse:count="3" type="image/jpeg" rel="http://vaemendis.net/opds-pse/stream"/></entry>
<entry><title>Space Adventures 02</title><id>65</id>
<link href="/opds/v1.2/books/65/pages/{pageNumber}?zero_based=true" xmlns:pse="http://vaemendis.net/opds-pse/ns" pse:count="2" type="image/jpeg" rel="http://vaemendis.net/opds-pse/stream"/></entry>
</feed>"#;

fn opds_app() -> Router {
    let signed = |headers: &HeaderMap| {
        header_text(headers, "authorization")
            == super::opds::basic_auth("demo@example.org", "komga-demo")
    };
    let answer = move |headers: HeaderMap, body: &'static str| {
        if signed(&headers) {
            xml_answer(body)
        } else {
            StatusCode::UNAUTHORIZED.into_response()
        }
    };
    Router::new()
        .route("/opds/v1.2/catalog", get(move |headers: HeaderMap| async move { answer(headers, OPDS_ROOT) }))
        .route(
            "/opds/v1.2/series",
            get(move |headers: HeaderMap, uri: Uri| async move {
                if params(&uri).contains_key("search") {
                    answer(headers, OPDS_SERIES_LIST)
                } else if params(&uri).contains_key("page") {
                    answer(headers, r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>All series</title></feed>"#)
                } else {
                    answer(headers, OPDS_SERIES_LIST)
                }
            }),
        )
        .route("/opds/v1.2/series/63", get(move |headers: HeaderMap| async move { answer(headers, OPDS_SERIES) }))
        .route("/opds/v1.2/series/70", get(move |headers: HeaderMap| async move { answer(headers, OPDS_BOOK_SERIES) }))
        .route(
            "/opds/v1.2/books/71/file/book.epub",
            get(move |headers: HeaderMap| async move {
                if signed(&headers) {
                    ([(header::CONTENT_TYPE, "application/epub+zip")], super::epub::tests::sample_book()).into_response()
                } else {
                    StatusCode::UNAUTHORIZED.into_response()
                }
            }),
        )
        .route(
            "/opds/v1.2/search",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "application/opensearchdescription+xml")],
                    r#"<OpenSearchDescription xmlns="http://a9.com/-/spec/opensearch/1.1/"><ShortName>Search</ShortName><Url template="/opds/v1.2/series?search={searchTerms}" type="application/atom+xml;profile=opds-catalog;kind=acquisition"/></OpenSearchDescription>"#,
                )
                    .into_response()
            }),
        )
}

#[tokio::test(flavor = "multi_thread")]
async fn opds_catalogues_stream_comics_page_by_page() {
    let address = serve(opds_app());
    let server = format!("http://{address}");
    let refused = connect(&input(ServerKind::Opds, server.clone(), "", "")).await;
    assert_eq!(
        refused.unwrap_err(),
        "This catalogue needs a user name and password"
    );
    // The server's address is enough: the catalogue is at Komga's well-known path.
    let connected = connect(&input(
        ServerKind::Opds,
        server.clone(),
        "demo@example.org",
        "komga-demo",
    ))
    .await
    .unwrap();
    assert_eq!(connected.config.url, format!("{server}/opds/v1.2/catalog"));
    assert_eq!(connected.config.name, "Comic Shelf");
    let (host, _dir, id) = connected_host(&connected).await;
    let comics = format!("{id}/comics");

    let popular = host
        .manga_list(&comics, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_eq!(
        popular.items[0].title, "Space Adventures",
        "All series is the browse list"
    );
    assert!(popular.has_next_page);
    assert!(
        host.manga_list(&comics, MangaList::Popular, 2)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    let series = popular.items[0].id.clone();
    assert_eq!(series, format!("feed:{server}/opds/v1.2/series/63"));
    let searched = host.manga_search(&comics, "space", 1).await.unwrap();
    assert_eq!(searched.items.len(), 2, "the template's results");

    let details = host.manga_details(&comics, &series).await.unwrap();
    assert_eq!(details.title, "Space Adventures");
    let chapters = host.manga_chapters(&comics, &series).await.unwrap();
    assert_eq!(chapters.len(), 2);
    let pages = host.manga_pages(&comics, &chapters[0].id).await.unwrap();
    let urls: Vec<&str> = pages.iter().map(|page| page.url.as_str()).collect();
    assert_eq!(
        urls,
        (0..3)
            .map(|n| format!("{server}/opds/v1.2/books/64/pages/{n}"))
            .collect::<Vec<_>>()
    );
    let (_, headers) = raygun_net::server_auth::authorize(urls[0], &Default::default());
    assert_eq!(headers[0].0, "Authorization", "pages sign in");
    let filters = host.manga_filters(&comics).await.unwrap();
    assert_eq!(filters.len(), 1, "the root's sections");

    // The same catalogue's EPUB books, chapter by chapter.
    let books = format!("{id}/books");
    let shelf = host
        .manga_list(&books, MangaList::Popular, 1)
        .await
        .unwrap();
    let book = shelf
        .items
        .iter()
        .find(|item| item.title == "The Time Machine")
        .unwrap();
    let details = host.manga_details(&books, &book.id).await.unwrap();
    assert_eq!(details.authors, ["H. G. Wells"]);
    assert_eq!(
        details.description.as_deref(),
        Some("A scientist travels to the year 802,701.")
    );
    let chapters = host.manga_chapters(&books, &book.id).await.unwrap();
    let names: Vec<&str> = chapters
        .iter()
        .map(|chapter| chapter.name.as_str())
        .collect();
    assert_eq!(names, ["I. Introduction", "II. The Machine", "Part 3"]);
    assert!(
        chapters[0]
            .id
            .ends_with("/opds/v1.2/books/71/file/book.epub"),
        "the smaller file"
    );
    let passage = host.novel_passage(&books, &chapters[1].id).await.unwrap();
    assert!(matches!(&passage[0], crate::novel::PassageBlock::Heading(text) if text == "II"));
    // A comic series has no book chapters, and a book series no comic ones.
    assert!(
        host.manga_chapters(&books, &series)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        host.manga_chapters(&comics, &book.id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[test]
fn addresses_get_a_scheme_and_keep_their_path() {
    let urls = address_candidates(" media.example.org:8096 ").unwrap();
    let urls: Vec<&str> = urls.iter().map(Url::as_str).collect();
    assert_eq!(
        urls,
        [
            "https://media.example.org:8096/",
            "http://media.example.org:8096/"
        ]
    );
    assert_eq!(
        address_candidates("https://www.example.org/ebooks.opds/#top").unwrap()[0].as_str(),
        "https://www.example.org/ebooks.opds/",
        "a catalogue's address as typed"
    );
    assert!(address_candidates("ftp://x.example").is_err());
    assert!(address_candidates("http://[::1]:8096").is_err());
    assert!(address_candidates("  ").is_err());
}

#[test]
fn connections_build_their_extension_folder() {
    let config = ServerConfig {
        kind: ServerKind::Subsonic,
        url: "https://music.example.org".into(),
        name: "music.example.org".into(),
        username: "listener".into(),
        user_id: None,
        secret: SecretKind::Password,
    };
    let verified = package(&config).unwrap();
    let manifest = &verified.manifest;
    assert!(manifest.id.starts_with("server.subsonic."));
    assert_eq!(
        manifest.id,
        config.extension_id(),
        "the same server and user, the same id"
    );
    assert_eq!(manifest.runtime, crate::manifest::Runtime::Server);
    assert_eq!(
        manifest.provides,
        ["music.metadata", "music.stream", "lyrics"]
    );
    let net: Vec<String> = manifest
        .net
        .patterns()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(net, ["music.example.org"]);
    let other = ServerConfig {
        username: "someone".into(),
        ..config.clone()
    };
    assert_ne!(other.extension_id(), config.extension_id());
}

// ---- Live: public demo servers ----

/// Jellyfin's public demo (demo.jellyfin.org/stable; user `demo`, no password): public-domain
/// films and series, Creative Commons music.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "network"]
async fn jellyfin_live() {
    let connected = connect(&input(
        ServerKind::Jellyfin,
        "https://demo.jellyfin.org/stable".into(),
        "demo",
        "",
    ))
    .await
    .unwrap();
    let (host, _dir, id) = connected_host(&connected).await;
    let videos = format!("{id}/videos");
    let popular = host
        .manga_list(&videos, MangaList::Popular, 1)
        .await
        .unwrap();
    assert!(!popular.items.is_empty());
    let title = &popular.items[0];
    let episodes = host.video_episodes(&videos, &title.id).await.unwrap();
    let servers = host.video_servers(&videos, &episodes[0].id).await.unwrap();
    assert!(!servers.is_empty(), "{} has servers", title.title);
    println!(
        "{}: {} episodes, {} servers",
        title.title,
        episodes.len(),
        servers.len()
    );
    let found = host.search_music("a", 5).await;
    let tracks = &found[0].tracks;
    assert!(!tracks.is_empty(), "{:?}", found[0].error);
    let candidates = host
        .stream_candidates(
            &query(
                &tracks[0].title,
                tracks[0].artist.as_deref().unwrap_or_default(),
            ),
            5,
            raygun_resolver::QualityTarget::Any,
        )
        .await;
    let candidate = &candidates[0].result.as_ref().unwrap()[0];
    host.resolve_stream(
        &candidate.provider_id,
        &candidate.id,
        crate::StreamUse::default(),
    )
    .await
    .unwrap();
}

/// Navidrome's public demo (demo.navidrome.org; `demo`/`demo`): Creative Commons music.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "network"]
async fn navidrome_live() {
    let connected = connect(&input(
        ServerKind::Subsonic,
        "demo.navidrome.org".into(),
        "demo",
        "demo",
    ))
    .await
    .unwrap();
    assert_eq!(connected.config.url, "https://demo.navidrome.org");
    let (host, _dir, _) = connected_host(&connected).await;
    let found = host.search_music("the", 5).await;
    assert!(!found[0].tracks.is_empty(), "{:?}", found[0].error);
    let albums = host.search_albums("the", 3).await;
    assert!(!albums.is_empty());
    let album = host
        .music_album(&albums[0].provider.id, &albums[0].id)
        .await
        .unwrap();
    assert!(!album.tracks.is_empty());
    println!("{} songs in {}", album.tracks.len(), albums[0].title);
}

/// Komga's public demo (demo.komga.org; `demo@komga.org`/`komga-demo`): public-domain comics.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "network"]
async fn komga_live() {
    let connected = connect(&input(
        ServerKind::Opds,
        "https://demo.komga.org".into(),
        "demo@komga.org",
        "komga-demo",
    ))
    .await
    .unwrap();
    let (host, _dir, id) = connected_host(&connected).await;
    let comics = format!("{id}/comics");
    let popular = host
        .manga_list(&comics, MangaList::Popular, 1)
        .await
        .unwrap();
    let mut read = false;
    for title in &popular.items {
        let chapters = host.manga_chapters(&comics, &title.id).await.unwrap();
        if let Some(first) = chapters.first() {
            let pages = host.manga_pages(&comics, &first.id).await.unwrap();
            println!(
                "{}: {} chapters, {} pages in the first",
                title.title,
                chapters.len(),
                pages.len()
            );
            assert!(!pages.is_empty());
            read = true;
            break;
        }
    }
    assert!(read, "a series with comics");
}

/// Project Gutenberg's OPDS catalogue (gutenberg.org/ebooks.opds/): public-domain EPUB books.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "network"]
async fn gutenberg_opds_live() {
    let connected = connect(&input(
        ServerKind::Opds,
        "https://www.gutenberg.org/ebooks.opds/".into(),
        "",
        "",
    ))
    .await
    .unwrap();
    assert_eq!(connected.config.name, "Project Gutenberg");
    assert!(connected.secret.is_none());
    let (host, _dir, id) = connected_host(&connected).await;
    let books = format!("{id}/books");
    let popular = host
        .manga_list(&books, MangaList::Popular, 1)
        .await
        .unwrap();
    assert!(popular.items.len() > 5);
    let found = host
        .manga_search(&books, "time machine wells", 1)
        .await
        .unwrap();
    let book = found.items.first().unwrap_or(&popular.items[0]);
    let chapters = host.manga_chapters(&books, &book.id).await.unwrap();
    assert!(chapters.len() > 2, "{} has chapters", book.title);
    let passage = host.novel_passage(&books, &chapters[1].id).await.unwrap();
    println!(
        "{}: {} chapters; the second has {} blocks",
        book.title,
        chapters.len(),
        passage.len()
    );
    assert!(!passage.is_empty());
}
