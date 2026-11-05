//! Song lists from outside Raygun (M46.21), as pure parsing: Spotify links and their embed pages,
//! CSV exports (Exportify's columns, or any with title and artist columns), pasted lines
//! ("Artist - Title"), and the text lines of a screenshot of a song list.

use std::collections::HashSet;

use raygun_model::ImportedSong;
use raygun_resolver::tidy_credit;

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
                artist: field(artist)
                    .map(|artists| tidy_credit(&artists.replace(";", ", ")))
                    .filter(|artists| !artists.is_empty()),
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
                artist: artist.map(tidy_credit).filter(|artist| !artist.is_empty()),
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

/// A row read from a screenshot with where its lines sit, so rows split in two can be joined.
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenRow {
    pub song: ImportedSong,
    /// The title line's top, left edge and height.
    pub top: f32,
    pub x: f32,
    pub height: f32,
    /// The artist line's left edge, top and height, when the row has one.
    pub artist_line: Option<(f32, f32, f32)>,
    /// The title line read as an artist line ("E " badge and "• album" dropped).
    pub as_artist: String,
    /// The title line carries an artist line's marks: an "E " badge or "Artist • Album".
    pub marked: bool,
}

/// An artist line's text without its marks, and whether it had any: the explicit badge ("E ")
/// before the name, and what follows " • " or " · " (the album, a play count). A quote left at
/// an end of the name (an icon read as one) goes too.
fn artist_text(text: &str) -> (String, bool) {
    let text = text.trim();
    let badge = text.strip_prefix("E ").or_else(|| text.strip_prefix("🅴 "));
    let plain = badge.unwrap_or(text);
    let name = plain.split(" • ").next().unwrap_or(plain);
    let name = name.split(" · ").next().unwrap_or(name).trim();
    (
        tidy_credit(name),
        badge.is_some() || name.len() < plain.trim().len(),
    )
}

/// Songs in a screenshot of a song list (Spotify, YouTube Music, Apple Music…): the list's text
/// column (the most common left edge), lines grouped into rows by the gaps between them, each
/// row's first line its title and the next its artist ("E " badges and "• album" dropped).
#[cfg(test)]
pub fn screenshot_rows(lines: Vec<TextLine>) -> Vec<ImportedSong> {
    let mut songs: Vec<ImportedSong> = Vec::new();
    for row in screenshot_layout(lines) {
        if !songs
            .iter()
            .any(|known| known.title == row.song.title && known.artist == row.song.artist)
        {
            songs.push(row.song);
        }
    }
    songs
}

/// The rows of a screenshot of a song list, top to bottom, repeats included (see
/// `screenshot_rows`).
pub fn screenshot_layout(mut lines: Vec<TextLine>) -> Vec<ScreenRow> {
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
    // An artist line may start further right, after an explicit badge read as a picture.
    let body: Vec<&TextLine> = lines
        .iter()
        .filter(|line| line.x >= column - 20.0 && line.x <= column + 48.0)
        .collect();
    let mut heights: Vec<f32> = body.iter().map(|line| line.height).collect();
    heights.sort_by(f32::total_cmp);
    let typical = heights[heights.len() / 2].max(1.0);
    let mut rows: Vec<Vec<&TextLine>> = Vec::new();
    for line in body {
        match rows.last_mut() {
            // A line under a row belongs to it when it's close, and no taller than the line
            // above (an artist or album line is never bigger than its title).
            Some(row)
                if row.len() < 3
                    && row.last().is_some_and(|last| {
                        line.y - last.y <= 2.2 * typical && line.height <= last.height * 1.2
                    }) =>
            {
                row.push(line);
            }
            _ => rows.push(vec![line]),
        }
    }
    rows.into_iter()
        .filter_map(|row| {
            let first = row[0];
            let title = strip_list_marks(&first.text);
            if title.is_empty() {
                return None;
            }
            let artist = row
                .get(1)
                .map(|line| artist_text(&line.text).0)
                .filter(|artist| !artist.is_empty());
            let (as_artist, marked) = artist_text(&first.text);
            Some(ScreenRow {
                song: ImportedSong {
                    title,
                    artist,
                    album: None,
                    duration_ms: None,
                    artwork_url: None,
                },
                top: first.y,
                x: first.x,
                height: first.height,
                artist_line: row.get(1).map(|line| (line.x, line.y, line.height)),
                as_artist,
                marked,
            })
        })
        .collect()
}

/// How sure Raygun is that a row is an artist line split off from the row before.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitCall {
    /// Join them.
    Join,
    /// Ask the sources which reading they know better.
    Unsure,
}

fn median(mut values: Vec<f32>) -> Option<f32> {
    values.sort_by(f32::total_cmp);
    values.get(values.len() / 2).copied()
}

/// Rows that are probably an artist line read as a song of its own: `(i, call)` says row `i + 1`
/// is the artist of row `i`. Only a row without an artist under a row without one can be; then
/// the evidence is an artist line's marks, a name known as an artist (`known_artists`,
/// normalized), a smaller line than the title above, the indent other rows' artist lines have,
/// and sitting closer to the row above than rows sit to each other.
pub fn split_rows(rows: &[ScreenRow], known_artists: &HashSet<String>) -> Vec<(usize, SplitCall)> {
    // How the list's whole rows look: their titles' and artists' heights, the artist line's
    // indent and its distance below the title.
    let whole: Vec<(&ScreenRow, (f32, f32, f32))> = rows
        .iter()
        .filter_map(|row| row.artist_line.map(|line| (row, line)))
        .collect();
    let title_height = median(whole.iter().map(|(row, _)| row.height).collect());
    let artist_height = median(whole.iter().map(|(_, (_, _, height))| *height).collect());
    let indent = median(whole.iter().map(|(row, (x, _, _))| x - row.x).collect());
    let offset = median(whole.iter().map(|(row, (_, y, _))| y - row.top).collect());

    let mut calls = Vec::new();
    let mut at = 0;
    while at + 1 < rows.len() {
        let (above, below) = (&rows[at], &rows[at + 1]);
        if above.song.artist.is_some() || below.song.artist.is_some() || below.as_artist.is_empty()
        {
            at += 1;
            continue;
        }
        let mut points = 0;
        if below.marked {
            points += 3;
        }
        if known_artists.contains(&raygun_resolver::normalize(&below.as_artist)) {
            points += 3;
        }
        // Size: an artist line is smaller than its title.
        if below.height <= above.height * 0.85 {
            points += 2;
        } else if below.height >= above.height * 1.1 {
            points -= 2;
        }
        if let (Some(title), Some(artist)) = (title_height, artist_height)
            && artist < title * 0.9
            && (below.height - artist).abs() < (below.height - title).abs()
        {
            points += 1;
        }
        // Indent: as other rows' artist lines, or to the right of the title.
        let shift = below.x - above.x;
        match indent {
            Some(indent) if indent.abs() > 4.0 => {
                if (shift - indent).abs() <= 4.0 {
                    points += 1;
                }
            }
            _ if shift > 6.0 => points += 1,
            _ => {}
        }
        // Gap: about where whole rows have their artist line, or closer to the row above than
        // the rows around are to them.
        let gap = below.top - above.top;
        let around: Vec<f32> = [
            at.checked_sub(1).map(|before| above.top - rows[before].top),
            rows.get(at + 2).map(|after| after.top - below.top),
        ]
        .into_iter()
        .flatten()
        .collect();
        match offset.filter(|offset| *offset > 0.0) {
            Some(offset) if gap <= offset * 1.5 => points += 2,
            Some(offset) if gap > offset * 2.5 => points -= 1,
            Some(_) => {}
            None if around.is_empty() => {}
            None if around.iter().all(|other| *other > gap * 1.25) => points += 2,
            None if around.iter().any(|other| *other < gap * 0.8) => points -= 1,
            None => {}
        }
        if points >= 4 {
            calls.push((at, SplitCall::Join));
            at += 2;
        } else if points >= 1 {
            calls.push((at, SplitCall::Unsure));
            at += 2;
        } else {
            at += 1;
        }
    }
    calls
}

/// Whether the sources know "title by artist" better than the two rows as songs of their own:
/// the best match's points for the joined reading, and for each row alone.
pub fn joined_reading_wins(joined: Option<i32>, title: Option<i32>, artist: Option<i32>) -> bool {
    let Some(joined) = joined.filter(|points| *points >= raygun_resolver::MIN_SCORE) else {
        return false;
    };
    let apart = (title.unwrap_or(0) + artist.unwrap_or(0)) / 2;
    // The row read as a song wins when it is a song the sources know as well.
    joined >= apart && artist.is_none_or(|alone| joined > alone)
}

/// The songs of a screenshot's rows with the rows at `joins` joined to the row under them; a
/// joined song carries the two songs it was made of.
pub fn join_rows(
    rows: Vec<ScreenRow>,
    joins: &[usize],
) -> Vec<(ImportedSong, Option<Vec<ImportedSong>>)> {
    let mut songs = Vec::with_capacity(rows.len());
    let mut rows = rows.into_iter().enumerate().peekable();
    while let Some((at, row)) = rows.next() {
        if joins.contains(&at)
            && let Some((_, below)) = rows.next_if(|(next, _)| *next == at + 1)
        {
            let song = ImportedSong {
                artist: Some(below.as_artist.clone()),
                ..row.song.clone()
            };
            songs.push((song, Some(vec![row.song, below.song])));
        } else {
            songs.push((row.song, None));
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
    fn imported_artists_lose_quotes_left_around_them() {
        let pasted = lines("\"Five Finger Death Punch\" - Welcome to the Circus\n");
        assert_eq!(pasted[0].artist.as_deref(), Some("Five Finger Death Punch"));
        let file = csv("title,artist\nBad Company,\"\"\"Five Finger Death Punch\"\n").unwrap();
        assert_eq!(file[0].artist.as_deref(), Some("Five Finger Death Punch"));
        // An icon before the name read as a quote.
        assert_eq!(
            artist_text("\" Five Finger Death Punch • F8"),
            ("Five Finger Death Punch".to_owned(), true)
        );
        assert_eq!(
            artist_text("\"Weird Al\" Yankovic"),
            ("\"Weird Al\" Yankovic".to_owned(), false)
        );
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

    fn line(y: f32, x: f32, height: f32, text: &str) -> TextLine {
        TextLine {
            x,
            y,
            height,
            text: text.into(),
        }
    }

    /// The songs after joining every split the layout calls, sure or not.
    fn joined(lines: Vec<TextLine>, known: &[&str]) -> Vec<(String, Option<String>, bool)> {
        let rows = screenshot_layout(lines);
        let known: HashSet<String> = known
            .iter()
            .map(|name| raygun_resolver::normalize(name))
            .collect();
        let joins: Vec<usize> = split_rows(&rows, &known)
            .into_iter()
            .map(|(at, _)| at)
            .collect();
        join_rows(rows, &joins)
            .into_iter()
            .map(|(song, parts)| (song.title, song.artist, parts.is_some()))
            .collect()
    }

    fn song(title: &str, artist: Option<&str>, joined: bool) -> (String, Option<String>, bool) {
        (title.into(), artist.map(str::to_owned), joined)
    }

    #[test]
    fn spotify_rows_split_by_small_line_heights_are_joined() {
        // Windows' OCR of Spotify's phone app giving short line heights, so every artist line
        // lands past the row gap, in the order OCR returns them (by block, not by line). The top
        // line is the artist of a song scrolled out of the picture.
        let lines = vec![
            line(533.0, 113.0, 15.0, "Charli xcx"),
            line(600.0, 113.0, 20.0, "Espresso"),
            line(740.0, 113.0, 20.0, "Birds of a Feather"),
            line(880.0, 113.0, 20.0, "APT."),
            line(653.0, 113.0, 15.0, "Sabrina Carpenter"),
            line(793.0, 141.0, 15.0, "E Billie Eilish"),
            line(933.0, 113.0, 15.0, "ROSÉ, Bruno Mars"),
            line(610.0, 980.0, 15.0, "2:55"),
            line(612.0, 59.0, 18.0, "1"),
        ];
        let rows = screenshot_layout(lines.clone());
        assert_eq!(rows.len(), 7, "every line is a row of its own");
        assert_eq!(
            split_rows(&rows, &HashSet::new()),
            vec![
                (1, SplitCall::Join),
                (3, SplitCall::Join),
                (5, SplitCall::Join)
            ]
        );
        assert_eq!(
            joined(lines, &[]),
            vec![
                song("Charli xcx", None, false),
                song("Espresso", Some("Sabrina Carpenter"), true),
                song("Birds of a Feather", Some("Billie Eilish"), true),
                song("APT.", Some("ROSÉ, Bruno Mars"), true),
            ]
        );
    }

    #[test]
    fn apple_music_rows_split_in_a_whole_list_are_joined() {
        // Apple Music's playlist view: most rows read whole, one artist line read further down.
        let lines = vec![
            line(400.0, 180.0, 29.0, "Loser"),
            line(452.0, 180.0, 21.0, "Tame Impala"),
            line(540.0, 180.0, 29.0, "Housewife"),
            line(610.0, 180.0, 21.0, "Tate McRae"),
            line(680.0, 180.0, 29.0, "Nicole Kidman"),
            line(732.0, 180.0, 21.0, "Lily Allen"),
        ];
        assert_eq!(
            joined(lines, &[]),
            vec![
                song("Loser", Some("Tame Impala"), false),
                song("Housewife", Some("Tate McRae"), true),
                song("Nicole Kidman", Some("Lily Allen"), false),
            ]
        );
    }

    #[test]
    fn unsure_splits_wait_for_the_sources_unless_the_artist_is_known() {
        // A line as tall as the title under it: only the gap says it's an artist line.
        let lines = vec![
            line(400.0, 180.0, 29.0, "Loser"),
            line(452.0, 180.0, 21.0, "Tame Impala"),
            line(540.0, 180.0, 29.0, "Let It Happen"),
            line(610.0, 180.0, 28.0, "Tame Impala"),
            line(680.0, 180.0, 29.0, "Housewife"),
            line(732.0, 180.0, 21.0, "Tate McRae"),
        ];
        let rows = screenshot_layout(lines);
        assert_eq!(
            split_rows(&rows, &HashSet::new()),
            vec![(1, SplitCall::Unsure)]
        );
        let known = HashSet::from([raygun_resolver::normalize("Tame Impala")]);
        assert_eq!(split_rows(&rows, &known), vec![(1, SplitCall::Join)]);
    }

    #[test]
    fn title_only_lists_stay_apart() {
        // An album's songs: titles alone, alike in size and spacing.
        let lines = vec![
            line(400.0, 113.0, 29.0, "Speak to Me"),
            line(500.0, 113.0, 29.0, "Breathe (In the Air)"),
            line(600.0, 113.0, 29.0, "On the Run"),
            line(700.0, 113.0, 29.0, "Time"),
        ];
        assert!(split_rows(&screenshot_layout(lines), &HashSet::new()).is_empty());
    }

    #[test]
    fn a_taller_line_starts_the_next_row() {
        // Tight spacing: the next title is close enough to group, but taller than an artist.
        let songs = screenshot_rows(vec![
            line(400.0, 113.0, 29.0, "Loser"),
            line(440.0, 113.0, 21.0, "Tame Impala"),
            line(480.0, 113.0, 29.0, "Housewife"),
            line(520.0, 113.0, 21.0, "Tate McRae"),
        ]);
        assert_eq!(songs.len(), 2);
        assert_eq!(songs[1].artist.as_deref(), Some("Tate McRae"));
    }

    #[test]
    fn the_sources_pick_the_reading_they_know() {
        // "Espresso" by Sabrina Carpenter beats two songs named "Espresso" and "Sabrina Carpenter".
        assert!(joined_reading_wins(Some(78), Some(40), None));
        assert!(joined_reading_wins(Some(78), Some(40), Some(25)));
        // A real song under it ("Nicole Kidman" by Lily Allen) isn't anyone's artist.
        assert!(!joined_reading_wins(None, Some(40), Some(40)));
        assert!(!joined_reading_wins(Some(38), Some(40), Some(40)));
        assert!(!joined_reading_wins(Some(20), None, None));
    }
}
