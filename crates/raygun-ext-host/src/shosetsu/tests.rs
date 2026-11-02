use std::net::SocketAddr;

use axum::Router;
use axum::extract::Path;
use axum::http::Uri;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde_json::json;

use super::ShosetsuExtension;
use crate::host::ExtensionSource;
use crate::manifest::Manifest;

fn html(body: String) -> Response {
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], body).into_response()
}

fn serve() -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route(
            "/list",
            get(|uri: Uri| async move {
                let page: u32 = url::form_urlencoded::parse(uri.query().unwrap_or_default().as_bytes())
                    .find(|(key, _)| key == "page")
                    .and_then(|(_, page)| page.parse().ok())
                    .unwrap_or(0);
                let rows = if page > 2 {
                    String::new()
                } else {
                    format!(
                        r#"<div class="row"><h3 class="title"><a href="/novel/a{page}">Novel A{page}</a></h3><img src="/cover/a.jpg"></div>
                           <div class="row"><h3 class="title"><a href="/novel/b{page}">Novel B{page}</a></h3><img data-src="https://img.example/b.jpg"></div>"#
                    )
                };
                html(format!("<html><body><div class=list>{rows}</div></body></html>"))
            }),
        )
        .route(
            "/search",
            post(|body: String| async move {
                let query = url::form_urlencoded::parse(body.as_bytes())
                    .find(|(key, _)| key == "keyword")
                    .map(|(_, value)| value.into_owned())
                    .unwrap_or_default();
                html(format!(
                    r#"<div class="row"><h3 class="title"><a href="/novel/found">Found {query}</a></h3></div>"#
                ))
            }),
        )
        .route(
            "/novel/{id}",
            get(|Path(id): Path<String>| async move {
                html(format!(
                    r#"<html><body>
                    <h1 class="title">The Novel {id}</h1>
                    <div class="info">
                      <div><h3>Author:</h3><a href="/a/1">Yamada</a><a href="/a/2">Abe</a></div>
                      <div><h3>Status:</h3><a>Completed</a></div>
                      <div><h3>Genre:</h3><a>Fantasy</a>, <a>Drama</a></div>
                    </div>
                    <div class="desc"><p>An elf mage.</p><p>Outlives her party.</p></div>
                    <ul class="chapters">
                      <li><a href="/chapter/{id}-1">Chapter 1</a></li>
                      <li><a href="/chapter/{id}-2">Chapter 2</a></li>
                    </ul>
                    </body></html>"#
                ))
            }),
        )
        .route(
            "/chapter/{id}",
            get(|Path(id): Path<String>| async move {
                html(format!(
                    r#"<html><body><div id="content">
                      <h2>{id}</h2><p>It was a dark night.</p>
                      <div class="ads">BUY NOW</div><script>track()</script>
                      <p>The end.</p>
                    </div></body></html>"#
                ))
            }),
        );
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            axum::serve(listener, app).await.unwrap();
        });
    });
    address
}

/// A script written the way Shosetsu's official ones are.
fn script(base: &str) -> String {
    format!(
        r##"-- {{"id":9001,"ver":"1.0.0","libVer":"1.0.0","author":"test","dep":["helper>=1.0.0"]}}
local helper = Require("helper")
local baseURL = "{base}"

local function shrinkURL(url) return url:gsub("^" .. baseURL:gsub("%p", "%%%0"), "") end
local function expandURL(url) return baseURL .. url end

local function parseList(doc)
    return map(doc:select("div.row"), function(row)
        local link = row:selectFirst("h3.title a")
        local img = row:selectFirst("img")
        return Novel {{
            title = link:text(),
            link = shrinkURL(link:attr("abs:href")),
            imageURL = img and (img:hasAttr("data-src") and img:attr("data-src") or img:attr("abs:src")) or nil,
        }}
    end)
end

return {{
    id = 9001,
    name = "Test Source",
    baseURL = baseURL,
    chapterType = ChapterType.HTML,
    hasSearch = true,
    searchFilters = {{
        DropdownFilter(2, "Order", {{ "Newest", "Popular" }}),
        FilterList("More", {{ CheckboxFilter(3, "Completed only"), TextFilter(4, "Author") }}),
    }},
    listings = {{
        Listing("Popular", true, function(data)
            assert(data[2] == 0 and data[3] == false and data[4] == "", "filter defaults")
            return parseList(GETDocument(baseURL .. "/list?page=" .. data[PAGE]))
        end),
    }},
    search = function(data)
        local body = FormBodyBuilder():add("keyword", data[QUERY]):build()
        local headers = HeadersBuilder():add("X-Test", "1"):build()
        return parseList(RequestDocument(POST(baseURL .. "/search", headers, body)))
    end,
    parseNovel = function(url, loadChapters)
        local doc = GETDocument(expandURL(url))
        local info = doc:selectFirst(".info")
        local novel = NovelInfo {{
            title = doc:selectFirst("h1.title"):text(),
            description = helper.paragraphs(doc:selectFirst(".desc")),
            authors = map(info:select("div:has(h3:containsOwn(uthor)) > a"), function(v) return v:text() end),
            genres = map(info:select("div:has(h3:contains(enre)) > a"), helper.text),
            status = info:selectFirst("div:has(h3:containsOwn(tatus)) > a"):text() == "Completed"
                and NovelStatus.COMPLETED or NovelStatus.PUBLISHING,
        }}
        if loadChapters then
            local chapters = AsList(map(doc:select("ul.chapters a"), function(v, i)
                return NovelChapter {{ title = v:text(), link = shrinkURL(v:attr("abs:href")), order = i }}
            end))
            novel:setChapters(chapters)
        end
        return novel
    end,
    getPassage = function(url)
        local content = GETDocument(expandURL(url)):selectFirst("#content")
        content:select(".ads, script"):remove()
        -- Jsoup's builders (ReadFromNet's): a new element back, and text kept as text.
        content:appendElement("p"):appendText("Read on <next> & more")
        return pageOfElem(content, true)
    end,
    shrinkURL = shrinkURL,
    expandURL = expandURL,
}}
"##
    )
}

const HELPER: &str = r#"-- {"ver":"1.0.0"}
local helper = {}
function helper.text(v) return v:text() end
function helper.paragraphs(element)
    return table.concat(map(element:select("p"), helper.text), "\n")
end
return helper
"#;

async fn load(code: &str, files: &[(&str, &str)]) -> Result<ShosetsuExtension, String> {
    let manifest = Manifest::parse(
        &json!({
            "id": "shosetsu.en.test", "name": "Test Source", "version": "1.0.0", "apiVersion": 1,
            "runtime": "shosetsu", "entry": "index.lua", "provides": ["novel.source"],
            "permissions": { "net": ["*"] },
        })
        .to_string(),
    )
    .unwrap();
    let mut all = vec![("index.lua", code)];
    all.extend_from_slice(files);
    ShosetsuExtension::load_with(
        manifest,
        ExtensionSource::built_in(&all),
        true,
        std::time::Duration::from_secs(2),
    )
    .await
}

#[tokio::test(flavor = "multi_thread")]
async fn a_shosetsu_script_serves_novel_source_calls() {
    let address = serve();
    let base = format!("http://{address}");
    let extension = load(&script(&base), &[("lib/helper.lua", HELPER)])
        .await
        .unwrap();
    assert_eq!(extension.providers()[0].id, "shosetsu.en.test/novel");

    let popular = extension
        .call("novel", "popular", &json!({ "page": 1 }))
        .await
        .unwrap();
    let items = popular["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["id"], "/novel/a1");
    assert_eq!(items[0]["title"], "Novel A1");
    assert_eq!(items[0]["coverUrl"], format!("{base}/cover/a.jpg"));
    assert_eq!(items[1]["coverUrl"], "https://img.example/b.jpg");
    assert_eq!(popular["hasNextPage"], true);
    let past_the_end = extension
        .call("novel", "popular", &json!({ "page": 3 }))
        .await
        .unwrap();
    assert_eq!(past_the_end["hasNextPage"], false);

    let found = extension
        .call("novel", "search", &json!({ "query": "elf", "page": 1 }))
        .await
        .unwrap();
    assert_eq!(found["items"][0]["title"], "Found elf");

    let details = extension
        .call("novel", "details", &json!({ "id": "/novel/x" }))
        .await
        .unwrap();
    assert_eq!(details["title"], "The Novel x");
    assert_eq!(details["authors"], json!(["Yamada", "Abe"]));
    assert_eq!(details["genres"], json!(["Fantasy", "Drama"]));
    assert_eq!(details["status"], "completed");
    assert_eq!(details["description"], "An elf mage.\nOutlives her party.");
    assert_eq!(details["url"], format!("{base}/novel/x"));
    let chapters = extension
        .call("novel", "chapters", &json!({ "id": "/novel/x" }))
        .await
        .unwrap();
    assert_eq!(
        chapters["chapters"],
        json!([
            { "id": "/chapter/x-1", "name": "Chapter 1", "number": 0.0 },
            { "id": "/chapter/x-2", "name": "Chapter 2", "number": 1.0 },
        ])
    );

    let passage = extension
        .call("novel", "passage", &json!({ "chapterId": "/chapter/x-1" }))
        .await
        .unwrap();
    let html = passage["html"].as_str().unwrap();
    assert!(
        html.contains("It was a dark night.") && html.contains("The end."),
        "{html}"
    );
    assert!(
        !html.contains("BUY NOW") && !html.contains("track()"),
        "removed elements stay out"
    );
    assert!(
        html.contains("<p>Read on &lt;next&gt; &amp; more</p>"),
        "appendElement and appendText: {html}"
    );
    assert_eq!(passage["baseUrl"], format!("{base}/chapter/x-1"));
    let blocks = crate::novel::parse_passage(&passage);
    assert_eq!(blocks[0], crate::novel::PassageBlock::Heading("x-1".into()));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_sandbox_holds() {
    for (code, expected) in [
        ("return io.open('x')", "io"),
        ("return os.execute('ls')", "os"),
        ("return os.remove('x')", "os"),
        ("return debug.getinfo(1)", "debug"),
        ("return dofile('x')", "dofile"),
        ("return load(string.char(27) .. 'Lua')()", "binary"),
        ("while true do end", "too long"),
        (
            "local t = {} for i = 1, 1e9 do t[i] = ('x'):rep(100) end",
            "memory",
        ),
        ("return Require('../secret')", "Missing Library"),
        ("return { name = 'x' }", "listings"),
    ] {
        let error = match load(code, &[]).await {
            Ok(_) => panic!("{code} loaded"),
            Err(error) => error,
        };
        let lower = error.to_lowercase();
        assert!(
            lower.contains(&expected.to_lowercase())
                || lower.contains("attempt to index")
                || lower.contains("attempt to call")
                || lower.contains("not enough memory")
                || lower.contains("nil value"),
            "{code}: {error}"
        );
    }
}

#[test]
fn lua53_escapes_become_lua52_ones() {
    assert_eq!(
        super::runtime::lua52_for_tests(r#"x = "\u{200D}" .. "\\u{41}""#),
        r#"x = "\226\128\141" .. "\\u{41}""#,
        "an escaped backslash stays as it is"
    );
    assert_eq!(super::runtime::lua52_for_tests("plain"), "plain");
}
