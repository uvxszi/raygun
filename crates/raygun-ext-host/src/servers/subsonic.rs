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
        let mut url = self.url(method, query);
        {
            let mut pairs = url.query_pairs_mut();
            for (name, value) in &self.auth {
                pairs.append_pair(name, value);
            }
            pairs.append_pair("f", "json");
        }
        let answer: Value = self.http.json(self.http.client().get(url)).await?;
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
            "artist": song.get("artist").and_then(Value::as_str),
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
            "artist": album.get("artist").and_then(Value::as_str),
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
                let mut url = self.url(
                    "stream",
                    &[
                        ("id", id),
                        (
                            "maxBitRate",
                            target.map_or(0, |kbps| kbps.round() as i64).to_string(),
                        ),
                        ("format", if readable { "raw" } else { "mp3" }.to_owned()),
                    ],
                );
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
                    || song
                        .get("artist")
                        .and_then(Value::as_str)
                        .is_some_and(|name| {
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
            let structured = answer
                .get("lyricsList")
                .and_then(|lyrics| lyrics.get("structuredLyrics"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
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

/// Pings the server with a sign-in; `Ok` when it accepts it.
pub(crate) async fn ping(http: &Http, base: &Url, sign: &Sign) -> Result<(), Failure> {
    Subsonic::new(http.clone(), base.clone(), sign)
        .call("ping", &[])
        .await
        .map(|_| ())
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
