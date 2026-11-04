//! Podcast feeds (M37): RSS 2.0 with the iTunes tags, and Atom. Lenient about the details real
//! feeds get wrong; only episodes with an audio (or video) enclosure are kept.

use std::borrow::Cow;

use ego_tree::iter::Edge;
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use raygun_db::podcasts::{EpisodeFields, PodcastFields};
use scraper::{Html, Node};

/// Feeds larger than this are refused (some long-running shows reach a few MB).
pub const MAX_FEED_BYTES: usize = 16 * 1024 * 1024;
/// Longest text kept from a description.
const MAX_DESCRIPTION: usize = 20_000;

#[derive(Debug, Default)]
struct Item {
    guid: Option<String>,
    title: Option<String>,
    description: Option<String>,
    summary: Option<String>,
    enclosure: Option<(String, Option<String>)>,
    duration: Option<String>,
    published: Option<String>,
    image: Option<String>,
    link: Option<String>,
    /// `itunes:episode` (M46.22: a book's chapter number).
    number: Option<String>,
    /// `<podcast:transcript>`: the best one as `<type>|<url>` (M46.32).
    transcript: Option<String>,
}

/// What the feed says about the podcast and its episodes, newest first as the feed lists them.
/// Episodes of a serial feed (an audiobook's chapters) carry their number: `itunes:episode`, else
/// their place in the feed.
pub fn parse(bytes: &[u8], feed_url: &str) -> Result<(PodcastFields, Vec<EpisodeFields>), String> {
    // Text isn't trimmed per event: entity references split it, and spaces around them count.
    let mut reader = Reader::from_reader(bytes);
    let mut buffer = Vec::new();
    // Element names from the root down (local names, lower case, with any prefix kept).
    let mut path: Vec<String> = Vec::new();
    let mut channel = Item::default();
    let mut author: Option<String> = None;
    let mut items: Vec<Item> = Vec::new();
    let mut current: Option<Item> = None;
    let mut text = String::new();
    let mut is_feed = false;
    let mut serial = false;

    loop {
        let event = reader
            .read_event_into(&mut buffer)
            .map_err(|error| format!("This isn't a readable feed: {error}"))?;
        match event {
            Event::Start(element) => {
                let name = name_of(&element);
                if matches!(name.as_str(), "rss" | "feed" | "rdf:rdf") {
                    is_feed = true;
                }
                if matches!(name.as_str(), "item" | "entry") {
                    current = Some(Item::default());
                }
                attributes(&element, &name, current.as_mut().unwrap_or(&mut channel));
                path.push(name);
                text.clear();
            }
            Event::Empty(element) => {
                let name = name_of(&element);
                attributes(&element, &name, current.as_mut().unwrap_or(&mut channel));
            }
            // Entity references arrive as their own events, so text needs no unescaping.
            Event::Text(content) => text.push_str(&content),
            Event::CData(content) => text.push_str(&content),
            Event::GeneralRef(reference) => {
                text.push_str(&unescape(&format!("&{};", &*reference)));
            }
            Event::End(_) => {
                let name = path.pop().unwrap_or_default();
                let value = std::mem::take(&mut text).trim().to_owned();
                if matches!(name.as_str(), "item" | "entry") {
                    if let Some(item) = current.take() {
                        items.push(item);
                    }
                    continue;
                }
                if value.is_empty() {
                    continue;
                }
                let parent = path.last().map(String::as_str).unwrap_or_default();
                let in_item = current.is_some();
                let target = current.as_mut().unwrap_or(&mut channel);
                match name.as_str() {
                    "title" if matches!(parent, "channel" | "item" | "entry" | "feed") => {
                        target.title.get_or_insert(value);
                    }
                    "guid" | "id" => {
                        target.guid.get_or_insert(value);
                    }
                    "description" | "content:encoded" | "content" | "subtitle" => {
                        // The longest of the descriptions is usually the full one.
                        if target
                            .description
                            .as_ref()
                            .is_none_or(|old| old.len() < value.len())
                        {
                            target.description = Some(value);
                        }
                    }
                    "itunes:summary" | "summary" => {
                        target.summary.get_or_insert(value);
                    }
                    "itunes:duration" => target.duration = Some(value),
                    "itunes:episode" => target.number = Some(value),
                    "itunes:type" if !in_item => {
                        serial = value.eq_ignore_ascii_case("serial");
                    }
                    "pubdate" | "published" | "updated" | "dc:date" => {
                        target.published.get_or_insert(value);
                    }
                    "url" if parent == "image" => {
                        target.image.get_or_insert(value);
                    }
                    "link" if matches!(parent, "channel" | "item") => {
                        target.link.get_or_insert(value);
                    }
                    "itunes:author" | "name" if current.is_none() => {
                        author.get_or_insert(value);
                    }
                    "author" | "managingeditor" if current.is_none() => {
                        author.get_or_insert(value);
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }

    if !is_feed {
        return Err("This address isn't a podcast feed".into());
    }
    let title = channel
        .title
        .as_deref()
        .map(plain_line)
        .filter(|title| !title.is_empty())
        .ok_or_else(|| "This feed has no title".to_owned())?;
    let podcast = PodcastFields {
        feed_url: feed_url.to_owned(),
        title,
        author: author
            .as_deref()
            .map(plain_line)
            .filter(|name| !name.is_empty()),
        description: channel
            .description
            .clone()
            .or(channel.summary.clone())
            .map(|text| clean_text(&text)),
        artwork: channel.image.clone().filter(|url| is_http(url)),
        link: channel.link.clone().filter(|url| is_http(url)),
        kind: "podcast".into(),
    };
    let episodes = items
        .into_iter()
        .enumerate()
        .filter_map(|(index, item)| {
            let (audio_url, audio_type) = item.enclosure?;
            if !is_http(&audio_url) {
                return None;
            }
            Some(EpisodeFields {
                guid: item.guid.unwrap_or_else(|| audio_url.clone()),
                title: item
                    .title
                    .as_deref()
                    .map(plain_line)
                    .filter(|title| !title.is_empty())
                    .unwrap_or_else(|| "Untitled episode".to_owned()),
                description: item
                    .description
                    .or(item.summary)
                    .map(|text| clean_text(&text)),
                audio_url,
                audio_type,
                duration_ms: item.duration.as_deref().and_then(duration_ms),
                published_at: item.published.as_deref().and_then(published_ms),
                artwork: item.image.filter(|url| is_http(url)),
                number: item
                    .number
                    .as_deref()
                    .and_then(|number| number.trim().parse::<i64>().ok())
                    .or_else(|| serial.then_some(index as i64 + 1)),
                transcript: item.transcript,
            })
        })
        .collect();
    Ok((podcast, episodes))
}

fn name_of(element: &BytesStart<'_>) -> String {
    element.name().as_ref().to_lowercase()
}

/// Reads the attributes that matter: enclosures, `itunes:image`, Atom links.
fn attributes(element: &BytesStart<'_>, name: &str, target: &mut Item) {
    let attribute = |key: &str| {
        element
            .attributes()
            .flatten()
            .find(|attribute| attribute.key.as_ref().eq_ignore_ascii_case(key))
            .and_then(|attribute| {
                attribute
                    .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                    .ok()
                    .map(|value| value.trim().to_owned())
            })
            .filter(|value| !value.is_empty())
    };
    match name {
        "enclosure" => {
            if let Some(url) = attribute("url") {
                let kind = attribute("type");
                let playable = kind
                    .as_deref()
                    .is_none_or(|kind| kind.starts_with("audio/") || kind.starts_with("video/"));
                if playable && target.enclosure.is_none() {
                    target.enclosure = Some((url, kind));
                }
            }
        }
        // Podcasting 2.0 transcripts (M46.32): timed formats first.
        "podcast:transcript" => {
            if let (Some(url), Some(kind)) = (attribute("url"), attribute("type")) {
                let rank = |kind: &str| match kind {
                    "text/vtt" => 3,
                    "application/srt" | "application/x-subrip" | "text/srt" => 2,
                    "application/json" => 1,
                    _ => 0,
                };
                let better = target
                    .transcript
                    .as_deref()
                    .and_then(|kept| kept.split_once('|'))
                    .is_none_or(|(kept, _)| rank(&kind) > rank(kept));
                if rank(&kind) > 0 && better && is_http(&url) {
                    target.transcript = Some(format!("{kind}|{url}"));
                }
            }
        }
        "itunes:image" | "media:thumbnail" => {
            if let Some(href) = attribute("href").or_else(|| attribute("url")) {
                // The iTunes image is the square artwork; prefer it.
                target.image = Some(href);
            }
        }
        // Atom: <link rel="enclosure" href=".." type=".."> and <link rel="alternate" href="..">.
        "link" => {
            let Some(href) = attribute("href") else {
                return;
            };
            match attribute("rel").as_deref() {
                Some("enclosure") if target.enclosure.is_none() => {
                    target.enclosure = Some((href, attribute("type")));
                }
                None | Some("alternate") => {
                    target.link.get_or_insert(href);
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn is_http(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// Turns XML entity references into text (feeds double-escape HTML in descriptions).
fn unescape(text: &str) -> String {
    quick_xml::escape::unescape(text)
        .map(|value| value.into_owned())
        .unwrap_or_else(|_| text.to_owned())
}

/// Elements that start a line of their own in show notes (list items do too, as "• ").
const BLOCKS: [&str; 21] = [
    "p",
    "div",
    "ul",
    "ol",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "tr",
    "table",
    "blockquote",
    "section",
    "article",
    "header",
    "footer",
    "pre",
    "dd",
    "dt",
    "hr",
];

/// A description as plain text: tags dropped, every HTML character reference read (`&rsquo;`,
/// `&nbsp;`), paragraphs kept as blank lines. Plain-text notes keep their own line breaks.
pub fn clean_text(html: &str) -> String {
    let marked_up = html.match_indices('<').any(|(at, _)| {
        html[at + 1..].starts_with(|ch: char| ch.is_ascii_alphabetic() || ch == '/')
    });
    if !marked_up {
        return paragraphs(&decode_entities(html), false);
    }
    let fragment = Html::parse_fragment(html);
    let mut out = String::with_capacity(html.len().min(MAX_DESCRIPTION));
    // Text inside these never shows.
    let mut hidden = 0usize;
    for edge in fragment.root_element().traverse() {
        match edge {
            Edge::Open(node) => match node.value() {
                Node::Text(text) if hidden == 0 => out.push_str(text),
                Node::Element(element) => match element.name() {
                    "script" | "style" | "template" => hidden += 1,
                    "br" => out.push('\n'),
                    "li" => out.push_str("\n• "),
                    name if BLOCKS.contains(&name) => out.push('\n'),
                    _ => {}
                },
                _ => {}
            },
            Edge::Close(node) => {
                if let Node::Element(element) = node.value() {
                    match element.name() {
                        "script" | "style" | "template" => hidden = hidden.saturating_sub(1),
                        name if BLOCKS.contains(&name) => out.push('\n'),
                        _ => {}
                    }
                }
            }
        }
        if out.len() >= MAX_DESCRIPTION {
            break;
        }
    }
    paragraphs(&out, true)
}

/// Lines trimmed, at most one blank line between paragraphs; `collapse` runs spaces together as
/// HTML does.
fn paragraphs(text: &str, collapse: bool) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = if collapse {
            // HTML's spaces; a no-break space stays.
            line.split_ascii_whitespace().collect::<Vec<_>>().join(" ")
        } else {
            line.trim().to_owned()
        };
        if !line.is_empty() || lines.last().is_some_and(|last| !last.is_empty()) {
            lines.push(line);
        }
    }
    let mut text = lines.join("\n").trim().to_owned();
    if text.len() > MAX_DESCRIPTION {
        let mut end = MAX_DESCRIPTION;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    text
}

/// One line of text from a feed (a title, a name): character references read and spaces run
/// together. CDATA keeps escaped HTML as it is ("Q&amp;A"), so titles need this after XML.
pub fn plain_line(text: &str) -> String {
    decode_entities(text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// HTML character references in `text`: `&amp;`, `&rsquo;`, `&#8217;`, `&#x2019;`. Anything that
/// isn't one (a lone `&`, an unknown name) stays as written.
pub fn decode_entities(text: &str) -> Cow<'_, str> {
    if !text.contains('&') {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        match reference(tail) {
            Some((decoded, used)) => {
                out.push_str(&decoded);
                rest = &tail[used..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    Cow::Owned(out)
}

/// What the reference at the start of `text` stands for, and how long it is.
fn reference(text: &str) -> Option<(String, usize)> {
    // Names are short: `&CounterClockwiseContourIntegral;` is the longest.
    let end = text[1..]
        .char_indices()
        .take_while(|(at, _)| *at < 33)
        .find(|(_, ch)| *ch == ';')?
        .0
        + 1;
    let name = &text[1..end];
    let decoded = match name.strip_prefix('#') {
        Some(number) => {
            let code = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse::<u32>().ok()?,
            };
            match code {
                0 => return None,
                // Windows-1252's characters, as browsers read them (`&#146;` is ’).
                0x80..=0x9f => html_reference(&text[..=end])?,
                _ => char::from_u32(code)?.to_string(),
            }
        }
        None if !name.is_empty() && name.chars().all(|ch| ch.is_ascii_alphanumeric()) => match name
        {
            "amp" => "&".to_owned(),
            "lt" => "<".to_owned(),
            "gt" => ">".to_owned(),
            "quot" => "\"".to_owned(),
            "apos" => "'".to_owned(),
            "nbsp" => "\u{a0}".to_owned(),
            _ => html_reference(&text[..=end])?,
        },
        None => return None,
    };
    Some((decoded, end + 1))
}

/// A reference read by the HTML parser, which knows every name; `None` when it isn't one. Every
/// name stands for one or two characters, so a longer answer is an old name read inside an
/// unknown one ("&notaname;" isn't "¬aname;").
fn html_reference(reference: &str) -> Option<String> {
    let fragment = Html::parse_fragment(reference);
    let text: String = fragment.root_element().text().collect();
    (text != reference && (1..=2).contains(&text.chars().count())).then_some(text)
}

/// `itunes:duration`: seconds, `MM:SS` or `HH:MM:SS`.
fn duration_ms(text: &str) -> Option<i64> {
    let parts: Vec<f64> = text
        .trim()
        .split(':')
        .map(|part| part.trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .ok()?;
    let seconds = parts.iter().fold(0.0, |total, part| total * 60.0 + part);
    (parts.len() <= 3 && seconds > 0.0).then(|| (seconds * 1000.0).round() as i64)
}

/// RSS dates (RFC 2822, often slightly off) and Atom dates (RFC 3339), as unix ms.
fn published_ms(text: &str) -> Option<i64> {
    let text = text.trim();
    chrono::DateTime::parse_from_rfc2822(text)
        .or_else(|_| chrono::DateTime::parse_from_rfc3339(text))
        .or_else(|_| {
            // "Tue, 5 Mar 2024 10:00:00 EST" style zones chrono doesn't know: retry as UTC.
            let without_zone = text.rsplit_once(' ').map_or(text, |(rest, _)| rest);
            chrono::DateTime::parse_from_rfc2822(&format!("{without_zone} +0000"))
        })
        .ok()
        .map(|date| date.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd"
     xmlns:content="http://purl.org/rss/1.0/modules/content/">
  <channel>
    <title>The Example Show</title>
    <link>https://example.com/show</link>
    <itunes:author>Ana &amp; Ben</itunes:author>
    <description><![CDATA[<p>Weekly talk.</p><p>About <b>things</b>.</p>]]></description>
    <itunes:image href="https://img.example/show.jpg"/>
    <item>
      <title>Two: Later</title>
      <guid isPermaLink="false">ep-2</guid>
      <pubDate>Tue, 05 Mar 2024 10:00:00 GMT</pubDate>
      <enclosure url="https://cdn.example/2.mp3" length="123" type="audio/mpeg"/>
      <itunes:duration>1:02:03</itunes:duration>
      <description>Short</description>
      <content:encoded><![CDATA[<p>The long notes &amp; links.</p>]]></content:encoded>
    </item>
    <item>
      <title>One</title>
      <pubDate>Mon, 4 Mar 2024 10:00:00 EST</pubDate>
      <enclosure url="https://cdn.example/1.m4a" type="audio/x-m4a"/>
      <itunes:duration>754</itunes:duration>
      <itunes:image href="https://img.example/1.jpg"/>
    </item>
    <item>
      <title>A blog post with no audio</title>
      <guid>post-1</guid>
    </item>
  </channel>
</rss>"#;

    #[test]
    fn reads_an_rss_podcast() {
        let (podcast, episodes) = parse(RSS.as_bytes(), "https://feeds.example/rss").unwrap();
        assert_eq!(podcast.title, "The Example Show");
        assert_eq!(podcast.author.as_deref(), Some("Ana & Ben"));
        assert_eq!(
            podcast.description.as_deref(),
            Some("Weekly talk.\n\nAbout things.")
        );
        assert_eq!(
            podcast.artwork.as_deref(),
            Some("https://img.example/show.jpg")
        );
        assert_eq!(podcast.link.as_deref(), Some("https://example.com/show"));
        assert_eq!(episodes.len(), 2, "the post without audio is skipped");

        let two = &episodes[0];
        assert_eq!(two.guid, "ep-2");
        assert_eq!(two.title, "Two: Later");
        assert_eq!(two.audio_type.as_deref(), Some("audio/mpeg"));
        assert_eq!(two.duration_ms, Some(3_723_000));
        assert_eq!(two.published_at, Some(1_709_632_800_000));
        assert_eq!(two.description.as_deref(), Some("The long notes & links."));

        let one = &episodes[1];
        assert_eq!(
            one.guid, "https://cdn.example/1.m4a",
            "no guid: the audio URL"
        );
        assert_eq!(one.duration_ms, Some(754_000));
        assert!(one.published_at.is_some(), "an unknown zone still parses");
        assert_eq!(one.artwork.as_deref(), Some("https://img.example/1.jpg"));
    }

    #[test]
    fn reads_an_atom_podcast_and_refuses_other_documents() {
        let atom = r#"<feed xmlns="http://www.w3.org/2005/Atom">
            <title>Atom Cast</title>
            <entry><title>E1</title><id>urn:e1</id><published>2024-03-05T10:00:00Z</published>
              <link rel="enclosure" type="audio/ogg" href="https://cdn.example/e1.ogg"/></entry>
          </feed>"#;
        let (podcast, episodes) = parse(atom.as_bytes(), "https://a.example/atom").unwrap();
        assert_eq!(podcast.title, "Atom Cast");
        assert_eq!(episodes[0].audio_url, "https://cdn.example/e1.ogg");
        assert_eq!(episodes[0].published_at, Some(1_709_632_800_000));

        assert!(parse(b"<html><body>Hi</body></html>", "x").is_err());
        assert!(parse(b"not xml at all <<<", "x").is_err());
    }

    /// LibriVox puts escaped HTML in CDATA ("Tasa &amp; El Rey"); show notes use HTML's names.
    #[test]
    fn titles_and_notes_read_every_character_reference() {
        let feed = r#"<?xml version="1.0"?>
<rss xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd" version="2.0"><channel>
  <title><![CDATA[Don Quijote 1 by Miguel de Cervantes Saavedra (1547 - 1616)]]></title>
  <itunes:author>LibriVox</itunes:author>
  <description><![CDATA[<p>It&rsquo;s a&nbsp;book &amp; more &copy; 1605</p><script>alert(1)</script>
    <ul><li>One</li><li>Two</li></ul>]]></description>
  <item><title><![CDATA[Dedicación: Al Duque de Béjar &amp; Prólogo]]></title>
    <description>Line one
Line two &#8211; still plain</description>
    <enclosure url="https://archive.org/download/q/q_02.mp3" type="audio/mpeg"/></item>
  <item><title>  Q&amp;amp;A &#x2014; part&#160;2  </title>
    <enclosure url="https://archive.org/download/q/q_03.mp3" type="audio/mpeg"/></item>
</channel></rss>"#;
        let (book, chapters) = parse(feed.as_bytes(), "https://librivox.org/rss/567").unwrap();
        assert_eq!(
            book.description.as_deref(),
            Some("It’s a\u{a0}book & more © 1605\n\n• One\n• Two")
        );
        assert_eq!(chapters[0].title, "Dedicación: Al Duque de Béjar & Prólogo");
        assert_eq!(
            chapters[0].description.as_deref(),
            Some("Line one\nLine two – still plain"),
            "plain notes keep their lines"
        );
        assert_eq!(chapters[1].title, "Q&A — part 2");
    }

    #[test]
    fn character_references() {
        assert_eq!(decode_entities("AT&T & co"), "AT&T & co");
        assert_eq!(decode_entities("&rsquo;&#39;&#x27;&amp;"), "’''&");
        assert_eq!(decode_entities("&#146;"), "’", "Windows-1252's characters");
        assert_eq!(
            decode_entities("&notarealname; &#0;"),
            "&notarealname; &#0;"
        );
        assert_eq!(decode_entities("a &b"), "a &b");
        assert!(matches!(decode_entities("plain"), Cow::Borrowed("plain")));
    }

    #[test]
    fn durations() {
        assert_eq!(duration_ms("45"), Some(45_000));
        assert_eq!(duration_ms("02:30"), Some(150_000));
        assert_eq!(duration_ms("1:00:00"), Some(3_600_000));
        assert_eq!(duration_ms("soon"), None);
    }

    #[test]
    fn a_serial_feed_numbers_its_chapters() {
        let feed = br#"<?xml version="1.0"?>
<rss xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd" version="2.0"><channel>
  <title><![CDATA[Pride and Prejudice by Jane Austen]]></title>
  <itunes:type>serial</itunes:type>
  <item><title>Chapters 1-3</title><itunes:episode><![CDATA[1]]></itunes:episode>
    <enclosure url="https://archive.org/download/pp/pp_01_64kb.mp3" type="audio/mpeg"/>
    <itunes:duration><![CDATA[00:18:52]]></itunes:duration></item>
  <item><title>Chapters 4-5</title>
    <enclosure url="https://archive.org/download/pp/pp_02_64kb.mp3" type="audio/mpeg"/></item>
</channel></rss>"#;
        let (book, chapters) = parse(feed, "https://librivox.org/rss/253").unwrap();
        assert_eq!(book.title, "Pride and Prejudice by Jane Austen");
        assert_eq!(chapters[0].number, Some(1));
        assert_eq!(chapters[0].duration_ms, Some(1_132_000));
        assert_eq!(
            chapters[1].number,
            Some(2),
            "no number: its place in a serial feed"
        );
        assert_eq!(chapters[1].published_at, None);
    }
}
