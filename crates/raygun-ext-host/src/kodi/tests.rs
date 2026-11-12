//! Kodi plugins run end to end: a plugin written here (served by a local server), and live
//! checks against legal add-ons of Kodi's own repository (ignored: they need the network).

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::Path;

use axum::Router;
use axum::http::{Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::{Value, json};

use super::addon::{ADDON_DIR, PACKAGE_INFO, PackageInfo};
use super::runtime::KodiExtension;
use crate::host::ExtensionSource;
use crate::manifest::{API_VERSION, MANIFEST_FILE, Manifest};

fn answer(value: Value) -> Response {
    (
        [(header::CONTENT_TYPE, "application/json")],
        value.to_string(),
    )
        .into_response()
}

fn query(uri: &Uri, key: &str) -> Option<String> {
    url::form_urlencoded::parse(uri.query().unwrap_or_default().as_bytes())
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.into_owned())
}

/// A small film site for the plugin below.
fn serve() -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route(
            "/films.json",
            get(|uri: Uri| async move {
                let page: u32 = query(&uri, "page").and_then(|page| page.parse().ok()).unwrap_or(1);
                let items: Vec<Value> = (1..=3)
                    .map(|n| {
                        let id = (page - 1) * 3 + n;
                        json!({
                            "id": id,
                            "title": format!("Film {id}"),
                            "thumb": format!("https://img.example.org/{id}.jpg"),
                            "plot": format!("[B]Plot[/B] of film {id}"),
                            "year": 1920 + id,
                        })
                    })
                    .collect();
                answer(json!({ "items": items, "more": page < 2 }))
            }),
        )
        .route(
            "/search",
            get(|uri: Uri| async move {
                let words = query(&uri, "q").unwrap_or_default();
                answer(json!([{ "id": 7, "title": format!("Found: {words}") }]))
            }),
        )
        .route(
            "/film/{id}",
            get(|axum::extract::Path(id): axum::extract::Path<String>| async move {
                let id = id.trim_end_matches(".json").to_owned();
                answer(json!({
                    "sources": [
                        { "label": "MP4 480p", "url": format!("https://media.example.org/{id}.mp4") },
                        { "label": "HLS 1080p", "url": format!("https://media.example.org/{id}/master.m3u8") },
                    ]
                }))
            }),
        );
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

/// A plugin written the way Kodi's are: `requests` and `urllib`, a SQLite cache in its profile,
/// a keyboard search sending Kodi elsewhere, a quality choice before it resolves.
const PLUGIN: &str = r#"
import json
import os
import sqlite3
import sys
import urllib.parse
import urllib.request

import requests
import xbmc
import xbmcaddon
import xbmcgui
import xbmcplugin
import xbmcvfs

# What add-ons import from the standard library and older Pythons, all there.
import cgi
import ctypes
import fcntl
import shelve
import socketserver
from concurrent.futures import ThreadPoolExecutor
from html.parser import HTMLParser

assert HTMLParser().unescape('&amp;') == '&'
with ThreadPoolExecutor(2) as pool:
    assert list(pool.map(lambda n: n * 2, [1, 2])) == [2, 4]

ADDON = xbmcaddon.Addon()
HANDLE = int(sys.argv[1])
BASE = ADDON.getSetting('server')
PARAMS = dict(urllib.parse.parse_qsl(sys.argv[2][1:]))
PROFILE = xbmcvfs.translatePath(ADDON.getAddonInfo('profile'))


def url(**query):
    return sys.argv[0] + '?' + urllib.parse.urlencode(query)


def remember(key, value):
    os.makedirs(PROFILE, exist_ok=True)
    database = sqlite3.connect(os.path.join(PROFILE, 'cache.db'))
    database.execute('CREATE TABLE IF NOT EXISTS cache (key TEXT PRIMARY KEY, value TEXT)')
    database.execute('INSERT OR REPLACE INTO cache VALUES (?, ?)', (key, value))
    database.commit()
    found = database.execute('SELECT value FROM cache WHERE key = ?', (key,)).fetchone()[0]
    database.close()
    with open(os.path.join(PROFILE, 'last.txt'), 'w') as handle:
        handle.write(found)
    return found


mode = PARAMS.get('mode')
if mode is None:
    for label, target in ((ADDON.getLocalizedString(30001), url(mode='films', page=1)),
                          (xbmc.getLocalizedString(137), url(mode='search'))):
        xbmcplugin.addDirectoryItem(HANDLE, target, xbmcgui.ListItem(label), True)
    xbmcplugin.endOfDirectory(HANDLE)
elif mode == 'films':
    page = int(PARAMS['page'])
    films = requests.get(BASE + '/films.json', params={'page': page}, timeout=10).json()
    for film in films['items']:
        item = xbmcgui.ListItem(film['title'])
        item.setArt({'thumb': film['thumb'], 'fanart': film['thumb']})
        tag = item.getVideoInfoTag()
        tag.setPlot(film['plot'])
        tag.setYear(film['year'])
        tag.setMediaType('movie')
        item.setProperty('IsPlayable', 'true')
        xbmcplugin.addDirectoryItem(HANDLE, url(mode='play', id=film['id']), item, False)
    if films['more']:
        xbmcplugin.addDirectoryItem(HANDLE, url(mode='films', page=page + 1),
                                    xbmcgui.ListItem('[COLOR lime]Next page[/COLOR]'), True)
    remember('page', str(page))
    xbmcplugin.setContent(HANDLE, 'movies')
    xbmcplugin.endOfDirectory(HANDLE)
elif mode == 'search':
    keyboard = xbmc.Keyboard('', 'Search')
    keyboard.doModal()
    if keyboard.isConfirmed():
        xbmc.executebuiltin('Container.Update(%s)' % url(mode='results', q=keyboard.getText()))
    xbmcplugin.endOfDirectory(HANDLE, cacheToDisc=False)
elif mode == 'results':
    with urllib.request.urlopen(BASE + '/search?' + urllib.parse.urlencode({'q': PARAMS['q']})) as answer:
        found = json.loads(answer.read().decode('utf-8'))
    for film in found:
        item = xbmcgui.ListItem(film['title'])
        item.setProperty('IsPlayable', 'true')
        xbmcplugin.addDirectoryItem(HANDLE, url(mode='play', id=film['id']), item, False)
    xbmcplugin.endOfDirectory(HANDLE)
elif mode == 'play':
    film = requests.get('%s/film/%s.json' % (BASE, PARAMS['id'])).json()
    choice = xbmcgui.Dialog().select('Quality', [source['label'] for source in film['sources']])
    if choice < 0:
        xbmcplugin.setResolvedUrl(HANDLE, False, xbmcgui.ListItem())
    else:
        source = film['sources'][choice]
        item = xbmcgui.ListItem(path=source['url'] + '|Referer=' + urllib.parse.quote(BASE + '/'))
        if source['url'].endswith('.m3u8'):
            item.setProperty('inputstream', 'inputstream.adaptive')
            item.setProperty('inputstream.adaptive.manifest_type', 'hls')
        xbmcplugin.setResolvedUrl(HANDLE, True, item)
"#;

const STRINGS: &str = "msgctxt \"#30001\"\nmsgid \"Films\"\nmsgstr \"\"\n";

/// The plugin installed as `package` would install it.
fn install(dir: &Path) -> (Manifest, ExtensionSource) {
    let folder = dir.join("kodi.plugin.video.fixture");
    let addon = folder.join(ADDON_DIR);
    std::fs::create_dir_all(addon.join("resources/language/resource.language.en_gb")).unwrap();
    std::fs::write(addon.join("default.py"), PLUGIN).unwrap();
    std::fs::write(
        addon.join("resources/language/resource.language.en_gb/strings.po"),
        STRINGS,
    )
    .unwrap();
    let info = PackageInfo {
        addon: "plugin.video.fixture".into(),
        name: "Fixture films".into(),
        version: "1.0.0".into(),
        author: "Raygun".into(),
        summary: String::new(),
        library: "default.py".into(),
        provides: vec!["video".into()],
        icon: String::new(),
        fanart: String::new(),
        dependencies: Vec::new(),
        settings: vec![super::addon::KodiSetting {
            id: "server".into(),
            kind: super::addon::SettingKind::Text,
            label: "Server".into(),
            default: String::new(),
            options: Vec::new(),
        }],
        answered: Vec::new(),
    };
    std::fs::write(
        folder.join(PACKAGE_INFO),
        serde_json::to_vec(&info).unwrap(),
    )
    .unwrap();
    let manifest = json!({
        "id": "kodi.plugin.video.fixture",
        "name": "Fixture films",
        "version": "1.0.0",
        "apiVersion": API_VERSION,
        "runtime": "kodi",
        "entry": "addon/default.py",
        "provides": ["video.source"],
        "permissions": { "net": ["*"] },
        "settings": super::addon::manifest_settings(&info.settings),
    })
    .to_string();
    std::fs::write(folder.join(MANIFEST_FILE), &manifest).unwrap();
    (
        Manifest::parse(&manifest).unwrap(),
        ExtensionSource::installed(folder),
    )
}

async fn call(extension: &KodiExtension, method: &str, args: Value) -> Value {
    extension
        .call("video", method, &args)
        .await
        .unwrap_or_else(|failure| panic!("{method}: {}", failure.message))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_plugin_lists_searches_and_resolves() {
    let address = serve();
    let dir = tempfile::tempdir().unwrap();
    let (manifest, source) = install(dir.path());
    let data = dir.path().join(".data/kodi.plugin.video.fixture");
    let settings = BTreeMap::from([("server".to_owned(), json!(format!("http://{address}")))]);
    let extension =
        KodiExtension::load_with(manifest, &source, Some(data.clone()), settings, true).unwrap();
    assert_eq!(extension.providers().len(), 1);

    let popular = call(&extension, "popular", json!({ "page": 1 })).await;
    let items = popular["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "the search entry is search's: {popular}");
    assert_eq!(items[0]["title"], "Films", "named by its strings.po");
    assert_eq!(items[0]["folder"], true);
    let films_id = items[0]["id"].as_str().unwrap().to_owned();
    assert_eq!(films_id, "/?mode=films&page=1");

    let filters = call(&extension, "filters", json!({})).await;
    assert_eq!(filters[0]["options"][0]["value"], films_id.as_str());

    let films = call(
        &extension,
        "search",
        json!({ "query": "", "page": 1, "filters": { "section": films_id } }),
    )
    .await;
    assert_eq!(films["items"].as_array().unwrap().len(), 3);
    assert_eq!(films["items"][0]["title"], "Film 1");
    assert_eq!(
        films["items"][0]["coverUrl"],
        "https://img.example.org/1.jpg"
    );
    assert_eq!(films["hasNextPage"], true, "its next page entry");
    let second = call(
        &extension,
        "search",
        json!({ "query": "", "page": 2, "filters": { "section": films_id } }),
    )
    .await;
    assert_eq!(second["items"][0]["title"], "Film 4");
    assert_eq!(second["hasNextPage"], false);
    assert_eq!(
        std::fs::read_to_string(data.join("userdata/addon_data/plugin.video.fixture/last.txt"))
            .unwrap(),
        "2",
        "files and SQLite in its own folder"
    );

    let film = films["items"][1]["id"].as_str().unwrap().to_owned();
    let details = call(&extension, "details", json!({ "id": film })).await;
    assert_eq!(details["title"], "Film 2");
    assert_eq!(details["description"], "Plot of film 2");
    assert_eq!(details["year"], 1922);
    assert_eq!(details["type"], "movie");
    let episodes = call(&extension, "episodes", json!({ "id": film })).await;
    assert_eq!(episodes["episodes"][0]["id"], film.as_str());

    let servers = call(&extension, "servers", json!({ "episodeId": film })).await;
    let servers = servers["servers"].as_array().unwrap();
    assert_eq!(servers.len(), 2, "the plugin's choice: {servers:?}");
    assert_eq!(servers[1]["name"], "HLS 1080p");
    let videos = call(
        &extension,
        "videos",
        json!({ "serverId": servers[1]["id"] }),
    )
    .await;
    let video = &videos["videos"][0];
    assert_eq!(video["url"], "https://media.example.org/2/master.m3u8");
    assert_eq!(video["format"], "hls");
    assert_eq!(video["headers"]["Referer"], format!("http://{address}/"));

    let found = call(
        &extension,
        "search",
        json!({ "query": "moon light", "page": 1 }),
    )
    .await;
    assert_eq!(found["items"][0]["title"], "Found: moon light");
}

// ---- Live: legal add-ons of Kodi's own repository ----

use reqwest::Url;

async fn fetch(client: &reqwest::Client, url: &Url, limit: u64) -> Result<Vec<u8>, String> {
    // Kodi's mirrors drop a connection now and then: once more before giving up.
    match fetch_once(client, url, limit).await {
        Ok(bytes) => Ok(bytes),
        Err(_) => fetch_once(client, url, limit).await,
    }
}

async fn fetch_once(client: &reqwest::Client, url: &Url, limit: u64) -> Result<Vec<u8>, String> {
    let response = client
        .get(url.clone())
        .send()
        .await
        .map_err(|error| format!("{url}: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("{url}: HTTP {}", response.status()));
    }
    let bytes = response.bytes().await.map_err(|error| error.to_string())?;
    if bytes.len() as u64 > limit {
        return Err(format!("{url}: too large"));
    }
    Ok(bytes.to_vec())
}

/// Kodi's own repository, read once per test.
async fn official_index(client: &reqwest::Client) -> (Url, crate::repo::RepoIndex) {
    let url = Url::parse(super::OFFICIAL_REPOSITORY).unwrap();
    let bytes = fetch(client, &url, 64 << 20).await.unwrap();
    let index = crate::repo::parse_index(&super::unpack(&bytes).unwrap()).unwrap();
    (url, index)
}

/// Installs one plugin of Kodi's repository as Raygun would.
async fn install_official(
    client: &reqwest::Client,
    index_url: &Url,
    index: &crate::repo::RepoIndex,
    kodi_id: &str,
    dir: &Path,
) -> Result<(Manifest, ExtensionSource), String> {
    let id = format!("kodi.{kodi_id}");
    let entry = index
        .extensions
        .iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| format!("{kodi_id} isn't listed"))?;
    let package_url =
        crate::repo::package_url(index_url, entry).map_err(|error| error.to_string())?;
    let bytes = fetch(client, &package_url, super::package::MAX_ADDON_BYTES).await?;
    let sources = super::package::library_sources(index_url);
    let verified = super::package::assemble(entry, &bytes, &sources, |url, limit| {
        let client = client.clone();
        async move { fetch(&client, &url, limit).await }
    })
    .await?;
    let folder = crate::package::install(dir, &verified).map_err(|error| error.to_string())?;
    Ok((
        verified.manifest.clone(),
        ExtensionSource::installed(folder),
    ))
}

async fn load_official(kodi_id: &str, dir: &Path) -> KodiExtension {
    let client = raygun_net::http_client().unwrap();
    let (index_url, index) = official_index(&client).await;
    let (manifest, source) = install_official(&client, &index_url, &index, kodi_id, dir)
        .await
        .unwrap();
    let data = dir.join(".data").join(&manifest.id);
    KodiExtension::load(manifest, &source, Some(data), BTreeMap::new())
        .await
        .unwrap()
}

async fn ask(extension: &KodiExtension, provider: &str, method: &str, args: Value) -> Value {
    let started = std::time::Instant::now();
    let answer = extension
        .call(provider, method, &args)
        .await
        .unwrap_or_else(|failure| panic!("{provider}.{method}: {}", failure.message));
    println!(
        "{provider}.{method} ({:?}): {}",
        started.elapsed(),
        answer.to_string().chars().take(600).collect::<String>()
    );
    answer
}

/// The Internet Archive's add-on (public-domain films): its collections, one collection's films,
/// and one film's streams.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "network"]
async fn kodi_archive_org_live() {
    let dir = tempfile::tempdir().unwrap();
    let extension = load_official("plugin.video.archive.org", dir.path()).await;
    let popular = ask(&extension, "video", "popular", json!({ "page": 1 })).await;
    let menu = popular["items"].as_array().unwrap();
    assert!(!menu.is_empty(), "its menu");
    let collections_id = menu[0]["id"].as_str().unwrap().to_owned();
    let collections = ask(
        &extension,
        "video",
        "search",
        json!({ "query": "", "page": 1, "filters": { "section": collections_id } }),
    )
    .await;
    let collection = collections["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["folder"] == true)
        .expect("a collection")
        .clone();
    let items = ask(
        &extension,
        "video",
        "search",
        json!({ "query": "", "page": 1, "filters": { "section": collection["id"] } }),
    )
    .await;
    assert!(
        !items["items"].as_array().unwrap().is_empty(),
        "a collection's items"
    );
    // A public-domain film (Nosferatu, 1922), found with the add-on's own search.
    let found = ask(
        &extension,
        "video",
        "search",
        json!({ "query": "nosferatu 1922", "page": 1 }),
    )
    .await;
    let film = found["items"][0]["id"]
        .as_str()
        .expect("its search")
        .to_owned();
    let details = ask(&extension, "video", "details", json!({ "id": film })).await;
    assert!(
        details["title"]
            .as_str()
            .is_some_and(|title| !title.is_empty())
    );
    // Playing reads a mark archive.org's pages no longer have: the add-on (as in Kodi) finds
    // nothing, which Raygun says plainly.
    match extension
        .call("video", "servers", &json!({ "episodeId": film }))
        .await
    {
        Ok(servers) => println!("servers: {servers}"),
        Err(failure) => println!(
            "servers: {}",
            failure.message.lines().next().unwrap_or_default()
        ),
    }
}

/// media.ccc.de (talks under Creative Commons licences): its menu, a conference's talks and one
/// talk's recordings, through `routing` and `requests`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "network"]
async fn kodi_media_ccc_live() {
    let dir = tempfile::tempdir().unwrap();
    let extension = load_official("plugin.video.media-ccc-de", dir.path()).await;
    let popular = ask(&extension, "video", "popular", json!({ "page": 1 })).await;
    assert!(!popular["items"].as_array().unwrap().is_empty());
    let found = ask(
        &extension,
        "video",
        "search",
        json!({ "query": "rust", "page": 1 }),
    )
    .await;
    let mut talk = found["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["folder"] != true)
        .map(|item| item["id"].as_str().unwrap().to_owned());
    if talk.is_none() {
        // Its conferences, followed into the first folders until a talk shows.
        let mut seen = vec!["/".to_owned()];
        let mut folder = popular["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| item["id"].as_str())
            .find(|id| id.contains("conferences"))
            .unwrap_or("/dir/conferences")
            .to_owned();
        for _ in 0..5 {
            seen.push(folder.clone());
            let listed = ask(
                &extension,
                "video",
                "search",
                json!({ "query": "", "page": 1, "filters": { "section": folder } }),
            )
            .await;
            let items = listed["items"].as_array().unwrap();
            if let Some(found) = items.iter().find(|item| item["folder"] != true) {
                talk = Some(found["id"].as_str().unwrap().to_owned());
                break;
            }
            let Some(next) = items
                .iter()
                .filter_map(|item| item["id"].as_str())
                .find(|id| !seen.iter().any(|known| known == id) && !id.contains("live"))
            else {
                break;
            };
            folder = next.to_owned();
        }
    }
    let talk = talk.expect("a talk");
    let servers = ask(&extension, "video", "servers", json!({ "episodeId": talk })).await;
    let server = &servers["servers"][0];
    let videos = match server.get("videos") {
        Some(videos) => videos.clone(),
        None => ask(
            &extension,
            "video",
            "videos",
            json!({ "serverId": server["id"] }),
        )
        .await["videos"]
            .clone(),
    };
    assert!(
        videos[0]["url"].as_str().unwrap().starts_with("https://"),
        "{videos}"
    );
}

/// SomaFM (listener-supported internet radio): its stations as a playlist, one station's stream.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "network"]
async fn kodi_somafm_live() {
    let dir = tempfile::tempdir().unwrap();
    let extension = load_official("plugin.audio.somafm", dir.path()).await;
    let playlists = ask(
        &extension,
        "music",
        "searchPlaylists",
        json!({ "query": "", "limit": 30, "featured": true }),
    )
    .await;
    let station_list = playlists["playlists"][0]["id"].as_str().unwrap().to_owned();
    let stations = ask(
        &extension,
        "music",
        "playlist",
        json!({ "id": station_list }),
    )
    .await;
    let station = &stations["tracks"][0];
    assert!(
        station["title"]
            .as_str()
            .is_some_and(|title| !title.is_empty())
    );
    let candidates = ask(
        &extension,
        "stream",
        "candidates",
        json!({ "title": station["title"], "artist": station.get("artist"), "limit": 5 }),
    )
    .await;
    let id = candidates["candidates"][0]["id"]
        .as_str()
        .expect("its own station")
        .to_owned();
    let stream = ask(&extension, "stream", "resolve", json!({ "id": id })).await;
    assert!(
        stream["source"]["url"]
            .as_str()
            .unwrap()
            .starts_with("http"),
        "{stream}"
    );
}

/// Every video and music plugin of Kodi's repository: installed, then its top folder listed.
/// Prints what fraction installs and lists, and why the rest don't (`KODI_SURVEY=plugin.a,…`
/// narrows it).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "network; several minutes"]
async fn kodi_official_survey() {
    let client = raygun_net::http_client().unwrap();
    let (index_url, index) = official_index(&client).await;
    let only: Vec<String> = std::env::var("KODI_SURVEY")
        .map(|list| {
            list.split(',')
                .map(|id| format!("kodi.{}", id.trim()))
                .collect()
        })
        .unwrap_or_default();
    let plugins: Vec<&crate::repo::RepoEntry> = index
        .extensions
        .iter()
        .filter(|entry| entry.not_for_raygun.is_none() && !entry.provides.is_empty())
        .filter(|entry| only.is_empty() || only.contains(&entry.id))
        .collect();
    let dir = tempfile::tempdir().unwrap();
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(6));
    let mut tasks = tokio::task::JoinSet::new();
    for entry in plugins {
        let (client, index_url, index) = (client.clone(), index_url.clone(), index.clone());
        let entry = entry.clone();
        let root = dir.path().to_path_buf();
        let semaphore = semaphore.clone();
        tasks.spawn(async move {
            let _turn = semaphore.acquire_owned().await;
            let kodi_id = entry.id.trim_start_matches("kodi.").to_owned();
            let folder = root.join(&kodi_id);
            std::fs::create_dir_all(&folder).unwrap();
            let installed = tokio::time::timeout(
                std::time::Duration::from_secs(180),
                install_official(&client, &index_url, &index, &kodi_id, &folder),
            )
            .await
            .unwrap_or_else(|_| Err("installing took too long".into()));
            let (manifest, source) = match installed {
                Ok(found) => found,
                Err(error) => return (kodi_id, "not installed", error),
            };
            let data = folder.join(".data");
            let extension =
                match KodiExtension::load(manifest, &source, Some(data), BTreeMap::new()).await {
                    Ok(extension) => extension,
                    Err(error) => return (kodi_id, "not loaded", error),
                };
            let video = extension
                .providers()
                .iter()
                .any(|provider| provider.kind == crate::video::VIDEO_SOURCE);
            let (provider, method, args) = if video {
                ("video", "popular", json!({ "page": 1 }))
            } else {
                (
                    "music",
                    "searchPlaylists",
                    json!({ "query": "", "limit": 50, "featured": true }),
                )
            };
            let answer = tokio::time::timeout(
                std::time::Duration::from_secs(90),
                extension.call(provider, method, &args),
            )
            .await;
            match answer {
                Err(_) => (kodi_id, "listing timed out", String::new()),
                Ok(Err(failure)) => (kodi_id, "listing failed", failure.message),
                Ok(Ok(value)) => {
                    let count = value["items"]
                        .as_array()
                        .or_else(|| value["playlists"].as_array())
                        .map_or(0, Vec::len);
                    if count > 0 {
                        (kodi_id, "listed", format!("{count} entries"))
                    } else {
                        (kodi_id, "listed nothing", value.to_string())
                    }
                }
            }
        });
    }
    let mut outcomes: BTreeMap<&'static str, Vec<(String, String)>> = BTreeMap::new();
    while let Some(done) = tasks.join_next().await {
        let (id, outcome, detail) = done.unwrap();
        outcomes.entry(outcome).or_default().push((id, detail));
    }
    let total: usize = outcomes.values().map(Vec::len).sum();
    println!("\n{total} plugins");
    for (outcome, plugins) in &outcomes {
        println!(
            "\n== {outcome}: {} ({:.0}%)",
            plugins.len(),
            plugins.len() as f64 * 100.0 / total.max(1) as f64
        );
        for (id, detail) in plugins {
            let first = detail.lines().next().unwrap_or_default();
            let cause = detail
                .lines()
                .rev()
                .find(|line| line.contains("Error") || line.contains("error"))
                .unwrap_or_default();
            println!(
                "  {id}: {first} | {}",
                cause.chars().take(160).collect::<String>()
            );
        }
    }
}
