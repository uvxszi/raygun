//! Song lists from outside Raygun (M46.21), as pure parsing: Spotify links and their embed pages,
//! CSV exports (Exportify's columns, or any with title and artist columns), pasted lines
//! ("Artist - Title"), and the text lines of a screenshot of a song list.

use raygun_model::ImportedSong;

/// What a Spotify link points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotifyLink {
    /// `playlist`, `album` or `track`.
    pub kind: String,
    pub id: String,
}

/// Reads `https://open.spotify.com/(intl-xx/)?(embed/)?{kind}/{id}?…` and `spotify:{kind}:{id}`.
pub fn spotify_link(text: &str) -> Option<SpotifyLink> {
    let text = text.trim();
    let (kind, id) = if let Some(rest) = text.strip_prefix("spotify:") {
        let mut parts = rest.split(':');
        (parts.next()?, parts.next()?)
    } else {
        let rest = text
            .strip_prefix("https://")
            .or_else(|| text.strip_prefix("http://"))
            .unwrap_or(text);
        let path = rest.strip_prefix("open.spotify.com/")?;
        let path = path.split(['?', '#']).next()?;
        let mut parts = path
            .split('/')
            .filter(|part| !part.is_empty() && !part.starts_with("intl-") && *part != "embed");
        (parts.next()?, parts.next()?)
    };
    let valid_id = id.len() >= 10 && id.chars().all(|c| c.is_ascii_alphanumeric());
    matches!(kind, "playlist" | "album" | "track")
        .then_some(())
        .filter(|()| valid_id)?;
    Some(SpotifyLink {
        kind: kind.to_owned(),
        id: id.to_owned(),
    })
}

/// A list read from an embed page: its name and songs.
#[derive(Debug, Clone, PartialEq)]
pub struct EmbedList {
    pub name: String,
    pub artwork_url: Option<String>,
    pub songs: Vec<ImportedSong>,
}

fn text_at<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

/// The largest picture in a list of `{url, maxWidth}`.
fn largest_image(value: Option<&serde_json::Value>) -> Option<String> {
    value?
        .as_array()?
        .iter()
        .max_by_key(|image| image.get("maxWidth").and_then(serde_json::Value::as_u64))
        .and_then(|image| text_at(image, "url"))
        .filter(|url| url.starts_with("https://"))
        .map(str::to_owned)
}

/// Reads Spotify's embed page: the `__NEXT_DATA__` JSON's entity, a playlist's or album's track
/// list (title, artists joined, duration) or a single track.
pub fn spotify_embed(html: &str) -> Result<EmbedList, String> {
    const START: &str = r#"<script id="__NEXT_DATA__" type="application/json">"#;
    let start = html
        .find(START)
        .ok_or("Spotify's page has changed; its song list wasn't found")?
        + START.len();
    let end = html[start..]
        .find("</script>")
        .ok_or("Spotify's page was cut short")?;
    let data: serde_json::Value =
        serde_json::from_str(&html[start..start + end]).map_err(|error| error.to_string())?;
    let entity = data
        .pointer("/props/pageProps/state/data/entity")
        .ok_or("Spotify didn't share this list; it may be private")?;
    let artwork_url = largest_image(entity.pointer("/visualIdentity/image"))
        .or_else(|| largest_image(entity.pointer("/coverArt/sources")));
    let name = text_at(entity, "name")
        .or_else(|| text_at(entity, "title"))
        .unwrap_or("Spotify")
        .to_owned();
    let album = (text_at(entity, "type") == Some("album")).then(|| name.clone());
    let songs: Vec<ImportedSong> = match entity
        .get("trackList")
        .and_then(serde_json::Value::as_array)
    {
        Some(list) => list
            .iter()
            .filter_map(|track| {
                let title = text_at(track, "title")?.to_owned();
                Some(ImportedSong {
                    title,
                    artist: text_at(track, "subtitle").map(str::to_owned),
                    album: album.clone(),
                    duration_ms: track
                        .get("duration")
                        .and_then(serde_json::Value::as_u64)
                        .and_then(|ms| u32::try_from(ms).ok()),
                    artwork_url: album.as_ref().and(artwork_url.clone()),
                })
            })
            .collect(),
        None => {
            let title = text_at(entity, "title")
                .or_else(|| text_at(entity, "name"))
                .ok_or("Spotify's page has no song")?;
            let artists: Vec<&str> = entity
                .get("artists")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|artist| text_at(artist, "name"))
                .collect();
            vec![ImportedSong {
                title: title.to_owned(),
                artist: (!artists.is_empty()).then(|| artists.join(", ")),
                album: None,
                duration_ms: entity
                    .get("duration")
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|ms| u32::try_from(ms).ok()),
                artwork_url: artwork_url.clone(),
            }]
        }
    };
    if songs.is_empty() {
        return Err("This list has no songs".into());
    }
    Ok(EmbedList {
        name,
        artwork_url,
        songs,
    })
}

/// Splits a CSV line, with `"quoted, fields"` and `""` for a quote.
fn csv_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(std::mem::take(&mut field)),
            _ => field.push(c),
        }
    }
    fields.push(field);
    fields
        .into_iter()
        .map(|field| field.trim().to_owned())
        .collect()
}

/// Lines of a CSV file, keeping line breaks inside quotes.
fn csv_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut quoted = false;
    for c in text.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                line.push(c);
            }
            '\n' if !quoted => lines.push(std::mem::take(&mut line)),
            '\r' if !quoted => {}
            _ => line.push(c),
        }
    }
    if !line.trim().is_empty() {
        lines.push(line);
    }
    lines
}

/// Reads a CSV export: Exportify's (`Track Name`, `Artist Name(s)`, `Album Name`,
/// `Duration (ms)`) or any with a title or track column and an artist column.
pub fn csv(text: &str) -> Result<Vec<ImportedSong>, String> {
    let text = text.trim_start_matches('\u{feff}');
    let lines = csv_lines(text);
    let header = csv_fields(lines.first().ok_or("The file is empty")?);
    let column = |names: &[&str]| {
        header.iter().position(|field| {
            let field = field.to_lowercase();
            names.iter().any(|name| field == *name)
        })
    };
    let title = column(&["track name", "title", "track", "song", "name"])
        .ok_or("The file has no title column (Track Name or Title)")?;
    let artist = column(&["artist name(s)", "artist", "artists", "artist name"]);
    let album = column(&["album name", "album"]);
    let duration = column(&["duration (ms)", "duration_ms"]);
    let songs: Vec<ImportedSong> = lines
        .iter()
        .skip(1)
        .map(|line| csv_fields(line))
        .filter_map(|fields| {
            let field = |index: Option<usize>| {
                index
                    .and_then(|index| fields.get(index))
                    .filter(|value| !value.is_empty())
                    .cloned()
            };
            Some(ImportedSong {
                title: field(Some(title))?,
                // Exportify separates artists with commas inside the field.
                artist: field(artist).map(|artists| artists.replace(";", ", ")),
                album: field(album),
                duration_ms: field(duration)
                    .and_then(|ms| ms.parse::<f64>().ok())
                    .map(|ms| ms as u32),
                artwork_url: None,
            })
        })
        .collect();
    if songs.is_empty() {
        return Err("The file has no songs".into());
    }
    Ok(songs)
}

/// Pasted lines: "Artist - Title" (or "Title by Artist", or a title alone), one song per line;
/// numbering and durations are left out.
pub fn lines(text: &str) -> Vec<ImportedSong> {
    text.lines()
        .filter_map(|line| {
            let line = strip_list_marks(line);
            if line.is_empty() {
                return None;
            }
            let (title, artist) = if let Some((artist, title)) = line.split_once(" - ") {
                (title.trim(), Some(artist.trim()))
            } else if let Some((artist, title)) = line.split_once(" – ") {
                (title.trim(), Some(artist.trim()))
            } else if let Some((title, artist)) = line.rsplit_once(" by ") {
                (title.trim(), Some(artist.trim()))
            } else {
                (line.as_str(), None)
            };
            (!title.is_empty()).then(|| ImportedSong {
                title: title.to_owned(),
                artist: artist
                    .filter(|artist| !artist.is_empty())
                    .map(str::to_owned),
                album: None,
                duration_ms: None,
                artwork_url: None,
            })
        })
        .collect()
}

/// "1. Song 3:45" → "Song": a leading number and a trailing duration go.
fn strip_list_marks(line: &str) -> String {
    let mut text = line.trim();
    let digits = text.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && digits < 4 {
        let rest = text[digits..].trim_start_matches(['.', ')', ':']);
        if rest.starts_with(' ') || rest.starts_with('\t') {
            text = rest.trim_start();
        }
    }
    let words: Vec<&str> = text.split_whitespace().collect();
    let words = match words.last() {
        Some(last) if is_duration(last) => &words[..words.len() - 1],
        _ => &words[..],
    };
    words.join(" ")
}

fn is_duration(word: &str) -> bool {
    let parts: Vec<&str> = word.split(':').collect();
    (2..=3).contains(&parts.len())
        && parts.iter().all(|part| {
            !part.is_empty() && part.len() <= 2 && part.chars().all(|c| c.is_ascii_digit())
        })
}

/// A line of text read from a picture: where it starts and how tall it is (pixels).
#[derive(Debug, Clone, PartialEq)]
pub struct TextLine {
    pub x: f32,
    pub y: f32,
    pub height: f32,
    pub text: String,
}

/// Words that are a player's controls rather than songs.
const CHROME: &[&str] = &[
    "preview",
    "save on spotify",
    "+ save on spotify",
    "play",
    "shuffle",
    "download",
    "follow",
    "add to playlist",
    "liked songs",
    "title",
    "album",
    "date added",
    "#",
];

/// Songs in a screenshot of a song list (Spotify, YouTube Music, Apple Music…): the list's text
/// column (the most common left edge), lines grouped into rows by the gaps between them, each
/// row's first line its title and the next its artist ("E " badges and "• album" dropped).
pub fn screenshot_rows(mut lines: Vec<TextLine>) -> Vec<ImportedSong> {
    lines.retain(|line| {
        let text = line.text.trim();
        let lower = text.to_lowercase();
        !text.is_empty()
            && !text.chars().all(|c| c.is_ascii_digit())
            && !is_duration(text)
            && !CHROME.contains(&lower.as_str())
    });
    if lines.is_empty() {
        return Vec::new();
    }
    lines.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    // The song column: the left edge most lines share.
    let column = lines
        .iter()
        .map(|line| line.x)
        .max_by_key(|x| {
            lines
                .iter()
                .filter(|other| (other.x - x).abs() <= 8.0)
                .count()
        })
        .unwrap_or(0.0);
    let body: Vec<&TextLine> = lines
        .iter()
        .filter(|line| (line.x - column).abs() <= 20.0)
        .collect();
    let mut heights: Vec<f32> = body.iter().map(|line| line.height).collect();
    heights.sort_by(f32::total_cmp);
    let typical = heights[heights.len() / 2].max(1.0);
    let mut rows: Vec<Vec<&TextLine>> = Vec::new();
    for line in body {
        match rows.last_mut() {
            Some(row)
                if row.len() < 3
                    && line.y - row.last().map_or(line.y, |last| last.y) <= 2.2 * typical =>
            {
                row.push(line);
            }
            _ => rows.push(vec![line]),
        }
    }
    let mut songs: Vec<ImportedSong> = Vec::new();
    for row in rows {
        let title = strip_list_marks(&row[0].text);
        if title.is_empty() {
            continue;
        }
        let artist = row.get(1).map(|line| {
            let text = line.text.trim();
            let text = text
                .strip_prefix("E ")
                .or_else(|| text.strip_prefix("🅴 "))
                .unwrap_or(text);
            let text = text.split(" • ").next().unwrap_or(text);
            text.split(" · ").next().unwrap_or(text).trim().to_owned()
        });
        let song = ImportedSong {
            title,
            artist: artist.filter(|artist| !artist.is_empty()),
            album: None,
            duration_ms: None,
            artwork_url: None,
        };
        if !songs
            .iter()
            .any(|known| known.title == song.title && known.artist == song.artist)
        {
            songs.push(song);
        }
    }
    songs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spotify_links_are_read_in_their_forms() {
        fn link(text: &str) -> Option<(String, String)> {
            spotify_link(text).map(|link| (link.kind, link.id))
        }
        let id = "37i9dQZF1DXcBWIGoYBM5M";
        let want = Some(("playlist".to_owned(), id.to_owned()));
        assert_eq!(
            link(&format!("https://open.spotify.com/playlist/{id}?si=abc")),
            want
        );
        assert_eq!(
            link(&format!("https://open.spotify.com/intl-de/playlist/{id}")),
            want
        );
        assert_eq!(link(&format!("open.spotify.com/embed/playlist/{id}")), want);
        assert_eq!(link(&format!("spotify:playlist:{id}")), want);
        assert_eq!(
            link("https://open.spotify.com/track/2LlQb7Uoj1kKyGhlkBf9aC").map(|(kind, _)| kind),
            Some("track".to_owned())
        );
        assert_eq!(
            link("https://open.spotify.com/artist/3fMbdgg4jU18AjLCKBhRSm"),
            None
        );
        assert_eq!(
            link("https://example.com/playlist/37i9dQZF1DXcBWIGoYBM5M"),
            None
        );
        assert_eq!(link("https://open.spotify.com/playlist/../../x"), None);
    }

    fn page(entity: serde_json::Value) -> String {
        format!(
            r#"<html><script id="__NEXT_DATA__" type="application/json">{}</script></html>"#,
            serde_json::json!({ "props": { "pageProps": { "state": { "data": { "entity": entity } } } } })
        )
    }

    #[test]
    fn embed_pages_give_their_songs() {
        let playlist = spotify_embed(&page(serde_json::json!({
            "type": "playlist", "name": "Today's Top Hits",
            "visualIdentity": { "image": [
                { "url": "https://i.scdn.co/small", "maxWidth": 64 },
                { "url": "https://i.scdn.co/big", "maxWidth": 640 }
            ] },
            "trackList": [
                { "title": "Loser", "subtitle": "Tame Impala", "duration": 223069 },
                { "title": "", "subtitle": "Nobody" },
                { "title": "Housewife", "subtitle": "Tate McRae" }
            ]
        })))
        .unwrap();
        assert_eq!(playlist.name, "Today's Top Hits");
        assert_eq!(
            playlist.artwork_url.as_deref(),
            Some("https://i.scdn.co/big")
        );
        assert_eq!(playlist.songs.len(), 2);
        assert_eq!(playlist.songs[0].duration_ms, Some(223_069));
        assert_eq!(
            playlist.songs[0].artwork_url, None,
            "a playlist's cover isn't its songs'"
        );

        let track = spotify_embed(&page(serde_json::json!({
            "type": "track", "title": "Under Pressure",
            "artists": [{ "name": "Queen" }, { "name": "David Bowie" }], "duration": 248000
        })))
        .unwrap();
        assert_eq!(track.songs[0].artist.as_deref(), Some("Queen, David Bowie"));
        assert!(spotify_embed("<html>gone</html>").is_err());
    }

    #[test]
    fn csv_exports_are_read() {
        let exportify = "\u{feff}Track URI,Track Name,Artist Name(s),Album Name,Duration (ms)\n\
            spotify:track:1,\"Hello, Goodbye\",The Beatles,Magical Mystery Tour,208000\n\
            spotify:track:2,Wonderwall,Oasis,\"(What's the Story) Morning Glory?\",258000\n\
            spotify:track:3,,Nobody,,\n";
        let songs = csv(exportify).unwrap();
        assert_eq!(songs.len(), 2);
        assert_eq!(songs[0].title, "Hello, Goodbye");
        assert_eq!(
            songs[1].album.as_deref(),
            Some("(What's the Story) Morning Glory?")
        );
        assert_eq!(songs[1].duration_ms, Some(258_000));
        let simple = csv("title,artist\nCreep,Radiohead\n").unwrap();
        assert_eq!(simple[0].artist.as_deref(), Some("Radiohead"));
        assert!(csv("foo,bar\n1,2\n").is_err());
    }

    #[test]
    fn pasted_lines_become_songs() {
        let songs = lines("1. Radiohead - Creep 3:58\n\nYesterday by The Beatles\nJust a title\n");
        assert_eq!(songs.len(), 3);
        assert_eq!(
            (songs[0].title.as_str(), songs[0].artist.as_deref()),
            ("Creep", Some("Radiohead"))
        );
        assert_eq!(songs[1].artist.as_deref(), Some("The Beatles"));
        assert_eq!(songs[2].artist, None);
    }

    #[test]
    fn screenshot_lines_become_rows() {
        let line = |y: f32, x: f32, h: f32, text: &str| TextLine {
            x,
            y,
            height: h,
            text: text.into(),
        };
        // From Windows' OCR of Spotify's embed (y, x, height).
        let songs = screenshot_rows(vec![
            line(150.0, 397.0, 60.0, "Today's Top Hits"),
            line(236.0, 398.0, 36.0, "Spotify"),
            line(473.0, 59.0, 19.0, "Preview"),
            line(606.0, 113.0, 29.0, "Loser"),
            line(626.0, 59.0, 26.0, "1"),
            line(659.0, 125.0, 18.0, "E Tame Impala"),
            line(747.0, 113.0, 29.0, "Housewife"),
            line(766.0, 57.0, 26.0, "2"),
            line(799.0, 110.0, 21.0, "Tate McRae"),
            line(887.0, 113.0, 29.0, "Nicole Kidman"),
            line(938.0, 113.0, 21.0, "Lily Allen"),
            line(990.0, 520.0, 20.0, "3:35"),
        ]);
        let pairs: Vec<(&str, Option<&str>)> = songs
            .iter()
            .map(|song| (song.title.as_str(), song.artist.as_deref()))
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("Loser", Some("Tame Impala")),
                ("Housewife", Some("Tate McRae")),
                ("Nicole Kidman", Some("Lily Allen")),
            ]
        );
    }
}
