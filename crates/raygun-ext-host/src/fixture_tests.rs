//! First-party extensions against local fixture servers (M15): the real bundles, with their API
//! base rewritten to point at a server on this machine.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;

use axum::Router;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::json;

use crate::{ExtensionHost, ExtensionSource, HostConfig};

fn query_params(uri: &Uri) -> HashMap<String, String> {
    uri.query()
        .unwrap_or_default()
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(key, value)| {
            let value = percent_decode(&value.replace('+', " "));
            (key.to_owned(), value)
        })
        .collect()
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(&text[index + 1..index + 3], 16)
        {
            out.push(byte);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn json_response(status: StatusCode, body: serde_json::Value) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "application/json")],
        body.to_string(),
    )
        .into_response()
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

/// A first-party extension whose `api_base` now points at `address`, with network access
/// limited to that address.
fn patched_extension(name: &str, api_base: &str, address: SocketAddr) -> ExtensionSource {
    patched_extension_bases(name, &[api_base], address)
}

/// [`patched_extension`] for an extension that talks to more than one API.
fn patched_extension_bases(name: &str, api_bases: &[&str], address: SocketAddr) -> ExtensionSource {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions")
        .join(name);
    let mut manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("raygun-extension.json")).unwrap())
            .unwrap();
    manifest["permissions"]["net"] = json!(["127.0.0.1"]);
    let mut code = std::fs::read_to_string(dir.join("dist/index.js")).unwrap();
    for api_base in api_bases {
        assert!(
            code.contains(api_base),
            "{name} bundle doesn't use {api_base}"
        );
        code = code.replace(api_base, &format!("http://{address}"));
    }
    ExtensionSource::built_in(&[
        ("raygun-extension.json", &manifest.to_string()),
        ("dist/index.js", &code),
    ])
}

#[test]
fn fixture_helpers_decode_queries() {
    let uri: Uri = "/api/get?track_name=Hello%2C%20Dolly&artist_name=A+B"
        .parse()
        .unwrap();
    let params = query_params(&uri);
    assert_eq!(params["track_name"], "Hello, Dolly");
    assert_eq!(params["artist_name"], "A B");
}

#[tokio::test]
async fn gutenberg_lists_books_splits_chapters_and_reads_passages() {
    let filler =
        "It was on a dreary night of November that I beheld the accomplishment of my toils. "
            .repeat(3);
    let html = format!(
        "<html><body><h1>Frankenstein</h1><p>Title page</p>\
         <p>*** START OF THE PROJECT GUTENBERG EBOOK FRANKENSTEIN ***</p>\
         <h2>CONTENTS</h2><p>{filler}</p>\
         <h2><a id='l1'></a>Letter 1<span class='pagenum'>{{1}}</span></h2><p>{filler}</p>\
         <img src='images/fig.jpg'><script>steal()</script>\
         <h2>Chapter 1</h2><p>Short</p><p>{filler}</p>\
         <p>*** END OF THE PROJECT GUTENBERG EBOOK FRANKENSTEIN ***</p>\
         <h2>Licence</h2><p>{filler}</p></body></html>"
    );
    let book_requests = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = book_requests.clone();
    let app = Router::new()
        .route(
            "/ebooks/search.opds/",
            get(|uri: Uri| async move {
                let params = query_params(&uri);
                let entries = match params.get("query").map(String::as_str) {
                    Some("nothing") => String::new(),
                    _ => "<entry><id>https://www.gutenberg.org/ebooks/authors/search.opds/?query=x</id><title>Authors</title></entry>\
                          <entry><id>https://www.gutenberg.org/ebooks/84.opds</id><title>Frankenstein &amp; more</title></entry>"
                        .to_owned(),
                };
                let start = params.get("start_index").cloned().unwrap_or_default();
                let sort = params.get("sort_order").cloned().unwrap_or_default();
                ([(header::CONTENT_TYPE, "application/atom+xml")], format!("<feed sort='{sort}' start='{start}'>{entries}</feed>")).into_response()
            }),
        )
        .route(
            "/ebooks/84.opds",
            get(|| async {
                let xml = r#"<feed><title>Frankenstein by Mary</title><entry>
                    <title>Frankenstein; or, The modern Prometheus</title>
                    <content type="xhtml"><div><p>Summary: A scientist builds a creature. (This is an automatically generated summary.)</p></div></content>
                    <author><name>Shelley, Mary Wollstonecraft</name></author>
                    <category scheme="http://purl.org/dc/terms/LCSH" term="Gothic fiction"/><category scheme="http://purl.org/dc/terms/LCSH" term="Monsters -- Fiction"/><category scheme="http://purl.org/dc/terms/LCSH" term="Gothic fiction"/><category scheme="http://purl.org/dc/terms/LCC" term="PR"/>
                    </entry></feed>"#;
                ([(header::CONTENT_TYPE, "application/atom+xml")], xml).into_response()
            }),
        )
        .route(
            "/ebooks/84.html.images",
            get(move || {
                counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                async move { ([(header::CONTENT_TYPE, "text/html")], html).into_response() }
            }),
        );
    let address = serve(app);

    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions/gutenberg");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("raygun-extension.json")).unwrap())
            .unwrap();
    manifest["permissions"]["net"] = json!(["127.0.0.1"]);
    let code = std::fs::read_to_string(dir.join("dist/index.js"))
        .unwrap()
        .replace("https://www.gutenberg.org", &format!("http://{address}"));
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(vec![ExtensionSource::built_in(&[
        ("raygun-extension.json", &manifest.to_string()),
        ("dist/index.js", &code),
    ])])
    .await;
    let provider = "dev.raygun.gutenberg/gutenberg";
    assert_eq!(host.providers(crate::novel::NOVEL_SOURCE).await.len(), 1);

    let popular = host
        .manga_list(provider, crate::manga::MangaList::Popular, 1)
        .await
        .unwrap();
    assert_eq!(popular.items.len(), 1, "author entries are skipped");
    assert_eq!(popular.items[0].id, "84");
    assert_eq!(popular.items[0].title, "Frankenstein & more");
    assert_eq!(
        popular.items[0].cover_url.as_deref(),
        Some(format!("http://{address}/cache/epub/84/pg84.cover.medium.jpg").as_str())
    );
    assert!(!popular.has_next_page);
    let none = host.manga_search(provider, "nothing", 2).await.unwrap();
    assert!(none.items.is_empty());

    let details = host.manga_details(provider, "84").await.unwrap();
    assert_eq!(details.title, "Frankenstein; or, The modern Prometheus");
    assert_eq!(details.authors, ["Mary Wollstonecraft Shelley"]);
    assert_eq!(
        details.description.as_deref(),
        Some("A scientist builds a creature.")
    );
    assert_eq!(details.genres, ["Gothic fiction", "Monsters"]);
    assert_eq!(details.status, "completed");

    let chapters = host.manga_chapters(provider, "84").await.unwrap();
    let names: Vec<_> = chapters
        .iter()
        .map(|chapter| chapter.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["Letter 1", "Chapter 1"],
        "no contents, licence or front matter"
    );
    assert_eq!(chapters[1].id, "84/1");
    assert_eq!(chapters[1].number, Some(2.0));

    let letter = host.novel_passage(provider, "84/0").await.unwrap();
    use crate::novel::PassageBlock;
    assert_eq!(letter[0], PassageBlock::Heading("Letter 1".into()));
    assert!(
        matches!(&letter[1], PassageBlock::Paragraph(text) if text.starts_with("It was on a dreary night"))
    );
    assert_eq!(
        letter[2],
        PassageBlock::Image(format!("http://{address}/ebooks/images/fig.jpg"))
    );
    assert_eq!(letter.len(), 3, "the script is gone");
    assert!(host.novel_passage(provider, "84/7").await.is_err());
    assert_eq!(
        book_requests.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the book is downloaded once for its chapters and passages"
    );
}

#[tokio::test]
async fn internet_archive_films_browse_details_episodes_and_videos() {
    let app = Router::new()
        .route(
            "/advancedsearch.php",
            get(|uri: Uri| async move {
                let params = query_params(&uri);
                assert_eq!(params["rows"], "30");
                let q = &params["q"];
                // Public-domain sets list items marked public domain; animation, openly
                // licensed ones too.
                let marks = "(licenseurl:(*zero*) OR (licenseurl:(*publicdomain*) AND -year:[1978 TO 9999]))";
                let wanted = if q.contains("collection:(animationandcartoons)") {
                    format!("AND ({marks} OR (licenseurl:(*creativecommons*) AND -licenseurl:(*publicdomain*)))")
                } else {
                    format!("AND {marks}")
                };
                assert!(q.ends_with(&wanted), "{q}");
                let docs = if q.contains("collection:(Film_Noir)") {
                    json!([{ "identifier": "noir", "title": "Detour" }])
                } else {
                    json!([
                        { "identifier": "chaplin", "title": "Charlie Chaplin Festival" },
                        { "identifier": "serial", "title": ["Flash Gordon", "alt"] },
                    ])
                };
                json_response(StatusCode::OK, json!({ "response": { "numFound": 45, "docs": docs } }))
            }),
        )
        .route(
            "/metadata/chaplin",
            get(|| async {
                json_response(StatusCode::OK, json!({
                    "metadata": {
                        "title": "Charlie Chaplin Festival", "date": "1938", "creator": "Charles Chaplin",
                        "subject": "comedy; silent", "description": "Four shorts.<br>See <a href=\"x\">IMDb</a> &amp; more.",
                        "collection": ["feature_films", "moviesandfilms"],
                        "licenseurl": "http://creativecommons.org/licenses/publicdomain/",
                    },
                    "files": [
                        { "name": "chaplin.mpeg", "format": "MPEG2", "source": "original", "height": "480" },
                        { "name": "chaplin.mp4", "format": "h.264", "source": "derivative", "original": "chaplin.mpeg", "height": "480" },
                        { "name": "chaplin_512kb.mp4", "format": "512Kb MPEG4", "source": "derivative", "original": "chaplin.mpeg", "height": "240" },
                        { "name": "chaplin.ogv", "format": "Ogg Video", "source": "derivative", "original": "chaplin.mpeg", "height": "304" },
                        { "name": "chaplin.asr.srt", "format": "SubRip", "source": "original" },
                    ],
                }))
            }),
        )
        .route(
            "/metadata/serial",
            get(|| async {
                json_response(StatusCode::OK, json!({
                    "metadata": {
                        "title": "Flash Gordon", "collection": "opensource_movies",
                        "licenseurl": "http://creativecommons.org/publicdomain/mark/1.0/",
                    },
                    "files": [
                        { "name": "ch 2.mp4", "format": "MPEG4", "source": "original", "title": "Chapter 2" },
                        { "name": "ch 1.mp4", "format": "MPEG4", "source": "original", "title": "Chapter 1" },
                    ],
                }))
            }),
        )
        // A 1984 film an uploader marked public domain.
        .route(
            "/metadata/marked",
            get(|| async {
                json_response(StatusCode::OK, json!({
                    "metadata": {
                        "title": "A 1984 film", "year": "1984", "collection": ["feature_films"],
                        "licenseurl": "https://creativecommons.org/publicdomain/mark/1.0/",
                    },
                    "files": [{ "name": "film.mp4", "format": "h.264", "source": "original" }],
                }))
            }),
        )
        // An upload in a public-domain collection with no public-domain mark or licence.
        .route(
            "/metadata/upload",
            get(|| async {
                json_response(StatusCode::OK, json!({
                    "metadata": { "title": "A recent film", "collection": ["feature_films"] },
                    "files": [{ "name": "film.mp4", "format": "h.264", "source": "original" }],
                }))
            }),
        );
    let address = serve(app);
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(vec![patched_extension(
        "internet-archive",
        "https://archive.org",
        address,
    )])
    .await;
    let provider = "dev.raygun.internet-archive/films";
    assert_eq!(host.providers(crate::video::VIDEO_SOURCE).await.len(), 1);

    let popular = host
        .manga_list(provider, crate::manga::MangaList::Popular, 1)
        .await
        .unwrap();
    assert!(popular.has_next_page, "30 of 45 shown");
    assert_eq!(popular.items[1].title, "Flash Gordon");
    assert_eq!(
        popular.items[0].cover_url.as_deref(),
        Some(format!("http://{address}/services/img/chaplin").as_str())
    );
    let mut filters = std::collections::BTreeMap::new();
    filters.insert(
        "collection".to_owned(),
        raygun_model::FilterValue::Select("Film_Noir".into()),
    );
    let noir = host
        .manga_search_filtered(provider, "", 2, &filters)
        .await
        .unwrap();
    assert_eq!(noir.items[0].title, "Detour");
    assert!(!noir.has_next_page, "page 2 of 45 at 30 a page is the last");
    filters.insert(
        "collection".to_owned(),
        raygun_model::FilterValue::Select("animationandcartoons".into()),
    );
    host.manga_search_filtered(provider, "sintel", 1, &filters)
        .await
        .unwrap();
    assert_eq!(host.manga_filters(provider).await.unwrap().len(), 2);

    let details = host.manga_details(provider, "chaplin").await.unwrap();
    assert_eq!(
        (details.media_type.as_deref(), details.year),
        (Some("movie"), Some(1938))
    );
    assert_eq!(
        details.artists,
        ["Charles Chaplin"],
        "creators are directors"
    );
    assert_eq!(details.genres, ["comedy", "silent"]);
    assert_eq!(
        details.description.as_deref(),
        Some("Four shorts.\nSee IMDb & more."),
        "HTML becomes text"
    );

    let film = host.video_episodes(provider, "chaplin").await.unwrap();
    assert_eq!(film.len(), 1);
    assert_eq!(
        (film[0].id.as_str(), film[0].name.as_str()),
        ("chaplin/chaplin.mpeg", "Film")
    );
    let serial = host.video_episodes(provider, "serial").await.unwrap();
    let names: Vec<_> = serial.iter().map(|episode| episode.name.as_str()).collect();
    assert_eq!(names, ["Chapter 1", "Chapter 2"], "in file order");

    let servers = host.video_servers(provider, &film[0].id).await.unwrap();
    assert_eq!(servers.len(), 1);
    let videos = servers[0].videos.as_ref().expect("resolved at once");
    let qualities: Vec<_> = videos.iter().map(|video| video.quality).collect();
    assert_eq!(qualities, [Some(480), Some(240)], "MP4s only, best first");
    assert_eq!(
        videos[0].url,
        format!("http://{address}/download/chaplin/chaplin.mp4")
    );
    assert_eq!(videos[0].label.as_deref(), Some("480p · h.264"));
    assert_eq!(
        videos[0].subtitles[0].label.as_deref(),
        Some("Automatic (English)")
    );
    let again = host.video_links(provider, &film[0].id).await.unwrap();
    assert_eq!(again.len(), 2);

    // An item that isn't public domain doesn't open, even by its id; nor does a recent film whose
    // uploader marked it public domain.
    let refused = host.manga_details(provider, "upload").await.unwrap_err();
    assert!(refused.contains("public domain"), "{refused}");
    assert!(host.video_episodes(provider, "upload").await.is_err());
    assert!(
        host.video_servers(provider, "upload/film.mp4")
            .await
            .is_err()
    );
    assert!(host.manga_details(provider, "marked").await.is_err());
}
