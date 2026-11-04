//! Subtitle files for TVs (Play on TV): what a source serves (SubRip, WebVTT, ASS/SSA) as WebVTT,
//! which Google Cast receivers read, or as SubRip, which most DLNA TVs read.

/// One line of subtitles on screen, in milliseconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cue {
    pub start_ms: u64,
    pub end_ms: u64,
    /// Lines separated by `\n`; only `<i>`, `<b>` and `<u>` tags are kept.
    pub text: String,
}

/// The cues of a subtitle file, in order. ASS/SSA is told by its `[Events]` section; anything
/// else is read as SubRip or WebVTT (timing lines with `-->`).
pub fn parse(text: &str) -> Vec<Cue> {
    let text = text.trim_start_matches('\u{feff}');
    let mut cues = if text.contains("[Events]") || text.contains("\nDialogue:") {
        parse_ass(text)
    } else {
        parse_timed(text)
    };
    cues.sort_by_key(|cue| cue.start_ms);
    cues
}

/// The cues as a WebVTT file.
pub fn to_vtt(cues: &[Cue]) -> String {
    let mut out = String::from("WEBVTT\n");
    for cue in cues {
        out.push_str(&format!(
            "\n{} --> {}\n{}\n",
            stamp(cue.start_ms, '.'),
            stamp(cue.end_ms, '.'),
            vtt_text(&cue.text)
        ));
    }
    out
}

/// The cues as a SubRip file.
pub fn to_srt(cues: &[Cue]) -> String {
    let mut out = String::new();
    for (index, cue) in cues.iter().enumerate() {
        out.push_str(&format!(
            "{}\r\n{} --> {}\r\n{}\r\n\r\n",
            index + 1,
            stamp(cue.start_ms, ','),
            stamp(cue.end_ms, ','),
            cue.text.replace('\n', "\r\n")
        ));
    }
    out
}

fn stamp(ms: u64, separator: char) -> String {
    format!(
        "{:02}:{:02}:{:02}{separator}{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}

/// Text WebVTT reads as text: `&` and stray `<` escaped, the kept tags left.
fn vtt_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find(['&', '<']) {
        out.push_str(&rest[..index]);
        let tail = &rest[index..];
        if let Some(after) = tail.strip_prefix('&') {
            out.push_str("&amp;");
            rest = after;
        } else if let Some(tag) = kept_tag(tail) {
            out.push_str(tag);
            rest = &tail[tag.len()..];
        } else {
            out.push_str("&lt;");
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out.replace("-->", "->")
}

/// `<i>`, `</b>`… at the start of `text`.
fn kept_tag(text: &str) -> Option<&str> {
    let end = text.find('>')?;
    let tag = &text[..=end];
    let name = tag
        .trim_start_matches('<')
        .trim_start_matches('/')
        .trim_end_matches('>');
    matches!(name, "i" | "b" | "u").then_some(tag)
}

/// Removes `{\an8}`-style override blocks and every tag but `<i>`, `<b>` and `<u>`.
fn clean(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    loop {
        match rest.find(['{', '<']) {
            None => {
                out.push_str(rest);
                break;
            }
            Some(index) => {
                out.push_str(&rest[..index]);
                let tail = &rest[index..];
                let close = if tail.starts_with('{') { '}' } else { '>' };
                match tail.find(close) {
                    Some(end) => {
                        if let Some(tag) = kept_tag(&tail[..=end]).filter(|_| close == '>') {
                            out.push_str(&tag.to_ascii_lowercase());
                        }
                        rest = &tail[end + 1..];
                    }
                    None => {
                        out.push_str(tail);
                        break;
                    }
                }
            }
        }
    }
    out.trim().to_owned()
}

/// SubRip and WebVTT: a timing line, then the cue's lines up to a blank one.
fn parse_timed(text: &str) -> Vec<Cue> {
    let mut cues = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let Some((start, end)) = line.split_once("-->") else {
            continue;
        };
        // WebVTT puts cue settings after the end time.
        let end = end.split_whitespace().next().unwrap_or_default();
        let (Some(start_ms), Some(end_ms)) = (timestamp(start.trim()), timestamp(end)) else {
            continue;
        };
        let mut body = Vec::new();
        while let Some(next) = lines.peek() {
            if next.trim().is_empty() || next.contains("-->") {
                break;
            }
            let cleaned = clean(next);
            if !cleaned.is_empty() {
                body.push(cleaned);
            }
            lines.next();
        }
        // A cue number before the next timing line isn't text.
        if body
            .last()
            .is_some_and(|last| last.chars().all(|c| c.is_ascii_digit()))
            && lines.peek().is_some_and(|next| next.contains("-->"))
        {
            body.pop();
        }
        if !body.is_empty() && end_ms > start_ms {
            cues.push(Cue {
                start_ms,
                end_ms,
                text: body.join("\n"),
            });
        }
    }
    cues
}

/// `01:02:03,456`, `01:02:03.456` or `02:03.456`.
fn timestamp(text: &str) -> Option<u64> {
    let text = text.trim();
    let (clock, fraction) = text.split_once([',', '.']).unwrap_or((text, "0"));
    let mut seconds = 0u64;
    for part in clock.split(':') {
        seconds = seconds * 60 + part.trim().parse::<u64>().ok()?;
    }
    let digits: String = fraction.chars().take_while(char::is_ascii_digit).collect();
    let millis = match digits.len() {
        0 => 0,
        1 => digits.parse::<u64>().ok()? * 100,
        2 => digits.parse::<u64>().ok()? * 10,
        _ => digits[..3].parse::<u64>().ok()?,
    };
    Some(seconds * 1000 + millis)
}

/// ASS/SSA: `Dialogue:` lines in the order the `Format:` line of `[Events]` gives.
fn parse_ass(text: &str) -> Vec<Cue> {
    let mut columns: Vec<String> = Vec::new();
    let mut in_events = false;
    let mut cues = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_events = line.eq_ignore_ascii_case("[Events]");
            continue;
        }
        if !in_events {
            continue;
        }
        if let Some(format) = line.strip_prefix("Format:") {
            columns = format
                .split(',')
                .map(|name| name.trim().to_ascii_lowercase())
                .collect();
            continue;
        }
        let Some(fields) = line.strip_prefix("Dialogue:") else {
            continue;
        };
        if columns.is_empty() {
            columns = [
                "layer", "start", "end", "style", "name", "marginl", "marginr", "marginv",
                "effect", "text",
            ]
            .map(str::to_owned)
            .to_vec();
        }
        let values: Vec<&str> = fields.splitn(columns.len(), ',').collect();
        let field = |name: &str| {
            columns
                .iter()
                .position(|column| column == name)
                .and_then(|index| values.get(index))
                .map(|value| value.trim())
        };
        let (Some(start_ms), Some(end_ms)) = (
            field("start").and_then(timestamp),
            field("end").and_then(timestamp),
        ) else {
            continue;
        };
        let raw = field("text").unwrap_or_default();
        let lines: Vec<String> = raw
            .replace("\\N", "\n")
            .replace("\\n", "\n")
            .replace("\\h", " ")
            .split('\n')
            .map(clean)
            .filter(|line| !line.is_empty())
            .collect();
        if !lines.is_empty() && end_ms > start_ms {
            cues.push(Cue {
                start_ms,
                end_ms,
                text: lines.join("\n"),
            });
        }
    }
    cues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subrip_becomes_webvtt() {
        let srt = "\u{feff}1\r\n00:00:01,000 --> 00:00:02,500\r\n{\\an8}<font color=\"#fff\">Hello</font> & <i>welcome</i>\r\n\r\n2\r\n00:01:02,040 --> 00:01:04,000\r\nTwo\r\nlines\r\n";
        let cues = parse(srt);
        assert_eq!(
            cues,
            [
                Cue {
                    start_ms: 1_000,
                    end_ms: 2_500,
                    text: "Hello & <i>welcome</i>".into()
                },
                Cue {
                    start_ms: 62_040,
                    end_ms: 64_000,
                    text: "Two\nlines".into()
                },
            ]
        );
        assert_eq!(
            to_vtt(&cues),
            "WEBVTT\n\n00:00:01.000 --> 00:00:02.500\nHello &amp; <i>welcome</i>\n\n00:01:02.040 --> 00:01:04.000\nTwo\nlines\n"
        );
    }

    #[test]
    fn webvtt_settings_and_short_stamps() {
        let vtt = "WEBVTT\n\nNOTE a note\n\n00:05.5 --> 00:07.250 align:start line:10%\n<c.yellow>Yellow</c> text\n";
        let cues = parse(vtt);
        assert_eq!(cues.len(), 1);
        assert_eq!((cues[0].start_ms, cues[0].end_ms), (5_500, 7_250));
        assert_eq!(cues[0].text, "Yellow text");
        assert_eq!(
            to_srt(&cues),
            "1\r\n00:00:05,500 --> 00:00:07,250\r\nYellow text\r\n\r\n"
        );
    }

    #[test]
    fn ass_dialogue_in_its_columns() {
        let ass = "[Script Info]\nTitle: x\n\n[V4+ Styles]\nFormat: Name, Fontname\nStyle: Default,Arial\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:03.50,0:00:05.00,Default,,0,0,0,,{\\i1}Hi,{\\i0} there\\Nsecond\\hline\nDialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,First\nComment: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,not shown\n";
        let cues = parse(ass);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].text, "First");
        assert_eq!(
            (cues[1].start_ms, cues[1].end_ms, cues[1].text.as_str()),
            (3_500, 5_000, "Hi, there\nsecond line")
        );
    }
}
