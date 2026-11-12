//! The runtime's side of Keiyoushi's `runWebView` (`kotlin.js`'s `webViewRun`): the native
//! `webView(op, args)` over the platform's hidden views (`crate::webview`). Each converted
//! extension opens one view at a time; a view loads with the extension's cookies for its page and
//! the User-Agent its requests carry, only what its network permissions allow, and hands its
//! cookies back to the extension's jar when it closes (as Mihon's WebView and OkHttp share one
//! store). A view whose run was abandoned (its call timed out) is closed at its deadline.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::time::Duration;

use reqwest::Url;
use reqwest::cookie::CookieStore as _;
use rquickjs::prelude::Async;
use rquickjs::{Ctx, Function, Object};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::browser_check::{CheckedRequest, parse_cookie_header};
use crate::cookies::CookieJar;
use crate::js::{err, ok};
use crate::permissions::NetPolicy;
use crate::webview::{
    self, Answer, Bridge, CHALLENGE_HOSTS, CHALLENGE_PATHS, Command, Event, Next, OpenView,
    StartScript, View,
};

/// What the runtime answers when the platform has no web views (`kotlin.js`'s `NO_VIEWS`).
const NO_VIEWS: &str = "[no-web-views] this device has no web views for sources";
/// Grace after a run's deadline before an abandoned view is closed.
const GRACE: Duration = Duration::from_secs(5);

/// Notes a browser check a view met, with the request to open for the person
/// (`crate::browser_check`), as the extension's requests note theirs.
pub(crate) type NoteCheck = Rc<dyn Fn(CheckedRequest, String)>;

/// The extension's views.
struct Views {
    extension_id: String,
    cookies: Arc<CookieJar>,
    policy: Arc<NetPolicy>,
    /// One view at a time for the extension.
    own: Arc<Semaphore>,
    open: RefCell<HashMap<String, Open>>,
    note_check: NoteCheck,
    /// Restarts the script's time budget after waiting (a wait isn't the script's work).
    restart: Rc<dyn Fn()>,
}

struct Open {
    view: Rc<View>,
    user_agent: String,
    _own: OwnedSemaphorePermit,
}

pub(crate) fn install<'js>(
    ctx: &Ctx<'js>,
    native: &Object<'js>,
    hooks: &super::runtime::Hooks,
) -> rquickjs::Result<()> {
    let views = Rc::new(Views {
        extension_id: hooks.extension_id.clone(),
        cookies: hooks.cookies.clone(),
        policy: hooks.policy.clone(),
        own: Arc::new(Semaphore::new(1)),
        open: RefCell::new(HashMap::new()),
        note_check: hooks.note_check.clone(),
        restart: hooks.restart.clone(),
    });
    native.set(
        "webView",
        Function::new(
            ctx.clone(),
            Async(move |op: String, args: String| {
                let views = views.clone();
                async move {
                    let answer = run(&views, &op, &args).await;
                    (views.restart)();
                    match answer {
                        Ok(value) => ok(value),
                        Err(message) => err(message),
                    }
                }
            }),
        )?,
    )?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenArgs {
    page: String,
    timeout_ms: f64,
    #[serde(default)]
    user_agent: Option<String>,
    #[serde(default = "yes")]
    javascript: bool,
    #[serde(default = "yes")]
    dom_storage: bool,
    #[serde(default)]
    block_images: bool,
    #[serde(default)]
    intercept: bool,
    #[serde(default)]
    scripts: Vec<StartScript>,
    #[serde(default)]
    bridges: Vec<Bridge>,
}

const fn yes() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ViewArgs {
    id: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    data: Option<String>,
    #[serde(default)]
    mime: Option<String>,
    #[serde(default)]
    script: Option<String>,
    #[serde(default)]
    wait_ms: Option<f64>,
    #[serde(default)]
    request: Option<u64>,
    #[serde(default)]
    response: Option<Answer>,
    #[serde(default)]
    reply: Option<u64>,
    #[serde(default)]
    message: Option<String>,
}

fn parse<T: serde::de::DeserializeOwned>(args: &str) -> Result<T, String> {
    serde_json::from_str(args).map_err(|error| format!("invalid web view arguments: {error}"))
}

/// Milliseconds as a wait, at most a run's longest.
fn wait_of(ms: Option<f64>) -> Duration {
    let ms = ms
        .filter(|ms| ms.is_finite() && *ms >= 0.0)
        .unwrap_or(1000.0);
    Duration::from_millis(ms as u64).min(webview::LONGEST_RUN)
}

async fn run(views: &Rc<Views>, op: &str, args: &str) -> Result<Value, String> {
    if op == "open" {
        return open(views, parse(args)?).await;
    }
    let args: ViewArgs = parse(args)?;
    let (view, user_agent) = {
        let open = views.open.borrow();
        let found = open.get(&args.id).ok_or("the web view was closed")?;
        (found.view.clone(), found.user_agent.clone())
    };
    let id = args.id.clone();
    match op {
        "load" => {
            let url = args.url.unwrap_or_default();
            views.allowed(&url)?;
            view.send(&Command::Load {
                id,
                url,
                headers: args.headers,
            })?;
            Ok(Value::Null)
        }
        "loadData" => {
            view.send(&Command::LoadData {
                id,
                base_url: args.base_url.unwrap_or_default(),
                data: args.data.unwrap_or_default(),
                mime: args.mime.unwrap_or_else(|| "text/html".to_owned()),
            })?;
            Ok(Value::Null)
        }
        "evaluate" => {
            let script = args.script.unwrap_or_default();
            let result = view.evaluate(&script, wait_of(args.wait_ms)).await?;
            Ok(json!(result))
        }
        "next" => match view.next(wait_of(args.wait_ms)).await {
            Next::Event(event) => {
                if let Event::Check { url } = &event {
                    let host = Url::parse(url)
                        .ok()
                        .and_then(|url| url.host_str().map(str::to_owned))
                        .unwrap_or_else(|| "the site".to_owned());
                    (views.note_check)(
                        CheckedRequest::new(url, "GET", [], None, &user_agent),
                        format!("{host} answered with a browser check in a web view"),
                    );
                }
                serde_json::to_value(event).map_err(|error| error.to_string())
            }
            Next::Idle => Ok(json!({ "kind": "idle" })),
            Next::Closed => Ok(json!({ "kind": "closed" })),
        },
        "answer" => {
            view.send(&Command::Answer {
                id,
                request: args.request.ok_or("no request to answer")?,
                response: args.response,
            })?;
            Ok(Value::Null)
        }
        "reply" => {
            view.send(&Command::Reply {
                id,
                reply: args.reply.ok_or("no message to answer")?,
                message: args.message.unwrap_or_default(),
            })?;
            Ok(Value::Null)
        }
        "stop" => {
            view.send(&Command::Stop { id })?;
            Ok(Value::Null)
        }
        "close" => {
            views.open.borrow_mut().remove(&id);
            let cookies = view.close().await;
            let taken = cookies
                .iter()
                .map(|found| views.keep_cookies(&found.url, &found.cookies))
                .sum::<usize>();
            Ok(json!({ "cookies": taken }))
        }
        _ => Err(format!("the web view has no {op}")),
    }
}

async fn open(views: &Rc<Views>, args: OpenArgs) -> Result<Value, String> {
    let host = webview::host().ok_or(NO_VIEWS)?;
    let timeout = webview::run_timeout(args.timeout_ms);
    let started = tokio::time::Instant::now();
    let own = tokio::time::timeout(timeout, views.own.clone().acquire_owned())
        .await
        .map_err(|_| "the source's other web view was open all that time".to_owned())?
        .map_err(|error| error.to_string())?;
    let turn = webview::turn(timeout.saturating_sub(started.elapsed())).await?;
    let left = timeout.saturating_sub(started.elapsed());
    let page = Url::parse(&args.page).ok();
    // The User-Agent the extension's requests to the page carry: a pass of a check there holds
    // only for it.
    let user_agent = args
        .user_agent
        .map(|agent| agent.trim().to_owned())
        .filter(|agent| !agent.is_empty())
        .or_else(|| {
            page.as_ref()
                .and_then(|page| views.cookies.user_agent_for(page))
        })
        .unwrap_or_else(raygun_net::user_agent);
    let cookies = page
        .as_ref()
        .and_then(|page| {
            let pairs = views.cookies.cookies(page)?;
            Some(webview::ViewCookies {
                url: page.to_string(),
                cookies: pairs.to_str().ok()?.to_owned(),
            })
        })
        .into_iter()
        .collect();
    let id = webview::new_id();
    let spec = OpenView {
        id: id.clone(),
        extension: views.extension_id.clone(),
        user_agent: user_agent.clone(),
        javascript: args.javascript,
        dom_storage: args.dom_storage,
        block_images: args.block_images,
        intercept: args.intercept,
        scripts: args.scripts,
        bridges: args.bridges,
        cookies,
        hosts: views
            .policy
            .patterns()
            .iter()
            .map(ToString::to_string)
            .collect(),
        challenge_hosts: CHALLENGE_HOSTS
            .iter()
            .map(|&host| host.to_owned())
            .collect(),
        challenge_paths: CHALLENGE_PATHS
            .iter()
            .map(|&path| path.to_owned())
            .collect(),
        timeout_ms: u64::try_from(left.as_millis()).unwrap_or(u64::MAX),
    };
    let view = Rc::new(View::open(host, spec, turn)?);
    views.open.borrow_mut().insert(
        id.clone(),
        Open {
            view,
            user_agent: user_agent.clone(),
            _own: own,
        },
    );
    // A run nobody waits for any more (its call timed out) leaves its view behind: closed once
    // its time is up.
    let weak: Weak<Views> = Rc::downgrade(views);
    let closing = id.clone();
    tokio::task::spawn_local(async move {
        tokio::time::sleep(left + GRACE).await;
        let Some(views) = weak.upgrade() else {
            return;
        };
        let left_open = views.open.borrow_mut().remove(&closing);
        if let Some(open) = left_open {
            tracing::debug!(extension = %views.extension_id, "a web view outlived its run");
            open.view.close().await;
        }
    });
    Ok(json!({ "id": id, "userAgent": user_agent }))
}

impl Views {
    /// Whether the view may load `url`: a web address the extension may reach, or a blank page.
    fn allowed(&self, url: &str) -> Result<(), String> {
        if url == "about:blank" {
            return Ok(());
        }
        let parsed = Url::parse(url).map_err(|_| format!("not a web address: {url}"))?;
        if self.policy.allows(&parsed) {
            Ok(())
        } else {
            Err(format!(
                "network access to {} is not permitted by the extension's manifest",
                parsed.host_str().unwrap_or(url)
            ))
        }
    }

    /// Keeps a view's cookies for `url` in the extension's jar, for the session (a view gives no
    /// expiry), leaving out those the jar sends there already. Returns how many it took.
    fn keep_cookies(&self, url: &str, header: &str) -> usize {
        let Ok(url) = Url::parse(url) else {
            return 0;
        };
        let known: Vec<String> = self
            .cookies
            .cookies(&url)
            .and_then(|value| value.to_str().ok().map(str::to_owned))
            .map(|value| value.split("; ").map(str::to_owned).collect())
            .unwrap_or_default();
        let lines: Vec<reqwest::header::HeaderValue> = parse_cookie_header(header)
            .into_iter()
            .filter(|cookie| !known.contains(&format!("{}={}", cookie.name, cookie.value)))
            .filter_map(|cookie| {
                let plain = |text: &str| !text.contains([';', '\r', '\n']);
                (plain(&cookie.name) && plain(&cookie.value)).then(|| {
                    reqwest::header::HeaderValue::from_str(&format!(
                        "{}={}; Path=/",
                        cookie.name, cookie.value
                    ))
                    .ok()
                })?
            })
            .collect();
        let taken = lines.len();
        if taken > 0 {
            self.cookies.set_cookies(&mut lines.iter(), &url);
        }
        taken
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn views() -> Rc<Views> {
        Rc::new(Views {
            extension_id: "x.y".into(),
            cookies: CookieJar::in_memory(),
            policy: Arc::new(NetPolicy::new(vec![
                crate::permissions::HostPattern::parse("comics.example").unwrap(),
            ])),
            own: Arc::new(Semaphore::new(1)),
            open: RefCell::new(HashMap::new()),
            note_check: Rc::new(|_, _| {}),
            restart: Rc::new(|| {}),
        })
    }

    #[test]
    fn views_load_only_what_the_extension_may_reach() {
        let views = views();
        assert!(views.allowed("https://comics.example/reader/1").is_ok());
        assert!(views.allowed("about:blank").is_ok());
        assert!(views.allowed("https://elsewhere.example/").is_err());
        assert!(views.allowed("file:///etc/passwd").is_err());
        assert!(views.allowed("javascript:alert(1)").is_err());
    }

    #[test]
    fn a_view_s_cookies_join_the_jar_for_the_session() {
        let views = views();
        let url = Url::parse("https://comics.example/").unwrap();
        views.cookies.set_cookies(
            &mut [reqwest::header::HeaderValue::from_static("kept=1; Path=/")].iter(),
            &url,
        );
        assert_eq!(
            views.keep_cookies("https://comics.example/a", "kept=1; token=abc; bad"),
            1
        );
        let sent = views.cookies.cookies(&url).unwrap();
        let sent = sent.to_str().unwrap();
        assert!(
            sent.contains("kept=1") && sent.contains("token=abc"),
            "{sent}"
        );
        assert_eq!(views.keep_cookies("not a url", "a=1"), 0);
    }
}
