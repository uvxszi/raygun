//! Converted sources driving web views (Keiyoushi's `runWebView`), against the scripted views of
//! `crate::webview::fake`: what the run sets up, the page's messages, scripts, intercepted
//! requests, local storage, timeouts and browser checks.

use std::sync::Arc;

use axum::Router;

use super::convert;
use super::tests::{extension, load_converted, sources};
use crate::fixture_tests::serve;
use crate::webview::fake::{FakeHost, FakePage};
use crate::webview::{Answer, Command};

const VIEWS_BUILD: &str = "keiyoushi {\n    name = \"Fixture Views\"\n    versionCode = 1\n    libVersion = \"1.6\"\n    source {\n        lang = \"en\"\n        baseUrl = \"https://views.example\"\n    }\n}\n";

const VIEWS_SOURCE: &str = r##"package eu.kanade.tachiyomi.extension.en.fixtureviews

import android.webkit.WebResourceResponse
import eu.kanade.tachiyomi.source.model.FilterList
import keiyoushi.source.KeiSource
import keiyoushi.utils.WebViewTimeoutException
import keiyoushi.utils.getLocalStorage
import keiyoushi.utils.parseAs
import keiyoushi.utils.runWebView
import java.io.ByteArrayInputStream
import java.io.IOException
import kotlin.time.Duration.Companion.seconds

@Source
abstract class FixtureViews : KeiSource() {
    override val supportsLatest = true

    override suspend fun getPopularManga(page: Int): MangasPage {
        val names = runWebView<String>(10.seconds) {
            userAgent = "Fixture/1.0"
            blockImages = true
            webMessageBridge("android", setOf(baseUrl)) { resolve(it) }
            onPageFinished {
                evaluateJs("document.title") { title ->
                    if (title != "\"History\"") reject(IOException("not the history page: $title"))
                }
            }
            loadUrl("$baseUrl/history")
        }.parseAs<List<String>>()
        var blocked = 0
        val token = runWebView<String>(timeout = 5.seconds) {
            interceptRequest { request ->
                if (request.url.toString().endsWith("/ads.js")) {
                    blocked++
                    WebResourceResponse("text/plain", "utf-8", 403, "Blocked", emptyMap(), ByteArrayInputStream(ByteArray(0)))
                } else {
                    null
                }
            }
            onPageFinished { url ->
                evaluateJs("window.token") { value -> resolve(value.replace("\"", "")) }
            }
            loadUrl("$baseUrl/reader")
        }
        val secret = getLocalStorage(baseUrl, "secret") ?: "none"
        val waited = try {
            runWebView<String>(timeout = 1.seconds) {
                loadUrl("$baseUrl/slow")
            }
        } catch (e: IOException) {
            "io"
        } catch (e: WebViewTimeoutException) {
            "timed out"
        }
        val titles = names + listOf(token, "blocked $blocked", secret, waited)
        return MangasPage(titles.map { name -> SManga.create().apply { title = name; url = "/$name" } }, false)
    }

    override suspend fun getLatestUpdates(page: Int): MangasPage {
        runWebView<Unit> {
            onPageFinished { resolve(Unit) }
            loadUrl("$baseUrl/guarded")
        }
        return MangasPage(emptyList(), false)
    }

    override suspend fun getSearchMangaList(page: Int, query: String, filters: FilterList): MangasPage {
        passCloudflare()
        return MangasPage(emptyList(), false)
    }

    private suspend fun passCloudflare() = runWebView<Unit> {
        onPageFinished { resolve(Unit) }
        loadUrl("$baseUrl/challenge")
    }

    override suspend fun fetchMangaUpdate(
        manga: SManga,
        chapters: List<SChapter>,
        fetchDetails: Boolean,
        fetchChapters: Boolean,
    ): SMangaUpdate = SMangaUpdate(manga, emptyList())

    override suspend fun getPageList(chapter: SChapter): List<Page> = emptyList()
}
"##;

const PROVIDER: &str = "mihon.en.fixtureviews/4242";

fn script(answers: &'static [(&'static str, &'static str)]) -> FakePage {
    FakePage {
        scripts: Some(Arc::new(move |script: &str| {
            answers
                .iter()
                .find(|(asked, _)| *asked == script)
                .map(|(_, value)| (*value).to_owned())
        })),
        ..FakePage::default()
    }
}

#[tokio::test]
async fn a_converted_source_reads_pages_through_web_views() {
    let fake = FakeHost::shared();
    let address = serve(Router::new());
    let base = format!("http://{address}");
    fake.serve(
        &format!("{base}/history"),
        FakePage {
            title: "History".into(),
            posts: vec![("android".into(), r#"["One","Two"]"#.into())],
            ..script(&[("document.title", "\"History\"")])
        },
    );
    fake.serve(
        &format!("{base}/reader"),
        FakePage {
            title: "Reader".into(),
            requests: vec![format!("{base}/ads.js"), format!("{base}/app.js")],
            ..script(&[("window.token", "\"tok-1\"")])
        },
    );
    fake.serve(
        &base,
        script(&[("localStorage.getItem(\"secret\")", "\"s3cret\"")]),
    );

    let extension = extension(
        "eu.kanade.tachiyomi.extension.en.fixtureviews",
        "https://views.example",
    );
    let converted = convert::convert(
        &extension,
        &sources(
            VIEWS_BUILD,
            &[(
                "src/eu/kanade/tachiyomi/extension/en/fixtureviews/FixtureViews.kt",
                VIEWS_SOURCE,
            )],
        ),
    )
    .unwrap();
    assert!(converted.code.contains("KT.webView(self, { check: false"));
    assert!(converted.code.contains("KT.webView(self, { check: true"));
    let host = load_converted(&converted, &extension, address).await;
    let listing = host
        .manga_list(PROVIDER, crate::manga::MangaList::Popular, 1)
        .await
        .unwrap();
    let titles: Vec<&str> = listing
        .items
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(
        titles,
        ["One", "Two", "tok-1", "blocked 1", "s3cret", "timed out"]
    );

    // The views opened as the runs set them up, within the extension's permissions.
    let seen = fake.seen.lock().unwrap();
    let history = seen
        .opened
        .iter()
        .find(|open| open.user_agent == "Fixture/1.0")
        .expect("the history view");
    assert!(history.block_images && !history.intercept);
    assert_eq!(history.bridges[0].name, "android");
    assert_eq!(history.bridges[0].origins, std::slice::from_ref(&base));
    assert_eq!(history.hosts, ["127.0.0.1"]);
    assert_eq!(history.extension, "mihon.en.fixtureviews");
    assert!(
        history
            .challenge_hosts
            .iter()
            .any(|known| known == "challenges.cloudflare.com")
    );
    assert!(
        seen.opened
            .iter()
            .any(|open| open.intercept && open.user_agent == raygun_net::user_agent()),
        "the reader's view intercepts, with the User-Agent sources send"
    );
    drop(seen);
    let answered = fake.answered.lock().unwrap();
    let ads = answered
        .iter()
        .find(|(url, _)| url.ends_with("/ads.js"))
        .expect("the ad answered");
    assert_eq!(
        ads.1,
        Some(Answer {
            status: 403,
            reason: "Blocked".into(),
            mime: Some("text/plain".into()),
            encoding: Some("utf-8".into()),
            ..Answer::default()
        })
    );
    let app = answered
        .iter()
        .find(|(url, _)| url.ends_with("/app.js"))
        .expect("the script answered");
    assert_eq!(app.1, None, "left to the view");
}

#[tokio::test]
async fn a_view_that_meets_a_check_fails_as_one_and_checks_are_never_run() {
    let fake = FakeHost::shared();
    let address = serve(Router::new());
    let base = format!("http://{address}");
    fake.serve(
        &format!("{base}/guarded"),
        FakePage {
            title: "Just a moment...".into(),
            check: true,
            ..FakePage::default()
        },
    );
    let extension = extension(
        "eu.kanade.tachiyomi.extension.en.fixtureviews",
        "https://views.example",
    );
    let converted = convert::convert(
        &extension,
        &sources(
            VIEWS_BUILD,
            &[(
                "src/eu/kanade/tachiyomi/extension/en/fixtureviews/FixtureViews.kt",
                VIEWS_SOURCE,
            )],
        ),
    )
    .unwrap();
    let host = load_converted(&converted, &extension, address).await;

    // A page that turns out to be a check: the call fails as one, with the page to open by hand.
    let error = host
        .manga_list(PROVIDER, crate::manga::MangaList::Latest, 1)
        .await
        .unwrap_err();
    assert!(error.contains("browser check"), "{error}");
    let challenge = host
        .browser_check(&error, None)
        .expect("the check is noted for the person to pass");
    assert_eq!(challenge.request.url, format!("{base}/guarded"));
    assert_eq!(challenge.request.user_agent, raygun_net::user_agent());

    // A run that passes a check never opens a view.
    let error = host
        .call(
            PROVIDER,
            "search",
            serde_json::json!({ "query": "x", "page": 1 }),
        )
        .await
        .unwrap_err();
    assert_eq!(
        error.kind(),
        Some(raygun_net::IssueKind::BotCheck),
        "{error}"
    );
    let loaded_challenge =
        fake.seen.lock().unwrap().commands.iter().any(
            |command| matches!(command, Command::Load { url, .. } if url.ends_with("/challenge")),
        );
    assert!(!loaded_challenge);
}
