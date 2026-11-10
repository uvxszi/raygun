//! The `js` runtime: one QuickJS runtime and context per extension (ARCHITECTURE §7.2).
//!
//! Everything here runs on the extension's lane (see `host.rs`), so JS values and the `Rc` state
//! behind the native functions never cross threads. Requests run on a shared network runtime
//! instead (`network`), a few per site at a time, so they go on while a lane is busy with script.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use raygun_model::ProviderInfo;
use raygun_net::issue::{self, IssueKind};
use reqwest::Url;
use rquickjs::prelude::Async;
use rquickjs::{
    AsyncContext, AsyncRuntime, CatchResultExt, CaughtError, Ctx, Function, IntoJs, Object, Promise,
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::{Notify, Semaphore};

use crate::browser_check::CheckedRequest;
use crate::content::ContentFilter;
use crate::cookies::CookieJar;
use crate::failure::{Failure, Trouble};
use crate::html;
use crate::manifest::{Manifest, Runtime, SettingDef, is_provider_id};
use crate::permissions::{HostPattern, NetPolicy, is_public_ip};
use crate::storage::KvStore;

const PRELUDE: &str = include_str!("prelude.js");
const MAX_REDIRECTS: usize = 10;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
pub const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
/// Longest single `raygun.time.sleep`: enough to pace rate-limited APIs, short of the call timeout.
const MAX_SLEEP_MS: f64 = 10_000.0;
/// Requests to one site at once, across every extension; more wait their turn, so many sources
/// on one site don't get them all refused.
const SITE_REQUESTS: usize = 6;
/// Requests extensions make at once in all: a phone has only so many sockets and TLS handshakes
/// in it.
const ALL_REQUESTS: usize = 32;
/// The longest a site's `Retry-After` is kept; a site that asks more is asked again then.
const LONGEST_HOLD: Duration = Duration::from_secs(60);
/// A request waits this long at most for a site that asked to be left alone; one that would wait
/// longer fails at once, as the site limiting requests.
const HOLD_WAIT: Duration = Duration::from_secs(5);
/// What requests met, kept per extension to explain a failed call (newest last).
const NET_LOG: usize = 32;
/// How much of a failed answer is kept to tell a browser check from a plain refusal.
const CHECK_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone)]
pub struct Limits {
    pub memory_bytes: usize,
    pub stack_bytes: usize,
    /// Longest run of synchronous JS before it is interrupted. The budget restarts whenever a
    /// call begins or a host request (`fetch`) completes, so time spent waiting doesn't count.
    pub cpu_budget: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            memory_bytes: 64 * 1024 * 1024,
            stack_bytes: 1024 * 1024,
            cpu_budget: Duration::from_secs(5),
        }
    }
}

/// State the native functions share with the extension.
struct Shared {
    manifest: Manifest,
    policy: Arc<NetPolicy>,
    client: reqwest::Client,
    /// The jar the client keeps cookies in, which also knows the User-Agent each site's passed
    /// browser check holds for.
    cookies: Arc<CookieJar>,
    storage: RefCell<KvStore>,
    settings: BTreeMap<String, serde_json::Value>,
    /// Settings the script declared while running (Miru's `registerSetting`), after the
    /// manifest's own.
    registered: Rc<RefCell<Vec<SettingDef>>>,
    providers: RefCell<Vec<ProviderInfo>>,
    deadline: Rc<Cell<Instant>>,
    cpu_budget: Duration,
    content: ContentFilter,
    /// What requests met lately, newest last.
    net_log: RefCell<VecDeque<(Instant, NetEvent)>>,
    /// Calls (and the load) running now; requests stop once none is left to wait for them.
    calls: Cell<usize>,
    idle: Notify,
}

/// What one request met.
#[derive(Debug, Clone)]
enum NetEvent {
    /// The site answered (2xx or 3xx).
    Answered,
    /// What went wrong, with the request as it was sent when a browser check answered it.
    Trouble(Trouble, Option<Box<CheckedRequest>>),
}

/// What a call's requests met: the worst trouble, whether one got a good answer, and the request
/// a browser check answered when that's the trouble.
type RequestsMet = (Option<Trouble>, bool, Option<Box<CheckedRequest>>);

impl Shared {
    fn restart_budget(&self) {
        self.deadline.set(Instant::now() + self.cpu_budget);
    }

    fn note(&self, event: NetEvent) {
        let mut log = self.net_log.borrow_mut();
        if log.len() >= NET_LOG {
            log.pop_front();
        }
        log.push_back((Instant::now(), event));
    }

    /// The worst trouble requests met since `since` (the first of the worst), whether one got a
    /// good answer, and the request that met a browser check when that's the trouble.
    fn since(&self, since: Instant) -> RequestsMet {
        let log = self.net_log.borrow();
        let mut worst: Option<(&Trouble, Option<&CheckedRequest>)> = None;
        let mut answered = false;
        for (_, event) in log.iter().filter(|(at, _)| *at >= since) {
            match event {
                NetEvent::Answered => answered = true,
                NetEvent::Trouble(trouble, checked) => {
                    if worst.is_none_or(|(worst, _)| trouble.kind.weight() > worst.kind.weight()) {
                        worst = Some((trouble, checked.as_deref()));
                    }
                }
            }
        }
        let checked = worst
            .and_then(|(_, checked)| checked.cloned())
            .map(Box::new);
        (worst.map(|(trouble, _)| trouble.clone()), answered, checked)
    }

    /// Resolves once no call is running.
    async fn idle(&self) {
        loop {
            let notified = self.idle.notified();
            if self.calls.get() == 0 {
                return;
            }
            notified.await;
        }
    }
}

/// A running call (or the load): the extension's requests go on while one is held.
struct Running(Rc<Shared>);

impl Running {
    fn new(shared: &Rc<Shared>) -> Self {
        shared.calls.set(shared.calls.get() + 1);
        Self(shared.clone())
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let left = self.0.calls.get().saturating_sub(1);
        self.0.calls.set(left);
        if left == 0 {
            // Nobody waits for the extension's requests any more (a search was left, or gave
            // up): stop them rather than finish them for nothing.
            self.0.idle.notify_waiters();
        }
    }
}

/// Wakes the runtime's driver once a call ends, finished or dropped.
///
/// rquickjs wakes whoever polled its spawned futures last. That is often the call, so once the
/// call is gone a wakeup meant for the extension's own futures (a request stopped because nobody
/// waits for it, or one a background promise still runs) would reach a task that no longer
/// exists, and the driver would sleep through it. Spawning a future into the runtime always wakes
/// the driver, which then polls everything that is due and takes the wakeups from then on.
struct WakeDriver(AsyncContext);

impl Drop for WakeDriver {
    fn drop(&mut self) {
        let context = self.0.clone();
        tokio::task::spawn_local(async move {
            context.with(|ctx| ctx.spawn(async {})).await;
        });
    }
}

pub struct JsExtension {
    shared: Rc<Shared>,
    // Field order matters: the context must drop before its runtime.
    context: AsyncContext,
    _runtime: AsyncRuntime,
}

impl JsExtension {
    /// Creates the runtime, installs the host API and evaluates the extension's entry script.
    pub async fn load(
        manifest: Manifest,
        code: String,
        storage: KvStore,
        cookies: Arc<CookieJar>,
        settings: BTreeMap<String, serde_json::Value>,
        limits: &Limits,
        content: ContentFilter,
    ) -> Result<Self, String> {
        // A Miru script (M46.x) is checked and rewritten as Miru does before anything runs; an
        // LNReader plugin is wrapped as LNReader evaluates it.
        let miru = match manifest.runtime {
            Runtime::Miru => Some(crate::miru::runtime::Prepared::new(&code)?),
            _ => None,
        };
        let lnreader = match manifest.runtime {
            Runtime::LnReader => Some(crate::lnreader::runtime::Prepared::new(&code)?),
            _ => None,
        };
        // A converted Mihon extension runs after the Mihon prelude and its themes.
        let mihon = match manifest.runtime {
            Runtime::Mihon => Some(crate::mihon::runtime::Prepared::new(&code)?),
            _ => None,
        };
        let policy = Arc::new(manifest.net.clone());
        let client =
            extension_client(policy.clone(), cookies.clone()).map_err(|error| error.to_string())?;
        let deadline = Rc::new(Cell::new(Instant::now() + limits.cpu_budget));
        let registered = Rc::new(RefCell::new(Vec::new()));
        let shared = Rc::new(Shared {
            manifest,
            policy,
            client,
            cookies: cookies.clone(),
            storage: RefCell::new(storage),
            settings,
            registered: registered.clone(),
            providers: RefCell::new(Vec::new()),
            deadline: deadline.clone(),
            cpu_budget: limits.cpu_budget,
            content,
            net_log: RefCell::new(VecDeque::new()),
            calls: Cell::new(0),
            idle: Notify::new(),
        });
        // Requests made while the script sets up belong to the load.
        let loading = Running::new(&shared);

        let runtime = AsyncRuntime::new().map_err(|error| error.to_string())?;
        // Drives the extension's own futures (its fetches) whatever call is waiting on them. Without
        // it they're polled by the last call into the runtime: two calls at once (a title's details
        // and episodes) left one waiting forever when the other finished first (M46.45). Ends when
        // the runtime is dropped. The host runs extensions on a `LocalSet`.
        tokio::task::spawn_local(runtime.drive());
        runtime.set_memory_limit(limits.memory_bytes).await;
        runtime.set_max_stack_size(limits.stack_bytes).await;
        runtime
            .set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline.get())))
            .await;
        let context = AsyncContext::full(&runtime)
            .await
            .map_err(|error| error.to_string())?;

        let filename = format!("{}/{}", shared.manifest.id, shared.manifest.entry);
        let natives = shared.clone();
        shared.restart_budget();
        context
            .with(move |ctx| -> Result<(), String> {
                install_natives(&ctx, natives).map_err(|error| error.to_string())?;
                eval_script(&ctx, PRELUDE, "raygun:prelude", true)?;
                if let Some(prepared) = &mihon {
                    return crate::mihon::runtime::install(
                        &ctx,
                        prepared,
                        crate::mihon::runtime::Hooks {
                            cookies: cookies.clone(),
                        },
                        &filename,
                    );
                }
                match (&miru, &lnreader) {
                    (Some(prepared), _) => crate::miru::runtime::install(
                        &ctx,
                        prepared,
                        crate::miru::runtime::Hooks {
                            cookies,
                            settings: registered,
                        },
                        &filename,
                    ),
                    (None, Some(prepared)) => crate::lnreader::runtime::install(
                        &ctx,
                        prepared,
                        crate::lnreader::runtime::Hooks {
                            settings: registered,
                        },
                        &filename,
                    ),
                    (None, None) => eval_script(&ctx, &code, &filename, true),
                }
            })
            .await?;

        // Scripts that finish setting up asynchronously (converted Nuclear plugins run their
        // `onLoad`/`onEnable` that way) publish a promise; their providers count once it settles.
        context
            .async_with(async |ctx| -> Result<(), String> {
                let ready: rquickjs::Value = ctx
                    .globals()
                    .get("__rg_ready")
                    .map_err(|error| error.to_string())?;
                if let Some(promise) = ready.into_promise() {
                    promise
                        .into_future::<()>()
                        .await
                        .catch(&ctx)
                        .map_err(|error| format!("setup failed: {error}"))?;
                }
                Ok(())
            })
            .await?;
        drop(loading);

        Ok(Self {
            shared,
            context,
            _runtime: runtime,
        })
    }

    pub fn manifest(&self) -> &Manifest {
        &self.shared.manifest
    }

    /// The user's values for this extension's settings (defaults not included).
    pub fn user_settings(&self) -> &BTreeMap<String, serde_json::Value> {
        &self.shared.settings
    }

    /// The settings it has: the manifest's, then those the script registered (Miru).
    pub fn settings(&self) -> Vec<SettingDef> {
        let mut settings = self.shared.manifest.settings.clone();
        for setting in self.shared.registered.borrow().iter() {
            if !settings.iter().any(|known| known.key == setting.key) {
                settings.push(setting.clone());
            }
        }
        settings
    }

    pub fn providers(&self) -> Vec<ProviderInfo> {
        self.shared.providers.borrow().clone()
    }

    /// Calls `method` on one of this extension's providers (its local id) and awaits the result.
    /// A failure carries what the call's requests met along the way.
    pub async fn call(
        &self,
        provider: &str,
        method: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, Failure> {
        // Dropped after `_running`, so the driver runs once the requests were told to stop.
        let _wake_driver = WakeDriver(self.context.clone());
        let _running = Running::new(&self.shared);
        let started = Instant::now();
        let (provider, method, args) = (provider.to_owned(), method.to_owned(), args.to_string());
        let label = format!("{}/{provider}.{method}", self.shared.manifest.id);
        self.shared.restart_budget();
        let answer = self
            .context
            .async_with(async move |ctx| {
                let call: Function = ctx
                    .globals()
                    .get("__rg_call")
                    .map_err(|error| error.to_string())?;
                let promise: Promise = call
                    .call((provider, method, args))
                    .catch(&ctx)
                    .map_err(|error| call_error(&label, error))?;
                let text: String = promise
                    .into_future::<String>()
                    .await
                    .catch(&ctx)
                    .map_err(|error| call_error(&label, error))?;
                serde_json::from_str(&text).map_err(|error| error.to_string())
            })
            .await;
        answer.map_err(|message| {
            let (trouble, answered, checked) = self.shared.since(started);
            Failure {
                message,
                trouble,
                answered,
                checked,
            }
        })
    }

    /// A YouTube video's audio stream from Raygun's YouTube module, asked through this
    /// extension's own fetch (its permissions and cookies), as its `raygun.youtube.stream` would.
    pub async fn youtube_stream(
        &self,
        video: &str,
        prefs: crate::youtube::StreamPrefs,
    ) -> Result<crate::youtube::Stream, String> {
        let _running = Running::new(&self.shared);
        crate::youtube::YouTube::shared()
            .stream(&ExtensionHttp(self.shared.clone()), video, prefs)
            .await
    }
}

/// What a failed provider call tells the user: the thrown message alone. The stack trace, which
/// only helps the extension's author, goes to the log.
fn call_error(label: &str, error: CaughtError<'_>) -> String {
    let full = error.to_string();
    tracing::debug!(call = %label, error = %full, "provider call failed");
    match &error {
        CaughtError::Exception(exception) => exception
            .message()
            .filter(|message| !message.trim().is_empty())
            .unwrap_or(full),
        _ => full,
    }
}

/// Evaluates a script in the global scope; `strict` as Raygun's own code runs, sloppy for
/// scripts written for runtimes that evaluate that way (Miru).
pub(crate) fn eval_script(
    ctx: &Ctx<'_>,
    code: &str,
    filename: &str,
    strict: bool,
) -> Result<(), String> {
    let mut options = rquickjs::context::EvalOptions::default();
    options.global = true;
    options.strict = strict;
    options.filename = Some(filename.to_owned());
    ctx.eval_with_options::<(), _>(code, options)
        .catch(ctx)
        .map_err(|error| error.to_string())
}

/// An HTTP client whose redirects obey the extension's permissions, like its own requests, and
/// that keeps the extension's cookies (M46.11).
fn extension_client(
    policy: Arc<NetPolicy>,
    cookies: Arc<CookieJar>,
) -> reqwest::Result<reqwest::Client> {
    let any_public = policy.patterns().contains(&HostPattern::AnyPublic);
    let resolver = PublicResolver {
        policy: policy.clone(),
    };
    let redirects = reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() >= MAX_REDIRECTS {
            attempt.error("too many redirects")
        } else if policy.allows(attempt.url()) {
            attempt.follow()
        } else {
            let denied = denied_message(attempt.url());
            attempt.error(denied)
        }
    });
    let mut builder = raygun_net::client_builder()
        .redirect(redirects)
        .cookie_provider(cookies)
        .timeout(REQUEST_TIMEOUT);
    if any_public {
        builder = builder.dns_resolver(Arc::new(resolver));
    }
    builder.build()
}

/// Resolves names for extensions that may reach any public host (`*`: Miru scripts, Nuclear
/// conversions): addresses on this machine or the local network are dropped, so a public name
/// can't lead there. Hosts the manifest names itself keep every address.
struct PublicResolver {
    policy: Arc<NetPolicy>,
}

impl reqwest::dns::Resolve for PublicResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let named = self.policy.names(name.as_str());
        Box::pin(async move {
            let host = name.as_str().to_owned();
            let found = tokio::net::lookup_host((host.as_str(), 0)).await?;
            let addresses: Vec<std::net::SocketAddr> = found
                .filter(|address| named || is_public_ip(address.ip()))
                .collect();
            if addresses.is_empty() {
                return Err(format!("{host} doesn't resolve to a public address").into());
            }
            Ok(Box::new(addresses.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

fn denied_message(url: &Url) -> String {
    format!(
        "network access to {} is not permitted by the extension's manifest",
        url.host_str().unwrap_or(url.as_str())
    )
}

/// The JSON envelopes natives answer with: `{ value }` or `{ error }`.
pub(crate) fn ok(value: impl serde::Serialize) -> String {
    json!({ "value": value }).to_string()
}

pub(crate) fn err(message: impl std::fmt::Display) -> String {
    json!({ "error": message.to_string() }).to_string()
}

fn install_natives<'js>(ctx: &Ctx<'js>, shared: Rc<Shared>) -> rquickjs::Result<()> {
    let native = Object::new(ctx.clone())?;

    let s = shared.clone();
    native.set(
        "log",
        Function::new(ctx.clone(), move |level: String, message: String| {
            let id = &s.manifest.id;
            match level.as_str() {
                "error" => tracing::error!(extension = %id, "{message}"),
                "warn" => tracing::warn!(extension = %id, "{message}"),
                "debug" => tracing::debug!(extension = %id, "{message}"),
                _ => tracing::info!(extension = %id, "{message}"),
            }
        })?,
    )?;

    let s = shared.clone();
    native.set(
        "fetch",
        Function::new(
            ctx.clone(),
            Async(move |url: String, options: String| {
                let s = s.clone();
                async move {
                    let result = fetch(&s, &url, &options).await;
                    s.restart_budget();
                    match result {
                        Ok(response) => Fetched::Answer(response),
                        Err(message) => Fetched::Failed(message),
                    }
                }
            }),
        )?,
    )?;

    let s = shared.clone();
    native.set(
        "sleep",
        Function::new(
            ctx.clone(),
            Async(move |ms: f64| {
                let s = s.clone();
                async move {
                    let ms = if ms.is_finite() {
                        ms.clamp(0.0, MAX_SLEEP_MS)
                    } else {
                        0.0
                    };
                    tokio::time::sleep(Duration::from_millis(ms as u64)).await;
                    s.restart_budget();
                }
            }),
        )?,
    )?;

    native.set(
        "htmlSelect",
        Function::new(
            ctx.clone(),
            |source: String, selector: String, fragment: bool| match html::select(
                &source, &selector, fragment,
            ) {
                Ok(elements) => ok(elements),
                Err(message) => err(message),
            },
        )?,
    )?;

    let s = shared.clone();
    native.set(
        "storageGet",
        Function::new(ctx.clone(), move |key: String| {
            ok(s.storage.borrow().get(&key).cloned().unwrap_or_default())
        })?,
    )?;
    let s = shared.clone();
    native.set(
        "storageSet",
        Function::new(
            ctx.clone(),
            move |key: String, value: String| match serde_json::from_str(&value) {
                Ok(value) => match s.storage.borrow_mut().set(&key, value) {
                    Ok(()) => ok(()),
                    Err(message) => err(message),
                },
                Err(error) => err(error),
            },
        )?,
    )?;
    let s = shared.clone();
    native.set(
        "storageRemove",
        Function::new(ctx.clone(), move |key: String| {
            match s.storage.borrow_mut().remove(&key) {
                Ok(()) => ok(()),
                Err(message) => err(message),
            }
        })?,
    )?;
    let s = shared.clone();
    native.set(
        "storageKeys",
        Function::new(ctx.clone(), move || ok(s.storage.borrow().keys()))?,
    )?;

    let s = shared.clone();
    native.set(
        "settingsGet",
        Function::new(ctx.clone(), move |key: String| {
            let value = s
                .settings
                .get(&key)
                .or_else(|| s.manifest.setting_default(&key))
                .cloned()
                .unwrap_or_default();
            ok(value)
        })?,
    )?;

    let s = shared.clone();
    native.set(
        "showNsfw",
        Function::new(ctx.clone(), move || s.content.show_nsfw())?,
    )?;

    let s = shared.clone();
    native.set(
        "registerProvider",
        Function::new(ctx.clone(), move |meta: String| match register(&s, &meta) {
            Ok(()) => ok(()),
            Err(message) => err(message),
        })?,
    )?;

    install_youtube(ctx, &native, &shared)?;
    ctx.globals().set("__rg", native)
}

/// `raygun.youtube` (the host side of Nuclear's `Ytdlp`): YouTube asked through the extension's
/// own fetch, so its permissions apply. Each answers a JSON envelope.
fn install_youtube<'js>(
    ctx: &Ctx<'js>,
    native: &Object<'js>,
    shared: &Rc<Shared>,
) -> rquickjs::Result<()> {
    use crate::youtube::{StreamPrefs, YouTube};

    let envelope = |result: Result<String, String>| match result {
        Ok(json) => format!("{{\"value\":{json}}}"),
        Err(message) => err(message),
    };
    let s = shared.clone();
    native.set(
        "youtubeSearch",
        Function::new(
            ctx.clone(),
            Async(move |query: String, limit: f64| {
                let s = s.clone();
                async move {
                    let limit = (limit.is_finite() && limit >= 1.0).then_some(limit as usize);
                    let found = YouTube::shared()
                        .search(&ExtensionHttp(s.clone()), &query, limit)
                        .await;
                    s.restart_budget();
                    envelope(found.and_then(|found| to_json(&found)))
                }
            }),
        )?,
    )?;
    let s = shared.clone();
    native.set(
        "youtubeStream",
        Function::new(
            ctx.clone(),
            Async(move |video: String, options: String| {
                let s = s.clone();
                async move {
                    let prefs: StreamPrefs = serde_json::from_str(&options).unwrap_or_default();
                    let found = YouTube::shared()
                        .stream(&ExtensionHttp(s.clone()), &video, prefs)
                        .await;
                    s.restart_budget();
                    envelope(found.and_then(|found| to_json(&found)))
                }
            }),
        )?,
    )?;
    let s = shared.clone();
    native.set(
        "youtubePlaylist",
        Function::new(
            ctx.clone(),
            Async(move |playlist: String| {
                let s = s.clone();
                async move {
                    let found = YouTube::shared()
                        .playlist(&ExtensionHttp(s.clone()), &playlist)
                        .await;
                    s.restart_budget();
                    envelope(found.and_then(|found| to_json(&found)))
                }
            }),
        )?,
    )?;
    Ok(())
}

fn to_json(value: &impl serde::Serialize) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| error.to_string())
}

/// The extension's own fetch, for the YouTube module.
struct ExtensionHttp(Rc<Shared>);

impl crate::youtube::Http for ExtensionHttp {
    async fn send(
        &self,
        request: crate::youtube::Request,
    ) -> Result<crate::youtube::Response, String> {
        let headers: BTreeMap<&str, &str> = request
            .headers
            .iter()
            .map(|(name, value)| (*name, value.as_str()))
            .collect();
        let options = json!({
            "method": request.method,
            "headers": headers,
            "body": request.body,
        })
        .to_string();
        let response = fetch(&self.0, &request.url, &options).await?;
        Ok(crate::youtube::Response {
            status: response.status,
            body: response.body,
        })
    }
}

/// Provider kinds Nuclear plugins become (`CATEGORY_KINDS` in nuclear-compat).
const NUCLEAR_KINDS: [&str; 3] = ["music.metadata", "music.stream", "lyrics"];

#[derive(Deserialize)]
struct ProviderMeta {
    kind: Option<String>,
    id: Option<String>,
    name: Option<String>,
    /// A video source's own Search pill (M46.35).
    #[serde(rename = "searchPill")]
    search_pill: Option<String>,
}

fn register(shared: &Shared, meta: &str) -> Result<(), String> {
    let meta: ProviderMeta = serde_json::from_str(meta).map_err(|error| error.to_string())?;
    let manifest = &shared.manifest;
    let kind = meta.kind.ok_or("provider kind is required")?;
    // A converted Nuclear plugin may register a kind its package didn't declare (OmniSource's
    // metadata, Bandcamp's streams); any kind Raygun runs for Nuclear plugins is allowed.
    let nuclear_kind = manifest.nuclear.is_some() && NUCLEAR_KINDS.contains(&kind.as_str());
    if !manifest.provides.contains(&kind) && !nuclear_kind {
        return Err(format!(
            "provider kind {kind:?} is not listed in the manifest's \"provides\""
        ));
    }
    let id = meta.id.ok_or("provider id is required")?;
    if !is_provider_id(&id) {
        return Err(format!("invalid provider id {id:?}"));
    }
    let full_id = format!("{}/{id}", manifest.id);
    let mut providers = shared.providers.borrow_mut();
    if providers.iter().any(|provider| provider.id == full_id) {
        return Err(format!("provider {id:?} is already registered"));
    }
    let name = meta
        .name
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| manifest.name.clone());
    let search_pill = meta
        .search_pill
        .map(|pill| pill.trim().chars().take(24).collect::<String>())
        .filter(|pill| !pill.is_empty() && kind == "video.source");
    providers.push(ProviderInfo {
        adult: false,
        id: full_id,
        extension_id: manifest.id.clone(),
        kind,
        name,
        search_pill,
    });
    Ok(())
}

#[derive(Deserialize)]
struct FetchOptions {
    method: String,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    #[serde(default)]
    body: Option<String>,
    /// A body of bytes (protobuf, a picture), in base64; it wins over `body`.
    #[serde(default, rename = "bodyBase64")]
    body_base64: Option<String>,
    /// The answer's bytes are wanted when they aren't text (`bytes` beside `body`).
    #[serde(default)]
    binary: bool,
}

struct FetchResponse {
    status: u16,
    url: String,
    headers: BTreeMap<String, String>,
    body: String,
    /// The answer's bytes, when they were asked for and aren't UTF-8 text.
    bytes: Option<Vec<u8>>,
}

/// What `raygun.http.fetch` hands the script: the answer as an object, its body a string of its
/// own (never escaped into a JSON envelope and parsed back), or why the request failed.
enum Fetched {
    Answer(FetchResponse),
    Failed(String),
}

impl<'js> IntoJs<'js> for Fetched {
    fn into_js(self, ctx: &Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        let object = Object::new(ctx.clone())?;
        match self {
            Self::Answer(response) => {
                let headers = Object::new(ctx.clone())?;
                for (name, value) in response.headers {
                    headers.set(name, value)?;
                }
                object.set("status", response.status)?;
                object.set("url", response.url)?;
                object.set("headers", headers)?;
                object.set("body", response.body)?;
                if let Some(bytes) = response.bytes {
                    object.set(
                        "bytes",
                        rquickjs::TypedArray::<u8>::new(ctx.clone(), bytes)?,
                    )?;
                }
            }
            Self::Failed(message) => object.set("error", message)?,
        }
        Ok(object.into_value())
    }
}

/// The runtime extensions' requests run on, apart from the lanes, so answers keep arriving while
/// a lane is busy running script. `None` if it couldn't start: requests then run on the lane.
/// Connected servers (`crate::servers`) send theirs here too.
pub(crate) fn network() -> Option<&'static tokio::runtime::Runtime> {
    static NETWORK: OnceLock<Option<tokio::runtime::Runtime>> = OnceLock::new();
    NETWORK
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("raygun-ext-net")
                .enable_all()
                .build()
                .inspect_err(|error| tracing::warn!(%error, "no network runtime for extensions"))
                .ok()
        })
        .as_ref()
}

/// Turns for requests: a few per site, more in all.
struct Gates {
    all: Arc<Semaphore>,
    sites: Mutex<HashMap<String, Arc<Semaphore>>>,
    holds: Holds,
}

/// Sites that answered "too many requests" or "busy" with a `Retry-After`: their requests wait
/// until then, so a source asked again (a song looking for another source, a search repeated)
/// gives the site the time it asked for.
#[derive(Default)]
struct Holds(Mutex<HashMap<String, Instant>>);

impl Holds {
    /// How long requests to `site` still wait at `now`, if they do.
    fn left(&self, site: &str, now: Instant) -> Option<Duration> {
        let mut holds = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let until = *holds.get(site)?;
        if until > now {
            return Some(until - now);
        }
        holds.remove(site);
        None
    }

    /// `site` asked at `now` to be left alone for `wait` (up to [`LONGEST_HOLD`]).
    fn hold(&self, site: &str, wait: Duration, now: Instant) {
        let until = now + wait.min(LONGEST_HOLD);
        let mut holds = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if holds.len() > 256 {
            holds.retain(|_, kept| *kept > now);
        }
        let kept = holds.entry(site.to_owned()).or_insert(until);
        *kept = (*kept).max(until);
    }
}

/// What a request held back for its site says, which [`asked_wait`] reads.
fn held_detail(host: &str, wait: Duration) -> String {
    format!(
        "{host} asked to wait {} s before more requests",
        wait.as_secs().max(1)
    )
}

/// How long a failed call's site still asked to be left alone, when one of its requests was held
/// back for that: whoever asks again can wait as long.
pub fn asked_wait(error: &crate::CallError) -> Option<Duration> {
    match error {
        crate::CallError::Failed(issue) if issue.kind == IssueKind::RateLimited => {
            let (_, rest) = issue.detail.split_once(" asked to wait ")?;
            let (seconds, _) = rest.split_once(" s before more requests")?;
            seconds.parse().ok().map(Duration::from_secs)
        }
        crate::CallError::Resting { last, .. } => asked_wait(last),
        _ => None,
    }
}

/// How long a 429 or 503 answer asks to wait (its `Retry-After` in seconds; dates aren't read).
fn retry_after(status: u16, headers: &BTreeMap<String, String>) -> Option<Duration> {
    if !matches!(status, 429 | 503) {
        return None;
    }
    let seconds = headers.get("retry-after")?.trim().parse::<u64>().ok()?;
    Some(Duration::from_secs(seconds))
}

impl Gates {
    fn get() -> &'static Self {
        static GATES: OnceLock<Gates> = OnceLock::new();
        GATES.get_or_init(|| Self {
            all: Arc::new(Semaphore::new(ALL_REQUESTS)),
            sites: Mutex::new(HashMap::new()),
            holds: Holds::default(),
        })
    }

    fn site(&self, host: &str) -> Arc<Semaphore> {
        let mut sites = self
            .sites
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if sites.len() > 256 {
            // Forget sites no request is using.
            sites.retain(|_, gate| Arc::strong_count(gate) > 1);
        }
        sites
            .entry(host.to_ascii_lowercase())
            .or_insert_with(|| Arc::new(Semaphore::new(SITE_REQUESTS)))
            .clone()
    }
}

/// An answer and what it says went wrong (a failed status), or why there was none.
type Exchanged = Result<(FetchResponse, Option<Trouble>), Trouble>;

async fn fetch(shared: &Shared, url: &str, options: &str) -> Result<FetchResponse, String> {
    let url = Url::parse(url).map_err(|error| format!("invalid URL {url:?}: {error}"))?;
    if !shared.policy.allows(&url) {
        return Err(denied_message(&url));
    }
    let options: FetchOptions =
        serde_json::from_str(options).map_err(|error| format!("invalid fetch options: {error}"))?;
    let method = reqwest::Method::from_bytes(options.method.to_ascii_uppercase().as_bytes())
        .map_err(|_| format!("invalid method {:?}", options.method))?;

    let host = url.host_str().unwrap_or_default().to_owned();
    // Requests take turns per server: the host and its port.
    let site = format!("{host}:{}", url.port_or_known_default().unwrap_or_default());
    // The extension's own User-Agent, else the one a browser check on the site was passed with
    // (the pass holds only for it), else the one sources send.
    let own_agent = options
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("user-agent"))
        .map(|(_, agent)| agent.clone());
    let agent = own_agent
        .clone()
        .or_else(|| shared.cookies.user_agent_for(&url))
        .unwrap_or_else(raygun_net::user_agent);
    let address = url.to_string();
    let mut request = shared.client.request(method.clone(), url);
    for (name, value) in &options.headers {
        request = request.header(name, value);
    }
    if own_agent.is_none() {
        request = request.header(reqwest::header::USER_AGENT, &agent);
    }
    if let Some(encoded) = &options.body_base64 {
        use base64::Engine as _;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|error| format!("invalid request body: {error}"))?;
        request = request.body(bytes);
    } else if let Some(body) = options.body.clone() {
        request = request.body(body);
    }
    // The request as sent, kept when a browser check answers it: the check window opens it.
    let checked = |trouble: &Trouble| {
        (trouble.kind == IssueKind::BotCheck).then(|| {
            Box::new(CheckedRequest::new(
                &address,
                method.as_str(),
                options
                    .headers
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.as_str())),
                options.body.as_deref(),
                &agent,
            ))
        })
    };
    let exchange = exchange(request, host, site, options.binary);
    let stopped = || "stopped: nothing waits for this request any more".to_owned();
    let exchanged = match network() {
        Some(network) => {
            let task = network.spawn(exchange);
            let abort = task.abort_handle();
            tokio::select! {
                joined = task => joined.map_err(|_| stopped())?,
                () = shared.idle() => {
                    abort.abort();
                    return Err(stopped());
                }
            }
        }
        None => tokio::select! {
            exchanged = exchange => exchanged,
            () = shared.idle() => return Err(stopped()),
        },
    };
    match exchanged {
        Ok((response, trouble)) => {
            shared.note(match trouble {
                Some(trouble) => {
                    let checked = checked(&trouble);
                    NetEvent::Trouble(trouble, checked)
                }
                None => NetEvent::Answered,
            });
            Ok(response)
        }
        Err(trouble) => {
            let message = trouble.detail.clone();
            shared.note(NetEvent::Trouble(trouble, None));
            Err(message)
        }
    }
}

/// Sends a request and reads its answer, waiting for a turn with the site (`host:port`) first.
/// Runs on the network runtime.
async fn exchange(
    request: reqwest::RequestBuilder,
    host: String,
    site: String,
    binary: bool,
) -> Exchanged {
    let gates = Gates::get();
    // A site that asked to be left alone gets the time it asked for: a short wait here, a longer
    // one fails at once (the caller tries later).
    if let Some(wait) = gates.holds.left(&site, Instant::now()) {
        if wait > HOLD_WAIT {
            return Err(Trouble {
                kind: IssueKind::RateLimited,
                detail: held_detail(&host, wait),
            });
        }
        tokio::time::sleep(wait).await;
    }
    // The gates are never closed, so acquiring only waits.
    let _site = gates.site(&site).acquire_owned().await;
    let _all = gates.all.clone().acquire_owned().await;
    let mut response = request
        .send()
        .await
        .map_err(|error| request_trouble(&host, &error))?;

    let status = response.status().as_u16();
    let final_url = response.url().to_string();
    let mut headers = BTreeMap::<String, String>::new();
    for (name, value) in response.headers() {
        let value = String::from_utf8_lossy(value.as_bytes());
        headers
            .entry(name.as_str().to_owned())
            .and_modify(|existing| {
                existing.push_str(", ");
                existing.push_str(&value);
            })
            .or_insert_with(|| value.into_owned());
    }
    if let Some(wait) = retry_after(status, &headers) {
        gates.holds.hold(&site, wait, Instant::now());
    }
    let too_large = || Trouble {
        kind: IssueKind::ExtensionFailed,
        detail: format!(
            "{host} sent more than {} MB",
            MAX_RESPONSE_BYTES / (1024 * 1024)
        ),
    };
    let length = response.content_length();
    if length.is_some_and(|length| length > MAX_RESPONSE_BYTES as u64) {
        return Err(too_large());
    }
    let mut body = Vec::with_capacity(length.map_or(0, |length| length as usize));
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| request_trouble(&host, &error))?
    {
        if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(too_large());
        }
        body.extend_from_slice(&chunk);
    }
    // Valid UTF-8 (nearly every page) becomes the string without a copy. Bytes that aren't
    // text are kept as they are when asked for, and spelled out only for a textual answer.
    let (body, bytes) = match String::from_utf8(body) {
        Ok(text) => (text, None),
        Err(error) if binary => {
            let textual = headers
                .get("content-type")
                .is_some_and(|kind| is_textual(kind));
            let bytes = error.into_bytes();
            let text = if textual {
                String::from_utf8_lossy(&bytes).into_owned()
            } else {
                String::new()
            };
            (text, Some(bytes))
        }
        Err(error) => (String::from_utf8_lossy(error.as_bytes()).into_owned(), None),
    };
    let mut check_end = body.len().min(CHECK_BYTES);
    while !body.is_char_boundary(check_end) {
        check_end -= 1;
    }
    let trouble = issue::answer_kind(
        status,
        headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str())),
        &body[..check_end],
    )
    .map(|kind| Trouble {
        kind,
        detail: issue::answer_detail(&host, status, kind),
    });
    Ok((
        FetchResponse {
            status,
            url: final_url,
            headers,
            body,
            bytes,
        },
        trouble,
    ))
}

/// Whether a `Content-Type` names text (a page, JSON, a script), which is read as such even when
/// its bytes aren't all UTF-8.
fn is_textual(kind: &str) -> bool {
    let kind = kind.to_ascii_lowercase();
    kind.starts_with("text/")
        || ["json", "xml", "javascript", "html", "x-www-form-urlencoded"]
            .iter()
            .any(|word| kind.contains(word))
}

/// What a failed request met. reqwest's own message repeats the whole URL; the innermost cause
/// ("operation timed out", a denied redirect) says what happened.
fn request_trouble(host: &str, error: &reqwest::Error) -> Trouble {
    // A redirect the manifest doesn't allow: the site moved where the extension may not go.
    let kind = if error.is_redirect() {
        IssueKind::SiteChanged
    } else {
        issue::request_kind(error)
    };
    Trouble {
        kind,
        detail: issue::request_detail(host, error),
    }
}

#[cfg(test)]
mod hold_tests {
    use super::*;

    #[test]
    fn a_site_that_asks_to_wait_is_given_the_time_it_asked() {
        let headers = |wait: &str| BTreeMap::from([("retry-after".to_owned(), wait.to_owned())]);
        assert_eq!(
            retry_after(429, &headers("12")),
            Some(Duration::from_secs(12))
        );
        assert_eq!(
            retry_after(503, &headers(" 3 ")),
            Some(Duration::from_secs(3))
        );
        assert_eq!(retry_after(200, &headers("12")), None, "only refusals ask");
        assert_eq!(
            retry_after(429, &headers("Wed, 21 Oct 2026 07:28:00 GMT")),
            None
        );
        assert_eq!(retry_after(429, &BTreeMap::new()), None);

        let holds = Holds::default();
        let start = Instant::now();
        assert_eq!(holds.left("api.example:443", start), None);
        holds.hold("api.example:443", Duration::from_secs(12), start);
        assert_eq!(
            holds.left("api.example:443", start + Duration::from_secs(2)),
            Some(Duration::from_secs(10))
        );
        assert_eq!(holds.left("other.example:443", start), None, "per site");
        // A shorter ask doesn't shorten the hold; a longer one is kept to the longest hold.
        holds.hold("api.example:443", Duration::from_secs(1), start);
        assert_eq!(
            holds.left("api.example:443", start),
            Some(Duration::from_secs(12))
        );
        holds.hold("api.example:443", Duration::from_secs(3_600), start);
        assert_eq!(holds.left("api.example:443", start), Some(LONGEST_HOLD));
        assert_eq!(holds.left("api.example:443", start + LONGEST_HOLD), None);

        // A call whose request was held back says how long, for whoever asks again.
        let held = crate::failure::into_error(
            Failure {
                message: "fetch failed".into(),
                trouble: Some(Trouble {
                    kind: IssueKind::RateLimited,
                    detail: held_detail("api.example", Duration::from_secs(25)),
                }),
                answered: false,
                checked: None,
            },
            "Example",
            "dev.example/stream",
            "resolve",
        );
        assert_eq!(asked_wait(&held), Some(Duration::from_secs(25)));
        let resting = crate::CallError::Resting {
            last: Box::new(held),
            failures: 3,
        };
        assert_eq!(asked_wait(&resting), Some(Duration::from_secs(25)));
        assert_eq!(asked_wait(&crate::CallError::Cancelled), None);
    }
}
