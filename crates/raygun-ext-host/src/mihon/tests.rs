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
    assert_eq!(store.name, "someone", "a generic project is named by its owner");
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
    Sources { files: map }
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
