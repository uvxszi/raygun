//! Hidden web views for converted Mihon sources (Keiyoushi's `runWebView { … }`; the design is in
//! `docs/extensions/formats/mihon.md`, "Web views").
//!
//! A source that reads a page a script draws (a token a page computes, its local storage, data a
//! reader's script decodes) drives a web view of its own. The platform's views are apart from
//! Raygun: their own browser data, no bridge to Raygun's pages or IPC, only the message objects the
//! source named (`webMessageBridge`) for the origins it named. They load only what the extension's
//! network permissions allow (`*` is any public host, never this device or the local network) and
//! never a browser check's provider (Turnstile, hCaptcha, reCAPTCHA, DataDome, Arkose):
//! Raygun never solves a check. A page that turns out to be a check ends the run as a browser
//! check, which the person may pass by hand (`crate::browser_check`), as other sources' do.
//!
//! The platform host ([`ViewHost`]: a hidden WebView in its own process on Android, a hidden
//! window on Windows) takes [`Command`]s and answers with [`Event`]s through [`deliver`]. The
//! runtime (`mihon/runtime.rs`'s `webView` native) opens a [`View`] per run, at most one per
//! extension at a time and [`MOST_VIEWS`] in all, each for [`LONGEST_RUN`] at most.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, PoisonError, RwLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot};

/// Most views open at once, all extensions together.
pub const MOST_VIEWS: usize = 3;
/// The longest a run may last, whatever the source asks.
pub const LONGEST_RUN: Duration = Duration::from_secs(60);
/// The shortest timeout a run gets.
pub const SHORTEST_RUN: Duration = Duration::from_secs(1);
/// How long a closing view has to hand back its cookies.
const CLOSING: Duration = Duration::from_secs(3);

/// Hosts of browser checks' providers: a view never loads them (their requests fail), so no
/// check is ever passed without the person. Subdomains count.
pub const CHALLENGE_HOSTS: &[&str] = &[
    "challenges.cloudflare.com",
    "hcaptcha.com",
    "recaptcha.net",
    "captcha-delivery.com",
    "arkoselabs.com",
    "funcaptcha.com",
    "perimeterx.net",
    "px-cloud.net",
    "px-cdn.net",
];

/// Paths of browser checks' scripts on any host (Cloudflare's challenge platform, Google's
/// reCAPTCHA): never loaded either.
pub const CHALLENGE_PATHS: &[&str] = &["/cdn-cgi/challenge-platform/", "/recaptcha/"];

/// What the platform's views do: they take commands, and answer with events through [`deliver`].
pub trait ViewHost: Send + Sync {
    /// Sends a command; an error means the platform couldn't take it (its views aren't running).
    fn send(&self, command: &Command) -> Result<(), String>;
}

static HOST: RwLock<Option<Arc<dyn ViewHost>>> = RwLock::new(None);

/// Installs the platform's views (the app does, at start). Replaces the ones before.
pub fn set_host(host: Arc<dyn ViewHost>) {
    *HOST.write().unwrap_or_else(PoisonError::into_inner) = Some(host);
}

/// The platform's views, when the app installed some.
pub fn host() -> Option<Arc<dyn ViewHost>> {
    HOST.read().unwrap_or_else(PoisonError::into_inner).clone()
}

/// A view to open, with what the run set up before its first load.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenView {
    pub id: String,
    /// The extension it's for: its views share their browser data (cookies, storage), no one
    /// else's do.
    pub extension: String,
    /// The User-Agent it loads with: the source's, else the one a check on the site was passed
    /// with, else the one sources send (`raygun_net::user_agent`).
    pub user_agent: String,
    pub javascript: bool,
    pub dom_storage: bool,
    pub block_images: bool,
    /// The page's requests come to the source to answer (`interceptRequest`).
    pub intercept: bool,
    /// Scripts run before a document's own (`injectAtDocumentStart`).
    pub scripts: Vec<StartScript>,
    /// Message objects the page posts to (`webMessageBridge`).
    pub bridges: Vec<Bridge>,
    /// The extension's cookies for the page's address, to send with it.
    pub cookies: Vec<ViewCookies>,
    /// Hosts it may load from: the extension's network permissions (`*` is any public host).
    pub hosts: Vec<String>,
    /// Never loaded: [`CHALLENGE_HOSTS`].
    pub challenge_hosts: Vec<String>,
    /// Never loaded: [`CHALLENGE_PATHS`].
    pub challenge_paths: Vec<String>,
    /// The platform closes the view by then whatever happens, ms.
    pub timeout_ms: u64,
}

/// A script and the origins it runs in (`https://site.example`, `*` for every one).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartScript {
    pub script: String,
    pub origins: Vec<String>,
}

/// A message object (`window.<name>.postMessage(text)`) and the origins that may post to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bridge {
    pub name: String,
    pub origins: Vec<String>,
}

/// Cookies for an address, as a `Cookie` header's pairs (`a=1; b=2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewCookies {
    pub url: String,
    pub cookies: String,
}

/// What the source answers an intercepted request with (`WebResourceResponse`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    /// 0 when the source gave none (a 200).
    #[serde(default)]
    pub status: u16,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub mime: Option<String>,
    #[serde(default)]
    pub encoding: Option<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    /// The body, base64.
    #[serde(default)]
    pub body: String,
    /// No body at all (`WebResourceResponse(null, null, null)`): the load fails.
    #[serde(default)]
    pub no_body: bool,
}

/// A command for the platform's views, as JSON `{"op": …, "id": …, …}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Command {
    Open(OpenView),
    Load {
        id: String,
        url: String,
        headers: BTreeMap<String, String>,
    },
    /// A page given as text, with `base_url` as its address (`loadData`).
    LoadData {
        id: String,
        base_url: String,
        data: String,
        mime: String,
    },
    /// Runs a script in the page; answers [`Event::Evaluated`] with its value as JSON.
    Evaluate {
        id: String,
        call: u64,
        script: String,
    },
    /// The source's answer to [`Event::Request`]: `None` lets the view load it itself.
    Answer {
        id: String,
        request: u64,
        response: Option<Answer>,
    },
    /// Posts a message back to the frame that sent [`Event::Message`].
    Reply {
        id: String,
        reply: u64,
        message: String,
    },
    Stop {
        id: String,
    },
    /// Closes the view; answers [`Event::Closed`] with its cookies.
    Close {
        id: String,
    },
}

/// What a view reports, as JSON `{"id": …, "kind": …, …}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Event {
    Started {
        url: String,
    },
    Finished {
        url: String,
    },
    Title {
        title: String,
    },
    /// A load failed: the page's (`main_frame`) or one of its resources'.
    Failed {
        url: String,
        #[serde(default)]
        code: i32,
        #[serde(default)]
        description: String,
        #[serde(default)]
        main_frame: bool,
    },
    /// The page's answer, when it failed (4xx, 5xx) or said it's a check (`cf-mitigated`).
    Answer {
        url: String,
        status: u16,
        #[serde(default)]
        mitigated: bool,
    },
    /// A message the page posted to a bridge.
    Message {
        bridge: String,
        message: String,
        #[serde(default)]
        origin: Option<String>,
        #[serde(default)]
        main_frame: Option<bool>,
        /// What [`Command::Reply`] answers through.
        #[serde(default)]
        reply: Option<u64>,
    },
    /// A request the page makes, for the source to answer ([`Command::Answer`]).
    Request {
        request: u64,
        url: String,
        method: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
        #[serde(default)]
        main_frame: bool,
    },
    Evaluated {
        call: u64,
        #[serde(default)]
        result: Option<String>,
    },
    /// The page's renderer went (crashed, or the system took it).
    Gone {
        #[serde(default)]
        crashed: bool,
    },
    Closed {
        #[serde(default)]
        cookies: Vec<ViewCookies>,
    },
    /// The page is a browser check (told by the host from what the view saw): the run ends.
    Check {
        url: String,
    },
}

#[derive(Debug, Deserialize)]
struct Delivered {
    id: String,
    #[serde(flatten)]
    event: Event,
}

/// Where a view's events go.
struct Slot {
    events: mpsc::UnboundedSender<Event>,
    evaluations: HashMap<u64, oneshot::Sender<Option<String>>>,
    closed: Option<oneshot::Sender<Vec<ViewCookies>>>,
}

static OPEN: LazyLock<Mutex<HashMap<String, Slot>>> = LazyLock::new(Default::default);
static PERMITS: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(MOST_VIEWS)));

fn open_views() -> std::sync::MutexGuard<'static, HashMap<String, Slot>> {
    OPEN.lock().unwrap_or_else(PoisonError::into_inner)
}

/// An event from the platform's views (`{"id", "kind", …}` JSON). Events of views no longer
/// open are dropped.
pub fn deliver(json: &str) {
    match serde_json::from_str::<Delivered>(json) {
        Ok(delivered) => deliver_event(&delivered.id, delivered.event),
        Err(error) => tracing::debug!(%error, "a web view report Raygun can't read"),
    }
}

/// One of a view's events.
pub fn deliver_event(id: &str, event: Event) {
    let mut open = open_views();
    let Some(slot) = open.get_mut(id) else {
        return;
    };
    match event {
        Event::Evaluated { call, result } => {
            if let Some(waiting) = slot.evaluations.remove(&call) {
                let _ = waiting.send(result);
            }
        }
        Event::Closed { cookies } => {
            if let Some(waiting) = slot.closed.take() {
                let _ = waiting.send(cookies);
            }
        }
        event => {
            let _ = slot.events.send(event);
        }
    }
}

/// A turn to open a view: one of [`MOST_VIEWS`], waited for until `wait`.
pub async fn turn(wait: Duration) -> Result<OwnedSemaphorePermit, String> {
    tokio::time::timeout(wait, PERMITS.clone().acquire_owned())
        .await
        .map_err(|_| "other sources' web views were open all that time".to_owned())?
        .map_err(|error| error.to_string())
}

/// What [`View::next`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    Event(Event),
    /// Nothing came in the time given.
    Idle,
    /// The view is gone.
    Closed,
}

/// An open view: its events, and what it saw of its pages (to tell a check).
pub struct View {
    id: String,
    host: Arc<dyn ViewHost>,
    events: tokio::sync::Mutex<mpsc::UnboundedReceiver<Event>>,
    /// A check was reported already.
    checked: AtomicBool,
    calls: AtomicU64,
    closed: AtomicBool,
    /// Main-frame pages it loaded, for their cookies.
    pages: Mutex<Vec<String>>,
    _turn: OwnedSemaphorePermit,
}

impl std::fmt::Debug for View {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("View").field("id", &self.id).finish()
    }
}

impl View {
    /// Opens `spec` on `host` with a `turn` ([`turn`]).
    pub fn open(
        host: Arc<dyn ViewHost>,
        spec: OpenView,
        turn: OwnedSemaphorePermit,
    ) -> Result<Self, String> {
        let (sender, receiver) = mpsc::unbounded_channel();
        let id = spec.id.clone();
        open_views().insert(
            id.clone(),
            Slot {
                events: sender,
                evaluations: HashMap::new(),
                closed: None,
            },
        );
        if let Err(error) = host.send(&Command::Open(spec)) {
            open_views().remove(&id);
            return Err(error);
        }
        Ok(Self {
            id,
            host,
            events: tokio::sync::Mutex::new(receiver),
            checked: AtomicBool::new(false),
            calls: AtomicU64::new(0),
            closed: AtomicBool::new(false),
            pages: Mutex::new(Vec::new()),
            _turn: turn,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// Sends a command of this view's.
    pub fn send(&self, command: &Command) -> Result<(), String> {
        if self.closed.load(Ordering::Acquire) {
            return Err("the web view was closed".to_owned());
        }
        self.host.send(command)
    }

    /// Runs `script` in the page: its value as JSON, `None` when the page went before answering.
    pub async fn evaluate(&self, script: &str, wait: Duration) -> Result<Option<String>, String> {
        let call = self.calls.fetch_add(1, Ordering::AcqRel) + 1;
        let (sender, receiver) = oneshot::channel();
        match open_views().get_mut(&self.id) {
            Some(slot) => slot.evaluations.insert(call, sender),
            None => return Ok(None),
        };
        self.send(&Command::Evaluate {
            id: self.id.clone(),
            call,
            script: script.to_owned(),
        })?;
        match tokio::time::timeout(wait, receiver).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(_)) => Ok(None),
            Err(_) => {
                if let Some(slot) = open_views().get_mut(&self.id) {
                    slot.evaluations.remove(&call);
                }
                Err("the page's script didn't answer in time".to_owned())
            }
        }
    }

    /// The next event, waiting up to `wait`. A page that is a browser check comes as
    /// [`Event::Check`] once, in place of the event that told it.
    pub async fn next(&self, wait: Duration) -> Next {
        let mut events = self.events.lock().await;
        let event = match tokio::time::timeout(wait, events.recv()).await {
            Ok(Some(event)) => event,
            Ok(None) => return Next::Closed,
            Err(_) => return Next::Idle,
        };
        drop(events);
        Next::Event(self.saw(event))
    }

    /// Notes what a page event tells; a check, when it shows one: a check's answer
    /// (`cf-mitigated`, in any language) or a check's title.
    fn saw(&self, event: Event) -> Event {
        if let Event::Started { url } = &event
            && let Ok(mut pages) = self.pages.lock()
            && !pages.contains(url)
            && pages.len() < 32
        {
            pages.push(url.clone());
        }
        if shows_check(&event) && !self.checked.swap(true, Ordering::AcqRel) {
            let url = match &event {
                Event::Answer { url, .. } => url.clone(),
                _ => self
                    .pages
                    .lock()
                    .ok()
                    .and_then(|pages| pages.last().cloned())
                    .unwrap_or_default(),
            };
            return Event::Check { url };
        }
        event
    }

    /// Main-frame pages it loaded.
    pub fn pages(&self) -> Vec<String> {
        self.pages
            .lock()
            .map(|pages| pages.clone())
            .unwrap_or_default()
    }

    /// Closes the view and hands back its cookies for the pages it loaded (none when it was
    /// closed already, or doesn't answer in time).
    pub async fn close(&self) -> Vec<ViewCookies> {
        if self.closed.swap(true, Ordering::AcqRel) {
            return Vec::new();
        }
        let (sender, receiver) = oneshot::channel();
        if let Some(slot) = open_views().get_mut(&self.id) {
            slot.closed = Some(sender);
        }
        let sent = self.host.send(&Command::Close {
            id: self.id.clone(),
        });
        let cookies = match sent {
            Ok(()) => tokio::time::timeout(CLOSING, receiver)
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        open_views().remove(&self.id);
        cookies
    }
}

impl Drop for View {
    fn drop(&mut self) {
        if !self.closed.swap(true, Ordering::AcqRel) {
            let _ = self.host.send(&Command::Close {
                id: self.id.clone(),
            });
        }
        open_views().remove(&self.id);
    }
}

/// Titles of checks' pages, as views tell them: stricter than the check window's, as a view's
/// pages (a reader's "Please wait…") aren't checks for saying so.
const CHECK_TITLES: &[&str] = &[
    "just a moment",
    "attention required",
    "checking your browser",
    "checking if the site connection is secure",
    "verify you are human",
    "verifying you are human",
    "ddos-guard",
    "making sure you're not a bot",
];

/// Whether an event shows a check by itself: a check's answer, or a check's title.
fn shows_check(event: &Event) -> bool {
    match event {
        Event::Answer { mitigated, .. } => *mitigated,
        Event::Title { title } => {
            let title = title.trim().to_lowercase();
            CHECK_TITLES.iter().any(|mark| title.contains(mark))
        }
        _ => false,
    }
}

/// Whether `url` is a browser check provider's ([`CHALLENGE_HOSTS`], [`CHALLENGE_PATHS`]).
pub fn is_challenge(url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(url) else {
        return false;
    };
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    CHALLENGE_HOSTS.iter().any(|known| {
        host == *known
            || host
                .strip_suffix(known)
                .is_some_and(|start| start.ends_with('.'))
    }) || CHALLENGE_PATHS
        .iter()
        .any(|path| url.path().starts_with(path))
}

/// A run's timeout: what the source asked (ms), within [`SHORTEST_RUN`] and [`LONGEST_RUN`].
pub fn run_timeout(asked_ms: f64) -> Duration {
    if !asked_ms.is_finite() || asked_ms <= 0.0 {
        return Duration::from_secs(30);
    }
    Duration::from_millis(asked_ms as u64).clamp(SHORTEST_RUN, LONGEST_RUN)
}

/// A fresh view id.
pub fn new_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!(
        "view-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(test)]
pub(crate) mod fake {
    //! A scripted host for tests: pages by address, scripts answered by a function of the page
    //! and the script, requests a page makes, and bridges that post what a script names.

    use super::*;

    /// Answers a script with its value as JSON.
    pub type Scripts = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

    /// A page the fake serves.
    #[derive(Clone, Default)]
    pub struct FakePage {
        pub title: String,
        /// Answers `cf-mitigated: challenge` (a Cloudflare check).
        pub check: bool,
        /// Requests the page makes once loaded (`url`), which the source may answer.
        pub requests: Vec<String>,
        /// Messages the page posts to its bridges once loaded: `(bridge, message)`.
        pub posts: Vec<(String, String)>,
        /// Answers a script with its value as JSON.
        pub scripts: Option<Scripts>,
    }

    /// What the fake was asked.
    #[derive(Default)]
    pub struct Seen {
        pub opened: Vec<OpenView>,
        pub commands: Vec<Command>,
    }

    #[derive(Default)]
    pub struct FakeHost {
        pub pages: Mutex<HashMap<String, FakePage>>,
        pub seen: Mutex<Seen>,
        /// How the source answered the pages' requests: their addresses and answers.
        pub answered: Mutex<Vec<(String, Option<Answer>)>>,
        /// What each view loaded last, by id.
        loaded: Mutex<HashMap<String, String>>,
        requests: AtomicU64,
        issued: Mutex<HashMap<u64, String>>,
    }

    impl FakeHost {
        /// The one fake every test shares (installed as the host once).
        pub fn shared() -> Arc<Self> {
            static SHARED: LazyLock<Arc<FakeHost>> = LazyLock::new(|| {
                let fake = Arc::new(FakeHost::default());
                set_host(fake.clone());
                fake
            });
            SHARED.clone()
        }

        pub fn serve(&self, url: &str, page: FakePage) {
            self.pages.lock().unwrap().insert(url.to_owned(), page);
        }

        fn page(&self, id: &str) -> Option<(String, FakePage)> {
            let url = self.loaded.lock().unwrap().get(id).cloned()?;
            let page = self.pages.lock().unwrap().get(&url).cloned()?;
            Some((url, page))
        }

        fn load(&self, id: &str, url: &str) {
            self.loaded
                .lock()
                .unwrap()
                .insert(id.to_owned(), url.to_owned());
            let page = self.pages.lock().unwrap().get(url).cloned();
            let id = id.to_owned();
            let url = url.to_owned();
            let requests = page.as_ref().map_or(0, |page| page.requests.len()) as u64;
            let first = self.requests.fetch_add(requests, Ordering::AcqRel) + 1;
            if let Some(page) = &page {
                let mut issued = self.issued.lock().unwrap();
                for (at, request) in page.requests.iter().enumerate() {
                    issued.insert(first + at as u64, request.clone());
                }
            }
            // Events come later, as a platform's do.
            tokio::spawn(async move {
                deliver_event(&id, Event::Started { url: url.clone() });
                let Some(page) = page else {
                    deliver_event(
                        &id,
                        Event::Failed {
                            url: url.clone(),
                            code: -2,
                            description: "net::ERR_NAME_NOT_RESOLVED".into(),
                            main_frame: true,
                        },
                    );
                    deliver_event(&id, Event::Finished { url });
                    return;
                };
                if page.check {
                    deliver_event(
                        &id,
                        Event::Answer {
                            url: url.clone(),
                            status: 403,
                            mitigated: true,
                        },
                    );
                }
                deliver_event(
                    &id,
                    Event::Title {
                        title: page.title.clone(),
                    },
                );
                for (at, request) in page.requests.iter().enumerate() {
                    deliver_event(
                        &id,
                        Event::Request {
                            request: first + at as u64,
                            url: request.clone(),
                            method: "GET".into(),
                            headers: BTreeMap::new(),
                            main_frame: false,
                        },
                    );
                }
                deliver_event(&id, Event::Finished { url });
                for (bridge, message) in page.posts {
                    deliver_event(
                        &id,
                        Event::Message {
                            bridge,
                            message,
                            origin: None,
                            main_frame: Some(true),
                            reply: None,
                        },
                    );
                }
            });
        }
    }

    impl ViewHost for FakeHost {
        fn send(&self, command: &Command) -> Result<(), String> {
            self.seen.lock().unwrap().commands.push(command.clone());
            match command {
                Command::Open(spec) => self.seen.lock().unwrap().opened.push(spec.clone()),
                Command::Load { id, url, .. } => self.load(id, url),
                Command::LoadData { id, base_url, .. } => self.load(id, base_url),
                Command::Evaluate { id, call, script } => {
                    let result = self
                        .page(id)
                        .and_then(|(_, page)| page.scripts)
                        .and_then(|scripts| scripts(script));
                    let (id, call) = (id.clone(), *call);
                    tokio::spawn(async move {
                        deliver_event(&id, Event::Evaluated { call, result });
                    });
                }
                Command::Close { id } => {
                    let url = self.loaded.lock().unwrap().remove(id);
                    let id = id.clone();
                    tokio::spawn(async move {
                        let cookies = url
                            .map(|url| {
                                vec![ViewCookies {
                                    url,
                                    cookies: "viewed=yes".into(),
                                }]
                            })
                            .unwrap_or_default();
                        deliver_event(&id, Event::Closed { cookies });
                    });
                }
                Command::Answer {
                    request, response, ..
                } => {
                    let url = self
                        .issued
                        .lock()
                        .unwrap()
                        .get(request)
                        .cloned()
                        .unwrap_or_default();
                    self.answered.lock().unwrap().push((url, response.clone()));
                }
                Command::Reply { .. } | Command::Stop { .. } => {}
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::{FakeHost, FakePage};
    use super::*;

    fn spec(id: &str) -> OpenView {
        OpenView {
            id: id.to_owned(),
            extension: "x.y".into(),
            javascript: true,
            timeout_ms: 5_000,
            ..OpenView::default()
        }
    }

    #[tokio::test]
    async fn a_view_reports_its_pages_and_answers_scripts() {
        let fake = FakeHost::shared();
        fake.serve(
            "https://views.example/a",
            FakePage {
                title: "A page".into(),
                scripts: Some(Arc::new(|script: &str| {
                    (script == "document.title").then(|| "\"A page\"".to_owned())
                })),
                ..FakePage::default()
            },
        );
        let id = new_id();
        let view = View::open(fake.clone(), spec(&id), turn(LONGEST_RUN).await.unwrap()).unwrap();
        view.send(&Command::Load {
            id: id.clone(),
            url: "https://views.example/a".into(),
            headers: BTreeMap::new(),
        })
        .unwrap();
        let mut kinds = Vec::new();
        while let Next::Event(event) = view.next(Duration::from_millis(500)).await {
            let finished = matches!(event, Event::Finished { .. });
            kinds.push(event);
            if finished {
                break;
            }
        }
        assert_eq!(
            kinds,
            [
                Event::Started {
                    url: "https://views.example/a".into()
                },
                Event::Title {
                    title: "A page".into()
                },
                Event::Finished {
                    url: "https://views.example/a".into()
                },
            ]
        );
        assert_eq!(
            view.evaluate("document.title", Duration::from_secs(1))
                .await
                .unwrap()
                .as_deref(),
            Some("\"A page\"")
        );
        assert_eq!(
            view.evaluate("nothing", Duration::from_secs(1))
                .await
                .unwrap(),
            None
        );
        let cookies = view.close().await;
        assert_eq!(cookies[0].cookies, "viewed=yes");
        assert!(view.send(&Command::Stop { id }).is_err(), "closed");
    }

    #[tokio::test]
    async fn a_check_ends_what_the_view_reports() {
        let fake = FakeHost::shared();
        fake.serve(
            "https://views.example/checked",
            FakePage {
                title: "Just a moment...".into(),
                check: true,
                ..FakePage::default()
            },
        );
        let id = new_id();
        let view = View::open(fake.clone(), spec(&id), turn(LONGEST_RUN).await.unwrap()).unwrap();
        view.send(&Command::Load {
            id: id.clone(),
            url: "https://views.example/checked".into(),
            headers: BTreeMap::new(),
        })
        .unwrap();
        let mut checks = 0;
        while let Next::Event(event) = view.next(Duration::from_millis(300)).await {
            if let Event::Check { url } = event {
                assert_eq!(url, "https://views.example/checked");
                checks += 1;
            }
        }
        assert_eq!(checks, 1, "told once");
    }

    #[test]
    fn checks_providers_are_told_by_host_and_path() {
        assert!(is_challenge(
            "https://challenges.cloudflare.com/turnstile/v0/api.js"
        ));
        assert!(is_challenge("https://newassets.hcaptcha.com/captcha/v1/x"));
        assert!(is_challenge(
            "https://comics.example/cdn-cgi/challenge-platform/h/b/orchestrate/jsch/v1"
        ));
        assert!(is_challenge("https://www.google.com/recaptcha/api.js"));
        assert!(!is_challenge("https://comics.example/reader/1"));
        assert!(!is_challenge("https://notcaptcha-delivery.com/"));
        assert!(!is_challenge("not a url"));
    }

    #[test]
    fn view_titles_tell_checks_strictly() {
        let title = |title: &str| {
            shows_check(&Event::Title {
                title: title.into(),
            })
        };
        assert!(title("Just a moment..."));
        assert!(title("DDoS-Guard"));
        assert!(!title("Please wait while the reader loads"));
        assert!(!title("Chapter 3"));
    }

    #[test]
    fn runs_take_the_time_asked_within_bounds() {
        assert_eq!(run_timeout(10_000.0), Duration::from_secs(10));
        assert_eq!(run_timeout(600_000.0), LONGEST_RUN);
        assert_eq!(run_timeout(5.0), SHORTEST_RUN);
        assert_eq!(run_timeout(f64::NAN), Duration::from_secs(30));
    }

    #[test]
    fn commands_and_events_have_the_platform_s_shape() {
        let command = serde_json::to_value(Command::LoadData {
            id: "v".into(),
            base_url: "https://x.example/".into(),
            data: "<p>".into(),
            mime: "text/html".into(),
        })
        .unwrap();
        assert_eq!(
            command,
            serde_json::json!({ "op": "loadData", "id": "v", "baseUrl": "https://x.example/", "data": "<p>", "mime": "text/html" })
        );
        let delivered: Delivered = serde_json::from_str(
            r#"{"id":"v","kind":"message","bridge":"android","message":"[1]","mainFrame":true}"#,
        )
        .unwrap();
        assert_eq!(delivered.id, "v");
        assert!(matches!(
            delivered.event,
            Event::Message { bridge, main_frame: Some(true), reply: None, .. } if bridge == "android"
        ));
        let open = serde_json::to_value(Command::Open(OpenView {
            id: "v".into(),
            timeout_ms: 1,
            ..OpenView::default()
        }))
        .unwrap();
        assert_eq!(open["op"], "open");
        assert_eq!(open["timeoutMs"], 1);
        assert!(open["challengeHosts"].is_array());
    }
}
