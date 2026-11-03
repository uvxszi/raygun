//! Feed transcripts (Podcasting 2.0 `<podcast:transcript>`): WebVTT, SRT and the JSON format,
//! turned into timed lines.

use raygun_db::captions::CaptionRow;
use serde_json::Value;

/// Parses a transcript of the given MIME type; `None` when the type has no timing.
pub fn parse(kind: &str, text: &str) -> Option<Vec<CaptionRow>> {
    let text = text.trim_start_matches('\u{feff}');
    match kind {
        "text/vtt" | "application/srt" | "application/x-subrip" | "text/srt" => {
            Some(parse_timed_text(text))
        }
        "application/json" => serde_json::from_str::<Value>(text)
            .ok()
            .map(|json| parse_json(&json)),
        _ => None,
    }
}

/// WebVTT and SRT: cues are blocks with a `start --> end` line, then text.
fn parse_timed_text(text: &str) -> Vec<CaptionRow> {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut rows = Vec::new();
    for block in text.split("\n\n") {
        let mut lines = block.lines().map(str::trim).filter(|line| !line.is_empty());
        let Some(timing) = lines.by_ref().find(|line| line.contains("-->")) else {
            continue;
        };
        let mut parts = timing.split("-->");
        let (Some(start), Some(end)) = (
            parts.next().and_then(timestamp),
            parts
                .next()
                .and_then(|end| end.split_whitespace().next())
                .and_then(timestamp),
        ) else {
            continue;
        };
        let words = lines.map(strip_tags).collect::<Vec<_>>().join(" ");
        let words = words.trim();
        if !words.is_empty() {
            rows.push(CaptionRow {
                start_ms: start,
                end_ms: end.max(start),
                text: unescape(words),
            });
        }
    }
    rows.sort_by_key(|row| row.start_ms);
    rows.dedup_by_key(|row| row.start_ms);
    rows
}

/// `hh:mm:ss.mmm`, `mm:ss.mmm` or SRT's `hh:mm:ss,mmm`.
fn timestamp(text: &str) -> Option<i64> {
    let text = text.trim().replace(',', ".");
    let (clock, fraction) = text.split_once('.').unwrap_or((&text, "0"));
    let mut seconds = 0i64;
    for part in clock.split(':') {
        seconds = seconds * 60 + part.trim().parse::<i64>().ok()?;
    }
    let digits: String = fraction
        .chars()
        .take_while(char::is_ascii_digit)
        .take(3)
        .collect();
    let millis = format!("{digits:0<3}").parse::<i64>().ok()?;
    Some(seconds * 1000 + millis)
}

/// Drops `<v Speaker>`, `<i>`, `<00:01.000>` and such.
fn strip_tags(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_tag = false;
    for character in line.chars() {
        match character {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(character),
            _ => {}
        }
    }
    out
}

fn unescape(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
}

/// The JSON format: `segments` of `{startTime, endTime, body, speaker}`, often a word each, joined
/// into lines that end at a sentence, a new speaker or 12 seconds.
fn parse_json(json: &Value) -> Vec<CaptionRow> {
    let mut rows: Vec<CaptionRow> = Vec::new();
    let mut speaker: Option<String> = None;
    let mut line: Option<CaptionRow> = None;
    let seconds =
        |value: Option<&Value>| value.and_then(Value::as_f64).map(|s| (s * 1000.0) as i64);
    for segment in json
        .get("segments")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let (Some(start), Some(body)) = (
            seconds(segment.get("startTime")),
            segment.get("body").and_then(Value::as_str),
        ) else {
            continue;
        };
        let body = body.trim();
        if body.is_empty() {
            continue;
        }
        let end = seconds(segment.get("endTime")).unwrap_or(start).max(start);
        let who = segment
            .get("speaker")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if who.is_some() && who != speaker {
            rows.extend(line.take());
            speaker = who;
        }
        match line.as_mut() {
            Some(current) => {
                current.text.push(' ');
                current.text.push_str(body);
                current.end_ms = end;
            }
            None => {
                line = Some(CaptionRow {
                    start_ms: start,
                    end_ms: end,
                    text: body.to_owned(),
                })
            }
        }
        if let Some(current) = &line {
            let sentence_end = current.text.ends_with(['.', '!', '?']);
            if sentence_end || current.end_ms - current.start_ms >= 12_000 {
                rows.extend(line.take());
            }
        }
    }
    rows.extend(line);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_webvtt() {
        let vtt = "WEBVTT\n\nNOTE made by hand\n\n1\n00:00:01.000 --> 00:00:04.500 align:start\n<v Ann>Welcome to the show.\n\n00:05.250 --> 00:07.000\nLine one\nline two &amp; more\n";
        let rows = parse("text/vtt", vtt).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].start_ms, rows[0].end_ms), (1_000, 4_500));
        assert_eq!(rows[0].text, "Welcome to the show.");
        assert_eq!(rows[1].start_ms, 5_250);
        assert_eq!(rows[1].text, "Line one line two & more");
    }

    #[test]
    fn reads_srt() {
        let srt = "1\r\n01:02:03,040 --> 01:02:05,000\r\n<i>Chapter Two</i>\r\n\r\n2\r\n01:02:05,000 --> 01:02:06,000\r\nIt rained.\r\n";
        let rows = parse("application/srt", srt).unwrap();
        assert_eq!(rows[0].start_ms, 3_723_040);
        assert_eq!(rows[0].text, "Chapter Two");
        assert_eq!(rows[1].text, "It rained.");
    }

    #[test]
    fn joins_json_words_into_lines() {
        let json = r#"{"version":"1.0.0","segments":[
            {"speaker":"Ann","startTime":0.5,"endTime":0.9,"body":"Hello"},
            {"speaker":"Ann","startTime":0.9,"endTime":1.4,"body":"there."},
            {"speaker":"Ann","startTime":1.6,"endTime":2.0,"body":"So"},
            {"speaker":"Bob","startTime":2.2,"endTime":2.6,"body":"Hi!"}
        ]}"#;
        let rows = parse("application/json", json).unwrap();
        let texts: Vec<&str> = rows.iter().map(|row| row.text.as_str()).collect();
        assert_eq!(texts, ["Hello there.", "So", "Hi!"]);
        assert_eq!((rows[0].start_ms, rows[0].end_ms), (500, 1_400));
    }

    #[test]
    fn untimed_types_are_left_out() {
        assert!(parse("text/html", "<p>Hi</p>").is_none());
    }
}
