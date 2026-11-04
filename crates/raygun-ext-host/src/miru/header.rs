//! A Miru script's metadata: the `// ==MiruExtension==` block at its top.
//!
//! The rules follow miru-repo's `main.go` (MIT), which writes the repository index from the
//! block: each `// @key value` line inside it. A script without the block falls back to the Miru
//! app's own `parseExtension` (miru-app, AGPL-3.0): every `@key value` anywhere in the file, the
//! last one winning.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::manga::MANGA_SOURCE;
use crate::novel::NOVEL_SOURCE;
use crate::video::VIDEO_SOURCE;

/// What a Miru extension serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiruType {
    /// Video: anime, films, series, live channels.
    Bangumi,
    Manga,
    /// Novels.
    Fikushon,
}

impl MiruType {
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "bangumi" => Some(Self::Bangumi),
            "manga" => Some(Self::Manga),
            "fikushon" => Some(Self::Fikushon),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bangumi => "bangumi",
            Self::Manga => "manga",
            Self::Fikushon => "fikushon",
        }
    }

    /// The Raygun provider kind a script of this type becomes.
    pub fn provider_kind(self) -> &'static str {
        match self {
            Self::Bangumi => VIDEO_SOURCE,
            Self::Manga => MANGA_SOURCE,
            Self::Fikushon => NOVEL_SOURCE,
        }
    }

    /// The kind in words, for descriptions.
    pub fn noun(self) -> &'static str {
        match self {
            Self::Bangumi => "video",
            Self::Manga => "comics",
            Self::Fikushon => "novel",
        }
    }
}

/// The fields Raygun reads from a script's metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub name: String,
    pub package: String,
    pub version: String,
    pub kind: MiruType,
    /// The base `this.request` puts before relative addresses, as the script wrote it.
    pub web_site: String,
    pub lang: Option<String>,
    pub license: Option<String>,
    pub author: Option<String>,
    pub icon: Option<String>,
    pub description: Option<String>,
    pub nsfw: bool,
}

static BLOCK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"MiruExtension([\s\S]+?)/MiruExtension").expect("valid regex"));
static ANYWHERE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"@(\w+)\s+(.*)").expect("valid regex"));

/// Every `@key value` of the script's metadata, by key.
pub fn fields(code: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    if let Some(block) = BLOCK.find(code) {
        for line in block.as_str().lines() {
            let Some(rest) = line.trim().strip_prefix("// @") else {
                continue;
            };
            let rest = rest.trim();
            let (key, value) = rest
                .split_once(char::is_whitespace)
                .map_or((rest, ""), |(key, value)| (key, value.trim()));
            if !key.is_empty() {
                fields.insert(key.to_owned(), value.to_owned());
            }
        }
    } else {
        for found in ANYWHERE.captures_iter(code) {
            fields.insert(found[1].to_owned(), found[2].trim().to_owned());
        }
    }
    fields
}

/// The script's metadata; `None` when a field Miru requires (name, package, version, type,
/// webSite) is missing, or the type isn't one Miru knows.
pub fn parse(code: &str) -> Option<Header> {
    let mut fields = fields(code);
    let mut take = |key: &str| {
        fields
            .remove(key)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    };
    let name = take("name")?;
    let package = take("package")?;
    let version = take("version")?;
    let kind = MiruType::parse(&take("type")?)?;
    // Miru requires the field but allows it empty: requests then need a full `Miru-Url`.
    let web_site = take("webSite").unwrap_or_default();
    Some(Header {
        name,
        package,
        version,
        kind,
        web_site,
        lang: take("lang"),
        license: take("license"),
        author: take("author"),
        icon: take("icon"),
        description: take("description"),
        nsfw: take("nsfw").is_some_and(|nsfw| nsfw == "true"),
    })
}

/// A Miru version (`v0.1.2`, `1.2`, `0.0.1-beta`) as semver; others become a pre-release of
/// 0.0.0 named after them, so they still compare and never look like an update forever.
pub fn semver_version(version: &str) -> String {
    let trimmed = version
        .trim()
        .trim_start_matches(['v', 'V'])
        .trim()
        .to_owned();
    if semver::Version::parse(&trimmed).is_ok() {
        return trimmed;
    }
    let numeric: Vec<&str> = trimmed.split('.').collect();
    if (1..=3).contains(&numeric.len())
        && numeric
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        let mut parts: Vec<u64> = numeric
            .iter()
            .map(|part| part.parse().unwrap_or(0))
            .collect();
        parts.resize(3, 0);
        return format!("{}.{}.{}", parts[0], parts[1], parts[2]);
    }
    let label: String = trimmed
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let label = label.trim_matches('-');
    if label.is_empty() {
        "0.0.0".to_owned()
    } else {
        format!("0.0.0-{label}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCRIPT: &str = "// ==MiruExtension==\r\n// @name         media.ccc.de\r\n// @version      v0.0.1\r\n// @author       Christian Weiske\r\n// @lang         en\r\n// @license      AGPL\r\n// @icon         https://media.ccc.de/favicon-96x96.png\r\n// @package      de.ccc.media\r\n// @type         bangumi\r\n// @webSite      https://api.media.ccc.de\r\n// @description  Video material provided by the Chaos Computer Club\r\n// @nsfw         false\r\n// ==/MiruExtension==\r\n/** @link https://github.com/voc/voctoweb\r\n * @type {string} */\r\nexport default class extends Extension {}\r\n";

    #[test]
    fn reads_the_metadata_block() {
        let header = parse(SCRIPT).unwrap();
        assert_eq!(header.name, "media.ccc.de");
        assert_eq!(header.package, "de.ccc.media");
        assert_eq!(header.version, "v0.0.1");
        assert_eq!(header.kind, MiruType::Bangumi);
        assert_eq!(header.web_site, "https://api.media.ccc.de");
        assert_eq!(header.author.as_deref(), Some("Christian Weiske"));
        assert_eq!(header.license.as_deref(), Some("AGPL"));
        assert!(!header.nsfw);
        assert_eq!(
            header.description.as_deref(),
            Some("Video material provided by the Chaos Computer Club")
        );
        // A JSDoc `@type` after the block is not the script's type.
        assert_eq!(header.kind.provider_kind(), "video.source");
    }

    #[test]
    fn without_a_block_any_line_counts() {
        let loose = "// @name Old\n// @package old.one\n// @version 1.0\n// @type manga\n// @webSite https://old.example\n// @nsfw true\n";
        let header = parse(loose).unwrap();
        assert_eq!(header.package, "old.one");
        assert_eq!(header.kind, MiruType::Manga);
        assert!(header.nsfw);
        assert!(parse("// @name x\n// @type comic\n").is_none());
        assert!(parse("export default class extends Extension {}").is_none());
    }

    #[test]
    fn versions_become_semver() {
        assert_eq!(semver_version("v0.1.2"), "0.1.2");
        assert_eq!(semver_version("1.2"), "1.2.0");
        assert_eq!(semver_version("V3"), "3.0.0");
        assert_eq!(semver_version("v1.0.0-beta.1"), "1.0.0-beta.1");
        assert_eq!(semver_version("2023 build 5"), "0.0.0-2023-build-5");
        assert_eq!(semver_version(""), "0.0.0");
        for version in ["v0.1.2", "1.2", "x y", "1.2.3.4"] {
            assert!(semver::Version::parse(&semver_version(version)).is_ok());
        }
    }
}
