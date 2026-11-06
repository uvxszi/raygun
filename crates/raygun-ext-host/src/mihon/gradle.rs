//! An extension's build file: Keiyoushi's `build.gradle.kts` (`keiyoushi { name, theme,
//! contentWarning, source { lang, baseUrl } }`) or the older `build.gradle` (`ext { extName,
//! extClass, themePkg, baseUrl, overrideVersionCode, isNsfw }`). Raygun reads what decides how a
//! source converts: its theme and the libraries it builds on. Its sources come from the store.

use std::sync::LazyLock;

use regex::Regex;

/// What a build file says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Build {
    pub name: Option<String>,
    /// The `lib-multisrc` theme it's built on (`madara`).
    pub theme: Option<String>,
    /// The source class (older builds: `.Foo` or the full name).
    pub class: Option<String>,
    pub base_url: Option<String>,
    /// Keiyoushi's libraries it uses (`lib:speedbinb`), and other dependencies.
    pub libraries: Vec<String>,
}

static THEME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*(?:theme|themePkg)\s*=\s*["']([\w.-]+)["']"#).expect("valid regex")
});
static NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*(?:name|extName)\s*=\s*["']([^"'\n]+)["']"#).expect("valid regex")
});
static CLASS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*extClass\s*=\s*["']([\w.]+)["']"#).expect("valid regex")
});
static BASE_URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*baseUrl\s*=\s*["'](https?://[^"'\s]+)["']"#).expect("valid regex")
});
static LIBRARY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"project\(\s*["']:(lib(?:-multisrc)?:[\w-]+)["']\s*\)"#).expect("valid regex")
});
static DEPENDENCY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"implementation\(\s*["']([\w.-]+:[\w.-]+)(?::[^"']*)?["']"#).expect("valid regex")
});

/// Reads a build file (either kind).
pub fn read(text: &str) -> Build {
    let first = |pattern: &Regex| {
        pattern
            .captures(text)
            .map(|found| found[1].trim().to_owned())
            .filter(|value| !value.is_empty())
    };
    let mut libraries: Vec<String> = LIBRARY
        .captures_iter(text)
        .map(|found| found[1].to_owned())
        .collect();
    libraries.extend(
        DEPENDENCY
            .captures_iter(text)
            .map(|found| found[1].to_owned()),
    );
    libraries.sort();
    libraries.dedup();
    Build {
        name: first(&NAME),
        theme: first(&THEME),
        class: first(&CLASS),
        base_url: first(&BASE_URL),
        libraries,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_kinds_of_build_file() {
        let kts = r#"import io.github.keiyoushi.gradle.api.ContentWarning

plugins {
    alias(kei.plugins.extension)
}

keiyoushi {
    name = "Comics Valley"
    versionCode = 2
    contentWarning = ContentWarning.NSFW
    libVersion = "1.6"
    theme = "madara"

    source {
        lang = "all"
        baseUrl = "https://comicsvalley.com"
        id = 1103204227230640533L
    }
}

dependencies {
    implementation(project(":lib:speedbinb"))
}
"#;
        let build = read(kts);
        assert_eq!(build.name.as_deref(), Some("Comics Valley"));
        assert_eq!(build.theme.as_deref(), Some("madara"));
        assert_eq!(build.base_url.as_deref(), Some("https://comicsvalley.com"));
        assert_eq!(build.libraries, ["lib:speedbinb"]);

        let groovy = "ext {\n    extName = 'Old Site'\n    extClass = '.OldSite'\n    themePkg = 'mangathemesia'\n    baseUrl = 'https://old.example'\n    overrideVersionCode = 3\n}\n\napply from: \"$rootDir/common.gradle\"\n";
        let build = read(groovy);
        assert_eq!(build.name.as_deref(), Some("Old Site"));
        assert_eq!(build.class.as_deref(), Some(".OldSite"));
        assert_eq!(build.theme.as_deref(), Some("mangathemesia"));
        assert!(build.libraries.is_empty());
    }
}
