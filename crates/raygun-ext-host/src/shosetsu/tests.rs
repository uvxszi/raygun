use std::net::SocketAddr;

use axum::Router;
use axum::extract::Path;
use axum::http::Uri;
use axum::http::header;
use axum::response::{Html, IntoResponse, Response};
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
        )
        // Bodies in another charset, as Russian and Japanese sites send them.
        .route(
            "/cp1251",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/html; charset=windows-1251")],
                    [b"<p>".as_slice(), &[0xCF, 0xF0, 0xE8, 0xE2, 0xE5, 0xF2], b"</p>"].concat(),
                )
                    .into_response()
            }),
        )
        // A bot check in the site's place.
        .route(
            "/guarded",
            get(|| async {
                (
                    axum::http::StatusCode::FORBIDDEN,
                    [(header::SERVER, "cloudflare")],
                    Html("<title>Just a moment...</title><script src=\"/cdn-cgi/challenge-platform/x\"></script>"),
                )
                    .into_response()
            }),
        )
        .route(
            "/api",
            get(|headers: axum::http::HeaderMap| async move {
                let agreed = headers
                    .get("x-agreed")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("no")
                    .to_owned();
                (
                    [(header::CONTENT_TYPE, "application/json")],
                    json!({ "agreed": agreed, "items": [{ "title": "Über", "n": 3 }, null] }).to_string(),
                )
                    .into_response()
            }),
        )
        .route(
            "/login",
            get(|| async {
                (
                    [(header::SET_COOKIE, "session=abc123; Path=/")],
                    Html("<p>in</p>"),
                )
                    .into_response()
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
    load_set(code, files, std::collections::BTreeMap::new()).await
}

async fn load_set(
    code: &str,
    files: &[(&str, &str)],
    settings: std::collections::BTreeMap<String, serde_json::Value>,
) -> Result<ShosetsuExtension, String> {
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
        crate::cookies::CookieJar::in_memory(),
        settings,
        true,
        std::time::Duration::from_secs(2),
    )
    .await
}

/// A script's settings model shows as extension settings, and the stored values reach it.
#[tokio::test(flavor = "multi_thread")]
async fn script_settings_are_extension_settings() {
    let code = r#"-- {"id":9003,"ver":"1.0.0","libVer":"1.0.0"}
local settings = {}
return {
    id = 9003, name = "Settings", baseURL = "https://example.org",
    settings = {
        DropdownFilter(1, "Language", { "English", "Español" }),
        SwitchFilter(2, "Hide notes"),
        TextFilter(3, "Token"),
    },
    updateSetting = function(id, value) settings[id] = value end,
    listings = { Listing("All", false, function()
        return { Novel { title = tostring(settings[1]) .. " " .. tostring(settings[2])
            .. " [" .. tostring(settings[3]) .. "]", link = "/x" } }
    end) },
    getPassage = function() return "" end,
    parseNovel = function() return NovelInfo {} end,
}"#;
    let mut stored = std::collections::BTreeMap::new();
    stored.insert("1".to_owned(), json!("Español"));
    stored.insert("2".to_owned(), json!(true));
    stored.insert("9".to_owned(), json!("gone"));
    let extension = load_set(code, &[], stored).await.unwrap();
    let keys: Vec<&str> = extension
        .manifest()
        .settings
        .iter()
        .map(|setting| setting.key.as_str())
        .collect();
    assert_eq!(keys, ["1", "2", "3"]);
    assert_eq!(
        extension.manifest().settings[0].options,
        ["English", "Español"]
    );
    assert_eq!(
        extension.user_settings().keys().collect::<Vec<_>>(),
        ["1", "2"],
        "a value for a setting the script no longer has is dropped"
    );
    let listing = extension
        .call("novel", "popular", &json!({ "page": 1 }))
        .await
        .unwrap();
    assert_eq!(
        listing["items"][0]["title"], "1 true []",
        "the index, the flag, the default"
    );
}

/// A listing keeps the authors a script names for its novels (most name none).
#[tokio::test(flavor = "multi_thread")]
async fn listings_keep_the_authors_a_script_names() {
    let code = r#"-- {"id":9010,"ver":"1.0.0","libVer":"1.0.0"}
return {
    id = 9010, name = "Authors", baseURL = "https://example.org",
    listings = { Listing("All", false, function()
        return {
            Novel { title = "Emma", link = "/emma", authors = { "Jane Austen" } },
            Novel { title = "Beowulf", link = "/beowulf" },
        }
    end) },
    getPassage = function() return "" end,
    parseNovel = function() return NovelInfo {} end,
}"#;
    let extension = load(code, &[]).await.unwrap();
    let listing = extension
        .call("novel", "popular", &json!({ "page": 1 }))
        .await
        .unwrap();
    assert_eq!(listing["items"][0]["authors"], json!(["Jane Austen"]));
    assert_eq!(listing["items"][1]["authors"], json!([]));
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
    // Newest first, as the app's "source order" expects; reading follows the numbers.
    assert_eq!(
        chapters["chapters"],
        json!([
            { "id": "/chapter/x-2", "name": "Chapter 2", "number": 1.0, "dateUpload": null },
            { "id": "/chapter/x-1", "name": "Chapter 1", "number": 0.0, "dateUpload": null },
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
        ("return Require('../secret')", "needs the library ../secret"),
        ("return { name = 'x' }", "listings"),
        ("return loadstring(string.char(27) .. 'Lua')()", "binary"),
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
    // LuaJ reads an unknown escape as the letter (Lua 5.1's rule); Lua 5.2 refuses it.
    assert_eq!(
        super::runtime::lua52_for_tests(r#"s:match("^([^\s]+)") -- "\s" [[\s]]"#),
        r#"s:match("^([^s]+)") -- "\s" [[\s]]"#,
        "comments and long strings stay as they are"
    );
    assert_eq!(
        super::runtime::lua52_for_tests(r#"a = [==[ "\q" ]==] .. '\q\'\n\065'"#),
        r#"a = [==[ "\q" ]==] .. 'q\'\n\065'"#
    );
    let lua = mlua::Lua::new();
    let text: String = lua
        .load(super::runtime::lua52_for_tests(r#"return "a\sb\%c""#))
        .eval()
        .unwrap();
    assert_eq!(text, "asb%c");
}

/// kotlin-lib's filters have a `state`: scripts set defaults with `setState`.
#[tokio::test(flavor = "multi_thread")]
async fn filter_states_are_the_defaults() {
    let code = r#"-- {"id":9004,"ver":"1.0.0","libVer":"1.0.0"}
local order = DropdownFilter(2, "Order", { "Newest", "Oldest", "Rating" })
order:setState(2)
local adult = CheckboxFilter(3, "Adult")
adult:setState(true)
return {
    id = 9004, name = "States", baseURL = "https://example.org",
    searchFilters = { order, adult, TextFilter(4, "Author") },
    listings = { Listing("All", false, function(data)
        return { Novel { title = tostring(data[2]) .. " " .. tostring(data[3]) .. " "
            .. tostring(order:getState()) .. " " .. tostring(TextFilter(9, "x"):getState() == ""), link = "/x" } }
    end) },
    getPassage = function() return "" end,
    parseNovel = function() return NovelInfo {} end,
}"#;
    let extension = load(code, &[]).await.unwrap();
    let listing = extension
        .call("novel", "popular", &json!({ "page": 1 }))
        .await
        .unwrap();
    assert_eq!(listing["items"][0]["title"], "2 true 2 true");
    let filters = extension
        .call("novel", "filters", &json!({}))
        .await
        .unwrap();
    let parsed = crate::manga::parse_filters(&filters);
    assert!(matches!(
        &parsed[0],
        raygun_model::SourceFilter::Select { default: Some(default), .. } if default == "2"
    ));
    assert!(matches!(
        &parsed[1],
        raygun_model::SourceFilter::Toggle { default: true, .. }
    ));
}

/// A script using what kotlin-lib gives beyond the basics: listings picked by name, filters, an
/// OkHttp client with an interceptor, `HttpUrl`, the cookie jar, dkjson, `os.date`, a string
/// chapter type, and a site answering with a bot check.
fn script_two(base: &str) -> String {
    format!(
        r##"-- {{"id":9002,"ver":"1.0.0","libVer":"1.0.0","dep":["dkjson>=1.0.0"]}}
local json = Require("dkjson")
local baseURL = "{base}"

local agree = Interceptor(function(chain)
    local request = chain:request()
    local url = request:url():newBuilder():addQueryParameter("agree", "1"):build()
    return chain:proceed(request:newBuilder():url(url):header("X-Agreed", "yes"):build())
end)
local client = HttpClientBuilder():addInterceptor(agree):connectTimeout(10):build()

local function describe(data)
    return "order=" .. tostring(data[2]) .. " action=" .. tostring(data[10])
        .. " drama=" .. tostring(data[11]) .. " mature=" .. tostring(data[20])
        .. " author=" .. tostring(data[30]) .. " query=" .. tostring(data[QUERY])
        .. " page=" .. tostring(data[PAGE])
end

return {{
    id = 9002,
    name = "Second Source",
    baseURL = baseURL,
    chapterType = ChapterType.STRING,
    searchFilters = {{
        DropdownFilter(2, "Order", {{ "Newest", "Rating" }}),
        FilterGroup("Genres", {{ CheckboxFilter(10, "Action"), CheckboxFilter(11, "Drama") }}),
        TriStateFilter(20, "Mature"),
        TextFilter(30, "Author"),
    }},
    listings = {{
        Listing("Latest", true, function(data)
            return {{ Novel {{ title = "latest " .. describe(data), link = "/latest" }} }}
        end),
        -- A site that ignores the page number: the same page every time.
        Listing("Popular", true, function(data)
            return {{ Novel {{ title = describe(data), link = "/same" }} }}
        end),
    }},
    search = function(data)
        local response = client:newCall(GET(baseURL .. "/api")):execute()
        local answer = json.decode(response:body():string())
        local seen = response:request():url():queryParameter("agree")
        return {{ Novel {{ title = answer.agreed .. " " .. tostring(seen) .. " " .. answer.items[1].title
            .. " " .. describe(data), link = "/found" }} }}
    end,
    parseNovel = function(url)
        GETDocument(baseURL .. "/login")
        local cookies = CookieJar():loadForRequest(HttpUrl(baseURL))
        local session = cookies:size() > 0 and cookies:get(0):value() or "none"
        local text = GETDocument(baseURL .. "/cp1251"):selectFirst("p"):text()
        return NovelInfo {{
            title = text,
            description = session .. " " .. os.date("!%Y-%m-%d", 86400),
            chapters = {{
                NovelChapter {{ title = "One", link = "one", release = "2024-05-01" }},
                NovelChapter {{ title = "Guarded", link = "guarded" }},
            }},
        }}
    end,
    getPassage = function(url)
        if url == "guarded" then return GETDocument(baseURL .. "/guarded"):text() end
        if url == "probe" then
            local f, err = (loadstring or load)(string.char(27) .. "LuaR\0", "x")
            return tostring(f) .. " " .. tostring(err)
        end
        return "First line\nSecond <line> & more\n\nThird line"
    end,
    expandURL = function(url) return baseURL .. "/" .. url end,
}}
"##
    )
}

/// dkjson's shape, with a decoder that only says it ran: the native one should answer.
const FAKE_DKJSON: &str = r#"-- {"ver":"1.0.1"}
local json = { version = "dkjson 2.5" }
function json.decode() return { agreed = "lua", items = { { title = "lua" } } } end
return json
"#;

#[tokio::test(flavor = "multi_thread")]
async fn shosetsu_calls_follow_the_app() {
    let address = serve();
    let base = format!("http://{address}");
    let extension = load(&script_two(&base), &[("lib/dkjson.lua", FAKE_DKJSON)])
        .await
        .unwrap();
    let call = |method: &'static str, args: serde_json::Value| {
        let extension = &extension;
        async move { extension.call("novel", method, &args).await }
    };

    // "Popular" is the listing named so, not the first; listings get no query, and the filters'
    // defaults.
    let popular = call("popular", json!({ "page": 1 })).await.unwrap();
    assert_eq!(
        popular["items"][0]["title"],
        "order=0 action=false drama=false mature=0 author= query=nil page=1"
    );
    assert_eq!(popular["hasNextPage"], true);
    // The site repeats its page: paging stops there.
    let again = call("popular", json!({ "page": 2 })).await.unwrap();
    assert_eq!(again["items"], json!([]));
    assert_eq!(again["hasNextPage"], false);
    let latest = call("latest", json!({ "page": 1 })).await.unwrap();
    assert!(
        latest["items"][0]["title"]
            .as_str()
            .unwrap()
            .starts_with("latest ")
    );

    // The filters as the app shows them.
    let filters = call("filters", json!({})).await.unwrap();
    let parsed = crate::manga::parse_filters(&filters);
    let keys: Vec<String> = parsed
        .iter()
        .map(|filter| match filter {
            raygun_model::SourceFilter::Select { key, .. }
            | raygun_model::SourceFilter::Tags { key, .. }
            | raygun_model::SourceFilter::Toggle { key, .. } => key.clone(),
        })
        .collect();
    assert_eq!(keys, ["listing", "2", "g10", "20"]);
    assert!(matches!(
        &parsed[0],
        raygun_model::SourceFilter::Select { default: Some(default), .. } if default == "2"
    ));

    // Chosen filters reach the script by id; an empty search shows the chosen listing.
    let chosen = json!({
        "listing": "1", "2": "1",
        "g10": { "include": ["11"], "exclude": [] },
        "20": { "include": [], "exclude": ["20"] },
    });
    let filtered = call(
        "search",
        json!({ "query": "", "page": 1, "filters": chosen }),
    )
    .await
    .unwrap();
    assert_eq!(
        filtered["items"][0]["title"],
        "latest order=1 action=false drama=true mature=2 author= query=nil page=1"
    );

    // A search through the script's own client: the interceptor's header and query, dkjson.
    let found = call("search", json!({ "query": "elf", "page": 1 }))
        .await
        .unwrap();
    assert_eq!(
        found["items"][0]["title"],
        "yes 1 Über order=0 action=false drama=false mature=0 author= query=elf page=1"
    );

    // Cookies the site set, a windows-1251 page, os.date, chapter dates, newest first.
    let details = call("details", json!({ "id": "/n" })).await.unwrap();
    assert_eq!(details["title"], "Привет");
    assert_eq!(details["description"], "abc123 1970-01-02");
    let chapters = call("chapters", json!({ "id": "/n" })).await.unwrap();
    assert_eq!(
        chapters["chapters"][1]["id"], "one",
        "unnumbered: the list reversed"
    );
    assert_eq!(chapters["chapters"][1]["dateUpload"], 1_714_521_600_000_i64);

    // A string chapter: a paragraph per line, as Shosetsu shows it.
    let passage = call("passage", json!({ "chapterId": "one" }))
        .await
        .unwrap();
    assert_eq!(
        crate::novel::parse_passage(&passage),
        [
            crate::novel::PassageBlock::Paragraph("First line".into()),
            crate::novel::PassageBlock::Paragraph("Second <line> & more".into()),
            crate::novel::PassageBlock::Paragraph("Third line".into()),
        ]
    );
    // Text-only loading, whichever loader a script reaches for.
    let probe = call("passage", json!({ "chapterId": "probe" }))
        .await
        .unwrap();
    let probe = probe["html"].as_str().unwrap();
    assert!(probe.contains("nil") && probe.contains("mode"), "{probe}");

    // A bot check says so, in words.
    let guarded = call("passage", json!({ "chapterId": "guarded" }))
        .await
        .unwrap_err();
    assert!(
        guarded.starts_with("Test Source wants a browser check"),
        "{guarded}"
    );
}

#[test]
fn jsoup_edits_work_on_the_tree() {
    let lua = mlua::Lua::new();
    super::dom::install(&lua, &lua.globals()).unwrap();
    let results: Vec<String> = lua
        .load(
            r##"
            local doc = Document([[<div id="c"><p class="Note">One<br>two</p><img data-src="/a.png"><span>x</span></div>
                <div id="other"><b>moved</b></div>]])
            local c = doc:selectFirst("#c")
            local img = c:selectFirst("img")
            img:attr("src", "https://img.example" .. img:attr("data-src")):removeAttr("data-src")
            c:selectFirst("p"):addClass("seen")
            local span = c:selectFirst("span")
            span:text("<b>not a tag</b>")
            c:select("p"):after("<hr>")
            c:appendChild(doc:selectFirst("#other b"))
            local made = doc:createElement("em")
            made:text("new")
            c:prependChild(made)
            local foreign = Document("<i>far</i>"):selectFirst("i")
            c:appendChild(foreign)
            return {
                img:outerHtml(),
                tostring(c:selectFirst(".note") ~= nil),
                c:selectFirst("p"):attr("class"),
                span:html(),
                c:selectFirst("p"):wholeText(),
                tostring(#doc:select("#other b")),
                c:child(0):text(),
                c:selectLast("i"):text(),
                c:selectFirst("img"):lastElementSibling():tagName(),
                tostring(c:selectFirst("b"):parents():size()),
                tostring(c:selectFirst("p") == doc:selectFirst("#c > p")),
                tostring(c:selectFirst("[src~=(?i)\\.PNG$]") ~= nil),
                tostring(c:selectFirst("[^data-]") == nil),
                Document("<script>var a = 1</script><p>text</p>"):data(),
            }
            "##,
        )
        .eval()
        .unwrap();
    assert_eq!(
        results,
        [
            r#"<img src="https://img.example/a.png">"#,
            "true",
            "Note seen",
            "&lt;b&gt;not a tag&lt;/b&gt;",
            "One\ntwo",
            "0",
            "new",
            "far",
            "i",
            "3",
            "true",
            "true",
            "true",
            "var a = 1",
        ]
    );
}

#[test]
fn chapters_and_dates_read_as_the_app_needs() {
    let lua = mlua::Lua::new();
    let numbered = super::runtime::chapters_for_tests(
        &lua,
        r#"return {
            { link = "b", title = "B", order = 2 },
            { link = "a", title = "A", order = "1" },
            { link = "c", title = "C", order = 3 },
        }"#,
    );
    let ids: Vec<&str> = numbered
        .iter()
        .map(|chapter| chapter["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["c", "b", "a"], "by order, newest first");
    for (text, expected) in [
        ("2024-05-01", Some(1_714_521_600_000)),
        ("2024-05-01 00:00:10", Some(1_714_521_610_000)),
        ("01.05.2024", Some(1_714_521_600_000)),
        ("May 1, 2024", Some(1_714_521_600_000)),
        ("1714521600", Some(1_714_521_600_000)),
        ("5 days ago", None),
    ] {
        assert_eq!(super::runtime::release_for_tests(text), expected, "{text}");
    }
    assert_eq!(
        super::runtime::lines_as_html_for_tests("a\n\n b < c \r\n"),
        "<p>a</p><p>b &lt; c</p>"
    );
}
