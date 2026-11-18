//! Whole Spotify playlists and albums through Spotify's Web API, with the person's own app
//! (a Client ID and secret from developer.spotify.com, kept in Settings → Import). Only the
//! documented endpoints are used: the Client Credentials flow (`accounts.spotify.com/api/token`),
//! Get Playlist / Get Playlist Items and Get Album, paged by their `next` links. Without an app,
//! links are read from the embed page, which shows at most 100 songs.

use base64::Engine as _;
use raygun_model::ImportedSong;
use serde_json::Value;

use super::parse::EmbedList;

const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
const API: &str = "https://api.spotify.com/v1";
/// Pages read at most (100 songs each): a list of 100,000 songs.
const MAX_PAGES: usize = 1000;

/// An app's access token (Client Credentials: public data only, nobody's account).
pub async fn token(client: &reqwest::Client, id: &str, secret: &str) -> Result<String, String> {
    let basic = base64::engine::general_purpose::STANDARD.encode(format!("{id}:{secret}"));
    let response = client
        .post(TOKEN_URL)
        .header("Authorization", format!("Basic {basic}"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("grant_type=client_credentials")
        .send()
        .await
        .map_err(|error| format!("Spotify couldn't be reached ({error})"))?;
    let status = response.status();
    let answer: Value = response.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(
            "Spotify didn't accept the app's Client ID and secret; check them in Settings → Import"
                .into(),
        );
    }
    answer
        .get("access_token")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "Spotify's answer had no access token".into())
}

async fn get(client: &reqwest::Client, token: &str, url: &str) -> Result<Value, (u16, String)> {
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|error| (0, format!("Spotify couldn't be reached ({error})")))?;
    let status = response.status().as_u16();
    if !response.status().is_success() {
        return Err((status, format!("Spotify answered HTTP {status}")));
    }
    response
        .json()
        .await
        .map_err(|_| (status, "Spotify's answer couldn't be read".to_owned()))
}

/// Every song of a playlist or album (`kind` as in a link), its name and cover.
pub async fn whole_list(
    client: &reqwest::Client,
    token: &str,
    kind: &str,
    id: &str,
) -> Result<EmbedList, String> {
    let not_found = |kind: &str| {
        format!(
            "Spotify has no such {kind} for apps; it may be private, or one of Spotify's own \
             playlists, which apps can't read"
        )
    };
    let (head, first_page) = match kind {
        "playlist" => {
            let head = get(
                client,
                token,
                &format!("{API}/playlists/{id}?fields=name,images"),
            )
            .await
            .map_err(|(status, message)| {
                if status == 404 {
                    not_found("playlist")
                } else {
                    message
                }
            })?;
            // Get Playlist Items: `/items` today, `/tracks` before it (the same pages).
            let mut page = None;
            for path in ["items", "tracks"] {
                match get(
                    client,
                    token,
                    &format!("{API}/playlists/{id}/{path}?limit=100&offset=0"),
                )
                .await
                {
                    Ok(answer) => {
                        page = Some(answer);
                        break;
                    }
                    Err((404, _)) => continue,
                    Err((_, message)) => return Err(message),
                }
            }
            (head, page.ok_or_else(|| not_found("playlist"))?)
        }
        "album" => {
            let head = get(client, token, &format!("{API}/albums/{id}"))
                .await
                .map_err(|(status, message)| {
                    if status == 404 {
                        not_found("album")
                    } else {
                        message
                    }
                })?;
            let page = head.get("tracks").cloned().unwrap_or(Value::Null);
            (head, page)
        }
        _ => return Err("Only playlists and albums are read through Spotify's Web API".into()),
    };
    let album = (kind == "album").then(|| album_of(&head));
    let mut songs = Vec::new();
    let mut page = first_page;
    for _ in 0..MAX_PAGES {
        songs.extend(page_songs(&page, album.as_ref()));
        let Some(next) = page.get("next").and_then(Value::as_str).map(str::to_owned) else {
            break;
        };
        page = get(client, token, &next)
            .await
            .map_err(|(_, message)| message)?;
    }
    Ok(EmbedList {
        name: head
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Spotify list")
            .to_owned(),
        artwork_url: first_image(&head),
        songs,
    })
}

/// An album's name and cover, for its tracks (which don't carry them).
#[derive(Debug, Clone)]
pub(crate) struct AlbumInfo {
    name: Option<String>,
    cover: Option<String>,
}

fn album_of(value: &Value) -> AlbumInfo {
    AlbumInfo {
        name: value.get("name").and_then(Value::as_str).map(str::to_owned),
        cover: first_image(value),
    }
}

fn first_image(value: &Value) -> Option<String> {
    value
        .get("images")?
        .as_array()?
        .first()?
        .get("url")?
        .as_str()
        .map(str::to_owned)
}

/// The songs of a paging object: a playlist's entries (`item`, or `track` before) or an album's
/// tracks; entries without a track (removed songs) are skipped.
pub(crate) fn page_songs(page: &Value, album: Option<&AlbumInfo>) -> Vec<ImportedSong> {
    let Some(items) = page.get("items").and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|entry| {
            let track = match album {
                Some(_) => entry,
                None => entry.get("item").or_else(|| entry.get("track"))?,
            };
            let title = track.get("name")?.as_str()?.trim();
            if title.is_empty() {
                return None;
            }
            let artists: Vec<&str> = track
                .get("artists")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|artist| artist.get("name").and_then(Value::as_str))
                .filter(|name| !name.trim().is_empty())
                .collect();
            let own_album = track.get("album");
            Some(ImportedSong {
                title: title.to_owned(),
                artist: (!artists.is_empty()).then(|| artists.join(", ")),
                album: own_album
                    .and_then(|album| album.get("name"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .or_else(|| album.and_then(|album| album.name.clone())),
                duration_ms: track
                    .get("duration_ms")
                    .and_then(Value::as_u64)
                    .and_then(|ms| u32::try_from(ms).ok()),
                artwork_url: own_album
                    .and_then(first_image)
                    .or_else(|| album.and_then(|album| album.cover.clone())),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn playlist_pages_read_items_and_older_track_entries() {
        let page = json!({ "items": [
            { "item": { "name": "Africa", "duration_ms": 295000, "artists": [{ "name": "Toto" }],
              "album": { "name": "Toto IV", "images": [{ "url": "https://i.scdn.co/a" }] } } },
            { "track": { "name": "Creep", "artists": [{ "name": "Radiohead" }, { "name": "X" }] } },
            { "track": null },
        ], "next": null });
        let songs = page_songs(&page, None);
        assert_eq!(songs.len(), 2);
        assert_eq!(songs[0].title, "Africa");
        assert_eq!(songs[0].album.as_deref(), Some("Toto IV"));
        assert_eq!(songs[0].duration_ms, Some(295_000));
        assert_eq!(songs[0].artwork_url.as_deref(), Some("https://i.scdn.co/a"));
        assert_eq!(songs[1].artist.as_deref(), Some("Radiohead, X"));
    }

    #[test]
    fn album_tracks_take_the_albums_name_and_cover() {
        let album = album_of(&json!({ "name": "Discovery", "images": [{ "url": "c" }] }));
        let page =
            json!({ "items": [{ "name": "One More Time", "artists": [{ "name": "Daft Punk" }] }] });
        let songs = page_songs(&page, Some(&album));
        assert_eq!(songs[0].album.as_deref(), Some("Discovery"));
        assert_eq!(songs[0].artwork_url.as_deref(), Some("c"));
    }
}
