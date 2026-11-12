//! Nuvio providers through the host as the app calls them: providers written here in the shape
//! Nuvio's are (CommonJS, `require` for cheerio and crypto-js, `fetch`, `getStreams(tmdbId,
//! mediaType, season, episode)`), against a stand-in for the Internet Archive serving a
//! public-domain film.

use std::collections::BTreeMap;
use std::net::SocketAddr;

use axum::Router;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use serde_json::json;

use crate::video::{ProviderStreams, StreamQuery, VIDEO_STREAMS};
use crate::{ExtensionHost, ExtensionSource, HostConfig};

/// A provider of public-domain films from the Internet Archive, by their TMDB ids (Night of the
/// Living Dead, 1968: TMDB 10331, IMDb tt0063350). The archive is the stand-in at SITE.
const ARCHIVE_PROVIDER: &str = r#"
const cheerio = require('cheerio-without-node-native');
const CryptoJS = require('crypto-js');
const BASE = 'SITE';
const FILMS = { '10331': 'night_of_the_living_dead', tt0063350: 'night_of_the_living_dead' };

async function getStreams(tmdbId, mediaType, season, episode) {
  if (mediaType !== 'movie') return [];
  const identifier = FILMS[tmdbId];
  if (!identifier) return [];
  const metadata = await (await fetch(BASE + '/metadata/' + identifier, {
    headers: { Accept: 'application/json', 'X-Key': TMDB_API_KEY },
  })).json();
  const page = await fetch(BASE + '/details/' + identifier);
  const $ = cheerio.load(await page.text());
  const title = $('h1 .item-title').text().trim();
  const signature = CryptoJS.MD5(identifier).toString();
  return metadata.files
    .filter(function (file) { return /\.mp4$/i.test(file.name); })
    .map(function (file) {
      return {
        name: 'Internet Archive',
        title: title + '\n' + file.name,
        url: BASE + '/download/' + identifier + '/' + encodeURIComponent(file.name),
        quality: file.height + 'p',
        size: (file.size / 1048576).toFixed(1) + ' MB',
        headers: { Referer: BASE + '/', 'X-Signature': signature },
        subtitles: [{ url: BASE + '/download/' + identifier + '/en.srt', language: 'en', name: 'English' }],
      };
    });
}

module.exports = { getStreams };
"#;

fn archive() -> Router {
    Router::new()
        .route(
            "/metadata/night_of_the_living_dead",
            get(|headers: HeaderMap| async move {
                // The provider's TMDB key arrives as the user set it.
                let keyed = headers.get("x-key").and_then(|key| key.to_str().ok()) == Some("key-123");
                let body = json!({
                    "files": [
                        { "name": "night_of_the_living_dead_512kb.mp4", "size": 163_577_856, "height": 480 },
                        { "name": "night_of_the_living_dead.mp4", "size": 1_073_741_824, "height": 1080 },
                        { "name": "night_of_the_living_dead.ogv", "size": 300_000_000, "height": 480 },
                    ],
                });
                let (status, body) = if keyed {
                    (StatusCode::OK, body.to_string())
                } else {
                    (StatusCode::UNAUTHORIZED, "{}".to_owned())
                };
                (status, [(header::CONTENT_TYPE, "application/json")], body)
            }),
        )
        .route(
            "/details/night_of_the_living_dead",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    r#"<html><body><h1><span class="item-title">Night of the Living Dead</span></h1></body></html>"#,
                )
            }),
        )
        .route(
            "/busy",
            get(|| async { (StatusCode::SERVICE_UNAVAILABLE, "busy").into_response() }),
        )
        .route(
            "/bytes",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "application/octet-stream")],
                    vec![0u8, 159, 146, 150, 255],
                )
            }),
        )
}

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

fn provider(id: &str, types: &[&str], code: &str, address: SocketAddr) -> ExtensionSource {
    let manifest = json!({
        "id": format!("nuvio.{id}"), "name": format!("Provider {id}"), "version": "1.0.0",
        "apiVersion": 1, "runtime": "nuvio", "entry": "index.js", "provides": [VIDEO_STREAMS],
        "permissions": { "net": ["127.0.0.1"] },
        "settings": [{ "key": "tmdbApiKey", "type": "text", "default": "" }],
        "nuvio": { "scraper": id, "types": types },
    });
    let code = code.replace("SITE", &format!("http://{address}"));
    ExtensionSource::built_in(&[
        ("raygun-extension.json", &manifest.to_string()),
        ("index.js", &code),
    ])
}

fn movie(tmdb: Option<&str>, imdb: Option<&str>) -> StreamQuery {
    StreamQuery {
        kind: "movie".into(),
        tmdb_id: tmdb.map(str::to_owned),
        imdb_id: imdb.map(str::to_owned),
        title: Some("Night of the Living Dead".into()),
        year: Some(1968),
        ..StreamQuery::default()
    }
}

fn only(mut answers: Vec<ProviderStreams>) -> ProviderStreams {
    assert_eq!(answers.len(), 1, "{answers:?}");
    answers.remove(0)
}

const LIMIT: std::time::Duration = std::time::Duration::from_secs(20);

#[tokio::test(flavor = "multi_thread")]
async fn a_provider_finds_a_films_streams_by_its_tmdb_id() {
    let address = serve(archive());
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let mut settings = crate::UserSettings::new();
    settings.insert(
        "nuvio.archive".into(),
        BTreeMap::from([("tmdbApiKey".to_owned(), json!("key-123"))]),
    );
    let infos = host
        .load_with_settings(
            vec![provider("archive", &["movie"], ARCHIVE_PROVIDER, address)],
            settings,
        )
        .await;
    assert_eq!(
        infos[0].status,
        raygun_model::ExtensionStatus::Loaded,
        "{:?}",
        infos[0].error
    );
    assert_eq!(infos[0].providers[0].kind, VIDEO_STREAMS);
    assert_eq!(infos[0].providers[0].id, "nuvio.archive/streams");

    let answer = only(host.video_streams(&movie(Some("10331"), None), LIMIT).await);
    let streams = answer.answer.unwrap();
    assert_eq!(streams.len(), 2, "the two MP4 files: {streams:?}");
    let full = &streams[1];
    assert_eq!(
        full.link.url,
        format!("http://{address}/download/night_of_the_living_dead/night_of_the_living_dead.mp4")
    );
    assert_eq!(full.link.quality, Some(1080));
    assert_eq!(full.name.as_deref(), Some("Internet Archive 1080p"));
    assert_eq!(
        full.description.as_deref(),
        Some("Night of the Living Dead\nnight_of_the_living_dead.mp4\n1024.0 MB"),
        "cheerio read the title; the size goes under it"
    );
    assert_eq!(full.size, Some(1_073_741_824));
    assert_eq!(full.link.headers["Referer"], format!("http://{address}/"));
    assert_eq!(
        full.link.headers["X-Signature"], "d1231c759f097df143a7c20d90b7b878",
        "crypto-js's MD5 of the identifier"
    );
    assert_eq!(full.link.subtitles[0].lang.as_deref(), Some("en"));
    assert_eq!(full.link.format, None, "a progressive file");

    // Without a TMDB id the IMDb id is asked, as Nuvio does when it can't find TMDB's.
    let by_imdb = only(
        host.video_streams(&movie(None, Some("tt0063350")), LIMIT)
            .await,
    );
    assert_eq!(by_imdb.answer.unwrap().len(), 2);
    // A movie-only provider isn't asked for an episode.
    let episode = StreamQuery {
        kind: "series".into(),
        tmdb_id: Some("10331".into()),
        season: Some(1),
        episode: Some(1),
        ..StreamQuery::default()
    };
    assert!(
        only(host.video_streams(&episode, LIMIT).await)
            .answer
            .unwrap()
            .is_empty()
    );
    // A title it doesn't know has no streams, and that isn't a failure.
    assert!(
        only(host.video_streams(&movie(Some("1"), None), LIMIT).await)
            .answer
            .unwrap()
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_globals_nuvio_gives_are_there() {
    let address = serve(archive());
    let code = r#"
    module.exports.getStreams = async function (tmdbId, mediaType, season, episode) {
      const encoder = new TextEncoder();
      const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', encoder.encode('abc')));
      const key = await crypto.subtle.importKey('raw', encoder.encode('key'), { name: 'HMAC', hash: 'SHA-256' }, false, ['sign']);
      const mac = new Uint8Array(await crypto.subtle.sign('HMAC', key, encoder.encode('The quick brown fox jumps over the lazy dog')));
      const aesKey = await crypto.subtle.importKey('raw', new Uint8Array(16), { name: 'AES-GCM' }, false, ['encrypt', 'decrypt']);
      const iv = crypto.getRandomValues(new Uint8Array(12));
      const sealed = await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, aesKey, encoder.encode('sealed'));
      const opened = new TextDecoder().decode(await crypto.subtle.decrypt({ name: 'AES-GCM', iv }, aesKey, sealed));
      const hex = (bytes) => Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
      const bytes = new Uint8Array(await (await fetch('SITE/bytes')).arrayBuffer());
      const busy = await fetch('SITE/busy');
      let ticks = 0;
      await new Promise((resolve) => { const id = setInterval(() => { if (++ticks === 3) { clearInterval(id); resolve(); } }, 1); });
      let aborted = '';
      try { await fetch('SITE/details/night_of_the_living_dead', { signal: AbortSignal.abort() }); } catch (error) { aborted = error.name; }
      const url = new URL('/a/b?c=d', 'SITE/x/');
      const facts = [
        hex(digest).slice(0, 16), hex(mac).slice(0, 16), opened, hex(bytes), busy.status, busy.ok,
        ticks, aborted, url.pathname + url.search, atob(btoa('ok')), typeof cheerio.load,
        SCRAPER_ID, JSON.stringify(SCRAPER_SETTINGS), typeof window, crypto.randomUUID().length,
        mediaType, season, episode, tmdbId,
      ];
      return [{ name: facts.join('|'), url: 'SITE/download/x.m3u8', infoHash: 'ignored' }];
    };
    "#;
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host
        .load(vec![provider("globals", &["movie", "tv"], code, address)])
        .await;
    assert_eq!(
        infos[0].status,
        raygun_model::ExtensionStatus::Loaded,
        "{:?}",
        infos[0].error
    );
    let episode = StreamQuery {
        kind: "series".into(),
        tmdb_id: Some("1396".into()),
        imdb_id: Some("tt0903747".into()),
        season: Some(2),
        episode: Some(3),
        ..StreamQuery::default()
    };
    let streams = only(host.video_streams(&episode, LIMIT).await)
        .answer
        .unwrap();
    let facts: Vec<&str> = streams[0].name.as_deref().unwrap().split('|').collect();
    assert_eq!(
        facts,
        [
            "ba7816bf8f01cfea",
            "f7bc83f430538424",
            "sealed",
            "009f9296ff",
            "503",
            "false",
            "3",
            "AbortError",
            "/a/b?c=d",
            "ok",
            "function",
            "globals",
            "{}",
            "object",
            "36",
            "tv",
            "2",
            "3",
            "1396",
        ]
    );
    assert_eq!(streams[0].link.format.as_deref(), Some("hls"));
}

#[tokio::test(flavor = "multi_thread")]
async fn torrents_settings_and_refusals() {
    let address = serve(archive());
    let code = r#"
    function onSettings() {
      return [
        { type: 'header', label: 'Streams' },
        { type: 'select', key: 'quality', label: 'Quality', defaultValue: 'hd',
          options: [{ label: 'High', value: 'hd' }, { label: 'Low', value: 'sd' }] },
        { type: 'toggle', key: 'torrents', label: 'Torrents', defaultValue: true },
        { type: 'text', key: 'token', label: 'Token' },
      ];
    }
    async function getStreams(tmdbId) {
      if (tmdbId === 'busy') {
        await fetch('SITE/busy');
        return [];
      }
      return [{
        title: 'Sintel (2010)\n1080p', size: '1.2 GB', seeders: 46, quality: '1080p',
        infoHash: '08ADA5A7A6183AAE1E09D831DF6748D566095A10', fileIdx: 1,
        sources: ['tracker:udp://tracker.example.org:1337', 'dht:08ada5a7'],
        provider: ['Example', SCRAPER_SETTINGS.quality, SCRAPER_SETTINGS.torrents].join(' '),
      }];
    }
    module.exports = { getStreams, onSettings };
    "#;
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let mut settings = crate::UserSettings::new();
    settings.insert(
        "nuvio.torrents".into(),
        BTreeMap::from([("quality".to_owned(), json!("Low"))]),
    );
    let infos = host
        .load_with_settings(
            vec![provider("torrents", &["movie", "tv"], code, address)],
            settings,
        )
        .await;
    let keys: Vec<&str> = infos[0]
        .settings
        .iter()
        .map(|setting| setting.key.as_str())
        .collect();
    assert_eq!(keys, ["tmdbApiKey", "quality", "torrents", "token"]);

    let streams = only(host.video_streams(&movie(Some("45745"), None), LIMIT).await)
        .answer
        .unwrap();
    let torrent = &streams[0];
    assert_eq!(torrent.link.format.as_deref(), Some("torrent"));
    assert_eq!(
        torrent.link.url,
        "magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10&dn=Sintel%20(2010)\
         &tr=udp%3A%2F%2Ftracker.example.org%3A1337&so=1"
    );
    assert_eq!(
        torrent.name.as_deref(),
        Some("Example sd true 1080p"),
        "the choice's value for the user's label, the switch's default"
    );
    assert_eq!(
        torrent.description.as_deref(),
        Some("Sintel (2010)\n1080p\n1.2 GB · Seeders: 46")
    );
    assert_eq!(torrent.size, Some(1_288_490_188));

    // A provider that met a refusal and found nothing says what it met.
    let busy = only(host.video_streams(&movie(Some("busy"), None), LIMIT).await);
    let error = busy.answer.unwrap_err().to_string();
    assert!(error.contains("503"), "{error}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_provider_without_get_streams_registers_nothing() {
    let address = serve(archive());
    // It names getStreams, but only in a comment: nothing is exported to call.
    let code =
        "// getStreams comes in the next version\nmodule.exports = { search: async () => [] };";
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host
        .load(vec![provider("empty", &["movie"], code, address)])
        .await;
    assert_eq!(infos[0].status, raygun_model::ExtensionStatus::Loaded);
    assert!(infos[0].providers.is_empty());
    assert_eq!(
        crate::purpose::installed(&infos[0]).as_deref(),
        Some(crate::purpose::NOTHING_CALLABLE)
    );
    assert!(
        host.video_streams(&movie(Some("1"), None), LIMIT)
            .await
            .is_empty()
    );
}
