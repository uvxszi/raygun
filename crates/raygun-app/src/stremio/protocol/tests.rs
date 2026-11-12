use serde_json::json;

use super::*;

fn fixture(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

fn extras(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

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
            { "type": "movie", "id": "year", "name": "New",
              "extra": [{ "name": "genre", "isRequired": true, "options": ["2026", "2025"] }] },
            { "type": "series", "id": "last-videos",
              "extra": [{ "name": "lastVideosIds", "isRequired": true, "optionsLimit": 100 }] },
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
        6,
        "the catalog without an id is dropped"
    );
    assert!(manifest.serves("meta", "movie", "tt0133093"));
    assert!(!manifest.serves("meta", "movie", "kitsu:1"), "id prefix");
    assert!(!manifest.serves("meta", "channel", "tt1"), "type");
    assert!(!manifest.serves("stream", "movie", "tt1"), "resource");

    let top = &manifest.catalogs[0];
    assert!(top.browsable() && top.searchable() && top.pages());
    assert_eq!(top.extra("genre").unwrap().options, ["Action", "Drama"]);
    let featured = &manifest.catalogs[1];
    assert_eq!(
        featured.extra("genre").unwrap().options,
        ["Action"],
        "old genres list"
    );
    let search_only = &manifest.catalogs[2];
    assert!(!search_only.browsable() && search_only.searchable());
    assert_eq!(search_only.default_required(), None, "nothing to pick");
    let by_year = &manifest.catalogs[3];
    assert!(!by_year.browsable() && !by_year.searchable());
    let new = &manifest.catalogs[4];
    assert!(!new.browsable(), "a required genre keeps it off the board");
    assert_eq!(
        new.default_required(),
        Some(extras(&[("genre", "2026")])),
        "Discover starts on its first option"
    );
    let last_videos = &manifest.catalogs[5];
    assert_eq!(
        last_videos.extra("lastVideosIds").unwrap().options_limit,
        100
    );
    assert_eq!(last_videos.default_required(), None);

    let objects = parse_manifest(&json!({
        "id": "x", "name": "X", "types": ["movie", "series"], "idPrefixes": ["tt"],
        "resources": [
            { "name": "stream", "types": ["series"], "idPrefixes": ["kitsu:"] },
            { "name": "subtitles", "types": ["movie"] },
            { "name": "meta", "idPrefixes": null }
        ],
        "behaviorHints": { "adult": true, "configurationRequired": true, "configurable": true, "p2p": true }
    }))
    .unwrap();
    assert!(objects.serves("stream", "series", "kitsu:5:1"));
    assert!(
        !objects.serves("stream", "movie", "kitsu:5"),
        "its own types"
    );
    assert!(
        !objects.serves("stream", "series", "tt1:1:1"),
        "its own prefixes"
    );
    assert!(
        objects.serves("subtitles", "movie", "kitsu:1"),
        "an object without prefixes takes every id, as in stremio-core"
    );
    assert!(
        objects.serves("meta", "series", "anything"),
        "an object without types takes the manifest's"
    );
    assert!(objects.adult && objects.configuration_required && objects.configurable && objects.p2p);
    assert_eq!(objects.resource_types("stream"), ["series"]);

    assert!(parse_manifest(&json!({ "name": "no id", "resources": ["meta"] })).is_err());
    assert!(parse_manifest(&json!({ "id": "x", "resources": [] })).is_err());
}

#[test]
fn a_tmdb_style_manifest() {
    let manifest = parse_manifest(&fixture(include_str!("fixtures/tmdb_manifest.json"))).unwrap();
    assert!(manifest.configurable && !manifest.configuration_required);
    assert_eq!(
        manifest.catalogs.len(),
        3,
        "the repeated type and id is read once"
    );
    assert_eq!(manifest.catalogs[0].name, "Popular");
    assert!(manifest.serves("meta", "movie", "tmdb:969681"));
    assert!(!manifest.serves("meta", "movie", "tt0032138"));
    let search = &manifest.catalogs[2];
    assert!(search.searchable() && !search.browsable());

    let popular = &manifest.catalogs[0];
    // Stremio sends skip first; unknown extras and values outside the options are left out.
    assert_eq!(
        popular.request_extras(
            &extras(&[("genre", "Action"), ("year", "2020"), ("search", "x")]),
            40
        ),
        extras(&[("skip", "40"), ("genre", "Action")])
    );
    assert_eq!(
        popular.request_extras(&extras(&[("genre", "Westerns")]), 0),
        Vec::<(String, String)>::new()
    );
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
            &extras(&[
                ("skip", "100"),
                ("genre", "Sci-Fi"),
                ("search", "the matrix")
            ])
        ),
        "https://v3-cinemeta.strem.io/catalog/movie/top/skip=100&genre=Sci-Fi&search=the%20matrix.json"
    );
    // `:` is encoded as Stremio's own clients encode it: addons behind a cache answer that.
    assert_eq!(
        resource_url(base, "stream", "series", "tt0903747:1:2", &[]),
        "https://v3-cinemeta.strem.io/stream/series/tt0903747%3A1%3A2.json"
    );
    assert_eq!(
        resource_url(
            "https://addon.example/%7B%22lang%22%3A%22en%22%7D/manifest.json?token=a%2Fb",
            "meta",
            "movie",
            "tmdb:1",
            &[]
        ),
        "https://addon.example/%7B%22lang%22%3A%22en%22%7D/meta/movie/tmdb%3A1.json?token=a%2Fb",
        "a configured address keeps its path and query"
    );
    assert_eq!(
        normalize_addon_url("stremio://addon.example/abc/manifest.json").unwrap(),
        "https://addon.example/abc/manifest.json"
    );
    assert_eq!(
        normalize_addon_url(" addon.example/abc/ ").unwrap(),
        "https://addon.example/abc/manifest.json"
    );
    assert_eq!(
        normalize_addon_url("https://addon.example/manifest.json?key=1#top").unwrap(),
        "https://addon.example/manifest.json?key=1",
        "a query stays where it is"
    );
    assert_eq!(
        normalize_addon_url("https://addon.example/cfg/configure").unwrap(),
        "https://addon.example/cfg/manifest.json",
        "a configuration page leads to its manifest"
    );
    assert_eq!(
        normalize_addon_url("http://192.168.1.5:7000").unwrap(),
        "http://192.168.1.5:7000/manifest.json"
    );
    assert!(normalize_addon_url("javascript:alert(1)").is_err());
    assert_eq!(
        base_url("https://addon.example/cfg/manifest.json?key=1"),
        "https://addon.example/cfg"
    );
    assert_eq!(
        configure_url("https://addon.example/manifest.json"),
        "https://addon.example/configure"
    );
}

#[test]
fn the_legacy_protocol() {
    let transport = normalize_addon_url("https://opensubtitles.strem.io/stremio/v1").unwrap();
    assert_eq!(transport, "https://opensubtitles.strem.io/stremio/v1");
    assert!(is_legacy(&transport));
    assert!(
        manifest_url(&transport)
            .starts_with("https://opensubtitles.strem.io/stremio/v1/q.json?b=eyJwYXJhbXMiOltd")
    );

    let manifest = legacy_answer(
        "manifest",
        fixture(include_str!("fixtures/legacy_manifest.json")),
    )
    .and_then(|value| parse_manifest(&value))
    .unwrap();
    assert_eq!(manifest.name, "OpenSubtitles");
    assert!(manifest.serves("subtitles", "movie", "tt0032138"));
    assert!(manifest.catalogs.is_empty());

    // stremio-core's own example: subtitles of tt0386676:5:1 are asked by `itemHash`.
    let url = resource_url(
        "https://legacywatchhub.strem.io/stremio/v1",
        "subtitles",
        "series",
        "tt0386676:5:1",
        &[],
    );
    let encoded = url.split("?b=").nth(1).unwrap();
    let request: Value = serde_json::from_slice(
        &base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(request["method"], "subtitles.find");
    assert_eq!(request["params"][1]["query"]["itemHash"], "tt0386676 5 1");
    let stream = resource_url(
        "https://legacywatchhub.strem.io/stremio/v1",
        "stream",
        "series",
        "tt0386676:5:2",
        &[],
    );
    let request: Value = serde_json::from_slice(
        &base64::engine::general_purpose::STANDARD
            .decode(stream.split("?b=").nth(1).unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        request["params"][1]["query"],
        json!({ "imdb_id": "tt0386676", "season": 5, "episode": 2, "type": "series" })
    );
    assert_eq!(
        legacy_answer(
            "subtitles",
            json!({ "result": { "id": "x", "all": [{ "url": "https://s/1.srt", "lang": "eng" }] } })
        )
        .map(|value| parse_subtitles(&value).len()),
        Ok(1)
    );
    assert_eq!(
        legacy_answer(
            "meta",
            json!({ "error": { "message": "not found", "code": 0 } })
        ),
        Err("not found".to_owned())
    );
}

#[test]
fn catalog_pages() {
    let metas = parse_metas(
        &fixture(include_str!("fixtures/catalog_page.json")),
        "movie",
    );
    let ids: Vec<&str> = metas.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(
        ids,
        ["tt12042730", "tt39195971", "csi-en-espanol"],
        "once each; an item with neither a name nor a picture is left out"
    );
    assert_eq!(
        metas[0].release_info.as_deref(),
        Some("2026"),
        "numbers read as text"
    );
    assert_eq!(metas[0].rating.as_deref(), Some("8.2"));
    assert_eq!(metas[1].name, "", "a poster alone is enough for a card");
    assert_eq!(
        (metas[2].kind.as_str(), metas[2].poster.as_deref()),
        ("movie", None)
    );

    let many: Vec<Value> = (0..1_600)
        .map(|index| json!({ "id": format!("ch{index}"), "type": "tv", "name": "Channel" }))
        .collect();
    assert_eq!(
        parse_metas(&json!({ "metas": many }), "tv").len(),
        1_600,
        "a channel list on one page is read whole"
    );
    assert_eq!(
        parse_metas(
            &json!({ "metasDetailed": [{ "id": "tt1", "name": "One" }] }),
            "movie"
        )
        .len(),
        1
    );
    assert!(parse_metas(&json!({ "metas": null }), "movie").is_empty());
}

#[test]
fn catalogs_and_meta() {
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
                { "id": "tt0903747:1:1", "title": "Pilot", "season": 1, "episode": 1 },
                { "id": "extra", "title": "Behind the scenes" }
            ]
        }}),
        "series",
    )
    .unwrap();
    assert_eq!(meta.preview.name, "Breaking Bad");
    assert_eq!(meta.cast, ["Bryan Cranston", "Aaron Paul"]);
    assert_eq!(meta.trailer_yt_ids, ["HhesaQXLuRY"]);
    assert_eq!(meta.imdb_id.as_deref(), Some("tt0903747"));
    let order: Vec<&str> = meta.videos.iter().map(|video| video.id.as_str()).collect();
    assert_eq!(
        order,
        [
            "tt0903747:1:1",
            "tt0903747:1:2",
            "tt0903747:2:1",
            "tt0903747:0:1",
            "extra"
        ],
        "by season and episode, specials after, unnumbered videos last"
    );
    assert_eq!(meta.videos[1].released.as_deref(), Some("2008-01-28"));
    assert!(parse_meta(&json!({}), "movie").is_none());
    assert!(
        parse_meta(
            &json!({ "meta": { "id": "tt0032138", "type": "movie" } }),
            "movie"
        )
        .is_none(),
        "details without a name are no details: the next addon is asked"
    );
}

#[test]
fn a_movie_known_by_another_id() {
    let meta = parse_meta(
        &fixture(include_str!("fixtures/tmdb_movie_meta.json")),
        "movie",
    )
    .unwrap();
    assert_eq!(meta.default_video_id.as_deref(), Some("tt22084616"));
    assert_eq!(meta.own_video_id(), "tt22084616");
    assert_eq!(meta.trailer_yt_ids, ["P3uI5sLosKU"]);
    assert_eq!(meta.genres, ["Science Fiction", "Action", "Adventure"]);
    assert_eq!(meta.ratings[0].value, "8.0");

    // Details a TMDB addon gives by its own id: streams are asked by IMDb's too.
    let mut tmdb = meta.clone();
    tmdb.preview.id = "tmdb:969681".into();
    tmdb.default_video_id = None;
    tmdb.tmdb_id = Some("969681".into());
    assert_eq!(
        tmdb.stream_ids_for_video("tmdb:969681"),
        ["tmdb:969681", "tt22084616"],
        "its own id, then IMDb's"
    );
}

#[test]
fn anime_episodes_map_to_imdb() {
    let meta = parse_meta(&fixture(include_str!("fixtures/kitsu_meta.json")), "series").unwrap();
    let ids: Vec<&str> = meta.videos.iter().map(|video| video.id.as_str()).collect();
    assert_eq!(
        ids,
        ["kitsu:1:1", "kitsu:1:2", "kitsu:1:27"],
        "sorted, once each, numbers written as text read"
    );
    assert_eq!(
        meta.stream_ids_for_video("kitsu:1:2"),
        ["kitsu:1:2", "tt0213338:1:2"],
        "the video's own IMDb mapping, not the anime database's numbering"
    );
    assert_eq!(
        meta.stream_ids_for_video("kitsu:1:27"),
        ["kitsu:1:27"],
        "no mapping for it, and anime numbering isn't IMDb's"
    );

    let tmdb_series = parse_meta(
        &json!({ "meta": { "id": "tmdb:1396", "type": "series", "name": "Breaking Bad",
            "imdb_id": "tt0903747",
            "videos": [{ "id": "tmdb:1396:1:1", "season": 1, "episode": 1, "title": "Pilot" }] } }),
        "series",
    )
    .unwrap();
    assert_eq!(
        tmdb_series.stream_ids_for_video("tmdb:1396:1:1"),
        ["tmdb:1396:1:1", "tt0903747:1:1"],
        "TMDB numbers episodes as IMDb does"
    );
}

#[test]
fn channel_videos_play_their_own_streams() {
    let meta = parse_meta(
        &fixture(include_str!("fixtures/channel_meta.json")),
        "channel",
    )
    .unwrap();
    let titles: Vec<&str> = meta
        .videos
        .iter()
        .map(|video| video.title.as_str())
        .collect();
    assert_eq!(
        titles,
        [
            "Newer upload",
            "Older upload",
            "A video with its own stream"
        ],
        "a channel's videos, newest first"
    );
    let youtube = &meta.videos[0].streams;
    assert_eq!(youtube.len(), 1);
    assert_eq!(
        (youtube[0].source, youtube[0].target.as_str()),
        (StreamSource::YouTube, "oNIN5ZY5t54"),
        "a yt_id video plays on YouTube"
    );
    assert_eq!(
        meta.videos[0].released.as_deref(),
        Some("2023-04-28T15:25:00.000Z")
    );
    let own = &meta.videos[2].streams;
    assert_eq!(
        (own[0].source, own[0].target.as_str()),
        (StreamSource::Direct, "https://cdn.example/live-1.m3u8")
    );
    assert!(!meta.live);
}

#[test]
fn streams_of_every_kind() {
    let streams = parse_streams(&fixture(include_str!("fixtures/streams_kinds.json")));
    let kinds: Vec<(StreamSource, Option<&str>)> = streams
        .iter()
        .map(|stream| (stream.source, stream.name.as_deref()))
        .collect();
    assert_eq!(
        kinds,
        [
            (StreamSource::Direct, Some("Pluto TV")),
            (StreamSource::YouTube, Some("Trailer")),
            (StreamSource::External, Some("Netflix")),
            (StreamSource::Torrent, Some("Torrent\n1080p")),
            (StreamSource::Torrent, Some("Magnet")),
            (StreamSource::Server, Some("Usenet")),
            (StreamSource::Server, Some("Archive")),
            (StreamSource::Server, Some("FTP")),
            (StreamSource::Title, Some("Oz the Great and Powerful")),
            (StreamSource::Title, Some("Cowboy Bebop")),
            (StreamSource::External, Some("Embed")),
        ],
        "a broken hash and a stream with nothing to play are dropped"
    );
    let hls = &streams[0];
    assert_eq!(hls.description.as_deref(), Some("CSI en Español"));
    assert!(
        hls.target
            .ends_with("master.m3u8?appName=web&deviceType=web")
    );
    assert_eq!(
        hls.headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect::<Vec<_>>(),
        [
            ("Referer", "https://site.example/"),
            ("User-Agent", "UA"),
            ("X-Token", "5")
        ],
        "the player's own headers, odd names and injected lines aren't the stream's to set"
    );
    assert_eq!(hls.subtitles[0].lang.as_deref(), Some("eng"));
    assert_eq!(hls.binge_group.as_deref(), Some("pluto-hls"));
    assert_eq!(
        (hls.filename.as_deref(), hls.size),
        (Some("csi.m3u8"), Some(1_234_567))
    );
    assert!(hls.not_web_ready);

    let torrent = &streams[3];
    assert_eq!(
        torrent.target,
        "magnet:?xt=urn:btih:5d640678eae57c72c0d096904fd7d7405ece6653&dn=Movie%201999.mkv\
         &tr=udp%3A%2F%2Ftracker.example%3A80%2Fannounce&so=1"
    );
    assert_eq!(torrent.description.as_deref(), Some("Movie.1999.1080p.mkv"));
    assert_eq!(streams[8].target, "movie/tt1623205");
    assert_eq!(streams[9].target, "series/kitsu:1", "decoded");
}

#[test]
fn subtitles_in_the_preferred_language() {
    let subtitles = parse_subtitles(&json!({ "subtitles": [
        { "url": "https://s/1.srt", "lang": "por" },
        { "url": "https://s/2.srt", "lang": "eng" },
        { "url": "https://s/2.srt", "lang": "eng" },
        { "url": "https://s/3.srt", "lang": "en" },
        { "url": "https://s/4.srt", "lang": "English" },
        { "lang": "fre" }
    ]}));
    assert_eq!(subtitles.len(), 4);
    let ordered = order_subtitles(subtitles, "en-US", 3);
    assert_eq!(
        ordered
            .iter()
            .map(|subtitle| subtitle.url.as_str())
            .collect::<Vec<_>>(),
        ["https://s/2.srt", "https://s/3.srt", "https://s/4.srt"],
        "English by any of its names first, repeats dropped, limited"
    );
    let portuguese = order_subtitles(
        parse_subtitles(&json!({ "subtitles": [
            { "url": "https://s/a.srt", "lang": "eng" },
            { "url": "https://s/b.srt", "lang": "pob" }
        ]})),
        "pt-BR",
        1,
    );
    assert_eq!(portuguese[0].url, "https://s/b.srt");

    // Thousands of subtitles: the preferred ones among them aren't lost.
    let mut many: Vec<Value> = (0..5_000)
        .map(|index| json!({ "url": format!("https://s/{index}.srt"), "lang": "ara" }))
        .collect();
    many.push(json!({ "url": "https://s/last.srt", "lang": "eng" }));
    let ordered = order_subtitles(parse_subtitles(&json!({ "subtitles": many })), "en", 20);
    assert_eq!(ordered[0].url, "https://s/last.srt");
}

#[test]
fn ratings_name_their_sites_and_link_imdb() {
    let meta = json!({
        "links": [
            { "name": "8.7", "category": "imdb", "url": "https://imdb.com/title/tt0133093" },
            { "name": "Action", "category": "Genres", "url": "stremio:///discover/x" },
            { "name": "88%", "category": "Rotten Tomatoes", "url": "https://www.rottentomatoes.com/m/matrix" },
            { "name": "Keanu Reeves", "category": "Cast" }
        ]
    });
    let ratings = parse_ratings(&meta, "tt0133093", Some("8.7"));
    let sources: Vec<(&str, &str)> = ratings
        .iter()
        .map(|rating| (rating.source.as_str(), rating.value.as_str()))
        .collect();
    assert_eq!(sources, [("IMDb", "8.7"), ("Rotten Tomatoes", "88%")]);

    // Without links, `imdbRating` is IMDb's, linked by the IMDb id.
    let bare = parse_ratings(&json!({}), "tt0133093", Some("8.7"));
    assert_eq!(
        bare[0].url.as_deref(),
        Some("https://www.imdb.com/title/tt0133093/")
    );
    assert!(parse_ratings(&json!({}), "kitsu:1", None).is_empty());
}

/// An addon manifest with these resources, types and catalogue types.
fn manifest_of(id: &str, resources: Value, types: &[&str], catalogs: &[&str]) -> Manifest {
    let catalogs: Vec<Value> = catalogs
        .iter()
        .enumerate()
        .map(|(index, kind)| json!({ "type": kind, "id": format!("c{index}"), "name": "List" }))
        .collect();
    parse_manifest(&json!({
        "id": id, "name": id, "version": "1.0.0",
        "resources": resources, "types": types, "catalogs": catalogs,
    }))
    .unwrap()
}

#[test]
fn addons_raygun_reads_nothing_from_are_not_for_raygun() {
    let collection = manifest_of("addons.list", json!(["addon_catalog"]), &["movie"], &[]);
    assert_eq!(
        collection.not_for_raygun(&[]).as_deref(),
        Some(
            "Only lists other Stremio addons, which Raygun doesn't read: add those addons themselves"
        )
    );
    let unknown = manifest_of("odd", json!(["calendar"]), &["movie"], &[]);
    assert_eq!(
        unknown.not_for_raygun(&[]).as_deref(),
        Some("Offers only calendar, which Raygun doesn't read")
    );
    // Only catalogues of a type nothing here opens or plays.
    let music = manifest_of(
        "music.lists",
        json!(["catalog"]),
        &["music"],
        &["music", "music"],
    );
    assert_eq!(
        music.not_for_raygun(&[]).as_deref(),
        Some("Only lists catalogs of music, which no addon here opens or plays")
    );
    // Another addon giving details for that type opens them.
    let music_meta = manifest_of("music.meta", json!(["meta", "stream"]), &["music"], &[]);
    assert_eq!(music.not_for_raygun(&[&music_meta]), None);
    // Itself doesn't count.
    assert!(music.not_for_raygun(&[&music]).is_some());
    // Lists of films open through the details and streams of other addons, as in Stremio.
    let films = manifest_of("films.lists", json!(["catalog"]), &["movie"], &["movie"]);
    assert_eq!(films.not_for_raygun(&[]), None);
    let empty = manifest_of("empty.lists", json!(["catalog"]), &["movie"], &[]);
    assert_eq!(
        empty.not_for_raygun(&[]).as_deref(),
        Some("Lists no catalogs, so it has nothing to show")
    );
    for useful in [
        manifest_of("streams", json!(["stream"]), &["movie"], &[]),
        manifest_of("subtitles", json!(["subtitles"]), &["movie", "series"], &[]),
        manifest_of(
            "details",
            json!(["catalog", "meta"]),
            &["music"],
            &["music"],
        ),
        parse_manifest(&cinemeta()).unwrap(),
    ] {
        assert_eq!(useful.not_for_raygun(&[]), None, "{}", useful.id);
    }
}

/// An SDK addon's settings, as its manifest declares them.
fn configurable() -> Manifest {
    parse_manifest(&json!({
        "id": "org.example.configurable", "name": "Configurable", "version": "1.0.0",
        "resources": ["stream"], "types": ["movie"],
        "behaviorHints": { "configurable": true, "configurationRequired": true },
        "config": [
            { "key": "apiKey", "type": "text", "title": "API key", "required": true },
            { "key": "region", "type": "text", "title": "Region (optional)", "required": true },
            { "key": "proxy", "type": "text", "title": "Proxy" },
            { "key": "limit", "type": "number", "title": "Limit", "default": 5, "required": "false" },
            { "key": "secret", "type": "password", "required": false },
            { "key": "hdOnly", "type": "checkbox", "title": "HD only", "default": "checked" },
            { "key": "lowFirst", "type": "checkbox", "title": "Low first" },
            { "key": "quality", "type": "select", "title": "Quality", "options": ["1080p", "720p"] },
            { "key": "sort", "type": "select", "options": ["size", "seeds"], "default": "seeds" },
            { "key": "nothing", "type": "select" },
            { "key": "broken", "type": "slider" },
            { "type": "text", "title": "No key" },
            { "key": "apiKey", "type": "text", "title": "Twice" }
        ]
    }))
    .unwrap()
}

#[test]
fn only_required_settings_are_required() {
    let config = configurable().config;
    let shown: Vec<(&str, StremioConfigKind, &str, bool)> = config
        .iter()
        .map(|field| {
            (
                field.key.as_str(),
                field.kind,
                field.title.as_str(),
                field.required,
            )
        })
        .collect();
    use StremioConfigKind::{Checkbox, Number, Password, Select, Text};
    assert_eq!(
        shown,
        [
            ("apiKey", Text, "API key", true),
            // "(optional)" in its title wins over `required`.
            ("region", Text, "Region (optional)", false),
            // No `required` is the SDK's `false`.
            ("proxy", Text, "Proxy", false),
            ("limit", Number, "Limit", false),
            // The title falls back on the key.
            ("secret", Password, "secret", false),
            ("hdOnly", Checkbox, "HD only", false),
            ("lowFirst", Checkbox, "Low first", false),
            ("quality", Select, "Quality", false),
            ("sort", Select, "sort", false),
        ],
        "fields without a key, of a type the SDK doesn't draw, or a select without choices, are \
         left out; a key comes once"
    );
    assert_eq!(config[3].default.as_deref(), Some("5"));
}

fn values(pairs: &[(&str, &str)]) -> Vec<StremioConfigValue> {
    pairs
        .iter()
        .map(|(key, value)| StremioConfigValue {
            key: (*key).to_owned(),
            value: (*value).to_owned(),
        })
        .collect()
}

#[test]
fn configured_addresses_are_built_as_the_sdk_builds_them() {
    let fields = configurable().config;
    let address = "https://addon.example.org/manifest.json";
    // Optional settings stay empty: they're sent empty, as the SDK's form sends them; ticked
    // boxes are sent as `on` and unticked ones left out; selects send their default or first
    // choice; untouched fields keep their defaults.
    let url = configured_url(
        address,
        &fields,
        &values(&[("apiKey", "k&y/1"), ("region", ""), ("hdOnly", "on")]),
    )
    .unwrap();
    let json = r#"{"apiKey":"k&y/1","region":"","proxy":"","limit":"5","secret":"","hdOnly":"on","quality":"1080p","sort":"seeds"}"#;
    assert_eq!(
        url,
        format!("https://addon.example.org/{}/manifest.json", encode(json))
    );
    assert!(url.starts_with("https://addon.example.org/%7B%22apiKey%22%3A%22k%26y%2F1%22"));
    // A required one can't be left empty.
    assert_eq!(
        configured_url(address, &fields, &values(&[("apiKey", "  ")])),
        Err("Fill in API key".to_owned())
    );
    assert_eq!(
        configured_url(
            address,
            &fields,
            &values(&[("apiKey", "k"), ("limit", "many")])
        ),
        Err("Limit takes a number".to_owned())
    );
    // Unticking a box ticked by default, and a choice that isn't an option.
    let url = configured_url(
        address,
        &fields,
        &values(&[
            ("apiKey", "k"),
            ("hdOnly", ""),
            ("lowFirst", "on"),
            ("sort", "nope"),
        ]),
    )
    .unwrap();
    let mut read: Vec<(String, String)> = config_in_url(&url)
        .into_iter()
        .map(|value| (value.key, value.value))
        .collect();
    read.sort();
    let mut sent = extras(&[
        ("apiKey", "k"),
        ("region", ""),
        ("proxy", ""),
        ("limit", "5"),
        ("secret", ""),
        ("lowFirst", "on"),
        ("quality", "1080p"),
        ("sort", "seeds"),
    ]);
    sent.sort();
    assert_eq!(read, sent, "a setup in the address reads back");
    // Set up again: the setup in the address is replaced, not nested.
    let again = configured_url(&url, &fields, &values(&[("apiKey", "new")])).unwrap();
    assert!(again.starts_with("https://addon.example.org/%7B%22apiKey%22%3A%22new%22"));
    assert_eq!(again.matches("manifest.json").count(), 1);
    // An addon in a folder of its own keeps it.
    let nested = configured_url(
        "https://host.example.org/addons/films/manifest.json",
        &fields,
        &values(&[("apiKey", "k")]),
    )
    .unwrap();
    assert!(nested.starts_with("https://host.example.org/addons/films/%7B"));
    assert!(config_in_url(address).is_empty());
    assert!(config_in_url("https://host.example.org/providers=a|b/manifest.json").is_empty());
}
