//! Mihon stores, conversions, and converted themes against local fixture sites.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::net::SocketAddr;

use axum::Router;
use axum::http::Uri;
use axum::response::Html;
use axum::routing::{get, post};
use reqwest::Url;
use serde_json::{Value, json};

use super::convert::{self, Converted};
use super::index::{self, ContentWarning, Read, StoreExtension, StoreSource};
use super::proto::write;
use super::source::Sources;
use crate::fixture_tests::serve;
use crate::repo::PackageFormat;
use crate::{ExtensionHost, ExtensionSource, HostConfig};

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut packed = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    packed.write_all(bytes).unwrap();
    packed.finish().unwrap()
}

/// A protobuf store shaped like Keiyoushi's `index.pb`.
fn proto_store() -> Vec<u8> {
    let extension = |name: &str,
                     pkg: &str,
                     code: u64,
                     version: &str,
                     warning: u64,
                     sources: &[(u64, &str, &str, &str)]| {
        let mut resources = Vec::new();
        write::text(
            1,
            &format!("https://github.com/keiyoushi/extensions/releases/download/x/{pkg}.apk"),
            &mut resources,
        );
        let (lang, folder) = pkg
            .trim_start_matches(index::PACKAGE_PREFIX)
            .split_once('.')
            .unwrap();
        write::text(
            2,
            &format!(
                "https://cdn.jsdelivr.net/gh/keiyoushi/extensions-source@main/src/{lang}/{folder}/res/mipmap-xhdpi/ic_launcher.png"
            ),
            &mut resources,
        );
        write::text(501, "https://example.org/x.jar", &mut resources);
        let mut out = Vec::new();
        write::text(1, name, &mut out);
        write::text(2, pkg, &mut out);
        write::bytes(3, &resources, &mut out);
        write::text(4, "1.6", &mut out);
        write::number(5, code, &mut out);
        write::text(6, version, &mut out);
        write::number(7, warning, &mut out);
        for (id, name, lang, base) in sources {
            let mut source = Vec::new();
            write::number(1, *id, &mut source);
            write::text(2, name, &mut source);
            write::text(3, lang, &mut source);
            write::text(4, base, &mut source);
            write::bytes(8, &source, &mut out);
        }
        out
    };
    let mut list = Vec::new();
    write::bytes(
        1,
        &extension(
            "Comics Valley",
            "eu.kanade.tachiyomi.extension.all.comicsvalley",
            106_002,
            "1.6.2",
            3,
            &[(
                1_103_204_227_230_640_533,
                "Comics Valley",
                "all",
                "https://comicsvalley.com",
            )],
        ),
        &mut list,
    );
    write::bytes(
        1,
        &extension(
            "Akuma",
            "eu.kanade.tachiyomi.extension.all.akuma",
            106_011,
            "1.6.11",
            1,
            &[
                (
                    1_843_646_695_111_412_994,
                    "Akuma",
                    "en",
                    "https://akuma.moe",
                ),
                (
                    1_707_768_678_643_970_514,
                    "Akuma",
                    "id",
                    "https://www.akuma.moe",
                ),
            ],
        ),
        &mut list,
    );
    write::bytes(
        1,
        &extension(
            "Anime site",
            "eu.kanade.tachiyomi.animeextension.en.anime",
            14,
            "14.4",
            1,
            &[(5, "Anime", "en", "https://anime.example")],
        ),
        &mut list,
    );
    let mut contact = Vec::new();
    write::text(1, "https://keiyoushi.github.io", &mut contact);
    let mut store = Vec::new();
    write::text(1, "Keiyoushi", &mut store);
    write::text(2, "KEI", &mut store);
    write::text(
        3,
        "9add655a78e96c4ec7a53ef89dccb557cb5d767489fac5e785d671a5a75d4da2",
        &mut store,
    );
    write::bytes(4, &contact, &mut store);
    write::bytes(101, &list, &mut store);
    gzip(&store)
}

#[test]
fn reads_a_protobuf_store_as_catalogue_entries() {
    let bytes = proto_store();
    assert!(index::detect(&bytes));
    let at: Url = "https://raw.githubusercontent.com/keiyoushi/extensions/repo/index.pb"
        .parse()
        .unwrap();
    let Read::Store(store) = index::read(&bytes, &at).unwrap() else {
        panic!("a store");
    };
    assert_eq!(store.name, "Keiyoushi");
    assert_eq!(
        store.website.as_deref(),
        Some("https://keiyoushi.github.io")
    );
    assert_eq!(store.extensions.len(), 3);
    let akuma = &store.extensions[1];
    assert_eq!(akuma.version_code, 106_011);
    assert_eq!(akuma.content, ContentWarning::Safe);
    assert_eq!(akuma.sources[0].id, "1843646695111412994");

    let repo = index::to_repo_index(&store).unwrap();
    assert_eq!(repo.name, "Keiyoushi");
    let ids: Vec<&str> = repo
        .extensions
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(
        ids,
        ["mihon.all.comicsvalley", "mihon.all.akuma"],
        "anime extensions are left out"
    );
    let valley = &repo.extensions[0];
    assert_eq!(valley.format, PackageFormat::Mihon);
    assert_eq!(valley.version, "1.6.2");
    assert_eq!(valley.content_rating, "nsfw");
    assert_eq!(valley.lang.as_deref(), Some("multi"));
    assert_eq!(valley.provides, ["manga.source"]);
    assert_eq!(
        valley.permissions.net,
        ["*.comicsvalley.com", "comicsvalley.com"]
    );
    assert!(valley.package.ends_with(".apk"));
    let akuma = &repo.extensions[1];
    assert_eq!(akuma.lang.as_deref(), Some("multi"));
    assert_eq!(
        akuma.permissions.net,
        ["*.akuma.moe", "akuma.moe"],
        "www. and the bare host are one"
    );
    assert_eq!(akuma.content_rating, "safe");
}

#[tokio::test]
async fn follows_a_legacy_list_to_its_newer_index() {
    let list = json!([
        { "name": "Outdated App", "pkg": "eu.kanade.tachiyomi.extension.all.keiyoushi", "apk": "tachiyomi-all.keiyoushi-v1.4.1.apk",
          "lang": "all", "code": 1, "version": "1.4.1", "nsfw": 0,
          "sources": [{ "name": "Outdated App", "lang": "all", "id": "1", "baseUrl": "https://keiyoushi.github.io" }] }
    ])
    .to_string();
    assert!(index::detect(list.as_bytes()));
    let repo_json = json!({
        "index_v2": "https://github.com/keiyoushi/extensions/raw/repo/index.pb",
        "meta": { "name": "Keiyoushi", "website": "https://keiyoushi.github.io", "signingKeyFingerprint": "9add" }
    })
    .to_string();
    let store = proto_store();
    let at: Url = "https://raw.githubusercontent.com/keiyoushi/extensions/repo/index.min.json"
        .parse()
        .unwrap();
    let asked = std::sync::Mutex::new(Vec::new());
    let (from, loaded) = index::load(&at, list.into_bytes(), |url: Url| {
        asked.lock().unwrap().push(url.to_string());
        let answer = match url.as_str() {
            "https://raw.githubusercontent.com/keiyoushi/extensions/repo/repo.json" => {
                Ok(repo_json.clone().into_bytes())
            }
            "https://github.com/keiyoushi/extensions/raw/repo/index.pb" => Ok(store.clone()),
            other => Err(format!("{other} answered HTTP 404")),
        };
        async move { answer }
    })
    .await
    .unwrap();
    assert_eq!(
        from.as_str(),
        "https://github.com/keiyoushi/extensions/raw/repo/index.pb"
    );
    assert_eq!(loaded.name, "Keiyoushi");
    assert_eq!(
        loaded.extensions.len(),
        3,
        "the newer index replaces the list"
    );
    assert_eq!(asked.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn legacy_lists_without_a_newer_index_are_stores_of_their_own() {
    let list = json!([
        { "name": "Tachiyomi: Example", "pkg": "eu.kanade.tachiyomi.extension.en.example", "apk": "tachiyomi-en.example-v1.4.12.apk",
          "lang": "en", "code": 12, "version": "1.4.12", "nsfw": 1,
          "sources": [{ "name": "Example", "lang": "en", "id": 4_611_686_018_427_387_904_u64, "baseUrl": "https://example.org" }] }
    ])
    .to_string();
    let at: Url = "https://raw.githubusercontent.com/someone/extensions/repo/index.min.json"
        .parse()
        .unwrap();
    let (_, store) = index::load(&at, list.into_bytes(), |url: Url| {
        let answer = Err(format!("{url} answered HTTP 404"));
        async move { answer }
    })
    .await
    .unwrap();
    assert_eq!(
        store.name, "someone",
        "a generic project is named by its owner"
    );
    let example = &store.extensions[0];
    assert_eq!(
        example.apk,
        "https://raw.githubusercontent.com/someone/extensions/repo/apk/tachiyomi-en.example-v1.4.12.apk"
    );
    assert_eq!(
        example.icon.as_deref(),
        Some(
            "https://raw.githubusercontent.com/someone/extensions/repo/icon/eu.kanade.tachiyomi.extension.en.example.png"
        )
    );
    assert_eq!(example.lib_version, "1.4");
    assert_eq!(example.content, ContentWarning::Nsfw);
    assert_eq!(
        example.sources[0].id, "4611686018427387904",
        "big ids stay exact"
    );
    let repo = index::to_repo_index(&store).unwrap();
    assert_eq!(repo.extensions[0].name, "Example");
    assert_eq!(repo.extensions[0].version, "1.4.12");
    // Its code is in the store repository's `-source` twin.
    let folder = super::source::locate(example, &at).unwrap();
    assert_eq!(
        folder.web_url(),
        "https://github.com/someone/extensions-source/tree/main/src/en/example"
    );
}

#[test]
fn a_list_read_without_its_address_keeps_its_files_relative() {
    let list = json!([
        { "name": "Tachiyomi: MangaDex", "pkg": "eu.kanade.tachiyomi.extension.all.mangadex",
          "apk": "tachiyomi-all.mangadex-v1.4.190.apk", "lang": "all", "code": 190,
          "version": "1.4.190", "nsfw": 1,
          "sources": [
            { "name": "MangaDex", "lang": "en", "id": "1", "baseUrl": "https://mangadex.org" },
            { "name": "MangaDex", "lang": "ja", "id": "2", "baseUrl": "https://mangadex.org" },
          ] },
        { "name": "Aniyomi: AllAnime", "pkg": "eu.kanade.tachiyomi.animeextension.en.allanime",
          "apk": "aniyomi-en.allanime-v14.7.apk", "lang": "en", "code": 7, "version": "14.7",
          "nsfw": 0, "sources": [{ "name": "AllAnime", "lang": "en", "id": "3", "baseUrl": "https://allanime.to" }] },
        { "name": "Bad", "pkg": "eu.kanade.tachiyomi.extension.en.bad", "apk": "../x.apk", "sources": [] },
    ])
    .to_string();
    let parsed = super::parse(&list).unwrap();
    let [mangadex] = parsed.extensions.as_slice() else {
        panic!("{:?}", parsed.extensions);
    };
    assert_eq!(mangadex.id, "mihon.all.mangadex");
    assert_eq!(mangadex.name, "MangaDex");
    assert_eq!(mangadex.package, "apk/tachiyomi-all.mangadex-v1.4.190.apk");
    assert_eq!(
        mangadex.icon.as_deref(),
        Some("icon/eu.kanade.tachiyomi.extension.all.mangadex.png")
    );
    assert_eq!(mangadex.content_rating, "nsfw");
    assert_eq!(mangadex.lang.as_deref(), Some("multi"));
    assert_eq!(
        super::extension_id("eu.kanade.tachiyomi.animeextension.en.allanime").as_deref(),
        Some("mihon.anime.en.allanime")
    );
    let anime_only =
        json!([{ "name": "AllAnime", "pkg": "eu.kanade.tachiyomi.animeextension.en.allanime",
        "apk": "a.apk", "sources": [] }])
        .to_string();
    assert!(
        super::parse(&anime_only).is_none(),
        "no comic sources to list"
    );
    assert!(super::parse(r#"[{"id":"x","site":"y","version":"1","url":"z"}]"#).is_none());
    assert!(super::runtime_present());
}

#[test]
fn tells_stores_from_other_indexes() {
    assert!(!index::detect(
        br#"{"formatVersion":1,"name":"Paraline","extensions":[]}"#
    ));
    assert!(!index::detect(
        br#"[{"id":"royalroad","site":"x","version":"1.0.0","url":"y"}]"#
    ));
    assert!(index::detect(
        br#"{"meta":{"name":"K","signingKeyFingerprint":"x"}}"#
    ));
    let json_store = json!({
        "name": "Store", "badgeLabel": "S", "signingKey": "ab",
        "contact": { "website": "https://store.example" },
        "extensionList": { "extensions": [{
            "name": "Site", "packageName": "eu.kanade.tachiyomi.extension.en.site",
            "resources": { "apkUrl": "apk/site.apk", "iconUrl": "icon/site.png" },
            "extensionLib": "1.6", "versionCode": 106_003, "versionName": "1.6.3",
            "contentWarning": "CONTENT_WARNING_MIXED",
            "sources": [{ "id": 7, "name": "Site", "language": "en", "homeUrl": "https://site.example" }]
        }]}
    })
    .to_string();
    assert!(index::detect(json_store.as_bytes()));
    let at: Url = "https://store.example/index.json".parse().unwrap();
    let Read::Store(store) = index::read(json_store.as_bytes(), &at).unwrap() else {
        panic!("a store");
    };
    assert_eq!(
        store.extensions[0].apk,
        "https://store.example/apk/site.apk"
    );
    assert_eq!(store.extensions[0].content, ContentWarning::Mixed);
    assert_eq!(
        store.extensions[0].sources[0].base_url,
        "https://site.example"
    );
}

fn extension(pkg: &str, base: &str) -> StoreExtension {
    StoreExtension {
        name: "Fixture Comics".into(),
        pkg: pkg.into(),
        apk: "https://example.org/x.apk".into(),
        icon: None,
        lib_version: "1.6".into(),
        version_code: 106_004,
        version_name: "1.6.4".into(),
        content: ContentWarning::Safe,
        sources: vec![StoreSource {
            id: "4242".into(),
            name: "Fixture Comics".into(),
            lang: "en".into(),
            base_url: base.into(),
        }],
    }
}

fn sources(build: &str, files: &[(&str, &str)]) -> Sources {
    let mut map = BTreeMap::new();
    map.insert("build.gradle.kts".to_owned(), build.to_owned());
    for (path, text) in files {
        map.insert((*path).to_owned(), (*text).to_owned());
    }
    Sources {
        files: map,
        ..Sources::default()
    }
}

const MADARA_BUILD: &str = "keiyoushi {\n    name = \"Fixture Comics\"\n    versionCode = 4\n    libVersion = \"1.6\"\n    theme = \"madara\"\n    source {\n        lang = \"en\"\n        baseUrl = \"https://fixture.example\"\n    }\n}\n";

const MADARA_SOURCE: &str = r#"package eu.kanade.tachiyomi.extension.en.fixturecomics

import eu.kanade.tachiyomi.multisrc.madara.Madara
import keiyoushi.annotation.Source
import java.time.format.DateTimeFormatter
import java.util.Locale

@Source
abstract class FixtureComics : Madara() {
    override val chapterMode = ChapterMode.MangaAjax
    override val mangaSubString = "comics"
    override val chapterDateFormat = DateTimeFormatter.ofPattern(
        "dd/MM/yyyy",
        Locale.ROOT,
    )
    override val mangaDetailsSelectorStatus = super.mangaDetailsSelectorStatus + ", .status-x"
    override fun OkHttpClient.Builder.configureClient() = rateLimit(2)
    override fun Headers.Builder.configureHeaders() = add("Origin", baseUrl)

    private fun helper() = "unused"

    companion object {
        private const val UNUSED = 1
    }
}
"#;

#[test]
fn converts_a_themed_extension_into_its_values() {
    let extension = extension(
        "eu.kanade.tachiyomi.extension.en.fixturecomics",
        "https://fixture.example",
    );
    let converted = convert::convert(
        &extension,
        &sources(
            MADARA_BUILD,
            &[(
                "src/eu/kanade/tachiyomi/extension/en/fixturecomics/FixtureComics.kt",
                MADARA_SOURCE,
            )],
        ),
    )
    .unwrap();
    assert_eq!(converted.theme.as_deref(), Some("madara"));
    let code = &converted.code;
    assert!(code.contains("mihon.theme(\"madara\""));
    assert!(code.contains("variant: \"ajax\""));
    assert!(code.contains("chapterMode: (t) => \"MangaAjax\""));
    assert!(
        code.contains("chapterDateFormat: (t) => K.dateFormat(\"dd/MM/yyyy\", K.locale(\"root\"))")
    );
    assert!(code.contains("t.base.mangaDetailsSelectorStatus"));
    assert!(code.contains("rateLimit: { permits: 2, periodMs: 1000 }"));
    assert!(code.contains("[\"Origin\", t.baseUrl]"));

    let package = convert::package(
        &extension,
        &converted,
        "Keiyoushi",
        Some("https://github.com/x"),
    )
    .unwrap();
    let manifest = &package.manifest;
    assert_eq!(manifest.id, "mihon.en.fixturecomics");
    assert_eq!(manifest.runtime, crate::manifest::Runtime::Mihon);
    assert_eq!(manifest.version.to_string(), "1.6.4");
    assert_eq!(
        manifest.mihon.as_ref().unwrap()["converter"],
        json!(convert::CONVERTER_VERSION)
    );
    assert_eq!(manifest.website.as_deref(), Some("https://fixture.example"));
}

#[test]
fn refuses_what_it_cant_convert_with_the_reason() {
    let extension = extension(
        "eu.kanade.tachiyomi.extension.en.fixturecomics",
        "https://fixture.example",
    );
    let reason = |build: &str, source: &str| {
        convert::convert(
            &extension,
            &sources(
                build,
                &[(
                    "src/eu/kanade/tachiyomi/extension/en/fixturecomics/FixtureComics.kt",
                    source,
                )],
            ),
        )
        .unwrap_err()
    };
    let own_code = MADARA_SOURCE.replace(
        "private fun helper() = \"unused\"",
        "override fun chapterFromElement(element: Element, mangaPath: String): SChapter? = super.chapterFromElement(element, mangaPath)?.apply { name = element.text() }",
    );
    assert!(reason(MADARA_BUILD, &own_code).contains("chapterFromElement"));
    let interceptor = MADARA_SOURCE.replace("rateLimit(2)", "addInterceptor(::descramble)");
    assert!(reason(MADARA_BUILD, &interceptor).contains("interceptor"));
    let library = format!(
        "{MADARA_BUILD}\ndependencies {{\n    implementation(project(\":lib:speedbinb\"))\n}}\n"
    );
    assert!(reason(&library, MADARA_SOURCE).contains("lib:speedbinb"));
    assert!(
        reason(
            &MADARA_BUILD.replace("madara", "comiciviewer"),
            MADARA_SOURCE
        )
        .contains("comiciviewer")
    );
    let starts = MADARA_SOURCE.replace(
        "private fun helper() = \"unused\"",
        "init { println(\"hi\") }",
    );
    assert!(reason(MADARA_BUILD, &starts).contains("starts"));
}

/// A converted extension with its sources pointed at `address`, loaded in a host of its own.
async fn load_converted(
    converted: &Converted,
    extension: &StoreExtension,
    address: SocketAddr,
) -> ExtensionHost {
    let base = format!("http://{address}");
    let mut local = extension.clone();
    for source in &mut local.sources {
        source.base_url.clone_from(&base);
    }
    let code = converted
        .code
        .replace(&extension.sources[0].base_url, &base);
    let package = convert::package(
        &local,
        &Converted {
            code,
            ..converted.clone()
        },
        "Fixture",
        None,
    )
    .unwrap();
    let mut files: Vec<(String, String)> = package
        .files()
        .iter()
        .map(|(name, bytes)| (name.clone(), String::from_utf8_lossy(bytes).into_owned()))
        .collect();
    let mut manifest: Value = serde_json::from_str(&files[0].1).unwrap();
    manifest["permissions"]["net"] = json!(["127.0.0.1"]);
    files[0].1 = manifest.to_string();
    let borrowed: Vec<(&str, &str)> = files
        .iter()
        .map(|(name, text)| (name.as_str(), text.as_str()))
        .collect();
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host.load(vec![ExtensionSource::built_in(&borrowed)]).await;
    assert_eq!(infos[0].error, None, "{infos:?}");
    assert_eq!(infos[0].providers.len(), 1);
    host
}

const MADARA_ARCHIVE: &str = r#"<div class="page-item-detail manga">
  <div class="item-thumb" data-post-id="11"><a href="/comics/first-one/"><img data-src="/covers/first.jpg" alt=""></a></div>
  <div class="item-summary"><div class="post-title font-title"><h3 class="h5"><a href="/comics/first-one/">First One</a></h3></div></div>
</div>
<div class="page-item-detail manga">
  <div class="item-thumb" data-post-id="12"><img srcset="/covers/second-110.jpg 110w, /covers/second-350.jpg 350w"></div>
  <div class="post-title"><h3><a href="/comics/second/">Second</a></h3></div>
</div>
<div class="page-item-detail ad"><div class="post-title"><h3><a href="/ads/">Not a comic</a></h3></div></div>"#;

const MADARA_TITLE: &str = r##"<html><head><link rel="shortlink" href="https://x/?p=11"></head><body>
<div class="post-title"><h1><span class="manga-title-badges hot">HOT</span> First One</h1></div>
<div class="summary_image"><a href="#"><img data-lazy-src="/covers/first-big.jpg"></a></div>
<div class="author-content"><a href="/author/a/">Ana</a><a href="/author/u/">Updating</a></div>
<div class="artist-content"><a href="/artist/b/">Bo</a></div>
<div class="genres-content"><a href="/manga-genre/action/">Action</a><a href="/manga-genre/drama/">Drama</a></div>
<div class="post-status"><div class="post-content_item"><div class="summary-heading"><h5>Status</h5></div><div class="summary-content">OnGoing</div></div></div>
<div class="description-summary"><div class="summary__content"><p>Line one.</p><p>Line two.</p></div></div>
</body></html>"##;

const MADARA_CHAPTERS: &str = r##"<ul class="main version-chap">
<li class="wp-manga-chapter"><a href="/comics/first-one/chapter-2/">Chapter 2 </a><span class="chapter-release-date"><i>05/01/2024</i></span></li>
<li class="wp-manga-chapter"><a href="/comics/first-one/chapter-1/">Chapter 1</a><span class="chapter-release-date"><a href="#" title="2 days ago">new</a></span></li>
</ul>"##;

const MADARA_READER: &str = r#"<html><body><div class="reading-content">
<div class="page-break no-gaps"><img id="image-0" data-src=" /pages/1.jpg " class="wp-manga-chapter-img"></div>
<div class="page-break no-gaps"><img id="image-1" src="https://cdn.example/pages/2.jpg" class="wp-manga-chapter-img"></div>
</div></body></html>"#;

#[tokio::test]
async fn a_converted_madara_source_reads_a_madara_site() {
    let app = Router::new()
        .route(
            "/wp-admin/admin-ajax.php",
            post(|body: String| async move {
                assert!(body.contains("action=madara_load_more"), "{body}");
                assert!(
                    body.contains("vars%5Bmeta_key%5D=_wp_manga_views"),
                    "popular sorts by views: {body}"
                );
                Html(MADARA_ARCHIVE)
            }),
        )
        .route("/comics/first-one/", get(|| async { Html(MADARA_TITLE) }))
        .route(
            "/comics/first-one/ajax/chapters/",
            post(|headers: axum::http::HeaderMap| async move {
                assert_eq!(headers["x-requested-with"], "XMLHttpRequest");
                Html(MADARA_CHAPTERS)
            }),
        )
        .route(
            "/comics/first-one/chapter-2/",
            get(|uri: Uri| async move {
                assert!(uri.query().is_none());
                Html(MADARA_READER)
            }),
        );
    let address = serve(app);
    let extension = extension(
        "eu.kanade.tachiyomi.extension.en.fixturecomics",
        "https://fixture.example",
    );
    let converted = convert::convert(
        &extension,
        &sources(
            MADARA_BUILD,
            &[(
                "src/eu/kanade/tachiyomi/extension/en/fixturecomics/FixtureComics.kt",
                MADARA_SOURCE,
            )],
        ),
    )
    .unwrap();
    let host = load_converted(&converted, &extension, address).await;
    let provider = "mihon.en.fixturecomics/4242";

    let popular = host
        .manga_list(provider, crate::manga::MangaList::Popular, 1)
        .await
        .unwrap();
    let titles: Vec<&str> = popular
        .items
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(
        titles,
        ["First One", "Second"],
        "items without a post id are left out"
    );
    assert_eq!(popular.items[0].id, "/comics/first-one/");
    assert_eq!(
        popular.items[0].cover_url.as_deref(),
        Some(format!("http://{address}/covers/first.jpg").as_str())
    );
    assert_eq!(
        popular.items[1].cover_url.as_deref(),
        Some(format!("http://{address}/covers/second-350.jpg").as_str())
    );
    assert!(!popular.has_next_page);

    let details = host
        .manga_details(provider, "/comics/first-one/")
        .await
        .unwrap();
    assert_eq!(details.title, "First One");
    assert_eq!(details.authors, ["Ana"], "placeholders are left out");
    assert_eq!(details.artists, ["Bo"]);
    assert_eq!(details.genres, ["Action", "Drama"]);
    assert_eq!(details.status, "ongoing");
    assert_eq!(
        details.description.as_deref(),
        Some("Line one.\n\nLine two.")
    );
    assert_eq!(
        details.cover_url.as_deref(),
        Some(format!("http://{address}/covers/first-big.jpg").as_str())
    );

    let chapters = host
        .manga_chapters(provider, "/comics/first-one/")
        .await
        .unwrap();
    assert_eq!(chapters.len(), 2);
    assert_eq!(chapters[0].id, "/comics/first-one/chapter-2/");
    assert_eq!(chapters[0].name, "Chapter 2");
    assert_eq!(chapters[0].number, Some(2.0));
    assert_eq!(
        chapters[0].date_upload,
        Some(1_704_412_800_000),
        "05/01/2024 as dd/MM/yyyy"
    );
    let two_days = chapters[1].date_upload.unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    assert!(
        (now - two_days - 2 * 86_400_000).abs() < 60_000,
        "2 days ago"
    );

    let pages = host
        .manga_pages(provider, "/comics/first-one/chapter-2/")
        .await
        .unwrap();
    let urls: Vec<&str> = pages.iter().map(|page| page.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            format!("http://{address}/pages/1.jpg").as_str(),
            "https://cdn.example/pages/2.jpg"
        ]
    );
    assert_eq!(
        pages[0].headers["Referer"],
        format!("http://{address}/comics/first-one/chapter-2/")
    );
}

const THEMESIA_BUILD: &str = "keiyoushi {\n    name = \"Fixture Scans\"\n    versionCode = 1\n    libVersion = \"1.6\"\n    theme = \"mangathemesia\"\n}\n";

const THEMESIA_SOURCE: &str = r#"package eu.kanade.tachiyomi.extension.en.fixturescans

import eu.kanade.tachiyomi.multisrc.mangathemesia.MangaThemesia

@Source
abstract class FixtureScans : MangaThemesia() {
    override val mangaUrlDirectory = "/series"
    override val datePattern = "MMM d, yyyy"
}
"#;

const THEMESIA_LIST: &str = r#"<html><body><div class="listupd">
<div class="bs"><div class="bsx"><a href="https://HOST/series/alpha/" title="Alpha"><div class="limit"><img src="https://HOST/covers/alpha.webp"></div></a></div></div>
<div class="bs"><div class="bsx"><a href="/series/beta/" title="Beta"><img data-src="/covers/beta.webp"></a></div></div>
</div><div class="hpage"><a class="r" href="?page=2">Next</a></div></body></html>"#;

const THEMESIA_TITLE: &str = r#"<html><body><div class="bigcontent">
<div class="thumb"><img src="/covers/alpha-big.webp"></div>
<h1 class="entry-title">Alpha</h1>
<div class="tsinfo"><div class="imptdt">Status <i>Completed</i></div><div class="imptdt">Author <i>Kei</i></div><div class="imptdt">Type <a href="/series/?type=manhwa">Manhwa</a></div></div>
<div class="wd-full"><span class="mgen"><a href="/genres/action/">ACTION</a><a href="/genres/romance/">romance</a></span></div>
<div class="entry-content entry-content-single" itemprop="description"><p>A story.</p></div>
</div>
<div class="eplister" id="chapterlist"><ul>
<li data-num="2"><div class="chbox"><div class="eph-num"><a href="https://HOST/alpha-chapter-2/"><span class="chapternum">Chapter 2</span><span class="chapterdate">Jan 5, 2024</span></a></div></div></li>
<li data-num="1"><div class="chbox"><div class="eph-num"><a href="/alpha-chapter-1/"><span class="chapternum">Chapter 1</span><span class="chapterdate">Dec 31, 2023</span></a></div></div></li>
</ul></div></body></html>"#;

const THEMESIA_READER: &str = r#"<html><body><div id="readerarea"></div>
<script>ts_reader.run({"prevUrl":"","nextUrl":"","sources":[{"source":"Server 1","images":["https:\/\/cdn.example\/a\/1.jpg","\/a\/2.jpg"]}]});</script>
</body></html>"#;

#[tokio::test]
async fn a_converted_mangathemesia_source_reads_its_site() {
    let app = Router::new()
        .route(
            "/series/",
            get(|uri: Uri| async move {
                let query = uri.query().unwrap_or_default().to_owned();
                assert!(
                    query.contains("order=popular") && query.contains("page=1"),
                    "{query}"
                );
                Html(THEMESIA_LIST.replace("HOST", "127.0.0.1"))
            }),
        )
        .route(
            "/series/alpha/",
            get(|| async { Html(THEMESIA_TITLE.replace("HOST", "127.0.0.1")) }),
        )
        .route("/alpha-chapter-2/", get(|| async { Html(THEMESIA_READER) }));
    let address = serve(app);
    let extension = extension(
        "eu.kanade.tachiyomi.extension.en.fixturescans",
        "https://fixture.example",
    );
    let converted = convert::convert(
        &extension,
        &sources(
            THEMESIA_BUILD,
            &[(
                "src/eu/kanade/tachiyomi/extension/en/fixturescans/FixtureScans.kt",
                THEMESIA_SOURCE,
            )],
        ),
    )
    .unwrap();
    let host = load_converted(&converted, &extension, address).await;
    let provider = "mihon.en.fixturescans/4242";

    let popular = host
        .manga_list(provider, crate::manga::MangaList::Popular, 1)
        .await
        .unwrap();
    let ids: Vec<&str> = popular.items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, ["/series/alpha/", "/series/beta/"]);
    assert_eq!(popular.items[1].title, "Beta");
    assert!(popular.has_next_page);

    let details = host
        .manga_details(provider, "/series/alpha/")
        .await
        .unwrap();
    assert_eq!(details.title, "Alpha");
    assert_eq!(details.status, "completed");
    assert_eq!(details.authors, ["Kei"]);
    assert_eq!(details.genres, ["Action", "Romance", "Manhwa"]);
    assert_eq!(details.description.as_deref(), Some("A story."));

    let chapters = host
        .manga_chapters(provider, "/series/alpha/")
        .await
        .unwrap();
    assert_eq!(chapters.len(), 2);
    assert_eq!(chapters[0].id, "/alpha-chapter-2/");
    assert_eq!(chapters[0].number, Some(2.0));
    assert_eq!(
        chapters[0].date_upload,
        Some(1_704_412_800_000),
        "Jan 5, 2024"
    );

    let pages = host
        .manga_pages(provider, "/alpha-chapter-2/")
        .await
        .unwrap();
    let urls: Vec<&str> = pages.iter().map(|page| page.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "https://cdn.example/a/1.jpg",
            format!("http://{address}/a/2.jpg").as_str()
        ]
    );
}

const STANDALONE_BUILD: &str = "keiyoushi {\n    name = \"Fixture Stand\"\n    versionCode = 3\n    libVersion = \"1.6\"\n    source {\n        lang = \"en\"\n        baseUrl = \"https://stand.example\"\n    }\n}\n";

const STANDALONE_SOURCE: &str = r##"package eu.kanade.tachiyomi.extension.en.fixturestand

import eu.kanade.tachiyomi.source.model.FilterList
import keiyoushi.source.KeiSource
import java.text.SimpleDateFormat
import java.util.Locale

@Source
abstract class FixtureStand : KeiSource() {
    override val supportsLatest = false

    override fun Headers.Builder.configureHeaders() = add("X-Fixture", "1")

    override fun OkHttpClient.Builder.configureClient() = rateLimit(4)

    override suspend fun getPopularManga(page: Int) = getSearchMangaList(page, "", FilterList())

    override suspend fun getLatestUpdates(page: Int): MangasPage = throw UnsupportedOperationException()

    override suspend fun getSearchMangaList(page: Int, query: String, filters: FilterList): MangasPage {
        val url = "$baseUrl/list/page/$page".toHttpUrl().newBuilder()
            .addQueryParameter("s", query)
            .build()
        val document = client.get(url, Fetch.DOCUMENT).asJsoup()
        val mangas = document.select(".listing .item a").map { element ->
            SManga.create().apply {
                title = element.selectFirst("h2")!!.text()
                thumbnail_url = element.selectFirst("img")?.absUrl("src")
                setUrlWithoutDomain(element.absUrl("href"))
            }
        }
        return MangasPage(mangas, hasNextPage = document.selectFirst(".next.page-numbers") != null)
    }

    override suspend fun fetchMangaUpdate(
        manga: SManga,
        chapters: List<SChapter>,
        fetchDetails: Boolean,
        fetchChapters: Boolean,
    ): SMangaUpdate {
        val document = client.get(getMangaUrl(manga), Fetch.DOCUMENT).asJsoup()
        val updatedManga = SManga.create().apply {
            title = document.selectFirst("h1")!!.text()
                .substringAfter("Read").substringBeforeLast("Online")
                .trim()
            thumbnail_url = document.selectFirst(".cover img")?.absUrl("src")
            description = document.select(".text p").joinToString("\n") { it.text() }
            genre = document.select(".infos a").joinToString { it.text() }
            status = when (document.selectFirst(".status")?.text()?.lowercase()) {
                "ongoing" -> SManga.ONGOING
                "completed" -> SManga.COMPLETED
                else -> SManga.UNKNOWN
            }
            setUrlWithoutDomain(document.location())
        }
        val updatedChapters = document.select(".chapters a").mapIndexed { index, element ->
            SChapter.create().apply {
                name = element.text()
                chapter_number = (index + 1).toFloat()
                date_upload = DATE_FORMAT.tryParse(element.attr("data-date"))
                setUrlWithoutDomain(element.absUrl("href"))
            }
        }.reversed()
        return SMangaUpdate(updatedManga, updatedChapters)
    }

    override suspend fun getPageList(chapter: SChapter): List<Page> = client.get(getChapterUrl(chapter), Fetch.DOCUMENT).asJsoup()
        .select("#images img")
        .filterNot { it.attr("src").isBlank() }
        .mapIndexed { index, element -> Page(index, imageUrl = element.absUrl("src")) }

    companion object {
        private val DATE_FORMAT = SimpleDateFormat("yyyy-MM-dd", Locale.ROOT)
    }
}
"##;

#[tokio::test]
async fn a_source_written_without_a_theme_is_converted_from_its_code() {
    let app = Router::new()
        .route(
            "/list/page/1",
            get(|uri: Uri, headers: axum::http::HeaderMap| async move {
                assert_eq!(headers["x-fixture"], "1", "its own headers");
                assert_eq!(uri.query(), Some("s="));
                Html(
                    r#"<div class="listing"><div class="item"><a href="/manga/one"><img src="/c/one.jpg"><h2>One Piece of Code</h2></a></div>
                    <div class="item"><a href="/manga/two"><h2>Two</h2></a></div></div><a class="next page-numbers" href="/list/page/2">Next</a>"#,
                )
            }),
        )
        .route(
            "/manga/one",
            get(|| async {
                Html(
                    r#"<html><body><h1>Read One Piece of Code Online</h1><div class="cover"><img src="/c/one-big.jpg"></div>
                    <div class="text"><p>First.</p><p>Second.</p></div><div class="infos"><a>Action</a><a>Comedy</a></div>
                    <span class="status">Ongoing</span>
                    <div class="chapters"><a href="/manga/one/2" data-date="2024-01-05">Chapter 2</a><a href="/manga/one/1" data-date="2023-12-31">Chapter 1</a></div>
                    </body></html>"#,
                )
            }),
        )
        .route(
            "/manga/one/2",
            get(|| async {
                Html(r#"<div id="images"><img src="/p/1.jpg"><img src=""><img src="https://cdn.example/p/2.jpg"></div>"#)
            }),
        );
    let address = serve(app);
    let extension = extension(
        "eu.kanade.tachiyomi.extension.en.fixturestand",
        "https://stand.example",
    );
    let converted = convert::convert(
        &extension,
        &sources(
            STANDALONE_BUILD,
            &[(
                "src/eu/kanade/tachiyomi/extension/en/fixturestand/FixtureStand.kt",
                STANDALONE_SOURCE,
            )],
        ),
    )
    .unwrap();
    assert_eq!(converted.theme, None);
    let host = load_converted(&converted, &extension, address).await;
    let provider = "mihon.en.fixturestand/4242";

    let popular = host
        .manga_list(provider, crate::manga::MangaList::Popular, 1)
        .await
        .unwrap();
    let titles: Vec<&str> = popular
        .items
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(titles, ["One Piece of Code", "Two"]);
    assert_eq!(popular.items[0].id, "/manga/one");
    assert_eq!(
        popular.items[0].cover_url.as_deref(),
        Some(format!("http://{address}/c/one.jpg").as_str())
    );
    assert!(popular.has_next_page);
    let latest = host
        .manga_list(provider, crate::manga::MangaList::Latest, 1)
        .await;
    assert!(latest.is_err(), "it has no latest titles");

    let details = host.manga_details(provider, "/manga/one").await.unwrap();
    assert_eq!(details.title, "One Piece of Code");
    assert_eq!(details.description.as_deref(), Some("First.\nSecond."));
    assert_eq!(details.genres, ["Action", "Comedy"]);
    assert_eq!(details.status, "ongoing");

    let chapters = host.manga_chapters(provider, "/manga/one").await.unwrap();
    let ids: Vec<&str> = chapters.iter().map(|chapter| chapter.id.as_str()).collect();
    assert_eq!(
        ids,
        ["/manga/one/1", "/manga/one/2"],
        "reversed as the source does"
    );
    assert_eq!(chapters[1].number, Some(1.0));
    assert_eq!(chapters[1].date_upload, Some(1_704_412_800_000));

    let pages = host.manga_pages(provider, "/manga/one/2").await.unwrap();
    let urls: Vec<&str> = pages.iter().map(|page| page.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            format!("http://{address}/p/1.jpg").as_str(),
            "https://cdn.example/p/2.jpg"
        ]
    );
    assert_eq!(pages[0].headers["Referer"], format!("http://{address}/"));
}

const NEXT_BUILD: &str = "keiyoushi {\n    name = \"Fixture Next\"\n    versionCode = 1\n    libVersion = \"1.6\"\n    source {\n        lang = \"en\"\n        baseUrl = \"https://next.example\"\n    }\n}\n";

const NEXT_SOURCE: &str = r##"package eu.kanade.tachiyomi.extension.en.fixturenext

import eu.kanade.tachiyomi.source.model.FilterList
import keiyoushi.source.KeiSource
import keiyoushi.utils.extractNextJs
import keiyoushi.utils.extractNextJsRsc
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonNames
import kotlinx.serialization.json.JsonObject

@Source
abstract class FixtureNext : KeiSource() {
    override val supportsLatest = false

    override suspend fun getPopularManga(page: Int): MangasPage {
        val series = client.get("$baseUrl/list", Fetch.DOCUMENT).asJsoup().extractNextJs<List<SeriesDto>>().orEmpty()
        return MangasPage(series.map { it.toSManga() }, false)
    }

    override suspend fun getLatestUpdates(page: Int): MangasPage = throw UnsupportedOperationException()

    override suspend fun getSearchMangaList(page: Int, query: String, filters: FilterList): MangasPage {
        val document = client.get("$baseUrl/search?q=$query", Fetch.DOCUMENT).asJsoup()
        val found = document.extractNextJs<SearchDto> { it is JsonObject && "results" in it }!!
        return MangasPage(found.entries.map { it.toSManga() }, found.hasNext)
    }

    override suspend fun fetchMangaUpdate(
        manga: SManga,
        chapters: List<SChapter>,
        fetchDetails: Boolean,
        fetchChapters: Boolean,
    ): SMangaUpdate {
        val data = client.get(getMangaUrl(manga), Fetch.XHR).extractNextJs<DetailsData>()!!
        val path = manga.url
        val updated = SManga.create().apply {
            title = data.manga.title
            description = data.manga.description
            genre = data.manga.genres.joinToString()
            url = path
        }
        val list = data.manga.chapters.map { chapter ->
            SChapter.create().apply {
                name = chapter.name
                url = "/read/" + chapter.id
            }
        }
        return SMangaUpdate(updated, list)
    }

    override suspend fun getPageList(chapter: SChapter): List<Page> {
        val body = client.get(getChapterUrl(chapter), Fetch.XHR).body.string()
        val dto = body.extractNextJsRsc<PagesDto>()!!
        return dto.images.values.mapIndexed { index, image -> Page(index, imageUrl = baseUrl + image) }
    }
}

@Serializable
class SeriesDto(
    val id: String,
    val title: String,
    val cover: String? = null,
) {
    fun toSManga(): SManga {
        val path = "/series/$id"
        val name = title
        val image = cover
        return SManga.create().apply {
            url = path
            title = name
            thumbnail_url = image
        }
    }
}

@Serializable
class SearchDto(
    @JsonNames("results") val entries: List<SeriesDto> = emptyList(),
    @SerialName("has_next") val hasNext: Boolean = false,
)

@Serializable
class DetailsData(val manga: MangaDto)

@Serializable
class MangaDto(
    val title: String,
    val description: String? = null,
    val genres: List<String> = emptyList(),
    val chapters: List<ChapterDto> = emptyList(),
)

@Serializable
class ChapterDto(val id: String, val name: String)

@Serializable
class PagesDto(val images: Map<String, String>, val count: Int)
"##;

/// A page's flight stream as the App Router pushes it: a row cut between two pushes, another
/// row base64-encoded.
fn next_flight_page() -> String {
    use base64::Engine as _;
    let root = base64::engine::general_purpose::STANDARD
        .encode("0:[\"$\",\"$1\",null,{\"children\":\"$3\"}]\n");
    format!(
        r#"<html><body><script>(self.__next_f=self.__next_f||[]).push([0])</script>
<script>self.__next_f.push([1,"1:\"$Sreact.fragment\"\n2:I[\"123\",[],\"default\"]\n3:[{{\"id\":\"one\",\"ti"])</script>
<script>self.__next_f.push([1,"tle\":\"One\",\"cover\":\"/c/1.jpg\"}},{{\"id\":\"two\",\"title\":\"$$Two\"}}]\n"])</script>
<script>self.__next_f.push([3,"{root}"])</script></body></html>"#
    )
}

#[tokio::test]
async fn next_js_pages_give_their_data() {
    use axum::http::header::CONTENT_TYPE;
    let app = Router::new()
        .route("/list", get(|| async { Html(next_flight_page()) }))
        .route(
            "/search",
            get(|| async {
                Html(
                    r#"<html><body><script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{"data":{"results":[{"id":"x","title":"X Marks"}],"has_next":true}}},"page":"/search"}</script></body></html>"#,
                )
            }),
        )
        .route(
            "/series/one",
            get(|| async {
                (
                    [(CONTENT_TYPE, "text/x-component")],
                    "0:{\"a\":\"$@1\"}\n1:{\"manga\":{\"title\":\"One\",\"description\":\"$2\",\"genres\":\"$W3\",\"chapters\":\"$4:list\"}}\n2:T5,Hello3:[\"Action\",\"Comedy\"]\n4:{\"list\":[{\"id\":\"c1\",\"name\":\"Chapter 1\"}]}\n",
                )
            }),
        )
        .route(
            "/read/c1",
            get(|| async {
                (
                    [(CONTENT_TYPE, "text/x-component")],
                    "1:[[\"b\",\"/p/2.jpg\"],[\"a\",\"/p/1.jpg\"]]\n0:{\"images\":\"$Q1\",\"count\":2}\n",
                )
            }),
        );
    let address = serve(app);
    let extension = extension(
        "eu.kanade.tachiyomi.extension.en.fixturenext",
        "https://next.example",
    );
    let converted = convert::convert(
        &extension,
        &sources(
            NEXT_BUILD,
            &[(
                "src/eu/kanade/tachiyomi/extension/en/fixturenext/FixtureNext.kt",
                NEXT_SOURCE,
            )],
        ),
    )
    .unwrap();
    let host = load_converted(&converted, &extension, address).await;
    let provider = "mihon.en.fixturenext/4242";

    let popular = host
        .manga_list(provider, crate::manga::MangaList::Popular, 1)
        .await
        .unwrap();
    let titles: Vec<&str> = popular
        .items
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(titles, ["One", "$Two"], "the flight stream's rows, joined");
    assert_eq!(popular.items[0].id, "/series/one");

    let found = host.manga_search(provider, "x", 1).await.unwrap();
    assert_eq!(
        found.items[0].title, "X Marks",
        "`__NEXT_DATA__`, by another name"
    );
    assert!(found.has_next_page);

    let details = host.manga_details(provider, "/series/one").await.unwrap();
    assert_eq!(details.title, "One");
    assert_eq!(details.description.as_deref(), Some("Hello"), "a text row");
    assert_eq!(details.genres, ["Action", "Comedy"], "a set");
    let chapters = host.manga_chapters(provider, "/series/one").await.unwrap();
    assert_eq!(chapters[0].id, "/read/c1", "a path into another row");

    let pages = host.manga_pages(provider, "/read/c1").await.unwrap();
    let urls: Vec<&str> = pages.iter().map(|page| page.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            format!("http://{address}/p/2.jpg").as_str(),
            format!("http://{address}/p/1.jpg").as_str()
        ],
        "a map"
    );
}

#[test]
fn code_the_runtime_lacks_is_refused_by_name() {
    let extension = extension(
        "eu.kanade.tachiyomi.extension.en.fixturestand",
        "https://stand.example",
    );
    let reason = |source: &str| {
        convert::convert(
            &extension,
            &sources(
                STANDALONE_BUILD,
                &[(
                    "src/eu/kanade/tachiyomi/extension/en/fixturestand/FixtureStand.kt",
                    source,
                )],
            ),
        )
        .unwrap_err()
    };
    // An interceptor reading pictures through an API the runtime doesn't have.
    let interceptor = STANDALONE_SOURCE.replace(
        "rateLimit(4)",
        "addInterceptor { chain -> val image = PdfRenderer(chain.proceed(chain.request()).body.byteStream())\n chain.proceed(chain.request()) }",
    );
    assert!(
        reason(&interceptor).contains("PdfRenderer"),
        "{}",
        reason(&interceptor)
    );
    let webview = STANDALONE_SOURCE.replace(
        "val document = client.get(url, Fetch.DOCUMENT).asJsoup()",
        "val document = webView.evaluateJavascript(url)",
    );
    assert!(
        reason(&webview).contains("webView") || reason(&webview).contains("evaluateJavascript")
    );
    let starts =
        STANDALONE_SOURCE.replace("override val supportsLatest = false", "init { setup() }");
    assert!(reason(&starts).contains("setup"), "{}", reason(&starts));
}

#[tokio::test]
async fn validation_loads_the_package_before_it_installs() {
    let extension = extension(
        "eu.kanade.tachiyomi.extension.en.fixturecomics",
        "https://fixture.example",
    );
    let converted = convert::convert(
        &extension,
        &sources(
            MADARA_BUILD,
            &[(
                "src/eu/kanade/tachiyomi/extension/en/fixturecomics/FixtureComics.kt",
                MADARA_SOURCE,
            )],
        ),
    )
    .unwrap();
    let package = convert::package(&extension, &converted, "Keiyoushi", None).unwrap();
    convert::validate(&package, 1).await.unwrap();
    let broken = Converted {
        code: converted
            .code
            .replace("mihon.theme(\"madara\"", "mihon.theme(\"nothing\""),
        ..converted
    };
    let package = convert::package(&extension, &broken, "Keiyoushi", None).unwrap();
    assert!(
        convert::validate(&package, 1)
            .await
            .unwrap_err()
            .contains("nothing")
    );
}

/// A converted source's own filters are shown, and searches set them.
#[tokio::test]
async fn a_converted_sources_filters_are_shown_and_used() {
    let app = Router::new().route(
        "/list/page/1",
        get(|uri: Uri| async move {
            let query = uri.query().unwrap_or_default().to_owned();
            let title = if query.contains("sort=popular") && query.contains("genres=comedy") {
                "Filtered"
            } else {
                "Plain"
            };
            Html(format!(
                r#"<div class="listing"><div class="item"><a href="/manga/one"><h2>{title}</h2></a></div></div>"#
            ))
        }),
    );
    let source = STANDALONE_SOURCE
        .replace(
            "override suspend fun getPopularManga(page: Int) = getSearchMangaList(page, \"\", FilterList())",
            "override suspend fun getPopularManga(page: Int) = getSearchMangaList(page, \"\", getFilterList())",
        )
        .replace(
            ".addQueryParameter(\"s\", query)",
            r#".addQueryParameter("s", query)
            .addQueryParameter("sort", filters.firstInstance<SortFilter>().selected)
            .addQueryParameter("genres", filters.firstInstance<GenreFilter>().state.filter { it.state }.joinToString(",") { it.value })"#,
        )
        .replace(
            "    companion object {",
            r#"    override fun getFilterList() = FilterList(
        Filter.Header("Filters"),
        SortFilter(),
        GenreFilter(listOf(Genre("Action", "action"), Genre("Comedy", "comedy"))),
    )

    companion object {"#,
        )
        + r#"
open class SelectFilter(name: String, private val options: List<Pair<String, String>>) :
    Filter.Select<String>(name, options.map { it.first }.toTypedArray()) {
    val selected get() = options[state].second
}

class SortFilter : SelectFilter("Sort", listOf("Latest" to "latest", "Popular" to "popular"))

class Genre(name: String, val value: String) : Filter.CheckBox(name)

class GenreFilter(genres: List<Genre>) : Filter.Group<Genre>("Genres", genres)
"#;
    let extension = extension(
        "eu.kanade.tachiyomi.extension.en.fixturestand",
        "https://stand.example",
    );
    let converted = convert::convert(
        &extension,
        &sources(
            STANDALONE_BUILD,
            &[(
                "src/eu/kanade/tachiyomi/extension/en/fixturestand/FixtureStand.kt",
                &source,
            )],
        ),
    )
    .unwrap();
    let host = load_converted(&converted, &extension, serve(app)).await;
    let provider = "mihon.en.fixturestand/4242";
    let filters = host.manga_filters(provider).await.unwrap();
    assert_eq!(filters.len(), 2, "{filters:?}");
    assert!(
        matches!(&filters[0], raygun_model::SourceFilter::Select { label, options, .. } if label == "Sort" && options.len() == 2)
    );
    assert!(
        matches!(&filters[1], raygun_model::SourceFilter::Tags { label, options, .. } if label == "Genres" && options[1].label == "Comedy")
    );
    let plain = host
        .manga_list(provider, crate::manga::MangaList::Popular, 1)
        .await
        .unwrap();
    assert_eq!(plain.items[0].title, "Plain");
    let mut chosen = BTreeMap::new();
    chosen.insert(
        "f1".to_owned(),
        raygun_model::FilterValue::Select("1".into()),
    );
    chosen.insert(
        "f2".to_owned(),
        raygun_model::FilterValue::Tags {
            include: vec!["1".into()],
            exclude: Vec::new(),
        },
    );
    let filtered = host
        .manga_search_filtered(provider, "", 1, &chosen)
        .await
        .unwrap();
    assert_eq!(filtered.items[0].title, "Filtered");
}

/// Interceptors and Keiyoushi's `addCookie` change the requests a converted source sends.
#[tokio::test]
async fn interceptors_and_cookies_reach_the_site() {
    let app = Router::new().route(
        "/list/page/1",
        get(|headers: axum::http::HeaderMap| async move {
            assert_eq!(headers["x-token"], "t", "the interceptor's header");
            assert_eq!(headers["cookie"], "lang=en", "the cookie added");
            Html(r#"<div class="listing"><div class="item"><a href="/manga/one"><h2>One</h2></a></div></div>"#)
        }),
    );
    let source = STANDALONE_SOURCE.replace(
        "override fun OkHttpClient.Builder.configureClient() = rateLimit(4)",
        r#"override fun OkHttpClient.Builder.configureClient() = addInterceptor { chain ->
        val request = chain.request().newBuilder().header("X-Token", "t").build()
        val response = chain.proceed(request)
        if (response.code == 418) throw IOException("teapot")
        response
    }.addCookie("lang" to "en")"#,
    );
    let extension = extension(
        "eu.kanade.tachiyomi.extension.en.fixturestand",
        "https://stand.example",
    );
    let converted = convert::convert(
        &extension,
        &sources(
            STANDALONE_BUILD,
            &[(
                "src/eu/kanade/tachiyomi/extension/en/fixturestand/FixtureStand.kt",
                &source,
            )],
        ),
    )
    .unwrap();
    let host = load_converted(&converted, &extension, serve(app)).await;
    let popular = host
        .manga_list(
            "mihon.en.fixturestand/4242",
            crate::manga::MangaList::Popular,
            1,
        )
        .await
        .unwrap();
    assert_eq!(popular.items[0].title, "One");
}

/// The names in one of `kotlin.js`'s tables (`const GLOBALS = { … }`): its top-level keys.
fn runtime_table(name: &str) -> std::collections::HashSet<String> {
    let runtime = include_str!("kotlin.js");
    let start = runtime
        .find(&format!("const {name} = {{"))
        .unwrap_or_else(|| panic!("kotlin.js has {name}"));
    let body = &runtime[start..];
    let mut depth = 0usize;
    let mut end = body.len();
    for (index, char) in body.char_indices() {
        match char {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = index;
                    break;
                }
            }
            _ => {}
        }
    }
    let key = regex::Regex::new(r"(?m)^ {4}([A-Za-z_][A-Za-z0-9_]*)\s*[:,(]").unwrap();
    key.captures_iter(&body[..end])
        .map(|found| found[1].to_owned())
        .collect()
}

/// The converter refuses code that needs what the runtime lacks, so its lists of the runtime's
/// functions and objects are the runtime's own.
#[test]
fn converter_knows_what_the_runtime_has() {
    use super::kotlin::emit::{GLOBALS, OBJECTS};
    let globals = runtime_table("GLOBALS");
    let objects = runtime_table("OBJECTS");
    for name in GLOBALS {
        assert!(globals.contains(*name), "kotlin.js's GLOBALS has {name}");
    }
    for name in OBJECTS {
        assert!(objects.contains(*name), "kotlin.js's OBJECTS has {name}");
    }
    for name in &globals {
        assert!(
            GLOBALS.contains(&name.as_str()),
            "the converter's GLOBALS has {name}"
        );
    }
    for name in &objects {
        assert!(
            OBJECTS.contains(&name.as_str()),
            "the converter's OBJECTS has {name}"
        );
    }
}

/// Calendars, zones, `awaitAll` and a `Mutex`, local extensions and `this@Class`, as converted
/// code uses them.
#[tokio::test]
async fn converted_code_reads_dates_and_waits_together() {
    let source = STANDALONE_SOURCE
        .replace(
            "override suspend fun getLatestUpdates(page: Int): MangasPage = throw UnsupportedOperationException()",
            "",
        )
        .replace(
        "override val supportsLatest = false",
        r#"override val supportsLatest = true

    private val lock = Mutex()

    private val zoned = SimpleDateFormat("yyyy-MM-dd HH:mm", Locale.ROOT).apply {
        timeZone = TimeZone.getTimeZone("GMT+7")
    }

    override suspend fun getLatestUpdates(page: Int): MangasPage = coroutineScope {
        val names = listOf("one", "two").map { name ->
            async { lock.withLock { name } }
        }.awaitAll()
        val day = Calendar.getInstance(TimeZone.getTimeZone("UTC")).apply {
            timeInMillis = 0L
            add(Calendar.DAY_OF_MONTH, 2)
            set(Calendar.HOUR_OF_DAY, 5)
        }.timeInMillis
        val local = DateTimeFormatter.ofPattern("yyyy-MM-dd").withZone(ZoneOffset.UTC)
        val third = LocalDate.parse("1970-01-03").atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli()
        fun StringBuilder.tag(value: String) = append("[").append(value).append("]")
        val tags = buildString {
            tag("a")
            this.tag("b")
        }
        val mangas = names.map { name ->
            Entry(name).toManga().apply {
                url = "/${day}/${zoned.parse("1970-01-01 07:00")?.time}/${local.parse("1970-01-02")}/$third/" +
                    Parser.unescapeEntities("&lt;&#233;&gt;", false) + tags +
                    "https://stand.example/comics/b".toHttpUrl().pathSegments[1] +
                    json.decodeFromString<Wrapper<Entry>>("""{"data":{"name":"c"}}""").data!!.shout
            }
        }
        MangasPage(mangas, String.format(Locale.US, "%d", page) == "2")
    }"#,
    ) + r#"
abstract class Named(private val label: String) {
    val shout get() = label.uppercase()
}

@Serializable
class Wrapper<T>(val data: T? = null)

@Serializable
class Entry(val name: String) : Named(name) {
    fun toManga(): SManga = SManga.create().apply {
        title = this@Entry.shout
    }
}
"#;
    let extension = extension(
        "eu.kanade.tachiyomi.extension.en.fixturestand",
        "https://stand.example",
    );
    let converted = convert::convert(
        &extension,
        &sources(
            STANDALONE_BUILD,
            &[(
                "src/eu/kanade/tachiyomi/extension/en/fixturestand/FixtureStand.kt",
                &source,
            )],
        ),
    )
    .unwrap();
    let host = load_converted(&converted, &extension, serve(Router::new())).await;
    let listing = host
        .manga_list(
            "mihon.en.fixturestand/4242",
            crate::manga::MangaList::Latest,
            2,
        )
        .await
        .unwrap();
    let titles: Vec<_> = listing
        .items
        .iter()
        .map(|item| item.title.clone())
        .collect();
    assert_eq!(titles, ["ONE", "TWO"]);
    assert_eq!(
        listing.items[0].id,
        "/190800000/0/86400000/172800000/<\u{e9}>[a][b]bC"
    );
    assert!(listing.has_next_page);
}

/// Sequences that don't end, `init` blocks, `++` on properties, names written with their
/// package, comparators and formats, as converted code uses them.
#[tokio::test]
async fn converted_code_reads_sequences_inits_and_qualified_names() {
    let source = STANDALONE_SOURCE
        .replace(
            "override suspend fun getLatestUpdates(page: Int): MangasPage = throw UnsupportedOperationException()",
            "",
        )
        .replace(
            "override val supportsLatest = false",
            r##"override val supportsLatest = true

    private val preferences by getPreferencesLazy {
        if (getString("old", "x") == "x") {
            edit().putString("server", "second").apply()
        }
    }

    private var served = 0

    init {
        served = 10
    }

    private fun Document.label(): String = selectFirst("b")!!.text()

    private inline fun <reified T> String.decoded(): T = parseAs<Wrap<T>>().value

    private fun pick(kind: Int): (String) -> String = when (kind) {
        0 -> { _ -> "none" }
        else -> { text -> text.uppercase() }
    }

    override suspend fun getLatestUpdates(page: Int): MangasPage {
        var seed = 7
        val random = generateSequence(seed) { x -> (x * 3) % 1000 }.drop(1).iterator()
        val firsts = List(3) { random.next() }
        val counted = generateSequence(1) { it + 1 }.map { it * 2 }.takeWhile { it < 9 }.toList()
        val box = Box("a-b")
        box.count++
        ++box.count
        served++
        val order = Filter.Sort.Selection(2, true)
        val host = java.net.URI("https://other.example/x/y").host
        val sorted = listOf("b", "C", "a").sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it })
        val left = mutableListOf(1, 2, 3).apply { removeLast() }
        val price = DecimalFormat("#,###.##").format(12345.678)
        val names = listOf(
            firsts.joinToString(","),
            counted.joinToString(","),
            "${box.parts.size}:${box.count}:$served",
            "${order.index}:${order.ascending}",
            host,
            sorted.joinToString(""),
            left.joinToString(""),
            price,
            pick(0)("x") + pick(1)("y"),
            preferences.getString("server", "first")!!,
            json.parseToJsonElement("""{"a":1,"b":2}""").jsonObject.mapValues { it.value.jsonPrimitive.int + 1 }.values.sum().toString(),
            Regex("[a-c[x-z]]+").find("--bxz--")!!.value,
            mapOf("a" to 1, "b" to 2).count { it.value > 1 }.toString(),
            "cb({\"n\":5})".parseAs<JsonObject> { it.substringAfter("(").substringBeforeLast(")") }["n"]!!.jsonPrimitive.int.toString(),
            listOf(1, 2, 3).groupBy({ it % 2 }, { it * 10 })[1]!!.sum().toString(),
            Jsoup.parse("<p><a>1</a><b>2</b><i>3</i></p>").select("p > :is(a, b)").size.toString(),
            Square(3).describe(),
            Names().apply { add("a"); add("a"); add("b") }.let { "${it.size}${it.firstName()}" },
            Jsoup.parse("<p>x</p>").toString().contains("<p>x</p>").toString() + "x".toString() + mapOf("a" to 1).toString(),
            mapOf("a" to 1).getValue("a").toString(),
            Regex("^(?i)ch(apter)?").containsMatchIn("Chapter 1").toString(),
            with(Jsoup.parse("<b>bold</b>")) { SManga.create().apply { title = label() } }.title,
            """{"value":{"n":4}}""".decoded<Item>().twice().toString(),
            javaClass.getResource("/assets/greeting.txt")!!.readText(),
            Layout.B.url("x"),
            QuickJs.create().use { engine -> engine.evaluate("var k = 2"); engine.evaluate("k * 21") }.toString(),
            "ff".toInt(16).toString(),
            OrderPick().state.toString(),
            "https://x.example/manga/32d76d19-8a05-4db0-9fc2-e0b0648fe9d0".contains(Regex("[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}")).toString(),
        )
        return MangasPage(names.map { name -> SManga.create().apply { title = name; url = "/$name" } }, false)
    }"##,
        ) + r#"
@Serializable
class Wrap<T>(val value: T)

@Serializable
class Item(val n: Int) {
    fun twice() = n * 2
}

enum class Layout(private val prefix: String) {
    A("/a/"),
    B("/b/"),
    ;

    fun url(slug: String) = "$prefix$slug"
}

open class Pick(name: String, values: Array<String>) : Filter.Select<String>(name, values)

class OrderPick : Pick("Order", arrayOf("a", "b", "c")) {
    init {
        state = 2
    }
}

abstract class Shape {
    fun describe() = "area " + area()

    abstract fun area(): Int
}

class Square(private val side: Int) : Shape() {
    override fun area() = side * side
}

class Names : LinkedHashSet<String>() {
    fun firstName() = first()
}

class Box(text: String) {
    lateinit var parts: List<String>
    var count = 0

    init {
        parts = text.split("-")
        count = parts.size
    }
}
"#;
    let extension = extension(
        "eu.kanade.tachiyomi.extension.en.fixturestand",
        "https://stand.example",
    );
    let mut code = sources(
        STANDALONE_BUILD,
        &[(
            "src/eu/kanade/tachiyomi/extension/en/fixturestand/FixtureStand.kt",
            &source,
        )],
    );
    code.assets
        .insert("assets/greeting.txt".to_owned(), b"hello".to_vec());
    let converted = convert::convert(&extension, &code).unwrap();
    let host = load_converted(&converted, &extension, serve(Router::new())).await;
    let listing = host
        .manga_list(
            "mihon.en.fixturestand/4242",
            crate::manga::MangaList::Latest,
            1,
        )
        .await
        .unwrap();
    let titles: Vec<_> = listing
        .items
        .iter()
        .map(|item| item.title.clone())
        .collect();
    assert_eq!(
        titles,
        [
            "21,63,189",
            "2,4,6,8",
            "2:4:11",
            "2:true",
            "other.example",
            "abC",
            "12",
            "12,345.68",
            "noneY",
            "second",
            "5",
            "bxz",
            "1",
            "5",
            "40",
            "2",
            "area 9",
            "2a",
            "truex{a=1}",
            "1",
            "true",
            "bold",
            "8",
            "hello",
            "/b/x",
            "42",
            "255",
            "2",
            "true",
        ]
    );
}

const JUMPS_SOURCE: &str = r##"package eu.kanade.tachiyomi.extension.en.fixturejumps

import eu.kanade.tachiyomi.source.model.FilterList
import keiyoushi.source.KeiSource

@Source
abstract class FixtureJumps : KeiSource() {
    override val supportsLatest = false

    override suspend fun getPopularManga(page: Int): MangasPage {
        val document = client.get("$baseUrl/list", Fetch.DOCUMENT).asJsoup()
        val mangas = mutableListOf<SManga>()
        for (element in document.select(".item")) {
            val name = element.selectFirst("h2")?.text() ?: continue
            element.attr("data-skip").takeIf { it == "yes" }?.let { continue }
            if (name == "Stop") break
            mangas += SManga.create().apply {
                title = name
                url = element.attr("data-url")
            }
        }
        val cookies = client.cookieJar.loadForRequest(baseUrl.toHttpUrl())
        val token = cookies.firstOrNull { it.name == "token" }?.value ?: "none"
        val note = document.selectFirst(".note")?.textNodes()?.firstOrNull()?.text()?.trim().orEmpty()
        mangas += SManga.create().apply {
            title = "token $token, note $note"
            url = "/x"
        }
        return MangasPage(mangas, false)
    }

    override suspend fun getLatestUpdates(page: Int): MangasPage = throw UnsupportedOperationException()

    override suspend fun getSearchMangaList(page: Int, query: String, filters: FilterList): MangasPage = getPopularManga(page)

    override suspend fun fetchMangaUpdate(
        manga: SManga,
        chapters: List<SChapter>,
        fetchDetails: Boolean,
        fetchChapters: Boolean,
    ): SMangaUpdate = SMangaUpdate(manga, emptyList())

    override suspend fun getPageList(chapter: SChapter): List<Page> = emptyList()
}
"##;

/// `break` and `continue` out of lambdas and expressions, the cookie jar read in code and Jsoup's
/// text nodes.
#[tokio::test]
async fn loops_jump_from_lambdas_and_cookies_are_read() {
    use axum::http::header::SET_COOKIE;
    let app = Router::new().route(
        "/list",
        get(|| async {
            (
                [(SET_COOKIE, "token=abc; Path=/")],
                Html(
                    r#"<div class="item" data-url="/a"><h2>A</h2></div><div class="item"><p>no title</p></div>
                    <div class="item" data-skip="yes" data-url="/s"><h2>Skipped</h2></div><div class="item" data-url="/b"><h2>B</h2></div>
                    <div class="item" data-url="/c"><h2>Stop</h2></div><div class="item" data-url="/d"><h2>D</h2></div>
                    <p class="note">  First words <b>bold</b> last words</p>"#,
                ),
            )
        }),
    );
    let address = serve(app);
    let extension = extension(
        "eu.kanade.tachiyomi.extension.en.fixturejumps",
        "https://jumps.example",
    );
    let build = "keiyoushi {\n    name = \"Fixture Jumps\"\n    versionCode = 1\n    libVersion = \"1.6\"\n    source {\n        lang = \"en\"\n        baseUrl = \"https://jumps.example\"\n    }\n}\n";
    let converted = convert::convert(
        &extension,
        &sources(
            build,
            &[(
                "src/eu/kanade/tachiyomi/extension/en/fixturejumps/FixtureJumps.kt",
                JUMPS_SOURCE,
            )],
        ),
    )
    .unwrap();
    let host = load_converted(&converted, &extension, address).await;
    let popular = host
        .manga_list(
            "mihon.en.fixturejumps/4242",
            crate::manga::MangaList::Popular,
            1,
        )
        .await
        .unwrap();
    let titles: Vec<&str> = popular
        .items
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(titles, ["A", "B", "token abc, note First words"]);
}

const REDIRECTS_SOURCE: &str = r##"package eu.kanade.tachiyomi.extension.en.fixtureredirects

import eu.kanade.tachiyomi.source.model.FilterList
import keiyoushi.source.KeiSource
import keiyoushi.utils.GraphQLException
import keiyoushi.utils.graphQLBody
import keiyoushi.utils.parseGraphQLAs

@Source
abstract class FixtureRedirects : KeiSource() {
    override val supportsLatest = false

    private var apiKey = "stale"

    private val keyRegex = Regex("key=([^;]+)")

    private suspend fun chapter(): String = client.post(
        "$baseUrl/graphql",
        headersBuilder().set("x-key", apiKey).build(),
        Fetch.XHR,
        graphQLBody(query = "{ chapter { pages } }"),
    ).parseGraphQLAs<ChapterData>().chapter!!.pages

    override suspend fun getPopularManga(page: Int): MangasPage {
        val pages = try {
            chapter()
        } catch (e: GraphQLException) {
            val noRedirectClient = client.newBuilder().followRedirects(false).build()
            val response = noRedirectClient.get("$baseUrl/moved", headers, Fetch.DOCUMENT, ensureSuccess = false)
            apiKey = response.headers("Set-Cookie").firstNotNullOfOrNull { keyRegex.find(it)?.groupValues?.get(1) } ?: "none"
            val moved = "${response.code} ${response.header("Location")}"
            response.close()
            "${e.message}, $moved, ${chapter()}"
        }
        val followed = client.get("$baseUrl/moved", Fetch.DOCUMENT).asJsoup().selectFirst("h2")?.text() ?: "lost"
        val manga = SManga.create().apply {
            title = "$pages, $followed"
            url = "/x"
        }
        return MangasPage(listOf(manga), false)
    }

    override suspend fun getLatestUpdates(page: Int): MangasPage = throw UnsupportedOperationException()

    override suspend fun getSearchMangaList(page: Int, query: String, filters: FilterList): MangasPage = getPopularManga(page)

    override suspend fun fetchMangaUpdate(
        manga: SManga,
        chapters: List<SChapter>,
        fetchDetails: Boolean,
        fetchChapters: Boolean,
    ): SMangaUpdate = SMangaUpdate(manga, emptyList())

    override suspend fun getPageList(chapter: SChapter): List<Page> = emptyList()
}

@Serializable
class ChapterData(val chapter: ChapterObject? = null)

@Serializable
class ChapterObject(val pages: String)
"##;

/// MangaHub's key refresh: a GraphQL error beside data (`{"data": {"chapter": null}, "errors":
/// [...]}`) fails `parseGraphQLAs`, and the new key comes with a redirect that a client built with
/// OkHttp's `followRedirects(false)` gets as it came (status, `Location`, cookies), while the
/// source's own client keeps following redirects.
#[tokio::test]
async fn a_client_built_not_to_follow_redirects_gets_them_as_they_came() {
    use axum::http::header::{CONTENT_TYPE, LOCATION, SET_COOKIE};
    let app = Router::new()
        .route(
            "/graphql",
            post(|headers: axum::http::HeaderMap| async move {
                let answer = if headers.get("x-key").is_some_and(|key| key == "fresh") {
                    json!({"data": {"chapter": {"pages": "p1"}}})
                } else {
                    json!({
                        "data": {"chapter": null},
                        "errors": [{"message": "api key invalid"}],
                    })
                };
                ([(CONTENT_TYPE, "application/json")], answer.to_string())
            }),
        )
        .route(
            "/moved",
            get(|| async {
                (
                    axum::http::StatusCode::FOUND,
                    [(LOCATION, "/target"), (SET_COOKIE, "key=fresh; Path=/")],
                )
            }),
        )
        .route("/target", get(|| async { Html("<h2>Arrived</h2>") }));
    let address = serve(app);
    let extension = extension(
        "eu.kanade.tachiyomi.extension.en.fixtureredirects",
        "https://redirects.example",
    );
    let build = "keiyoushi {\n    name = \"Fixture Redirects\"\n    versionCode = 1\n    libVersion = \"1.6\"\n    source {\n        lang = \"en\"\n        baseUrl = \"https://redirects.example\"\n    }\n}\n";
    let converted = convert::convert(
        &extension,
        &sources(
            build,
            &[(
                "src/eu/kanade/tachiyomi/extension/en/fixtureredirects/FixtureRedirects.kt",
                REDIRECTS_SOURCE,
            )],
        ),
    )
    .unwrap();
    let host = load_converted(&converted, &extension, address).await;
    let popular = host
        .manga_list(
            "mihon.en.fixtureredirects/4242",
            crate::manga::MangaList::Popular,
            1,
        )
        .await
        .unwrap();
    let titles: Vec<&str> = popular
        .items
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(titles, ["api key invalid, 302 /target, p1, Arrived"]);
}

/// A source whose client puts its pictures back together (`BitmapFactory`, `Canvas`, okio's
/// `Buffer`) and draws its notes as pages (`lib:textinterceptor`): its pages are fetched through
/// it, and the pictures come back drawn.
#[tokio::test]
async fn pictures_a_source_draws_are_fetched_through_it() {
    // A 2×4 picture: red above, blue below; the site serves it the other way up.
    let mut png = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_fn(2, 4, |_, y| {
        if y < 2 {
            image::Rgba([255, 0, 0, 255])
        } else {
            image::Rgba([0, 0, 255, 255])
        }
    })
    .write_to(&mut png, image::ImageFormat::Png)
    .unwrap();
    let png = png.into_inner();
    let app = Router::new()
        .route(
            "/manga/one/1",
            get(|| async { Html(r#"<div id="images"><img src="/pic.png#flip"></div>"#) }),
        )
        .route(
            "/pic.png",
            get(move || {
                let png = png.clone();
                async move { ([("content-type", "image/png")], png) }
            }),
        );
    let source = STANDALONE_SOURCE
        .replace(
            "override fun OkHttpClient.Builder.configureClient() = rateLimit(4)",
            "override fun OkHttpClient.Builder.configureClient() = addInterceptor(FlipInterceptor()).addInterceptor(TextInterceptor())",
        )
        .replace(
            r#"        .mapIndexed { index, element -> Page(index, imageUrl = element.absUrl("src")) }"#,
            r#"        .mapIndexed { index, element -> Page(index, imageUrl = element.absUrl("src")) } +
        Page(1, imageUrl = TextInterceptorHelper.createUrl("Note", "Hello <b>there</b>"))"#,
        )
        + r#"
class FlipInterceptor : Interceptor {
    override fun intercept(chain: Interceptor.Chain): Response {
        val response = chain.proceed(chain.request())
        if (chain.request().url.fragment != "flip") return response
        val image = response.body.use { BitmapFactory.decodeStream(it.byteStream()) }
        val width = image.width
        val height = image.height
        val result = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(result)
        val half = height / 2
        canvas.drawBitmap(image, Rect(0, half, width, height), Rect(0, 0, width, half), null)
        canvas.save()
        canvas.translate(0f, half.toFloat())
        canvas.drawBitmap(image, Rect(0, 0, width, half), Rect(0, 0, width, half), null)
        canvas.restore()
        val buffer = Buffer()
        result.compress(Bitmap.CompressFormat.PNG, 100, buffer.outputStream())
        image.recycle()
        result.recycle()
        return response.newBuilder().body(buffer.asResponseBody("image/png".toMediaType(), buffer.size)).build()
    }
}
"#;
    let extension = extension(
        "eu.kanade.tachiyomi.extension.en.fixturestand",
        "https://stand.example",
    );
    let build = format!(
        "{STANDALONE_BUILD}dependencies {{\n    implementation(project(\":lib:textinterceptor\"))\n}}\n"
    );
    let converted = convert::convert(
        &extension,
        &sources(
            &build,
            &[(
                "src/eu/kanade/tachiyomi/extension/en/fixturestand/FixtureStand.kt",
                &source,
            )],
        ),
    )
    .unwrap();
    assert!(converted.code.contains("imagesFromSource: true"));
    let host = load_converted(&converted, &extension, serve(app)).await;
    let provider = "mihon.en.fixturestand/4242";
    let pages = host.manga_pages(provider, "/manga/one/1").await.unwrap();
    assert_eq!(pages.len(), 2);
    let (address, by) = raygun_net::images::source_marked(&pages[0].url).unwrap();
    assert_eq!(by, provider);
    assert!(address.ends_with("/pic.png#flip"), "{address}");

    let bytes = host
        .manga_image(provider, address, &pages[0].headers)
        .await
        .unwrap();
    let picture = image::load_from_memory(&bytes).unwrap().to_rgba8();
    assert_eq!(picture.dimensions(), (2, 4));
    assert_eq!(
        picture.get_pixel(0, 0),
        &image::Rgba([0, 0, 255, 255]),
        "blue on top"
    );
    assert_eq!(
        picture.get_pixel(1, 3),
        &image::Rgba([255, 0, 0, 255]),
        "red below"
    );

    // The note, drawn as a page 1000 pixels wide (where the system has a font).
    let (address, _) = raygun_net::images::source_marked(&pages[1].url).unwrap();
    assert!(address.starts_with("http://tachiyomi-lib-textinterceptor/Note/"));
    match host.manga_image(provider, address, &pages[1].headers).await {
        Ok(bytes) => {
            let note = image::load_from_memory(&bytes).unwrap().to_rgba8();
            assert_eq!(note.width(), 1000);
            assert!(note.pixels().any(|pixel| pixel.0[0] < 128), "some ink");
        }
        Err(error) => assert!(error.contains("no font"), "{error}"),
    }
}

/// A protobuf message: `(number, wire type, bytes)` fields, written as the wire has them.
fn proto(fields: &[(u32, u8, Vec<u8>)]) -> Vec<u8> {
    fn varint(mut value: u64, out: &mut Vec<u8>) {
        loop {
            let byte = (value & 127) as u8;
            value >>= 7;
            if value == 0 {
                out.push(byte);
                return;
            }
            out.push(byte | 128);
        }
    }
    let mut out = Vec::new();
    for (number, wire, bytes) in fields {
        varint(u64::from(*number) * 8 + u64::from(*wire), &mut out);
        if *wire == 2 {
            varint(bytes.len() as u64, &mut out);
        }
        out.extend_from_slice(bytes);
    }
    out
}

#[tokio::test]
async fn protobuf_answers_and_bodies_convert() {
    // The answer: two titles (one with packed tags), whether there's more, a signed number.
    let titles = [(7u8, "First"), (9, "Second")];
    let answer = proto(
        &titles
            .iter()
            .map(|(id, name)| {
                let mut fields = vec![(1, 0, vec![*id]), (2, 2, name.as_bytes().to_vec())];
                if *id == 7 {
                    fields.push((4, 2, vec![3, 150, 1]));
                }
                (1, 2, proto(&fields))
            })
            .chain([(2, 0, vec![1]), (3, 0, vec![3])])
            .collect::<Vec<_>>(),
    );
    let app = Router::new().route(
        "/api/list",
        post(move |body: axum::body::Bytes| {
            let answer = answer.clone();
            async move {
                // The request: page 1 and the query "x".
                if body.as_ref() != proto(&[(1, 0, vec![1]), (2, 2, b"x".to_vec())]).as_slice() {
                    return (axum::http::StatusCode::BAD_REQUEST, Vec::new());
                }
                (axum::http::StatusCode::OK, answer)
            }
        }),
    );
    let source = STANDALONE_SOURCE.replace(
        "    override suspend fun getPopularManga(page: Int) = getSearchMangaList(page, \"\", FilterList())\n",
        r#"    override suspend fun getPopularManga(page: Int): MangasPage {
        val body = ListRequest(page, "x").toRequestBodyProto()
        val result = client.post("$baseUrl/api/list", body = body).parseAsProto<ListResponse>()
        val mangas = result.titles.map { title ->
            SManga.create().apply {
                url = "/manga/${title.id}"
                this.title = title.name + title.tags.joinToString("") { "-$it" }
            }
        }
        return MangasPage(mangas, hasNextPage = result.next && result.delta == -2)
    }
"#,
    ) + r#"
@Serializable
class ListRequest(@ProtoNumber(1) val page: Int, @ProtoNumber(2) val query: String)

@Serializable
class ListResponse(
    @ProtoNumber(1) val titles: List<TitleDto> = emptyList(),
    @ProtoNumber(2) val next: Boolean = false,
    @ProtoNumber(3) @ProtoType(ProtoIntegerType.SIGNED) val delta: Int = 0,
)

@Serializable
class TitleDto(
    @ProtoNumber(1) val id: Int,
    @ProtoNumber(2) val name: String,
    @ProtoNumber(4) val tags: List<Int> = emptyList(),
)
"#;
    let extension = extension(
        "eu.kanade.tachiyomi.extension.en.fixturestand",
        "https://stand.example",
    );
    let converted = convert::convert(
        &extension,
        &sources(
            STANDALONE_BUILD,
            &[(
                "src/eu/kanade/tachiyomi/extension/en/fixturestand/FixtureStand.kt",
                &source,
            )],
        ),
    )
    .unwrap();
    let host = load_converted(&converted, &extension, serve(app)).await;
    let listing = host
        .manga_list(
            "mihon.en.fixturestand/4242",
            crate::manga::MangaList::Popular,
            1,
        )
        .await
        .unwrap();
    let titles: Vec<&str> = listing
        .items
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(titles, ["First-3-150", "Second"]);
    assert!(listing.has_next_page);
}

#[tokio::test]
async fn a_srcset_read_as_an_address_keeps_its_list() {
    // Jsoup resolves `abs:srcset` as Java's `URL` does, spaces kept: the source splits it.
    let app = Router::new().route(
        "/list/page/1",
        get(|| async {
            Html(
                r#"<div class="listing"><div class="item"><a href="/manga/one"><img srcset="/c/one-110.jpg 110w, /c/one-175.jpg 175w"><h2>One</h2></a></div></div>"#,
            )
        }),
    );
    let source = STANDALONE_SOURCE.replace(
        r#"thumbnail_url = element.selectFirst("img")?.absUrl("src")"#,
        r#"thumbnail_url = element.selectFirst("img")?.attr("abs:srcset")?.split(" ")?.getOrNull(2)"#,
    );
    let address = serve(app);
    let extension = extension(
        "eu.kanade.tachiyomi.extension.en.fixturestand",
        "https://stand.example",
    );
    let converted = convert::convert(
        &extension,
        &sources(
            STANDALONE_BUILD,
            &[(
                "src/eu/kanade/tachiyomi/extension/en/fixturestand/FixtureStand.kt",
                &source,
            )],
        ),
    )
    .unwrap();
    let host = load_converted(&converted, &extension, address).await;
    let popular = host
        .manga_list(
            "mihon.en.fixturestand/4242",
            crate::manga::MangaList::Popular,
            1,
        )
        .await
        .unwrap();
    assert_eq!(
        popular.items[0].cover_url.as_deref(),
        Some(format!("http://{address}/c/one-175.jpg").as_str())
    );
}
