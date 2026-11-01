use serde_json::json;

use super::*;

fn cinemeta() -> Value {
    json!({
        "id": "com.linvo.cinemeta", "version": "3.0.14", "name": "Cinemeta",
        "description": "The official addon for movie and series catalogs",
        "resources": ["catalog", "meta", "addon_catalog"],
        "types": ["movie", "series"], "idPrefixes": ["tt"],
        "catalogs": [
            { "type": "movie", "id": "top", "name": "Popular",
              "extra": [{ "name": "genre", "options": ["Action", "Drama"] }, { "name": "search" }, { "name": "skip" }] },
            { "type": "movie", "id": "imdbRating", "name": "Featured", "genres": ["Action"],
              "extraSupported": ["genre", "skip"] },
            { "type": "series", "id": "search-only", "name": "Search",
              "extra": [{ "name": "search", "isRequired": true }] },
            { "type": "movie", "id": "by-year", "extraRequired": ["year"], "extraSupported": ["year"] },
            { "name": "no id or type" }
        ]
    })
}

#[test]
fn manifests_resources_and_catalogs() {
    let manifest = parse_manifest(&cinemeta()).unwrap();
    assert_eq!(manifest.name, "Cinemeta");
    assert_eq!(
        manifest.catalogs.len(),
        4,
        "the catalog without an id is dropped"
    );
    assert!(manifest.serves("meta", "movie", "tt0133093"));
    assert!(!manifest.serves("meta", "movie", "kitsu:1"), "id prefix");
    assert!(!manifest.serves("meta", "channel", "tt1"), "type");
    assert!(!manifest.serves("stream", "movie", "tt1"), "resource");

    let top = &manifest.catalogs[0];
    assert!(top.browsable() && top.searchable());
    assert_eq!(top.extra("genre").unwrap().options, ["Action", "Drama"]);
    let featured = &manifest.catalogs[1];
    assert_eq!(
        featured.extra("genre").unwrap().options,
        ["Action"],
        "old genres list"
    );
    let search_only = &manifest.catalogs[2];
    assert!(!search_only.browsable() && search_only.search_only_ok());
    let by_year = &manifest.catalogs[3];
    assert!(!by_year.browsable() && !by_year.search_only_ok());

    let objects = parse_manifest(&json!({
        "id": "x", "name": "X", "types": ["movie"],
        "resources": [{ "name": "stream", "types": ["series"], "idPrefixes": ["kitsu:"] }],
        "behaviorHints": { "adult": true, "configurationRequired": true }
    }))
    .unwrap();
    assert!(objects.serves("stream", "series", "kitsu:5:1"));
    assert!(!objects.serves("stream", "movie", "kitsu:5"));
    assert!(objects.adult && objects.configuration_required);

    assert!(parse_manifest(&json!({ "name": "no id", "resources": ["meta"] })).is_err());
    assert!(parse_manifest(&json!({ "id": "x", "resources": [] })).is_err());
}

#[test]
fn resource_urls_and_addresses() {
    let base = "https://v3-cinemeta.strem.io/manifest.json";
    assert_eq!(
        resource_url(base, "meta", "series", "tt0903747", &[]),
        "https://v3-cinemeta.strem.io/meta/series/tt0903747.json"
    );
    assert_eq!(
        resource_url(
            base,
            "catalog",
            "movie",
            "top",
            &[
                ("genre", "Sci-Fi".into()),
                ("search", "the matrix".into()),
                ("skip", "100".into())
            ]
        ),
        "https://v3-cinemeta.strem.io/catalog/movie/top/genre=Sci-Fi&search=the%20matrix&skip=100.json"
    );
    assert_eq!(
        resource_url(base, "stream", "series", "tt0903747:1:2", &[]),
        "https://v3-cinemeta.strem.io/stream/series/tt0903747:1:2.json"
    );
    assert_eq!(
        normalize_addon_url("stremio://addon.example/abc/manifest.json").unwrap(),
        "https://addon.example/abc/manifest.json"
    );
    assert_eq!(
        normalize_addon_url(" addon.example/abc/ ").unwrap(),
        "https://addon.example/abc/manifest.json"
    );
    assert!(normalize_addon_url("javascript:alert(1)").is_err());
}

#[test]
fn catalogs_and_meta() {
    let metas = parse_metas(
        &json!({ "metas": [
            { "id": "tt1", "type": "movie", "name": "One", "poster": "https://img/1.jpg",
              "releaseInfo": "1999", "imdbRating": "8.7" },
            { "id": "tt2", "name": "Two", "poster": "ftp://bad", "posterShape": "landscape" },
            { "name": "No id" }
        ]}),
        "movie",
    );
    assert_eq!(metas.len(), 2);
    assert_eq!(metas[0].rating.as_deref(), Some("8.7"));
    assert_eq!(
        (metas[1].kind.as_str(), metas[1].poster.as_deref()),
        ("movie", None)
    );
    assert_eq!(metas[1].poster_shape, "landscape");

    let meta = parse_meta(
        &json!({ "meta": {
            "id": "tt0903747", "type": "series", "name": "Breaking Bad",
            "genres": ["Crime", "Drama"], "cast": "Bryan Cranston, Aaron Paul",
            "trailerStreams": [{ "ytId": "HhesaQXLuRY" }],
            "videos": [
                { "id": "tt0903747:2:1", "name": "Seven Thirty-Seven", "season": 2, "episode": 1 },
                { "id": "tt0903747:0:1", "title": "Special", "season": 0, "episode": 1 },
                { "id": "tt0903747:1:2", "title": "Cat's in the Bag...", "season": 1, "number": 2,
                  "thumbnail": "https://img/e2.jpg", "firstAired": "2008-01-28" },
                { "id": "tt0903747:1:1", "title": "Pilot", "season": 1, "episode": 1 }
            ]
        }}),
        "series",
    )
    .unwrap();
    assert_eq!(meta.preview.name, "Breaking Bad");
    assert_eq!(meta.cast, ["Bryan Cranston", "Aaron Paul"]);
    assert_eq!(meta.trailer_yt_ids, ["HhesaQXLuRY"]);
    let order: Vec<&str> = meta.videos.iter().map(|video| video.id.as_str()).collect();
    assert_eq!(
        order,
        [
            "tt0903747:1:1",
            "tt0903747:1:2",
            "tt0903747:2:1",
            "tt0903747:0:1"
        ],
        "by season and episode, specials last"
    );
    assert_eq!(meta.videos[1].released.as_deref(), Some("2008-01-28"));
    assert!(parse_meta(&json!({}), "movie").is_none());
}

#[test]
fn streams_and_subtitles() {
    let streams = parse_streams(&json!({ "streams": [
        { "name": "HD", "title": "1080p", "url": "https://cdn.example/v.m3u8",
          "behaviorHints": { "proxyHeaders": { "request": {
              "Referer": "https://site.example/", "Host": "evil", "User-Agent": "UA" } } },
          "subtitles": [{ "url": "https://cdn.example/en.vtt", "lang": "eng" }] },
        { "ytId": "abc123", "name": "Trailer" },
        { "externalUrl": "https://www.netflix.com/title/1", "name": "Netflix" },
        { "infoHash": "5d640678eae57c72c0d096904fd7d7405ece6653", "fileIdx": 1 },
        { "name": "Nothing playable" }
    ]}));
    assert_eq!(streams.len(), 4);
    assert_eq!(streams[0].source, StreamSource::Direct);
    assert_eq!(streams[0].description.as_deref(), Some("1080p"));
    assert_eq!(
        streams[0]
            .headers
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["Referer", "User-Agent"],
        "Host isn't the stream's to set"
    );
    assert_eq!(streams[0].subtitles[0].lang.as_deref(), Some("eng"));
    assert_eq!(
        streams[1..]
            .iter()
            .map(|stream| stream.source)
            .collect::<Vec<_>>(),
        [
            StreamSource::YouTube,
            StreamSource::External,
            StreamSource::Torrent
        ]
    );

    let subtitles = parse_subtitles(&json!({ "subtitles": [
        { "url": "https://s/1.srt", "lang": "por" },
        { "url": "https://s/2.srt", "lang": "eng" },
        { "url": "https://s/2.srt", "lang": "eng" },
        { "url": "https://s/3.srt", "lang": "en" },
        { "lang": "fre" }
    ]}));
    assert_eq!(subtitles.len(), 4);
    let ordered = order_subtitles(subtitles, "en-US", 2);
    assert_eq!(
        ordered
            .iter()
            .map(|subtitle| subtitle.url.as_str())
            .collect::<Vec<_>>(),
        ["https://s/2.srt", "https://s/3.srt"],
        "English first, repeats dropped, limited"
    );
}
