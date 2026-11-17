//! A downloaded file's name: from the server's `Content-Disposition`, the browser's guess or the
//! address, made safe for every file system, with an extension from its type when it has none,
//! and kept apart from files already there ("report (1).pdf").

use crate::export::names::safe_name;

/// The name a `Content-Disposition` header gives: `filename*` (RFC 5987, percent-encoded UTF-8)
/// first, then `filename`.
pub fn disposition_name(header: &str) -> Option<String> {
    let mut plain = None;
    let mut extended = None;
    for part in split_params(header) {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        match key.as_str() {
            "filename*" => {
                // charset'language'percent-encoded
                let mut pieces = value.splitn(3, '\'');
                let charset = pieces.next().unwrap_or_default();
                let encoded = pieces.nth(1).unwrap_or(value);
                let bytes = percent_decode(unquote(encoded).as_bytes());
                let text = if charset.eq_ignore_ascii_case("iso-8859-1") {
                    bytes.iter().map(|byte| *byte as char).collect()
                } else {
                    String::from_utf8_lossy(&bytes).into_owned()
                };
                extended = Some(text);
            }
            "filename" => {
                let value = unquote(value);
                // Some servers percent-encode the plain form too.
                let decoded = if value.contains('%') {
                    String::from_utf8(percent_decode(value.as_bytes())).unwrap_or(value)
                } else {
                    value
                };
                plain = Some(decoded);
            }
            _ => {}
        }
    }
    extended
        .or(plain)
        .map(|name| last_segment(&name).trim().to_owned())
        .filter(|name| !name.is_empty())
}

/// `; `-separated parameters, minding quoted strings.
fn split_params(header: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut escaped = false;
    for char in header.chars() {
        match char {
            _ if escaped => {
                current.push(char);
                escaped = false;
            }
            '\\' if quoted => {
                current.push(char);
                escaped = true;
            }
            '"' => {
                quoted = !quoted;
                current.push(char);
            }
            ';' if !quoted => parts.push(std::mem::take(&mut current)),
            _ => current.push(char),
        }
    }
    parts.push(current);
    parts
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    match value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    {
        Some(inner) => {
            let mut out = String::new();
            let mut chars = inner.chars();
            while let Some(char) = chars.next() {
                if char == '\\' {
                    if let Some(next) = chars.next() {
                        out.push(next);
                    }
                } else {
                    out.push(char);
                }
            }
            out
        }
        None => value.to_owned(),
    }
}

pub fn percent_decode(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        if input[index] == b'%'
            && index + 2 < input.len()
            && let (Some(high), Some(low)) = (hex(input[index + 1]), hex(input[index + 2]))
        {
            out.push(high << 4 | low);
            index += 3;
            continue;
        }
        out.push(input[index]);
        index += 1;
    }
    out
}

fn hex(byte: u8) -> Option<u8> {
    (byte as char).to_digit(16).map(|digit| digit as u8)
}

/// The part after the last slash or backslash: a name never carries a path.
fn last_segment(name: &str) -> &str {
    name.rsplit(['/', '\\']).next().unwrap_or(name)
}

/// The name a file is saved under: the server's, else the browser's guess, else the address's
/// last part, else "download"; with an extension from its type when it has none.
pub fn file_name(
    url: &str,
    disposition: Option<&str>,
    hint: Option<&str>,
    mime: Option<&str>,
) -> String {
    let from_url = || {
        let parsed = url::Url::parse(url).ok()?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return None;
        }
        let segment = parsed
            .path_segments()?
            .rev()
            .find(|part| !part.is_empty())?;
        let decoded = String::from_utf8_lossy(&percent_decode(segment.as_bytes())).into_owned();
        Some(decoded)
    };
    let name = disposition
        .and_then(disposition_name)
        .or_else(|| {
            hint.map(|hint| last_segment(hint).trim().to_owned())
                .filter(|hint| !hint.is_empty())
        })
        .or_else(from_url)
        .unwrap_or_else(|| "download".to_owned());
    let name = safe_name(&name);
    let has_extension = extension(&name).is_some();
    match mime.and_then(extension_for) {
        Some(extension) if !has_extension => format!("{name}.{extension}"),
        _ => name,
    }
}

/// A name's extension (`pdf` for `report.pdf`); dot files and trailing dots have none.
fn extension(name: &str) -> Option<&str> {
    let (stem, extension) = name.rsplit_once('.')?;
    (!stem.is_empty() && !extension.is_empty() && extension.len() <= 8 && !extension.contains(' '))
        .then_some(extension)
}

/// A common type's extension.
fn extension_for(mime: &str) -> Option<&'static str> {
    let mime = mime.split(';').next()?.trim().to_ascii_lowercase();
    Some(match mime.as_str() {
        "application/pdf" => "pdf",
        "application/zip" | "application/x-zip-compressed" => "zip",
        "application/epub+zip" => "epub",
        "application/x-cbz" | "application/vnd.comicbook+zip" => "cbz",
        "application/x-7z-compressed" => "7z",
        "application/x-rar-compressed" | "application/vnd.rar" => "rar",
        "application/gzip" | "application/x-gzip" => "gz",
        "application/x-tar" => "tar",
        "application/json" => "json",
        "application/xml" | "text/xml" => "xml",
        "application/vnd.android.package-archive" => "apk",
        "application/x-msdownload" | "application/vnd.microsoft.portable-executable" => "exe",
        "application/x-msi" => "msi",
        "application/msword" => "doc",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "docx",
        "application/vnd.ms-excel" => "xls",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "xlsx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => "pptx",
        "text/plain" => "txt",
        "text/csv" => "csv",
        "text/html" => "html",
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/avif" => "avif",
        "image/jxl" => "jxl",
        "image/svg+xml" => "svg",
        "audio/mpeg" => "mp3",
        "audio/mp4" | "audio/x-m4a" => "m4a",
        "audio/ogg" => "ogg",
        "audio/opus" => "opus",
        "audio/flac" | "audio/x-flac" => "flac",
        "audio/wav" | "audio/x-wav" => "wav",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/x-matroska" => "mkv",
        "video/quicktime" => "mov",
        "font/ttf" => "ttf",
        "font/otf" => "otf",
        "font/woff2" => "woff2",
        _ => return None,
    })
}

/// `name`, or `name (1)`, `name (2)`… before its extension: the first one not `taken`.
pub fn unique_name(name: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(name) {
        return name.to_owned();
    }
    let (stem, extension) = match extension(name) {
        Some(extension) => (&name[..name.len() - extension.len() - 1], Some(extension)),
        None => (name, None),
    };
    for number in 1..10_000 {
        let candidate = match extension {
            Some(extension) => format!("{stem} ({number}).{extension}"),
            None => format!("{stem} ({number})"),
        };
        if !taken(&candidate) {
            return candidate;
        }
    }
    format!(
        "{stem} ({}){}",
        uuid::Uuid::now_v7(),
        extension.map(|e| format!(".{e}")).unwrap_or_default()
    )
}
