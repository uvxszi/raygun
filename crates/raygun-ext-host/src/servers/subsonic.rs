//! Subsonic servers and those speaking its API (subsonic.org/pages/api.jsp, with OpenSubsonic's
//! additions, opensubsonic.netlify.app): Navidrome, Gonic, Airsonic, Ampache... Music as
//! `music.metadata`, `music.stream` and `lyrics`. Requests sign in with the salted token of the
//! password (`t`, `s`), the plain password where a server only takes that (`p=enc:`, for LDAP
//! users), or an OpenSubsonic API key (`apiKey`).

use md5::{Digest, Md5};
use raygun_net::IssueKind;
use reqwest::Url;
use serde_json::{Value, json};

use super::http::{Http, id_arg, number_arg, text_arg, under};
use crate::failure::{Failure, Trouble};

/// The API version Raygun speaks (1.16.1: `search3`, `getAlbum`, `stream`'s `maxBitRate`).
const API_VERSION: &str = "1.16.1";
const CLIENT: &str = "Raygun";

/// How a Subsonic server takes the sign-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Sign {
    /// The salted token of the password.
    Token { username: String, password: String },
    /// The password itself, hex-encoded (servers that can't check tokens, such as LDAP ones).
    Plain { username: String, password: String },
    /// An OpenSubsonic API key.
    Key(String),
}

impl Sign {
    /// The sign-in's query parameters, with this session's `salt`.
    fn query(&self, salt: &str) -> Vec<(String, String)> {
        let mut query = match self {
            Self::Token { username, password } => {
                let token: String = Md5::digest(format!("{password}{salt}").as_bytes())
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect();
                vec![
                    ("u".to_owned(), username.clone()),
                    ("t".to_owned(), token),
                    ("s".to_owned(), salt.to_owned()),
                ]
            }
            Self::Plain { username, password } => {
                let hex: String = password.bytes().map(|byte| format!("{byte:02x}")).collect();
                vec![
                    ("u".to_owned(), username.clone()),
                    ("p".to_owned(), format!("enc:{hex}")),
                ]
            }
            Self::Key(key) => vec![("apiKey".to_owned(), key.clone())],
        };
        query.push(("v".to_owned(), API_VERSION.to_owned()));
        query.push(("c".to_owned(), CLIENT.to_owned()));
        query
    }
}

/// A Subsonic server signed in to.
pub(crate) struct Subsonic {
    http: Http,
    base: Url,
    /// The sign-in's query parameters (fixed for the session).
    auth: Vec<(String, String)>,
}

/// A failed Subsonic answer (`status: failed`): its code says what kind of trouble it is.
fn api_failure(host: &str, error: &Value) -> Failure {
    let code = error
        .get("code")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("no reason given")
        .to_owned();
    let kind = match code {
        40 | 41 | 42 | 43 | 44 | 50 => Some(IssueKind::Denied),
        70 => Some(IssueKind::NotFound),
        _ => None,
    };
    Failure {
        message: format!("{host} said: {message} (Subsonic error {code})"),
        trouble: kind.map(|kind| Trouble {
            kind,
            detail: format!("{host} said: {message} (Subsonic error {code})"),
        }),
        answered: true,
        checked: None,
    }
}

/// The Subsonic error code of a failure, when the server gave one.
pub(crate) fn error_code(failure: &Failure) -> Option<i64> {
    let start = failure.message.rfind("(Subsonic error ")? + "(Subsonic error ".len();
    failure.message[start..].trim_end_matches(')').parse().ok()
}

impl Subsonic {
    pub fn new(http: Http, base: Url, sign: &Sign) -> Self {
        let salt = uuid::Uuid::new_v4().simple().to_string()[..12].to_owned();
        Self {
            http,
            base,
            auth: sign.query(&salt),
        }
    }

    /// The sign-in's query parameters, which `raygun_net::server_auth` adds to cover addresses.
    pub fn auth_query(&self) -> Vec<(String, String)> {
        self.auth.clone()
    }

    fn url(&self, method: &str, query: &[(&str, String)]) -> Url {
        let mut url = under(&self.base, &format!("rest/{method}.view"));
        {
            let mut pairs = url.query_pairs_mut();
            for (name, value) in query {
                pairs.append_pair(name, value);
            }
        }
        url
    }

    /// Calls `method` and returns the answer inside `subsonic-response`.
    pub async fn call(&self, method: &str, query: &[(&str, String)]) -> Result<Value, Failure> {
        self.send(method, query, false).await
    }

    /// Calls `method`, its parameters in a form when `post` (OpenSubsonic's `formPost`: long
    /// lists of songs don't fit in an address).
    async fn send(
        &self,
        method: &str,
        query: &[(&str, String)],
        post: bool,
    ) -> Result<Value, Failure> {
        let request = if post {
            let mut form = url::form_urlencoded::Serializer::new(String::new());
            for (name, value) in query {
                form.append_pair(name, value);
            }
            for (name, value) in &self.auth {
                form.append_pair(name, value);
            }
            form.append_pair("f", "json");
            self.http
                .client()
                .post(under(&self.base, &format!("rest/{method}.view")))
                .header(
                    reqwest::header::CONTENT_TYPE,
                    "application/x-www-form-urlencoded",
                )
                .body(form.finish())
        } else {
            let mut url = self.url(method, query);
            {
                let mut pairs = url.query_pairs_mut();
                for (name, value) in &self.auth {
                    pairs.append_pair(name, value);
                }
                pairs.append_pair("f", "json");
            }
            self.http.client().get(url)
        };
        let answer: Value = self.http.json(request).await?;
        let host = self.base.host_str().unwrap_or_default();
        let response = answer
            .get("subsonic-response")
            .ok_or_else(|| super::http::unreadable(host, &"no subsonic-response"))?;
        if response.get("status").and_then(Value::as_str) == Some("ok") {
            Ok(response.clone())
        } else {
            Err(api_failure(
                host,
                response.get("error").unwrap_or(&Value::Null),
            ))
        }
    }

    fn cover(&self, art: Option<&Value>) -> Option<String> {
        let art = art.and_then(value_text)?;
        let mut url = under(&self.base, "rest/getCoverArt.view");
        url.query_pairs_mut()
            .append_pair("id", &art)
            .append_pair("size", "600");
        Some(url.to_string())
    }

    fn track(&self, song: &Value) -> Value {
        json!({
            "id": song.get("id").and_then(value_text),
            "title": song.get("title").and_then(Value::as_str).unwrap_or_default(),
            "artist": artist_of(song),
            "album": song.get("album").and_then(Value::as_str),
            "durationMs": song.get("duration").and_then(Value::as_f64).map(|seconds| (seconds * 1000.0) as i64),
            "artworkUrl": self.cover(song.get("coverArt").or_else(|| song.get("albumId"))),
            "explicit": song.get("explicitStatus").and_then(Value::as_str) == Some("explicit"),
        })
    }

    fn album(&self, album: &Value) -> Value {
        let kind = album
            .get("releaseTypes")
            .and_then(Value::as_array)
            .and_then(|types| types.first())
            .and_then(Value::as_str)
            .map(str::to_ascii_lowercase)
            .filter(|kind| {
                ["album", "single", "ep", "compilation", "live"].contains(&kind.as_str())
            });
        json!({
            "id": album.get("id").and_then(value_text),
            "title": album.get("name").or_else(|| album.get("title")).and_then(Value::as_str).unwrap_or_default(),
            "artist": artist_of(album),
            "year": album.get("year").and_then(Value::as_i64),
            "artworkUrl": self.cover(album.get("coverArt").or_else(|| album.get("id"))),
            "trackCount": album.get("songCount").and_then(Value::as_i64),
            "kind": kind,
        })
    }

    async fn search3(
        &self,
        query: &str,
        songs: u32,
        albums: u32,
        artists: u32,
    ) -> Result<Value, Failure> {
        let answer = self
            .call(
                "search3",
                &[
                    ("query", query.to_owned()),
                    ("songCount", songs.to_string()),
                    ("albumCount", albums.to_string()),
                    ("artistCount", artists.to_string()),
                ],
            )
            .await?;
        Ok(answer.get("searchResult3").cloned().unwrap_or(Value::Null))
    }

    pub async fn music_call(&self, method: &str, args: &Value) -> Result<Value, Failure> {
        let query = text_arg(args, "query").unwrap_or_default();
        let limit = number_arg(args, "limit", 20, 1..=100);
        match method {
            "search" => {
                let found = self.search3(&query, limit, 0, 0).await?;
                Ok(
                    json!({ "tracks": list(&found, "song").iter().map(|song| self.track(song)).collect::<Vec<_>>() }),
                )
            }
            "searchArtists" => {
                let found = self.search3(&query, 0, 0, limit).await?;
                let artists: Vec<Value> = list(&found, "artist")
                    .iter()
                    .map(|artist| {
                        json!({
                            "id": artist.get("id").and_then(value_text),
                            "name": artist.get("name").and_then(Value::as_str).unwrap_or_default(),
                            "imageUrl": artist
                                .get("artistImageUrl")
                                .and_then(Value::as_str)
                                .filter(|url| url.starts_with("http"))
                                .map(str::to_owned)
                                .or_else(|| self.cover(artist.get("coverArt"))),
                        })
                    })
                    .collect();
                Ok(json!({ "artists": artists }))
            }
            "searchAlbums" => {
                let found = self.search3(&query, 0, limit, 0).await?;
                Ok(
                    json!({ "albums": list(&found, "album").iter().map(|album| self.album(album)).collect::<Vec<_>>() }),
                )
            }
            "album" => {
                let id = id_arg(args, "id")?;
                let answer = self.call("getAlbum", &[("id", id)]).await?;
                let album = answer.get("album").cloned().unwrap_or(Value::Null);
                let tracks: Vec<Value> = list(&album, "song")
                    .iter()
                    .map(|song| self.track(song))
                    .collect();
                Ok(json!({ "album": self.album(&album), "tracks": tracks }))
            }
            "artistReleases" => {
                let found = self.search3(&query, 0, 0, 10).await?;
                let Some(artist) = list(&found, "artist").into_iter().find(|artist| {
                    artist
                        .get("name")
                        .and_then(Value::as_str)
                        .is_some_and(|name| name.eq_ignore_ascii_case(query.trim()))
                }) else {
                    return Ok(json!({ "releases": [] }));
                };
                let id = artist.get("id").and_then(value_text).unwrap_or_default();
                let answer = self.call("getArtist", &[("id", id)]).await?;
                let mut albums = list(answer.get("artist").unwrap_or(&Value::Null), "album");
                albums.sort_by_key(|album| {
                    std::cmp::Reverse(album.get("year").and_then(Value::as_i64))
                });
                let releases: Vec<Value> = albums
                    .iter()
                    .take(limit as usize)
                    .map(|album| self.album(album))
                    .collect();
                Ok(json!({ "releases": releases }))
            }
            "searchPlaylists" => {
                if args.get("featured").and_then(Value::as_bool) == Some(true) {
                    return Ok(json!({ "playlists": [] }));
                }
                // Subsonic can't search playlists: the user's are filtered by name.
                let answer = self.call("getPlaylists", &[]).await?;
                let wanted = query.to_lowercase();
                let playlists: Vec<Value> =
                    list(answer.get("playlists").unwrap_or(&Value::Null), "playlist")
                        .iter()
                        .filter(|playlist| {
                            playlist
                                .get("name")
                                .and_then(Value::as_str)
                                .is_some_and(|name| {
                                    wanted.is_empty() || name.to_lowercase().contains(&wanted)
                                })
                        })
                        .take(limit as usize)
                        .map(|playlist| self.playlist(playlist))
                        .collect();
                Ok(json!({ "playlists": playlists }))
            }
            "playlist" => {
                let id = id_arg(args, "id")?;
                let answer = self.call("getPlaylist", &[("id", id)]).await?;
                let playlist = answer.get("playlist").cloned().unwrap_or(Value::Null);
                let tracks: Vec<Value> = list(&playlist, "entry")
                    .iter()
                    .map(|song| self.track(song))
                    .collect();
                Ok(json!({ "playlist": self.playlist(&playlist), "tracks": tracks }))
            }
            other => Err(Failure::said(format!("music has no method {other}"))),
        }
    }

    fn playlist(&self, playlist: &Value) -> Value {
        json!({
            "id": playlist.get("id").and_then(value_text),
            "title": playlist.get("name").and_then(Value::as_str).unwrap_or_default(),
            "author": playlist.get("owner").and_then(Value::as_str),
            "artworkUrl": self.cover(playlist.get("coverArt")),
            "trackCount": playlist.get("songCount").and_then(Value::as_i64),
        })
    }

    /// Songs that may be `title` by `artist`: searched with both, then the title alone.
    async fn find_songs(&self, title: &str, artist: Option<&str>) -> Result<Vec<Value>, Failure> {
        if let Some(artist) = artist {
            let found = self.search3(&format!("{title} {artist}"), 20, 0, 0).await?;
            let songs = list(&found, "song");
            if !songs.is_empty() {
                return Ok(songs);
            }
        }
        let found = self.search3(title, 20, 0, 0).await?;
        Ok(list(&found, "song"))
    }

    pub async fn stream_call(&self, method: &str, args: &Value) -> Result<Value, Failure> {
        match method {
            "candidates" => {
                let Some(title) = text_arg(args, "title") else {
                    return Ok(json!({ "candidates": [] }));
                };
                let limit = number_arg(args, "limit", 10, 1..=50);
                let songs = self
                    .find_songs(&title, text_arg(args, "artist").as_deref())
                    .await?;
                let candidates: Vec<Value> = songs
                    .iter()
                    .take(limit as usize)
                    .map(|song| {
                        let kbps = song
                            .get("bitRate")
                            .and_then(Value::as_i64)
                            .filter(|kbps| *kbps > 0);
                        let suffix = song
                            .get("suffix")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_ascii_uppercase();
                        let quality = match kbps {
                            Some(kbps) => format!("{suffix} {kbps} kbps"),
                            None => suffix,
                        };
                        let track = self.track(song);
                        json!({
                            "id": track["id"],
                            "title": track["title"],
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
                let target = args
                    .get("targetKbps")
                    .and_then(Value::as_f64)
                    .filter(|kbps| kbps.is_finite() && *kbps > 0.0);
                let decodable = args.get("decodable").and_then(Value::as_bool) == Some(true);
                // The file's own format plays as it is unless Raygun must decode it itself.
                let suffix = if decodable {
                    self.call("getSong", &[("id", id.clone())])
                        .await
                        .ok()
                        .and_then(|answer| {
                            answer
                                .get("song")
                                .and_then(|song| song.get("suffix"))
                                .and_then(Value::as_str)
                                .map(str::to_ascii_lowercase)
                        })
                } else {
                    None
                };
                let readable = suffix.as_deref().is_none_or(|suffix| {
                    ["mp3", "flac", "m4a", "aac", "ogg", "oga", "wav", "mp4"].contains(&suffix)
                });
                let mut query = vec![
                    ("id", id),
                    (
                        "maxBitRate",
                        target.map_or(0, |kbps| kbps.round() as i64).to_string(),
                    ),
                ];
                if let Some(format) = stream_format(target.is_some(), decodable, readable) {
                    query.push(("format", format.to_owned()));
                }
                let mut url = self.url("stream", &query);
                {
                    let mut pairs = url.query_pairs_mut();
                    for (name, value) in &self.auth {
                        pairs.append_pair(name, value);
                    }
                }
                Ok(json!({
                    "source": { "kind": "url", "url": url.to_string(), "headers": {} },
                    "expiresAt": super::stream_expiry(),
                }))
            }
            other => Err(Failure::said(format!("streams has no method {other}"))),
        }
    }

    pub async fn lyrics_call(&self, method: &str, args: &Value) -> Result<Value, Failure> {
        if method != "lyrics" {
            return Err(Failure::said(format!("lyrics has no method {method}")));
        }
        let Some(title) = text_arg(args, "title") else {
            return Ok(Value::Null);
        };
        let artist = text_arg(args, "artist");
        let songs = self.find_songs(&title, artist.as_deref()).await?;
        let wanted_artist = artist.as_deref().unwrap_or_default().to_lowercase();
        let song = songs.iter().find(|song| {
            song.get("title")
                .and_then(Value::as_str)
                .is_some_and(|name| name.eq_ignore_ascii_case(&title))
                && (wanted_artist.is_empty()
                    || artist_of(song).is_some_and(|name| {
                        let name = name.to_lowercase();
                        name.contains(&wanted_artist) || wanted_artist.contains(&name)
                    }))
        });
        let Some(song) = song else {
            return Ok(Value::Null);
        };
        // OpenSubsonic's lyrics, timed when the server has them.
        let id = song.get("id").and_then(value_text).unwrap_or_default();
        if let Ok(answer) = self.call("getLyricsBySongId", &[("id", id)]).await {
            // songLyrics 2 adds translations and pronunciations beside the words as sung.
            let structured: Vec<Value> = answer
                .get("lyricsList")
                .and_then(|lyrics| lyrics.get("structuredLyrics"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|lyrics| {
                    lyrics
                        .get("kind")
                        .and_then(Value::as_str)
                        .is_none_or(|kind| kind.eq_ignore_ascii_case("main"))
                })
                .collect();
            let best = structured
                .iter()
                .find(|lyrics| lyrics.get("synced").and_then(Value::as_bool) == Some(true))
                .or_else(|| structured.first());
            if let Some(lyrics) = best {
                let offset = lyrics
                    .get("offset")
                    .and_then(Value::as_i64)
                    .unwrap_or_default();
                let lines = list(lyrics, "line");
                if lyrics.get("synced").and_then(Value::as_bool) == Some(true) {
                    let synced: Vec<Value> = lines
                        .iter()
                        .map(|line| json!({
                            "timeMs": (line.get("start").and_then(Value::as_i64).unwrap_or_default() + offset).max(0),
                            "text": line.get("value").and_then(Value::as_str).unwrap_or_default(),
                        }))
                        .collect();
                    if !synced.is_empty() {
                        return Ok(json!({ "synced": synced }));
                    }
                }
                let plain: Vec<&str> = lines
                    .iter()
                    .filter_map(|line| line.get("value").and_then(Value::as_str))
                    .collect();
                if !plain.is_empty() {
                    return Ok(json!({ "plain": plain.join("\n") }));
                }
            }
        }
        let answer = self
            .call(
                "getLyrics",
                &[
                    (
                        "artist",
                        song.get("artist")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                    ),
                    ("title", title),
                ],
            )
            .await?;
        Ok(answer
            .get("lyrics")
            .and_then(|lyrics| lyrics.get("value"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map_or(Value::Null, |text| json!({ "plain": text })))
    }
}

/// The server's album lists offered as charts (`getAlbumList2`'s types): id, name, group.
const ALBUM_CHARTS: [(&str, &str, &str); 5] = [
    ("newest", "Recently added", "new"),
    ("recent", "Recently played", "trending"),
    ("frequent", "Most played", "top"),
    ("starred", "Starred", "top"),
    ("highest", "Highest rated", "top"),
];
/// Genres offered as charts at most, the ones with the most songs.
const GENRE_CHARTS: usize = 24;
/// Albums whose songs make up an album chart, at most (each is a request).
const CHART_ALBUMS: usize = 8;

impl Subsonic {
    /// The library to browse as `music.charts`: the album lists Subsonic keeps (recently added,
    /// played most, starred...), random songs, each genre, each decade, and internet radio.
    pub async fn charts_call(&self, method: &str, args: &Value) -> Result<Value, Failure> {
        let limit = number_arg(args, "limit", 50, 1..=200);
        match method {
            "charts" => {
                let mut charts: Vec<Value> = ALBUM_CHARTS
                    .iter()
                    .map(|(id, name, group)| {
                        json!({ "id": format!("albums:{id}"), "name": name, "group": group })
                    })
                    .collect();
                charts.push(json!({ "id": "random", "name": "Random songs", "group": "top" }));
                let genres = self.call("getGenres", &[]).await?;
                let mut genres = list(genres.get("genres").unwrap_or(&Value::Null), "genre");
                genres.sort_by_key(|genre| {
                    std::cmp::Reverse(genre.get("songCount").and_then(Value::as_i64))
                });
                for genre in genres.iter().take(GENRE_CHARTS) {
                    let Some(name) = genre_name(genre) else {
                        continue;
                    };
                    charts.push(json!({
                        "id": format!("genre:{name}"),
                        "name": name,
                        "tag": name,
                        "group": "genre",
                    }));
                }
                // The decades the library has music from, newest first.
                let years = self
                    .call(
                        "getAlbumList2",
                        &[
                            ("type", "byYear".to_owned()),
                            ("fromYear", "3000".to_owned()),
                            ("toYear", "1".to_owned()),
                            ("size", "500".to_owned()),
                        ],
                    )
                    .await
                    .map(|answer| list(answer.get("albumList2").unwrap_or(&Value::Null), "album"))
                    .unwrap_or_default();
                let mut decades: Vec<i64> = years
                    .iter()
                    .filter_map(|album| album.get("year").and_then(Value::as_i64))
                    .filter(|year| *year > 0)
                    .map(|year| year - year % 10)
                    .collect();
                decades.sort_unstable_by(|a, b| b.cmp(a));
                decades.dedup();
                for decade in decades {
                    charts.push(json!({
                        "id": format!("decade:{decade}"),
                        "name": format!("{decade}s"),
                        "group": "decade",
                    }));
                }
                let stations = self.radio_stations().await.unwrap_or_default();
                if !stations.is_empty() {
                    charts.push(json!({ "id": "radio", "name": "Internet radio", "group": "top" }));
                }
                Ok(json!({ "charts": charts }))
            }
            "chart" => {
                let id = id_arg(args, "id")?;
                let tracks = if let Some(kind) = id.strip_prefix("albums:") {
                    let answer = self
                        .call(
                            "getAlbumList2",
                            &[
                                ("type", kind.to_owned()),
                                ("size", CHART_ALBUMS.to_string()),
                            ],
                        )
                        .await?;
                    self.album_songs(
                        &list(answer.get("albumList2").unwrap_or(&Value::Null), "album"),
                        limit,
                    )
                    .await?
                } else if let Some(decade) = id
                    .strip_prefix("decade:")
                    .and_then(|decade| decade.parse::<i64>().ok())
                {
                    let answer = self
                        .call(
                            "getAlbumList2",
                            &[
                                ("type", "byYear".to_owned()),
                                ("fromYear", decade.to_string()),
                                ("toYear", (decade + 9).to_string()),
                                ("size", CHART_ALBUMS.to_string()),
                            ],
                        )
                        .await?;
                    self.album_songs(
                        &list(answer.get("albumList2").unwrap_or(&Value::Null), "album"),
                        limit,
                    )
                    .await?
                } else if let Some(genre) = id.strip_prefix("genre:") {
                    self.genre_songs(genre, limit).await?
                } else if id == "random" {
                    let answer = self
                        .call("getRandomSongs", &[("size", limit.to_string())])
                        .await?;
                    list(answer.get("randomSongs").unwrap_or(&Value::Null), "song")
                        .iter()
                        .map(|song| self.track(song))
                        .collect()
                } else if id == "radio" {
                    self.radio_stations()
                        .await?
                        .iter()
                        .filter_map(|station| {
                            let url = station.get("streamUrl").and_then(Value::as_str)?;
                            url.starts_with("http").then(|| {
                                json!({
                                    "id": format!("radio:{}", station.get("id").and_then(value_text).unwrap_or_default()),
                                    "title": station.get("name").and_then(Value::as_str).unwrap_or(url),
                                    "artist": "Internet radio",
                                    "source": { "kind": "url", "url": url, "headers": {} },
                                })
                            })
                        })
                        .collect()
                } else {
                    return Err(Failure::said("This list is gone; open Browse again"));
                };
                Ok(json!({ "tracks": tracks }))
            }
            "tagSongs" => {
                let Some(tag) = text_arg(args, "tag") else {
                    return Ok(json!({ "tracks": [] }));
                };
                let mut names = vec![tag.to_lowercase()];
                names.extend(
                    args.get("aliases")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(str::to_lowercase),
                );
                let genres = self.call("getGenres", &[]).await?;
                let genre = list(genres.get("genres").unwrap_or(&Value::Null), "genre")
                    .iter()
                    .filter_map(genre_name)
                    .find(|name| names.contains(&name.to_lowercase()));
                let tracks = match genre {
                    Some(genre) => self.genre_songs(&genre, limit).await?,
                    None => Vec::new(),
                };
                Ok(json!({ "tracks": tracks }))
            }
            other => Err(Failure::said(format!("charts has no method {other}"))),
        }
    }

    /// The songs of `albums`, in order, `limit` at most.
    async fn album_songs(&self, albums: &[Value], limit: u32) -> Result<Vec<Value>, Failure> {
        let mut tracks = Vec::new();
        for album in albums.iter().take(CHART_ALBUMS) {
            if tracks.len() >= limit as usize {
                break;
            }
            let Some(id) = album.get("id").and_then(value_text) else {
                continue;
            };
            let answer = self.call("getAlbum", &[("id", id)]).await?;
            tracks.extend(
                list(answer.get("album").unwrap_or(&Value::Null), "song")
                    .iter()
                    .map(|song| self.track(song)),
            );
        }
        tracks.truncate(limit as usize);
        Ok(tracks)
    }

    async fn genre_songs(&self, genre: &str, limit: u32) -> Result<Vec<Value>, Failure> {
        let answer = self
            .call(
                "getSongsByGenre",
                &[("genre", genre.to_owned()), ("count", limit.to_string())],
            )
            .await?;
        Ok(
            list(answer.get("songsByGenre").unwrap_or(&Value::Null), "song")
                .iter()
                .map(|song| self.track(song))
                .collect(),
        )
    }

    async fn radio_stations(&self) -> Result<Vec<Value>, Failure> {
        let answer = self.call("getInternetRadioStations", &[]).await?;
        Ok(list(
            answer.get("internetRadioStations").unwrap_or(&Value::Null),
            "internetRadioStation",
        ))
    }
}

/// A genre's name: OpenSubsonic's `value`, or the text of the old XML-turned-JSON shape.
fn genre_name(genre: &Value) -> Option<String> {
    genre
        .get("value")
        .or_else(|| genre.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
}

/// The artist a song or album credits: OpenSubsonic's `displayArtist` (every artist, as the
/// server joins them), else Subsonic's `artist`.
fn artist_of(item: &Value) -> Option<&str> {
    ["displayArtist", "artist"]
        .iter()
        .filter_map(|key| item.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .find(|name| !name.is_empty())
}

/// The `format` a stream asks for: the file as it is when no bitrate is aimed for; with one,
/// the server's own choice (it converts only files above the bitrate), or MP3 when Raygun decodes
/// the stream itself (the server may choose Opus, which it can't); MP3 for files Raygun can't
/// read.
pub(crate) fn stream_format(aimed: bool, decodable: bool, readable: bool) -> Option<&'static str> {
    if !readable || (aimed && decodable) {
        Some("mp3")
    } else if aimed {
        None
    } else {
        Some("raw")
    }
}

/// Pings the server with a sign-in; the answer when it accepts it (`type` names the server:
/// `navidrome`, `gonic`...).
pub(crate) async fn ping(http: &Http, base: &Url, sign: &Sign) -> Result<Value, Failure> {
    Subsonic::new(http.clone(), base.clone(), sign)
        .call("ping", &[])
        .await
}

/// Whether a Subsonic answer comes from Navidrome.
pub(crate) fn is_navidrome(answer: &Value) -> bool {
    answer
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind.eq_ignore_ascii_case("navidrome"))
}

/// A song on the server, as [`Library`] gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Song {
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
    /// The cover's address, without the sign-in (requests add it).
    pub artwork_url: Option<String>,
}

/// A playlist on the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub owner: Option<String>,
    /// Whether this user may change it: their own, and not a smart playlist.
    pub editable: bool,
}

/// The play queue the server keeps for the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayQueue {
    pub songs: Vec<Song>,
    /// The song playing, by id.
    pub current: Option<String>,
    pub position_ms: u64,
    /// The client that saved it last.
    pub changed_by: Option<String>,
}

/// Songs sent in one request at most, without a form.
const IDS_PER_ADDRESS: usize = 40;

/// The user's library on a Subsonic server, for keeping it in step with Raygun's: stars,
/// playlists, scrobbles and the play queue.
pub struct Library {
    client: Subsonic,
    username: String,
    /// Whether the server takes forms (`formPost`), once asked.
    forms: std::sync::Mutex<Option<bool>>,
}

impl Library {
    /// The library of a connection, signed in with its kept `secret`.
    pub fn open(config: &super::ServerConfig, secret: &str) -> Result<Self, String> {
        if !config.kind.is_subsonic() {
            return Err(format!(
                "{} servers keep no such library",
                config.kind.label()
            ));
        }
        let base = config.base()?;
        let http = Http::new(&base)?;
        let sign = sign_for(config, secret.to_owned());
        Ok(Self {
            client: Subsonic::new(http, base, &sign),
            username: config.username.clone(),
            forms: std::sync::Mutex::new(None),
        })
    }

    fn song(&self, song: &Value) -> Option<Song> {
        let id = song.get("id").and_then(value_text)?;
        let title = song.get("title").and_then(Value::as_str)?.trim().to_owned();
        (!title.is_empty()).then(|| Song {
            id,
            title,
            artist: artist_of(song).map(str::to_owned),
            album: song.get("album").and_then(Value::as_str).map(str::to_owned),
            duration_ms: song
                .get("duration")
                .and_then(Value::as_f64)
                .map(|seconds| (seconds * 1000.0) as u32),
            artwork_url: self
                .client
                .cover(song.get("coverArt").or_else(|| song.get("albumId"))),
        })
    }

    fn songs(&self, parent: &Value, key: &str) -> Vec<Song> {
        list(parent, key)
            .iter()
            .filter_map(|song| self.song(song))
            .collect()
    }

    /// Whether the server takes long requests as forms.
    async fn takes_forms(&self) -> bool {
        if let Some(known) = *self
            .forms
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
        {
            return known;
        }
        let known = self
            .client
            .call("getOpenSubsonicExtensions", &[])
            .await
            .map(|answer| {
                list(&answer, "openSubsonicExtensions")
                    .iter()
                    .any(|extension| {
                        extension.get("name").and_then(Value::as_str) == Some("formPost")
                    })
            })
            .unwrap_or(false);
        *self
            .forms
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(known);
        known
    }

    /// Calls `method` with `query` and a list of song ids, as one form when the server takes
    /// forms, else in the address (`whole` says the list can't be split over requests).
    async fn with_ids(
        &self,
        method: &str,
        query: &[(&str, String)],
        name: &str,
        ids: &[String],
        whole: bool,
    ) -> Result<Value, String> {
        let forms = ids.len() > IDS_PER_ADDRESS && self.takes_forms().await;
        let chunks: Vec<&[String]> = if forms || whole || ids.is_empty() {
            vec![ids]
        } else {
            ids.chunks(IDS_PER_ADDRESS).collect()
        };
        let mut last = Value::Null;
        for chunk in chunks {
            let mut all: Vec<(&str, String)> = query.to_vec();
            all.extend(chunk.iter().map(|id| (name, id.clone())));
            last = self
                .client
                .send(method, &all, forms)
                .await
                .map_err(|failure| failure.message)?;
        }
        Ok(last)
    }

    /// Songs the server finds for `query`.
    pub async fn search(&self, query: &str, limit: u32) -> Result<Vec<Song>, String> {
        let found = self
            .client
            .search3(query, limit, 0, 0)
            .await
            .map_err(|failure| failure.message)?;
        Ok(self.songs(&found, "song"))
    }

    /// The songs the user starred.
    pub async fn starred(&self) -> Result<Vec<Song>, String> {
        let answer = self
            .client
            .call("getStarred2", &[])
            .await
            .map_err(|failure| failure.message)?;
        Ok(self.songs(answer.get("starred2").unwrap_or(&Value::Null), "song"))
    }

    /// Stars songs, or takes their stars off.
    pub async fn star(&self, ids: &[String], starred: bool) -> Result<(), String> {
        if ids.is_empty() {
            return Ok(());
        }
        let method = if starred { "star" } else { "unstar" };
        self.with_ids(method, &[], "id", ids, false)
            .await
            .map(|_| ())
    }

    /// The playlists the user sees: their own and those shared with them.
    pub async fn playlists(&self) -> Result<Vec<Playlist>, String> {
        let answer = self
            .client
            .call("getPlaylists", &[])
            .await
            .map_err(|failure| failure.message)?;
        Ok(
            list(answer.get("playlists").unwrap_or(&Value::Null), "playlist")
                .iter()
                .filter_map(|playlist| self.playlist_head(playlist))
                .collect(),
        )
    }

    fn playlist_head(&self, playlist: &Value) -> Option<Playlist> {
        let owner = playlist
            .get("owner")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let mine = owner.as_deref().is_none_or(|owner| {
            self.username.is_empty() || owner.eq_ignore_ascii_case(&self.username)
        });
        Some(Playlist {
            id: playlist.get("id").and_then(value_text)?,
            name: playlist
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            owner,
            editable: mine && playlist.get("readonly").and_then(Value::as_bool) != Some(true),
        })
    }

    /// A playlist's songs, in order.
    pub async fn playlist(&self, id: &str) -> Result<Vec<Song>, String> {
        let answer = self
            .client
            .call("getPlaylist", &[("id", id.to_owned())])
            .await
            .map_err(|failure| failure.message)?;
        Ok(self.songs(answer.get("playlist").unwrap_or(&Value::Null), "entry"))
    }

    /// Makes a playlist of `ids`; its id.
    pub async fn create_playlist(&self, name: &str, ids: &[String]) -> Result<String, String> {
        let answer = self
            .with_ids(
                "createPlaylist",
                &[("name", name.to_owned())],
                "songId",
                ids,
                true,
            )
            .await?;
        if let Some(id) = answer
            .get("playlist")
            .and_then(|playlist| playlist.get("id"))
            .and_then(value_text)
        {
            return Ok(id);
        }
        // Servers before API 1.14 answer nothing: the newest of that name is it.
        self.playlists()
            .await?
            .into_iter()
            .rev()
            .find(|playlist| playlist.name == name)
            .map(|playlist| playlist.id)
            .ok_or_else(|| "the server didn't say which playlist it made".to_owned())
    }

    /// Makes a playlist hold exactly `ids`, in order.
    pub async fn replace_playlist(&self, id: &str, ids: &[String]) -> Result<(), String> {
        if ids.is_empty() {
            // `createPlaylist` with no songs leaves them: each is taken out instead.
            let count = self.playlist(id).await?.len();
            if count == 0 {
                return Ok(());
            }
            let mut query = vec![("playlistId", id.to_owned())];
            query.extend((0..count).map(|index| ("songIndexToRemove", index.to_string())));
            return self
                .client
                .call("updatePlaylist", &query)
                .await
                .map(|_| ())
                .map_err(|failure| failure.message);
        }
        self.with_ids(
            "createPlaylist",
            &[("playlistId", id.to_owned())],
            "songId",
            ids,
            true,
        )
        .await
        .map(|_| ())
    }

    pub async fn rename_playlist(&self, id: &str, name: &str) -> Result<(), String> {
        self.client
            .call(
                "updatePlaylist",
                &[("playlistId", id.to_owned()), ("name", name.to_owned())],
            )
            .await
            .map(|_| ())
            .map_err(|failure| failure.message)
    }

    pub async fn delete_playlist(&self, id: &str) -> Result<(), String> {
        self.client
            .call("deletePlaylist", &[("id", id.to_owned())])
            .await
            .map(|_| ())
            .map_err(|failure| failure.message)
    }

    /// Tells the server a song was played (`submission`) or is playing now, at `time_ms`.
    pub async fn scrobble(&self, id: &str, time_ms: i64, submission: bool) -> Result<(), String> {
        self.client
            .call(
                "scrobble",
                &[
                    ("id", id.to_owned()),
                    ("time", time_ms.to_string()),
                    ("submission", submission.to_string()),
                ],
            )
            .await
            .map(|_| ())
            .map_err(|failure| failure.message)
    }

    /// The play queue the server keeps; `None` when it keeps none.
    pub async fn play_queue(&self) -> Result<Option<PlayQueue>, String> {
        let answer = match self.client.call("getPlayQueue", &[]).await {
            Ok(answer) => answer,
            // Some servers say "not found" for an empty queue.
            Err(failure) if error_code(&failure) == Some(70) => return Ok(None),
            Err(failure) => return Err(failure.message),
        };
        let Some(queue) = answer.get("playQueue") else {
            return Ok(None);
        };
        let songs = self.songs(queue, "entry");
        if songs.is_empty() {
            return Ok(None);
        }
        Ok(Some(PlayQueue {
            songs,
            current: queue.get("current").and_then(value_text),
            position_ms: queue
                .get("position")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            changed_by: queue
                .get("changedBy")
                .and_then(Value::as_str)
                .filter(|client| !client.is_empty())
                .map(str::to_owned),
        }))
    }

    /// Keeps `ids` as the user's play queue on the server, at `current` and `position_ms`.
    pub async fn save_play_queue(
        &self,
        ids: &[String],
        current: Option<&str>,
        position_ms: u64,
    ) -> Result<(), String> {
        let mut query = vec![("position", position_ms.to_string())];
        if let Some(current) = current {
            query.push(("current", current.to_owned()));
        }
        self.with_ids("savePlayQueue", &query, "id", ids, true)
            .await
            .map(|_| ())
    }
}

/// How a connection signs in, from its kept secret.
pub(crate) fn sign_for(config: &super::ServerConfig, secret: String) -> Sign {
    match config.secret {
        super::SecretKind::ApiKey => Sign::Key(secret),
        super::SecretKind::PlainPassword => Sign::Plain {
            username: config.username.clone(),
            password: secret,
        },
        _ => Sign::Token {
            username: config.username.clone(),
            password: secret,
        },
    }
}

/// A list field that servers send as an array, a single object, or not at all.
fn list(parent: &Value, key: &str) -> Vec<Value> {
    match parent.get(key) {
        Some(Value::Array(items)) => items.clone(),
        Some(item @ Value::Object(_)) => vec![item.clone()],
        _ => Vec::new(),
    }
}

/// An id as text: servers send strings, some old ones numbers.
fn value_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_salted_md5s_of_the_password() {
        // The example in Subsonic's API documentation.
        let sign = Sign::Token {
            username: "admin".into(),
            password: "sesame".into(),
        };
        let query = sign.query("c19b2d");
        assert!(query.contains(&("t".into(), "26719a1196d2a940705a59634eb18eab".into())));
        assert!(query.contains(&("s".into(), "c19b2d".into())));
        let plain = Sign::Plain {
            username: "admin".into(),
            password: "sesame".into(),
        };
        assert!(
            plain
                .query("x")
                .contains(&("p".into(), "enc:736573616d65".into()))
        );
        let key = Sign::Key("k".into()).query("x");
        assert!(
            key.iter().all(|(name, _)| name != "u"),
            "a key names no user"
        );
    }

    #[test]
    fn failures_keep_their_code() {
        let failure = api_failure(
            "demo.example.org",
            &json!({ "code": 40, "message": "Wrong username or password" }),
        );
        assert_eq!(error_code(&failure), Some(40));
        assert_eq!(failure.trouble.unwrap().kind, IssueKind::Denied);
        assert_eq!(list(&json!({ "song": { "id": "1" } }), "song").len(), 1);
        assert!(list(&json!({}), "song").is_empty());
    }
}
