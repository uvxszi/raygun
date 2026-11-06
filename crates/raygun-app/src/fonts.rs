//! The device's own fonts, for the font pickers (Settings → Appearance and Reader).
//!
//! A desktop webview finds installed families by name, so there the list is families alone, read
//! from the system's font folders by fontdb (fontconfig's on Linux). Android's WebView knows few
//! names, so there each family comes with its files, served at the stream proxy's `/font/` route
//! for `@font-face` ([`android`]). Fonts without Latin letters (script fallbacks, symbols, emoji)
//! are left out: none makes an interface or reading font.

use raygun_model::SystemFont;
use raygun_net::FontLinks;

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
mod android;

/// The installed families, read on first use and kept.
pub struct FontService {
    links: FontLinks,
    cache: tokio::sync::Mutex<Option<Vec<SystemFont>>>,
}

impl FontService {
    pub fn new(links: FontLinks) -> Self {
        Self {
            links,
            cache: tokio::sync::Mutex::new(None),
        }
    }

    /// The families by name; read again with `refresh` (fonts installed since).
    pub async fn list(&self, refresh: bool) -> Vec<SystemFont> {
        let mut cache = self.cache.lock().await;
        if refresh || cache.is_none() {
            let links = self.links.clone();
            let fonts = tokio::task::spawn_blocking(move || installed(&links))
                .await
                .unwrap_or_else(|error| {
                    tracing::warn!(%error, "listing fonts failed");
                    Vec::new()
                });
            tracing::info!(families = fonts.len(), "fonts listed");
            *cache = Some(fonts);
        }
        cache.clone().unwrap_or_default()
    }
}

/// Whether a face draws Latin letters, which an interface or a book needs.
fn draws_latin(face: &ttf_parser::Face<'_>) -> bool {
    ['a', 'z', 'A', 'Z']
        .into_iter()
        .all(|letter| face.glyph_index(letter).is_some())
}

#[cfg(target_os = "android")]
fn installed(links: &FontLinks) -> Vec<SystemFont> {
    android::installed(links)
}

#[cfg(target_os = "ios")]
fn installed(_links: &FontLinks) -> Vec<SystemFont> {
    Vec::new()
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn installed(_links: &FontLinks) -> Vec<SystemFont> {
    let mut database = fontdb::Database::new();
    database.load_system_fonts();
    // By lower-case name: the list is in alphabetical order, and names differing in case are one
    // family.
    let mut families = std::collections::BTreeMap::new();
    for face in database.faces() {
        let Some((name, _)) = face.families.first() else {
            continue;
        };
        let name = name.trim();
        let key = name.to_lowercase();
        // macOS keeps its own interface fonts under names starting with a dot.
        if name.is_empty() || name.starts_with('.') || families.contains_key(&key) {
            continue;
        }
        let latin = database
            .with_face_data(face.id, |data, index| {
                ttf_parser::Face::parse(data, index).is_ok_and(|face| draws_latin(&face))
            })
            .unwrap_or(false);
        if latin {
            families.insert(key, name.to_owned());
        }
    }
    families
        .into_values()
        .map(|family| SystemFont {
            family,
            faces: Vec::new(),
        })
        .collect()
}

#[cfg(all(test, not(any(target_os = "android", target_os = "ios"))))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn installed_families_are_named_once_in_order() {
        let proxy = raygun_net::StreamProxy::start(raygun_net::http_client().unwrap(), None)
            .await
            .unwrap();
        let links = proxy.fonts();
        let fonts = tokio::task::spawn_blocking(move || installed(&links))
            .await
            .unwrap();
        let keys: Vec<_> = fonts
            .iter()
            .map(|font| font.family.to_lowercase())
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(keys, sorted);
        assert!(
            fonts
                .iter()
                .all(|font| !font.family.trim().is_empty() && font.faces.is_empty()),
            "desktop webviews take installed families by name"
        );
        #[cfg(windows)]
        assert!(
            fonts.iter().any(|font| font.family == "Arial"),
            "Windows always has Arial"
        );
    }
}
