//! What an extension, plugin or add-on from another app's catalogue is for, and whether it gives
//! Raygun anything to call. One that doesn't (a Nuclear mini player, a scrobbler, a Kodi skin, a
//! Mihon extension without sources) is "Not for Raygun": the catalogue says why instead of
//! offering Install, installing it is refused, and one installed before is marked in Installed.
//!
//! Each format's reader decides from what its index says where that's enough (`RepoEntry::
//! not_for_raygun`: Nuclear's categories in `repo`, Kodi's extension points in `kodi`, a Mihon
//! store's sources in `mihon::index`); a Nuclear plugin's own package is read again before it's
//! converted (`nuclear::not_for_raygun`), and an extension that loaded is judged by what it
//! registered ([`installed`]). Stremio addons are judged by their manifests in the app.

use raygun_model::{ExtensionInfo, ExtensionStatus};

use crate::repo::{PackageFormat, RepoEntry};

/// Why a Raygun package that provides nothing (and runs no site scripts) isn't for Raygun.
pub const NOTHING_CALLABLE: &str = "Provides nothing Raygun can call";

/// What an install or an add refused for this reason says.
pub fn message(reason: &str) -> String {
    format!("Not for Raygun: {reason}")
}

/// Marks a Raygun package that would provide nothing: no provider kinds and no site scripts, so
/// nothing it holds could be called. Other formats are marked by their readers.
pub fn mark(entry: &mut RepoEntry) {
    if entry.not_for_raygun.is_none()
        && entry.format == PackageFormat::Raygun
        && entry.provides.is_empty()
        && entry.sites.is_empty()
    {
        entry.not_for_raygun = Some(NOTHING_CALLABLE.to_owned());
    }
}

/// Why an extension that loaded gives Raygun nothing to call: it registered no providers and runs
/// no site scripts. `None` when it gives something, didn't load (its error says why), or waits
/// to be set up. A converted Nuclear plugin whose package names a kind Raygun runs isn't counted
/// out by what it registered: a plugin may register its providers only once its settings are
/// filled in.
pub fn installed(info: &ExtensionInfo) -> Option<String> {
    if info.status != ExtensionStatus::Loaded
        || !info.providers.is_empty()
        || !info.sites.is_empty()
        || info.needs_setup.is_some()
    {
        return None;
    }
    match &info.compatibility {
        Some(report) if !report.supported.is_empty() => None,
        Some(report) => Some(crate::nuclear::purpose(
            &info.name,
            info.description.as_deref(),
            &report.unsupported_categories,
            &[],
        )),
        None => Some(NOTHING_CALLABLE.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use raygun_model::{CompatReport, ExtensionOrigin, ProviderInfo, SetupNeed, SetupReason};

    use super::*;

    fn info() -> ExtensionInfo {
        ExtensionInfo {
            id: "dev.example.nothing".into(),
            name: "Nothing".into(),
            version: "1.0.0".into(),
            description: None,
            content_rating: "safe".into(),
            adult: false,
            origin: ExtensionOrigin::Installed,
            status: ExtensionStatus::Loaded,
            error: None,
            provides: Vec::new(),
            net: Vec::new(),
            providers: Vec::new(),
            settings: Vec::new(),
            compatibility: None,
            icon: None,
            website: None,
            repository: None,
            needs_setup: None,
            sites: Vec::new(),
            not_for_raygun: None,
        }
    }

    #[test]
    fn installed_extensions_that_register_nothing_are_not_for_raygun() {
        assert_eq!(installed(&info()).as_deref(), Some(NOTHING_CALLABLE));
        let provider = ProviderInfo {
            id: "dev.example.nothing/songs".into(),
            extension_id: "dev.example.nothing".into(),
            kind: "music.metadata".into(),
            name: "Songs".into(),
            search_pill: None,
            adult: false,
        };
        let calls = ExtensionInfo {
            providers: vec![provider],
            ..info()
        };
        assert_eq!(installed(&calls), None);
        let scripts = ExtensionInfo {
            sites: vec!["https://*.example.org/*".into()],
            ..info()
        };
        assert_eq!(installed(&scripts), None, "site scripts run in the browser");
        let failed = ExtensionInfo {
            status: ExtensionStatus::Failed,
            ..info()
        };
        assert_eq!(installed(&failed), None, "its error says why");
        let waiting = ExtensionInfo {
            needs_setup: Some(SetupNeed {
                reason: SetupReason::Settings,
                missing: vec!["API key".into()],
                detail: None,
            }),
            ..info()
        };
        assert_eq!(installed(&waiting), None, "it registers once set up");
    }

    #[test]
    fn nuclear_plugins_are_judged_by_their_report() {
        let report = |supported: &[&str], unsupported: &[&str]| CompatReport {
            package: "nuclear-plugin".into(),
            converter: 7,
            supported: supported.iter().map(|kind| (*kind).to_owned()).collect(),
            unsupported_categories: unsupported.iter().map(|kind| (*kind).to_owned()).collect(),
            stubbed_apis: Vec::new(),
            ui_imports: Vec::new(),
        };
        let keyed = ExtensionInfo {
            compatibility: Some(report(&["music.stream"], &[])),
            ..info()
        };
        assert_eq!(installed(&keyed), None, "it may wait for its settings");
        let scrobbler = ExtensionInfo {
            name: "Last.fm Scrobbler".into(),
            compatibility: Some(report(&[], &["scrobbling"])),
            ..info()
        };
        assert_eq!(
            installed(&scrobbler).as_deref(),
            Some("Scrobbles what Nuclear plays to Last.fm; Raygun doesn't scrobble yet")
        );
    }

    #[test]
    fn raygun_packages_that_provide_nothing_are_marked() {
        let entry = |provides: &[&str], sites: &[&str]| RepoEntry {
            id: "dev.example.entry".into(),
            name: "Entry".into(),
            version: "1.0.0".into(),
            api_version: crate::manifest::API_VERSION,
            description: None,
            author: None,
            provides: provides.iter().map(|kind| (*kind).to_owned()).collect(),
            permissions: Default::default(),
            content_rating: "safe".into(),
            lang: None,
            package: "entry.zip".into(),
            icon: None,
            sha256: "0".repeat(64),
            size: 1,
            format: PackageFormat::Raygun,
            sites: sites.iter().map(|site| (*site).to_owned()).collect(),
            not_for_raygun: None,
        };
        let mut empty = entry(&[], &[]);
        mark(&mut empty);
        assert_eq!(empty.not_for_raygun.as_deref(), Some(NOTHING_CALLABLE));
        for mut usable in [
            entry(&["lyrics"], &[]),
            entry(&["site.scripts"], &["https://*.example.org/*"]),
        ] {
            mark(&mut usable);
            assert_eq!(usable.not_for_raygun, None);
        }
        let mut stremio = RepoEntry {
            format: PackageFormat::Stremio,
            ..entry(&[], &[])
        };
        mark(&mut stremio);
        assert_eq!(
            stremio.not_for_raygun, None,
            "an addon is judged by its manifest"
        );
        assert_eq!(
            message("A screensaver for Kodi"),
            "Not for Raygun: A screensaver for Kodi"
        );
    }
}
