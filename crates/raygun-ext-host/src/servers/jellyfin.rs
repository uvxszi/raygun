//! Jellyfin and Emby servers (Emby's API, which Jellyfin forked, under `/emby` and with its own
//! sign-in header): films and series as a `video.source`, music as `music.metadata`,
//! `music.stream` and `lyrics`. Requests sign in with the access token the server gave at sign-in
//! (or an API key); stored covers need none, since both servers serve images to anyone.

use std::collections::BTreeMap;

use reqwest::Url;
use serde::Deserialize;
use serde_json::{Value, json};

use super::http::{Http, date_ms, id_arg, number_arg, segment as path_part, text_arg, under};
use crate::failure::Failure;

/// Titles per catalogue page.
const PAGE_SIZE: u32 = 30;
/// Ticks (100 ns) per millisecond.
const TICKS_PER_MS: i64 = 10_000;
/// The app's name and version as servers list the device.
const CLIENT: &str = "Raygun";

/// A Jellyfin or Emby server signed in to.
pub(crate) struct Jellyfin {
    http: Http,
    base: Url,
    user_id: String,
    token: String,
    device_id: String,
    emby: bool,
}

/// The headers that sign requests in: Jellyfin's `Authorization: MediaBrowser …`, Emby's
/// `X-Emby-Authorization` with `X-Emby-Token`.
pub(crate) fn auth_headers(
    emby: bool,
    device_id: &str,
    token: Option<&str>,
) -> Vec<(String, String)> {
    let version = env!("CARGO_PKG_VERSION");
    let mut client = format!(
        "MediaBrowser Client=\"{CLIENT}\", Device=\"{CLIENT}\", DeviceId=\"{device_id}\", Version=\"{version}\""
    );
    if emby {
        let mut headers = vec![("X-Emby-Authorization".to_owned(), client)];
        if let Some(token) = token {
            headers.push(("X-Emby-Token".to_owned(), token.to_owned()));
        }
        headers
    } else {
        if let Some(token) = token {
            client.push_str(&format!(", Token=\"{token}\""));
        }
        vec![("Authorization".to_owned(), client)]
    }
}

/// What `System/Info/Public` says about a server.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub(crate) struct PublicInfo {
    pub server_name: String,
    pub product_name: String,
    pub version: String,
}

impl PublicInfo {
    pub fn is_emby(&self) -> bool {
        self.product_name.to_ascii_lowercase().contains("emby")
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SignedIn {
    access_token: String,
    user: SignedUser,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SignedUser {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Me {
    id: String,
}

/// Signs in with a user name and password: the user's id and the access token to keep.
pub(crate) async fn sign_in(
    http: &Http,
    base: &Url,
    emby: bool,
    device_id: &str,
    username: &str,
    password: &str,
) -> Result<(String, String), Failure> {
    let mut request = http
        .client()
        .post(under(base, "Users/AuthenticateByName"))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(json!({ "Username": username, "Pw": password }).to_string());
    for (name, value) in auth_headers(emby, device_id, None) {
        request = request.header(name, value);
    }
    let signed: SignedIn = http.json(request).await?;
    Ok((signed.user.id, signed.access_token))
}

/// The user an API key or token belongs to; API keys belong to none, and then the first user
/// the server lists is used.
pub(crate) async fn token_user(
    http: &Http,
    base: &Url,
    emby: bool,
    device_id: &str,
    token: &str,
) -> Result<String, Failure> {
    let sign = |request: reqwest::RequestBuilder| {
        auth_headers(emby, device_id, Some(token))
            .into_iter()
            .fold(request, |request, (name, value)| {
                request.header(name, value)
            })
    };
    match http
        .json::<Me>(sign(http.client().get(under(base, "Users/Me"))))
        .await
    {
        Ok(me) => Ok(me.id),
        // A key belongs to no user, so `Users/Me` may refuse it; the user list says whether the
        // server takes it.
        Err(_) => {
            let users: Vec<Me> = http
                .json(sign(http.client().get(under(base, "Users"))))
                .await?;
            users
                .into_iter()
                .next()
                .map(|user| user.id)
                .ok_or_else(|| Failure::said("the server lists no users for this key"))
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct ItemsPage {
    items: Vec<Item>,
    total_record_count: Option<i64>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(rename_all = "PascalCase", default)]
struct Item {
    id: String,
    name: String,
    #[serde(rename = "Type")]
    kind: String,
    overview: Option<String>,
    genres: Vec<String>,
    production_year: Option<i32>,
    premiere_date: Option<String>,
    run_time_ticks: Option<i64>,
    index_number: Option<f64>,
    parent_index_number: Option<f64>,
    status: Option<String>,
    people: Vec<Person>,
    image_tags: BTreeMap<String, String>,
    series_id: Option<String>,
    series_primary_image_tag: Option<String>,
    album: Option<String>,
    album_id: Option<String>,
    album_primary_image_tag: Option<String>,
    album_artist: Option<String>,
    artists: Vec<String>,
    child_count: Option<i64>,
    has_lyrics: Option<bool>,
    media_sources: Vec<MediaSourceInfo>,
    chapters: Vec<Chapter>,
    collection_type: Option<String>,
    container: Option<String>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(rename_all = "PascalCase", default)]
struct Person {
    name: String,
    #[serde(rename = "Type")]
    kind: String,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(rename_all = "PascalCase", default)]
struct MediaSourceInfo {
    id: String,
    name: Option<String>,
    container: Option<String>,
    bitrate: Option<i64>,
    media_streams: Vec<MediaStream>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(rename_all = "PascalCase", default)]
struct MediaStream {
    #[serde(rename = "Type")]
    kind: String,
    index: i64,
    codec: Option<String>,
    language: Option<String>,
    display_title: Option<String>,
    is_external: bool,
    is_text_subtitle_stream: bool,
    height: Option<i64>,
    bit_rate: Option<i64>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(rename_all = "PascalCase", default)]
struct Chapter {
    start_position_ticks: i64,
    name: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct Segments {
    items: Vec<Segment>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct Segment {
    #[serde(rename = "Type")]
    kind: String,
    start_ticks: i64,
    end_ticks: i64,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct LyricsAnswer {
    lyrics: Vec<LyricLine>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct LyricLine {
    text: String,
    start: Option<i64>,
}

/// Fields every listing asks for.
const LIST_FIELDS: &str = "PrimaryImageAspectRatio,ProductionYear,Genres";

impl Jellyfin {
    pub fn new(
        http: Http,
        base: Url,
        user_id: String,
        token: String,
        device_id: String,
        emby: bool,
    ) -> Self {
        Self {
            http,
            base,
            user_id,
            token,
            device_id,
            emby,
        }
    }

    /// The headers that sign in, for streams the player fetches.
    pub fn headers(&self) -> Vec<(String, String)> {
        auth_headers(self.emby, &self.device_id, Some(&self.token))
    }

    fn get(&self, path: &str, query: &[(&str, String)]) -> reqwest::RequestBuilder {
        let mut url = under(&self.base, path);
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query);
        }
        self.headers().into_iter().fold(
            self.http
                .client()
                .get(url)
                .header(reqwest::header::ACCEPT, "application/json"),
            |request, (name, value)| request.header(name, value),
        )
    }

    async fn items(&self, query: Vec<(&str, String)>) -> Result<ItemsPage, Failure> {
        let mut query = query;
        query.push(("userId", self.user_id.clone()));
        self.http.json(self.get("Items", &query)).await
    }

    async fn item(&self, id: &str, fields: &str) -> Result<Item, Failure> {
        self.http
            .json(self.get(
                &format!("Items/{}", path_part(id)),
                &[
                    ("userId", self.user_id.clone()),
                    ("Fields", fields.to_owned()),
                ],
            ))
            .await
    }

    fn image(&self, id: &str, tag: Option<&str>, height: u32) -> Option<String> {
        let tag = tag?;
        let mut url = under(
            &self.base,
            &format!("Items/{}/Images/Primary", path_part(id)),
        );
        url.query_pairs_mut()
            .append_pair("tag", tag)
            .append_pair("maxHeight", &height.to_string())
            .append_pair("quality", "90");
        Some(url.to_string())
    }

    /// An item's poster: its own, else its series' (episodes) or album's (songs).
    fn cover(&self, item: &Item, height: u32) -> Option<String> {
        self.image(
            &item.id,
            item.image_tags.get("Primary").map(String::as_str),
            height,
        )
        .or_else(|| {
            self.image(
                item.series_id.as_deref()?,
                item.series_primary_image_tag.as_deref(),
                height,
            )
        })
        .or_else(|| {
            self.image(
                item.album_id.as_deref()?,
                item.album_primary_image_tag.as_deref(),
                height,
            )
        })
    }

    // ---- video.source ----

    pub async fn video_call(&self, method: &str, args: &Value) -> Result<Value, Failure> {
        match method {
            "popular" => self.catalog(args, "CommunityRating", None).await,
            "latest" => self.catalog(args, "DateCreated", None).await,
            "search" => {
                let query = text_arg(args, "query");
                self.catalog(args, "SortName", query.as_deref()).await
            }
            "filters" => self.filters().await,
            "details" => self.details(&id_arg(args, "id")?).await,
            "episodes" => self.episodes(&id_arg(args, "id")?).await,
            "servers" => self.servers(&id_arg(args, "episodeId")?).await,
            "videos" => self.videos(&id_arg(args, "serverId")?).await,
            other => Err(no_method("videos", other)),
        }
    }

    /// A page of films and series, newest, best rated or found, narrowed by the filters.
    async fn catalog(
        &self,
        args: &Value,
        sort: &str,
        search: Option<&str>,
    ) -> Result<Value, Failure> {
        let page = number_arg(args, "page", 1, 1..=10_000);
        let filters = args.get("filters").cloned().unwrap_or(Value::Null);
        let choice = |key: &str| {
            filters
                .get(key)
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty() && *value != "all" && *value != "any")
                .map(str::to_owned)
        };
        let types = match choice("type").as_deref() {
            Some("movie") => "Movie",
            Some("series") => "Series",
            _ => "Movie,Series",
        };
        let (sort_by, order) = match choice("sort").as_deref() {
            Some("added") => ("DateCreated", "Descending"),
            Some("released") => ("PremiereDate", "Descending"),
            Some("rating") => ("CommunityRating", "Descending"),
            Some("name") => ("SortName", "Ascending"),
            _ if sort == "SortName" => ("SortName", "Ascending"),
            _ => (sort, "Descending"),
        };
        let mut query = vec![
            ("Recursive", "true".to_owned()),
            ("IncludeItemTypes", types.to_owned()),
            ("SortBy", format!("{sort_by},SortName")),
            ("SortOrder", order.to_owned()),
            ("StartIndex", ((page - 1) * PAGE_SIZE).to_string()),
            ("Limit", PAGE_SIZE.to_string()),
            ("Fields", LIST_FIELDS.to_owned()),
            ("ImageTypeLimit", "1".to_owned()),
            ("EnableTotalRecordCount", "true".to_owned()),
        ];
        if let Some(term) = search {
            query.push(("searchTerm", term.to_owned()));
        }
        if let Some(library) = choice("library") {
            query.push(("ParentId", library));
        }
        if let Some(genre) = choice("genre") {
            query.push(("Genres", genre));
        }
        if filters.get("unplayed").and_then(Value::as_bool) == Some(true) {
            query.push(("Filters", "IsUnplayed".to_owned()));
        }
        if filters.get("favorites").and_then(Value::as_bool) == Some(true) {
            query.push(("IsFavorite", "true".to_owned()));
        }
        let found = self.items(query).await?;
        let shown = u64::from((page - 1) * PAGE_SIZE) + found.items.len() as u64;
        let more = found
            .total_record_count
            .is_some_and(|total| (total.max(0) as u64) > shown);
        let items: Vec<Value> = found
            .items
            .iter()
            .map(|item| json!({ "id": item.id, "title": item.name, "coverUrl": self.cover(item, 450) }))
            .collect();
        Ok(json!({ "items": items, "hasNextPage": more }))
    }

    async fn filters(&self) -> Result<Value, Failure> {
        #[derive(Deserialize, Default)]
        #[serde(rename_all = "PascalCase", default)]
        struct Views {
            items: Vec<Item>,
        }
        let views: Views = self
            .http
            .json(self.get("UserViews", &[("userId", self.user_id.clone())]))
            .await
            .or_else(|_| Ok::<_, Failure>(Views::default()))?;
        let mut libraries = vec![json!({ "value": "all", "label": "All libraries" })];
        libraries.extend(
            views
                .items
                .iter()
                .filter(|view| {
                    matches!(
                        view.collection_type.as_deref(),
                        None | Some("movies" | "tvshows" | "homevideos" | "mixed" | "boxsets")
                    )
                })
                .map(|view| json!({ "value": view.id, "label": view.name })),
        );
        let genres = self
            .http
            .json::<ItemsPage>(self.get(
                "Genres",
                &[
                    ("userId", self.user_id.clone()),
                    ("IncludeItemTypes", "Movie,Series".to_owned()),
                    ("SortBy", "SortName".to_owned()),
                    ("Limit", "200".to_owned()),
                ],
            ))
            .await
            .unwrap_or_default();
        let mut genre_options = vec![json!({ "value": "any", "label": "Any genre" })];
        genre_options.extend(
            genres
                .items
                .iter()
                .map(|genre| json!({ "value": genre.name, "label": genre.name })),
        );
        Ok(json!([
            { "kind": "select", "key": "library", "label": "Library", "options": libraries, "default": "all" },
            { "kind": "select", "key": "type", "label": "Type", "default": "all", "options": [
                { "value": "all", "label": "Films and series" },
                { "value": "movie", "label": "Films" },
                { "value": "series", "label": "Series" },
            ] },
            { "kind": "select", "key": "sort", "label": "Sort", "default": "name", "options": [
                { "value": "name", "label": "Name" },
                { "value": "added", "label": "Date added" },
                { "value": "released", "label": "Release date" },
                { "value": "rating", "label": "Rating" },
            ] },
            { "kind": "select", "key": "genre", "label": "Genre", "options": genre_options, "default": "any" },
            { "kind": "toggle", "key": "unplayed", "label": "Not watched", "default": false },
            { "kind": "toggle", "key": "favorites", "label": "Favorites", "default": false },
        ]))
    }

    async fn details(&self, id: &str) -> Result<Value, Failure> {
        let item = self
            .item(
                id,
                "Overview,Genres,People,ProductionYear,Status,PremiereDate",
            )
            .await?;
        let people = |kind: &str| -> Vec<String> {
            item.people
                .iter()
                .filter(|person| person.kind == kind)
                .map(|person| person.name.clone())
                .take(20)
                .collect()
        };
        let kind = if item.kind == "Series" {
            "series"
        } else {
            "movie"
        };
        let status = match item.status.as_deref() {
            Some("Continuing") => "ongoing",
            Some("Ended") => "completed",
            _ => "unknown",
        };
        let mut page = under(&self.base, "web/");
        page.set_fragment(Some(&format!("/details?id={}", item.id)));
        Ok(json!({
            "id": item.id,
            "title": item.name,
            "coverUrl": self.cover(&item, 900),
            "description": item.overview,
            "genres": item.genres,
            "type": kind,
            "year": item.production_year,
            "cast": people("Actor"),
            "directors": people("Director"),
            "status": status,
            "url": page.to_string(),
        }))
    }

    async fn episodes(&self, id: &str) -> Result<Value, Failure> {
        let item = self.item(id, "PremiereDate").await?;
        if item.kind != "Series" && item.kind != "Season" {
            // A film (or any single video) is its own one episode.
            return Ok(json!({ "episodes": [{
                "id": item.id,
                "name": item.name,
                "number": 1,
                "dateUpload": item.premiere_date.as_deref().and_then(date_ms),
            }] }));
        }
        let series = if item.kind == "Season" {
            item.series_id.clone().unwrap_or_else(|| item.id.clone())
        } else {
            item.id.clone()
        };
        let mut query = vec![
            ("userId", self.user_id.clone()),
            ("Fields", "PremiereDate".to_owned()),
            ("EnableImages", "false".to_owned()),
        ];
        if item.kind == "Season" {
            query.push(("seasonId", item.id.clone()));
        }
        let found: ItemsPage = self
            .http
            .json(self.get(&format!("Shows/{}/Episodes", path_part(&series)), &query))
            .await?;
        let episodes: Vec<Value> = found
            .items
            .iter()
            .map(|episode| {
                json!({
                    "id": episode.id,
                    "name": episode.name,
                    "number": episode.index_number,
                    "season": episode.parent_index_number,
                    "dateUpload": episode.premiere_date.as_deref().and_then(date_ms),
                })
            })
            .collect();
        Ok(json!({ "episodes": episodes }))
    }

    /// An episode's versions, each as the file itself and as a stream the server converts
    /// (H.264 and AAC in HLS) for players that can't decode the original.
    async fn servers(&self, episode: &str) -> Result<Value, Failure> {
        let item = self.item(episode, "MediaSources,Chapters").await?;
        let timestamps = self.timestamps(&item).await;
        let several = item.media_sources.len() > 1;
        let mut servers = Vec::new();
        for source in &item.media_sources {
            let version = source
                .name
                .clone()
                .filter(|name| several && !name.trim().is_empty());
            let label = |what: &str| match &version {
                Some(version) => format!("{version} · {what}"),
                None => what.to_owned(),
            };
            servers.push(json!({
                "id": format!("{}|{}|direct", item.id, source.id),
                "name": label("Original"),
                "videos": [self.video(&item, source, false, &timestamps)],
            }));
            servers.push(json!({
                "id": format!("{}|{}|hls", item.id, source.id),
                "name": label("Converted"),
                "videos": [self.video(&item, source, true, &timestamps)],
            }));
        }
        Ok(json!({ "servers": servers }))
    }

    async fn videos(&self, server: &str) -> Result<Value, Failure> {
        let mut parts = server.splitn(3, '|');
        let (Some(id), Some(source_id), Some(mode)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(Failure::said(format!("unknown server {server}")));
        };
        let item = self.item(id, "MediaSources,Chapters").await?;
        let source = item
            .media_sources
            .iter()
            .find(|source| source.id == source_id)
            .ok_or_else(|| Failure::said("this version of the video is gone from the server"))?;
        let timestamps = self.timestamps(&item).await;
        Ok(json!({ "videos": [self.video(&item, source, mode == "hls", &timestamps)] }))
    }

    fn video(
        &self,
        item: &Item,
        source: &MediaSourceInfo,
        converted: bool,
        timestamps: &[Value],
    ) -> Value {
        let height = source
            .media_streams
            .iter()
            .find(|stream| stream.kind == "Video")
            .and_then(|stream| stream.height);
        let (url, format) = if converted {
            let mut url = under(
                &self.base,
                &format!("Videos/{}/master.m3u8", path_part(&item.id)),
            );
            url.query_pairs_mut()
                .append_pair("MediaSourceId", &source.id)
                .append_pair("DeviceId", &self.device_id)
                .append_pair("PlaySessionId", &uuid::Uuid::new_v4().simple().to_string())
                .append_pair("VideoCodec", "h264")
                .append_pair("AudioCodec", "aac")
                .append_pair("VideoBitrate", "8000000")
                .append_pair("AudioBitrate", "192000")
                .append_pair("MaxAudioChannels", "2")
                .append_pair("TranscodingMaxAudioChannels", "2")
                .append_pair("SegmentContainer", "ts")
                .append_pair("BreakOnNonKeyFrames", "true");
            (url, "hls")
        } else {
            let mut url = under(
                &self.base,
                &format!("Videos/{}/stream", path_part(&item.id)),
            );
            url.query_pairs_mut()
                .append_pair("static", "true")
                .append_pair("mediaSourceId", &source.id);
            (url, "progressive")
        };
        // The original file carries its own subtitles; a converted stream only those sent apart.
        let subtitles: Vec<Value> = source
            .media_streams
            .iter()
            .filter(|stream| stream.kind == "Subtitle" && stream.is_text_subtitle_stream)
            .filter(|stream| converted || stream.is_external)
            .map(|stream| {
                let url = under(
                    &self.base,
                    &format!(
                        "Videos/{}/{}/Subtitles/{}/0/Stream.vtt",
                        path_part(&item.id),
                        path_part(&source.id),
                        stream.index
                    ),
                );
                json!({
                    "url": url.to_string(),
                    "lang": stream.language.clone().unwrap_or_default(),
                    "label": stream.display_title.clone().or_else(|| stream.codec.clone()).unwrap_or_default(),
                })
            })
            .collect();
        let headers: BTreeMap<String, String> = self.headers().into_iter().collect();
        let label = match (converted, height) {
            (true, _) => "Converted (H.264)".to_owned(),
            (false, Some(height)) => format!("{height}p"),
            (false, None) => "Original".to_owned(),
        };
        json!({
            "url": url.to_string(),
            "label": label,
            "quality": height,
            "headers": headers,
            "format": format,
            "subtitles": subtitles,
            "timestamps": timestamps,
        })
    }

    /// Chapters, and the intro, credits and recap segments the server found (10.10 and newer).
    async fn timestamps(&self, item: &Item) -> Vec<Value> {
        let mut marks: Vec<Value> = item
            .chapters
            .iter()
            .map(|chapter| {
                json!({
                    "start": chapter.start_position_ticks as f64 / 10_000_000.0,
                    "name": chapter.name.clone().unwrap_or_default(),
                    "type": "other",
                })
            })
            .collect();
        let segments: Segments = self
            .http
            .json(self.get(&format!("MediaSegments/{}", path_part(&item.id)), &[]))
            .await
            .unwrap_or_default();
        let kinds: Vec<Value> = segments
            .items
            .iter()
            .filter_map(|segment| {
                let kind = match segment.kind.as_str() {
                    "Intro" => "opening",
                    "Outro" => "ending",
                    "Recap" => "recap",
                    _ => return None,
                };
                Some(json!({
                    "start": segment.start_ticks as f64 / 10_000_000.0,
                    "end": segment.end_ticks as f64 / 10_000_000.0,
                    "type": kind,
                }))
            })
            .collect();
        // Segments say more than chapter names; with them the plain chapters stay out.
        if !kinds.is_empty() {
            marks = kinds;
        }
        marks
    }

    // ---- music ----

    fn track(&self, item: &Item) -> Value {
        let artist = if item.artists.is_empty() {
            item.album_artist.clone()
        } else {
            Some(item.artists.join(", "))
        };
        json!({
            "id": item.id,
            "title": item.name,
            "artist": artist,
            "album": item.album,
            "durationMs": item.run_time_ticks.map(|ticks| ticks / TICKS_PER_MS),
            "artworkUrl": self.cover(item, 600),
        })
    }

    fn album(&self, item: &Item) -> Value {
        json!({
            "id": item.id,
            "title": item.name,
            "artist": item.album_artist.clone().or_else(|| item.artists.first().cloned()),
            "year": item.production_year,
            "artworkUrl": self.cover(item, 600),
            "trackCount": item.child_count,
        })
    }

    async fn songs(&self, term: &str, limit: u32) -> Result<Vec<Item>, Failure> {
        let found = self
            .items(vec![
                ("Recursive", "true".to_owned()),
                ("IncludeItemTypes", "Audio".to_owned()),
                ("searchTerm", term.to_owned()),
                ("Limit", limit.to_string()),
                ("Fields", "MediaSources".to_owned()),
            ])
            .await?;
        Ok(found.items)
    }

    pub async fn music_call(&self, method: &str, args: &Value) -> Result<Value, Failure> {
        let query = text_arg(args, "query").unwrap_or_default();
        let limit = number_arg(args, "limit", 20, 1..=100);
        match method {
            "search" => {
                let songs = self.songs(&query, limit).await?;
                let tracks: Vec<Value> = songs.iter().map(|item| self.track(item)).collect();
                Ok(json!({ "tracks": tracks }))
            }
            "searchArtists" => {
                let found: ItemsPage = self
                    .http
                    .json(self.get(
                        "Artists",
                        &[
                            ("userId", self.user_id.clone()),
                            ("searchTerm", query),
                            ("Limit", limit.to_string()),
                        ],
                    ))
                    .await?;
                let artists: Vec<Value> = found
                    .items
                    .iter()
                    .map(|artist| json!({ "id": artist.id, "name": artist.name, "imageUrl": self.cover(artist, 600) }))
                    .collect();
                Ok(json!({ "artists": artists }))
            }
            "searchAlbums" => {
                let found = self
                    .items(vec![
                        ("Recursive", "true".to_owned()),
                        ("IncludeItemTypes", "MusicAlbum".to_owned()),
                        ("searchTerm", query),
                        ("Limit", limit.to_string()),
                        ("Fields", "ProductionYear,ChildCount".to_owned()),
                    ])
                    .await?;
                let albums: Vec<Value> = found.items.iter().map(|item| self.album(item)).collect();
                Ok(json!({ "albums": albums }))
            }
            "album" => {
                let id = id_arg(args, "id")?;
                let album = self.item(&id, "ProductionYear,ChildCount").await?;
                let songs = self
                    .items(vec![
                        ("ParentId", id),
                        ("Recursive", "true".to_owned()),
                        ("IncludeItemTypes", "Audio".to_owned()),
                        (
                            "SortBy",
                            "ParentIndexNumber,IndexNumber,SortName".to_owned(),
                        ),
                    ])
                    .await?;
                let tracks: Vec<Value> = songs.items.iter().map(|item| self.track(item)).collect();
                Ok(json!({ "album": self.album(&album), "tracks": tracks }))
            }
            "artistReleases" => {
                let found: ItemsPage = self
                    .http
                    .json(self.get(
                        "Artists",
                        &[
                            ("userId", self.user_id.clone()),
                            ("searchTerm", query.clone()),
                            ("Limit", "10".to_owned()),
                        ],
                    ))
                    .await?;
                let Some(artist) = found
                    .items
                    .iter()
                    .find(|artist| artist.name.eq_ignore_ascii_case(query.trim()))
                else {
                    return Ok(json!({ "releases": [] }));
                };
                let releases = self
                    .items(vec![
                        ("Recursive", "true".to_owned()),
                        ("IncludeItemTypes", "MusicAlbum".to_owned()),
                        ("AlbumArtistIds", artist.id.clone()),
                        ("SortBy", "ProductionYear,PremiereDate,SortName".to_owned()),
                        ("SortOrder", "Descending".to_owned()),
                        ("Limit", limit.to_string()),
                        ("Fields", "ProductionYear,ChildCount".to_owned()),
                    ])
                    .await?;
                let releases: Vec<Value> =
                    releases.items.iter().map(|item| self.album(item)).collect();
                Ok(json!({ "releases": releases }))
            }
            "searchPlaylists" => {
                if args.get("featured").and_then(Value::as_bool) == Some(true) {
                    return Ok(json!({ "playlists": [] }));
                }
                let found = self
                    .items(vec![
                        ("Recursive", "true".to_owned()),
                        ("IncludeItemTypes", "Playlist".to_owned()),
                        ("MediaTypes", "Audio".to_owned()),
                        ("searchTerm", query),
                        ("Limit", limit.to_string()),
                        ("Fields", "ChildCount".to_owned()),
                    ])
                    .await?;
                let playlists: Vec<Value> = found
                    .items
                    .iter()
                    .map(|item| {
                        json!({
                            "id": item.id,
                            "title": item.name,
                            "artworkUrl": self.cover(item, 600),
                            "trackCount": item.child_count,
                        })
                    })
                    .collect();
                Ok(json!({ "playlists": playlists }))
            }
            "playlist" => {
                let id = id_arg(args, "id")?;
                let playlist = self.item(&id, "ChildCount").await?;
                let songs: ItemsPage = self
                    .http
                    .json(self.get(
                        &format!("Playlists/{}/Items", path_part(&id)),
                        &[("userId", self.user_id.clone())],
                    ))
                    .await?;
                let tracks: Vec<Value> = songs
                    .items
                    .iter()
                    .filter(|item| item.kind == "Audio")
                    .map(|item| self.track(item))
                    .collect();
                Ok(json!({
                    "playlist": {
                        "id": playlist.id,
                        "title": playlist.name,
                        "artworkUrl": self.cover(&playlist, 600),
                        "trackCount": tracks.len(),
                    },
                    "tracks": tracks,
                }))
            }
            other => Err(no_method("music", other)),
        }
    }

    pub async fn stream_call(&self, method: &str, args: &Value) -> Result<Value, Failure> {
        match method {
            "candidates" => {
                let Some(title) = text_arg(args, "title") else {
                    return Ok(json!({ "candidates": [] }));
                };
                let limit = number_arg(args, "limit", 10, 1..=50);
                let songs = self.songs(&title, 25).await?;
                let candidates: Vec<Value> = songs
                    .iter()
                    .take(limit as usize)
                    .map(|item| {
                        let source = item.media_sources.first();
                        let kbps = source
                            .and_then(|source| source.bitrate)
                            .filter(|bitrate| *bitrate > 0)
                            .map(|bitrate| bitrate / 1000);
                        let container = source
                            .and_then(|source| source.container.clone())
                            .or_else(|| item.container.clone())
                            .unwrap_or_default()
                            .to_ascii_uppercase();
                        let quality = match kbps {
                            Some(kbps) => format!("{container} {kbps} kbps"),
                            None => container,
                        };
                        let track = self.track(item);
                        json!({
                            "id": item.id,
                            "title": item.name,
                            "artist": track["artist"],
                            "durationMs": track["durationMs"],
                            "quality": quality.trim(),
                            "bitrateKbps": kbps,
                        })
                    })
                    .collect();
                Ok(json!({ "candidates": candidates }))
            }
            "resolve" => {
                let id = id_arg(args, "id")?;
                let target = args.get("targetKbps").and_then(Value::as_f64);
                let decodable = args.get("decodable").and_then(Value::as_bool) == Some(true);
                Ok(json!({
                    "source": {
                        "kind": "url",
                        "url": self.audio_url(&id, target, decodable),
                        "headers": self.headers().into_iter().collect::<BTreeMap<_, _>>(),
                    },
                    "expiresAt": super::stream_expiry(),
                }))
            }
            other => Err(no_method("streams", other)),
        }
    }

    /// A song's audio as the server's universal endpoint gives it: the file itself when the
    /// player takes its format and its bitrate is within `target`, else converted to it.
    fn audio_url(&self, id: &str, target: Option<f64>, decodable: bool) -> String {
        let containers = if decodable {
            "mp3,aac,m4a|aac,flac,wav,ogg|vorbis"
        } else {
            "opus,webm|opus,mp3,aac,m4a|aac,m4b|aac,flac,webma,webm|webma,wav,ogg"
        };
        let bitrate = target
            .filter(|kbps| kbps.is_finite() && *kbps > 0.0)
            .map_or(1_000_000_000, |kbps| (kbps * 1000.0) as i64);
        let mut url = under(&self.base, &format!("Audio/{}/universal", path_part(id)));
        url.query_pairs_mut()
            .append_pair("UserId", &self.user_id)
            .append_pair("DeviceId", &self.device_id)
            .append_pair("MaxStreamingBitrate", &bitrate.to_string())
            .append_pair("Container", containers)
            .append_pair("TranscodingContainer", "mp3")
            .append_pair("TranscodingProtocol", "http")
            .append_pair("AudioCodec", "mp3");
        url.to_string()
    }

    pub async fn lyrics_call(&self, method: &str, args: &Value) -> Result<Value, Failure> {
        if method != "lyrics" {
            return Err(no_method("lyrics", method));
        }
        let Some(title) = text_arg(args, "title") else {
            return Ok(Value::Null);
        };
        let artist = text_arg(args, "artist").unwrap_or_default().to_lowercase();
        let songs = self.songs(&title, 10).await?;
        let Some(song) = songs.iter().find(|song| {
            song.has_lyrics != Some(false)
                && song.name.eq_ignore_ascii_case(&title)
                && (artist.is_empty()
                    || song
                        .artists
                        .iter()
                        .chain(song.album_artist.iter())
                        .any(|name| {
                            artist.contains(&name.to_lowercase())
                                || name.to_lowercase().contains(&artist)
                        }))
        }) else {
            return Ok(Value::Null);
        };
        let answer: LyricsAnswer = match self
            .http
            .json(self.get(&format!("Audio/{}/Lyrics", path_part(&song.id)), &[]))
            .await
        {
            Ok(answer) => answer,
            Err(failure)
                if failure
                    .trouble
                    .as_ref()
                    .is_some_and(|trouble| trouble.kind == raygun_net::IssueKind::NotFound) =>
            {
                return Ok(Value::Null);
            }
            Err(failure) => return Err(failure),
        };
        if answer.lyrics.is_empty() {
            return Ok(Value::Null);
        }
        if answer.lyrics.iter().all(|line| line.start.is_some()) {
            let synced: Vec<Value> = answer
                .lyrics
                .iter()
                .map(|line| json!({ "timeMs": line.start.unwrap_or_default() / TICKS_PER_MS, "text": line.text }))
                .collect();
            Ok(json!({ "synced": synced }))
        } else {
            let plain: Vec<&str> = answer
                .lyrics
                .iter()
                .map(|line| line.text.as_str())
                .collect();
            Ok(json!({ "plain": plain.join("\n") }))
        }
    }
}

fn no_method(provider: &str, method: &str) -> Failure {
    Failure::said(format!("{provider} has no method {method}"))
}
